//! 系统接口：健康检查与时钟校准。

use axum::Json;
use chrono::Utc;
use task_reminder_core::protocol::{HealthResponse, ServerTimeResponse, SCHEMA_VERSION};

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version: SCHEMA_VERSION.to_string(),
    })
}

pub async fn server_time() -> Json<ServerTimeResponse> {
    let now = Utc::now();
    Json(ServerTimeResponse {
        unix_millis: now.timestamp_millis(),
        iso_utc: now.to_rfc3339(),
    })
}
