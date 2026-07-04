mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn create_list_get_delete_module() {
    let (app, _) = common::setup().await;

    // create (array body, like hawkBit)
    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/softwaremodules",
        Some(json!([{"name": "rootfs", "version": "1.0.0", "type": "os", "vendor": "acme"}])))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = common::body_json(resp).await;
    let id = body[0]["id"].as_i64().unwrap();
    assert_eq!(body[0]["name"], "rootfs");
    assert_eq!(body[0]["type"], "os");
    assert!(body[0]["createdAt"].as_i64().unwrap() > 0);

    // list with FIQL
    let resp = app.clone().oneshot(common::req("GET", "/rest/v1/softwaremodules?q=name==root*", None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["size"], 1);
    assert_eq!(body["content"][0]["id"], id);

    // get one
    let resp = app.clone().oneshot(common::req("GET", &format!("/rest/v1/softwaremodules/{id}"), None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // update
    let resp = app.clone().oneshot(common::req("PUT", &format!("/rest/v1/softwaremodules/{id}"),
        Some(json!({"description": "root filesystem"})))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["description"], "root filesystem");

    // delete then 404
    let resp = app.clone().oneshot(common::req("DELETE", &format!("/rest/v1/softwaremodules/{id}"), None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app.clone().oneshot(common::req("GET", &format!("/rest/v1/softwaremodules/{id}"), None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = common::body_json(resp).await;
    assert_eq!(body["errorCode"], "hawkbit.server.error.repo.entitiyNotFound");
}

#[tokio::test]
async fn duplicate_module_conflicts_and_unknown_type_rejected() {
    let (app, _) = common::setup().await;
    let m = json!([{"name": "fw", "version": "1", "type": "os"}]);
    assert_eq!(app.clone().oneshot(common::req("POST", "/rest/v1/softwaremodules", Some(m.clone()))).await.unwrap().status(), StatusCode::CREATED);
    assert_eq!(app.clone().oneshot(common::req("POST", "/rest/v1/softwaremodules", Some(m))).await.unwrap().status(), StatusCode::CONFLICT);
    assert_eq!(app.clone().oneshot(common::req("POST", "/rest/v1/softwaremodules",
        Some(json!([{"name": "x", "version": "1", "type": "nope"}])))).await.unwrap().status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn type_endpoints_are_seeded_and_readonly() {
    let (app, _) = common::setup().await;
    let resp = app.clone().oneshot(common::req("GET", "/rest/v1/softwaremoduletypes", None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 4);
    let resp = app.clone().oneshot(common::req("GET", "/rest/v1/distributionsettypes", None)).await.unwrap();
    assert_eq!(common::body_json(resp).await["total"], 3);
}
