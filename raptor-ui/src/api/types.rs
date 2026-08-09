//! Software-module-type, distribution-set-type, and target-type CRUD, plus
//! the DS-type module-composition and target-type DS-compatibility
//! sub-resources, and the target <-> target-type assignment endpoint.

use raptor_api_types::*;

use super::{ApiResult, delete, get_json, list_path, post_json, post_no_content, put_json};

// ---- software module types ----

pub async fn list_sm_types(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<SmTypeRest>> {
    get_json(&list_path("/rest/v1/softwaremoduletypes", offset, limit, q)).await
}

pub async fn create_sm_type(t: &SmTypeCreate) -> ApiResult<Vec<SmTypeRest>> {
    post_json("/rest/v1/softwaremoduletypes", std::slice::from_ref(t)).await
}

pub async fn update_sm_type(id: i64, u: &SmTypeUpdate) -> ApiResult<SmTypeRest> {
    put_json(&format!("/rest/v1/softwaremoduletypes/{id}"), u).await
}

pub async fn delete_sm_type(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/softwaremoduletypes/{id}")).await
}

// ---- distribution set types ----

pub async fn list_ds_types(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<DsTypeRest>> {
    get_json(&list_path(
        "/rest/v1/distributionsettypes",
        offset,
        limit,
        q,
    ))
    .await
}

pub async fn create_ds_type(t: &DsTypeCreate) -> ApiResult<Vec<DsTypeRest>> {
    post_json("/rest/v1/distributionsettypes", std::slice::from_ref(t)).await
}

pub async fn update_ds_type(id: i64, u: &DsTypeUpdate) -> ApiResult<DsTypeRest> {
    put_json(&format!("/rest/v1/distributionsettypes/{id}"), u).await
}

pub async fn delete_ds_type(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/distributionsettypes/{id}")).await
}

pub async fn ds_type_mandatory(id: i64) -> ApiResult<Vec<SmTypeRest>> {
    get_json::<PagedList<SmTypeRest>>(&format!(
        "/rest/v1/distributionsettypes/{id}/mandatorymoduletypes"
    ))
    .await
    .map(|p| p.content)
}

pub async fn ds_type_optional(id: i64) -> ApiResult<Vec<SmTypeRest>> {
    get_json::<PagedList<SmTypeRest>>(&format!(
        "/rest/v1/distributionsettypes/{id}/optionalmoduletypes"
    ))
    .await
    .map(|p| p.content)
}

pub async fn ds_type_add_mandatory(id: i64, module_type_id: i64) -> ApiResult<()> {
    post_no_content(
        &format!("/rest/v1/distributionsettypes/{id}/mandatorymoduletypes"),
        &TypeRef { id: module_type_id },
    )
    .await
}

pub async fn ds_type_add_optional(id: i64, module_type_id: i64) -> ApiResult<()> {
    post_no_content(
        &format!("/rest/v1/distributionsettypes/{id}/optionalmoduletypes"),
        &TypeRef { id: module_type_id },
    )
    .await
}

pub async fn ds_type_remove_mandatory(id: i64, module_type_id: i64) -> ApiResult<()> {
    delete(&format!(
        "/rest/v1/distributionsettypes/{id}/mandatorymoduletypes/{module_type_id}"
    ))
    .await
}

pub async fn ds_type_remove_optional(id: i64, module_type_id: i64) -> ApiResult<()> {
    delete(&format!(
        "/rest/v1/distributionsettypes/{id}/optionalmoduletypes/{module_type_id}"
    ))
    .await
}

// ---- target types ----

pub async fn list_target_types(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<TargetTypeRest>> {
    get_json(&list_path("/rest/v1/targettypes", offset, limit, q)).await
}

pub async fn create_target_type(t: &TargetTypeCreate) -> ApiResult<Vec<TargetTypeRest>> {
    post_json("/rest/v1/targettypes", std::slice::from_ref(t)).await
}

pub async fn update_target_type(id: i64, u: &TargetTypeUpdate) -> ApiResult<TargetTypeRest> {
    put_json(&format!("/rest/v1/targettypes/{id}"), u).await
}

pub async fn delete_target_type(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/targettypes/{id}")).await
}

pub async fn target_type_compatible(id: i64) -> ApiResult<Vec<DsTypeRest>> {
    get_json::<PagedList<DsTypeRest>>(&format!(
        "/rest/v1/targettypes/{id}/compatibledistributionsettypes"
    ))
    .await
    .map(|p| p.content)
}

pub async fn target_type_add_compatible(id: i64, ds_type_id: i64) -> ApiResult<()> {
    post_no_content(
        &format!("/rest/v1/targettypes/{id}/compatibledistributionsettypes"),
        &TypeRef { id: ds_type_id },
    )
    .await
}

pub async fn target_type_remove_compatible(id: i64, ds_type_id: i64) -> ApiResult<()> {
    delete(&format!(
        "/rest/v1/targettypes/{id}/compatibledistributionsettypes/{ds_type_id}"
    ))
    .await
}

// ---- target <-> target type assignment ----

pub async fn assign_target_type(cid: &str, target_type_id: i64) -> ApiResult<()> {
    post_no_content(
        &format!("/rest/v1/targets/{cid}/targettype"),
        &TypeRef { id: target_type_id },
    )
    .await
}

pub async fn unassign_target_type(cid: &str) -> ApiResult<()> {
    delete(&format!("/rest/v1/targets/{cid}/targettype")).await
}
