use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{runners::AsyncRunner, ContainerAsync},
};
use todo_ws_app::audit::AuditLogger;
use todo_ws_app::auth::create_jwt;
use todo_ws_app::broadcast::Broadcaster;
use todo_ws_app::config::{AppConfig, DatabaseConfig, JwtConfig, ServerConfig};
use todo_ws_app::db::{create_pool, initialize_database, DbPool};
use todo_ws_app::password::hash_password;

/// The signing secret every integration test's `AppConfig` (see
/// [`test_app_config`]) and every token [`login_as_test_user`] issues both
/// use -- has to be the same value on both sides for `require_auth` to
/// accept a test-issued token.
pub const JWT_SECRET: &str = "integration-test-secret";

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

/// A ready-to-register `Broadcaster` for tests that build an `App` directly.
/// Module 7 added a `web::Data<Broadcaster>` extractor to
/// `routes::tasks::create_task`, so any test that hits `POST /tasks` now has
/// to register one or the handler's extractor fails before it runs. No
/// subscriber is attached here -- `broadcast` with nobody listening is a
/// no-op (see `broadcast::Broadcaster::broadcast`), so these tests exercise
/// the REST path exactly as before.
///
/// `#[allow(dead_code)]`: like `login_as_test_user`, this is used by
/// `tasks_test.rs` but not `auth_test.rs`, and `tests/common/mod.rs` compiles
/// fresh into each test binary.
#[allow(dead_code)]
pub fn test_broadcaster() -> Broadcaster {
    Broadcaster::new()
}

/// A ready-to-register `AppConfig` for tests that build an `App` directly.
/// Only `jwt.secret` is ever read by the routes under test here --
/// `server`/`database` are populated with unused placeholders so the struct
/// is complete, not because any test route touches them.
pub fn test_app_config() -> AppConfig {
    AppConfig {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            log_level: "info".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://unused".to_string(),
        },
        jwt: JwtConfig {
            secret: JWT_SECRET.to_string(),
        },
    }
}

/// Inserts a user with a real, hashed password (via
/// `crate::password::hash_password` -- no shortcut around the hashing this
/// app actually uses) and returns its id. Exercises the same insert shape
/// `routes::auth::login`'s `SELECT` expects to find.
pub async fn insert_user(pool: &DbPool, username: &str, password: &str) -> i32 {
    let password_hash = hash_password(password).expect("password should hash");

    sqlx::query_scalar("INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id")
        .bind(username)
        .bind(password_hash)
        .fetch_one(pool.as_ref())
        .await
        .expect("test user should insert")
}

/// Inserts a test user (see [`insert_user`]) and returns a bearer token for
/// it, signed with [`JWT_SECRET`] and ready to attach as
/// `Authorization: Bearer <token>` -- for tests that need an authenticated
/// request but aren't specifically exercising `POST /login` itself.
///
/// `#[allow(dead_code)]`: `tests/common/mod.rs` is compiled fresh as part of
/// each integration test binary -- `tasks_test.rs` uses this, `auth_test.rs`
/// doesn't (it calls `insert_user` and goes through the real `/login`
/// instead), so the latter binary would otherwise warn on an unused
/// function that a sibling binary does use.
#[allow(dead_code)]
pub async fn login_as_test_user(pool: &DbPool) -> String {
    let user_id = insert_user(pool, "test-user", "correct horse battery staple").await;
    create_jwt(&user_id.to_string(), JWT_SECRET)
}

/// A *second*, distinct user's token -- for the scoping tests that need two
/// authenticated identities against the same database. A different
/// username than [`login_as_test_user`]'s (the `users.username` unique
/// constraint would reject a re-insert anyway), so the two users' rows --
/// and therefore their tasks -- never collide. Same `#[allow(dead_code)]`
/// caveat as [`login_as_test_user`].
#[allow(dead_code)]
pub async fn login_as_second_test_user(pool: &DbPool) -> String {
    let user_id = insert_user(pool, "other-user", "another correct horse battery staple").await;
    create_jwt(&user_id.to_string(), JWT_SECRET)
}
