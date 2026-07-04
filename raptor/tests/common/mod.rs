#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Request, Response};
use axum::Router;
use base64::Engine;
use migration::{Migrator, MigratorTrait};
use raptor::config::Config;
use raptor::state::AppState;
use sea_orm::Database;
use std::sync::LazyLock;

pub const TEST_PASSWORD: &str = "raptor-test";

static TEST_HASH: LazyLock<String> = LazyLock::new(|| {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    let salt = SaltString::generate(&mut OsRng);
    argon2::Argon2::default().hash_password(TEST_PASSWORD.as_bytes(), &salt).unwrap().to_string()
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
    figment::Figment::new().merge(Toml::string(&toml)).extract().unwrap()
}

pub async fn setup() -> (Router, AppState) {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let db = Database::connect(&url).await.unwrap();
    if url.starts_with("postgres") {
        // fresh schema per run; CI uses --test-threads=1
        Migrator::fresh(&db).await.unwrap();
    } else {
        Migrator::up(&db, None).await.unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let cfg = test_config(dir.path());
    std::mem::forget(dir); // keep tempdir alive for the test process
    let state = AppState::new(db, cfg);
    (raptor::app::build_app(state.clone()), state)
}

pub fn mgmt_auth_header() -> String {
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("admin:{TEST_PASSWORD}")))
}

pub fn req(method: &str, uri: &str, body: Option<serde_json::Value>) -> Request<Body> {
    let b = Request::builder().method(method).uri(uri)
        .header(header::AUTHORIZATION, mgmt_auth_header())
        .header(header::CONTENT_TYPE, "application/json");
    match body {
        Some(j) => b.body(Body::from(j.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

pub async fn body_json(resp: Response<Body>) -> serde_json::Value {
    let bytes = http_body_util::BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}
