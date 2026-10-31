#![allow(clippy::unwrap_used)] // helpers outside #[test] fns aren't covered by allow-unwrap-in-tests
mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_200() {
    let (app, _) = common::setup().await;
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
