//! OpenID Connect (`[mgmt.oidc]`): provider discovery and keys, loaded in the
//! background so an IdP outage never stops raptor; JWT validation for bearer
//! tokens and ID tokens; role mapping; just-in-time `users` rows; and the
//! pending-login store behind the console's authorization-code flow.

use crate::auth::principal::{Principal, Role, Source};
use crate::config::{Config, OidcConfig};
use crate::entity::user as users;
use crate::error::AppError;
use crate::state::AppState;
use base64::Engine;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const LEEWAY_SECS: u64 = 60;
const PENDING_TTL: Duration = Duration::from_secs(10 * 60);
const KID_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const MAX_BACKOFF: Duration = Duration::from_secs(5 * 60);
/// Asymmetric only: an `HS*` token would be verified with key material the
/// provider publishes, which is the classic algorithm-confusion hole.
const ALGORITHMS: &[Algorithm] = &[
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
    Algorithm::EdDSA,
];

pub struct Oidc {
    pub cfg: OidcConfig,
    redirect_uri: String,
    http: reqwest::Client,
    provider: RwLock<Option<Arc<Provider>>>,
    last_kid_refresh: Mutex<Option<Instant>>,
    pending: Mutex<HashMap<String, Pending>>,
}

struct Provider {
    /// As the provider spells it; tokens carry exactly this `iss`, which may
    /// differ from the config by a trailing slash (Authentik ends with one).
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    keys: JwkSet,
}

pub struct Pending {
    pub nonce: String,
    pub verifier: String,
    created: Instant,
}

/// Why a token was refused: `NoRole` is a 403, everything else a 401.
#[derive(Debug)]
pub enum Refused {
    Unavailable,
    Invalid(String),
    NoRole,
}

impl From<Refused> for AppError {
    fn from(r: Refused) -> Self {
        match r {
            Refused::NoRole => AppError::Forbidden("no raptor role in token".into()),
            Refused::Unavailable | Refused::Invalid(_) => AppError::Unauthorized,
        }
    }
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}

/// Startup checks for `[mgmt.oidc]`; network problems are not checked here.
pub fn validate(cfg: &Config) -> Result<(), String> {
    let Some(o) = &cfg.mgmt.oidc else {
        return Ok(());
    };
    if cfg.url.as_deref().is_none_or(|u| u.trim().is_empty()) {
        return Err("[mgmt.oidc] needs `url` set: the redirect URI is derived from it".into());
    }
    url::Url::parse(&o.issuer).map_err(|e| format!("[mgmt.oidc] issuer: {e}"))?;
    if o.client_id.trim().is_empty() {
        return Err("[mgmt.oidc] client_id is empty".into());
    }
    if o.admin_values.is_empty() && o.read_only_values.is_empty() {
        return Err("[mgmt.oidc] needs admin_values or read_only_values".into());
    }
    Ok(())
}

impl Oidc {
    pub fn from_config(cfg: &Config) -> Option<Arc<Self>> {
        let o = cfg.mgmt.oidc.clone()?;
        let base = cfg.url.as_deref().unwrap_or_default().trim_end_matches('/');
        Some(Arc::new(Self {
            cfg: o,
            redirect_uri: format!("{base}/rest/v1/oidc/callback"),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            provider: RwLock::new(None),
            last_kid_refresh: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
        }))
    }

    pub fn is_ready(&self) -> bool {
        self.provider().is_some()
    }

    fn provider(&self) -> Option<Arc<Provider>> {
        self.provider
            .read()
            .expect("lock poisoned by an earlier panic")
            .clone()
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, String> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| format!("{url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("{url}: HTTP {}", resp.status()));
        }
        let body = resp.bytes().await.map_err(|e| format!("{url}: {e}"))?;
        serde_json::from_slice(&body).map_err(|e| format!("{url}: {e}"))
    }

    /// Fetches the discovery document and the keys, replacing what is cached.
    pub async fn load(&self) -> Result<(), String> {
        let issuer = self.cfg.issuer.trim_end_matches('/');
        let d: Discovery = self
            .get_json(&format!("{issuer}/.well-known/openid-configuration"))
            .await?;
        if d.issuer.trim_end_matches('/') != issuer {
            return Err(format!(
                "discovery names issuer {:?}, configured {:?}",
                d.issuer, self.cfg.issuer
            ));
        }
        let keys: JwkSet = self.get_json(&d.jwks_uri).await?;
        if keys.keys.is_empty() {
            tracing::warn!(
                issuer = self.cfg.issuer,
                "oidc provider publishes no signing keys, so every token will be refused; \
                 select a signing key on the provider (Authentik otherwise signs with the \
                 client secret, HS256, which raptor does not accept)"
            );
        }
        *self
            .provider
            .write()
            .expect("lock poisoned by an earlier panic") = Some(Arc::new(Provider {
            issuer: d.issuer,
            authorization_endpoint: d.authorization_endpoint,
            token_endpoint: d.token_endpoint,
            jwks_uri: d.jwks_uri,
            keys,
        }));
        Ok(())
    }

    /// Retries [`Self::load`] with backoff (1 s doubling to 5 min) until it
    /// succeeds. Never gives up: the provider may just be down.
    pub fn spawn_loader(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut wait = Duration::from_secs(1);
            loop {
                match self.load().await {
                    Ok(()) => {
                        tracing::info!(issuer = self.cfg.issuer, "oidc provider loaded");
                        return;
                    }
                    Err(e) => tracing::error!(
                        issuer = self.cfg.issuer,
                        error = e,
                        retry_in_secs = wait.as_secs(),
                        "oidc provider unavailable; SSO and bearer tokens refused until it loads"
                    ),
                }
                tokio::time::sleep(wait).await;
                wait = (wait * 2).min(MAX_BACKOFF);
            }
        });
    }

    /// The key for `kid`, refreshing the key set once (rate-limited) when the
    /// provider has rotated to a key we have not seen.
    async fn key(&self, kid: Option<&str>) -> Result<DecodingKey, Refused> {
        let p = self.provider().ok_or(Refused::Unavailable)?;
        if let Some(k) = find_key(&p.keys, kid) {
            return Ok(k);
        }
        let due = {
            let mut last = self
                .last_kid_refresh
                .lock()
                .expect("lock poisoned by an earlier panic");
            let due = last.is_none_or(|t| t.elapsed() >= KID_REFRESH_INTERVAL);
            if due {
                *last = Some(Instant::now());
            }
            due
        };
        if !due {
            return Err(Refused::Invalid("unknown key id".into()));
        }
        let keys: JwkSet = self.get_json(&p.jwks_uri).await.map_err(|e| {
            tracing::warn!(error = e, "oidc key refresh failed");
            Refused::Invalid("unknown key id".into())
        })?;
        let found = find_key(&keys, kid);
        *self
            .provider
            .write()
            .expect("lock poisoned by an earlier panic") = Some(Arc::new(Provider {
            issuer: p.issuer.clone(),
            authorization_endpoint: p.authorization_endpoint.clone(),
            token_endpoint: p.token_endpoint.clone(),
            jwks_uri: p.jwks_uri.clone(),
            keys,
        }));
        found.ok_or_else(|| Refused::Invalid("unknown key id".into()))
    }

    /// Verifies signature, `iss`, `aud` and `exp`/`nbf` (with leeway), plus
    /// `nonce` when given (ID tokens), and returns the claims.
    pub async fn verify(
        &self,
        token: &str,
        audience: &str,
        nonce: Option<&str>,
    ) -> Result<Map<String, Value>, Refused> {
        let header =
            jsonwebtoken::decode_header(token).map_err(|e| Refused::Invalid(e.to_string()))?;
        if !ALGORITHMS.contains(&header.alg) {
            return Err(Refused::Invalid(format!("algorithm {:?}", header.alg)));
        }
        let key = self.key(header.kid.as_deref()).await?;
        let issuer = self.provider().ok_or(Refused::Unavailable)?.issuer.clone();
        let mut v = Validation::new(header.alg);
        v.leeway = LEEWAY_SECS;
        v.validate_nbf = true;
        v.set_issuer(&[issuer]);
        v.set_audience(&[audience]);
        v.set_required_spec_claims(&["exp", "iss", "aud"]);
        let claims = jsonwebtoken::decode::<Map<String, Value>>(token, &key, &v)
            .map_err(|e| Refused::Invalid(e.to_string()))?
            .claims;
        if let Some(n) = nonce
            && claims.get("nonce").and_then(Value::as_str) != Some(n)
        {
            return Err(Refused::Invalid("nonce mismatch".into()));
        }
        Ok(claims)
    }

    /// Username and role from verified claims.
    pub fn identity(&self, claims: &Map<String, Value>) -> Result<(String, Role), Refused> {
        let username = claims
            .get(&self.cfg.username_claim)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Refused::Invalid(format!("no {} claim", self.cfg.username_claim)))?;
        let values: Vec<&str> = match claims.get(&self.cfg.role_claim) {
            Some(Value::String(s)) => vec![s.as_str()],
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        let has = |want: &[String]| values.iter().any(|v| want.iter().any(|w| w == v));
        let role = if has(&self.cfg.admin_values) {
            Role::Admin
        } else if has(&self.cfg.read_only_values) {
            Role::ReadOnly
        } else {
            return Err(Refused::NoRole);
        };
        Ok((username.to_string(), role))
    }

    /// Starts a console login: remembers state, nonce and PKCE verifier, and
    /// returns the provider's authorization URL.
    pub fn begin_login(&self) -> Option<String> {
        let p = self.provider()?;
        let state = crate::util::random_token();
        let nonce = crate::util::random_token();
        // 64 hex chars: inside PKCE's 43..=128 unreserved-character range.
        let verifier = format!(
            "{}{}",
            crate::util::random_token(),
            crate::util::random_token()
        );
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(verifier.as_bytes()));
        let mut url = url::Url::parse(&p.authorization_endpoint).ok()?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.cfg.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", &self.cfg.scopes.join(" "))
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        let mut m = self
            .pending
            .lock()
            .expect("lock poisoned by an earlier panic");
        m.retain(|_, p| p.created.elapsed() < PENDING_TTL);
        m.insert(
            state,
            Pending {
                nonce,
                verifier,
                created: Instant::now(),
            },
        );
        Some(url.into())
    }

    /// The pending login for `state`, consumed; `None` if unknown or stale.
    pub fn take_pending(&self, state: &str) -> Option<Pending> {
        self.pending
            .lock()
            .expect("lock poisoned by an earlier panic")
            .remove(state)
            .filter(|p| p.created.elapsed() < PENDING_TTL)
    }

    /// Exchanges an authorization code for the ID token.
    pub async fn exchange(&self, code: &str, verifier: &str) -> Result<String, Refused> {
        let p = self.provider().ok_or(Refused::Unavailable)?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", code)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("client_id", &self.cfg.client_id)
            .append_pair("client_secret", &self.cfg.client_secret)
            .append_pair("code_verifier", verifier)
            .finish();
        let resp = self
            .http
            .post(&p.token_endpoint)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| Refused::Invalid(format!("token endpoint: {e}")))?;
        if !resp.status().is_success() {
            return Err(Refused::Invalid(format!(
                "token endpoint: HTTP {}",
                resp.status()
            )));
        }
        #[derive(Deserialize)]
        struct TokenResponse {
            id_token: String,
        }
        let body = resp
            .bytes()
            .await
            .map_err(|e| Refused::Invalid(format!("token endpoint: {e}")))?;
        let t: TokenResponse = serde_json::from_slice(&body)
            .map_err(|e| Refused::Invalid(format!("token endpoint: {e}")))?;
        Ok(t.id_token)
    }
}

fn find_key(keys: &JwkSet, kid: Option<&str>) -> Option<DecodingKey> {
    let jwk = match kid {
        Some(k) => keys.find(k)?,
        // A token without `kid` is only unambiguous against a single key.
        None if keys.keys.len() == 1 => &keys.keys[0],
        None => return None,
    };
    DecodingKey::from_jwk(jwk).ok()
}

/// The principal for a verified OIDC identity, creating or refreshing its
/// `users` row. Refuses a disabled row, and a name owned by a local user or
/// the config admin, so a provider can never take over those accounts.
pub async fn resolve_user(
    st: &AppState,
    username: &str,
    role: Role,
) -> Result<Principal, AppError> {
    if username == st.cfg.mgmt.username {
        tracing::warn!(
            user = username,
            "oidc identity collides with the config admin"
        );
        return Err(AppError::Unauthorized);
    }
    let now = crate::util::now_ms();
    let row = users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(&st.db)
        .await?;
    let row = match row {
        Some(r) => r,
        None => {
            let insert = users::ActiveModel {
                username: Set(username.to_string()),
                password_hash: Set(None),
                role: Set(role.as_str().to_string()),
                source: Set(Source::Oidc.as_str().to_string()),
                enabled: Set(true),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(&st.db)
            .await;
            match insert {
                Ok(r) => r,
                // A concurrent first request won the unique index; use its row.
                Err(_) => users::Entity::find()
                    .filter(users::Column::Username.eq(username))
                    .one(&st.db)
                    .await?
                    .ok_or(AppError::Unauthorized)?,
            }
        }
    };
    if row.source != Source::Oidc.as_str() {
        tracing::warn!(user = username, "oidc identity collides with a local user");
        return Err(AppError::Unauthorized);
    }
    if !row.enabled {
        return Err(AppError::Unauthorized);
    }
    if row.role != role.as_str() {
        let mut am: users::ActiveModel = row.into();
        am.role = Set(role.as_str().to_string());
        am.updated_at = Set(now);
        am.update(&st.db).await?;
    }
    Ok(Principal {
        username: username.to_string(),
        role,
        source: Source::Oidc,
    })
}

/// Bearer branch of `mgmt_auth`.
pub async fn bearer(st: &AppState, token: &str) -> Result<Principal, AppError> {
    let Some(oidc) = &st.oidc else {
        return Err(AppError::Unauthorized);
    };
    let claims = oidc
        .verify(token, oidc.cfg.audience(), None)
        .await
        .inspect_err(|e| tracing::debug!(reason = ?e, "bearer token refused"))?;
    let (username, role) = oidc.identity(&claims)?;
    resolve_user(st, &username, role).await
}
