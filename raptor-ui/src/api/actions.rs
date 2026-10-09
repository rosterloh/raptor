//! Cross-target action listing.

use raptor_api_types::*;

use super::{ApiResult, get_json, list_path, with_sort};

pub async fn all_actions(
    offset: u64,
    limit: u64,
    q: Option<&str>,
    sort: Option<&str>,
) -> ApiResult<PagedList<ActionRest>> {
    get_json(&with_sort(
        list_path("/rest/v1/actions", offset, limit, q),
        sort,
    ))
    .await
}
