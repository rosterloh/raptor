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
//! With export on, incoming W3C `traceparent` headers are honoured (see
//! [`propagate_trace_context`]) so raptor's spans join a caller's trace, and
//! the OTel crates' own diagnostics (export failures and the like) reach
//! stdout at `warn` but are kept out of the OTLP pipeline itself.
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
            if let Some(t) = &self.tracer
                && let Err(e) = t.shutdown()
            {
                eprintln!("otel tracer shutdown: {e}");
            }
            if let Some(m) = &self.meter
                && let Err(e) = m.shutdown()
            {
                eprintln!("otel meter shutdown: {e}");
            }
            if let Some(l) = &self.logger
                && let Err(e) = l.shutdown()
            {
                eprintln!("otel logger shutdown: {e}");
            }
        }
    }
}

fn env_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| DEFAULT_FILTER.into())
}

/// Prefix of the tracing targets the OpenTelemetry crates report their own
/// diagnostics on (`opentelemetry_sdk`, `opentelemetry-otlp`, ...).
#[cfg(feature = "otel")]
const OTEL_TARGET_PREFIX: &str = "opentelemetry";

/// Filter for the stdout `fmt` layer when export is on: the usual filter plus
/// the SDK's warnings and errors, so an unreachable collector is visible.
/// Left alone when `RUST_LOG` already says what it wants for those targets.
#[cfg(feature = "otel")]
fn stdout_filter() -> tracing_subscriber::EnvFilter {
    let filter = env_filter();
    let env_names_otel = std::env::var("RUST_LOG").is_ok_and(|v| v.contains(OTEL_TARGET_PREFIX));
    if env_names_otel {
        filter
    } else {
        filter.add_directive("opentelemetry=warn".parse().expect("static directive"))
    }
}

/// Whether an event or span may be exported over OTLP. The exporters' own
/// stack (the OTel SDK and the HTTP/gRPC clients under it) is excluded:
/// exporting its events would feed a failing export back into itself.
#[cfg(feature = "otel")]
fn exportable(meta: &tracing::Metadata<'_>) -> bool {
    exportable_target(meta.target())
}

#[cfg(feature = "otel")]
fn exportable_target(target: &str) -> bool {
    const EXPORTER_TARGETS: [&str; 5] = [OTEL_TARGET_PREFIX, "hyper", "h2", "tonic", "reqwest"];
    !EXPORTER_TARGETS.iter().any(|t| target.starts_with(t))
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
    use opentelemetry_sdk::Resource;
    use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
    use opentelemetry_sdk::propagation::TraceContextPropagator;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing_subscriber::Layer as _;
    use tracing_subscriber::filter::{FilterExt as _, filter_fn};
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;

    let resource = Resource::builder()
        .with_service_name(cfg.service_name.clone())
        .build();

    let mut tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter(cfg)?)
        .with_resource(resource.clone());
    // Unset keeps the SDK default, which honours OTEL_TRACES_SAMPLER.
    if let Some(ratio) = cfg.sample_ratio {
        tracer_provider = tracer_provider.with_sampler(trace_sampler(ratio)?);
    }
    let tracer_provider = tracer_provider.build();

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
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer = tracer_provider.tracer("raptor");
    let meter = meter_provider.meter("raptor");
    let metrics = Metrics::new(&meter);

    let log_bridge =
        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(stdout_filter()))
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(env_filter().and(filter_fn(exportable))),
        )
        .with(log_bridge.with_filter(env_filter().and(filter_fn(exportable))))
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

/// Parent-based sampling: a request carrying a `traceparent` keeps the
/// caller's decision; traces raptor starts itself are sampled at `ratio`.
#[cfg(feature = "otel")]
fn trace_sampler(ratio: f64) -> Result<opentelemetry_sdk::trace::ParentBasedSampler, BoxError> {
    use opentelemetry_sdk::trace::Sampler;
    if !(0.0..=1.0).contains(&ratio) {
        return Err(format!("otel.sample_ratio must be between 0.0 and 1.0, got {ratio}").into());
    }
    Ok(Sampler::parent_based(Sampler::TraceIdRatioBased(ratio)))
}

/// Reads W3C trace-context headers off an incoming request.
#[cfg(feature = "otel")]
struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

#[cfg(feature = "otel")]
impl opentelemetry::propagation::Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Middleware: run the request under the trace context its `traceparent`
/// header carries, so the request span and everything beneath it become
/// children of the caller's span. Must sit outside `TraceLayer`, which opens
/// the request span. Without the header it is a no-op.
#[cfg(feature = "otel")]
pub async fn propagate_trace_context(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use opentelemetry::context::FutureExt as _;
    let parent = opentelemetry::global::get_text_map_propagator(|p| {
        p.extract(&HeaderExtractor(req.headers()))
    });
    next.run(req).with_context(parent).await
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

#[cfg(all(test, feature = "otel"))]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use opentelemetry::trace::{TraceContextExt as _, TracerProvider as _};
    use opentelemetry_sdk::propagation::TraceContextPropagator;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tower::ServiceExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;
    use tracing_subscriber::layer::SubscriberExt as _;

    const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";

    /// Send one request through `propagate_trace_context` and report the
    /// trace id and sampling decision of a span opened inside the handler.
    async fn handler_span(sample_ratio: f64, traceparent: Option<&str>) -> (String, bool) {
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
        let provider = SdkTracerProvider::builder()
            .with_sampler(trace_sampler(sample_ratio).unwrap())
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
        let _guard = tracing::subscriber::set_default(subscriber);

        let app = Router::new()
            .route(
                "/",
                get(|| async {
                    let cx = tracing::info_span!("handler").context();
                    let sc = cx.span().span_context().clone();
                    format!("{} {}", sc.trace_id(), sc.is_sampled())
                }),
            )
            .layer(axum::middleware::from_fn(propagate_trace_context));
        let mut req = Request::get("/");
        if let Some(tp) = traceparent {
            req = req.header("traceparent", tp);
        }
        let resp = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        let (id, sampled) = body.split_once(' ').unwrap();
        (id.to_string(), sampled == "true")
    }

    #[tokio::test]
    async fn incoming_traceparent_is_joined_and_its_sampling_followed() {
        let tp = format!("00-{TRACE_ID}-00f067aa0ba902b7-01");
        let (id, sampled) = handler_span(0.0, Some(&tp)).await;
        assert_eq!(id, TRACE_ID);
        assert!(sampled, "a sampled caller wins over sample_ratio = 0");
    }

    #[tokio::test]
    async fn root_traces_follow_sample_ratio() {
        let (id, sampled) = handler_span(0.0, None).await;
        assert_ne!(id, TRACE_ID);
        assert!(!sampled);
        let (_, sampled) = handler_span(1.0, None).await;
        assert!(sampled);
    }

    #[test]
    fn sample_ratio_out_of_range_is_rejected() {
        assert!(trace_sampler(1.5).is_err());
        assert!(trace_sampler(-0.1).is_err());
        assert!(trace_sampler(f64::NAN).is_err());
    }

    #[test]
    fn exporter_stack_is_not_exported() {
        assert!(exportable_target("raptor::api"));
        assert!(exportable_target("tower_http::trace"));
        assert!(!exportable_target("opentelemetry_sdk"));
        assert!(!exportable_target("opentelemetry-otlp"));
        assert!(!exportable_target("hyper_util::client"));
    }
}
