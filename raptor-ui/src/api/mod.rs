//! All HTTP access. Same-origin fetch sends the session cookie by default;
//! any non-login 401 hard-redirects to /ui/login.

use raptor_api_types::*;
use serde::de::DeserializeOwned;
use serde::Serialize;

mod actions;
mod distribution_sets;
mod modules;
mod rollouts;
mod system;
mod tags;
mod target_filters;
mod targets;

pub use actions::*;
pub use distribution_sets::*;
pub use modules::*;
pub use rollouts::*;
pub use system::*;
pub use tags::*;
pub use target_filters::*;
pub use targets::*;

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    Unauthorized,
    Server { status: u16, message: String },
    Network(String),
}

impl ApiError {
    /// The server's message on its own, without the `(HTTP nnn)` suffix
    /// `Display` adds — for showing an error inline against a form field.
    pub fn message(&self) -> String {
        match self {
            ApiError::Server { message, .. } => message.clone(),
            other => other.to_string(),
        }
    }
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
        "http://localhost:8088".to_string()
    }
}

fn redirect_to_login() {
    dioxus::prelude::navigator().replace(crate::Route::Login {});
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

/// Marks requests as SPA-originated so the server skips `WWW-Authenticate`
/// on 401s — otherwise browsers pop their native Basic-Auth dialog.
const AJAX_HEADER: (&str, &str) = ("X-Requested-With", "XMLHttpRequest");

async fn get_json<T: DeserializeOwned>(path: &str) -> ApiResult<T> {
    let resp = reqwest::Client::new()
        .get(format!("{}{path}", base()))
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
        .send()
        .await
        .map_err(net)?;
    check(resp).await?.json().await.map_err(net)
}

async fn get_opt<T: DeserializeOwned>(path: &str) -> ApiResult<Option<T>> {
    let resp = reqwest::Client::new()
        .get(format!("{}{path}", base()))
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
        .send()
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
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
        .json(body)
        .send()
        .await
        .map_err(net)?;
    check(resp).await?.json().await.map_err(net)
}

async fn put_json<B: Serialize + ?Sized, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> ApiResult<T> {
    let resp = reqwest::Client::new()
        .put(format!("{}{path}", base()))
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
        .json(body)
        .send()
        .await
        .map_err(net)?;
    check(resp).await?.json().await.map_err(net)
}

/// POST with no request body, decoding a JSON response (start/pause/resume).
async fn post_empty<T: DeserializeOwned>(path: &str) -> ApiResult<T> {
    let resp = reqwest::Client::new()
        .post(format!("{}{path}", base()))
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
        .send()
        .await
        .map_err(net)?;
    check(resp).await?.json().await.map_err(net)
}

/// POST with neither request nor response body of interest (tag assignment).
async fn post_nothing(path: &str) -> ApiResult<()> {
    let resp = reqwest::Client::new()
        .post(format!("{}{path}", base()))
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
        .send()
        .await
        .map_err(net)?;
    check(resp).await?;
    Ok(())
}

async fn post_no_content<B: Serialize + ?Sized>(path: &str, body: &B) -> ApiResult<()> {
    let resp = reqwest::Client::new()
        .post(format!("{}{path}", base()))
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
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
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
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
        .header(AJAX_HEADER.0, AJAX_HEADER.1)
        .send()
        .await
        .map_err(net)?;
    check(resp).await?;
    Ok(())
}
