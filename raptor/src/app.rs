use crate::state::AppState;
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
    app.layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}
