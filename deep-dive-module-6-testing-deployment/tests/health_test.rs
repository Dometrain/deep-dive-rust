//! Module 8's health-check test -- and the point of it is the *failure* path.
//! A `/health` that only ever gets called while the database is up could
//! always return `200` regardless of what it checked, and no one would notice.
//! This test breaks the dependency on purpose (stops the Postgres container
//! out from under the pool) and asserts the endpoint reports `503`.
//!
//! Unlike this crate's other integration tests, it deliberately does *not* use
//! `tests/common` -- it builds the container and pool inline. `/health` only
//! runs `SELECT 1`, so it needs no migrations and no seeded users, and pulling
//! in `common` would just drag in a pile of helpers this one test never calls.

use std::sync::Arc;

use actix_web::{http::StatusCode, test, web, App};
use sqlx::PgPool;
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
use todo_ws_app::{db::DbPool, routes::health};

#[actix_web::test]
async fn health_reports_ok_then_unavailable_once_the_database_is_gone() {
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres test container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("postgres container should expose port 5432");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool: DbPool = Arc::new(
        PgPool::connect(&url)
            .await
            .expect("test database pool should connect"),
    );

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .configure(health::configure),
    )
    .await;

    // Baseline: the database is up, so this instance is ready.
    let request = test::TestRequest::get().uri("/health").to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Stop the container out from under the pool -- the app's own connection is
    // now genuinely broken, not just slow.
    drop(container);

    // The container may take a moment to actually stop after `drop`, so poll
    // until the check flips to 503 rather than asserting on the very first read
    // -- a fixed sleep would be either flaky (too short) or needlessly slow
    // (too long).
    let mut status = StatusCode::OK;
    for _ in 0..30 {
        let request = test::TestRequest::get().uri("/health").to_request();
        status = test::call_service(&app, request).await.status();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
