//! Cross-target action listing.

use raptor_api_types::*;

use super::{ApiResult, get_json, list_path};

pub async fn all_actions(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<ActionRest>> {
    get_json(&list_path("/rest/v1/actions", offset, limit, q)).await
}
