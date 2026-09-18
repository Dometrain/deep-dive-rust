// Re-exports for the routes module.
//
// No single re-exported `configure` here -- unlike earlier modules, there
// are now four: `routes::auth::configure` (public, `/register` + `/login`),
// `routes::tasks::configure` (wrapped in `crate::middleware::require_auth`),
// Module 7's `routes::websocket::configure` (public `/ws`), and Module 8's
// `routes::health::configure` (public `/health`). `main.rs` calls each
// explicitly by its full path.
pub mod auth;
pub mod health;
pub mod tasks;
pub mod websocket;
