//! Assembles the axum `Router`: merges the mgmt and DDI routers (and the
//! embedded UI, when built with `embed-ui`), and — when metrics export is
//! enabled — layers request-metrics middleware. No route logic of its own;
//! that lives in `api::mgmt` and `api::ddi`.

use crate::metrics;
use crate::state::AppState;
use axum::Router;
use axum::extract::{MatchedPath, Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;

pub fn build_app(state: AppState) -> Router {
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(crate::api::mgmt::login::routes())
        .merge(crate::api::mgmt::router(state.clone()))
        .merge(crate::api::ddi::router(state.clone()));
    #[cfg(feature = "embed-ui")]
    let app = app
        .route("/ui", get(crate::ui::serve))
        .route("/ui/{*path}", get(crate::ui::serve));
    let app = app
        .layer(middleware::from_fn(log_mgmt_writes))
        .layer(tower_http::trace::TraceLayer::new_for_http());
    // Only attach the metrics middleware when export is live, so builds without
    // OTLP configured carry no per-request instrumentation overhead.
    let app = if state.metrics.enabled() {
        app.layer(middleware::from_fn_with_state(state.clone(), track_metrics))
    } else {
        app
    };
    app.with_state(state)
}

/// Classify a matched route into a low-cardinality API label.
fn api_group(route: &str) -> &'static str {
    if route.contains("/controller/v1/") {
        metrics::API_DDI
    } else if route.starts_with("/rest/") {
        metrics::API_MGMT
    } else {
        metrics::API_OTHER
    }
}

/// One `info` line per Management API write: the operator's audit trail.
/// Reads stay at `TraceLayer`'s debug level, as do DDI polls — the console
/// and TUI poll list endpoints every few seconds, and a fleet polls
/// constantly, either of which would bury the writes.
async fn log_mgmt_writes(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    if method.is_safe() || api_group(&path) != metrics::API_MGMT {
        return next.run(req).await;
    }
    let start = std::time::Instant::now();
    let resp = next.run(req).await;
    tracing::info!(
        %method,
        path,
        status = resp.status().as_u16(),
        latency_ms = start.elapsed().as_millis() as u64,
        "mgmt request"
    );
    resp
}

/// Records request count + duration keyed by the *matched* route template
/// (placeholders, not concrete ids) so metric cardinality stays bounded.
async fn track_metrics(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let start = std::time::Instant::now();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());
    let method = req.method().as_str().to_string();
    let api = api_group(&route);
    let resp = next.run(req).await;
    state.metrics.record_http(
        api,
        &route,
        &method,
        resp.status().as_u16(),
        start.elapsed().as_secs_f64(),
    );
    resp
}
