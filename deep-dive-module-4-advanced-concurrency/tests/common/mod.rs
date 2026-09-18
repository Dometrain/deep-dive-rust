use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{runners::AsyncRunner, ContainerAsync},
};
use todo_concurrency_app::audit::AuditLogger;
use todo_concurrency_app::consumer::{self, EventPublisher};
use todo_concurrency_app::db::{create_pool, initialize_database, DbPool};

/// Spins up a throwaway Postgres container, runs the migrations against it and
/// returns the connection pool together with the container handle.
///
/// The returned `ContainerAsync` must be kept alive for the duration of the
/// test; dropping it stops and removes the container. Each test gets its own
/// container, which guarantees a clean database (fresh sequences, no leftover
/// rows) even when tests run in parallel.
pub async fn setup_test_db() -> (DbPool, ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres test container should start");

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("postgres container should expose port 5432");

    // Default credentials baked into the postgres testcontainers module.
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{host_port}/postgres");

    let pool = create_pool(&database_url)
        .await
        .expect("test database pool should be created");

    initialize_database(&pool)
        .await
        .expect("test migrations should run");

    (pool, container)
}

/// A ready-to-register `AuditLogger` for tests that build an `App` directly
/// (rather than through `main.rs`) and so need to supply every `web::Data`
/// a handler extracts. The paired `AuditWorker` is intentionally dropped
/// here, not joined -- these are short-lived test processes, and the
/// background thread simply exits with the process; `main.rs` is the one
/// place this app actually needs the join (see `audit::AuditWorker`).
pub fn test_audit_logger() -> AuditLogger {
    AuditLogger::spawn().0
}

/// A ready-to-register `EventPublisher` for the same reason as
/// `test_audit_logger` above. The paired background consumer task is never
/// spawned here at all -- nothing in these tests needs it to actually run,
/// only for `create_task` to have somewhere to publish to.
pub fn test_event_publisher() -> EventPublisher {
    consumer::channel().0
}
