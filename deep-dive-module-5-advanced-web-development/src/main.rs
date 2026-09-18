use actix_cors::Cors;
use actix_web::{middleware::from_fn, web, App, HttpServer};
use anyhow::Context;
use todo_web_app::{
    audit::AuditLogger,
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
    trace!("Starting todo-web-app");

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
                    .max_age(3600),
            )
            .wrap(from_fn(middleware::timing))
            .wrap(TracingLogger::default())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(recent_tasks.clone()))
            .app_data(web::Data::new(audit_logger.clone()))
            .configure(routes::tasks::configure)
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
