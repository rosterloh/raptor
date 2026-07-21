//! OTLP telemetry wiring. Gated on the `otel` feature — run with
//! `cargo test -p raptor --features otel --test telemetry`.
//!
//! This is a single-test binary on purpose: `telemetry::init` installs the
//! process-global tracing subscriber and OTel providers, which can only happen
//! once per process. It asserts the subscriber/meter come up cleanly with an
//! `[otel]` config present; full pipeline verification against a live collector
//! is manual (see the README "Observability" section).
#![cfg(feature = "otel")]

use raptor::config::{OtelConfig, OtelProtocol};

// `init` builds a tonic/hyper channel, so it must run inside a Tokio runtime —
// exactly as `main` (`#[tokio::main]`) does in production.
#[tokio::test(flavor = "multi_thread")]
async fn otel_pipeline_initializes_and_records() {
    // Endpoint points at a (very likely dead) local port. Exporter construction
    // is lazy — build() succeeds without a live collector; only background
    // export attempts would fail, which is fine for this assertion.
    let otel = OtelConfig {
        endpoint: "http://127.0.0.1:4317".into(),
        service_name: "raptor-test".into(),
        protocol: OtelProtocol::Grpc,
        headers: std::collections::HashMap::new(),
    };

    let (guard, metrics) =
        raptor::telemetry::init(Some(&otel)).expect("otel subscriber/meter should initialize");

    assert!(
        metrics.enabled(),
        "metrics handle must be live when an [otel] endpoint is configured"
    );

    // Exercise every instrument to prove the meter accepts recordings.
    metrics.action_created();
    metrics.action_finished();
    metrics.action_failed();
    metrics.action_canceled();
    metrics.bytes_uploaded(1024);
    metrics.bytes_downloaded(2048);
    metrics.auth_failure("ddi");
    metrics.record_http("mgmt", "/rest/v1/targets", "GET", 200, 0.012);
    metrics.observe_fleet(&[("pending".into(), 3), ("in_sync".into(), 7)], 2);

    // Flush + shut down the exporters; must not panic.
    guard.shutdown();
}
