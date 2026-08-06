//! Thin HTTP client: Basic auth against raptor's Management API, decoding
//! hawkBit's `ErrorBody` envelope on any non-2xx response. `raptor/src/auth/
//! mgmt.rs::check_auth` accepts `Authorization: Basic` on every mgmt route,
//! so this needs no session/cookie handling.

use crate::config::Resolved;
use anyhow::{Result, anyhow};
use raptor_api_types::ErrorBody;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub struct Client {
    http: reqwest::Client,
    base: String,
    user: String,
    pass: String,
}

impl Client {
    pub fn new(cfg: &Resolved) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: cfg.url.clone(),
            user: cfg.user.clone(),
            pass: cfg.pass.clone(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .get(self.url(path))
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;
        Self::body(resp).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let resp = self
            .http
            .post(self.url(path))
            .basic_auth(&self.user, Some(&self.pass))
            .json(body)
            .send()
            .await?;
        Self::body(resp).await
    }

    pub async fn post_empty<B: Serialize>(&self, path: &str, body: &B) -> Result<()> {
        let resp = self
            .http
            .post(self.url(path))
            .basic_auth(&self.user, Some(&self.pass))
            .json(body)
            .send()
            .await?;
        Self::empty(resp).await
    }

    pub async fn put<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let resp = self
            .http
            .put(self.url(path))
            .basic_auth(&self.user, Some(&self.pass))
            .json(body)
            .send()
            .await?;
        Self::body(resp).await
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let resp = self
            .http
            .delete(self.url(path))
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;
        Self::empty(resp).await
    }

    pub async fn upload<T: DeserializeOwned>(
        &self,
        path: &str,
        filename: String,
        bytes: Vec<u8>,
    ) -> Result<T> {
        let part = reqwest::multipart::Part::bytes(bytes).file_name(filename);
        let form = reqwest::multipart::Form::new().part("file", part);
        let resp = self
            .http
            .post(self.url(path))
            .basic_auth(&self.user, Some(&self.pass))
            .multipart(form)
            .send()
            .await?;
        Self::body(resp).await
    }

    async fn body<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(Self::error(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| anyhow!("decoding response: {e}\nbody: {text}"))
    }

    async fn empty(resp: reqwest::Response) -> Result<()> {
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        let text = resp.text().await?;
        Err(Self::error(status, &text))
    }

    fn error(status: reqwest::StatusCode, text: &str) -> anyhow::Error {
        let message = serde_json::from_str::<ErrorBody>(text)
            .map(|e| e.message)
            .unwrap_or_else(|_| text.to_string());
        if status.as_u16() == 401 {
            anyhow!("unauthorized — run 'raptorctl login'")
        } else {
            anyhow!("{message} (HTTP {})", status.as_u16())
        }
    }
}
