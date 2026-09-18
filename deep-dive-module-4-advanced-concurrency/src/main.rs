use actix_web::{web, App, HttpServer};
use std::io;
use todo_concurrency_app::{
    audit::AuditLogger,
    cache::RecentTasksCache,
    config::load_config,
    consumer,
    db::{create_pool, initialize_database},
    routes, telemetry,
};
use tracing::{info, trace};

/// How many recently created task ids `RecentTasksCache` remembers.
const RECENT_TASKS_CAPACITY: usize = 10;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[actix_web::main]
async fn main() -> io::Result<()> {
    let config = load_config().map_err(io::Error::other)?;

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
    trace!("Starting todo-concurrency-app");

    let pool = create_pool(&config.database.url)
        .await
        .map_err(io::Error::other)?;
    initialize_database(&pool).await.map_err(io::Error::other)?;

    // `RecentTasksCache` clones cheaply (it's an `Arc` underneath -- see
    // `cache.rs`), so every worker thread below gets its own clone that
    // still points at the same shared cache.
    let recent_tasks = RecentTasksCache::new(RECENT_TASKS_CAPACITY);

    // Module 4's threads + channels lesson, applied for real: `spawn` starts
    // one background thread that owns the audit trail; `audit_worker` is
    // kept exactly once, here, and joined after `.run().await?` below --
    // see `audit::AuditWorker::shutdown` for why that join can't hang.
    let (audit_logger, audit_worker) = AuditLogger::spawn();

    // The async sibling of the audit trail above: `consumer::channel`
    // stands in for subscribing to a message topic (Kafka, SQS, ...), and
    // `tokio::spawn` -- not `thread::spawn` -- starts the background task
    // that consumes it, running concurrently with the HTTP server below on
    // the same Tokio runtime. See `consumer.rs` for the full picture,
    // including what swapping in a real broker would change.
    let (event_publisher, event_receiver) = consumer::channel();
    let event_consumer = tokio::spawn(consumer::run(event_receiver));

    let bind_address = (config.server.host.clone(), config.server.port);
    info!("Listening on http://{}:{}", bind_address.0, bind_address.1);

    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(recent_tasks.clone()))
            .app_data(web::Data::new(audit_logger.clone()))
            .app_data(web::Data::new(event_publisher.clone()))
            .configure(routes::tasks::configure)
    })
    .bind(bind_address)?
    .run()
    .await?;

    // By the time `.run().await` above resolves, actix-web has already
    // stopped every worker and dropped every `App` instance it built (and,
    // with it, every `web::Data<AuditLogger>` / `web::Data<EventPublisher>`
    // clone handed to a handler) -- so every sender either background
    // consumer had is already gone, and both of the joins below return as
    // soon as their consumer drains what's left.
    audit_worker.shutdown();
    // `.await` on a `tokio::task::JoinHandle` is the async counterpart of
    // `std::thread::JoinHandle::join()` above -- same "wait for the
    // background consumer to actually finish" idea, async instead of
    // blocking the calling thread while it waits.
    event_consumer.await.expect("event consumer task panicked");

    // Flush any buffered spans before the process exits.
    if let Some(provider) = tracer_provider {
        if let Err(error) = provider.shutdown() {
            eprintln!("Failed to flush OTEL spans: {error}");
        }
    }

    Ok(())
}
