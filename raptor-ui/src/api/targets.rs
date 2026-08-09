//! Target (device) endpoints: listing, detail, attributes, actions, and assignment.

use raptor_api_types::*;
use std::collections::BTreeMap;

use super::{ApiResult, delete, get_json, get_opt, list_path, post_json, post_nothing};

pub async fn list_targets(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<TargetRest>> {
    get_json(&list_path("/rest/v1/targets", offset, limit, q)).await
}

pub async fn get_target(cid: &str) -> ApiResult<TargetRest> {
    get_json(&format!("/rest/v1/targets/{cid}")).await
}

pub async fn target_attributes(cid: &str) -> ApiResult<BTreeMap<String, String>> {
    get_json(&format!("/rest/v1/targets/{cid}/attributes")).await
}

pub async fn target_actions(
    cid: &str,
    offset: u64,
    limit: u64,
) -> ApiResult<PagedList<ActionRest>> {
    get_json(&list_path(
        &format!("/rest/v1/targets/{cid}/actions"),
        offset,
        limit,
        None,
    ))
    .await
}

pub async fn assigned_ds(cid: &str) -> ApiResult<Option<DsRest>> {
    get_opt(&format!("/rest/v1/targets/{cid}/assignedDS")).await
}

pub async fn installed_ds(cid: &str) -> ApiResult<Option<DsRest>> {
    get_opt(&format!("/rest/v1/targets/{cid}/installedDS")).await
}

pub async fn assign_ds(cid: &str, ds_id: i64, forced: bool) -> ApiResult<AssignResult> {
    post_json(
        &format!("/rest/v1/targets/{cid}/assignedDS"),
        &DsAssignment {
            id: ds_id,
            assign_type: Some(if forced { "forced" } else { "soft" }.into()),
            // the console only offers forced/soft; timeforced/downloadonly are
            // Management-API only for now
            forcetime: None,
        },
    )
    .await
}

/// One action's status history: assignment → download → downloaded → feedback →
/// finished, with whatever messages the device attached. Fetched per action on
/// demand rather than for every row, so opening the history tab costs one
/// request and expanding an action costs one more.
pub async fn action_status_history(
    cid: &str,
    aid: i64,
    offset: u64,
    limit: u64,
) -> ApiResult<PagedList<ActionStatusRest>> {
    get_json(&list_path(
        &format!("/rest/v1/targets/{cid}/actions/{aid}/status"),
        offset,
        limit,
        None,
    ))
    .await
}

pub async fn cancel_action(cid: &str, aid: i64, force: bool) -> ApiResult<()> {
    let suffix = if force { "?force=true" } else { "" };
    delete(&format!("/rest/v1/targets/{cid}/actions/{aid}{suffix}")).await
}

pub async fn delete_target(cid: &str) -> ApiResult<()> {
    delete(&format!("/rest/v1/targets/{cid}")).await
}

pub async fn auto_confirm_status(cid: &str) -> ApiResult<AutoConfirmState> {
    get_json(&format!("/rest/v1/targets/{cid}/autoConfirm")).await
}

pub async fn activate_auto_confirm(cid: &str) -> ApiResult<()> {
    post_nothing(&format!("/rest/v1/targets/{cid}/autoConfirm/activate")).await
}

pub async fn deactivate_auto_confirm(cid: &str) -> ApiResult<()> {
    post_nothing(&format!("/rest/v1/targets/{cid}/autoConfirm/deactivate")).await
}
