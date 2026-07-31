//! Rollout endpoints: listing, detail, groups, and lifecycle control.

use raptor_api_types::*;

use super::{ApiResult, delete, get_json, list_path, post_empty};

pub async fn list_rollouts(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<RolloutRest>> {
    get_json(&list_path("/rest/v1/rollouts", offset, limit, q)).await
}

pub async fn get_rollout(id: i64) -> ApiResult<RolloutRest> {
    get_json(&format!("/rest/v1/rollouts/{id}")).await
}

pub async fn rollout_groups(
    id: i64,
    offset: u64,
    limit: u64,
) -> ApiResult<PagedList<RolloutGroupRest>> {
    get_json(&list_path(
        &format!("/rest/v1/rollouts/{id}/deploygroups"),
        offset,
        limit,
        None,
    ))
    .await
}

pub async fn start_rollout(id: i64) -> ApiResult<RolloutRest> {
    post_empty(&format!("/rest/v1/rollouts/{id}/start")).await
}

pub async fn pause_rollout(id: i64) -> ApiResult<RolloutRest> {
    post_empty(&format!("/rest/v1/rollouts/{id}/pause")).await
}

pub async fn resume_rollout(id: i64) -> ApiResult<RolloutRest> {
    post_empty(&format!("/rest/v1/rollouts/{id}/resume")).await
}

pub async fn delete_rollout(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/rollouts/{id}")).await
}
