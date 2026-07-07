mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

fn login_req(user: &str, pass: &str) -> Request<Body> {
    Request::post("/rest/v1/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": user, "password": pass}).to_string(),
        ))
        .unwrap()
}

#[tokio::test]
async fn login_sets_session_cookie() {
    let (app, _) = common::setup().await;
    let resp = app
        .oneshot(login_req("admin", common::TEST_PASSWORD))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let c = resp.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(c.starts_with("raptor_session="));
    assert!(c.contains("HttpOnly"));
    assert!(c.contains("SameSite=Strict"));
    assert!(c.contains("Path=/"));
    assert!(!c.contains("Secure"));
}

#[tokio::test]
async fn login_secure_flag_behind_tls_proxy() {
    let (app, _) = common::setup().await;
    let mut req = login_req("admin", common::TEST_PASSWORD);
    req.headers_mut()
        .insert("x-forwarded-proto", "https".parse().unwrap());
    let resp = app.oneshot(req).await.unwrap();
    assert!(resp.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Secure"));
}

#[tokio::test]
async fn bad_password_rejected_without_cookie() {
    let (app, _) = common::setup().await;
    let resp = app.oneshot(login_req("admin", "wrong")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().get(header::SET_COOKIE).is_none());
    assert!(resp.headers().get("www-authenticate").is_none());
}

#[tokio::test]
async fn logout_clears_cookie() {
    let (app, _) = common::setup().await;
    let resp = app
        .clone()
        .oneshot(login_req("admin", common::TEST_PASSWORD))
        .await
        .unwrap();
    let cookie_pair = resp.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let resp = app
        .oneshot(
            Request::post("/rest/v1/logout")
                .header(header::COOKIE, &cookie_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(resp.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
}
