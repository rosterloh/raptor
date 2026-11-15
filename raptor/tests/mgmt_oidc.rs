#![allow(clippy::unwrap_used)]
//! OIDC against an in-process stub issuer (discovery, JWKS, token endpoint)
//! that signs test tokens with fixed throwaway P-256 keys.
mod common;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::routing::{get, post};
use base64::Engine;
use common::{body_json, insert_user, setup};
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use raptor::state::AppState;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const KEY_A: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgaOuHDnTDIUVxDm+m
DJqVMGMYY+F2QZeqyUCH2w8xxzWhRANCAAQ2krdqDIGTqHOq0MGMTib1+qq+TwB+
K5ndwHbutyHOwJRdyhWtE5azmWTVuZDsvosN+n4LyFXOkj3Vhqfb1UrP
-----END PRIVATE KEY-----
";
const KEY_B: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgvTIzUgrOdBSmvd6t
MyU+pp+/NPEzkdSC88Zm2bERdNWhRANCAASygumX/C6h5jS+CJkUdIIdxHNIe7pO
IA5G5FbGeWf9pgWmJEwYNqrHYLl/9iVrFhz1OWUbk/i06/wjtmuMzmli
-----END PRIVATE KEY-----
";

#[derive(Default)]
struct Stub {
    issuer: Mutex<String>,
    up: AtomicBool,
    keys: Mutex<JwkSet>,
    jwks_fetches: AtomicUsize,
    id_token: Mutex<String>,
    token_form: Mutex<HashMap<String, String>>,
}

/// PKCS#8 DER from the PEM body (`from_ec_pem` would need the `use_pem` feature).
fn key(pem: &str) -> EncodingKey {
    let b64: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    EncodingKey::from_ec_der(
        &base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap(),
    )
}

fn jwk(pem: &str, kid: &str) -> Jwk {
    let key = key(pem);
    let mut j = Jwk::from_encoding_key(&key, Algorithm::ES256).unwrap();
    j.common.key_id = Some(kid.into());
    j
}

fn sign(pem: &str, kid: &str, claims: &Value) -> String {
    let mut h = Header::new(Algorithm::ES256);
    h.kid = Some(kid.into());
    jsonwebtoken::encode(&h, claims, &key(pem)).unwrap()
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

async fn discovery(State(s): State<Arc<Stub>>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !s.up.load(Ordering::SeqCst) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let iss = s.issuer.lock().unwrap().clone();
    let base = iss.trim_end_matches('/');
    axum::Json(json!({
        "issuer": iss,
        "authorization_endpoint": format!("{base}/auth"),
        "token_endpoint": format!("{base}/token"),
        "jwks_uri": format!("{base}/jwks"),
    }))
    .into_response()
}

async fn jwks(State(s): State<Arc<Stub>>) -> axum::Json<JwkSet> {
    s.jwks_fetches.fetch_add(1, Ordering::SeqCst);
    axum::Json(s.keys.lock().unwrap().clone())
}

async fn token(State(s): State<Arc<Stub>>, body: String) -> axum::Json<Value> {
    *s.token_form.lock().unwrap() = url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect();
    axum::Json(json!({"id_token": s.id_token.lock().unwrap().clone(), "token_type": "Bearer"}))
}

/// A stub issuer serving key `a`, and raptor configured against it.
async fn harness(up: bool) -> (axum::Router, AppState, Arc<Stub>) {
    harness_at(up, "", false).await
}

/// The stub serves its issuer under `path`. With `slash`, the provider's own
/// issuer (and so every token's `iss`) ends in `/`, Authentik-style, while
/// raptor's config leaves it off.
async fn harness_at(up: bool, path: &str, slash: bool) -> (axum::Router, AppState, Arc<Stub>) {
    let stub = Arc::new(Stub::default());
    stub.up.store(up, Ordering::SeqCst);
    *stub.keys.lock().unwrap() = JwkSet {
        keys: vec![jwk(KEY_A, "a")],
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let iss = format!("http://{}{path}", listener.local_addr().unwrap());
    *stub.issuer.lock().unwrap() = if slash {
        format!("{iss}/")
    } else {
        iss.clone()
    };
    let routes = axum::Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/token", post(token))
        .with_state(stub.clone());
    let app = if path.is_empty() {
        routes
    } else {
        axum::Router::new().nest(path, routes)
    };
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let (_, st) = setup().await;
    let mut cfg = st.cfg.clone();
    cfg.mgmt.oidc = Some(
        serde_json::from_value(json!({
            "issuer": iss,
            "client_id": "raptor",
            "client_secret": "s3cret",
            "role_claim": "groups",
            "admin_values": ["raptor-admins"],
            "read_only_values": ["raptor-viewers"],
        }))
        .unwrap(),
    );
    raptor::auth::oidc::validate(&cfg).unwrap();
    let st = AppState::new(st.db.clone(), cfg, st.store.clone());
    if up {
        st.oidc.as_ref().unwrap().load().await.unwrap();
    }
    (raptor::app::build_app(st.clone()), st, stub)
}

fn claims(stub: &Stub, user: &str, groups: Value) -> Value {
    json!({
        "iss": stub.issuer.lock().unwrap().clone(),
        "aud": "raptor",
        "exp": now() + 300,
        "iat": now(),
        "preferred_username": user,
        "groups": groups,
    })
}

fn bearer(token: &str, method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap()
}

async fn status(app: &axum::Router, r: Request<Body>) -> StatusCode {
    app.clone().oneshot(r).await.unwrap().status()
}

fn get_req(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn valid_bearer_creates_jit_user() {
    let (app, st, stub) = harness(true).await;
    let t = sign(
        KEY_A,
        "a",
        &claims(&stub, "carol", json!(["raptor-admins"])),
    );
    let resp = app
        .clone()
        .oneshot(bearer(&t, "GET", "/rest/v1/userinfo"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let me = body_json(resp).await;
    assert_eq!(me["username"], "carol");
    assert_eq!(me["role"], "admin");

    let users = body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/users", None))
            .await
            .unwrap(),
    )
    .await;
    let carol = &users["content"][0];
    assert_eq!(carol["username"], "carol");
    assert_eq!(carol["source"], "oidc");
    assert_eq!(carol["role"], "admin");

    // The role is re-read from every token; the row follows it.
    let t = sign(KEY_A, "a", &claims(&stub, "carol", json!("raptor-viewers")));
    assert_eq!(
        status(&app, bearer(&t, "POST", "/rest/v1/targets")).await,
        StatusCode::FORBIDDEN
    );
    let row = raptor::entity::user::Entity::find_by_id(carol["id"].as_i64().unwrap());
    use sea_orm::EntityTrait;
    assert_eq!(row.one(&st.db).await.unwrap().unwrap().role, "read_only");
}

#[tokio::test]
async fn authentik_style_issuer_with_trailing_slash() {
    let (app, _st, stub) = harness_at(true, "/application/o/raptor", true).await;
    let t = sign(
        KEY_A,
        "a",
        &claims(&stub, "carol", json!(["raptor-admins"])),
    );
    assert!(
        claims(&stub, "x", Value::Null)["iss"]
            .as_str()
            .unwrap()
            .ends_with("/raptor/")
    );
    assert_eq!(
        status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(&app, get_req("/rest/v1/oidc/login")).await,
        StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn bad_tokens_are_401() {
    let (app, _st, stub) = harness(true).await;
    let ok = claims(&stub, "carol", json!(["raptor-admins"]));
    let mut cases = Vec::new();
    let mut c = ok.clone();
    c["exp"] = json!(now() - 120);
    cases.push(("expired", sign(KEY_A, "a", &c)));
    let mut c = ok.clone();
    c["aud"] = json!("other-client");
    cases.push(("wrong audience", sign(KEY_A, "a", &c)));
    let mut c = ok.clone();
    c["iss"] = json!("http://evil.example");
    cases.push(("wrong issuer", sign(KEY_A, "a", &c)));
    let mut c = ok.clone();
    c["nbf"] = json!(now() + 600);
    cases.push(("not yet valid", sign(KEY_A, "a", &c)));
    // Right kid, wrong key: the signature check, not the kid lookup, fails it.
    cases.push(("bad signature", sign(KEY_B, "a", &ok)));
    cases.push(("garbage", "not.a.jwt".into()));
    for (what, t) in cases {
        assert_eq!(
            status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
            StatusCode::UNAUTHORIZED,
            "{what}"
        );
    }
    // Within the 60 s leeway an expired token still passes.
    let mut c = ok.clone();
    c["exp"] = json!(now() - 30);
    assert_eq!(
        status(
            &app,
            bearer(&sign(KEY_A, "a", &c), "GET", "/rest/v1/targets")
        )
        .await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn hs256_token_is_refused() {
    let (app, _st, stub) = harness(true).await;
    let mut h = Header::new(Algorithm::HS256);
    h.kid = Some("a".into());
    let t = jsonwebtoken::encode(
        &h,
        &claims(&stub, "carol", json!(["raptor-admins"])),
        &EncodingKey::from_secret(b"anything"),
    )
    .unwrap();
    assert_eq!(
        status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn key_rotation_refreshes_once_then_rate_limits() {
    let (app, _st, stub) = harness(true).await;
    let fetches = stub.jwks_fetches.load(Ordering::SeqCst);
    stub.keys.lock().unwrap().keys.push(jwk(KEY_B, "b"));
    let t = sign(
        KEY_B,
        "b",
        &claims(&stub, "carol", json!(["raptor-admins"])),
    );
    assert_eq!(
        status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
        StatusCode::OK
    );
    assert_eq!(stub.jwks_fetches.load(Ordering::SeqCst), fetches + 1);

    let t = sign(
        KEY_B,
        "zzz",
        &claims(&stub, "carol", json!(["raptor-admins"])),
    );
    assert_eq!(
        status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        stub.jwks_fetches.load(Ordering::SeqCst),
        fetches + 1,
        "a second unknown kid within a minute must not refetch"
    );
}

#[tokio::test]
async fn issuer_down_at_startup_then_recovers() {
    let (app, st, stub) = harness(false).await;
    st.oidc.clone().unwrap().spawn_loader();

    assert_eq!(
        status(&app, get_req("/rest/v1/oidc/login")).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let t = sign(
        KEY_A,
        "a",
        &claims(&stub, "carol", json!(["raptor-admins"])),
    );
    assert_eq!(
        status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
        StatusCode::UNAUTHORIZED
    );
    // Devices and the config admin don't need the provider.
    assert_eq!(
        status(&app, get_req("/DEFAULT/controller/v1/dev1")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(&app, common::req("GET", "/rest/v1/targets", None)).await,
        StatusCode::OK
    );

    stub.up.store(true, Ordering::SeqCst);
    let oidc = st.oidc.clone().unwrap();
    for _ in 0..50 {
        if oidc.is_ready() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        oidc.is_ready(),
        "loader should retry within its first backoff"
    );
    assert_eq!(
        status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(&app, get_req("/rest/v1/oidc/login")).await,
        StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn role_mapping_and_no_match() {
    let (app, _st, stub) = harness(true).await;
    let viewer = sign(KEY_A, "a", &claims(&stub, "vic", json!(["raptor-viewers"])));
    assert_eq!(
        status(&app, bearer(&viewer, "GET", "/rest/v1/targets")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(&app, bearer(&viewer, "POST", "/rest/v1/targets")).await,
        StatusCode::FORBIDDEN
    );
    // Admin wins when both match.
    let both = sign(
        KEY_A,
        "a",
        &claims(&stub, "bo", json!(["raptor-viewers", "raptor-admins"])),
    );
    assert_eq!(
        status(&app, bearer(&both, "GET", "/rest/v1/users")).await,
        StatusCode::OK
    );
    let sales = sign(KEY_A, "a", &claims(&stub, "sam", json!(["sales"])));
    assert_eq!(
        status(&app, bearer(&sales, "GET", "/rest/v1/targets")).await,
        StatusCode::FORBIDDEN
    );
    let none = sign(KEY_A, "a", &claims(&stub, "sam", Value::Null));
    assert_eq!(
        status(&app, bearer(&none, "GET", "/rest/v1/targets")).await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn disabled_and_colliding_users_are_refused() {
    let (app, st, stub) = harness(true).await;
    let id = insert_user(&st, "carol", "admin", "oidc").await;
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            &format!("/rest/v1/users/{id}"),
            Some(json!({"enabled": false})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let t = sign(
        KEY_A,
        "a",
        &claims(&stub, "carol", json!(["raptor-admins"])),
    );
    assert_eq!(
        status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
        StatusCode::UNAUTHORIZED
    );

    // A provider identity can't take over a local user or the config admin.
    insert_user(&st, "dave", "admin", "local").await;
    for name in ["dave", st.cfg.mgmt.username.as_str()] {
        let t = sign(KEY_A, "a", &claims(&stub, name, json!(["raptor-admins"])));
        assert_eq!(
            status(&app, bearer(&t, "GET", "/rest/v1/targets")).await,
            StatusCode::UNAUTHORIZED,
            "{name}"
        );
    }
}

#[tokio::test]
async fn oidc_user_has_no_password_login() {
    let (app, st, _stub) = harness(true).await;
    insert_user(&st, "carol", "admin", "oidc").await;
    let r = Request::post("/rest/v1/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "carol", "password": common::TEST_PASSWORD}).to_string(),
        ))
        .unwrap();
    assert_eq!(status(&app, r).await, StatusCode::UNAUTHORIZED);
    let basic = base64::engine::general_purpose::STANDARD
        .encode(format!("carol:{}", common::TEST_PASSWORD));
    let r = Request::get("/rest/v1/targets")
        .header(header::AUTHORIZATION, format!("Basic {basic}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(&app, r).await, StatusCode::UNAUTHORIZED);
}

fn location_params(resp: &axum::http::Response<Body>) -> HashMap<String, String> {
    let loc = resp.headers()[header::LOCATION].to_str().unwrap();
    url::Url::parse(loc)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

#[tokio::test]
async fn full_callback_flow() {
    let (app, _st, stub) = harness(true).await;
    assert_eq!(
        status(&app, get_req("/rest/v1/oidc/login?probe")).await,
        StatusCode::NO_CONTENT
    );
    let resp = app
        .clone()
        .oneshot(get_req("/rest/v1/oidc/login"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let p = location_params(&resp);
    assert_eq!(p["response_type"], "code");
    assert_eq!(p["client_id"], "raptor");
    assert_eq!(
        p["redirect_uri"],
        "http://localhost:8080/rest/v1/oidc/callback"
    );
    assert_eq!(p["code_challenge_method"], "S256");

    // An ID token carrying someone else's nonce is refused.
    let mut c = claims(&stub, "erin", json!(["raptor-admins"]));
    c["nonce"] = json!("not-the-nonce");
    *stub.id_token.lock().unwrap() = sign(KEY_A, "a", &c);
    let cb = format!("/rest/v1/oidc/callback?code=xyz&state={}", p["state"]);
    assert_eq!(status(&app, get_req(&cb)).await, StatusCode::UNAUTHORIZED);

    // A fresh login with the right nonce succeeds.
    let resp = app
        .clone()
        .oneshot(get_req("/rest/v1/oidc/login"))
        .await
        .unwrap();
    let p = location_params(&resp);
    c["nonce"] = json!(p["nonce"]);
    *stub.id_token.lock().unwrap() = sign(KEY_A, "a", &c);
    let cb = format!("/rest/v1/oidc/callback?code=xyz&state={}", p["state"]);
    let resp = app.clone().oneshot(get_req(&cb)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers()[header::LOCATION], "/ui");
    let cookie = resp.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let form = stub.token_form.lock().unwrap().clone();
    assert_eq!(form["code"], "xyz");
    assert_eq!(form["client_secret"], "s3cret");
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(form["code_verifier"].as_bytes()));
    assert_eq!(challenge, p["code_challenge"]);

    let r = Request::get("/rest/v1/userinfo")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let me = body_json(app.clone().oneshot(r).await.unwrap()).await;
    assert_eq!(me["username"], "erin");

    // The state is single-use.
    assert_eq!(status(&app, get_req(&cb)).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn forged_state_and_unmapped_login() {
    let (app, _st, stub) = harness(true).await;
    assert_eq!(
        status(
            &app,
            get_req("/rest/v1/oidc/callback?code=xyz&state=forged")
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    let resp = app
        .clone()
        .oneshot(get_req("/rest/v1/oidc/login"))
        .await
        .unwrap();
    let p = location_params(&resp);
    let mut c = claims(&stub, "sam", json!(["sales"]));
    c["nonce"] = json!(p["nonce"]);
    *stub.id_token.lock().unwrap() = sign(KEY_A, "a", &c);
    let resp = app
        .clone()
        .oneshot(get_req(&format!(
            "/rest/v1/oidc/callback?code=xyz&state={}",
            p["state"]
        )))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(resp.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn without_oidc_no_routes_and_no_bearer() {
    let (app, _st) = setup().await;
    assert_eq!(
        status(&app, get_req("/rest/v1/oidc/login?probe")).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(&app, bearer("x", "GET", "/rest/v1/targets")).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn startup_validation() {
    let (_, st) = setup().await;
    let oidc: raptor::config::OidcConfig = serde_json::from_value(json!({
        "issuer": "https://id.example.com/realms/ops",
        "client_id": "raptor",
        "client_secret": "s",
        "role_claim": "groups",
        "admin_values": ["a"],
    }))
    .unwrap();
    let mut cfg = st.cfg.clone();
    cfg.mgmt.oidc = Some(oidc.clone());
    assert!(raptor::auth::oidc::validate(&cfg).is_ok());
    cfg.url = None;
    assert!(
        raptor::auth::oidc::validate(&cfg)
            .unwrap_err()
            .contains("url")
    );
    cfg.url = st.cfg.url.clone();
    let mut bad = oidc.clone();
    bad.issuer = "not a url".into();
    cfg.mgmt.oidc = Some(bad);
    assert!(raptor::auth::oidc::validate(&cfg).is_err());
    let mut bad = oidc;
    bad.admin_values.clear();
    cfg.mgmt.oidc = Some(bad);
    assert!(raptor::auth::oidc::validate(&cfg).is_err());
    // The secret never reaches a log line.
    assert!(!format!("{:?}", cfg.mgmt.oidc).contains("\"s\""));
}
