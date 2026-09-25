use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use tracing::Subscriber;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::registry::LookupSpan;

/// Builds an OpenTelemetry layer that exports spans over OTLP (gRPC) to the
/// endpoint from the standard `OTEL_EXPORTER_OTLP_ENDPOINT` env var
/// (default: `http://localhost:4317`), e.g. a Jaeger v2 collector.
///
/// Returns `None` when `OTEL_EXPORTER_OTLP_ENDPOINT` is unset, so tracing
/// stays stdout-only unless an OTLP backend is explicitly configured.
pub fn otel_layer<S>() -> Option<(
    OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>,
    SdkTracerProvider,
)>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;

    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("OTLP span exporter should build");

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder()
                .with_service_name("todo-web-app")
                .build(),
        )
        .build();

    let tracer = provider.tracer("todo-web-app");
    Some((OpenTelemetryLayer::new(tracer), provider))
}
