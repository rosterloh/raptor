pub mod dto;
pub mod software_modules;
pub mod types;

use crate::state::AppState;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/rest/v1/softwaremodules", post(software_modules::create).get(software_modules::list))
        .route("/rest/v1/softwaremodules/{id}",
            get(software_modules::get_one).put(software_modules::update).delete(software_modules::delete))
        .route("/rest/v1/softwaremoduletypes", get(types::sm_types))
        .route("/rest/v1/softwaremoduletypes/{id}", get(types::sm_type))
        .route("/rest/v1/distributionsettypes", get(types::ds_types))
        .route("/rest/v1/distributionsettypes/{id}", get(types::ds_type))
        .route_layer(middleware::from_fn_with_state(state, crate::auth::mgmt::mgmt_auth))
}
