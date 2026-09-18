use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{runners::AsyncRunner, ContainerAsync},
};
use todo_traits_app::db::{create_pool, initialize_database, DbPool};

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
