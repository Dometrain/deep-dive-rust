// Re-exports for the routes module.
//
// No single re-exported `configure` here -- unlike earlier modules, there
// are now three: `routes::auth::configure` (public, just `/login`),
// `routes::tasks::configure` (wrapped in `crate::middleware::require_auth`),
// and Module 7's `routes::websocket::configure` (public `/ws`). `main.rs`
// calls all three explicitly by their full path.
pub mod auth;
pub mod tasks;
pub mod websocket;
