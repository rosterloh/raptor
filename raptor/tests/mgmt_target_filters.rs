mod common;

use axum::http::StatusCode;
use raptor::domain::target_filter::auto_assign_all;
use serde_json::json;
use tower::ServiceExt;

/// Creates a complete DS (os module) and returns its id.
async fn complete_ds(app: &axum::Router) -> i64 {
    let sm = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/softwaremodules",
                Some(json!([{"name": "rootfs", "version": "1.0", "type": "os"}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/distributionsets",
                Some(json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap()
}

async fn create_filter(app: &axum::Router, name: &str, query: &str) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targetfilters",
                Some(json!({"name": name, "query": query})),
            ))
            .await
            .unwrap(),
    )
    .await
}

async fn assigned_ds_id(app: &axum::Router, cid: &str) -> Option<i64> {
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/targets/{cid}/assignedDS"),
            None,
        ))
        .await
        .unwrap();
    if resp.status() == StatusCode::NO_CONTENT {
        return None;
    }
    common::body_json(resp).await["id"].as_i64()
}

#[tokio::test]
async fn crud_lifecycle() {
    let (app, _) = common::setup().await;

    let f = create_filter(&app, "beta", "controllerId==dev-*").await;
    let id = f["id"].as_i64().unwrap();
    assert_eq!(f["name"], "beta");
    assert_eq!(f["query"], "controllerId==dev-*");
    assert!(f["autoAssignDistributionSet"].is_null());

    // list
    let list = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targetfilters", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["content"][0]["id"], id);

    // update query
    let updated = common::body_json(
        app.clone()
            .oneshot(common::req(
                "PUT",
                &format!("/rest/v1/targetfilters/{id}"),
                Some(json!({"query": "name==prod-*"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(updated["query"], "name==prod-*");

    // delete
    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/targetfilters/{id}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/targetfilters/{id}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn invalid_fiql_rejected() {
    let (app, _) = common::setup().await;
    // unknown field -> 400
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targetfilters",
            Some(json!({"name": "bad", "query": "bogusField==1"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = common::body_json(resp).await;
    assert_eq!(
        body["errorCode"],
        "hawkbit.server.error.rest.body.notReadable"
    );
}

#[tokio::test]
async fn duplicate_name_conflicts() {
    let (app, _) = common::setup().await;
    create_filter(&app, "dup", "name==*").await;
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targetfilters",
            Some(json!({"name": "dup", "query": "name==*"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn set_auto_assign_assigns_existing_matching_targets() {
    let (app, _) = common::setup().await;
    let ds = complete_ds(&app).await;

    // two matching targets + one non-matching
    for cid in ["dev-1", "dev-2", "prod-1"] {
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets",
                Some(json!([{"controllerId": cid}])),
            ))
            .await
            .unwrap();
    }

    let f = create_filter(&app, "beta", "controllerId==dev-*").await;
    let id = f["id"].as_i64().unwrap();

    // attach the auto-assign DS
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
            Some(json!({"id": ds})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["autoAssignDistributionSet"], ds);
    assert_eq!(body["autoAssignActionType"], "forced");

    // matching targets got the DS; non-matching did not
    assert_eq!(assigned_ds_id(&app, "dev-1").await, Some(ds));
    assert_eq!(assigned_ds_id(&app, "dev-2").await, Some(ds));
    assert_eq!(assigned_ds_id(&app, "prod-1").await, None);

    // GET autoAssignDS returns the DS
    let got = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(got["id"], ds);

    // DELETE autoAssignDS clears it
    let cleared = common::body_json(
        app.clone()
            .oneshot(common::req(
                "DELETE",
                &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(cleared["autoAssignDistributionSet"].is_null());
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn set_auto_assign_incomplete_ds_rejected() {
    let (app, _) = common::setup().await;
    // DS with no modules is incomplete
    let ds = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/distributionsets",
                Some(json!([{"name": "empty", "version": "1.0", "type": "os"}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let f = create_filter(&app, "beta", "controllerId==dev-*").await;
    let id = f["id"].as_i64().unwrap();
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
            Some(json!({"id": ds})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_then_match_via_ddi() {
    // anonymous DDI so an unknown controller auto-registers on poll
    let (app, _) = common::setup().await;
    let ds = complete_ds(&app).await;
    let f = create_filter(&app, "beta", "controllerId==dev-*").await;
    let id = f["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
            Some(json!({"id": ds})),
        ))
        .await
        .unwrap();

    // a brand-new target registers by polling the DDI root (no auth header needed
    // since the test config has ddi.anonymous = true)
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/DEFAULT/controller/v1/dev-99")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    // the same poll already offers a deploymentBase link from the auto-assignment
    assert!(
        body["_links"]["deploymentBase"]["href"].is_string(),
        "expected deploymentBase link, got {body}"
    );
    assert_eq!(assigned_ds_id(&app, "dev-99").await, Some(ds));
}

#[tokio::test]
async fn periodic_sweep_assigns_later_target() {
    let (app, st) = common::setup().await;
    let ds = complete_ds(&app).await;
    let f = create_filter(&app, "beta", "controllerId==dev-*").await;
    let id = f["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
            Some(json!({"id": ds})),
        ))
        .await
        .unwrap();

    // target created via mgmt API after the filter's DS was attached
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "dev-7"}])),
        ))
        .await
        .unwrap();
    assert_eq!(assigned_ds_id(&app, "dev-7").await, None);

    // the periodic sweep picks it up
    auto_assign_all(&st).await.unwrap();
    assert_eq!(assigned_ds_id(&app, "dev-7").await, Some(ds));

    // running the sweep again is a no-op (does not create a second action)
    auto_assign_all(&st).await.unwrap();
    let actions = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/dev-7/actions", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(actions["total"], 1);
}

/// A target type accepting no distribution-set type, so any assignment to a
/// target carrying it fails. Returns its id.
async fn incompatible_target_type(app: &axum::Router) -> i64 {
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targettypes",
                Some(json!([{"name": "nothing-fits", "compatibledistributionsettypes": []}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap()
}

/// #148: an earlier filter matching a target that cannot take its DS must not
/// stop auto-assignment for the filters behind it.
#[tokio::test]
async fn sweep_carries_on_past_a_target_that_cannot_take_the_ds() {
    let (app, st) = common::setup().await;
    let ds = complete_ds(&app).await;
    let tt = incompatible_target_type(&app).await;
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "a-1", "targetType": tt}])),
        ))
        .await
        .unwrap();

    let a = create_filter(&app, "a", "controllerId==a-*").await["id"]
        .as_i64()
        .unwrap();
    let b = create_filter(&app, "b", "controllerId==b-*").await["id"]
        .as_i64()
        .unwrap();
    for id in [a, b] {
        // Attaching runs the filter at once; a-1 is skipped, not a 400 for a
        // setting that has already been saved.
        let resp = app
            .clone()
            .oneshot(common::req(
                "POST",
                &format!("/rest/v1/targetfilters/{id}/autoAssignDS"),
                Some(json!({"id": ds})),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "filter {id}");
    }

    // Arrives after the filters, so only the sweep can assign it.
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "b-1"}])),
        ))
        .await
        .unwrap();

    auto_assign_all(&st).await.unwrap();
    assert_eq!(assigned_ds_id(&app, "a-1").await, None);
    assert_eq!(assigned_ds_id(&app, "b-1").await, Some(ds));
}
