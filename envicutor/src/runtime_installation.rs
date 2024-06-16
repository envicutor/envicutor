use std::{
    collections::HashMap,
    fs::Permissions,
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use crate::{globals::DB_PATH, temp_dir::TempDir, transaction::Transaction, units::WholeSeconds};
use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    process::Command,
    sync::{RwLock, Semaphore},
    task,
};

const MAX_BOX_ID: u64 = 2147483647;

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
    name: String,
    nix_shell: String,
    compile_script: String,
    run_script: String,
    source_file_name: String,
}

#[derive(Serialize)]
pub struct InstallationResponse {
    stdout: String,
    stderr: String,
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

const NIX_BIN_PATH: &str = "/home/envicutor/.nix-profile/bin";

const INTERNAL_SERVER_ERROR_RESPONSE: (StatusCode, Json<StaticMessage>) = (
    StatusCode::INTERNAL_SERVER_ERROR,
    Json(StaticMessage {
        message: "Internal server error",
    }),
);

pub async fn install_runtime(
    installation_timeout: WholeSeconds,
    semaphore: Arc<Semaphore>,
    box_id: Arc<AtomicU64>,
    metadata_cache: Arc<RwLock<HashMap<u32, String>>>,
    Json(req): Json<AddRuntimeRequest>,
) -> Result<Response<Body>, Response<Body>> {
    validate_request(&req)?;

    // TODO: abstract modulo logic
    let current_box_id = box_id.fetch_add(1, Ordering::SeqCst) % MAX_BOX_ID;

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

    let _permit = semaphore.acquire().await.map_err(|e| {
        eprintln!("Failed to acquire semaphore: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let mut cmd = Command::new("env");
    cmd.arg("-i")
        .arg("PATH=/bin")
        .arg(format!("{NIX_BIN_PATH}/nix-shell"))
        .args(["--timeout".to_string(), installation_timeout.to_string()])
        .arg(nix_shell_path)
        .args(["--run", "/bin/bash -c export"]);
    let cmd_res = cmd.output().await.map_err(|e| {
        eprintln!("Failed to run nix-shell: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;
    let stdout = String::from_utf8_lossy(&cmd_res.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&cmd_res.stderr).trim().to_string();

    if cmd_res.status.success() {
        let runtime_name = req.name.clone();
        let source_file_name = req.source_file_name;

        let (runtime_id, mut trx) = task::spawn_blocking(move || {
            let connection = Connection::open(DB_PATH).map_err(|e| {
                eprintln!("Failed to open SQLite connection: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;

            connection
                .execute(
                    "INSERT INTO runtime (name, source_file_name) VALUES (?, ?)",
                    (&runtime_name, source_file_name),
                )
                .map_err(|e| {
                    eprintln!("Failed to execute statement: {e}");
                    INTERNAL_SERVER_ERROR_RESPONSE.into_response()
                })?;

            let trx = Transaction::init(
                move |conn| {
                    let res = conn.execute("DELETE FROM runtime WHERE name = ?", [&runtime_name]);
                    if let Err(e) = res {
                        eprintln!(
                            "Failed to remove runtime with name: {runtime_name} during rollback\nError: {e}"
                        );
                    }
                },
            );

            let row_id = connection
                .query_row("SELECT last_insert_rowid()", (), |row| row.get(0))
                .map_err(|e| {
                    eprintln!("Failed to get last inserted row id: {e}");
                    INTERNAL_SERVER_ERROR_RESPONSE.into_response()
                })?;

            Ok((row_id, trx))
        })
        .await
        .map_err(|e| {
            eprintln!("Failed to spawn blocking task: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })??;

        let runtime_dir = format!("/envicutor/runtimes/{runtime_id}");
        crate::fs::create_dir_replacing_existing(&runtime_dir)
            .await
            .map_err(|e| {
                eprintln!("Failed to create: {runtime_dir}, error: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;

        if !req.compile_script.is_empty() {
            let compile_script_path = format!("{runtime_dir}/compile");
            crate::fs::write_file_and_set_permissions(
                &compile_script_path,
                &req.compile_script,
                Permissions::from_mode(0o755),
            )
            .await
            .map_err(|e| {
                eprintln!("Failed to write compile script: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
        }

        let run_script_path = format!("{runtime_dir}/run");
        crate::fs::write_file_and_set_permissions(
            &run_script_path,
            &req.run_script,
            Permissions::from_mode(0o755),
        )
        .await
        .map_err(|e| {
            eprintln!("Failed to write run script: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;

        let env_script_path = format!("{runtime_dir}/env");
        crate::fs::write_file_and_set_permissions(
            &env_script_path,
            &stdout,
            Permissions::from_mode(0o755),
        )
        .await
        .map_err(|e| {
            eprintln!("Failed to write env script: {e}");
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
        trx.commit();
    }

    Ok((
        StatusCode::OK,
        Json(InstallationResponse { stdout, stderr }),
    )
        .into_response())
}

pub async fn update_nix(
    nix_update_timeout: WholeSeconds,
    semaphore: Arc<Semaphore>,
) -> Result<Response<Body>, Response<Body>> {
    let mut cmd = Command::new(format!("{NIX_BIN_PATH}/nix-env"));
    cmd.arg("--install")
        .args(["--file", "<nixpkgs>"])
        .args(["--attr", "nix", "cacert"])
        .args(["-I", "nixpkgs=channel:nixpkgs-unstable"])
        .args(["--timeout".to_string(), nix_update_timeout.to_string()]);

    let _permit = semaphore.acquire().await.map_err(|e| {
        eprintln!("Failed to acquire installation semaphore: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    let cmd_res = cmd.output().await.map_err(|e| {
        eprintln!("Failed to get the output of the nix update command: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })?;

    Ok((
        StatusCode::OK,
        Json(InstallationResponse {
            stdout: String::from_utf8_lossy(&cmd_res.stdout).to_string(),
            stderr: String::from_utf8_lossy(&cmd_res.stderr).to_string(),
        }),
    )
        .into_response())
}
