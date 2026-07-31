mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
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
async fn config_data_link_only_advertised_while_attributes_are_wanted() {
    let (app, _) = common::setup().await;
    let poll = || async {
        common::body_json(
            app.clone()
                .oneshot(ddi_get("/DEFAULT/controller/v1/attr-gate"))
                .await
                .unwrap(),
        )
        .await
    };

    // fresh registration: attributes wanted
    assert_eq!(
        poll().await["_links"]["configData"]["href"],
        "http://localhost:8080/DEFAULT/controller/v1/attr-gate/configData"
    );
    assert_eq!(
        common::body_json(
            app.clone()
                .oneshot(common::req("GET", "/rest/v1/targets/attr-gate", None))
                .await
                .unwrap()
        )
        .await["requestAttributes"],
        json!(true)
    );

    // attributes uploaded: link goes away, so the device stops re-uploading
    let resp = app
        .clone()
        .oneshot(
            Request::put("/DEFAULT/controller/v1/attr-gate/configData")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"data": {"hw": "rev2"}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(poll().await["_links"].get("configData").is_none());

    // operator re-arms it via the hawkBit requestAttributes flag
    let t = common::body_json(
        app.clone()
            .oneshot(common::req(
                "PUT",
                "/rest/v1/targets/attr-gate",
                Some(json!({"requestAttributes": true})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["requestAttributes"], json!(true));
    assert_eq!(
        poll().await["_links"]["configData"]["href"],
        "http://localhost:8080/DEFAULT/controller/v1/attr-gate/configData"
    );
}

/// A forwarded-for header is only believed when an operator has named it as
/// coming from a trusted proxy — otherwise any device could claim any address.
#[tokio::test]
async fn poll_records_address_only_from_a_trusted_proxy_header() {
    let poll_with_xff = |app: axum::Router, cid: &str| {
        let uri = format!("/DEFAULT/controller/v1/{cid}");
        async move {
            app.oneshot(
                Request::get(uri)
                    .header("x-forwarded-for", "203.0.113.9, 10.0.0.5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };
    let address_of = |app: axum::Router, cid: &str| {
        let uri = format!("/rest/v1/targets/{cid}");
        async move {
            common::body_json(app.oneshot(common::req("GET", &uri, None)).await.unwrap()).await
        }
    };

    // untrusted (default): header ignored, and oneshot has no socket peer
    let (app, state) = common::setup().await;
    poll_with_xff(app.clone(), "untrusted-dev").await;
    let t = address_of(app.clone(), "untrusted-dev").await;
    assert_eq!(t["address"], json!(null));
    assert_eq!(t["ipAddress"], json!(null));

    // trusted: the proxy-appended (rightmost) hop is recorded, not the
    // device-supplied one to its left
    let mut cfg = state.cfg.clone();
    cfg.ddi.trusted_proxy_header = Some("x-forwarded-for".into());
    let trusted = raptor::app::build_app(raptor::state::AppState::new(
        state.db.clone(),
        cfg,
        state.store.clone(),
    ));
    poll_with_xff(trusted.clone(), "proxied-dev").await;
    let t = address_of(trusted.clone(), "proxied-dev").await;
    assert_eq!(t["address"], json!("10.0.0.5"));
    assert_eq!(t["ipAddress"], json!("10.0.0.5"));
}

#[tokio::test]
async fn poll_with_foreign_tenant_still_serves_default_links() {
    let (app, _) = common::setup().await;
    let body = common::body_json(
        app.clone()
            .oneshot(ddi_get("/OTHER/controller/v1/wrong-tenant"))
            .await
            .unwrap(),
    )
    .await;
    // accepted (single-tenant server), but every emitted link says DEFAULT
    assert_eq!(
        body["_links"]["configData"]["href"],
        "http://localhost:8080/DEFAULT/controller/v1/wrong-tenant/configData"
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
