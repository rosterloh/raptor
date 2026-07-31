//! Type-catalogue endpoints, split by resource family: software-module types
//! (`/rest/v1/softwaremoduletypes`), distribution-set types
//! (`/rest/v1/distributionsettypes`), and target types
//! (`/rest/v1/targettypes`). Handlers are re-exported here so callers keep
//! using `types::sm_types` etc. regardless of the split.

mod distribution_set_types;
mod software_module_types;
mod target_types;

pub use distribution_set_types::{
    ds_type, ds_type_add_mandatory, ds_type_add_optional, ds_type_create, ds_type_delete,
    ds_type_mandatory, ds_type_optional, ds_type_remove_module, ds_type_update, ds_types,
};
pub use software_module_types::{
    sm_type, sm_type_create, sm_type_delete, sm_type_update, sm_types,
};
pub use target_types::{
    tt_add_compat, tt_compat_list, tt_create, tt_delete, tt_list, tt_one, tt_remove_compat,
    tt_update,
};

use crate::state::AppState;
use axum::Router;
use axum::routing::get;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/rest/v1/softwaremoduletypes",
            get(sm_types).post(sm_type_create),
        )
        .route(
            "/rest/v1/softwaremoduletypes/{id}",
            get(sm_type).put(sm_type_update).delete(sm_type_delete),
        )
        .route(
            "/rest/v1/distributionsettypes",
            get(ds_types).post(ds_type_create),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}",
            get(ds_type).put(ds_type_update).delete(ds_type_delete),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}/mandatorymoduletypes",
            get(ds_type_mandatory).post(ds_type_add_mandatory),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}/mandatorymoduletypes/{mid}",
            axum::routing::delete(ds_type_remove_module),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}/optionalmoduletypes",
            get(ds_type_optional).post(ds_type_add_optional),
        )
        .route(
            "/rest/v1/distributionsettypes/{id}/optionalmoduletypes/{mid}",
            axum::routing::delete(ds_type_remove_module),
        )
        .route("/rest/v1/targettypes", get(tt_list).post(tt_create))
        .route(
            "/rest/v1/targettypes/{id}",
            get(tt_one).put(tt_update).delete(tt_delete),
        )
        .route(
            "/rest/v1/targettypes/{id}/compatibledistributionsettypes",
            get(tt_compat_list).post(tt_add_compat),
        )
        .route(
            "/rest/v1/targettypes/{id}/compatibledistributionsettypes/{dsid}",
            axum::routing::delete(tt_remove_compat),
        )
}
