mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use tower::ServiceExt;

// Task 8 adds real /rest/v1 routes; this test uses softwaremoduletypes once it exists.
// Until then, mount a probe route through the same middleware.
async fn probe_app() -> axum::Router {
    let (_, state) = common::setup().await;
    axum::Router::new()
        .route("/rest/v1/probe", axum::routing::get(|| async { "ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            raptor::auth::mgmt::mgmt_auth,
        ))
        .with_state(state)
}

#[tokio::test]
async fn missing_credentials_rejected() {
    let app = probe_app().await;
    let resp = app
        .oneshot(Request::get("/rest/v1/probe").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().get("www-authenticate").is_some());
}

#[tokio::test]
async fn wrong_password_rejected() {
    use base64::Engine;
    let app = probe_app().await;
    let bad = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode("admin:wrong")
    );
    let resp = app
        .oneshot(
            Request::get("/rest/v1/probe")
                .header(header::AUTHORIZATION, bad)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn valid_credentials_accepted() {
    let app = probe_app().await;
    let resp = app
        .oneshot(
            Request::get("/rest/v1/probe")
                .header(header::AUTHORIZATION, common::mgmt_auth_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
