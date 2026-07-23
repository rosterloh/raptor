mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

/// Create a software module and return its id.
async fn make_module(app: &axum::Router) -> i64 {
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/softwaremodules",
                Some(json!([{"name": "fw", "version": "1.0", "type": "os"}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap()
}

/// Create a distribution set and return its id.
async fn make_ds(app: &axum::Router, sm: i64) -> i64 {
    common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/distributionsets",
        Some(json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}])))).await.unwrap()).await[0]["id"].as_i64().unwrap()
}

#[tokio::test]
async fn target_metadata_crud() {
    let (app, _) = common::setup().await;
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "d1"}])),
        ))
        .await
        .unwrap();

    // empty list
    let resp = app
        .clone()
        .oneshot(common::req("GET", "/rest/v1/targets/d1/metadata", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 0);
    assert!(body["content"].as_array().unwrap().is_empty());

    // create two
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/d1/metadata",
            Some(json!([{"key": "region", "value": "eu"}, {"key": "tier", "value": "gold"}])),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = common::body_json(resp).await;
    assert_eq!(body[0]["key"], "region");
    assert_eq!(body[0]["value"], "eu");
    // no targetVisible on target metadata
    assert!(body[0].get("targetVisible").is_none());

    // list ordered by key
    let body = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1/metadata", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["total"], 2);
    assert_eq!(body["content"][0]["key"], "region");
    assert_eq!(body["content"][1]["key"], "tier");

    // get one
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            "/rest/v1/targets/d1/metadata/region",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["value"], "eu");

    // unknown key -> 404
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "GET",
                "/rest/v1/targets/d1/metadata/nope",
                None
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    // duplicate create -> 409
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/d1/metadata",
                Some(json!([{"key": "region", "value": "us"}])),
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );

    // update
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            "/rest/v1/targets/d1/metadata/region",
            Some(json!({"value": "us"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["value"], "us");

    // update unknown key -> 404
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "PUT",
                "/rest/v1/targets/d1/metadata/nope",
                Some(json!({"value": "x"})),
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    // delete
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "DELETE",
                "/rest/v1/targets/d1/metadata/region",
                None,
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "GET",
                "/rest/v1/targets/d1/metadata/region",
                None,
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    // delete unknown -> 404
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "DELETE",
                "/rest/v1/targets/d1/metadata/nope",
                None,
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    // metadata on unknown target -> 404
    assert_eq!(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/ghost/metadata", None))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn ds_metadata_crud() {
    let (app, _) = common::setup().await;
    let sm = make_module(&app).await;
    let ds = make_ds(&app, sm).await;

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/distributionsets/{ds}/metadata"),
            Some(json!([{"key": "channel", "value": "beta"}])),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(common::body_json(resp).await[0]
        .get("targetVisible")
        .is_none());

    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/distributionsets/{ds}/metadata/channel"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["value"], "beta");

    // metadata on unknown DS -> 404
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "GET",
                "/rest/v1/distributionsets/9999/metadata",
                None,
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn sm_metadata_target_visible_flag() {
    let (app, _) = common::setup().await;
    let sm = make_module(&app).await;

    // create one visible, one not
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/softwaremodules/{sm}/metadata"),
            Some(json!([
                {"key": "sha", "value": "abc", "targetVisible": true},
                {"key": "buildhost", "value": "ci-7"}
            ])),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = common::body_json(resp).await;
    assert_eq!(body[0]["key"], "sha");
    assert_eq!(body[0]["targetVisible"], true);
    // defaults to false when omitted
    assert_eq!(body[1]["key"], "buildhost");
    assert_eq!(body[1]["targetVisible"], false);

    // update value + flip targetVisible
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            &format!("/rest/v1/softwaremodules/{sm}/metadata/buildhost"),
            Some(json!({"value": "ci-8", "targetVisible": true})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["value"], "ci-8");
    assert_eq!(body["targetVisible"], true);

    // duplicate within request -> 409
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "POST",
                &format!("/rest/v1/softwaremodules/{sm}/metadata"),
                Some(json!([{"key": "dup", "value": "1"}, {"key": "dup", "value": "2"}])),
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn target_visible_sm_metadata_in_deployment_chunk() {
    let (app, _) = common::setup().await;

    // module + artifact
    let sm = make_module(&app).await;
    // visible + hidden metadata
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/softwaremodules/{sm}/metadata"),
            Some(json!([
                {"key": "signature", "value": "sig-1", "targetVisible": true},
                {"key": "internal", "value": "secret"}
            ])),
        ))
        .await
        .unwrap();

    let ds = make_ds(&app, sm).await;
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "d1"}])),
        ))
        .await
        .unwrap();
    let assign = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/d1/assignedDS",
                Some(json!({"id": ds, "type": "forced"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let action_id = assign["assignedActions"][0]["id"].as_i64().unwrap();

    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::get(format!(
                "/DEFAULT/controller/v1/d1/deploymentBase/{action_id}"
            ))
            .body(axum::body::Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    let chunk = &body["deployment"]["chunks"][0];
    let md = chunk["metadata"].as_array().unwrap();
    // only the targetVisible entry appears
    assert_eq!(md.len(), 1);
    assert_eq!(md[0]["key"], "signature");
    assert_eq!(md[0]["value"], "sig-1");
    // and it is not wrapped with targetVisible in the DDI shape
    assert!(md[0].get("targetVisible").is_none());
}

#[tokio::test]
async fn no_metadata_means_no_chunk_metadata_key() {
    let (app, _) = common::setup().await;
    let sm = make_module(&app).await;
    let ds = make_ds(&app, sm).await;
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "d1"}])),
        ))
        .await
        .unwrap();
    let assign = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/d1/assignedDS",
                Some(json!({"id": ds, "type": "forced"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let action_id = assign["assignedActions"][0]["id"].as_i64().unwrap();

    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::get(format!(
                "/DEFAULT/controller/v1/d1/deploymentBase/{action_id}"
            ))
            .body(axum::body::Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    // absent, not an empty array (hawkBit omits the key when empty)
    assert!(body["deployment"]["chunks"][0].get("metadata").is_none());
}
