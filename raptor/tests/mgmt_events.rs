mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use raptor::events::{Event, Events};
use raptor::state::AppState;
use raptor_api_types::{ActionEvent, DownloadEvent, TargetEvent};
use serde_json::json;
use tower::ServiceExt;

fn target(cid: &str) -> Event {
    Event::Target(TargetEvent {
        controller_id: cid.into(),
    })
}

async fn open(app: &axum::Router, query: &str) -> axum::http::Response<Body> {
    app.clone()
        .oneshot(common::req("GET", &format!("/rest/v1/events{query}"), None))
        .await
        .unwrap()
}

async fn create_target(app: &axum::Router, cid: &str) {
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": cid}])),
        ))
        .await
        .unwrap();
}

/// Creates dev-1 with an active action; returns the action id.
async fn active_action(app: &axum::Router) -> i64 {
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
    create_target(app, "dev-1").await;
    let body = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/dev-1/assignedDS",
                Some(json!({"id": ds, "type": "forced"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    body["assignedActions"][0]["id"].as_i64().unwrap()
}

fn dl(action_id: i64, sent: u64) -> DownloadEvent {
    DownloadEvent {
        controller_id: "dev-1".into(),
        action_id,
        filename: "f.bin".into(),
        sent,
        total: 10,
    }
}

#[tokio::test]
async fn events_requires_auth() {
    let (app, _) = common::setup().await;
    let resp = app
        .oneshot(Request::get("/rest/v1/events").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn events_accepts_session_cookie() {
    let (app, _) = common::setup().await;
    let login = app
        .clone()
        .oneshot(
            Request::post("/rest/v1/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"username": "admin", "password": common::TEST_PASSWORD}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let resp = app
        .oneshot(
            Request::get("/rest/v1/events")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "text/event-stream");
}

#[tokio::test]
async fn unknown_target_filter_is_404() {
    let (app, _) = common::setup().await;
    assert_eq!(
        open(&app, "?target=nope").await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn target_filter_isolates_targets() {
    let (app, state) = common::setup().await;
    create_target(&app, "dev-1").await;
    let mut body = open(&app, "?target=dev-1").await.into_body();
    state.events.publish(target("dev-2"));
    state.events.publish(target("dev-1"));
    let (name, data) = common::sse_next(&mut body).await;
    assert_eq!(name, "target");
    assert_eq!(data, json!({"controllerId": "dev-1"}));
}

#[tokio::test]
async fn unfiltered_stream_gets_notices_not_data() {
    let (app, state) = common::setup().await;
    let mut body = open(&app, "").await.into_body();
    state.events.publish(Event::Download(dl(1, 5)));
    state.events.publish(target("dev-1"));
    assert_eq!(common::sse_next(&mut body).await.0, "target");
}

#[tokio::test]
async fn rollout_filter_matches_action_rollout_id() {
    let (app, state) = common::setup().await;
    let mut body = open(&app, "?rollout=7").await.into_body();
    let act = |rollout_id| {
        Event::Action(ActionEvent {
            controller_id: "dev-1".into(),
            action_id: 1,
            rollout_id,
        })
    };
    state.events.publish(act(None));
    state.events.publish(act(Some(7)));
    let (name, data) = common::sse_next(&mut body).await;
    assert_eq!(name, "action");
    assert_eq!(data["rolloutId"], 7);
}

#[tokio::test]
async fn snapshot_sent_on_connect() {
    let (app, state) = common::setup().await;
    let action = active_action(&app).await;
    state.events.record_download(dl(action, 4));
    let mut body = open(&app, "?target=dev-1").await.into_body();
    let (name, data) = common::sse_next(&mut body).await;
    assert_eq!(name, "download");
    assert_eq!(data["sent"], 4);
}

#[tokio::test]
async fn no_snapshot_for_inactive_action() {
    let (app, state) = common::setup().await;
    create_target(&app, "dev-1").await;
    state.events.record_download(dl(9999, 4));
    let mut body = open(&app, "?target=dev-1").await.into_body();
    common::sse_none(&mut body, 300).await;
    assert!(!state.events.action_ids().contains(&9999));
}

#[tokio::test]
async fn lagged_subscriber_gets_resync() {
    let (_, base) = common::setup().await;
    let state = AppState::with_events(
        base.db.clone(),
        base.cfg.clone(),
        base.store.clone(),
        Events::new(2),
    );
    let app = raptor::app::build_app(state.clone());
    let mut body = open(&app, "").await.into_body();
    for _ in 0..5 {
        state.events.publish(target("dev-1"));
    }
    let (name, data) = common::sse_next(&mut body).await;
    assert_eq!(name, "resync");
    assert_eq!(data, json!({}));
}

#[tokio::test]
async fn dropped_subscriber_is_released() {
    let (app, state) = common::setup().await;
    let resp = open(&app, "").await;
    assert_eq!(state.events.receiver_count(), 1);
    drop(resp);
    state.events.publish(target("dev-1"));
    assert_eq!(state.events.receiver_count(), 0);
}

#[tokio::test]
async fn stream_opened_after_shutdown_ends() {
    use http_body_util::BodyExt;
    let (app, state) = common::setup().await;
    state.events.shutdown();
    let mut body = open(&app, "").await.into_body();
    let next = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .expect("stream did not end");
    assert!(next.is_none());
}

#[tokio::test]
async fn snapshot_respects_rollout_filter() {
    let (app, state) = common::setup().await;
    let action = active_action(&app).await;
    state.events.record_download(dl(action, 4));
    let mut body = open(&app, "?target=dev-1&rollout=7").await.into_body();
    common::sse_none(&mut body, 300).await;
}

fn ddi_feedback(action_id: i64, execution: &str, finished: &str) -> Request<Body> {
    let body = json!({
        "id": action_id.to_string(),
        "time": "20260704T120000",
        "status": {"execution": execution, "result": {"finished": finished}, "details": ["msg"]}
    });
    Request::post(format!(
        "/DEFAULT/controller/v1/dev-1/deploymentBase/{action_id}/feedback"
    ))
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(body.to_string()))
    .unwrap()
}

#[tokio::test]
async fn assign_publishes_target_and_action() {
    let (app, _) = common::setup().await;
    create_target(&app, "dev-1").await;
    let mut body = open(&app, "?target=dev-1").await.into_body();
    let action = active_action(&app).await;
    let mut names = [
        common::sse_next(&mut body).await,
        common::sse_next(&mut body).await,
    ];
    names.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(names[0].0, "action");
    assert_eq!(names[0].1["actionId"], action);
    assert_eq!(names[0].1["controllerId"], "dev-1");
    assert_eq!(names[1].0, "target");
    assert_eq!(names[1].1, json!({"controllerId": "dev-1"}));
}

#[tokio::test]
async fn feedback_publishes_action_and_finish_publishes_target() {
    let (app, _) = common::setup().await;
    let action = active_action(&app).await;
    let mut body = open(&app, "?target=dev-1").await.into_body();
    let resp = app
        .clone()
        .oneshot(ddi_feedback(action, "proceeding", "none"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let (name, data) = common::sse_next(&mut body).await;
    assert_eq!(name, "action");
    assert_eq!(data["actionId"], action);

    let resp = app
        .clone()
        .oneshot(ddi_feedback(action, "closed", "success"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let mut seen = vec![];
    for _ in 0..2 {
        seen.push(common::sse_next(&mut body).await.0);
    }
    assert!(seen.contains(&"target".to_string()), "{seen:?}");
}

#[tokio::test]
async fn rollout_start_publishes_rollout() {
    let (app, _) = common::setup().await;
    let action = active_action(&app).await; // creates dev-1 + a DS
    let _ = action;
    let ds = common::body_json(open_get(&app, "/rest/v1/distributionsets").await).await["content"]
        [0]["id"]
        .as_i64()
        .unwrap();
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
    let id = r["id"].as_i64().unwrap();
    let mut body = open(&app, &format!("?rollout={id}")).await.into_body();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/start"),
            None,
        ))
        .await
        .unwrap();
    // Action events carrying this rolloutId may precede it.
    let mut found = false;
    for _ in 0..10 {
        let (name, data) = common::sse_next(&mut body).await;
        if name == "rollout" {
            assert_eq!(data["rolloutId"], id);
            found = true;
            break;
        }
    }
    assert!(found, "no rollout event");
}

async fn open_get(app: &axum::Router, uri: &str) -> axum::http::Response<Body> {
    app.clone()
        .oneshot(common::req("GET", uri, None))
        .await
        .unwrap()
}

#[tokio::test]
async fn invalidate_cancel_rollouts_publishes_rollout() {
    let (app, _) = common::setup().await;
    active_action(&app).await;
    let ds = common::body_json(open_get(&app, "/rest/v1/distributionsets").await).await["content"]
        [0]["id"]
        .as_i64()
        .unwrap();
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
    let id = r["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/start"),
            None,
        ))
        .await
        .unwrap();
    let mut body = open(&app, &format!("?rollout={id}")).await.into_body();
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/distributionsets/{ds}/invalidate"),
            Some(json!({"cancelRollouts": true})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let mut found = false;
    for _ in 0..10 {
        if common::sse_next(&mut body).await.0 == "rollout" {
            found = true;
            break;
        }
    }
    assert!(found, "no rollout event");
}
