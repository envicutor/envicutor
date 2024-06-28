use std::{
    collections::HashMap,
    env,
    path::Path,
    str::FromStr,
    sync::{atomic::AtomicU64, Arc},
};

use axum::{
    body::Body,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Router,
};
use envicutor::{
    api::{
        deletion::delete_runtime,
        execution::execute,
        installation::{install_runtime, update_nix},
        listing::list_runtimes,
    },
    globals::{DB_PATH, RUNTIMES_DIR},
    limits::{MandatoryLimits, SystemLimits},
    types::{Metadata, Runtime, WholeSeconds},
};
use rusqlite::Connection;
use tokio::{
    signal::{self, unix::SignalKind},
    sync::{RwLock, Semaphore},
};

const DEFAULT_PORT: &str = "5000";

fn get_mandatory_parsed_env_var<T>(var_name: &str) -> T
where
    T: FromStr,
{
    env::var(var_name)
        .unwrap_or_else(|_| panic!("Missing {var_name} environment variable"))
        .parse()
        .unwrap_or_else(|_| {
            panic!("Invalid {var_name} environment variable");
        })
}

fn get_limits_from_env_var(prefix: &str) -> MandatoryLimits {
    MandatoryLimits {
        wall_time: get_mandatory_parsed_env_var(&format!("{prefix}_WALL_TIME")),
        cpu_time: get_mandatory_parsed_env_var(&format!("{prefix}_CPU_TIME")),
        memory: get_mandatory_parsed_env_var(&format!("{prefix}_MEMORY")),
        extra_time: get_mandatory_parsed_env_var(&format!("{prefix}_EXTRA_TIME")),
        max_open_files: get_mandatory_parsed_env_var(&format!("{prefix}_MAX_OPEN_FILES")),
        max_file_size: get_mandatory_parsed_env_var(&format!("{prefix}_MAX_FILE_SIZE")),
        max_number_of_processes: get_mandatory_parsed_env_var(&format!(
            "{prefix}_MAX_NUMBER_OF_PROCESSES"
        )),
    }
}

fn check_and_get_system_limits() -> SystemLimits {
    SystemLimits {
        compile: get_limits_from_env_var("COMPILE"),
        run: get_limits_from_env_var("RUN"),
    }
}

async fn get_health() -> Response<Body> {
    "Up and running\n".into_response()
}

fn get_runtimes() -> Metadata {
    let connection = Connection::open(DB_PATH)
        .unwrap_or_else(|e| panic!("Failed to open SQLite connection: {e}"));
    let mut stmt = connection
        .prepare("SELECT id, name, source_file_name FROM runtime")
        .unwrap_or_else(|e| panic!("Failed to prepare SQL statement: {}", e));
    let mut metadata_cache = HashMap::new();
    let runtime_iter = stmt
        .query_map([], |row| {
            let id: u32 = row.get(0)?;
            let name: String = row.get(1)?;
            let source_file_name: String = row.get(2)?;
            Ok((id, name, source_file_name))
        })
        .unwrap_or_else(|e| {
            panic!("Failed to get id and name from the row: {e}");
        });

    for runtime in runtime_iter {
        let (id, name, source_file_name) = runtime.unwrap_or_else(|e| {
            panic!("Failed to get runtime from database: {e}");
        });
        eprintln!("Loading {id}: {name}");
        metadata_cache.insert(
            id,
            Runtime {
                name,
                source_file_name,
                is_compiled: Path::new(&format!("{RUNTIMES_DIR}/{id}/compile"))
                    .try_exists()
                    .unwrap_or_else(|e| {
                        panic!("Could not check if compile script exists: {e}");
                    }),
            },
        );
    }
    metadata_cache
}

#[tokio::main]
async fn main() {
    let installation_timeout: WholeSeconds = get_mandatory_parsed_env_var("INSTALLATION_TIMEOUT");
    let update_timeout: WholeSeconds = get_mandatory_parsed_env_var("UPDATE_TIMEOUT");
    let system_limits = check_and_get_system_limits();
    let max_concurrent_submissions: usize =
        get_mandatory_parsed_env_var("MAX_CONCURRENT_SUBMISSIONS");
    let execution_semaphore = Arc::new(Semaphore::new(max_concurrent_submissions));

    let box_id = Arc::new(AtomicU64::new(0));
    let metadata_cache = Arc::new(RwLock::new(get_runtimes()));
    let installation_lock = Arc::new(RwLock::new(0));
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
                let box_id = box_id.clone();
                let metadata_cache = metadata_cache.clone();
                let installation_lock = installation_lock.clone();
                move |req| {
                    install_runtime(
                        installation_timeout,
                        box_id,
                        metadata_cache,
                        installation_lock,
                        req,
                    )
                }
            }),
        )
        .route(
            "/runtimes/:id",
            delete({
                let metadata_cache = metadata_cache.clone();
                move |req| delete_runtime(req, metadata_cache)
            }),
        )
        .route(
            "/update",
            post({
                let installation_lock = installation_lock.clone();
                move || update_nix(update_timeout, installation_lock)
            }),
        )
        .route(
            "/execute",
            post({
                let metadata_cache = metadata_cache.clone();
                let installation_lock = installation_lock.clone();
                let box_id = box_id.clone();
                let system_limits = system_limits.clone();
                let execution_semaphore = execution_semaphore.clone();
                move |query, req| {
                    execute(
                        execution_semaphore,
                        box_id,
                        metadata_cache,
                        installation_lock,
                        system_limits,
                        req,
                        query,
                    )
                }
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
