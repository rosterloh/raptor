use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use base64::Engine;
use migration::{Migrator, MigratorTrait};
use raptor::config::Config;
use raptor::state::AppState;
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use serde_json::json;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tower::ServiceExt;

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
artifact_dir = "unused"
url = "http://localhost:8080"
[ddi]
anonymous = true
gateway_token = "gw-token"
[mgmt]
username = "admin"
password_hash = "{}"
"#,
        *TEST_HASH
    );
    use figment::providers::{Format, Toml};
    let mut cfg: Config = figment::Figment::new()
        .merge(Toml::string(&toml))
        .extract()
        .unwrap();
    // Set after parsing: a path can hold characters a TOML string would need escaped.
    cfg.artifact_dir = artifact_dir.into();
    cfg
}

/// The S3 settings for `TEST_S3_ENDPOINT`, when set, with a fresh key prefix
/// so parallel tests never see each other's blobs.
pub fn test_s3_config() -> Option<raptor::config::S3Config> {
    let endpoint = std::env::var("TEST_S3_ENDPOINT").ok()?;
    let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.into());
    Some(raptor::config::S3Config {
        endpoint,
        bucket: var("TEST_S3_BUCKET", "raptor-test"),
        region: var("TEST_S3_REGION", "us-east-1"),
        access_key: var("TEST_S3_ACCESS_KEY", "testkey"),
        secret_key: var("TEST_S3_SECRET_KEY", "testsecret"),
        prefix: format!("test-{}/", raptor::util::random_token()),
        path_style: true,
        ca_cert: None,
        part_size: 5 * 1024 * 1024,
    })
}

/// Local-disk store by default; the S3 backend when `TEST_S3_ENDPOINT` is set
/// (mirrors `TEST_DATABASE_URL` for Postgres), so the whole suite can run
/// against a real bucket.
fn test_store(dir: std::path::PathBuf) -> raptor::storage::ArtifactStore {
    match test_s3_config() {
        Some(s3) => raptor::storage::ArtifactStore::with_s3(
            &dir,
            raptor::storage::S3Store::new(&s3).unwrap(),
        )
        .unwrap(),
        None => raptor::storage::ArtifactStore::new(dir).unwrap(),
    }
}

pub async fn setup() -> (Router, AppState) {
    setup_with(|_| {}).await
}

/// setup() with the config edited first.
pub async fn setup_with(edit: impl FnOnce(&mut Config)) -> (Router, AppState) {
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
    // Under target/ so `cargo clean` reaps the directories forgotten below.
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let dir_path = dir.path().to_path_buf();
    let mut cfg = test_config(&dir_path);
    edit(&mut cfg);
    std::mem::forget(dir); // keep tempdir alive for the test process
    let store = test_store(dir_path);
    let state = AppState::new(db, cfg, store);
    (raptor::app::build_app(state.clone()), state)
}

/// setup() state only, with ddi.anonymous overridden.
pub async fn setup_with_anonymous(anonymous: bool) -> AppState {
    setup_with(|c| c.ddi.anonymous = anonymous).await.1
}

/// setup() with cfg.url overridden to the given base, and ddi.anonymous off,
/// so `_links` carry a real, dialable port and TargetToken auth is enforced.
pub async fn setup_with_url(url: &str) -> (Router, AppState) {
    setup_with(|c| {
        c.url = Some(url.into());
        c.ddi.anonymous = false;
    })
    .await
}

/// setup() with `[ddi.header_auth]` replaced and ddi.anonymous off, so
/// certificate auth is the only credential-free path.
pub async fn setup_with_header_auth(
    header_auth: raptor::config::HeaderAuthConfig,
) -> (Router, AppState) {
    setup_with(|c| {
        c.ddi.anonymous = false;
        c.ddi.header_auth = header_auth;
    })
    .await
}

/// A real server on a loopback port (so `_links` are dialable and TargetToken
/// auth is enforced), seeded over HTTP with one module and artifact, a
/// distribution set and a target. Returns the base URL, a client and the set id.
pub async fn serve_seeded(cid: &str, token: &str) -> (String, reqwest::Client, i64) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (app, _) = setup_with_url(&base).await;
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let http = reqwest::Client::new();
    let post = |path: &str| {
        http.post(format!("{base}{path}"))
            .basic_auth("admin", Some(TEST_PASSWORD))
    };
    let id = |v: serde_json::Value| v[0]["id"].as_i64().unwrap();

    let sm = id(post("/rest/v1/softwaremodules")
        .json(&json!([{"name": "fw", "version": "1.0", "type": "os"}]))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap());
    let part = reqwest::multipart::Part::bytes(b"raptor-e2e-payload".to_vec()).file_name("fw.bin");
    post(&format!("/rest/v1/softwaremodules/{sm}/artifacts"))
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let ds = id(post("/rest/v1/distributionsets")
        .json(&json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}]))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap());
    post("/rest/v1/targets")
        .json(&json!([{"controllerId": cid, "securityToken": token}]))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    (base, http, ds)
}

pub fn mgmt_auth_header() -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("admin:{TEST_PASSWORD}"))
    )
}

/// Stores a user whose password is [`TEST_PASSWORD`], hashed cheaply like the
/// config admin's (see [`TEST_HASH`]); returns its id.
pub async fn insert_user(st: &AppState, username: &str, role: &str, source: &str) -> i64 {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    raptor::entity::user::ActiveModel {
        username: Set(username.into()),
        password_hash: Set((source == "local").then(|| TEST_HASH.clone())),
        role: Set(role.into()),
        source: Set(source.into()),
        enabled: Set(true),
        created_at: Set(0),
        updated_at: Set(0),
        ..Default::default()
    }
    .insert(&st.db)
    .await
    .unwrap()
    .id
}

pub fn req(method: &str, uri: &str, body: Option<serde_json::Value>) -> Request<Body> {
    req_as("admin", method, uri, body)
}

/// [`req`] with Basic auth as `user` / [`TEST_PASSWORD`].
pub fn req_as(
    user: &str,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> Request<Body> {
    let auth = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{TEST_PASSWORD}"))
    );
    let b = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, auth)
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

/// Next complete SSE message (`event:` + `data:`) from the body, skipping
/// comment/keep-alive lines. Panics if none arrives within 2 s.
pub async fn sse_next(body: &mut Body) -> (String, serde_json::Value) {
    use http_body_util::BodyExt;
    let mut buf = String::new();
    loop {
        if let Some(i) = buf.find("\n\n") {
            let msg: String = buf.drain(..i + 2).collect();
            let mut name = None;
            let mut data = None;
            for l in msg.lines() {
                if let Some(v) = l.strip_prefix("event:") {
                    name = Some(v.trim().to_string());
                } else if let Some(v) = l.strip_prefix("data:") {
                    data = Some(serde_json::from_str(v.trim()).unwrap());
                }
            }
            if let (Some(n), Some(d)) = (name, data) {
                return (n, d);
            }
            continue;
        }
        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
            .await
            .expect("timed out waiting for SSE frame")
            .expect("SSE body ended")
            .unwrap();
        if let Ok(data) = frame.into_data() {
            buf.push_str(std::str::from_utf8(&data).unwrap());
        }
    }
}

/// Assert no SSE message arrives within `ms`.
pub async fn sse_none(body: &mut Body, ms: u64) {
    let r = tokio::time::timeout(std::time::Duration::from_millis(ms), sse_next(body)).await;
    assert!(r.is_err(), "unexpected SSE message: {:?}", r.unwrap());
}

const BOUNDARY: &str = "raptorboundary";

/// Authenticated multipart POST carrying one `file` part.
pub fn multipart_upload(uri: &str, filename: &str, content: &[u8]) -> Request<Body> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post(uri)
        .header(header::AUTHORIZATION, mgmt_auth_header())
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap()
}

/// Unauthenticated GET, for DDI routes under the default anonymous config.
pub fn ddi_get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

pub async fn status(app: &Router, req: Request<Body>) -> StatusCode {
    app.clone().oneshot(req).await.unwrap().status()
}

pub async fn create_target(app: &Router, cid: &str) {
    app.clone()
        .oneshot(req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": cid}])),
        ))
        .await
        .unwrap();
}

/// Creates an `os` software module at version 1.0; returns its id.
pub async fn create_module(app: &Router, name: &str) -> i64 {
    let resp = app
        .clone()
        .oneshot(req(
            "POST",
            "/rest/v1/softwaremodules",
            Some(json!([{"name": name, "version": "1.0", "type": "os"}])),
        ))
        .await
        .unwrap();
    body_json(resp).await[0]["id"].as_i64().unwrap()
}

/// Assigns the set to the target; returns the new action's id.
pub async fn assign(app: &Router, cid: &str, ds: i64) -> i64 {
    let resp = app
        .clone()
        .oneshot(req(
            "POST",
            &format!("/rest/v1/targets/{cid}/assignedDS"),
            Some(json!({"id": ds})),
        ))
        .await
        .unwrap();
    body_json(resp).await["assignedActions"][0]["id"]
        .as_i64()
        .unwrap()
}
