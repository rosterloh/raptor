//! Device (`/{tenant}/controller/v1/...`) API: each submodule owns one
//! DDI resource — polling base (`root`), config data, deployment/installed
//! base, feedback/cancel, and confirmation flow. [`router`] wires the routes
//! and applies `ddi_auth` middleware once, covering every route below it.

pub mod artifacts;
pub mod config_data;
pub mod confirmation;
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

/// DDI base for the plain-HTTP artifact links (`download-http`), from
/// `[ddi] artifact_http_url`. `None` — the default — means "reuse the normal
/// base", which is what keeps single-scheme deployments working.
pub fn ddi_http_base(cfg: &crate::config::Config, cid: &str) -> Option<String> {
    cfg.ddi
        .artifact_http_url
        .as_deref()
        .map(|u| ddi_base(u.trim_end_matches('/'), cid))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/{tenant}/controller/v1/{controllerId}", get(root::poll))
        .route("/{tenant}/controller/v1/{controllerId}/configData", put(config_data::put_config_data))
        .route("/{tenant}/controller/v1/{controllerId}/deploymentBase/{actionId}", get(deployment::deployment_base))
        .route("/{tenant}/controller/v1/{controllerId}/deploymentBase/{actionId}/feedback", post(feedback::deployment_feedback))
        .route("/{tenant}/controller/v1/{controllerId}/confirmationBase/{actionId}", get(confirmation::confirmation_base))
        .route("/{tenant}/controller/v1/{controllerId}/confirmationBase/{actionId}/feedback", post(confirmation::confirmation_feedback))
        .route("/{tenant}/controller/v1/{controllerId}/confirmationBase/activateAutoConfirm", post(confirmation::activate_auto_confirm))
        .route("/{tenant}/controller/v1/{controllerId}/confirmationBase/deactivateAutoConfirm", post(confirmation::deactivate_auto_confirm))
        .route("/{tenant}/controller/v1/{controllerId}/cancelAction/{actionId}", get(feedback::cancel_action))
        .route("/{tenant}/controller/v1/{controllerId}/cancelAction/{actionId}/feedback", post(feedback::cancel_feedback))
        .route("/{tenant}/controller/v1/{controllerId}/installedBase/{actionId}", get(deployment::installed_base))
        .route("/{tenant}/controller/v1/{controllerId}/softwaremodules/{moduleId}/artifacts", get(artifacts::list))
        .route("/{tenant}/controller/v1/{controllerId}/softwaremodules/{moduleId}/artifacts/{filename}", get(artifacts::download))
        .route_layer(middleware::from_fn_with_state(state, crate::auth::ddi::ddi_auth))
}
