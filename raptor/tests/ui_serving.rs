#![cfg(feature = "embed-ui")]

mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use tower::ServiceExt;

// rust-embed reads the folder from disk at runtime in debug builds, so tests
// can create placeholder assets. Release builds embed at compile time — CI
// runs `dx build --release` first (see .github/workflows/ci.yml ui job).
fn write_placeholder_assets() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/dx/raptor-ui/release/web/public");
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), "<html>raptor-ui</html>").unwrap();
    std::fs::write(dir.join("assets/app.css"), "body{}").unwrap();
    std::fs::create_dir_all(dir.join("ui")).unwrap();
    std::fs::write(dir.join("ui/nested.css"), ".n{}").unwrap();
}

async fn get(app: &axum::Router, path: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn serves_index_and_assets() {
    write_placeholder_assets();
    let (app, _) = common::setup().await;
    let resp = get(&app, "/ui").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    let resp = get(&app, "/ui/assets/app.css").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/css"));
}

#[tokio::test]
async fn spa_fallback_for_client_routes_but_404_for_missing_files() {
    write_placeholder_assets();
    let (app, _) = common::setup().await;
    // client-side route (no extension) → index.html
    let resp = get(&app, "/ui/targets/some-device").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    // missing file (has extension) → 404
    let resp = get(&app, "/ui/assets/missing.js").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nested_ui_subfolder_asset_is_served_single_stripped() {
    write_placeholder_assets();
    let (app, _) = common::setup().await;
    let resp = get(&app, "/ui/ui/nested.css").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/css"));
}

#[tokio::test]
async fn ui_does_not_require_auth() {
    write_placeholder_assets();
    let (app, _) = common::setup().await;
    assert_eq!(get(&app, "/ui").await.status(), StatusCode::OK);
}
