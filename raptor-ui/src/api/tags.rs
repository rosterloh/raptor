//! Target and DS tags: the same shape behind different prefixes, so the
//! callers pass the prefix and these stay one implementation each.

use super::{ApiResult, delete, get_json, list_path, post_json, post_nothing, put_json};
use raptor_api_types::{PagedList, TagCreate, TagRest, TagUpdate};

pub async fn list_tags(
    prefix: &str,
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<TagRest>> {
    get_json(&list_path(prefix, offset, limit, q)).await
}

pub async fn create_tag(prefix: &str, t: &TagCreate) -> ApiResult<Vec<TagRest>> {
    post_json(prefix, std::slice::from_ref(t)).await
}

pub async fn update_tag(prefix: &str, id: i64, u: &TagUpdate) -> ApiResult<TagRest> {
    put_json(&format!("{prefix}/{id}"), u).await
}

pub async fn delete_tag(prefix: &str, id: i64) -> ApiResult<()> {
    delete(&format!("{prefix}/{id}")).await
}

/// How many entities carry a tag — a one-row page read for its `total`.
pub async fn count_tagged(prefix: &str, id: i64) -> ApiResult<u64> {
    get_json::<PagedList<serde_json::Value>>(&list_path(
        &format!("{prefix}/{id}/assigned"),
        0,
        1,
        None,
    ))
    .await
    .map(|p| p.total)
}

/// The tags carried by one target (raptor extension: `/targets/{cid}/tags`).
pub async fn target_tags(cid: &str) -> ApiResult<Vec<TagRest>> {
    get_json::<PagedList<TagRest>>(&format!("/rest/v1/targets/{cid}/tags?sort=name:ASC"))
        .await
        .map(|p| p.content)
}

pub async fn ds_tags(id: i64) -> ApiResult<Vec<TagRest>> {
    get_json::<PagedList<TagRest>>(&format!(
        "/rest/v1/distributionsets/{id}/tags?sort=name:ASC"
    ))
    .await
    .map(|p| p.content)
}

/// Assign/unassign by entity key — `cid` for targets, the id for sets. The
/// assign response body is the affected entity, which callers re-read anyway.
pub async fn assign_tag(prefix: &str, tag_id: i64, key: &str) -> ApiResult<()> {
    post_nothing(&format!("{prefix}/{tag_id}/assigned/{key}")).await
}

pub async fn unassign_tag(prefix: &str, tag_id: i64, key: &str) -> ApiResult<()> {
    delete(&format!("{prefix}/{tag_id}/assigned/{key}")).await
}
