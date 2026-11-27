//! 统一的 HTTP 错误响应。

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use task_reminder_core::protocol::ApiErrorBody;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),
    #[error("内部错误: {0}")]
    Internal(String),
}

impl ServerError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Self::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        let message = match &self {
            Self::Database(err) => {
                tracing::error!(error = %err, "database error");
                "服务器数据库异常，请稍后重试".to_string()
            }
            Self::Internal(detail) => {
                tracing::error!(error = %detail, "internal error");
                "服务器内部错误".to_string()
            }
            other => other.to_string(),
        };
        (
            status,
            Json(ApiErrorBody {
                code: code.to_string(),
                message,
            }),
        )
            .into_response()
    }
}

pub type ServerResult<T> = Result<T, ServerError>;
