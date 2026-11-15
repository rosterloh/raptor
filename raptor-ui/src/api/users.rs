//! Operator users (admin only) and the caller's own `/rest/v1/userinfo`.

use raptor_api_types::*;

use super::{ApiResult, delete, get_json, list_path, post_json, put_json};

pub async fn userinfo() -> ApiResult<UserInfo> {
    get_json("/rest/v1/userinfo").await
}

pub async fn list_users(offset: u64, limit: u64) -> ApiResult<PagedList<UserRest>> {
    get_json(&list_path("/rest/v1/users", offset, limit, None)).await
}

pub async fn create_user(u: &UserCreate) -> ApiResult<UserRest> {
    post_json("/rest/v1/users", u).await
}

pub async fn update_user(id: i64, u: &UserUpdate) -> ApiResult<UserRest> {
    put_json(&format!("/rest/v1/users/{id}"), u).await
}

pub async fn delete_user(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/users/{id}")).await
}
