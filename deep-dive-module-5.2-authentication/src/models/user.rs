//! The `users` table's row type (see
//! `migrations/20240201000000_add_users.sql`), plus the `/login` request and
//! response DTOs built on it -- Module 6's applied-for-real `User` model.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A row from the `users` table. `password_hash` never appears in a JSON
/// response -- `#[serde(skip_serializing)]` makes that a guarantee an
/// accidental `HttpResponse::Ok().json(user)` can't violate, rather than a
/// rule every call site has to remember on its own.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// The body a client POSTs to `/register`. Structurally identical to
/// [`LoginRequest`] today, but kept as its own type on purpose: registration
/// is where extra fields (an email, a password confirmation, an invite code)
/// naturally land later, and a distinct type is what stops that growth from
/// silently changing what `/login` accepts too.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

/// Returned by both `/login` and `/register` -- a bearer token is a bearer
/// token regardless of which endpoint minted it, so both share this one shape.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
}
