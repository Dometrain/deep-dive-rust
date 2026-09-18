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

    /// Raised when `routes::tasks::get_task`'s `tokio::select!` timeout
    /// (Module 4's applied lesson) wins its race against the database
    /// query -- see `routes::tasks::with_timeout`. Mapped to `504`, not
    /// `408` (that's for a slow *client*, not a slow dependency on our
    /// side) or a plain `503` (which says nothing about *why* the server
    /// couldn't respond) -- `504 Gateway Timeout` is the closest standard
    /// status for "a dependency this request needed took too long," even
    /// though Postgres isn't literally an upstream HTTP server the way the
    /// name implies.
    #[error("Request timed out")]
    Timeout,
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
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
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
            Self::Timeout => "Request timed out",
        };

        json_error(self.status_code(), message)
    }
}
