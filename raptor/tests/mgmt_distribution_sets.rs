mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

async fn create_module(app: &axum::Router) -> i64 {
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/softwaremodules",
            Some(json!([{"name": "rootfs", "version": "1.0", "type": "os"}])),
        ))
        .await
        .unwrap();
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

    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            "/rest/v1/distributionsets?q=name==sta*",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(common::body_json(resp).await["total"], 1);

    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/distributionsets/{ds_id}/assignedSM"),
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["content"][0]["name"], "rootfs");
}

#[tokio::test]
async fn ds_without_modules_incomplete_until_assigned() {
    let (app, _) = common::setup().await;
    let sm = create_module(&app).await;

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/distributionsets",
            Some(json!([{"name": "empty", "version": "1", "type": "os"}])),
        ))
        .await
        .unwrap();
    let ds = common::body_json(resp).await;
    assert_eq!(ds[0]["complete"], false);
    let ds_id = ds[0]["id"].as_i64().unwrap();

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/distributionsets/{ds_id}/assignedSM"),
            Some(json!([{"id": sm}])),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/distributionsets/{ds_id}"),
            None,
        ))
        .await
        .unwrap();
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
    let check = app
        .clone()
        .oneshot(common::req(
            "GET",
            "/rest/v1/distributionsets?q=name==dup-in-req",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(common::body_json(check).await["total"], 0);
}

#[tokio::test]
async fn update_ds_fields() {
    let (app, _) = common::setup().await;
    let created = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/distributionsets",
                Some(json!([{"name": "stable", "version": "1.0", "type": "os"}])),
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = created[0]["id"].as_i64().unwrap();

    // PUT changes name/version/description/requiredMigrationStep
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            &format!("/rest/v1/distributionsets/{id}"),
            Some(json!({
                "name": "renamed",
                "version": "2.0",
                "description": "now with notes",
                "requiredMigrationStep": true
            })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["name"], "renamed");
    assert_eq!(body["version"], "2.0");
    assert_eq!(body["description"], "now with notes");
    assert_eq!(body["requiredMigrationStep"], true);

    // omitted fields are left unchanged
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            &format!("/rest/v1/distributionsets/{id}"),
            Some(json!({"description": "just the description"})),
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["name"], "renamed");
    assert_eq!(body["version"], "2.0");
    assert_eq!(body["description"], "just the description");
}

#[tokio::test]
async fn update_ds_conflicting_name_version_409() {
    let (app, _) = common::setup().await;
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/distributionsets",
            Some(json!([
                {"name": "a", "version": "1.0", "type": "os"},
                {"name": "b", "version": "1.0", "type": "os"}
            ])),
        ))
        .await
        .unwrap();
    let list = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                "/rest/v1/distributionsets?q=name==b",
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let b_id = list["content"][0]["id"].as_i64().unwrap();

    // renaming b -> a:1.0 collides with the existing a:1.0
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            &format!("/rest/v1/distributionsets/{b_id}"),
            Some(json!({"name": "a"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn update_unknown_ds_404() {
    let (app, _) = common::setup().await;
    let resp = app
        .oneshot(common::req(
            "PUT",
            "/rest/v1/distributionsets/999",
            Some(json!({"name": "x"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
