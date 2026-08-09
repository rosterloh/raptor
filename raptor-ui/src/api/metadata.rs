//! Key/value metadata on targets, software modules and distribution sets: the
//! same shape behind three prefixes, so the callers pass the prefix and these
//! stay one implementation each.

use super::{ApiResult, delete, get_json, list_path, post_json, put_json};
use raptor_api_types::{MetadataCreate, MetadataRest, MetadataUpdate, PagedList};

pub async fn list_metadata(prefix: &str) -> ApiResult<Vec<MetadataRest>> {
    get_json::<PagedList<MetadataRest>>(&list_path(prefix, 0, 100, None))
        .await
        .map(|p| p.content)
}

/// The create endpoint takes an array; a single entry in means a single
/// entry out.
pub async fn create_metadata(prefix: &str, c: &MetadataCreate) -> ApiResult<MetadataRest> {
    post_json::<_, Vec<MetadataRest>>(prefix, std::slice::from_ref(c))
        .await
        .and_then(|mut v| {
            v.pop()
                .ok_or_else(|| super::ApiError::Network("empty response".into()))
        })
}

pub async fn update_metadata(
    prefix: &str,
    key: &str,
    u: &MetadataUpdate,
) -> ApiResult<MetadataRest> {
    put_json(&format!("{prefix}/{key}"), u).await
}

pub async fn delete_metadata(prefix: &str, key: &str) -> ApiResult<()> {
    delete(&format!("{prefix}/{key}")).await
}
