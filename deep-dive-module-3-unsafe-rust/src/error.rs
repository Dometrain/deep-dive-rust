use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::Serialize;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database operation failed")]
    Database(#[from] sqlx::Error),

    #[error("Database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Task not found")]
    TaskNotFound,

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

/// Builds the same `{"error": "..."}` JSON body used by [`AppError`], for call
/// sites (like the JSON extractor's error handler in `routes::tasks`) that
/// need to report a client error before an `AppError` value exists.
pub(crate) fn json_error(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(ErrorBody { error: message })
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::TaskNotFound => StatusCode::NOT_FOUND,
            Self::InvalidInput(_) => StatusCode::BAD_REQUEST,
            Self::Database(_) | Self::Migration(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let message = match self {
            Self::Database(error) => {
                error!("Database operation failed: {error}");
                "Internal server error"
            }
            Self::Migration(error) => {
                error!("Database migration failed: {error}");
                "Internal server error"
            }
            Self::TaskNotFound => "Task not found",
            Self::InvalidInput(message) => message,
        };

        json_error(self.status_code(), message)
    }
}
