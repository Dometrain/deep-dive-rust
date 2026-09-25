//! The HTTP/WebSocket contract between the backend (`todo-ws-app`) and the
//! Leptos frontend (`todo-ws-frontend`).
//!
//! Every type here is exactly the JSON shape the two ends exchange -- and
//! deliberately nothing more. The backend's own `models::Task` is richer
//! (validated `TaskTitle`/`TaskDescription` newtypes, a `sqlx::FromRow` impl,
//! an FFI-backed `etag()`), none of which mean anything to a browser and some
//! of which can't even compile to WASM. What crosses the wire, though, is
//! just this flat, serde-friendly view -- so this is the half worth sharing.
//!
//! Because the backend serializes its `models::Task` and the frontend
//! deserializes *this* `Task`, the two must agree field-for-field. That
//! agreement isn't left to chance: the backend carries a contract test
//! (`tests/contract_test.rs`) that round-trips its own `Task` through JSON
//! into this one and asserts they match, so a field renamed on one side and
//! not the other fails the backend's test suite rather than silently breaking
//! the frontend at runtime.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A task as it appears on the wire -- the body of `GET /tasks`,
/// `POST /tasks`, and every `/ws` broadcast. `id`, `title` and `description`
/// arrive as bare JSON scalars here because the backend's newtypes
/// (`TaskId`, `TaskTitle`, `TaskDescription`) are all `#[serde(transparent)]`
/// / newtype-transparent, so they serialize as their inner value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
}

/// The body a client POSTs to `/tasks`. The backend re-validates every field
/// (non-empty, length-bounded title; description length; due date not in the
/// past) as it deserializes into its own newtypes -- so this permissive shape
/// is only the *request*, never a promise the server trusts it blindly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<DateTime<Utc>>,
}

/// The body a client POSTs to `/register`. Structurally identical to
/// [`LoginRequest`] on the wire, but its own type on purpose -- the backend
/// keeps them separate (see its `models::user`) so registration can grow
/// extra fields later without silently changing what `/login` accepts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

/// The body a client POSTs to `/login` (Module 6's endpoint).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// What `/login` *and* `/register` return on success (registration logs you
/// straight in) -- the bearer token the frontend then attaches to every
/// `/tasks` request as `Authorization: Bearer <token>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
}
