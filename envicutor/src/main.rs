use std::{
    env,
    fs::Permissions,
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use envicutor::{
    limits::{MandatoryLimits, SystemLimits},
    requests::AddRuntimeRequest,
};
use rusqlite::Connection;
use tokio::{fs, process::Command, sync::Semaphore, task};

const MAX_BOX_ID: u64 = 900;
const DEFAULT_PORT: &str = "5000";
const METADATA_FILE_NAME: &str = "metadata.txt";
const DB_PATH: &str = "/envicutor/runtimes/db.sql";

#[derive(serde::Serialize)]
struct Message {
    message: String,
}

async fn install_package(
    system_limits: SystemLimits,
    semaphore: Arc<Semaphore>,
    box_id: Arc<AtomicU64>,
    Json(req): Json<AddRuntimeRequest>,
) -> Result<impl IntoResponse, (StatusCode, impl IntoResponse)> {
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
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(Message {
                message: bad_request_message.to_string(),
            })
            .into_response(),
        ));
    }

    let current_box_id = box_id.fetch_add(1, Ordering::SeqCst) % MAX_BOX_ID;
    Command::new("isolate")
        .args(&["--init", "--cg", &format!("-b{}", current_box_id)])
        .output()
        .await
        .map_err(|e| {
            eprintln!("Could not create isolate environment: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Message {
                    message: "Internal server error".to_string(),
                }),
            )
        })?;

    let workdir = format!("/tmp/{current_box_id}");
    if let Ok(true) = fs::try_exists(&workdir).await {
        fs::remove_dir_all(&workdir).await.map_err(|e| {
            eprintln!("Failed to remove: {workdir}, error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Message {
                    message: "Internal server error".to_string(),
                }),
            )
        })?;
    }
    fs::create_dir(&workdir).await.map_err(|e| {
        eprintln!("Failed to create: {workdir}, error: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Message {
                message: "Internal server error".to_string(),
            }),
        )
    })?;

    let nix_shell_path = format!("{workdir}/box/shell.nix");
    fs::write(&nix_shell_path, &req.nix_shell)
        .await
        .map_err(|e| {
            eprintln!("Could not create nix shell: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Message {
                    message: "Internal server error".to_string(),
                }),
            )
        })?;

    let metadata_file_path = format!("{workdir}/{METADATA_FILE_NAME}");
    let limits = req
        .get_limits(&system_limits.installation)
        .map_err(|message| (StatusCode::BAD_REQUEST, Json(Message { message })))?;

    let permit = semaphore.acquire().await.map_err(|e| {
        eprintln!("Failed to acquire semaphore: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Message {
                message: "Internal server error".to_string(),
            }),
        )
    })?;

    let cmd_res = Command::new("isolate")
        .args(&[
            "--run",
            &format!("--meta={}", metadata_file_path),
            "--cg",
            "--dir=/nix/store:rw,dev",
            &format!("--dir={}", workdir),
            &format!("--cg-mem={}", limits.memory),
            &format!("--wall-time={}", limits.wall_time),
            &format!("--time={}", limits.cpu_time),
            &format!("--extra-time={}", limits.extra_time),
            &format!("--open-files={}", limits.max_open_files),
            &format!("--fsize={}", limits.max_file_size),
            &format!("--processes={}", limits.max_number_of_processes),
            &format!("-b{}", current_box_id),
            "--",
            "nix-shell",
            &format!("{}/shell.nix", workdir),
            "--run",
            "export",
        ])
        .output()
        .await
        .map_err(|e| {
            eprintln!("Failed to run isolate command: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Message {
                    message: "Internal server error".to_string(),
                }),
            )
        })?;

    if cmd_res.status.success() {
        let runtime_name = req.name;
        let source_file_name = req.source_file_name;

        let runtime_id: u32 = task::spawn_blocking(move || {
            let connection = Connection::open(DB_PATH).map_err(|e| {
                eprintln!("Failed to open SQLite connection: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(Message {
                        message: "Internal server error".to_string(),
                    }),
                )
            })?;

            connection
                .execute(
                    "INSERT INTO runtime (name, source_file_name) VALUES (?, ?)",
                    (runtime_name, source_file_name),
                )
                .map_err(|e| {
                    eprintln!("Failed to prepare statement: {e}");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(Message {
                            message: "Internal server error".to_string(),
                        }),
                    )
                })?;

            let row_id = connection
                .query_row("SELECT last_insert_rowid()", (), |row| row.get(0))
                .map_err(|e| {
                    eprintln!("Failed to get last inserted row id: {e}");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(Message {
                            message: "Internal server error".to_string(),
                        }),
                    )
                })?;

            Ok::<u32, (StatusCode, Json<Message>)>(row_id)
        })
        .await
        .map_err(|e| {
            eprintln!("Failed to spawn blocking task: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Message {
                    message: "Internal server error".to_string(),
                }),
            )
        })??;

        let runtime_dir = format!("/envicutor/runtimes/{runtime_id}");
        // Consider abstracting if more duplications of exists+remove+create arise
        if let Ok(true) = fs::try_exists(&runtime_dir).await {
            fs::remove_dir_all(&runtime_dir).await.map_err(|e| {
                eprintln!("Failed to remove: {runtime_dir}, error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(Message {
                        message: "Internal server error".to_string(),
                    }),
                )
            })?;
        }
        fs::create_dir(&runtime_dir).await.map_err(|e| {
            eprintln!("Failed to create: {runtime_dir}, error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Message {
                    message: "Internal server error".to_string(),
                }),
            )
        })?;

        if !req.compile_script.is_empty() {
            let compile_script_path = format!("{runtime_dir}/compile");
            fs::write(&compile_script_path, req.compile_script)
                .await
                .map_err(|e| {
                    eprintln!("Failed to write compile script: {e}");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(Message {
                            message: "Internal server error".to_string(),
                        }),
                    )
                })?;
            fs::set_permissions(&compile_script_path, Permissions::from_mode(0o755))
                .await
                .map_err(|e| {
                    eprintln!("Failed to set permissions on compile script: {e}");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(Message {
                            message: "Internal server error".to_string(),
                        }),
                    )
                })?;
        }

        let run_script_path = format!("{runtime_dir}/run");
        fs::write(&run_script_path, &req.run_script)
            .await
            .map_err(|e| {
                eprintln!("Failed to write run script: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(Message {
                        message: "Internal server error".to_string(),
                    }),
                )
            })?;
        fs::set_permissions(&run_script_path, Permissions::from_mode(0o755))
            .await
            .map_err(|e| {
                eprintln!("Failed to set permissions on run script: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(Message {
                        message: "Internal server error".to_string(),
                    }),
                )
            })?;

        let env_script_path = format!("{runtime_dir}/env");
        fs::write(&env_script_path, &cmd_res.stdout)
            .await
            .map_err(|e| {
                eprintln!("Failed to write env script: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(Message {
                        message: "Internal server error".to_string(),
                    }),
                )
            })?;
        fs::set_permissions(&env_script_path, Permissions::from_mode(0o755))
            .await
            .map_err(|e| {
                eprintln!("Failed to set permissions on env script: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(Message {
                        message: "Internal server error".to_string(),
                    }),
                )
            })?;

        fs::write(&(format!("{runtime_dir}/shell.nix")), &req.nix_shell)
            .await
            .map_err(|e| {
                eprintln!("Failed to write shell.nix: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(Message {
                        message: "Internal server error".to_string(),
                    }),
                )
            })?;
    }
    /*
    - metadata_cache.set(runtime_id, {name: req.name})
    */

    drop(permit);
    Ok((StatusCode::OK, ().into_response()))
}

fn get_limits_from_env_var(prefix: &str) -> MandatoryLimits {
    MandatoryLimits {
        wall_time: env::var(format!("{prefix}_WALL_TIME"))
            .expect(&format!("Missing {prefix}_WALL_TIME environment variable"))
            .parse()
            .expect(&format!("Invalid {prefix}_WALL_TIME")),
        cpu_time: env::var(format!("{prefix}_CPU_TIME"))
            .expect(&format!("Missing {prefix}_CPU_TIME environment variable"))
            .parse()
            .expect(&format!("Invalid {prefix}_CPU_TIME")),
        memory: env::var(format!("{prefix}_MEMORY"))
            .expect(&format!("Missing {prefix}_MEMORY environment variable"))
            .parse()
            .expect(&format!("Invalid {prefix}_MEMORY")),
        extra_time: env::var(format!("{prefix}_EXTRA_TIME"))
            .expect(&format!("Missing {prefix}_EXTRA_TIME environment variable"))
            .parse()
            .expect(&format!("Invalid {prefix}_EXTRA_TIME")),
        max_open_files: env::var(format!("{prefix}_MAX_OPEN_FILES"))
            .expect(&format!(
                "Missing {prefix}_MAX_OPEN_FILES environment variable"
            ))
            .parse()
            .expect(&format!("Invalid {prefix}_MAX_OPEN_FILES")),
        max_file_size: env::var(format!("{prefix}_MAX_FILE_SIZE"))
            .expect(&format!(
                "Missing {prefix}_MAX_FILE_SIZE environment variable"
            ))
            .parse()
            .expect(&format!("Invalid {prefix}_MAX_FILE_SIZE")),
        max_number_of_processes: env::var(format!("{prefix}_MAX_NUMBER_OF_PROCESSES"))
            .expect(&format!(
                "Missing {prefix}_MAX_NUMBER_OF_PROCESSES environment variable"
            ))
            .parse()
            .expect(&format!("Invalid {prefix}_MAX_NUMBER_OF_PROCESSES")),
    }
}

fn check_and_get_system_limits() -> SystemLimits {
    return SystemLimits {
        installation: get_limits_from_env_var("INSTALLATION"),
    };
}

#[tokio::main]
async fn main() {
    let system_limits = check_and_get_system_limits();
    // Currently we only allow one package installation at a time to avoid concurrency issues with Nix and SQLite
    let installation_semaphore = Arc::new(Semaphore::new(1));
    let box_id = Arc::new(AtomicU64::new(0));
    let app = Router::new().route(
        "/install",
        post({
            let system_limits = system_limits.clone();
            let installation_semaphore = installation_semaphore.clone();
            let box_id = box_id.clone();
            move |req| install_package(system_limits, installation_semaphore, box_id, req)
        }),
    );

    let port = env::var("PORT").unwrap_or_else(|_| {
        eprintln!("Could not find PORT environment variable, defaulting to 5000");
        DEFAULT_PORT.into()
    });
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind to address");
    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}
