// Re-exports for the routes module.
//
// No single re-exported `configure` here -- unlike earlier modules, there
// are now two: `routes::auth::configure` (public, just `/login`) and
// `routes::tasks::configure` (wrapped in `crate::middleware::require_auth`).
// `main.rs` calls both explicitly by their full path.
pub mod auth;
pub mod tasks;
