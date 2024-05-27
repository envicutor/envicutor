use std::{env, sync::Arc};

use axum::{routing::post, Router};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::sync::{Mutex, Semaphore};

const MAX_BOX_ID: u32 = 8388608;
const DEFAULT_PORT: &str = "5000";

async fn install_package(
    semaphore: Arc<Semaphore>,
    box_id: Arc<Mutex<u32>>,
    db_pool: Arc<Pool<Postgres>>,
) {
    let mut box_id = box_id.lock().await;
    *box_id = (*box_id + 1) % MAX_BOX_ID;
    let new_box_id = *box_id;
    drop(box_id);
}

#[tokio::main]
async fn main() {
    let installation_semaphore = Arc::new(Semaphore::new(1));
    let box_id = Arc::new(Mutex::new(0));
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
            move || install_package(installation_semaphore, box_id, db_pool)
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
