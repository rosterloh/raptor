pub mod config_data;
pub mod root;

use crate::state::AppState;
use axum::middleware;
use axum::routing::{get, put};
use axum::Router;

/// Canonical DDI base for a controller; raptor always emits tenant DEFAULT in links.
pub fn ddi_base(base: &str, cid: &str) -> String {
    format!("{base}/DEFAULT/controller/v1/{cid}")
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/{tenant}/controller/v1/{controllerId}", get(root::poll))
        .route("/{tenant}/controller/v1/{controllerId}/configData", put(config_data::put_config_data))
        .route_layer(middleware::from_fn_with_state(state, crate::auth::ddi::ddi_auth))
}
