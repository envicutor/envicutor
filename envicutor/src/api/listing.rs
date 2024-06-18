use std::sync::Arc;

use axum::{response::IntoResponse, Json};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::types::Metadata;

#[derive(Serialize)]
pub struct Runtime {
    id: u32,
    name: String,
}

pub async fn list_runtimes(metadata_cache: Arc<RwLock<Metadata>>) -> impl IntoResponse {
    let metadata_guard = metadata_cache.read().await;
    let mut runtimes: Vec<Runtime> = Vec::new();
    for (key, value) in metadata_guard.iter() {
        runtimes.push(Runtime {
            id: *key,
            name: value.name.clone(),
        });
    }
    Json(runtimes)
}
