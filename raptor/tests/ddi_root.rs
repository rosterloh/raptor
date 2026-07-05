mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

fn ddi_get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap() // common config: anonymous=true
}

#[tokio::test]
async fn poll_auto_registers_unknown_device() {
    let (app, _) = common::setup().await;
    let resp = app
        .clone()
        .oneshot(ddi_get("/DEFAULT/controller/v1/new-dev"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["config"]["polling"]["sleep"], "00:05:00");
    assert_eq!(
        body["_links"]["configData"]["href"],
        "http://localhost:8080/DEFAULT/controller/v1/new-dev/configData"
    );
    assert!(body["_links"].get("deploymentBase").is_none());

    // registered target now visible via mgmt with pollStatus
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/new-dev", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["updateStatus"], "registered");
    assert!(t["pollStatus"]["lastRequestAt"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn poll_shows_deployment_link_when_assigned() {
    let (app, _) = common::setup().await;
    // fixture: module + ds + target + assignment via mgmt
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
                Some(
                    json!([{"name": "r1", "version": "1", "type": "os", "modules": [{"id": sm}]}]),
                ),
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
    let a = common::body_json(
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
    let action_id = a["assignedActions"][0]["id"].as_i64().unwrap();

    let body = common::body_json(
        app.clone()
            .oneshot(ddi_get("/DEFAULT/controller/v1/d1"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        body["_links"]["deploymentBase"]["href"],
        format!("http://localhost:8080/DEFAULT/controller/v1/d1/deploymentBase/{action_id}")
    );

    // soft cancel -> link flips to cancelAction
    app.clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/targets/d1/actions/{action_id}"),
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(
        app.clone()
            .oneshot(ddi_get("/DEFAULT/controller/v1/d1"))
            .await
            .unwrap(),
    )
    .await;
    assert!(body["_links"].get("deploymentBase").is_none());
    assert_eq!(
        body["_links"]["cancelAction"]["href"],
        format!("http://localhost:8080/DEFAULT/controller/v1/d1/cancelAction/{action_id}")
    );
}

#[tokio::test]
async fn config_data_modes() {
    let (app, _) = common::setup().await;
    app.clone()
        .oneshot(ddi_get("/DEFAULT/controller/v1/attr-dev"))
        .await
        .unwrap(); // register

    let put = |data: serde_json::Value| {
        Request::put("/DEFAULT/controller/v1/attr-dev/configData")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(data.to_string()))
            .unwrap()
    };

    // merge (default mode) — legacy SWUpdate-style body with extra fields must be accepted
    let resp = app.clone().oneshot(put(json!({"id": "", "time": "", "status": {"execution": "closed", "result": {"finished": "success"}}, "data": {"hw": "rev2", "os": "linux"}}))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let attrs = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                "/rest/v1/targets/attr-dev/attributes",
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(attrs, json!({"hw": "rev2", "os": "linux"}));

    // replace
    app.clone()
        .oneshot(put(json!({"mode": "replace", "data": {"only": "this"}})))
        .await
        .unwrap();
    let attrs = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                "/rest/v1/targets/attr-dev/attributes",
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(attrs, json!({"only": "this"}));

    // remove
    app.clone()
        .oneshot(put(json!({"mode": "remove", "data": {"only": ""}})))
        .await
        .unwrap();
    let attrs = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                "/rest/v1/targets/attr-dev/attributes",
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(attrs, json!({}));
}
