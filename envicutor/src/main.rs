use std::{
    env, io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::{process::Command, sync::Semaphore};

const MAX_BOX_ID: u64 = 900;
const DEFAULT_PORT: &str = "5000";
const METADATA_FILE_NAME: &str = "metadata.txt";

#[derive(serde::Serialize)]
struct Message<'a> {
    message: &'a str,
}

#[derive(Deserialize)]
struct AddRuntimeRequest {
    name: String,
    description: String,
    nix_shell: String,
    compile_command: String,
    run_command: String,
}

enum InternalError {
    IsolateEnvironmentCreation(io::Error),
}

impl IntoResponse for InternalError {
    fn into_response(self) -> Response {
        match self {
            InternalError::IsolateEnvironmentCreation(e) => {
                eprintln!("Failed to create isolate environment: {e}");
            }
        };
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Message {
                message: "An internal error has occurred",
            }),
        )
            .into_response();
    }
}

async fn install_package(
    semaphore: Arc<Semaphore>,
    box_id: Arc<AtomicU64>,
    db_pool: Arc<Pool<Postgres>>,
    Json(req): Json<AddRuntimeRequest>,
) -> Result<impl IntoResponse, InternalError> {
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
                message: bad_request_message,
            })
            .into_response(),
        ));
    }

    let current_box_id = box_id.fetch_add(1, Ordering::SeqCst) % MAX_BOX_ID;
    let workdir_output = Command::new("isolate")
        .args(&["--init", "--cg", &format!("-b{}", current_box_id)])
        .output()
        .await
        .map_err(|e| InternalError::IsolateEnvironmentCreation(e))?
        .stdout;
    let workdir_str = String::from_utf8_lossy(&workdir_output);
    let workdir = workdir_str.trim();
    let metadata_file_path = format!("{workdir}/{METADATA_FILE_NAME}");
    let nix_shell_path = format!("{workdir}/box/shell.nix");
    Ok((StatusCode::OK, ().into_response()))
}

#[tokio::main]
async fn main() {
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
            let installation_semaphore = installation_semaphore.clone();
            let box_id = box_id.clone();
            let db_pool = db_pool.clone();
            move |req| install_package(installation_semaphore, box_id, db_pool, req)
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
