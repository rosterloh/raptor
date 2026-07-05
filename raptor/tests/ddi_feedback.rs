mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

async fn fixture(app: &axum::Router) -> i64 {
    let sm = common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/softwaremodules",
        Some(json!([{"name": "fw", "version": "1", "type": "os"}])))).await.unwrap()).await[0]["id"].as_i64().unwrap();
    let ds = common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/distributionsets",
        Some(json!([{"name": "r", "version": "1", "type": "os", "modules": [{"id": sm}]}])))).await.unwrap()).await[0]["id"].as_i64().unwrap();
    app.clone().oneshot(common::req("POST", "/rest/v1/targets", Some(json!([{"controllerId": "d1"}])))).await.unwrap();
    let r = common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/targets/d1/assignedDS",
        Some(json!({"id": ds})))).await.unwrap()).await;
    r["assignedActions"][0]["id"].as_i64().unwrap()
}

fn feedback(action_id: i64, kind: &str, execution: &str, finished: &str) -> Request<Body> {
    let uri = format!("/DEFAULT/controller/v1/d1/{kind}/{action_id}/feedback");
    let body = json!({
        "id": action_id.to_string(),
        "time": "20260704T120000",
        "status": {"execution": execution, "result": {"finished": finished}, "details": ["msg"]}
    });
    Request::post(&uri).header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn success_feedback_finishes_action_and_syncs_target() {
    let (app, _) = common::setup().await;
    let aid = fixture(&app).await;

    // progress feedback keeps it running
    assert_eq!(app.clone().oneshot(feedback(aid, "deploymentBase", "proceeding", "none")).await.unwrap().status(), StatusCode::OK);
    let t = common::body_json(app.clone().oneshot(common::req("GET", "/rest/v1/targets/d1", None)).await.unwrap()).await;
    assert_eq!(t["updateStatus"], "pending");

    // closed/success finishes
    assert_eq!(app.clone().oneshot(feedback(aid, "deploymentBase", "closed", "success")).await.unwrap().status(), StatusCode::OK);
    let t = common::body_json(app.clone().oneshot(common::req("GET", "/rest/v1/targets/d1", None)).await.unwrap()).await;
    assert_eq!(t["updateStatus"], "in_sync");
    let a = common::body_json(app.clone().oneshot(common::req("GET", &format!("/rest/v1/targets/d1/actions/{aid}"), None)).await.unwrap()).await;
    assert_eq!(a["detailStatus"], "finished");
    assert_eq!(a["status"], "finished");

    // installedDS now reports the DS
    let resp = app.clone().oneshot(common::req("GET", "/rest/v1/targets/d1/installedDS", None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // further feedback on closed action -> 410
    assert_eq!(app.clone().oneshot(feedback(aid, "deploymentBase", "proceeding", "none")).await.unwrap().status(), StatusCode::GONE);
}

#[tokio::test]
async fn failure_feedback_errors_target() {
    let (app, _) = common::setup().await;
    let aid = fixture(&app).await;
    assert_eq!(app.clone().oneshot(feedback(aid, "deploymentBase", "closed", "failure")).await.unwrap().status(), StatusCode::OK);
    let t = common::body_json(app.clone().oneshot(common::req("GET", "/rest/v1/targets/d1", None)).await.unwrap()).await;
    assert_eq!(t["updateStatus"], "error");
    let a = common::body_json(app.clone().oneshot(common::req("GET", &format!("/rest/v1/targets/d1/actions/{aid}"), None)).await.unwrap()).await;
    assert_eq!(a["detailStatus"], "error");
}

#[tokio::test]
async fn cancel_handshake() {
    let (app, _) = common::setup().await;
    let aid = fixture(&app).await;

    // cancelAction 404 while running
    let resp = app.clone().oneshot(Request::get(&format!("/DEFAULT/controller/v1/d1/cancelAction/{aid}")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // operator soft-cancels
    app.clone().oneshot(common::req("DELETE", &format!("/rest/v1/targets/d1/actions/{aid}"), None)).await.unwrap();

    // device fetches cancel payload
    let resp = app.clone().oneshot(Request::get(&format!("/DEFAULT/controller/v1/d1/cancelAction/{aid}")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["id"], aid.to_string());
    assert_eq!(body["cancelAction"]["stopId"], aid.to_string());

    // device confirms
    assert_eq!(app.clone().oneshot(feedback(aid, "cancelAction", "closed", "success")).await.unwrap().status(), StatusCode::OK);
    let a = common::body_json(app.clone().oneshot(common::req("GET", &format!("/rest/v1/targets/d1/actions/{aid}"), None)).await.unwrap()).await;
    assert_eq!(a["detailStatus"], "canceled");
    assert_eq!(a["status"], "finished");
    let t = common::body_json(app.clone().oneshot(common::req("GET", "/rest/v1/targets/d1", None)).await.unwrap()).await;
    assert_eq!(t["updateStatus"], "registered"); // nothing installed yet
}
