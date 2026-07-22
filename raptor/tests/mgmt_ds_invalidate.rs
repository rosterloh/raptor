mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

/// Complete DS (name `stable`) + `n` targets `dev-0..dev-n`. Returns the DS id.
async fn fixture(app: &axum::Router, n: usize) -> i64 {
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
    let ds = common::body_json(
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
        .unwrap();
    for i in 0..n {
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets",
                Some(json!([{"controllerId": format!("dev-{i}")}])),
            ))
            .await
            .unwrap();
    }
    ds
}

async fn invalidate(app: &axum::Router, id: i64, body: serde_json::Value) -> StatusCode {
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/distributionsets/{id}/invalidate"),
            Some(body),
        ))
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn invalidate_sets_valid_false_and_blocks_assignment() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 1).await;

    assert_eq!(invalidate(&app, ds, json!({})).await, StatusCode::OK);

    let body = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/distributionsets/{ds}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["valid"], false);

    // assigning an invalidated set is rejected
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/dev-0/assignedDS",
            Some(json!({"id": ds})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invalidate_force_cancels_active_action_and_resets_target() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 1).await;

    // assign -> active action, target pending
    let assign = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/dev-0/assignedDS",
                Some(json!({"id": ds, "type": "forced"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let aid = assign["assignedActions"][0]["id"].as_i64().unwrap();

    assert_eq!(
        invalidate(&app, ds, json!({"actionCancelationType": "force"})).await,
        StatusCode::OK
    );

    // action closed
    let a = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/dev-0/actions/{aid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(a["status"], "finished");
    assert_eq!(a["detailStatus"], "canceled");

    // target no longer pending
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/dev-0", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["updateStatus"], "registered");

    // the cancellation is recorded in the action's status history
    let hist = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/dev-0/actions/{aid}/status?sort=id:DESC"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(hist["content"][0]["type"], "canceled");
    assert_eq!(
        hist["content"][0]["messages"][0],
        "distribution set invalidated"
    );
}

#[tokio::test]
async fn invalidate_detaches_auto_assign() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 0).await;

    let filter = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targetfilters",
                Some(json!({"name": "beta", "query": "controllerId==dev-*"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let fid = filter["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targetfilters/{fid}/autoAssignDS"),
            Some(json!({"id": ds})),
        ))
        .await
        .unwrap();

    assert_eq!(invalidate(&app, ds, json!({})).await, StatusCode::OK);

    let f = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targetfilters/{fid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(f["autoAssignDistributionSet"].is_null());
}

#[tokio::test]
async fn invalidate_cancel_rollouts_stops_running_rollout() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 3).await;

    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(json!({
                    "name": "r1",
                    "distributionSetId": ds,
                    "targetFilterQuery": "controllerId==dev-*",
                    "amountGroups": 1,
                    "successCondition": {"condition": "THRESHOLD", "expression": "100"},
                })),
            ))
            .await
            .unwrap(),
    )
    .await;
    let rid = r["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{rid}/start"),
            None,
        ))
        .await
        .unwrap();

    assert_eq!(
        invalidate(&app, ds, json!({"cancelRollouts": true})).await,
        StatusCode::OK
    );

    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{rid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["status"], "stopped");
}

#[tokio::test]
async fn invalidate_error_paths() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 0).await;

    // unknown id -> 404
    assert_eq!(
        invalidate(&app, 9999, json!({})).await,
        StatusCode::NOT_FOUND
    );

    // bad cancelation type -> 400
    assert_eq!(
        invalidate(&app, ds, json!({"actionCancelationType": "bogus"})).await,
        StatusCode::BAD_REQUEST
    );

    // first invalidation ok, second -> 409
    assert_eq!(invalidate(&app, ds, json!({})).await, StatusCode::OK);
    assert_eq!(invalidate(&app, ds, json!({})).await, StatusCode::CONFLICT);
}
