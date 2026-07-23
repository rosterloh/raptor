pub mod actions;
pub mod artifacts;
pub mod distribution_sets;
pub mod dto;
pub mod login;
pub mod rollouts;
pub mod software_modules;
pub mod system;
pub mod target_filters;
pub mod targets;
pub mod types;

use crate::state::AppState;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

pub fn router(state: AppState) -> Router<AppState> {
    let max_artifact_size = state.cfg.max_artifact_size as usize;
    Router::new()
        .route(
            "/rest/v1/softwaremodules",
            post(software_modules::create).get(software_modules::list),
        )
        .route(
            "/rest/v1/softwaremodules/{id}",
            get(software_modules::get_one)
                .put(software_modules::update)
                .delete(software_modules::delete),
        )
        .route(
            "/rest/v1/softwaremoduletypes",
            get(types::sm_types).post(types::sm_type_create),
        )
        .route(
            "/rest/v1/softwaremoduletypes/{id}",
            get(types::sm_type)
                .put(types::sm_type_update)
                .delete(types::sm_type_delete),
        )
        .route(
            "/rest/v1/distributionsettypes",
            get(types::ds_types).post(types::ds_type_create),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}",
            get(types::ds_type)
                .put(types::ds_type_update)
                .delete(types::ds_type_delete),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}/mandatorymoduletypes",
            get(types::ds_type_mandatory).post(types::ds_type_add_mandatory),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}/mandatorymoduletypes/{mid}",
            axum::routing::delete(types::ds_type_remove_module),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}/optionalmoduletypes",
            get(types::ds_type_optional).post(types::ds_type_add_optional),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}/optionalmoduletypes/{mid}",
            axum::routing::delete(types::ds_type_remove_module),
        )
        .route(
            "/rest/v1/targettypes",
            get(types::tt_list).post(types::tt_create),
        )
        .route(
            "/rest/v1/targettypes/{id}",
            get(types::tt_one)
                .put(types::tt_update)
                .delete(types::tt_delete),
        )
        .route(
            "/rest/v1/targettypes/{id}/compatibledistributionsettypes",
            get(types::tt_compat_list).post(types::tt_add_compat),
        )
        .route(
            "/rest/v1/targettypes/{id}/compatibledistributionsettypes/{dsid}",
            axum::routing::delete(types::tt_remove_compat),
        )
        .route(
            "/rest/v1/softwaremodules/{id}/artifacts",
            post(artifacts::upload)
                .get(artifacts::list)
                .layer(axum::extract::DefaultBodyLimit::max(max_artifact_size)),
        )
        .route(
            "/rest/v1/softwaremodules/{id}/artifacts/{aid}",
            get(artifacts::get_one).delete(artifacts::delete),
        )
        .route(
            "/rest/v1/softwaremodules/{id}/artifacts/{aid}/download",
            get(artifacts::download),
        )
        .route("/rest/v1/targets", post(targets::create).get(targets::list))
        .route(
            "/rest/v1/targets/{cid}",
            get(targets::get_one)
                .put(targets::update)
                .delete(targets::delete),
        )
        .route(
            "/rest/v1/targets/{cid}/attributes",
            get(targets::attributes),
        )
        .route(
            "/rest/v1/targets/{cid}/targettype",
            post(targets::assign_type).delete(targets::unassign_type),
        )
        .route(
            "/rest/v1/targets/{cid}/autoConfirm",
            get(targets::auto_confirm_status),
        )
        .route(
            "/rest/v1/targets/{cid}/autoConfirm/activate",
            post(targets::activate_auto_confirm),
        )
        .route(
            "/rest/v1/targets/{cid}/autoConfirm/deactivate",
            post(targets::deactivate_auto_confirm),
        )
        .route(
            "/rest/v1/distributionsets",
            post(distribution_sets::create).get(distribution_sets::list),
        )
        .route(
            "/rest/v1/distributionsets/{id}",
            get(distribution_sets::get_one)
                .put(distribution_sets::update)
                .delete(distribution_sets::delete),
        )
        .route(
            "/rest/v1/distributionsets/{id}/assignedSM",
            post(distribution_sets::assign_modules).get(distribution_sets::assigned_modules),
        )
        .route(
            "/rest/v1/distributionsets/{id}/invalidate",
            post(distribution_sets::invalidate),
        )
        .route(
            "/rest/v1/targets/{cid}/assignedDS",
            post(actions::assign).get(actions::assigned_ds),
        )
        .route(
            "/rest/v1/targets/{cid}/installedDS",
            get(actions::installed_ds),
        )
        .route(
            "/rest/v1/targets/{cid}/actions",
            get(actions::target_actions),
        )
        .route(
            "/rest/v1/targets/{cid}/actions/{aid}",
            get(actions::target_action).delete(actions::cancel_action),
        )
        .route(
            "/rest/v1/targets/{cid}/actions/{aid}/status",
            get(actions::action_status_history),
        )
        .route("/rest/v1/actions", get(actions::all_actions))
        .route(
            "/rest/v1/rollouts",
            post(rollouts::create).get(rollouts::list),
        )
        .route(
            "/rest/v1/rollouts/{id}",
            get(rollouts::get_one).delete(rollouts::delete),
        )
        .route("/rest/v1/rollouts/{id}/start", post(rollouts::start))
        .route("/rest/v1/rollouts/{id}/pause", post(rollouts::pause))
        .route("/rest/v1/rollouts/{id}/resume", post(rollouts::resume))
        .route("/rest/v1/rollouts/{id}/deploygroups", get(rollouts::groups))
        .route(
            "/rest/v1/rollouts/{id}/deploygroups/{gid}",
            get(rollouts::group_one),
        )
        .route(
            "/rest/v1/rollouts/{id}/deploygroups/{gid}/targets",
            get(rollouts::group_targets),
        )
        .route(
            "/rest/v1/targetfilters",
            post(target_filters::create).get(target_filters::list),
        )
        .route(
            "/rest/v1/targetfilters/{id}",
            get(target_filters::get_one)
                .put(target_filters::update)
                .delete(target_filters::delete),
        )
        .route(
            "/rest/v1/targetfilters/{id}/autoAssignDS",
            get(target_filters::get_auto_assign)
                .post(target_filters::set_auto_assign)
                .delete(target_filters::delete_auto_assign),
        )
        .route("/rest/v1/system/configs", get(system::get_configs))
        .route(
            "/rest/v1/system/configs/{key}",
            get(system::get_config)
                .put(system::config_read_only)
                .delete(system::config_read_only),
        )
        .route("/rest/v1/system/statistics", get(system::statistics))
        .route_layer(middleware::from_fn_with_state(
            state,
            crate::auth::mgmt::mgmt_auth,
        ))
}
