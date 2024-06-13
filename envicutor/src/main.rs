use std::{
    collections::HashMap,
    env,
    sync::{atomic::AtomicU64, Arc},
};

use axum::{routing::post, Router};
use envicutor::{
    limits::{MandatoryLimits, SystemLimits},
    runtime_installation::install_runtime,
};
use tokio::sync::{RwLock, Semaphore};

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

fn check_and_get_system_limits() -> SystemLimits {
    SystemLimits {
        installation: get_limits_from_env_var("INSTALLATION"),
    }
}

#[tokio::main]
async fn main() {
    let system_limits = check_and_get_system_limits();
    // Currently we only allow one runtime installation at a time to avoid concurrency issues with Nix and SQLite
    let installation_semaphore = Arc::new(Semaphore::new(1));
    let box_id = Arc::new(AtomicU64::new(0));
    let metadata_cache = Arc::new(RwLock::new(HashMap::new()));
    let app = Router::new().route(
        "/install",
        post({
            let system_limits = system_limits.clone();
            let installation_semaphore = installation_semaphore.clone();
            let box_id = box_id.clone();
            let metadata_cache = metadata_cache.clone();
            move |req| {
                install_runtime(
                    system_limits,
                    installation_semaphore,
                    box_id,
                    metadata_cache,
                    req,
                )
            }
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
