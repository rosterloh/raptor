mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

async fn login_cookie(app: &axum::Router) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::post("/rest/v1/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"username": "admin", "password": common::TEST_PASSWORD}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    resp.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

fn get_targets(cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::get("/rest/v1/targets");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn session_cookie_authenticates_mgmt_call() {
    let (app, _) = common::setup().await;
    let cookie = login_cookie(&app).await;
    let resp = app.oneshot(get_targets(Some(&cookie))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn bogus_cookie_rejected() {
    let (app, _) = common::setup().await;
    let resp = app
        .oneshot(get_targets(Some("raptor_session=deadbeef")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_invalidates_session() {
    let (app, _) = common::setup().await;
    let cookie = login_cookie(&app).await;
    app.clone()
        .oneshot(
            Request::post("/rest/v1/logout")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let resp = app.oneshot(get_targets(Some(&cookie))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn basic_auth_still_works() {
    let (app, _) = common::setup().await;
    let resp = app
        .oneshot(
            Request::get("/rest/v1/targets")
                .header(header::AUTHORIZATION, common::mgmt_auth_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
