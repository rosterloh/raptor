use crate::metrics;
use crate::state::AppState;
use axum::extract::{MatchedPath, Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;

pub fn build_app(state: AppState) -> Router {
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/rest/v1/login", post(crate::api::mgmt::login::login))
        .route("/rest/v1/logout", post(crate::api::mgmt::login::logout))
        .merge(crate::api::mgmt::router(state.clone()))
        .merge(crate::api::ddi::router(state.clone()));
    #[cfg(feature = "embed-ui")]
    let app = app
        .route("/ui", get(crate::ui::serve))
        .route("/ui/{*path}", get(crate::ui::serve));
    let app = app.layer(tower_http::trace::TraceLayer::new_for_http());
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
