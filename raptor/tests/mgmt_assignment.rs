mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

async fn fixture(app: &axum::Router) -> (String, i64) {
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
    let ds = common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/distributionsets",
        Some(json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}])))).await.unwrap()).await[0]["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "dev-1"}])),
        ))
        .await
        .unwrap();
    ("dev-1".to_string(), ds)
}

#[tokio::test]
async fn assign_creates_action_and_marks_pending() {
    let (app, _) = common::setup().await;
    let (cid, ds) = fixture(&app).await;

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targets/{cid}/assignedDS"),
            Some(json!({"id": ds, "type": "forced"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["assigned"], 1);
    assert_eq!(body["alreadyAssigned"], 0);
    assert_eq!(body["total"], 1);
    let action_id = body["assignedActions"][0]["id"].as_i64().unwrap();

    // target now pending
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", &format!("/rest/v1/targets/{cid}"), None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["updateStatus"], "pending");

    // action visible, pending, type update
    let a = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/{cid}/actions/{action_id}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(a["status"], "pending");
    assert_eq!(a["detailStatus"], "running");
    assert_eq!(a["type"], "update");
    assert_eq!(a["forceType"], "forced");

    // assignedDS reflects it
    let ds_json = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/{cid}/assignedDS"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(ds_json["id"], ds);

    // re-assign same DS -> alreadyAssigned
    let body = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                &format!("/rest/v1/targets/{cid}/assignedDS"),
                Some(json!({"id": ds})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["alreadyAssigned"], 1);
    assert_eq!(body["assigned"], 0);
}

#[tokio::test]
async fn new_assignment_supersedes_active_action() {
    let (app, _) = common::setup().await;
    let (cid, ds1) = fixture(&app).await;
    let ds2 = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/distributionsets",
                Some(json!([{"name": "next", "version": "2.0", "type": "app", "modules": []}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64();
    // ds2 has no modules -> incomplete -> assignment must 400
    let ds2 = ds2.unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targets/{cid}/assignedDS"),
            Some(json!({"id": ds1})),
        ))
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targets/{cid}/assignedDS"),
            Some(json!({"id": ds2})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // complete ds2, then supersede
    let sm2 = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/softwaremodules",
                Some(json!([{"name": "app", "version": "1", "type": "application"}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/distributionsets/{ds2}/assignedSM"),
            Some(json!([{"id": sm2}])),
        ))
        .await
        .unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targets/{cid}/assignedDS"),
            Some(json!({"id": ds2})),
        ))
        .await
        .unwrap();

    // exactly one active (pending) action remains
    let actions = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/{cid}/actions?q=active==true"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(actions["total"], 1);
    // the superseded one is canceled
    let all = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/{cid}/actions?sort=id:ASC"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(all["content"][0]["detailStatus"], "canceled");
}

#[tokio::test]
async fn cancel_action_soft_and_forced() {
    let (app, _) = common::setup().await;
    let (cid, ds) = fixture(&app).await;
    let body = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                &format!("/rest/v1/targets/{cid}/assignedDS"),
                Some(json!({"id": ds})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let aid = body["assignedActions"][0]["id"].as_i64().unwrap();

    // soft cancel -> canceling, still pending
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "DELETE",
                &format!("/rest/v1/targets/{cid}/actions/{aid}"),
                None
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let a = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/{cid}/actions/{aid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(a["detailStatus"], "canceling");
    assert_eq!(a["type"], "cancel");

    // force cancel -> canceled, inactive
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "DELETE",
                &format!("/rest/v1/targets/{cid}/actions/{aid}?force=true"),
                None
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let a = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/{cid}/actions/{aid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(a["detailStatus"], "canceled");
    assert_eq!(a["status"], "finished");

    // canceling an inactive action -> 410
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "DELETE",
                &format!("/rest/v1/targets/{cid}/actions/{aid}"),
                None
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::GONE
    );
}

#[tokio::test]
async fn action_status_history_lists_entries_with_messages() {
    let (app, _) = common::setup().await;
    let (cid, ds) = fixture(&app).await;

    // assign -> creates the action and its initial status row
    let assign = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                &format!("/rest/v1/targets/{cid}/assignedDS"),
                Some(json!({"id": ds, "type": "forced"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let aid = assign["assignedActions"][0]["id"].as_i64().unwrap();

    // one entry so far, hawkBit-shaped
    let hist = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/{cid}/actions/{aid}/status"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(hist["total"], 1);
    assert!(hist["content"][0]["type"].is_string());
    assert!(hist["content"][0]["reportedAt"].is_i64());
    assert!(hist["content"][0]["messages"].is_array());

    // force-cancel appends a "canceled" status carrying a message
    assert_eq!(
        app.clone()
            .oneshot(common::req(
                "DELETE",
                &format!("/rest/v1/targets/{cid}/actions/{aid}?force=true"),
                None,
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );

    // newest-first via sort: the canceled entry with its message on top
    let hist2 = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/{cid}/actions/{aid}/status?sort=id:DESC"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(hist2["total"], 2);
    assert_eq!(hist2["content"][0]["type"], "canceled");
    assert_eq!(
        hist2["content"][0]["messages"][0],
        "force canceled by operator"
    );
}

#[tokio::test]
async fn action_status_history_unknown_action_404() {
    let (app, _) = common::setup().await;
    let (cid, _) = fixture(&app).await;
    assert_eq!(
        app.oneshot(common::req(
            "GET",
            &format!("/rest/v1/targets/{cid}/actions/999/status"),
            None,
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::NOT_FOUND
    );
}
