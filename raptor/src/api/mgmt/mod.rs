pub mod actions;
pub mod artifacts;
pub mod distribution_sets;
pub mod dto;
pub mod software_modules;
pub mod targets;
pub mod types;

use crate::state::AppState;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

pub fn router(state: AppState) -> Router<AppState> {
    let max_artifact_size = state.cfg.max_artifact_size as usize;
    Router::new()
        .route("/rest/v1/softwaremodules", post(software_modules::create).get(software_modules::list))
        .route("/rest/v1/softwaremodules/{id}",
            get(software_modules::get_one).put(software_modules::update).delete(software_modules::delete))
        .route("/rest/v1/softwaremoduletypes", get(types::sm_types))
        .route("/rest/v1/softwaremoduletypes/{id}", get(types::sm_type))
        .route("/rest/v1/distributionsettypes", get(types::ds_types))
        .route("/rest/v1/distributionsettypes/{id}", get(types::ds_type))
        .route("/rest/v1/softwaremodules/{id}/artifacts", post(artifacts::upload).get(artifacts::list)
            .layer(axum::extract::DefaultBodyLimit::max(max_artifact_size)))
        .route("/rest/v1/softwaremodules/{id}/artifacts/{aid}", get(artifacts::get_one).delete(artifacts::delete))
        .route("/rest/v1/softwaremodules/{id}/artifacts/{aid}/download", get(artifacts::download))
        .route("/rest/v1/targets", post(targets::create).get(targets::list))
        .route("/rest/v1/targets/{cid}", get(targets::get_one).put(targets::update).delete(targets::delete))
        .route("/rest/v1/targets/{cid}/attributes", get(targets::attributes))
        .route("/rest/v1/distributionsets", post(distribution_sets::create).get(distribution_sets::list))
        .route("/rest/v1/distributionsets/{id}", get(distribution_sets::get_one).delete(distribution_sets::delete))
        .route("/rest/v1/distributionsets/{id}/assignedSM", post(distribution_sets::assign_modules).get(distribution_sets::assigned_modules))
        .route("/rest/v1/targets/{cid}/assignedDS", post(actions::assign).get(actions::assigned_ds))
        .route("/rest/v1/targets/{cid}/installedDS", get(actions::installed_ds))
        .route("/rest/v1/targets/{cid}/actions", get(actions::target_actions))
        .route("/rest/v1/targets/{cid}/actions/{aid}", get(actions::target_action).delete(actions::cancel_action))
        .route("/rest/v1/actions", get(actions::all_actions))
        .route_layer(middleware::from_fn_with_state(state, crate::auth::mgmt::mgmt_auth))
}
