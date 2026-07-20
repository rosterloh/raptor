mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use raptor::entity::action_status;
use raptor::state::AppState;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use tower::ServiceExt;

/// setup() but with the DDI confirmation flow enabled.
async fn setup_confirm() -> (axum::Router, AppState) {
    let (_, state) = common::setup().await;
    let mut cfg = state.cfg.clone();
    cfg.ddi.confirmation_flow = true;
    let state = AppState::new(state.db.clone(), cfg, state.store.clone());
    (raptor::app::build_app(state.clone()), state)
}

/// Creates os module + complete DS + target `d1`, assigns the DS, returns action id.
async fn assign_fixture(app: &axum::Router) -> i64 {
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

fn ddi_get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn ddi_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(uri)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn poll_links(app: &axum::Router) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(ddi_get("/DEFAULT/controller/v1/d1"))
            .await
            .unwrap(),
    )
    .await["_links"]
        .clone()
}

async fn detail_status(app: &axum::Router, action_id: i64) -> String {
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
    .await["detailStatus"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn flow_enabled_waits_then_confirms_to_deployment() {
    let (app, _) = setup_confirm().await;
    let action_id = assign_fixture(&app).await;

    // poll offers confirmationBase, not deploymentBase
    let links = poll_links(&app).await;
    assert_eq!(
        links["confirmationBase"]["href"],
        format!("http://localhost:8080/DEFAULT/controller/v1/d1/confirmationBase/{action_id}")
    );
    assert!(links.get("deploymentBase").is_none());
    assert_eq!(
        detail_status(&app, action_id).await,
        "wait_for_confirmation"
    );

    // confirmationBase renders the DS under a `confirmation` key
    let cb = common::body_json(
        app.clone()
            .oneshot(ddi_get(&format!(
                "/DEFAULT/controller/v1/d1/confirmationBase/{action_id}"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(cb["id"], action_id.to_string());
    assert_eq!(cb["confirmation"]["chunks"][0]["part"], "os");
    assert_eq!(cb["confirmation"]["chunks"][0]["name"], "fw");
    // deploymentBase must 404 while still waiting
    let resp = app
        .clone()
        .oneshot(ddi_get(&format!(
            "/DEFAULT/controller/v1/d1/deploymentBase/{action_id}"
        )))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // confirm
    let resp = app
        .clone()
        .oneshot(ddi_post(
            &format!("/DEFAULT/controller/v1/d1/confirmationBase/{action_id}/feedback"),
            json!({"confirmation": "confirmed", "details": ["ok"]}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // now poll offers deploymentBase and the action is running
    let links = poll_links(&app).await;
    assert_eq!(
        links["deploymentBase"]["href"],
        format!("http://localhost:8080/DEFAULT/controller/v1/d1/deploymentBase/{action_id}")
    );
    assert!(links.get("confirmationBase").is_none());
    assert_eq!(detail_status(&app, action_id).await, "running");
    let resp = app
        .clone()
        .oneshot(ddi_get(&format!(
            "/DEFAULT/controller/v1/d1/deploymentBase/{action_id}"
        )))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn denied_keeps_action_waiting_with_history() {
    let (app, state) = setup_confirm().await;
    let action_id = assign_fixture(&app).await;

    let resp = app
        .clone()
        .oneshot(ddi_post(
            &format!("/DEFAULT/controller/v1/d1/confirmationBase/{action_id}/feedback"),
            json!({"confirmation": "denied", "details": ["not now"]}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // still waiting; poll still offers confirmationBase
    assert_eq!(
        detail_status(&app, action_id).await,
        "wait_for_confirmation"
    );
    let links = poll_links(&app).await;
    assert!(links.get("confirmationBase").is_some());
    assert!(links.get("deploymentBase").is_none());

    // a denied ActionStatus row was recorded
    let statuses = action_status::Entity::find()
        .filter(action_status::Column::ActionId.eq(action_id))
        .all(&state.db)
        .await
        .unwrap();
    assert!(statuses.iter().any(|s| s.status == "denied"));
}

#[tokio::test]
async fn auto_confirm_skips_wait_state() {
    let (app, _) = setup_confirm().await;
    // register d1 then activate auto-confirm before assigning
    app.clone()
        .oneshot(ddi_get("/DEFAULT/controller/v1/d1"))
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/d1/autoConfirm/activate",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let action_id = assign_fixture(&app).await;
    // straight to running: deploymentBase, no confirmationBase
    let links = poll_links(&app).await;
    assert!(links.get("deploymentBase").is_some());
    assert!(links.get("confirmationBase").is_none());
    assert_eq!(detail_status(&app, action_id).await, "running");
}

#[tokio::test]
async fn activating_auto_confirm_releases_pending_action() {
    let (app, _) = setup_confirm().await;
    let action_id = assign_fixture(&app).await;
    assert_eq!(
        detail_status(&app, action_id).await,
        "wait_for_confirmation"
    );

    // status endpoint reflects inactive, then active
    let st = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1/autoConfirm", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(st["active"], false);

    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/d1/autoConfirm/activate",
            None,
        ))
        .await
        .unwrap();

    // the previously-waiting action is now running
    assert_eq!(detail_status(&app, action_id).await, "running");
    assert!(poll_links(&app).await.get("deploymentBase").is_some());
    let st = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1/autoConfirm", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(st["active"], true);
}

#[tokio::test]
async fn ddi_activate_auto_confirm_endpoint() {
    let (app, _) = setup_confirm().await;
    let action_id = assign_fixture(&app).await;

    // device activates auto-confirm over DDI
    let resp = app
        .clone()
        .oneshot(ddi_post(
            "/DEFAULT/controller/v1/d1/confirmationBase/activateAutoConfirm",
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(detail_status(&app, action_id).await, "running");
}

#[tokio::test]
async fn flow_disabled_goes_straight_to_running() {
    // default setup: confirmation_flow off
    let (app, _) = common::setup().await;
    let action_id = assign_fixture(&app).await;
    let links = poll_links(&app).await;
    assert!(links.get("deploymentBase").is_some());
    assert!(links.get("confirmationBase").is_none());
    assert_eq!(detail_status(&app, action_id).await, "running");
}
