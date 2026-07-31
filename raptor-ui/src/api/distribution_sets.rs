//! Distribution set endpoints: listing, detail, creation, and module assignment.

use raptor_api_types::*;

use super::{ApiResult, delete, get_json, list_path, post_json, post_no_content};

pub async fn list_ds(offset: u64, limit: u64, q: Option<&str>) -> ApiResult<PagedList<DsRest>> {
    get_json(&list_path("/rest/v1/distributionsets", offset, limit, q)).await
}

pub async fn get_ds(id: i64) -> ApiResult<DsRest> {
    get_json(&format!("/rest/v1/distributionsets/{id}")).await
}

pub async fn create_ds(ds: &DsCreate) -> ApiResult<Vec<DsRest>> {
    post_json("/rest/v1/distributionsets", std::slice::from_ref(ds)).await
}

pub async fn delete_ds(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/distributionsets/{id}")).await
}

pub async fn ds_assign_modules(id: i64, module_ids: &[i64]) -> ApiResult<()> {
    let body: Vec<ModuleRef> = module_ids.iter().map(|&id| ModuleRef { id }).collect();
    post_no_content(&format!("/rest/v1/distributionsets/{id}/assignedSM"), &body).await
}
