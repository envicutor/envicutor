use std::sync::Arc;

use axum::{
    body::Body,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rusqlite::Connection;
use tokio::{sync::RwLock, task};

use crate::{
    api::common_responses::{StaticMessage, INTERNAL_SERVER_ERROR_RESPONSE},
    globals::DB_PATH,
    types::Metadata,
};

pub async fn delete_runtime(
    Path(id): Path<u32>,
    metadata_cache: Arc<RwLock<Metadata>>,
) -> Result<(), Response<Body>> {
    let affected_rows = task::spawn_blocking(move || {
        let conn = Connection::open(DB_PATH).map_err(|e| {
            eprintln!("Failed to open SQLite connection: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;
        let mut stmt = conn
            .prepare("DELETE FROM runtime WHERE id = ?")
            .map_err(|e| {
                eprintln!("Failed to open SQLite connection: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
        let affected_rows = stmt.execute([id]).map_err(|e| {
            eprintln!("Failed to prepare SQLite statement: {e}");
            INTERNAL_SERVER_ERROR_RESPONSE.into_response()
        })?;
        Ok(affected_rows)
    })
    .await
    .map_err(|e| {
        eprintln!("Failed to spawn blocking task: {e}");
        INTERNAL_SERVER_ERROR_RESPONSE.into_response()
    })??;
    if affected_rows == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(StaticMessage {
                message: "Could not find the specified runtime",
            }),
        )
            .into_response());
    }
    let mut metadata_guard = metadata_cache.write().await;
    metadata_guard.remove(&id);
    Ok(())
}
