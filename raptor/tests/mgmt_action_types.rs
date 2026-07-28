//! The four hawkBit assignment types (forced/soft/timeforced/downloadonly),
//! how each renders in `deploymentBase`, the force-escalation endpoint, and the
//! downloadonly completion path.

mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

/// os module + complete DS `stable:1.0` + target `d1`. Returns the DS id.
async fn ds_fixture(app: &axum::Router) -> i64 {
    let sm = common::body_json(
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
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "d1"}])),
        ))
        .await
        .unwrap();
    ds
}

async fn assign(app: &axum::Router, body: serde_json::Value) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/d1/assignedDS",
                Some(body),
            ))
            .await
            .unwrap(),
    )
    .await
}

/// `deployment.download` / `deployment.update` as the device sees them.
async fn ddi_modes(app: &axum::Router, action_id: i64) -> (String, String) {
    let body = common::body_json(
        app.clone()
            .oneshot(
                Request::get(format!(
                    "/DEFAULT/controller/v1/d1/deploymentBase/{action_id}"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let d = &body["deployment"];
    (
        d["download"].as_str().unwrap().to_string(),
        d["update"].as_str().unwrap().to_string(),
    )
}

async fn action(app: &axum::Router, action_id: i64) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/d1/actions/{action_id}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn each_action_type_renders_its_own_handling_modes() {
    // forced and soft: both fields track the type
    for (assign_type, want) in [
        ("forced", ("forced", "forced")),
        ("soft", ("attempt", "attempt")),
    ] {
        let (app, _) = common::setup().await;
        let ds = ds_fixture(&app).await;
        let r = assign(&app, json!({"id": ds, "type": assign_type})).await;
        let aid = r["assignedActions"][0]["id"].as_i64().unwrap();
        let (download, update) = ddi_modes(&app, aid).await;
        assert_eq!(
            (download.as_str(), update.as_str()),
            want,
            "type {assign_type}"
        );
        assert_eq!(action(&app, aid).await["forceType"], json!(assign_type));
    }

    // downloadonly: fetch the bytes, skip the install
    let (app, _) = common::setup().await;
    let ds = ds_fixture(&app).await;
    let r = assign(&app, json!({"id": ds, "type": "downloadonly"})).await;
    let aid = r["assignedActions"][0]["id"].as_i64().unwrap();
    assert_eq!(ddi_modes(&app, aid).await, ("forced".into(), "skip".into()));
    assert_eq!(action(&app, aid).await["forceType"], json!("downloadonly"));
}

#[tokio::test]
async fn timeforced_is_soft_until_its_deadline_then_forced() {
    let now = chrono::Utc::now().timestamp_millis();

    // deadline in the future: still soft
    let (app, _) = common::setup().await;
    let ds = ds_fixture(&app).await;
    let r = assign(
        &app,
        json!({"id": ds, "type": "timeforced", "forcetime": now + 3_600_000}),
    )
    .await;
    let aid = r["assignedActions"][0]["id"].as_i64().unwrap();
    assert_eq!(
        ddi_modes(&app, aid).await,
        ("attempt".into(), "attempt".into())
    );
    let a = action(&app, aid).await;
    assert_eq!(a["forceType"], json!("timeforced"));
    assert_eq!(a["forceTime"], json!(now + 3_600_000));

    // deadline already passed: forced, and the mgmt view collapses to `forced`
    // too so operator and device agree
    let (app, _) = common::setup().await;
    let ds = ds_fixture(&app).await;
    let r = assign(
        &app,
        json!({"id": ds, "type": "timeforced", "forcetime": now - 1_000}),
    )
    .await;
    let aid = r["assignedActions"][0]["id"].as_i64().unwrap();
    assert_eq!(
        ddi_modes(&app, aid).await,
        ("forced".into(), "forced".into())
    );
    assert_eq!(action(&app, aid).await["forceType"], json!("forced"));

    // no forcetime at all behaves as already-reached (hawkBit's default of 0)
    let (app, _) = common::setup().await;
    let ds = ds_fixture(&app).await;
    let r = assign(&app, json!({"id": ds, "type": "timeforced"})).await;
    let aid = r["assignedActions"][0]["id"].as_i64().unwrap();
    assert_eq!(
        ddi_modes(&app, aid).await,
        ("forced".into(), "forced".into())
    );
}

#[tokio::test]
async fn downloaded_feedback_closes_only_downloadonly_actions() {
    let feedback = |aid: i64, execution: &str| {
        Request::post(format!(
            "/DEFAULT/controller/v1/d1/deploymentBase/{aid}/feedback"
        ))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"status": {"execution": execution, "result": {"finished": "none"}}}).to_string(),
        ))
        .unwrap()
    };

    // downloadonly: `downloaded` completes the action, but nothing was installed
    // so installedDS stays empty
    let (app, _) = common::setup().await;
    let ds = ds_fixture(&app).await;
    let r = assign(&app, json!({"id": ds, "type": "downloadonly"})).await;
    let aid = r["assignedActions"][0]["id"].as_i64().unwrap();
    let resp = app
        .clone()
        .oneshot(feedback(aid, "downloaded"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let a = action(&app, aid).await;
    assert_eq!(a["status"], json!("finished"));
    assert_eq!(a["detailStatus"], json!("downloaded"));
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["updateStatus"], json!("in_sync"));
    let resp = app
        .clone()
        .oneshot(common::req("GET", "/rest/v1/targets/d1/installedDS", None))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "downloadonly must not set an installed DS"
    );
    // no deploymentBase link left to poll
    let poll = common::body_json(
        app.clone()
            .oneshot(
                Request::get("/DEFAULT/controller/v1/d1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(poll["_links"].get("deploymentBase").is_none());

    // a forced action treats `downloaded` as progress only, exactly as before
    let (app, _) = common::setup().await;
    let ds = ds_fixture(&app).await;
    let r = assign(&app, json!({"id": ds, "type": "forced"})).await;
    let aid = r["assignedActions"][0]["id"].as_i64().unwrap();
    app.clone()
        .oneshot(feedback(aid, "downloaded"))
        .await
        .unwrap();
    let a = action(&app, aid).await;
    assert_eq!(a["status"], json!("pending"));
    assert_eq!(a["detailStatus"], json!("running"));
}

#[tokio::test]
async fn force_type_can_be_escalated_on_a_running_action() {
    let (app, _) = common::setup().await;
    let ds = ds_fixture(&app).await;
    let r = assign(&app, json!({"id": ds, "type": "soft"})).await;
    let aid = r["assignedActions"][0]["id"].as_i64().unwrap();
    assert_eq!(
        ddi_modes(&app, aid).await,
        ("attempt".into(), "attempt".into())
    );

    let updated = common::body_json(
        app.clone()
            .oneshot(common::req(
                "PUT",
                &format!("/rest/v1/targets/d1/actions/{aid}"),
                Some(json!({"forceType": "forced"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(updated["forceType"], json!("forced"));
    assert_eq!(
        ddi_modes(&app, aid).await,
        ("forced".into(), "forced".into())
    );

    // the escalation is recorded in the action's status history
    let hist = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/d1/actions/{aid}/status"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let types: Vec<&str> = hist["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["type"].as_str().unwrap())
        .collect();
    assert!(types.contains(&"forced"), "history was {types:?}");
}

#[tokio::test]
async fn escalating_a_finished_action_is_gone_and_bad_types_are_rejected() {
    let (app, _) = common::setup().await;
    let ds = ds_fixture(&app).await;

    // unknown assignment type -> 400, and no action is created
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/d1/assignedDS",
            Some(json!({"id": ds, "type": "eventually"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let r = assign(&app, json!({"id": ds, "type": "soft"})).await;
    let aid = r["assignedActions"][0]["id"].as_i64().unwrap();

    // unknown force type on PUT -> 400
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            &format!("/rest/v1/targets/d1/actions/{aid}"),
            Some(json!({"forceType": "very"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // force-quit closes it, after which escalation is Gone
    app.clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/targets/d1/actions/{aid}?force=true"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(action(&app, aid).await["detailStatus"], json!("canceled"));
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            &format!("/rest/v1/targets/d1/actions/{aid}"),
            Some(json!({"forceType": "forced"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::GONE);
}
