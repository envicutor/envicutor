use axum::{http::StatusCode, Json};

#[derive(serde::Serialize)]
pub struct Message {
    pub message: String,
}


#[derive(serde::Serialize)]
pub struct StaticMessage {
    pub message: &'static str,
}

pub const INTERNAL_SERVER_ERROR_RESPONSE: (StatusCode, Json<StaticMessage>) = (
    StatusCode::INTERNAL_SERVER_ERROR,
    Json(StaticMessage {
        message: "Internal server error",
    }),
);
