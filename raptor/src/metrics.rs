//! Application metrics.
//!
//! This module is compiled unconditionally so call sites stay clean — handlers
//! and domain code call `state.metrics.action_created()` and friends without
//! `#[cfg]` noise. When the `otel` feature is off (or OTLP export is not
//! configured), every method is an inlined no-op and the handle is zero-sized,
//! so there is no runtime cost.
//!
//! The instruments mirror what matters for an OTA server: HTTP request volume
//! and latency (DDI polling is the capacity signal), deployment action
//! lifecycle counts, artifact bytes moved, auth failures, and gauges for
//! fleet state (targets by update status, active actions).

/// Which API surface an HTTP request hit — kept low-cardinality on purpose.
pub const API_DDI: &str = "ddi";
pub const API_MGMT: &str = "mgmt";
pub const API_OTHER: &str = "other";

#[cfg(not(feature = "otel"))]
mod imp {
    /// No-op metrics handle used when the `otel` feature is disabled.
    #[derive(Clone, Default)]
    pub struct Metrics;

    impl Metrics {
        pub fn disabled() -> Self {
            Metrics
        }
        #[inline]
        pub fn enabled(&self) -> bool {
            false
        }
        #[inline]
        pub fn record_http(&self, _api: &str, _route: &str, _method: &str, _status: u16, _s: f64) {}
        #[inline]
        pub fn action_created(&self) {}
        #[inline]
        pub fn action_finished(&self) {}
        #[inline]
        pub fn action_failed(&self) {}
        #[inline]
        pub fn action_canceled(&self) {}
        #[inline]
        pub fn bytes_uploaded(&self, _n: u64) {}
        #[inline]
        pub fn bytes_downloaded(&self, _n: u64) {}
        #[inline]
        pub fn auth_failure(&self, _zone: &str) {}
        #[inline]
        pub fn observe_fleet(&self, _by_status: &[(String, i64)], _active_actions: i64) {}
    }
}

#[cfg(feature = "otel")]
mod imp {
    use opentelemetry::metrics::{Counter, Gauge, Histogram, Meter};
    use opentelemetry::KeyValue;
    use std::sync::Arc;

    struct Instruments {
        http_requests: Counter<u64>,
        http_duration: Histogram<f64>,
        actions_created: Counter<u64>,
        actions_finished: Counter<u64>,
        actions_failed: Counter<u64>,
        actions_canceled: Counter<u64>,
        artifact_bytes_uploaded: Counter<u64>,
        artifact_bytes_downloaded: Counter<u64>,
        auth_failures: Counter<u64>,
        targets_by_status: Gauge<u64>,
        active_actions: Gauge<u64>,
    }

    impl Instruments {
        fn new(meter: &Meter) -> Self {
            Self {
                http_requests: meter
                    .u64_counter("raptor.http.requests")
                    .with_description("HTTP requests handled, by api/route/method/status")
                    .build(),
                http_duration: meter
                    .f64_histogram("raptor.http.request.duration")
                    .with_unit("s")
                    .with_description("HTTP request duration in seconds")
                    .build(),
                actions_created: meter
                    .u64_counter("raptor.actions.created")
                    .with_description("Deployment actions created")
                    .build(),
                actions_finished: meter
                    .u64_counter("raptor.actions.finished")
                    .with_description("Deployment actions that finished successfully")
                    .build(),
                actions_failed: meter
                    .u64_counter("raptor.actions.failed")
                    .with_description("Deployment actions that ended in error")
                    .build(),
                actions_canceled: meter
                    .u64_counter("raptor.actions.canceled")
                    .with_description("Deployment actions canceled")
                    .build(),
                artifact_bytes_uploaded: meter
                    .u64_counter("raptor.artifact.bytes.uploaded")
                    .with_unit("By")
                    .with_description("Artifact bytes uploaded via the management API")
                    .build(),
                artifact_bytes_downloaded: meter
                    .u64_counter("raptor.artifact.bytes.downloaded")
                    .with_unit("By")
                    .with_description("Artifact bytes served to devices/operators")
                    .build(),
                auth_failures: meter
                    .u64_counter("raptor.auth.failures")
                    .with_description("Rejected requests, by zone (ddi/mgmt)")
                    .build(),
                targets_by_status: meter
                    .u64_gauge("raptor.targets")
                    .with_description("Targets by update_status")
                    .build(),
                active_actions: meter
                    .u64_gauge("raptor.actions.active")
                    .with_description("Currently active deployment actions")
                    .build(),
            }
        }
    }

    /// Cheap-to-clone metrics handle. `None` when export is not configured, in
    /// which case every method returns immediately.
    #[derive(Clone, Default)]
    pub struct Metrics {
        inner: Option<Arc<Instruments>>,
    }

    impl Metrics {
        pub fn disabled() -> Self {
            Metrics { inner: None }
        }

        pub fn new(meter: &Meter) -> Self {
            Metrics {
                inner: Some(Arc::new(Instruments::new(meter))),
            }
        }

        #[inline]
        pub fn enabled(&self) -> bool {
            self.inner.is_some()
        }

        pub fn record_http(&self, api: &str, route: &str, method: &str, status: u16, secs: f64) {
            let Some(i) = &self.inner else { return };
            let attrs = [
                KeyValue::new("api", api.to_string()),
                KeyValue::new("route", route.to_string()),
                KeyValue::new("method", method.to_string()),
                KeyValue::new("status", status as i64),
            ];
            i.http_requests.add(1, &attrs);
            i.http_duration.record(secs, &attrs);
        }

        pub fn action_created(&self) {
            if let Some(i) = &self.inner {
                i.actions_created.add(1, &[]);
            }
        }
        pub fn action_finished(&self) {
            if let Some(i) = &self.inner {
                i.actions_finished.add(1, &[]);
            }
        }
        pub fn action_failed(&self) {
            if let Some(i) = &self.inner {
                i.actions_failed.add(1, &[]);
            }
        }
        pub fn action_canceled(&self) {
            if let Some(i) = &self.inner {
                i.actions_canceled.add(1, &[]);
            }
        }
        pub fn bytes_uploaded(&self, n: u64) {
            if let Some(i) = &self.inner {
                i.artifact_bytes_uploaded.add(n, &[]);
            }
        }
        pub fn bytes_downloaded(&self, n: u64) {
            if let Some(i) = &self.inner {
                i.artifact_bytes_downloaded.add(n, &[]);
            }
        }
        pub fn auth_failure(&self, zone: &str) {
            if let Some(i) = &self.inner {
                i.auth_failures
                    .add(1, &[KeyValue::new("zone", zone.to_string())]);
            }
        }

        /// Record fleet-state gauges. Called periodically from the background
        /// sweep so the observations track a real snapshot without needing an
        /// async observable callback.
        pub fn observe_fleet(&self, by_status: &[(String, i64)], active_actions: i64) {
            let Some(i) = &self.inner else { return };
            for (status, count) in by_status {
                i.targets_by_status.record(
                    *count as u64,
                    &[KeyValue::new("update_status", status.clone())],
                );
            }
            i.active_actions.record(active_actions.max(0) as u64, &[]);
        }
    }
}

pub use imp::Metrics;
