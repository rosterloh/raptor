use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use tower::ServiceExt;

async fn fixture(app: &axum::Router) -> i64 {
    let sm = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/softwaremodules",
                Some(json!([{"name": "fw", "version": "1", "type": "os"}])),
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
                Some(json!([{"name": "r", "version": "1", "type": "os", "modules": [{"id": sm}]}])),
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
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/d1/assignedDS",
                Some(json!({"id": ds})),
            ))
            .await
            .unwrap(),
    )
    .await;
    r["assignedActions"][0]["id"].as_i64().unwrap()
}

fn feedback(action_id: i64, kind: &str, execution: &str, finished: &str) -> Request<Body> {
    let uri = format!("/DEFAULT/controller/v1/d1/{kind}/{action_id}/feedback");
    let body = json!({
        "id": action_id.to_string(),
        "time": "20260704T120000",
        "status": {"execution": execution, "result": {"finished": finished}, "details": ["msg"]}
    });
    Request::post(&uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn success_feedback_finishes_action_and_syncs_target() {
    let (app, _) = common::setup().await;
    let aid = fixture(&app).await;

    // progress feedback keeps it running
    assert_eq!(
        app.clone()
            .oneshot(feedback(aid, "deploymentBase", "proceeding", "none"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["updateStatus"], "pending");

    // closed/success finishes
    assert_eq!(
        app.clone()
            .oneshot(feedback(aid, "deploymentBase", "closed", "success"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["updateStatus"], "in_sync");
    let a = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/d1/actions/{aid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(a["detailStatus"], "finished");
    assert_eq!(a["status"], "finished");

    // installedDS now reports the DS
    let resp = app
        .clone()
        .oneshot(common::req("GET", "/rest/v1/targets/d1/installedDS", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // further feedback on closed action -> 410
    assert_eq!(
        app.clone()
            .oneshot(feedback(aid, "deploymentBase", "proceeding", "none"))
            .await
            .unwrap()
            .status(),
        StatusCode::GONE
    );
}

#[tokio::test]
async fn failure_feedback_errors_target() {
    let (app, _) = common::setup().await;
    let aid = fixture(&app).await;
    assert_eq!(
        app.clone()
            .oneshot(feedback(aid, "deploymentBase", "closed", "failure"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["updateStatus"], "error");
    let a = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/d1/actions/{aid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(a["detailStatus"], "error");
}

#[tokio::test]
async fn cancel_handshake() {
    let (app, _) = common::setup().await;
    let aid = fixture(&app).await;

    // cancelAction 404 while running
    let resp = app
        .clone()
        .oneshot(
            Request::get(format!("/DEFAULT/controller/v1/d1/cancelAction/{aid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // operator soft-cancels
    app.clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/targets/d1/actions/{aid}"),
            None,
        ))
        .await
        .unwrap();

    // device fetches cancel payload
    let resp = app
        .clone()
        .oneshot(
            Request::get(format!("/DEFAULT/controller/v1/d1/cancelAction/{aid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["id"], aid.to_string());
    assert_eq!(body["cancelAction"]["stopId"], aid.to_string());

    // device confirms
    assert_eq!(
        app.clone()
            .oneshot(feedback(aid, "cancelAction", "closed", "success"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let a = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/d1/actions/{aid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(a["detailStatus"], "canceled");
    assert_eq!(a["status"], "finished");
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["updateStatus"], "registered"); // nothing installed yet
}

fn feedback_with_code(
    action_id: i64,
    execution: &str,
    finished: &str,
    code: Option<i32>,
) -> Request<Body> {
    let mut status = json!({"execution": execution, "result": {"finished": finished}});
    if let Some(c) = code {
        status["code"] = json!(c);
    }
    Request::post(format!(
        "/DEFAULT/controller/v1/d1/deploymentBase/{action_id}/feedback"
    ))
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(json!({ "status": status }).to_string()))
    .unwrap()
}

/// `status.code` lands on the status entry and as the action's
/// `lastStatusCode`, which each report replaces, even with none (#216).
#[tokio::test]
async fn feedback_status_code_is_recorded_and_filterable() {
    let (app, _) = common::setup().await;
    let aid = fixture(&app).await;
    let get = |uri: String| {
        let app = app.clone();
        async move {
            common::body_json(app.oneshot(common::req("GET", &uri, None)).await.unwrap()).await
        }
    };
    let action = || get(format!("/rest/v1/targets/d1/actions/{aid}"));
    let send = |execution: &'static str, finished: &'static str, code| {
        let app = app.clone();
        async move {
            app.oneshot(feedback_with_code(aid, execution, finished, code))
                .await
                .unwrap()
                .status()
        }
    };

    assert_eq!(send("proceeding", "none", Some(1234)).await, StatusCode::OK);
    assert_eq!(action().await["lastStatusCode"], 1234);

    // A report without a code clears it, as hawkBit does.
    assert_eq!(send("proceeding", "none", None).await, StatusCode::OK);
    assert!(action().await.get("lastStatusCode").is_none());

    assert_eq!(send("closed", "failure", Some(42)).await, StatusCode::OK);
    assert_eq!(action().await["lastStatusCode"], 42);

    let history = get(format!("/rest/v1/targets/d1/actions/{aid}/status")).await;
    let codes: Vec<_> = history["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.get("code").cloned())
        .collect();
    // assignment ("running"), then the three reports
    assert_eq!(codes, vec![None, Some(json!(1234)), None, Some(json!(42))]);

    assert_eq!(
        get("/rest/v1/actions?q=lastStatusCode==42".into()).await["total"],
        1
    );
    assert_eq!(
        get("/rest/v1/actions?q=lastStatusCode==1234".into()).await["total"],
        0
    );
}

#[tokio::test]
async fn cancel_feedback_records_status_code() {
    let (app, _) = common::setup().await;
    let aid = fixture(&app).await;
    app.clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/targets/d1/actions/{aid}"),
            None,
        ))
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(
            Request::post(format!("/DEFAULT/controller/v1/d1/cancelAction/{aid}/feedback"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"status": {"execution": "rejected", "result": {"finished": "none"}, "code": 7}})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let a = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/d1/actions/{aid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(a["lastStatusCode"], 7);
}
