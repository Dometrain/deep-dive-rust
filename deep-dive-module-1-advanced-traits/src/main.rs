use actix_web::{web, App, HttpServer};
use std::io;
use todo_traits_app::{
    config::load_config,
    db::{create_pool, initialize_database},
    routes, telemetry,
};
use tracing::{info, trace};
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
    trace!("Starting todo-traits-app");

    let pool = create_pool(&config.database.url)
        .await
        .map_err(io::Error::other)?;
    initialize_database(&pool).await.map_err(io::Error::other)?;

    let bind_address = (config.server.host.clone(), config.server.port);
    info!("Listening on http://{}:{}", bind_address.0, bind_address.1);

    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(config.clone()))
            .configure(routes::tasks::configure)
    })
    .bind(bind_address)?
    .run()
    .await?;

    // Flush any buffered spans before the process exits.
    if let Some(provider) = tracer_provider {
        if let Err(error) = provider.shutdown() {
            eprintln!("Failed to flush OTEL spans: {error}");
        }
    }

    Ok(())
}
