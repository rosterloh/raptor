mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn create_list_update_delete_target() {
    let (app, _) = common::setup().await;

    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/targets",
        Some(json!([{"controllerId": "dev-1"}, {"controllerId": "dev-2", "name": "Device 2", "securityToken": "tok2"}])))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = common::body_json(resp).await;
    assert_eq!(body[0]["controllerId"], "dev-1");
    assert_eq!(body[0]["name"], "dev-1");            // defaults to controllerId
    assert_eq!(body[0]["updateStatus"], "unknown");
    assert_eq!(body[0]["securityToken"].as_str().unwrap().len(), 32);
    assert_eq!(body[1]["securityToken"], "tok2");
    assert!(body[0]["pollStatus"].is_null());

    // FIQL filter
    let resp = app.clone().oneshot(common::req("GET", "/rest/v1/targets?q=controllerId==dev-2", None)).await.unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["content"][0]["name"], "Device 2");

    // update
    let resp = app.clone().oneshot(common::req("PUT", "/rest/v1/targets/dev-1",
        Some(json!({"description": "first device"})))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["description"], "first device");

    // attributes empty map
    let resp = app.clone().oneshot(common::req("GET", "/rest/v1/targets/dev-1/attributes", None)).await.unwrap();
    assert_eq!(common::body_json(resp).await, json!({}));

    // delete
    assert_eq!(app.clone().oneshot(common::req("DELETE", "/rest/v1/targets/dev-1", None)).await.unwrap().status(), StatusCode::OK);
    assert_eq!(app.clone().oneshot(common::req("GET", "/rest/v1/targets/dev-1", None)).await.unwrap().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn duplicate_controller_id_conflicts() {
    let (app, _) = common::setup().await;
    let t = json!([{"controllerId": "dup"}]);
    assert_eq!(app.clone().oneshot(common::req("POST", "/rest/v1/targets", Some(t.clone()))).await.unwrap().status(), StatusCode::CREATED);
    assert_eq!(app.clone().oneshot(common::req("POST", "/rest/v1/targets", Some(t))).await.unwrap().status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn partial_write_prevented_on_bulk_conflict() {
    let (app, _) = common::setup().await;

    // Create "already-there" beforehand
    app.clone().oneshot(common::req("POST", "/rest/v1/targets",
        Some(json!([{"controllerId": "already-there"}])))).await.unwrap();

    // POST array with brand-new and already-there (conflict)
    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/targets",
        Some(json!([{"controllerId": "brand-new"}, {"controllerId": "already-there"}])))).await.unwrap();

    // Should return 409 because second item conflicts
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Verify brand-new was NOT created (no partial write)
    let check = app.clone().oneshot(common::req("GET", "/rest/v1/targets/brand-new", None)).await.unwrap();
    assert_eq!(check.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn duplicate_within_request_rejected() {
    let (app, _) = common::setup().await;

    // POST array with two identical brand-new controllerIds (duplicate within same request)
    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/targets",
        Some(json!([{"controllerId": "dup-in-req"}, {"controllerId": "dup-in-req"}])))).await.unwrap();

    // Should return 409 Conflict, not 500
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Verify nothing was persisted (404 on GET)
    let check = app.clone().oneshot(common::req("GET", "/rest/v1/targets/dup-in-req", None)).await.unwrap();
    assert_eq!(check.status(), StatusCode::NOT_FOUND);
}
