//! `GET /health` -- Module 8's readiness check.
//!
//! *Readiness*, not *liveness*: it answers "should traffic be sent to this
//! instance right now?", not merely "is the process running?". A process can
//! be alive while its database connection is dead -- alive, but useless -- so
//! this endpoint proves the app can actually reach Postgres before reporting
//! healthy. It's what `docker-compose.yml`'s `app` healthcheck (and any real
//! load balancer or orchestrator) polls.
//!
//! Registered at the whole-`App` level in `main.rs`, deliberately *outside*
//! the `/tasks` scope's `require_auth` (Module 6): a load balancer polling
//! this shouldn't have to carry a bearer token.

use crate::db::DbPool;
use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health));
}

/// Reports `200 OK` only if this instance can reach the database right now.
/// `SELECT 1` is the cheapest possible real query -- it proves a connection
/// can be acquired from the pool and a round trip completes, without touching
/// any real table.
///
/// Returns a plain `HttpResponse`, not a `Result<_, AppError>`: a readiness
/// check that itself needed the database up just to *report* that the database
/// is down would be a strange failure mode. A failure is `503 Service
/// Unavailable`, not `500` -- the app isn't broken, a dependency is
/// unreachable right now (the same reasoning `AppError::Timeout` used `504`
/// rather than `500`).
pub async fn health(pool: web::Data<DbPool>) -> HttpResponse {
    // `pool.get_ref().as_ref()` unwraps `web::Data<DbPool>` -> `&DbPool`
    // (`Arc<PgPool>`) -> `&PgPool`, the double-unwrap every handler in this app
    // does to reach the pool `sqlx` needs.
    match sqlx::query("SELECT 1")
        .execute(pool.get_ref().as_ref())
        .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}
