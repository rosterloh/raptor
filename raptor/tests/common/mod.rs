#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, header};
use base64::Engine;
use migration::{Migrator, MigratorTrait};
use raptor::config::Config;
use raptor::state::AppState;
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-process counter making each test's Postgres schema name unique.
static SCHEMA_SEQ: AtomicU64 = AtomicU64::new(0);

pub const TEST_PASSWORD: &str = "raptor-test";

/// The test operator's password hash, deliberately generated with argon2's
/// *weakest* parameters rather than its defaults.
///
/// Every authenticated request in the suite runs `verify_creds`, and argon2's
/// real parameters are tuned to cost ~0.5s in an unoptimised test build — which
/// made password verification, not the code under test, by far the largest
/// line in the CI bill. The cost parameters live inside the PHC string, so the
/// server still verifies through exactly the same `Argon2::default()
/// .verify_password` path, reading these params back off the hash; a wrong
/// password is still rejected. Only the work factor changes, and a fixture
/// credential for an in-memory database has nothing to resist.
///
/// This is test-only. `raptor hash-password`, which generates the hash an
/// operator actually deploys, is untouched and still uses argon2's defaults.
static TEST_HASH: LazyLock<String> = LazyLock::new(|| {
    use argon2::PasswordHasher;
    let argon2 = argon2::Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(argon2::Params::MIN_M_COST, 1, 1, None).unwrap(),
    );
    argon2
        .hash_password_with_salt(TEST_PASSWORD.as_bytes(), &raptor::util::random_salt())
        .unwrap()
        .to_string()
});

pub fn test_config(artifact_dir: &std::path::Path) -> Config {
    let toml = format!(
        r#"
database_url = "unused"
artifact_dir = "{}"
url = "http://localhost:8080"
[ddi]
anonymous = true
gateway_token = "gw-token"
[mgmt]
username = "admin"
password_hash = "{}"
"#,
        artifact_dir.display(),
        *TEST_HASH
    );
    use figment::providers::{Format, Toml};
    figment::Figment::new()
        .merge(Toml::string(&toml))
        .extract()
        .unwrap()
}

pub async fn setup() -> (Router, AppState) {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let db = if url.starts_with("postgres") {
        // Isolate each test in its own schema so the Postgres suite can run in
        // parallel — there is no shared `public` schema for concurrent tests to
        // clobber, so no `--test-threads=1` is needed.
        let schema = format!(
            "test_{}_{}",
            std::process::id(),
            SCHEMA_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let admin = Database::connect(&url).await.unwrap();
        admin
            .execute_unprepared(&format!("CREATE SCHEMA \"{schema}\""))
            .await
            .unwrap();
        let mut opt = ConnectOptions::new(url);
        opt.set_schema_search_path(schema);
        let db = Database::connect(opt).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    } else {
        let db = Database::connect(&url).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    };
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path().to_path_buf();
    let cfg = test_config(&dir_path);
    std::mem::forget(dir); // keep tempdir alive for the test process
    let store = raptor::storage::ArtifactStore::new(dir_path).unwrap();
    let state = AppState::new(db, cfg, store);
    (raptor::app::build_app(state.clone()), state)
}

/// Like setup() but returns only state, with ddi.anonymous overridden.
pub async fn setup_with_anonymous(anonymous: bool) -> AppState {
    let (_, state) = setup().await;
    // Config is plain data: rebuild with the flag flipped
    let mut cfg = state.cfg.clone();
    cfg.ddi.anonymous = anonymous;
    AppState::new(state.db.clone(), cfg, state.store.clone())
}

/// Like setup() but with cfg.url overridden to the given base, and ddi.anonymous off,
/// so `_links` carry a real, dialable port and TargetToken auth is enforced.
pub async fn setup_with_url(url: &str) -> (Router, AppState) {
    let (_, state) = setup().await;
    let mut cfg = state.cfg.clone();
    cfg.url = Some(url.into());
    cfg.ddi.anonymous = false;
    let state = AppState::new(state.db.clone(), cfg, state.store.clone());
    (raptor::app::build_app(state.clone()), state)
}

/// Like setup() but with the rollout approval gate turned on, so a created
/// rollout lands in `waiting_for_approval` instead of `ready`.
pub async fn setup_with_rollout_approval() -> (Router, AppState) {
    let (_, state) = setup().await;
    let mut cfg = state.cfg.clone();
    cfg.rollout_approval_enabled = true;
    let state = AppState::new(state.db.clone(), cfg, state.store.clone());
    (raptor::app::build_app(state.clone()), state)
}

/// Like setup() but with the quota config replaced, so a test can drive a
/// limit without inserting hawkBit's default of hundreds of entities.
pub async fn setup_with_quota(quota: raptor::config::QuotaConfig) -> (Router, AppState) {
    let (_, state) = setup().await;
    let mut cfg = state.cfg.clone();
    cfg.quota = quota;
    let state = AppState::new(state.db.clone(), cfg, state.store.clone());
    (raptor::app::build_app(state.clone()), state)
}

/// Like setup() but with the action-cleanup config replaced.
pub async fn setup_with_cleanup(cleanup: raptor::config::CleanupConfig) -> (Router, AppState) {
    let (_, state) = setup().await;
    let mut cfg = state.cfg.clone();
    cfg.cleanup = cleanup;
    let state = AppState::new(state.db.clone(), cfg, state.store.clone());
    (raptor::app::build_app(state.clone()), state)
}

pub fn mgmt_auth_header() -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("admin:{TEST_PASSWORD}"))
    )
}

pub fn req(method: &str, uri: &str, body: Option<serde_json::Value>) -> Request<Body> {
    let b = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, mgmt_auth_header())
        .header(header::CONTENT_TYPE, "application/json");
    match body {
        Some(j) => b.body(Body::from(j.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

pub async fn body_json(resp: Response<Body>) -> serde_json::Value {
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}
