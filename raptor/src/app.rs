use crate::state::AppState;
use axum::routing::get;
use axum::Router;

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}
