mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

const BOUNDARY: &str = "raptorboundary";

fn upload(uri: &str, filename: &str, content: &[u8]) -> Request<Body> {
    use axum::http::header;
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post(uri)
        .header(header::AUTHORIZATION, common::mgmt_auth_header())
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body)).unwrap()
}

/// Fixture: os module w/ artifact "fw.bin" (b"hello world"), ds "stable:1.0", target d1, forced assignment.
/// Returns (module_id, action_id).
async fn deploy_fixture(app: &axum::Router) -> (i64, i64) {
    let sm = common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/softwaremodules",
        Some(json!([{"name": "fw", "version": "1.0", "type": "os"}])))).await.unwrap()).await[0]["id"].as_i64().unwrap();
    app.clone().oneshot(upload(&format!("/rest/v1/softwaremodules/{sm}/artifacts"), "fw.bin", b"hello world")).await.unwrap();
    let ds = common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/distributionsets",
        Some(json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}])))).await.unwrap()).await[0]["id"].as_i64().unwrap();
    app.clone().oneshot(common::req("POST", "/rest/v1/targets", Some(json!([{"controllerId": "d1"}])))).await.unwrap();
    let r = common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/targets/d1/assignedDS",
        Some(json!({"id": ds, "type": "forced"})))).await.unwrap()).await;
    (sm, r["assignedActions"][0]["id"].as_i64().unwrap())
}

#[tokio::test]
async fn deployment_base_matches_hawkbit_shape() {
    let (app, _) = common::setup().await;
    let (sm, action_id) = deploy_fixture(&app).await;

    let resp = app.clone().oneshot(Request::get(&format!("/DEFAULT/controller/v1/d1/deploymentBase/{action_id}"))
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;

    // golden shape check (actionHistory checked loosely)
    let ddi = "http://localhost:8080/DEFAULT/controller/v1/d1";
    assert_eq!(body["id"], action_id.to_string());
    assert_eq!(body["deployment"]["download"], "forced");
    assert_eq!(body["deployment"]["update"], "forced");
    let chunk = &body["deployment"]["chunks"][0];
    assert_eq!(chunk["part"], "os");
    assert_eq!(chunk["version"], "1.0");
    assert_eq!(chunk["name"], "fw");
    let art = &chunk["artifacts"][0];
    assert_eq!(art["filename"], "fw.bin");
    assert_eq!(art["size"], 11);
    assert_eq!(art["hashes"]["sha1"], "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    assert_eq!(art["hashes"]["md5"], "5eb63bbbe01eeed093cb22bb8f5acdc3");
    assert_eq!(art["hashes"]["sha256"], "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    assert_eq!(art["_links"]["download-http"]["href"], format!("{ddi}/softwaremodules/{sm}/artifacts/fw.bin"));
    assert_eq!(art["_links"]["md5sum-http"]["href"], format!("{ddi}/softwaremodules/{sm}/artifacts/fw.bin.MD5SUM"));

    // wrong controller -> 404
    let resp = app.clone().oneshot(Request::get(&format!("/DEFAULT/controller/v1/other/deploymentBase/{action_id}"))
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
