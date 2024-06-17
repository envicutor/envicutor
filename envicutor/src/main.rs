use std::{
    collections::HashMap,
    env,
    sync::{atomic::AtomicU64, Arc},
};

use axum::{
    body::Body,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use envicutor::{
    api::{
        installation::{install_runtime, update_nix},
        listing::list_runtimes,
    },
    globals::DB_PATH,
    limits::{MandatoryLimits, SystemLimits},
    types::{Metadata, WholeSeconds},
};
use rusqlite::Connection;
use tokio::{
    signal::{self, unix::SignalKind},
    sync::{RwLock, Semaphore},
};

const DEFAULT_PORT: &str = "5000";

fn get_limits_from_env_var(prefix: &str) -> MandatoryLimits {
    MandatoryLimits {
        wall_time: env::var(format!("{prefix}_WALL_TIME"))
            .unwrap_or_else(|_| panic!("Missing {prefix}_WALL_TIME environment variable"))
            .parse()
            .unwrap_or_else(|_| panic!("Invalid {prefix}_WALL_TIME")),
        cpu_time: env::var(format!("{prefix}_CPU_TIME"))
            .unwrap_or_else(|_| panic!("Missing {prefix}_CPU_TIME environment variable"))
            .parse()
            .unwrap_or_else(|_| panic!("Invalid {prefix}_CPU_TIME")),
        memory: env::var(format!("{prefix}_MEMORY"))
            .unwrap_or_else(|_| panic!("Missing {prefix}_MEMORY environment variable"))
            .parse()
            .unwrap_or_else(|_| panic!("Invalid {prefix}_MEMORY")),
        extra_time: env::var(format!("{prefix}_EXTRA_TIME"))
            .unwrap_or_else(|_| panic!("Missing {prefix}_EXTRA_TIME environment variable"))
            .parse()
            .unwrap_or_else(|_| panic!("Invalid {prefix}_EXTRA_TIME")),
        max_open_files: env::var(format!("{prefix}_MAX_OPEN_FILES"))
            .unwrap_or_else(|_| panic!("Missing {prefix}_MAX_OPEN_FILES environment variable"))
            .parse()
            .unwrap_or_else(|_| panic!("Invalid {prefix}_MAX_OPEN_FILES")),
        max_file_size: env::var(format!("{prefix}_MAX_FILE_SIZE"))
            .unwrap_or_else(|_| panic!("Missing {prefix}_MAX_FILE_SIZE environment variable"))
            .parse()
            .unwrap_or_else(|_| panic!("Invalid {prefix}_MAX_FILE_SIZE")),
        max_number_of_processes: env::var(format!("{prefix}_MAX_NUMBER_OF_PROCESSES"))
            .unwrap_or_else(|_| {
                panic!("Missing {prefix}_MAX_NUMBER_OF_PROCESSES environment variable")
            })
            .parse()
            .unwrap_or_else(|_| panic!("Invalid {prefix}_MAX_NUMBER_OF_PROCESSES")),
    }
}

#[allow(dead_code)]
fn check_and_get_system_limits() -> SystemLimits {
    SystemLimits {
        installation: get_limits_from_env_var("INSTALLATION"),
    }
}

fn get_whole_seconds_or_set_default(env_var: &str, default: WholeSeconds) -> WholeSeconds {
    env::var(env_var)
        .unwrap_or_else(|_| {
            eprintln!(
                "Could not find {env_var} environment variable, defaulting to {default} seconds"
            );
            default.to_string()
        })
        .parse()
        .unwrap_or_else(|_| {
            panic!("Invalid {env_var}");
        })
}

async fn get_health() -> Response<Body> {
    "Up and running\n".into_response()
}

fn get_runtimes() -> Metadata {
    let connection = Connection::open(DB_PATH)
        .unwrap_or_else(|e| panic!("Failed to open SQLite connection: {e}"));
    let mut stmt = connection
        .prepare("SELECT id, name FROM runtime")
        .unwrap_or_else(|e| panic!("Failed to prepare SQL statement: {}", e));
    let mut metadata_cache = HashMap::new();
    let runtime_iter = stmt
        .query_map([], |row| {
            let id: u32 = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((id, name))
        })
        .unwrap_or_else(|e| {
            panic!("Failed to get id and name from the row: {e}");
        });

    for runtime in runtime_iter {
        let (id, name) = runtime.unwrap_or_else(|e| {
            panic!("Failed to get runtime from database: {e}");
        });
        eprintln!("Loading {id}: {name}");
        metadata_cache.insert(id, name);
    }
    metadata_cache
}

#[tokio::main]
async fn main() {
    let installation_timeout = get_whole_seconds_or_set_default("INSTALLATION_TIMEOUT", 120);
    let update_timeout = get_whole_seconds_or_set_default("UPDATE_TIMEOUT", 240);

    // Currently we only allow one runtime installation at a time to avoid concurrency issues with Nix and SQLite
    let installation_semaphore = Arc::new(Semaphore::new(1));
    let box_id = Arc::new(AtomicU64::new(0));
    let metadata_cache = Arc::new(RwLock::new(get_runtimes()));
    let app = Router::new()
        .route("/health", get(get_health))
        .route(
            "/runtimes",
            get({
                let metadata_cache = metadata_cache.clone();
                move || list_runtimes(metadata_cache)
            }),
        )
        .route(
            "/runtimes",
            post({
                let installation_semaphore = installation_semaphore.clone();
                let box_id = box_id.clone();
                let metadata_cache = metadata_cache.clone();
                move |req| {
                    install_runtime(
                        installation_timeout,
                        installation_semaphore,
                        box_id,
                        metadata_cache,
                        req,
                    )
                }
            }),
        )
        .route(
            "/update",
            post({
                let installation_semaphore = installation_semaphore.clone();
                move || update_nix(update_timeout, installation_semaphore)
            }),
        );

    let port = env::var("PORT").unwrap_or_else(|_| {
        eprintln!("Could not find PORT environment variable, defaulting to 5000");
        DEFAULT_PORT.into()
    });

    let signal = async {
        signal::unix::signal(SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
        eprintln!("Received SIGTERM, shutting down...");
    };

    eprintln!("Listening on port {port}");
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind to address");
    axum::serve(listener, app)
        .with_graceful_shutdown(signal)
        .await
        .expect("Failed to start server");
}
