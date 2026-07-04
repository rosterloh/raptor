mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

async fn create_module(app: &axum::Router) -> i64 {
    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/softwaremodules",
        Some(json!([{"name": "rootfs", "version": "1.0", "type": "os"}])))).await.unwrap();
    common::body_json(resp).await[0]["id"].as_i64().unwrap()
}

#[tokio::test]
async fn create_ds_with_modules_and_query() {
    let (app, _) = common::setup().await;
    let sm = create_module(&app).await;

    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/distributionsets",
        Some(json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}])))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let ds = common::body_json(resp).await;
    let ds_id = ds[0]["id"].as_i64().unwrap();
    assert_eq!(ds[0]["complete"], true);
    assert_eq!(ds[0]["type"], "os");
    assert_eq!(ds[0]["modules"][0]["id"], sm);

    let resp = app.clone().oneshot(common::req("GET", "/rest/v1/distributionsets?q=name==sta*", None)).await.unwrap();
    assert_eq!(common::body_json(resp).await["total"], 1);

    let resp = app.clone().oneshot(common::req("GET", &format!("/rest/v1/distributionsets/{ds_id}/assignedSM"), None)).await.unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["content"][0]["name"], "rootfs");
}

#[tokio::test]
async fn ds_without_modules_incomplete_until_assigned() {
    let (app, _) = common::setup().await;
    let sm = create_module(&app).await;

    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/distributionsets",
        Some(json!([{"name": "empty", "version": "1", "type": "app"}])))).await.unwrap();
    let ds = common::body_json(resp).await;
    assert_eq!(ds[0]["complete"], false);
    let ds_id = ds[0]["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(common::req("POST", &format!("/rest/v1/distributionsets/{ds_id}/assignedSM"),
        Some(json!([{"id": sm}])))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app.clone().oneshot(common::req("GET", &format!("/rest/v1/distributionsets/{ds_id}"), None)).await.unwrap();
    assert_eq!(common::body_json(resp).await["complete"], true);
}

#[tokio::test]
async fn duplicate_within_request_rejected() {
    let (app, _) = common::setup().await;

    // POST array with two identical brand-new DS (same name+version twice, both new)
    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/distributionsets",
        Some(json!([{"name": "dup-in-req", "version": "1.0", "type": "os"}, {"name": "dup-in-req", "version": "1.0", "type": "os"}])))).await.unwrap();

    // Should return 409 Conflict, not 500
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Verify nothing was persisted (list total should be 0)
    let check = app.clone().oneshot(common::req("GET", "/rest/v1/distributionsets?q=name==dup-in-req", None)).await.unwrap();
    assert_eq!(common::body_json(check).await["total"], 0);
}
