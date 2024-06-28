use std::sync::{atomic::AtomicU64, Arc};

use anyhow::{anyhow, Error};
use axum::{
    body::Body,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::{prelude::BASE64_STANDARD, Engine};
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    sync::{RwLock, Semaphore},
    task,
};

use crate::{
    api::common_functions::get_next_box_id,
    api::common_responses::{Message, INTERNAL_SERVER_ERROR_RESPONSE},
    globals::RUNTIMES_DIR,
    isolate::{Isolate, StageResult},
    limits::{GetLimits, Limits, SystemLimits},
    strings::NewLine,
    types::Metadata,
};

const SOURCE_ZIP_NAME: &str = "source.zip";

#[derive(Deserialize)]
pub struct ExecutionQuery {
    is_project: bool,
}

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
    extract: Option<StageResult>,
    compile: Option<StageResult>,
    run: Option<StageResult>,
}

pub async fn renew_box(box_id: &Arc<AtomicU64>, execution_box: &mut Isolate) -> Result<(), Error> {
    let new_box = Isolate::init(get_next_box_id(box_id))
        .await
        .map_err(|e| anyhow!("Failed to initialize run sandbox: {e}"))?;
    fs::rename(
        format!("{}/submission", &execution_box.box_dir),
        format!("{}/submission", &new_box.box_dir),
    )
    .await
    .map_err(|e| {
        anyhow!(
            "Failed to move {} to {}: {}",
            execution_box.box_dir,
            new_box.box_dir,
            e
        )
    })?;
    *execution_box = new_box;
    Ok(())
}

pub async fn execute(
    semaphore: Arc<Semaphore>,
    box_id: Arc<AtomicU64>,
    metadata_cache: Arc<RwLock<Metadata>>,
    installation_lock: Arc<RwLock<u8>>,
    system_limits: SystemLimits,
    Json(mut req): Json<ExecutionRequest>,
    query: Option<Query<ExecutionQuery>>,
) -> Result<Response<Body>, Response<Body>> {
    let _installation_guard = installation_lock.read().await;
    let _permit = semaphore.acquire().await.map_err(|e| {
        eprintln!("Failed to acquire execution semaphore: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;
    let is_project = if let Some(query) = query {
        query.is_project
    } else {
        false
    };
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
    let runtime = metadata_guard.get(&req.runtime_id).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(Message {
                message: format!("Runtime with id: {} does not exist", req.runtime_id),
            }),
        )
            .into_response()
    })?;

    let current_box_id = get_next_box_id(&box_id);
    let mut execution_box = Isolate::init(current_box_id).await.map_err(|e| {
        eprintln!("Failed to initialize sandbox: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let initial_submission_dir = format!("{}/submission", execution_box.box_dir);
    fs::create_dir(&initial_submission_dir).await.map_err(|e| {
        eprintln!("Failed to create submission directory: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    req.source_code.add_new_line_if_none();

    if is_project {
        let (req_ret, decoded_res) = task::spawn_blocking(move || {
            let decoded = BASE64_STANDARD.decode(&req.source_code);
            (req, decoded)
        })
        .await
        .map_err(|e| {
            eprintln!("Failed to spawn blocking decoding task: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;
        // Errors returned from decoding should be safe to show in response
        let decoded = decoded_res.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(Message {
                    message: e.to_string(),
                }),
            )
                .into_response()
        })?;
        req = req_ret;
        fs::write(
            format!("{}/{}", initial_submission_dir, SOURCE_ZIP_NAME),
            &decoded,
        )
        .await
    } else {
        fs::write(
            format!("{}/{}", initial_submission_dir, runtime.source_file_name),
            &req.source_code,
        )
        .await
    }
    .map_err(|e| {
        eprintln!(
            "Failed to write the source code in {}: {}",
            execution_box.box_dir, e
        );
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let extraction_result = if is_project {
        let res = execution_box
            .run(
                &[],
                &compile_limits,
                None,
                "/box/submission",
                None,
                &["unzip", "-qq", SOURCE_ZIP_NAME],
            )
            .await
            .map_err(|e| {
                eprintln!("Failed to run isolate to unzip the source file: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
        if res.exit_code != Some(0) {
            return Ok(Json(ExecutionResponse {
                extract: Some(res),
                compile: None,
                run: None,
            })
            .into_response());
        }
        renew_box(&box_id, &mut execution_box).await.map_err(|e| {
            eprintln!("Failed to renew box after extraction: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;
        Some(res)
    } else {
        None
    };

    let runtime_dir = format!("{}/{}", RUNTIMES_DIR, req.runtime_id);
    let mounts = ["/nix", &format!("/runtime={runtime_dir}")];

    let compile_result = if runtime.is_compiled {
        let res = execution_box
            .run(
                &mounts,
                &compile_limits,
                None,
                "/box/submission",
                Some(&format!("{runtime_dir}/env")),
                &["/runtime/compile"],
            )
            .await
            .map_err(|e| {
                eprintln!("Failed to compile submission: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;

        if res.exit_code == Some(0) {
            renew_box(&box_id, &mut execution_box).await.map_err(|e| {
                eprintln!("Failed to renew box: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
        } else {
            return Ok(Json(ExecutionResponse {
                extract: extraction_result,
                compile: Some(res),
                run: None,
            })
            .into_response());
        }
        Some(res)
    } else {
        None
    };

    let stdin = if let Some(mut s) = req.input {
        s.add_new_line_if_none();
        Some(s)
    } else {
        None
    };

    let run_result = Some(
        execution_box
            .run(
                &mounts,
                &run_limits,
                stdin.as_deref(),
                "/box/submission",
                Some(&format!("{runtime_dir}/env")),
                &["/runtime/run"],
            )
            .await
            .map_err(|e| {
                eprintln!("Failed to run submission: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?,
    );

    Ok(Json(ExecutionResponse {
        extract: extraction_result,
        compile: compile_result,
        run: run_result,
    })
    .into_response())
}
