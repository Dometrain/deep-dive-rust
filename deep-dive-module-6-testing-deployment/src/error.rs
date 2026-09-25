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

    /// Module 6's applied error variant: raised by
    /// `middleware::require_auth` for a missing/malformed/expired/
    /// bad-signature bearer token, and by `routes::auth::login` for a
    /// wrong username or password -- deliberately the *same* variant (and
    /// the same message) for both a nonexistent username and a wrong
    /// password, so a client can't use the response to tell those two
    /// cases apart. See `routes::auth::authenticate_user`.
    #[error("Authentication failed")]
    Unauthorized,

    /// Raised by `routes::auth::register` when the requested username is
    /// already taken -- a unique-constraint violation on `users.username`,
    /// mapped to `409 Conflict` rather than a generic `500`. That distinction
    /// matters to the caller: `409` says "this specific request can't succeed
    /// as-is, pick another name," where a `500` would wrongly suggest the
    /// server itself is broken and the same request might work on a retry.
    #[error("{0}")]
    Conflict(String),
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
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Conflict(_) => StatusCode::CONFLICT,
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
            Self::Unauthorized => "Authentication failed",
            Self::Conflict(message) => message,
        };

        json_error(self.status_code(), message)
    }
}
