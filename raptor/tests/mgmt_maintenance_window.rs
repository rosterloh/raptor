mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

/// A window that is always open: it starts every minute and runs for nearly a
/// day, so whenever the test happens to run, the last start is at most 59
/// seconds back. Deterministic without an injectable clock.
fn open_window() -> Value {
    json!({"schedule": "0 * * * * ?", "duration": "23:59:59", "timezone": "+00:00"})
}

/// A window that is never open yet: its first start is in 2199, so there is no
/// previous occurrence to be inside of. Also deterministic.
fn future_window() -> Value {
    json!({"schedule": "0 0 2 * * ? 2199", "duration": "01:00:00", "timezone": "+00:00"})
}

/// DS "stable:1.0" with one module, plus target d1. Returns the DS id.
async fn fixture(app: &axum::Router) -> i64 {
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

/// Assigns `ds` to d1, with a maintenance window when one is given. Returns the
/// raw response so tests can assert on rejections too.
async fn assign(app: &axum::Router, ds: i64, window: Option<Value>) -> axum::http::Response<Body> {
    let mut body = json!({"id": ds, "type": "forced"});
    if let Some(w) = window {
        body["maintenanceWindow"] = w;
    }
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/d1/assignedDS",
            Some(body),
        ))
        .await
        .unwrap()
}

async fn assign_ok(app: &axum::Router, ds: i64, window: Option<Value>) -> i64 {
    let resp = assign(app, ds, window).await;
    assert_eq!(resp.status(), StatusCode::OK);
    common::body_json(resp).await["assignedActions"][0]["id"]
        .as_i64()
        .unwrap()
}

async fn deployment_base(app: &axum::Router, action_id: i64) -> Value {
    common::body_json(
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
    .await
}

async fn action_json(app: &axum::Router, action_id: i64) -> Value {
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
async fn assignment_stores_the_window_and_echoes_it_to_operators() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app).await;
    let aid = assign_ok(&app, ds, Some(future_window())).await;

    let mw = &action_json(&app, aid).await["maintenanceWindow"];
    assert_eq!(mw["schedule"], "0 0 2 * * ? 2199");
    assert_eq!(mw["duration"], "01:00:00");
    assert_eq!(mw["timezone"], "+00:00");
    // 2199-01-01T02:00:00Z in epoch millis — the first start of that schedule.
    assert_eq!(mw["nextStartAt"], 7_226_589_600_000_i64);
}

/// The device is told to download now and hold the install: `update: skip` with
/// `maintenanceWindow: unavailable`, while `download` keeps the action's real
/// mode. This is the whole point of the feature.
#[tokio::test]
async fn deployment_base_defers_the_install_outside_the_window() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app).await;
    let aid = assign_ok(&app, ds, Some(future_window())).await;

    let dep = deployment_base(&app, aid).await["deployment"].clone();
    assert_eq!(dep["download"], "forced");
    assert_eq!(dep["update"], "skip");
    assert_eq!(dep["maintenanceWindow"], "unavailable");
}

#[tokio::test]
async fn deployment_base_releases_the_install_inside_the_window() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app).await;
    let aid = assign_ok(&app, ds, Some(open_window())).await;

    let dep = deployment_base(&app, aid).await["deployment"].clone();
    assert_eq!(dep["download"], "forced");
    assert_eq!(dep["update"], "forced");
    assert_eq!(dep["maintenanceWindow"], "available");
}

/// Assignments without a window must look exactly as they did before this
/// feature existed — stock clients parse these payloads strictly.
#[tokio::test]
async fn assignment_without_a_window_is_untouched() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app).await;
    let aid = assign_ok(&app, ds, None).await;

    let dep = deployment_base(&app, aid).await["deployment"].clone();
    assert_eq!(dep["update"], "forced");
    assert!(
        dep.get("maintenanceWindow").is_none(),
        "the key must be absent, not null: {dep}"
    );
    assert!(
        action_json(&app, aid)
            .await
            .get("maintenanceWindow")
            .is_none(),
        "operators should not see an empty window either"
    );
}

/// Quartz — and so hawkBit — spells a stepped range `0/15`. croner 4 rejects
/// that by default in favour of `*/15`, which would turn a schedule that
/// assigned fine yesterday into a 400 on upgrade; `quartz()` opts back in.
#[tokio::test]
async fn quartz_shortcut_step_schedules_are_assignable() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app).await;

    let resp = assign(
        &app,
        ds,
        Some(json!({"schedule": "0 0/15 * * * ?", "duration": "00:10:00", "timezone": "+00:00"})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let page = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1/actions", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(page["total"], 1);
    assert_eq!(
        page["content"][0]["maintenanceWindow"]["schedule"], "0 0/15 * * * ?",
        "the schedule must round-trip as the operator wrote it"
    );
}

/// A window the server cannot evaluate is rejected at assignment time rather
/// than leaving a device that never installs.
#[tokio::test]
async fn malformed_windows_are_rejected_without_creating_an_action() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app).await;

    for w in [
        json!({"schedule": "not a cron", "duration": "01:00:00", "timezone": "+00:00"}),
        // Five fields: Unix cron, missing Quartz's leading seconds.
        json!({"schedule": "0 2 * * ?", "duration": "01:00:00", "timezone": "+00:00"}),
        json!({"schedule": "0 0 2 * * ?", "duration": "2 hours", "timezone": "+00:00"}),
        json!({"schedule": "0 0 2 * * ?", "duration": "01:00:00", "timezone": "CET"}),
        // A year already past never opens again.
        json!({"schedule": "0 0 2 * * ? 2018", "duration": "01:00:00", "timezone": "+00:00"}),
    ] {
        let resp = assign(&app, ds, Some(w.clone())).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "for {w}");
    }

    // A partial window is a malformed body rather than a bad value, but it must
    // not slip through as "no window" either.
    let resp = assign(
        &app,
        ds,
        Some(json!({"schedule": "0 0 2 * * ?", "duration": "01:00:00"})),
    )
    .await;
    assert!(resp.status().is_client_error(), "got {}", resp.status());

    // Nothing was assigned by any of the above.
    let page = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/d1/actions", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(page["total"], 0);
}
