use std::sync::{atomic::AtomicU64, Arc};

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
    types::Metadata,
};

use super::common_functions::get_next_box_id;

#[derive(Deserialize)]
pub struct ExecutionRequest {
    runtime_id: u32,
    source_code: String,
    input: Vec<Option<String>>,
    compile_limits: Option<Limits>,
    run_limits: Option<Limits>,
}

#[derive(Serialize)]
pub struct ExecutionResponse {
    compile: Option<StageResult>,
    run: Option<Vec<StageResult>>,
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
    let runtime = metadata_guard.get(&req.runtime_id).ok_or_else(|| {
        eprintln!(
            "Failed to get runtime info, runtime of id {} does not exist in the cache",
            req.runtime_id
        );
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let mut execution_box = Isolate::init(current_box_id).await.map_err(|e| {
        eprintln!("Failed to initialize sandbox: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;
    fs::create_dir(format!("{}/submission", &execution_box.box_dir))
        .await
        .map_err(|e| {
            eprintln!("Failed to create submission directory: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;

    req.source_code.add_new_line_if_none();
    fs::write(
        format!(
            "{}/submission/{}",
            execution_box.box_dir, runtime.source_file_name
        ),
        &req.source_code,
    )
    .await
    .map_err(|e| {
        eprintln!(
            "Failed to write the source code in {}: {}",
            execution_box.box_dir, e
        );
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let _permit = semaphore.acquire().await.map_err(|e| {
        eprintln!("Failed to acquire execution semaphore: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;
    let runtime_dir = format!("{}/{}", RUNTIMES_DIR, req.runtime_id);
    let mounts = ["/nix", &format!("/runtime={runtime_dir}")];

    let compile_result = if runtime.is_compiled {
        let res = execution_box
            .run(
                &mounts,
                &compile_limits,
                None,
                "/box/submission",
                &format!("{runtime_dir}/env"),
                &["/runtime/compile"],
            )
            .await
            .map_err(|e| {
                eprintln!("Failed to compile submission: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;

        if res.exit_code == Some(0) {
            let run_box = Isolate::init(get_next_box_id(&box_id)).await.map_err(|e| {
                eprintln!("Failed to initialize run sandbox: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
            fs::rename(
                format!("{}/submission", &execution_box.box_dir),
                format!("{}/submission", &run_box.box_dir),
            )
            .await
            .map_err(|e| {
                eprintln!(
                    "Failed to move {} to {}: {}",
                    execution_box.box_dir, run_box.box_dir, e
                );
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
            execution_box = run_box;
        } else {
            return Ok(Json(ExecutionResponse {
                compile: Some(res),
                run: None,
            })
            .into_response());
        }
        Some(res)
    } else {
        None
    };

    let mut run_results = Vec::new();
    if req.input.is_empty() {
        req.input.push(None);
    }

    let initial_state = execution_box;
    // TODO: don't do copying stuff if only one test case
    for input in req.input {
        let stdin = if let Some(mut s) = input {
            s.add_new_line_if_none();
            Some(s)
        } else {
            None
        };
        let mut new_run_box = Isolate::init(get_next_box_id(&box_id)).await.map_err(|e| {
            eprintln!("Failed to initialize run sandbox: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;

        let src = format!("{}/submission", initial_state.box_dir);
        let dest = format!("{}/submission", new_run_box.box_dir);
        crate::fs::copy_dir_all(&src, &dest).await.map_err(|e| {
            eprintln!("Failed to copy submission directory from {src} to {dest}: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;

        let run_result = new_run_box
            .run(
                &mounts,
                &run_limits,
                stdin.as_deref(),
                "/box/submission",
                &format!("{runtime_dir}/env"),
                &["/runtime/run"],
            )
            .await
            .map_err(|e| {
                eprintln!("Failed to run submission: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
        run_results.push(run_result);
    }

    Ok(Json(ExecutionResponse {
        compile: compile_result,
        run: Some(run_results),
    })
    .into_response())
}
