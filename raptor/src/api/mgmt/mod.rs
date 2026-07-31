//! Management (`/rest/v1/...`) API: each submodule owns one resource family
//! and exposes a `routes()` (or `router()`) building just its own routes;
//! [`router`] merges them all and applies the `mgmt_auth` middleware once,
//! covering every merged route. `login` is deliberately excluded — see its
//! module doc.

pub mod actions;
pub mod artifacts;
pub mod distribution_sets;
pub mod login;
pub mod mappers;
pub mod metadata;
pub mod rollouts;
pub mod software_modules;
pub mod system;
pub mod tags;
pub mod target_filters;
pub mod targets;
pub mod types;

use crate::state::AppState;
use axum::Router;
use axum::middleware;

pub fn router(state: AppState) -> Router<AppState> {
    let max_artifact_size = state.cfg.max_artifact_size as usize;
    Router::new()
        .merge(software_modules::routes())
        .merge(artifacts::routes(max_artifact_size))
        .merge(types::routes())
        .merge(metadata::routes())
        .merge(targets::routes())
        .merge(distribution_sets::routes())
        .merge(actions::routes())
        .merge(rollouts::routes())
        .merge(target_filters::routes())
        .merge(tags::routes())
        .merge(system::routes())
        .merge(login::gated_routes())
        .route_layer(middleware::from_fn_with_state(
            state,
            crate::auth::mgmt::mgmt_auth,
        ))
}
