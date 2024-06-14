use std::{
    collections::HashMap,
    fs::Permissions,
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use crate::{
    isolate::Isolate,
    limits::{GetLimits, Limits, SystemLimits},
    temp_dir::TempDir,
};
use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rusqlite::Connection;
use serde::Deserialize;
use tokio::{
    fs,
    sync::{RwLock, Semaphore},
    task,
};

const MAX_BOX_ID: u64 = 2147483647;
const DB_PATH: &str = "/envicutor/runtimes/db.sql";

#[derive(serde::Serialize)]
struct Message {
    message: String,
}

#[derive(serde::Serialize)]
pub struct StaticMessage {
    message: &'static str,
}

#[derive(Deserialize)]
pub struct AddRuntimeRequest {
    pub name: String,
    pub description: String,
    pub nix_shell: String,
    pub compile_script: String,
    pub run_script: String,
    pub source_file_name: String,
    limits: Option<Limits>,
}

fn validate_request(req: &AddRuntimeRequest) -> Result<(), Response<Body>> {
    let bad_request_message = if req.name.is_empty() {
        "Name can't be empty"
    } else if req.nix_shell.is_empty() {
        "Nix shell can't be empty"
    } else if req.run_script.is_empty() {
        "Run command can't be empty"
    } else if req.source_file_name.is_empty() {
        "Source file name can't be empty"
    } else {
        ""
    };
    if !bad_request_message.is_empty() {
        Err((
            StatusCode::BAD_REQUEST,
            Json(Message {
                message: bad_request_message.to_string(),
            }),
        )
            .into_response())
    } else {
        Ok(())
    }
}

const INTERNAL_SERVER_ERROR_RESPONSE: (StatusCode, Json<StaticMessage>) = (
    StatusCode::INTERNAL_SERVER_ERROR,
    Json(StaticMessage {
        message: "Internal server error",
    }),
);

pub async fn install_runtime(
    system_limits: SystemLimits,
    semaphore: Arc<Semaphore>,
    box_id: Arc<AtomicU64>,
    metadata_cache: Arc<RwLock<HashMap<u32, String>>>,
    Json(req): Json<AddRuntimeRequest>,
) -> Result<Response<Body>, Response<Body>> {
    validate_request(&req)?;

    let current_box_id = box_id.fetch_add(1, Ordering::SeqCst) % MAX_BOX_ID;
    let isolate = Isolate::init(current_box_id).await.map_err(|e| {
        eprintln!("Failed to initialize isolate: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let workdir = TempDir::new(format!("/tmp/{current_box_id}"))
        .await
        .map_err(|e| {
            eprintln!("Failed to create workdir: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;

    let nix_shell_path = format!("{}/shell.nix", workdir.path);
    fs::write(&nix_shell_path, &req.nix_shell)
        .await
        .map_err(|e| {
            eprintln!("Could not write nix shell file: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;

    let limits = req
        .limits
        .get(&system_limits.installation)
        .map_err(|message| (StatusCode::BAD_REQUEST, Json(Message { message })).into_response())?;

    let permit = semaphore.acquire().await.map_err(|e| {
        eprintln!("Failed to acquire semaphore: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let mounts = ["/nix/store:rw,dev", workdir.path.as_str()];
    let args = [
        "nix-shell".to_string(),
        format!("{}/shell.nix", workdir.path),
        "--run".to_string(),
        "export".to_string(),
    ];
    let result = isolate.run(&mounts, &limits, &args).await.map_err(|e| {
        eprintln!("Failed to run isolate: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    if result.exit_code == Some(0) {
        let runtime_name = req.name.clone();
        let source_file_name = req.source_file_name;

        let runtime_id: u32 = task::spawn_blocking(move || {
            let connection = Connection::open(DB_PATH).map_err(|e| {
                eprintln!("Failed to open SQLite connection: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;

            connection
                .execute(
                    "INSERT INTO runtime (name, source_file_name) VALUES (?, ?)",
                    (runtime_name, source_file_name),
                )
                .map_err(|e| {
                    eprintln!("Failed to execute statement: {e}");
                    INTERNAL_SERVER_ERROR_RESPONSE.into_response()
                })?;

            let row_id = connection
                .query_row("SELECT last_insert_rowid()", (), |row| row.get(0))
                .map_err(|e| {
                    eprintln!("Failed to get last inserted row id: {e}");
                    INTERNAL_SERVER_ERROR_RESPONSE.into_response()
                })?;

            Ok::<u32, Response<Body>>(row_id)
        })
        .await
        .map_err(|e| {
            eprintln!("Failed to spawn blocking task: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })??;

        let runtime_dir = format!("/envicutor/runtimes/{runtime_id}");
        // Consider abstracting if more duplications of exists+remove+create arise
        if let Ok(true) = fs::try_exists(&runtime_dir).await {
            fs::remove_dir_all(&runtime_dir).await.map_err(|e| {
                eprintln!("Failed to remove: {runtime_dir}, error: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
        }
        fs::create_dir(&runtime_dir).await.map_err(|e| {
            eprintln!("Failed to create: {runtime_dir}, error: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;

        if !req.compile_script.is_empty() {
            let compile_script_path = format!("{runtime_dir}/compile");
            fs::write(&compile_script_path, req.compile_script)
                .await
                .map_err(|e| {
                    eprintln!("Failed to write compile script: {e}");
                    INTERNAL_SERVER_ERROR_RESPONSE.into_response()
                })?;
            fs::set_permissions(&compile_script_path, Permissions::from_mode(0o755))
                .await
                .map_err(|e| {
                    eprintln!("Failed to set permissions on compile script: {e}");
                    INTERNAL_SERVER_ERROR_RESPONSE.into_response()
                })?;
        }

        let run_script_path = format!("{runtime_dir}/run");
        fs::write(&run_script_path, &req.run_script)
            .await
            .map_err(|e| {
                eprintln!("Failed to write run script: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
        fs::set_permissions(&run_script_path, Permissions::from_mode(0o755))
            .await
            .map_err(|e| {
                eprintln!("Failed to set permissions on run script: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;

        let env_script_path = format!("{runtime_dir}/env");
        fs::write(&env_script_path, &result.stdout)
            .await
            .map_err(|e| {
                eprintln!("Failed to write env script: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
        fs::set_permissions(&env_script_path, Permissions::from_mode(0o755))
            .await
            .map_err(|e| {
                eprintln!("Failed to set permissions on env script: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;

        fs::write(&(format!("{runtime_dir}/shell.nix")), &req.nix_shell)
            .await
            .map_err(|e| {
                eprintln!("Failed to write shell.nix: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;

        let mut metadata_guard = metadata_cache.write().await;
        metadata_guard.insert(runtime_id, req.name);
        drop(metadata_guard);
    }

    drop(permit);

    Ok((StatusCode::OK, Json(result)).into_response())
}
