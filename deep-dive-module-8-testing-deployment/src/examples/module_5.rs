//! Standalone teaching snippets for *Deep Dive: Rust* Module 5 ("Advanced
//! Web Development"), one submodule per code sample in the lesson -- same
//! structure as [`crate::examples::module_4`].
//!
//! Lesson 5.1 covers Actix-web middleware: CORS, a custom `from_fn`
//! middleware, and -- the part most tutorials skip -- the actual order
//! `.wrap()` calls execute in, proven with a real, recorded test instead of
//! just asserted in prose. Lesson 5.2 covers PostgreSQL with SQLx
//! (connecting, querying, migrating, pooling) plus two samples moved over
//! from a retired "Advanced Error Handling" module: `anyhow::Context` for
//! startup errors, and `?`-based error propagation across layers.
//!
//! A note on 5.2 that doesn't apply to most other courses' Module 5: this
//! codebase has been PostgreSQL-based since early in *Deep Dive: Rust*, not
//! SQLite, so there's no live "migrate to Postgres" step to demonstrate --
//! `db::connection`, `db::queries` and `migrations/` already are this
//! lesson, applied for real, tested end-to-end in `tests/tasks_test.rs`
//! since long before this module. See this crate's `README.md` for the
//! full explanation. What *was* still missing -- clear, contextual startup
//! error messages -- is this module's actual applied contribution; see
//! `main.rs`.
//!
//! The applied version of 5.1 lives in the real HTTP API too:
//! [`crate::middleware::timing`], the one `from_fn` middleware that ships
//! outside this `examples` module, wired up in `main.rs` alongside CORS.

/// 5.1 #1 -- CORS Middleware.
pub mod cors_middleware {
    use actix_cors::Cors;

    /// The same `Cors` configuration `main.rs` uses for real, pulled out
    /// here so the standalone sample and the applied one can't drift apart.
    pub fn build() -> Cors {
        Cors::default()
            .allow_any_origin() // Development only -- see this crate's README.
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
            .max_age(3600)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use actix_web::{
            http::{header, Method},
            test, web, App, HttpResponse,
        };

        #[actix_web::test]
        async fn a_preflight_request_gets_a_cors_allow_origin_header() {
            let app = test::init_service(
                App::new()
                    .wrap(build())
                    .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
            )
            .await;

            // A real CORS preflight: an `OPTIONS` request naming the method
            // the actual request would use.
            let req = test::TestRequest::default()
                .method(Method::OPTIONS)
                .uri("/")
                .insert_header((header::ORIGIN, "https://example.com"))
                .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "GET"))
                .to_request();

            let res = test::call_service(&app, req).await;

            assert!(
                res.status().is_success(),
                "a valid preflight request should not be rejected"
            );
            // `allow_any_origin()` doesn't send a literal `*` here -- it
            // echoes the request's own `Origin` back (confirmed against a
            // real running server, see this crate's README) -- so this
            // checks the actual value, not just that some header showed up.
            assert_eq!(
                res.headers()
                    .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                    .unwrap(),
                "https://example.com"
            );
        }
    }
}

/// 5.1 #2 -- Custom Middleware.
pub mod custom_middleware {
    use actix_web::{
        body::MessageBody,
        dev::{ServiceRequest, ServiceResponse},
        http::header::{HeaderName, HeaderValue},
        middleware::Next,
        Error,
    };

    /// The generic shape of the lesson: read/act before calling `next`,
    /// then act again on the response it hands back. This app's real
    /// middleware, [`crate::middleware::timing`], is the exact same shape
    /// with a computed value (elapsed time) instead of a fixed one.
    pub async fn add_custom_header(
        req: ServiceRequest,
        next: Next<impl MessageBody>,
    ) -> Result<ServiceResponse<impl MessageBody>, Error> {
        let mut res = next.call(req).await?;
        res.headers_mut().insert(
            HeaderName::from_static("x-powered-by"),
            HeaderValue::from_static("rust-actix-lesson"),
        );
        Ok(res)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use actix_web::{middleware::from_fn, test, web, App, HttpResponse};

        #[actix_web::test]
        async fn adds_the_header_to_every_response() {
            let app = test::init_service(
                App::new()
                    .wrap(from_fn(add_custom_header))
                    .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
            )
            .await;

            let req = test::TestRequest::get().uri("/").to_request();
            let res = test::call_service(&app, req).await;

            assert_eq!(
                res.headers().get("x-powered-by").unwrap(),
                "rust-actix-lesson"
            );
        }
    }
}

/// 5.1 #3 -- Middleware Order.
///
/// Actix-web's own docs put this bluntly: "if you use `wrap()` multiple
/// times, the last occurrence will be executed first." `middleware_a` and
/// `middleware_b` below each record a "before" and "after" entry into a
/// shared, request-scoped log -- registering them and reading that log back
/// after one real request is what proves the rule, rather than asking you
/// to trust a comment.
pub mod middleware_order {
    use actix_web::{
        body::MessageBody,
        dev::{ServiceRequest, ServiceResponse},
        middleware::Next,
        web, Error,
    };
    use std::sync::Mutex;

    /// Shared via `web::Data` so both middlewares below record into the
    /// same `Vec`, in whatever order they actually execute in.
    pub type ExecutionLog = web::Data<Mutex<Vec<&'static str>>>;

    pub async fn middleware_a(
        req: ServiceRequest,
        next: Next<impl MessageBody>,
    ) -> Result<ServiceResponse<impl MessageBody>, Error> {
        // Grabbed before `next.call` moves `req` away, so it's still usable
        // afterward to record the "after" entry too.
        let log = req
            .app_data::<ExecutionLog>()
            .expect("ExecutionLog should be registered")
            .clone();
        log.lock().unwrap().push("A before");
        let res = next.call(req).await?;
        log.lock().unwrap().push("A after");
        Ok(res)
    }

    pub async fn middleware_b(
        req: ServiceRequest,
        next: Next<impl MessageBody>,
    ) -> Result<ServiceResponse<impl MessageBody>, Error> {
        let log = req
            .app_data::<ExecutionLog>()
            .expect("ExecutionLog should be registered")
            .clone();
        log.lock().unwrap().push("B before");
        let res = next.call(req).await?;
        log.lock().unwrap().push("B after");
        Ok(res)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use actix_web::{middleware::from_fn, test, App, HttpResponse};

        #[actix_web::test]
        async fn the_last_registered_middleware_runs_first_for_the_request() {
            let log: ExecutionLog = web::Data::new(Mutex::new(Vec::new()));

            let app = test::init_service(
                App::new()
                    .app_data(log.clone())
                    .wrap(from_fn(middleware_a)) // registered first -> innermost
                    .wrap(from_fn(middleware_b)) // registered last -> outermost
                    .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
            )
            .await;

            let req = test::TestRequest::get().uri("/").to_request();
            test::call_service(&app, req).await;

            assert_eq!(
                *log.lock().unwrap(),
                vec!["B before", "A before", "A after", "B after"],
                "B, registered last, should wrap A -- running before it on \
                 the way in and after it on the way out"
            );
        }
    }
}

/// 5.2 #1 -- PostgreSQL Setup.
pub mod postgresql_setup {
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;

    /// Configuring a pool builder doesn't connect to anything by itself --
    /// nothing here needs a live Postgres instance, which is what makes it
    /// testable without Docker. `connect_lazy` (used in the tests below)
    /// goes one step further: it builds a working `Pool` with no `.await`
    /// at all, deferring the real network connection until the pool's
    /// first query. Contrast with `db::connection::create_pool`, which this
    /// app actually uses at startup -- it connects *eagerly*
    /// (`.connect().await`), specifically so a bad URL fails immediately at
    /// startup instead of on whatever request happens to hit the database
    /// first.
    pub fn build_pool_options() -> PgPoolOptions {
        PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // `connect_lazy` never `.await`s -- it's a synchronous call -- but
        // it still needs to run *inside* a Tokio context: sqlx spawns a
        // background idle-connection reaper task as part of building the
        // pool, even before the pool's first real query. Plain `#[test]`
        // has no runtime at all, so this panics with "this functionality
        // requires a Tokio context" without `#[tokio::test]` here.
        #[tokio::test]
        async fn a_well_formed_url_builds_a_lazy_pool() {
            let pool = build_pool_options().connect_lazy("postgres://user:pass@localhost:5432/db");
            assert!(pool.is_ok());
        }

        #[test]
        fn a_malformed_url_is_rejected_even_though_nothing_connects_yet() {
            // `connect_lazy` defers the network connection, not URL
            // parsing -- a syntactically invalid URL is still caught here,
            // immediately, with no database involved at all.
            let pool = build_pool_options().connect_lazy("not a url");
            assert!(pool.is_err());
        }
    }
}

/// 5.2 #2 -- SQLx with PostgreSQL.
///
/// No new code here on purpose: running a real query needs a real
/// connection, and this app already has one, fully exercised.
/// `db::queries::get_task`, `db::queries::list_tasks` and the rest of that
/// file are this lesson, applied for real, since long before this module --
/// and they're tested end-to-end against a throwaway Postgres container in
/// `tests/tasks_test.rs`. A standalone sample here would just be a shorter,
/// untested copy of code that already exists and already has real coverage.
pub mod sqlx_with_postgres {}

/// 5.2 #3 -- Migrations.
pub mod migrations {
    /// Reads this app's actual migration file straight off disk -- proof
    /// the migration this lesson describes already exists and is
    /// well-formed, without needing a live database to check it against.
    pub fn init_migration_sql() -> &'static str {
        include_str!("../../migrations/20240101000000_init.sql")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_apps_real_migration_creates_the_tasks_table() {
            let sql = init_migration_sql().to_lowercase();
            assert!(sql.contains("create table"));
            assert!(sql.contains("tasks"));
        }
    }
}

/// 5.2 #4 -- Connection Pooling.
pub mod connection_pooling {
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;

    /// `Arc`-wrapping a pool -- this app's `db::DbPool` is exactly
    /// `Arc<PgPool>` -- is what lets every request handler share the *same*
    /// pool instead of each opening its own connections. Cloning the `Arc`
    /// is cheap (an atomic reference-count bump) regardless of how many
    /// real database connections the pool manages underneath.
    pub fn cloned_handles_share_one_pool() -> bool {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy("postgres://user:pass@localhost:5432/db")
            .expect("well-formed URL");
        let shared = Arc::new(pool);

        let handle_for_handler_a = Arc::clone(&shared);
        let handle_for_handler_b = Arc::clone(&shared);

        Arc::ptr_eq(&handle_for_handler_a, &handle_for_handler_b)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // See `postgresql_setup::tests` for why this needs `#[tokio::test]`
        // even though nothing here `.await`s anything -- `connect_lazy`
        // still needs a Tokio context available to build the pool.
        #[tokio::test]
        async fn cloned_handles_point_at_the_same_pool() {
            assert!(cloned_handles_share_one_pool());
        }
    }
}

/// 5.2 #5 -- Error Context with `anyhow` *(moved from the retired Advanced
/// Error Handling module)*.
pub mod error_context_with_anyhow {
    use anyhow::{Context, Result};

    fn parse_port(raw: &str) -> Result<u16, std::num::ParseIntError> {
        raw.parse()
    }

    /// The applied version of this lesson lives in `main`, wrapping this
    /// app's real startup steps (config, database connection, migrations).
    /// This standalone version wraps a deliberately simple failure --
    /// parsing a port number -- so the behavior is testable in isolation.
    pub fn load_port(raw: &str) -> Result<u16> {
        parse_port(raw).with_context(|| format!("invalid APP_SERVER_PORT: {raw:?}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_valid_port_parses_normally() {
            assert_eq!(load_port("8080").unwrap(), 8080);
        }

        #[test]
        fn an_invalid_port_keeps_the_original_error_and_adds_context() {
            let err = load_port("not-a-port").unwrap_err();
            assert!(err.to_string().contains("invalid APP_SERVER_PORT"));
            // `.context()` wraps the original error, it doesn't replace it
            // -- the underlying `ParseIntError` is still in the chain.
            assert!(err
                .chain()
                .any(|cause| cause.to_string().contains("invalid digit")));
        }
    }
}

/// 5.2 #6 -- Error Propagation Across Layers *(moved from the retired
/// Advanced Error Handling module)*.
pub mod error_propagation_across_layers {
    use crate::error::AppError;

    /// Stands in for a data-access function like `db::queries::get_task` --
    /// same shape: look something up, `?`-propagate a lookup failure as an
    /// `AppError` rather than returning something the caller has to
    /// interpret.
    pub fn find_task(id: i64, existing_ids: &[i64]) -> Result<i64, AppError> {
        existing_ids
            .iter()
            .find(|&&existing| existing == id)
            .copied()
            .ok_or(AppError::TaskNotFound)
    }

    /// Stands in for a route handler like `routes::tasks::get_task` -- calls
    /// the data-access function with `?`, one layer up, and never has to
    /// `match` on what kind of error it might have been.
    pub fn handle_get_task(id: i64, existing_ids: &[i64]) -> Result<i64, AppError> {
        let task_id = find_task(id, existing_ids)?;
        Ok(task_id)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use actix_web::{http::StatusCode, ResponseError};

        #[test]
        fn a_found_task_propagates_through_both_layers() {
            assert_eq!(handle_get_task(2, &[1, 2, 3]).unwrap(), 2);
        }

        #[test]
        fn a_missing_task_becomes_a_404_without_a_match_anywhere_in_the_handler() {
            let err = handle_get_task(99, &[1, 2, 3]).unwrap_err();
            assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        }
    }
}
