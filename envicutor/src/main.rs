use std::{
    env,
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
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::{fs, process::Command, sync::Semaphore};

const MAX_BOX_ID: u64 = 900;
const DEFAULT_PORT: &str = "5000";
const METADATA_FILE_NAME: &str = "metadata.txt";

#[derive(serde::Serialize)]
struct Message {
    message: String,
}

async fn install_package(
    system_limits: SystemLimits,
    semaphore: Arc<Semaphore>,
    box_id: Arc<AtomicU64>,
    db_pool: Arc<Pool<Postgres>>,
    Json(req): Json<AddRuntimeRequest>,
) -> Result<impl IntoResponse, (StatusCode, impl IntoResponse)> {
    let bad_request_message = if req.name.is_empty() {
        "Name can't be empty"
    } else if req.nix_shell.is_empty() {
        "Nix shell can't be empty"
    } else if req.run_command.is_empty() {
        "Run command can't be empty"
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
    let workdir_output = Command::new("isolate")
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
        })?
        .stdout;
    let workdir_str = String::from_utf8_lossy(&workdir_output);
    let workdir = workdir_str.trim();
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
    // Currently we only allow one package installation at a time to avoid concurrency issues with Nix
    let installation_semaphore = Arc::new(Semaphore::new(1));
    let box_id = Arc::new(AtomicU64::new(0));
    let db_user = env::var("DB_USER").expect("Need a DB_USER environment variable");
    let db_password = env::var("DB_PASSWORD").expect("Need a DB_PASSWORD environment variable");
    let db_name = env::var("DB_NAME").expect("Need a DB_NAME environment variable");
    let db_host = env::var("DB_HOST").expect("Need a DB_HOST environment variable");
    let connection_string = format!("postgres://{db_user}:{db_password}@{db_host}/{db_name}");
    let db_pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(50)
            .connect(&connection_string)
            .await
            .unwrap(),
    );

    let app = Router::new().route(
        "/install",
        post({
            let system_limits = system_limits.clone();
            let installation_semaphore = installation_semaphore.clone();
            let box_id = box_id.clone();
            let db_pool = db_pool.clone();
            move |req| install_package(system_limits, installation_semaphore, box_id, db_pool, req)
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
