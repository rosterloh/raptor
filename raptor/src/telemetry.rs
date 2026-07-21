//! Telemetry setup: the tracing subscriber, and — behind the `otel` feature —
//! OpenTelemetry (OTLP) export of traces, metrics and logs.
//!
//! [`init`] is always called from `main`. Without an `[otel]` config section
//! (or without the `otel` build feature) it installs exactly the previous
//! stdout `fmt` subscriber and returns a no-op [`crate::metrics::Metrics`]. With
//! a config present it additionally layers a `tracing-opentelemetry` span
//! exporter and an `opentelemetry-appender-tracing` logs bridge on top of the
//! same `fmt` layer (stdout logging is never removed), and wires up a meter.
//!
//! [`TelemetryGuard`] must be kept alive for the process lifetime; call
//! [`TelemetryGuard::shutdown`] on graceful shutdown to flush the exporters.

use crate::config::OtelConfig;
use crate::metrics::Metrics;

type BoxError = Box<dyn std::error::Error>;

/// Default tracing filter, matching raptor's historical stdout behaviour.
const DEFAULT_FILTER: &str = "raptor=info,tower_http=info";

/// Holds the OTLP providers so they can be flushed on shutdown. Empty (and
/// cheap) when export is not enabled.
#[derive(Default)]
pub struct TelemetryGuard {
    #[cfg(feature = "otel")]
    tracer: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    #[cfg(feature = "otel")]
    meter: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
    #[cfg(feature = "otel")]
    logger: Option<opentelemetry_sdk::logs::SdkLoggerProvider>,
}

impl TelemetryGuard {
    /// Flush and shut down the OTLP exporters. Safe to call when export is off.
    pub fn shutdown(&self) {
        #[cfg(feature = "otel")]
        {
            if let Some(t) = &self.tracer {
                if let Err(e) = t.shutdown() {
                    eprintln!("otel tracer shutdown: {e}");
                }
            }
            if let Some(m) = &self.meter {
                if let Err(e) = m.shutdown() {
                    eprintln!("otel meter shutdown: {e}");
                }
            }
            if let Some(l) = &self.logger {
                if let Err(e) = l.shutdown() {
                    eprintln!("otel logger shutdown: {e}");
                }
            }
        }
    }
}

fn env_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| DEFAULT_FILTER.into())
}

/// Install the global tracing subscriber and, if `otel` is `Some` and the
/// build has the `otel` feature, the OTLP export pipeline.
pub fn init(otel: Option<&OtelConfig>) -> Result<(TelemetryGuard, Metrics), BoxError> {
    #[cfg(feature = "otel")]
    if let Some(cfg) = otel {
        return init_otel(cfg);
    }
    #[cfg(not(feature = "otel"))]
    let _ = otel; // export ignored without the feature; parse-and-ignore is intentional

    tracing_subscriber::fmt()
        .with_env_filter(env_filter())
        .init();
    Ok((TelemetryGuard::default(), Metrics::disabled()))
}

#[cfg(feature = "otel")]
fn init_otel(cfg: &OtelConfig) -> Result<(TelemetryGuard, Metrics), BoxError> {
    use opentelemetry::metrics::MeterProvider as _;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use opentelemetry_sdk::Resource;
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;

    let resource = Resource::builder()
        .with_service_name(cfg.service_name.clone())
        .build();

    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter(cfg)?)
        .with_resource(resource.clone())
        .build();

    let meter_provider = SdkMeterProvider::builder()
        .with_reader(PeriodicReader::builder(metric_exporter(cfg)?).build())
        .with_resource(resource.clone())
        .build();

    let logger_provider = opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter(cfg)?)
        .with_resource(resource)
        .build();

    // Make the providers globally discoverable (e.g. for any `global::meter`
    // callers) in addition to the handles we thread through explicitly.
    opentelemetry::global::set_tracer_provider(tracer_provider.clone());
    opentelemetry::global::set_meter_provider(meter_provider.clone());

    let tracer = tracer_provider.tracer("raptor");
    let meter = meter_provider.meter("raptor");
    let metrics = Metrics::new(&meter);

    let log_bridge =
        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider);

    tracing_subscriber::registry()
        .with(env_filter())
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(log_bridge)
        .try_init()?;

    tracing::info!(
        endpoint = %cfg.endpoint,
        protocol = ?cfg.protocol,
        service_name = %cfg.service_name,
        "OpenTelemetry OTLP export enabled"
    );

    Ok((
        TelemetryGuard {
            tracer: Some(tracer_provider),
            meter: Some(meter_provider),
            logger: Some(logger_provider),
        },
        metrics,
    ))
}

/// Build a gRPC request-metadata map from the configured headers.
#[cfg(feature = "otel")]
fn grpc_metadata(
    headers: &std::collections::HashMap<String, String>,
) -> tonic::metadata::MetadataMap {
    use tonic::metadata::{MetadataKey, MetadataValue};
    let mut md = tonic::metadata::MetadataMap::with_capacity(headers.len());
    for (k, v) in headers {
        if let (Ok(key), Ok(val)) = (
            MetadataKey::from_bytes(k.as_bytes()),
            MetadataValue::try_from(v),
        ) {
            md.insert(key, val);
        }
    }
    md
}

#[cfg(feature = "otel")]
fn span_exporter(cfg: &OtelConfig) -> Result<opentelemetry_otlp::SpanExporter, BoxError> {
    use crate::config::OtelProtocol;
    use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig, WithTonicConfig};
    Ok(match cfg.protocol {
        OtelProtocol::Grpc => SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&cfg.endpoint)
            .with_metadata(grpc_metadata(&cfg.headers))
            .build()?,
        OtelProtocol::Http => SpanExporter::builder()
            .with_http()
            .with_endpoint(&cfg.endpoint)
            .with_headers(cfg.headers.clone())
            .build()?,
    })
}

#[cfg(feature = "otel")]
fn metric_exporter(cfg: &OtelConfig) -> Result<opentelemetry_otlp::MetricExporter, BoxError> {
    use crate::config::OtelProtocol;
    use opentelemetry_otlp::{MetricExporter, WithExportConfig, WithHttpConfig, WithTonicConfig};
    Ok(match cfg.protocol {
        OtelProtocol::Grpc => MetricExporter::builder()
            .with_tonic()
            .with_endpoint(&cfg.endpoint)
            .with_metadata(grpc_metadata(&cfg.headers))
            .build()?,
        OtelProtocol::Http => MetricExporter::builder()
            .with_http()
            .with_endpoint(&cfg.endpoint)
            .with_headers(cfg.headers.clone())
            .build()?,
    })
}

#[cfg(feature = "otel")]
fn log_exporter(cfg: &OtelConfig) -> Result<opentelemetry_otlp::LogExporter, BoxError> {
    use crate::config::OtelProtocol;
    use opentelemetry_otlp::{LogExporter, WithExportConfig, WithHttpConfig, WithTonicConfig};
    Ok(match cfg.protocol {
        OtelProtocol::Grpc => LogExporter::builder()
            .with_tonic()
            .with_endpoint(&cfg.endpoint)
            .with_metadata(grpc_metadata(&cfg.headers))
            .build()?,
        OtelProtocol::Http => LogExporter::builder()
            .with_http()
            .with_endpoint(&cfg.endpoint)
            .with_headers(cfg.headers.clone())
            .build()?,
    })
}
