//! Target filter endpoints: listing, CRUD, and auto-assign-distribution-set control.

use raptor_api_types::*;

use super::targets::list_targets;
use super::{ApiResult, delete, get_json, list_path, post_json, put_json};

pub async fn list_target_filters(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<TargetFilterRest>> {
    get_json(&list_path("/rest/v1/targetfilters", offset, limit, q)).await
}

pub async fn create_target_filter(f: &TargetFilterCreate) -> ApiResult<TargetFilterRest> {
    post_json("/rest/v1/targetfilters", f).await
}

pub async fn update_target_filter(id: i64, u: &TargetFilterUpdate) -> ApiResult<TargetFilterRest> {
    put_json(&format!("/rest/v1/targetfilters/{id}"), u).await
}

pub async fn delete_target_filter(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/targetfilters/{id}")).await
}

pub async fn set_auto_assign_ds(id: i64, ds_id: i64, forced: bool) -> ApiResult<TargetFilterRest> {
    post_json(
        &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
        &AutoAssignRequest {
            id: ds_id,
            action_type: Some(if forced { "forced" } else { "soft" }.into()),
        },
    )
    .await
}

/// Detaches the auto-assign set. The API answers with the updated filter, which
/// the caller re-reads from the list instead, so the body is discarded.
pub async fn clear_auto_assign_ds(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/targetfilters/{id}/autoAssignDS")).await
}

/// How many targets a FIQL query currently matches — a one-row page read for
/// its `total`, used by the filter form's live preview.
pub async fn count_targets(q: &str) -> ApiResult<u64> {
    list_targets(0, 1, Some(q)).await.map(|p| p.total)
}
