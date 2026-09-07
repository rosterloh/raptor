//! Rollout endpoints: listing, detail, groups, and lifecycle control.

use raptor_api_types::*;

use super::{ApiResult, delete, get_json, list_path, post_empty, post_nothing};

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

/// hawkBit takes the decision note as a query parameter, not a body, and both
/// endpoints answer 204 — so unlike the lifecycle calls below these give back
/// nothing and the caller has to re-fetch the rollout. An empty remark is left
/// off entirely rather than sent as `?remark=`.
fn decision_path(id: i64, op: &str, remark: &str) -> String {
    let remark = remark.trim();
    if remark.is_empty() {
        return format!("/rest/v1/rollouts/{id}/{op}");
    }
    format!(
        "/rest/v1/rollouts/{id}/{op}?remark={}",
        crate::logic::urlencode(remark)
    )
}

pub async fn approve_rollout(id: i64, remark: &str) -> ApiResult<()> {
    post_nothing(&decision_path(id, "approve", remark)).await
}

pub async fn deny_rollout(id: i64, remark: &str) -> ApiResult<()> {
    post_nothing(&decision_path(id, "deny", remark)).await
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

pub async fn stop_rollout(id: i64) -> ApiResult<RolloutRest> {
    post_empty(&format!("/rest/v1/rollouts/{id}/stop")).await
}

pub async fn delete_rollout(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/rollouts/{id}")).await
}

#[cfg(test)]
mod tests {
    use super::decision_path;

    #[test]
    fn remark_is_optional_and_encoded() {
        assert_eq!(decision_path(7, "deny", "  "), "/rest/v1/rollouts/7/deny");
        assert_eq!(
            decision_path(7, "approve", "ok by ops"),
            "/rest/v1/rollouts/7/approve?remark=ok%20by%20ops"
        );
    }
}
