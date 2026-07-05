pub mod config_data;
pub mod deployment;
pub mod feedback;
pub mod root;

use crate::state::AppState;
use axum::middleware;
use axum::routing::{get, post, put};
use axum::Router;

/// Canonical DDI base for a controller; raptor always emits tenant DEFAULT in links.
pub fn ddi_base(base: &str, cid: &str) -> String {
    format!("{base}/DEFAULT/controller/v1/{cid}")
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/{tenant}/controller/v1/{controllerId}", get(root::poll))
        .route("/{tenant}/controller/v1/{controllerId}/configData", put(config_data::put_config_data))
        .route("/{tenant}/controller/v1/{controllerId}/deploymentBase/{actionId}", get(deployment::deployment_base))
        .route("/{tenant}/controller/v1/{controllerId}/deploymentBase/{actionId}/feedback", post(feedback::deployment_feedback))
        .route("/{tenant}/controller/v1/{controllerId}/cancelAction/{actionId}", get(feedback::cancel_action))
        .route("/{tenant}/controller/v1/{controllerId}/cancelAction/{actionId}/feedback", post(feedback::cancel_feedback))
        .route_layer(middleware::from_fn_with_state(state, crate::auth::ddi::ddi_auth))
}
