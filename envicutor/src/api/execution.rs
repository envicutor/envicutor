use std::{
    fs::Permissions,
    os::unix::fs::PermissionsExt,
    sync::{atomic::AtomicU64, Arc},
};

use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    sync::{RwLock, Semaphore},
};

use crate::{
    api::common_responses::{Message, INTERNAL_SERVER_ERROR_RESPONSE},
    globals::RUNTIMES_DIR,
    isolate::{Isolate, StageResult},
    limits::{GetLimits, Limits, SystemLimits},
    strings::NewLine,
    temp_box::TempBox,
    types::Metadata,
};

use super::common_functions::get_next_box_id;

#[derive(Deserialize)]
pub struct ExecutionRequest {
    runtime_id: u32,
    source_code: String,
    input: Option<String>,
    compile_limits: Option<Limits>,
    run_limits: Option<Limits>,
}

#[derive(Serialize)]
pub struct ExecutionResponse {
    compile: Option<StageResult>,
    run: Option<StageResult>,
}

pub async fn execute(
    semaphore: Arc<Semaphore>,
    box_id: Arc<AtomicU64>,
    metadata_cache: Arc<RwLock<Metadata>>,
    system_limits: SystemLimits,
    Json(mut req): Json<ExecutionRequest>,
) -> Result<Response<Body>, Response<Body>> {
    let compile_limits = req
        .compile_limits
        .get(&system_limits.compile)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(Message {
                    message: format!("Invalid compile limits: {e}"),
                }),
            )
                .into_response()
        })?;
    let run_limits = req.run_limits.get(&system_limits.run).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(Message {
                message: format!("Invalid run limits: {e}"),
            }),
        )
            .into_response()
    })?;
    let metadata_guard = metadata_cache.read().await;
    if !metadata_guard.contains_key(&req.runtime_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(Message {
                message: format!("Runtime with id: {} does not exist", req.runtime_id),
            }),
        )
            .into_response());
    }

    let current_box_id = get_next_box_id(&box_id);
    let workdir = TempBox::new(current_box_id).await.map_err(|e| {
        eprintln!("Failed to create temp directory (workdir): {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;
    fs::set_permissions(&workdir.path, Permissions::from_mode(0o777))
        .await
        .map_err(|e| {
            eprintln!(
                "Failed to set permissions on: {}\nError: {}",
                workdir.path, e
            );
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;

    let runtime = metadata_guard.get(&req.runtime_id).ok_or_else(|| {
        eprintln!(
            "Failed to get runtime info, runtime of id {} does not exist in the cache",
            req.runtime_id
        );
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    req.source_code.add_new_line_if_none();
    fs::write(
        format!("{}/{}", workdir.path, runtime.source_file_name),
        &req.source_code,
    )
    .await
    .map_err(|e| {
        eprintln!(
            "Failed to write the source code in {}\nError: {}",
            workdir.path, e
        );
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let _permit = semaphore.acquire().await.map_err(|e| {
        eprintln!("Failed to acquire execution semaphore: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;
    let runtime_dir = format!("{}/{}", RUNTIMES_DIR, req.runtime_id);
    let mounts = [
        &format!("/submission={}:rw", workdir.path),
        "/nix",
        &format!("/runtime={runtime_dir}"),
    ];

    let compile_result = if runtime.is_compiled {
        let mut compile_sandbox = Isolate::init(current_box_id).await.map_err(|e| {
            eprintln!("Failed to initialize compile sandbox: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;
        Some(
            compile_sandbox
                .run(
                    &mounts,
                    &compile_limits,
                    None,
                    "/submission",
                    &format!("{runtime_dir}/env"),
                    &["/runtime/compile"],
                )
                .await
                .map_err(|e| {
                    eprintln!("Failed to compile submission: {e}");
                    INTERNAL_SERVER_ERROR_RESPONSE.into_response()
                })?,
        )
    } else {
        None
    };

    // If there is a compile_result and its exit_code is 0 or there isn't a compile_result, run
    let should_run = if let Some(cs) = &compile_result {
        cs.exit_code == Some(0)
    } else {
        true
    };

    let run_result = if should_run {
        let next_box_id = get_next_box_id(&box_id);
        let mut run_sandbox = Isolate::init(next_box_id).await.map_err(|e| {
            eprintln!("Failed to initialize run sandbox: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;
        let stdin = if let Some(mut s) = req.input {
            s.add_new_line_if_none();
            Some(s)
        } else {
            None
        };

        Some(
            run_sandbox
                .run(
                    &mounts,
                    &run_limits,
                    stdin.as_deref(),
                    "/submission",
                    &format!("{runtime_dir}/env"),
                    &["/runtime/run"],
                )
                .await
                .map_err(|e| {
                    eprintln!("Failed to run submission: {e}");
                    INTERNAL_SERVER_ERROR_RESPONSE.into_response()
                })?,
        )
    } else {
        None
    };

    Ok(Json(ExecutionResponse {
        compile: compile_result,
        run: run_result,
    })
    .into_response())
}
