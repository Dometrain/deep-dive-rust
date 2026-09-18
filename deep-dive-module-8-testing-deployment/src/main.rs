use actix_cors::Cors;
use actix_web::{middleware::from_fn, web, App, HttpServer};
use anyhow::Context;
use todo_ws_app::{
    audit::AuditLogger,
    broadcast::Broadcaster,
    cache::RecentTasksCache,
    config::load_config,
    db::{create_pool, initialize_database},
    middleware, routes, telemetry,
};
use tracing::{info, trace};

/// How many recently created task ids `RecentTasksCache` remembers.
const RECENT_TASKS_CAPACITY: usize = 10;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    // Module 5's `anyhow::Context` lesson
    // (`examples::module_5::error_context_with_anyhow`), applied for real:
    // every step below used to collapse into the same generic
    // `io::Error::other(...)`, which threw away exactly the detail that
    // mattered -- *which* step failed. `.context(...)` keeps that detail
    // without needing a new `AppError` variant per startup step (see the
    // lesson's own "Key Points" for why that trade only makes sense at this
    // outer edge, not in request handlers).
    let config = load_config().context("failed to load configuration")?;

    // Initialize tracing with JSON output
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.server.log_level));

    // Optional OTLP export (e.g. to Jaeger), enabled by setting
    // OTEL_EXPORTER_OTLP_ENDPOINT. Stdout logging stays active either way.
    let otel = telemetry::otel_layer();
    let (otel_layer, tracer_provider) = otel.unzip();

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .json()
                .with_target(false)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(false)
                .with_line_number(true),
        )
        .with(otel_layer)
        .init();

    if tracer_provider.is_some() {
        info!("OTLP span export enabled");
    }
    trace!("Starting todo-ws-app");

    let pool = create_pool(&config.database.url)
        .await
        // Deliberately doesn't include `config.database.url` in this
        // message, unlike this module's own lesson sample -- a Postgres
        // URL embeds credentials (see the `skip_all` comment on
        // `db::connection::create_pool`, which exists for exactly this
        // reason). "failed to connect to the database" is enough detail to
        // act on without also being a way to leak a password into logs.
        .context("failed to connect to the database")?;
    initialize_database(&pool)
        .await
        .context("failed to run database migrations")?;

    // `RecentTasksCache` clones cheaply (it's an `Arc` underneath -- see
    // `cache.rs`), so every worker thread below gets its own clone that
    // still points at the same shared cache.
    let recent_tasks = RecentTasksCache::new(RECENT_TASKS_CAPACITY);

    // Module 4's threads + channels lesson, applied for real: `spawn` starts
    // one background thread that owns the audit trail; `audit_worker` is
    // kept exactly once, here, and joined after `.run().await?` below --
    // see `audit::AuditWorker::shutdown` for why that join can't hang.
    let (audit_logger, audit_worker) = AuditLogger::spawn();

    // Module 7's applied-for-real broadcast fan-out. Cloned cheaply per worker
    // below (it's a `broadcast::Sender` handle underneath -- see
    // `broadcast.rs`), so every worker's handlers and every `/ws` connection
    // share the same channel: a task created on one worker reaches a WebSocket
    // client served by any other.
    let broadcaster = Broadcaster::new();

    let bind_address = (config.server.host.clone(), config.server.port);
    info!("Listening on http://{}:{}", bind_address.0, bind_address.1);

    HttpServer::new(move || {
        App::new()
            // Module 5's middleware-order lesson
            // (`examples::module_5::middleware_order`), applied for real.
            // `.wrap()` calls nest like an onion: the *last* one here is
            // the *outermost* layer, so read this bottom-up for what a
            // request actually hits first:
            //   1. `TracingLogger` -- sees and logs every request, even
            //      ones the layers below short-circuit.
            //   2. `middleware::timing` -- wraps CORS, so even a
            //      CORS-short-circuited preflight response still gets an
            //      `X-Response-Time-Ms` header.
            //   3. `Cors` -- innermost of the three, closest to the actual
            //      routes.
            .wrap(
                Cors::default()
                    .allow_any_origin() // Development only -- see README.
                    .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
                    // Module 7's Leptos frontend runs on a different origin
                    // (`trunk serve`, port 8080 vs. the app's own), so its
                    // `Authorization: Bearer ...` and `Content-Type:
                    // application/json` request headers only survive the CORS
                    // preflight if they're explicitly allowed. Development
                    // only, same caveat as `allow_any_origin` above.
                    .allow_any_header()
                    .max_age(3600),
            )
            .wrap(from_fn(middleware::timing))
            .wrap(TracingLogger::default())
            .app_data(web::Data::new(pool.clone()))
            // `config` is `Arc<AppConfig>` (see `config::load_config`) --
            // `web::Data<T>` already Arc-wraps whatever it holds for cheap
            // per-worker cloning, so registering `web::Data::new(config
            // .clone())` directly would produce `web::Data<Arc<AppConfig>>`,
            // not `web::Data<AppConfig>`. Every handler and
            // `middleware::require_auth` extracts the latter -- `.as_ref()
            // .clone()` here unwraps the outer `Arc` once, at startup, so
            // the type registered actually matches what they ask for.
            .app_data(web::Data::new(config.as_ref().clone()))
            .app_data(web::Data::new(recent_tasks.clone()))
            .app_data(web::Data::new(audit_logger.clone()))
            .app_data(web::Data::new(broadcaster.clone()))
            // `/login` (public) and `/tasks` (protected -- see
            // `routes::tasks::configure`'s own `.wrap(from_fn(
            // middleware::require_auth))`, scoped to just that path, not
            // registered here at the whole-`App` level).
            .configure(routes::auth::configure)
            .configure(routes::tasks::configure)
            // Module 7's `GET /ws`, registered at the whole-`App` level and
            // deliberately *outside* the `/tasks` scope's `require_auth` --
            // unprotected for now (see `mod-7.md`'s Exercises for adding
            // `middleware::require_auth` around it).
            .configure(routes::websocket::configure)
            // Module 8's `GET /health` readiness check -- also public: a load
            // balancer or `docker-compose` healthcheck polling it shouldn't
            // need a bearer token.
            .configure(routes::health::configure)
    })
    .bind(bind_address)?
    .run()
    .await?;

    // By the time `.run().await` above resolves, actix-web has already
    // stopped every worker and dropped every `App` instance it built (and,
    // with it, every `web::Data<AuditLogger>` clone handed to a handler) --
    // so every `Sender` clone the audit channel had is already gone, and
    // this join returns as soon as the worker drains what's left.
    audit_worker.shutdown();

    // Flush any buffered spans before the process exits.
    if let Some(provider) = tracer_provider {
        if let Err(error) = provider.shutdown() {
            eprintln!("Failed to flush OTEL spans: {error}");
        }
    }

    Ok(())
}
