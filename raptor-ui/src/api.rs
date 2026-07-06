//! All HTTP access. Same-origin fetch sends the session cookie by default;
//! any non-login 401 hard-redirects to /ui/login.
//!
//! This module's public surface is consumed by pages/components added in
//! later tasks (11-15); until then, in this bin crate, `-D warnings` would
//! flag every item here as dead code.
#![allow(dead_code)]

use raptor_api_types::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    Unauthorized,
    Server { status: u16, message: String },
    Network(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unauthorized => write!(f, "not logged in"),
            ApiError::Server { status, message } => write!(f, "{message} (HTTP {status})"),
            ApiError::Network(e) => write!(f, "network error: {e}"),
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

fn base() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().unwrap().location().origin().unwrap()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "http://localhost:8080".to_string()
    }
}

fn redirect_to_login() {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = web_sys::window().unwrap().location().set_href("/ui/login");
    }
}

fn net(e: reqwest::Error) -> ApiError {
    ApiError::Network(e.to_string())
}

async fn check(resp: reqwest::Response) -> ApiResult<reqwest::Response> {
    let status = resp.status().as_u16();
    if status == 401 {
        redirect_to_login();
        return Err(ApiError::Unauthorized);
    }
    if status >= 400 {
        let message = resp
            .json::<ErrorBody>()
            .await
            .map(|e| e.message)
            .unwrap_or_else(|_| format!("HTTP {status}"));
        return Err(ApiError::Server { status, message });
    }
    Ok(resp)
}

async fn get_json<T: DeserializeOwned>(path: &str) -> ApiResult<T> {
    let resp = reqwest::get(format!("{}{path}", base()))
        .await
        .map_err(net)?;
    check(resp).await?.json().await.map_err(net)
}

async fn get_opt<T: DeserializeOwned>(path: &str) -> ApiResult<Option<T>> {
    let resp = reqwest::get(format!("{}{path}", base()))
        .await
        .map_err(net)?;
    if resp.status().as_u16() == 204 {
        return Ok(None);
    }
    check(resp).await?.json().await.map(Some).map_err(net)
}

async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> ApiResult<T> {
    let resp = reqwest::Client::new()
        .post(format!("{}{path}", base()))
        .json(body)
        .send()
        .await
        .map_err(net)?;
    check(resp).await?.json().await.map_err(net)
}

async fn post_no_content<B: Serialize + ?Sized>(path: &str, body: &B) -> ApiResult<()> {
    let resp = reqwest::Client::new()
        .post(format!("{}{path}", base()))
        .json(body)
        .send()
        .await
        .map_err(net)?;
    check(resp).await?;
    Ok(())
}

async fn delete(path: &str) -> ApiResult<()> {
    let resp = reqwest::Client::new()
        .delete(format!("{}{path}", base()))
        .send()
        .await
        .map_err(net)?;
    check(resp).await?;
    Ok(())
}

fn list_path(prefix: &str, offset: u64, limit: u64, q: Option<&str>) -> String {
    let mut p = format!("{prefix}?offset={offset}&limit={limit}");
    if let Some(q) = q {
        p.push_str("&q=");
        p.push_str(&crate::logic::urlencode(q));
    }
    p
}

// ---- auth ----

pub async fn login(username: &str, password: &str) -> ApiResult<()> {
    // Deliberately not via check(): a failed login must show inline, not redirect.
    let resp = reqwest::Client::new()
        .post(format!("{}/rest/v1/login", base()))
        .json(&LoginRequest {
            username: username.into(),
            password: password.into(),
        })
        .send()
        .await
        .map_err(net)?;
    match resp.status().as_u16() {
        204 => Ok(()),
        401 => Err(ApiError::Server {
            status: 401,
            message: "invalid username or password".into(),
        }),
        s => Err(ApiError::Server {
            status: s,
            message: format!("HTTP {s}"),
        }),
    }
}

pub async fn logout() -> ApiResult<()> {
    let resp = reqwest::Client::new()
        .post(format!("{}/rest/v1/logout", base()))
        .send()
        .await
        .map_err(net)?;
    check(resp).await?;
    Ok(())
}

// ---- targets ----

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
        },
    )
    .await
}

pub async fn cancel_action(cid: &str, aid: i64, force: bool) -> ApiResult<()> {
    let suffix = if force { "?force=true" } else { "" };
    delete(&format!("/rest/v1/targets/{cid}/actions/{aid}{suffix}")).await
}

pub async fn delete_target(cid: &str) -> ApiResult<()> {
    delete(&format!("/rest/v1/targets/{cid}")).await
}

// ---- distribution sets ----

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

// ---- software modules & artifacts ----

pub async fn list_modules(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<SmRest>> {
    get_json(&list_path("/rest/v1/softwaremodules", offset, limit, q)).await
}

pub async fn get_module(id: i64) -> ApiResult<SmRest> {
    get_json(&format!("/rest/v1/softwaremodules/{id}")).await
}

pub async fn create_module(m: &SmCreate) -> ApiResult<Vec<SmRest>> {
    post_json("/rest/v1/softwaremodules", std::slice::from_ref(m)).await
}

pub async fn delete_module(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/softwaremodules/{id}")).await
}

pub async fn module_artifacts(id: i64) -> ApiResult<Vec<ArtifactRest>> {
    get_json(&format!("/rest/v1/softwaremodules/{id}/artifacts")).await
}

pub async fn delete_artifact(module_id: i64, artifact_id: i64) -> ApiResult<()> {
    delete(&format!(
        "/rest/v1/softwaremodules/{module_id}/artifacts/{artifact_id}"
    ))
    .await
}

pub fn artifact_download_href(module_id: i64, artifact_id: i64) -> String {
    format!("/rest/v1/softwaremodules/{module_id}/artifacts/{artifact_id}/download")
}

// ---- actions ----

pub async fn all_actions(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<ActionRest>> {
    get_json(&list_path("/rest/v1/actions", offset, limit, q)).await
}
