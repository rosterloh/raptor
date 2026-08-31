mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use raptor::domain::rollout::evaluate_rollouts;
use raptor::entity::action;
use raptor::state::AppState;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use tower::ServiceExt;

/// Creates a complete DS and `n` targets named dev-1..dev-n. Returns the DS id.
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

fn create_body(
    ds: i64,
    groups: i64,
    success_pct: &str,
    error_pct: Option<&str>,
) -> serde_json::Value {
    let mut body = json!({
        "name": "r1",
        "distributionSetId": ds,
        "targetFilterQuery": "controllerId==dev-*",
        "amountGroups": groups,
        "successCondition": {"condition": "THRESHOLD", "expression": success_pct},
    });
    if let Some(e) = error_pct {
        body["errorCondition"] = json!({"condition": "THRESHOLD", "expression": e});
    }
    body
}

/// Directly finishes every active action belonging to `group_id` (bypasses the DDI
/// feedback HTTP flow, which is exercised elsewhere) so tests can drive the evaluator
/// deterministically.
async fn finish_group_actions(st: &AppState, group_id: i64, as_error: bool) {
    let actions = action::Entity::find()
        .filter(action::Column::RolloutGroupId.eq(group_id))
        .filter(action::Column::Active.eq(true))
        .all(&st.db)
        .await
        .unwrap();
    for a in actions {
        let mut am: action::ActiveModel = a.into();
        am.status = Set(if as_error {
            "error".into()
        } else {
            "finished".into()
        });
        am.active = Set(false);
        am.update(&st.db).await.unwrap();
    }
}

#[tokio::test]
async fn create_splits_targets_into_groups() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 5).await;

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/rollouts",
            Some(create_body(ds, 2, "100", None)),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let r = common::body_json(resp).await;
    assert_eq!(r["status"], "ready");
    assert_eq!(r["totalTargets"], 5);
    let id = r["id"].as_i64().unwrap();

    let groups = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{id}/deploygroups"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(groups["total"], 2);
    assert_eq!(groups["content"][0]["totalTargets"], 3);
    assert_eq!(groups["content"][1]["totalTargets"], 2);
}

#[tokio::test]
async fn start_only_schedules_first_group() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 2, "100", None)),
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = r["id"].as_i64().unwrap();

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/start"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let r = common::body_json(resp).await;
    assert_eq!(r["status"], "running");

    let groups = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{id}/deploygroups"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(groups["content"][0]["status"], "running");
    assert_eq!(groups["content"][1]["status"], "ready");

    // only group 0's targets have actions
    let all_actions = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/actions", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(all_actions["total"], 2);
}

#[tokio::test]
async fn success_threshold_advances_group_and_finishes_rollout() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 2, "100", None)),
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

    let groups = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{id}/deploygroups"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let g0 = groups["content"][0]["id"].as_i64().unwrap();
    let g1 = groups["content"][1]["id"].as_i64().unwrap();

    finish_group_actions(&st, g0, false).await;
    evaluate_rollouts(&st).await.unwrap();

    let groups = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{id}/deploygroups"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(groups["content"][0]["status"], "finished");
    assert_eq!(groups["content"][1]["status"], "running");

    let r = common::body_json(
        app.clone()
            .oneshot(common::req("GET", &format!("/rest/v1/rollouts/{id}"), None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["status"], "running");

    finish_group_actions(&st, g1, false).await;
    evaluate_rollouts(&st).await.unwrap();

    let r = common::body_json(
        app.clone()
            .oneshot(common::req("GET", &format!("/rest/v1/rollouts/{id}"), None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["status"], "finished");
}

#[tokio::test]
async fn error_threshold_pauses_rollout_without_advancing() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 2, "100", Some("50"))),
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

    let groups = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{id}/deploygroups"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let g0 = groups["content"][0]["id"].as_i64().unwrap();

    finish_group_actions(&st, g0, true).await;
    evaluate_rollouts(&st).await.unwrap();

    let r = common::body_json(
        app.clone()
            .oneshot(common::req("GET", &format!("/rest/v1/rollouts/{id}"), None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["status"], "paused");

    // group 1 never got scheduled
    let all_actions = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/actions", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(all_actions["total"], 2);
}

#[tokio::test]
async fn pause_resume_and_delete() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 1, "100", None)),
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

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/pause"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["status"], "paused");

    // evaluator ignores paused rollouts
    let groups = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{id}/deploygroups"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let g0 = groups["content"][0]["id"].as_i64().unwrap();
    finish_group_actions(&st, g0, false).await;
    evaluate_rollouts(&st).await.unwrap();
    let r = common::body_json(
        app.clone()
            .oneshot(common::req("GET", &format!("/rest/v1/rollouts/{id}"), None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["status"], "paused");

    // resume re-evaluates immediately and finishes the (only) group
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/resume"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let r = common::body_json(
        app.clone()
            .oneshot(common::req("GET", &format!("/rest/v1/rollouts/{id}"), None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["status"], "finished");

    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/rollouts/{id}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .clone()
        .oneshot(common::req("GET", &format!("/rest/v1/rollouts/{id}"), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// `totalTargetsPerStatus` with the zero-valued keys dropped, so assertions
/// only have to name the buckets that matter.
fn per_status(v: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    v["totalTargetsPerStatus"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, n)| n.as_i64() != Some(0))
        .map(|(k, n)| (k.clone(), n.clone()))
        .collect()
}

async fn get_json(app: &axum::Router, path: &str) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(common::req("GET", path, None))
            .await
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn targets_per_status_tracks_group_progress() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 2, "100", None)),
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = r["id"].as_i64().unwrap();

    // not started: every target is notstarted, on the rollout and its groups
    assert_eq!(
        per_status(&r),
        json!({"notstarted": 4}).as_object().cloned().unwrap()
    );
    let groups = get_json(&app, &format!("/rest/v1/rollouts/{id}/deploygroups")).await;
    assert_eq!(
        per_status(&groups["content"][0]),
        json!({"notstarted": 2}).as_object().cloned().unwrap()
    );
    assert_eq!(
        per_status(&groups["content"][1]),
        json!({"notstarted": 2}).as_object().cloned().unwrap()
    );

    // started: group 0 is deploying, group 1 waits its turn
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                &format!("/rest/v1/rollouts/{id}/start"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        per_status(&r),
        json!({"running": 2, "scheduled": 2})
            .as_object()
            .cloned()
            .unwrap()
    );
    let groups = get_json(&app, &format!("/rest/v1/rollouts/{id}/deploygroups")).await;
    assert_eq!(
        per_status(&groups["content"][0]),
        json!({"running": 2}).as_object().cloned().unwrap()
    );
    assert_eq!(
        per_status(&groups["content"][1]),
        json!({"scheduled": 2}).as_object().cloned().unwrap()
    );
    let g0 = groups["content"][0]["id"].as_i64().unwrap();
    let g1 = groups["content"][1]["id"].as_i64().unwrap();

    // group 0 succeeds, which schedules group 1
    finish_group_actions(&st, g0, false).await;
    evaluate_rollouts(&st).await.unwrap();
    let r = get_json(&app, &format!("/rest/v1/rollouts/{id}")).await;
    assert_eq!(
        per_status(&r),
        json!({"finished": 2, "running": 2})
            .as_object()
            .cloned()
            .unwrap()
    );
    let g = get_json(&app, &format!("/rest/v1/rollouts/{id}/deploygroups/{g0}")).await;
    assert_eq!(
        per_status(&g),
        json!({"finished": 2}).as_object().cloned().unwrap()
    );

    // group 1 fails
    finish_group_actions(&st, g1, true).await;
    let r = get_json(&app, &format!("/rest/v1/rollouts/{id}")).await;
    assert_eq!(
        per_status(&r),
        json!({"finished": 2, "error": 2})
            .as_object()
            .cloned()
            .unwrap()
    );
    let g = get_json(&app, &format!("/rest/v1/rollouts/{id}/deploygroups/{g1}")).await;
    assert_eq!(
        per_status(&g),
        json!({"error": 2}).as_object().cloned().unwrap()
    );
}

#[tokio::test]
async fn rollout_list_reports_targets_per_status() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 3).await;
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/rollouts",
            Some(create_body(ds, 1, "100", None)),
        ))
        .await
        .unwrap();

    let list = get_json(&app, "/rest/v1/rollouts").await;
    assert_eq!(list["total"], 1);
    assert_eq!(
        per_status(&list["content"][0]),
        json!({"notstarted": 3}).as_object().cloned().unwrap()
    );
}

#[tokio::test]
async fn rollout_action_type_is_inherited_by_the_actions_it_creates() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let mut body = create_body(ds, 1, "100", None);
    body["type"] = json!("downloadonly");

    let r = common::body_json(
        app.clone()
            .oneshot(common::req("POST", "/rest/v1/rollouts", Some(body)))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["type"], json!("downloadonly"));
    let id = r["id"].as_i64().unwrap();

    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/start"),
            None,
        ))
        .await
        .unwrap();

    // the started group's actions carry the rollout's type, so the device is
    // told to download and skip installing
    let a = get_json(&app, "/rest/v1/targets/dev-1/actions").await;
    let aid = a["content"][0]["id"].as_i64().unwrap();
    assert_eq!(a["content"][0]["forceType"], json!("downloadonly"));
    let dep = common::body_json(
        app.clone()
            .oneshot(
                axum::http::Request::get(format!(
                    "/DEFAULT/controller/v1/dev-1/deploymentBase/{aid}"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(dep["deployment"]["download"], json!("forced"));
    assert_eq!(dep["deployment"]["update"], json!("skip"));
}

#[tokio::test]
async fn rollout_rejects_an_unknown_action_type() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 1).await;
    let mut body = create_body(ds, 1, "100", None);
    body["type"] = json!("whenever");
    let resp = app
        .clone()
        .oneshot(common::req("POST", "/rest/v1/rollouts", Some(body)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// The one action `dev-{i}` has, as `(id, detailStatus)`.
async fn device_action(app: &axum::Router, i: usize) -> (i64, String) {
    let page = get_json(app, &format!("/rest/v1/targets/dev-{i}/actions")).await;
    let a = &page["content"][0];
    (
        a["id"].as_i64().unwrap(),
        a["detailStatus"].as_str().unwrap().to_string(),
    )
}

fn ddi_cancel_feedback(cid: &str, action_id: i64) -> Request<Body> {
    let body = json!({
        "id": action_id.to_string(),
        "time": "20260704T120000",
        "status": {"execution": "closed", "result": {"finished": "success"}, "details": []}
    });
    Request::post(format!(
        "/DEFAULT/controller/v1/{cid}/cancelAction/{action_id}/feedback"
    ))
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(body.to_string()))
    .unwrap()
}

/// Stop soft-cancels the actions the rollout issued — devices are told over DDI
/// rather than having the action yanked out from under them — and the rollout
/// only reaches the terminal `stopped` once they have all confirmed.
#[tokio::test]
async fn stop_cancels_issued_actions_and_settles_once_devices_confirm() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 1, "100", None)),
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

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/stop"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // Devices still have the cancel to pick up, so this is not terminal yet.
    assert_eq!(common::body_json(resp).await["status"], "stopping");

    let groups = get_json(&app, &format!("/rest/v1/rollouts/{id}/deploygroups")).await;
    assert_eq!(groups["content"][0]["status"], "stopped");

    // Both actions are cancelling and still active — a hard cancel would have
    // closed them out without ever telling the device.
    let (a0, s0) = device_action(&app, 0).await;
    let (a1, s1) = device_action(&app, 1).await;
    assert_eq!((s0.as_str(), s1.as_str()), ("canceling", "canceling"));

    // A polling device is served the cancel over DDI.
    let resp = app
        .clone()
        .oneshot(
            Request::get(format!("/DEFAULT/controller/v1/dev-0/cancelAction/{a0}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        common::body_json(resp).await["cancelAction"]["stopId"],
        a0.to_string()
    );

    // One device confirming is not enough to settle the rollout.
    app.clone()
        .oneshot(ddi_cancel_feedback("dev-0", a0))
        .await
        .unwrap();
    evaluate_rollouts(&st).await.unwrap();
    assert_eq!(
        get_json(&app, &format!("/rest/v1/rollouts/{id}")).await["status"],
        "stopping"
    );

    app.clone()
        .oneshot(ddi_cancel_feedback("dev-1", a1))
        .await
        .unwrap();
    evaluate_rollouts(&st).await.unwrap();
    assert_eq!(
        get_json(&app, &format!("/rest/v1/rollouts/{id}")).await["status"],
        "stopped"
    );
    assert_eq!(device_action(&app, 0).await.1, "canceled");
}

/// A rollout with nothing in flight has nothing to wait for, so it does not
/// linger in `stopping`.
#[tokio::test]
async fn stop_settles_immediately_when_no_action_is_in_flight() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 1, "100", None)),
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

    // Both devices are done; the rollout just has not been evaluated yet.
    let groups = get_json(&app, &format!("/rest/v1/rollouts/{id}/deploygroups")).await;
    let g0 = groups["content"][0]["id"].as_i64().unwrap();
    finish_group_actions(&st, g0, false).await;

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/stop"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["status"], "stopped");
}

/// Stopping abandons the groups that had not finished yet without rewriting the
/// history of the ones that had.
#[tokio::test]
async fn stop_leaves_already_finished_groups_alone() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 2, "100", None)),
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

    // Drive the first group to completion so the second one is scheduled.
    let groups = get_json(&app, &format!("/rest/v1/rollouts/{id}/deploygroups")).await;
    let g0 = groups["content"][0]["id"].as_i64().unwrap();
    finish_group_actions(&st, g0, false).await;
    evaluate_rollouts(&st).await.unwrap();

    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/stop"),
            None,
        ))
        .await
        .unwrap();

    let groups = get_json(&app, &format!("/rest/v1/rollouts/{id}/deploygroups")).await;
    assert_eq!(groups["content"][0]["status"], "finished");
    assert_eq!(groups["content"][1]["status"], "stopped");
    // The second group's device still has a cancel to acknowledge.
    assert_eq!(
        get_json(&app, &format!("/rest/v1/rollouts/{id}")).await["status"],
        "stopping"
    );
}

/// Stop is an operation on a live rollout: legal from `running` and `paused`,
/// rejected everywhere else — including a second stop.
#[tokio::test]
async fn stop_is_rejected_only_once_terminal_or_draining() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let stop = |id: i64| {
        let app = app.clone();
        async move {
            app.oneshot(common::req(
                "POST",
                &format!("/rest/v1/rollouts/{id}/stop"),
                None,
            ))
            .await
            .unwrap()
            .status()
        }
    };
    let create = |body: serde_json::Value| {
        let app = app.clone();
        async move {
            common::body_json(
                app.oneshot(common::req("POST", "/rest/v1/rollouts", Some(body)))
                    .await
                    .unwrap(),
            )
            .await["id"]
                .as_i64()
                .unwrap()
        }
    };

    // `ready` — never started, so there are no actions to cancel, but stopping
    // it is how an operator retires it while keeping the record (hawkBit's
    // ROLLOUT_STATUS_STOPPABLE includes READY). It settles straight to
    // `stopped`, nothing being in flight.
    let id = create(create_body(ds, 1, "100", None)).await;
    assert_eq!(stop(id).await, StatusCode::OK);
    assert_eq!(
        get_json(&app, &format!("/rest/v1/rollouts/{id}")).await["status"],
        "stopped"
    );
    // ...and a stopped rollout is terminal, so a second stop is refused.
    assert_eq!(stop(id).await, StatusCode::BAD_REQUEST);

    // `paused` is a live rollout: stop is the escalation from it. (A fresh
    // rollout, under a different name — rollout names are unique.)
    let mut body = create_body(ds, 1, "100", None);
    body["name"] = serde_json::json!("r2");
    let id = create(body).await;
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/start"),
            None,
        ))
        .await
        .unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{id}/pause"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(stop(id).await, StatusCode::OK);

    // Already stopping — the cancels are out, a second stop is a no-op error.
    assert_eq!(stop(id).await, StatusCode::BAD_REQUEST);
}

// --- Approval workflow (#17) -------------------------------------------------

/// Creates a rollout over `n` targets and returns its id and the create
/// response, so a test can assert on the status it landed in.
async fn create_rollout(app: &axum::Router, ds: i64) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(create_body(ds, 1, "100", None)),
            ))
            .await
            .unwrap(),
    )
    .await
}

async fn post_status(app: &axum::Router, path: &str) -> StatusCode {
    app.clone()
        .oneshot(common::req("POST", path, None))
        .await
        .unwrap()
        .status()
}

/// The gate is off by default: a rollout is created `ready` and starts without
/// anyone approving anything. This is the "unchanged behavior" half of #17's
/// acceptance criteria.
#[tokio::test]
async fn approval_disabled_leaves_rollouts_directly_startable() {
    let (app, _st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = create_rollout(&app, ds).await;
    assert_eq!(r["status"], "ready");
    // Never decided on, so hawkBit's approval fields stay off the wire.
    assert!(r.get("approveDecidedBy").is_none());
    assert!(r.get("approvalRemark").is_none());

    let id = r["id"].as_i64().unwrap();
    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/start")).await,
        StatusCode::OK
    );
}

/// With the gate on, a new rollout waits for a decision and cannot be started.
/// Its targets are `notstarted`, not `scheduled` — nothing has been issued.
#[tokio::test]
async fn approval_enabled_holds_new_rollouts_until_approved() {
    let (app, _st) = common::setup_with_rollout_approval().await;
    let ds = fixture(&app, 2).await;
    let r = create_rollout(&app, ds).await;
    let id = r["id"].as_i64().unwrap();
    assert_eq!(r["status"], "waiting_for_approval");
    assert_eq!(
        per_status(&r),
        serde_json::json!({"notstarted": 2})
            .as_object()
            .cloned()
            .unwrap()
    );

    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/start")).await,
        StatusCode::BAD_REQUEST
    );

    // Approve, then the same start succeeds.
    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/approve")).await,
        StatusCode::NO_CONTENT
    );
    let r = get_json(&app, &format!("/rest/v1/rollouts/{id}")).await;
    assert_eq!(r["status"], "ready");
    assert_eq!(r["approveDecidedBy"], "admin");
    // No remark was given, so none is reported.
    assert!(r.get("approvalRemark").is_none());

    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/start")).await,
        StatusCode::OK
    );
    assert_eq!(
        get_json(&app, &format!("/rest/v1/rollouts/{id}")).await["status"],
        "running"
    );
}

/// Denial is terminal: the rollout can never be started, and a second decision
/// on it is refused.
#[tokio::test]
async fn denied_rollout_can_never_be_started() {
    let (app, _st) = common::setup_with_rollout_approval().await;
    let ds = fixture(&app, 2).await;
    let id = create_rollout(&app, ds).await["id"].as_i64().unwrap();

    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/deny")).await,
        StatusCode::NO_CONTENT
    );
    let r = get_json(&app, &format!("/rest/v1/rollouts/{id}")).await;
    assert_eq!(r["status"], "approval_denied");
    assert_eq!(r["approveDecidedBy"], "admin");
    // Denied is a pre-start state too: its targets were never scheduled.
    assert_eq!(
        per_status(&r),
        serde_json::json!({"notstarted": 2})
            .as_object()
            .cloned()
            .unwrap()
    );

    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/start")).await,
        StatusCode::BAD_REQUEST
    );
    // There is no way back out of a denial — not even by approving it after.
    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/approve")).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get_json(&app, &format!("/rest/v1/rollouts/{id}")).await["status"],
        "approval_denied"
    );
}

/// hawkBit takes the note as a `remark` query parameter on both endpoints.
#[tokio::test]
async fn approval_records_the_remark_query_parameter() {
    let (app, _st) = common::setup_with_rollout_approval().await;
    let ds = fixture(&app, 2).await;
    let id = create_rollout(&app, ds).await["id"].as_i64().unwrap();

    assert_eq!(
        post_status(
            &app,
            &format!("/rest/v1/rollouts/{id}/deny?remark=fleet%20is%20frozen"),
        )
        .await,
        StatusCode::NO_CONTENT
    );
    let r = get_json(&app, &format!("/rest/v1/rollouts/{id}")).await;
    assert_eq!(r["approvalRemark"], "fleet is frozen");
    assert_eq!(r["status"], "approval_denied");
}

/// Approving something that is not waiting for a decision is a 400, and a
/// rollout that was never gated is exactly that case.
#[tokio::test]
async fn approving_a_rollout_that_is_not_waiting_is_rejected() {
    let (app, _st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let id = create_rollout(&app, ds).await["id"].as_i64().unwrap();

    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/approve")).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post_status(&app, &format!("/rest/v1/rollouts/{id}/deny")).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get_json(&app, &format!("/rest/v1/rollouts/{id}")).await["status"],
        "ready"
    );
}

/// A decision on a rollout that does not exist is a 404, not a 400.
#[tokio::test]
async fn approving_an_unknown_rollout_is_not_found() {
    let (app, _st) = common::setup_with_rollout_approval().await;
    assert_eq!(
        post_status(&app, "/rest/v1/rollouts/9999/approve").await,
        StatusCode::NOT_FOUND
    );
}

/// hawkBit's `ROLLOUT_STATUS_STOPPABLE` includes both approval states, so an
/// operator can retire a rollout they are holding — or one they denied —
/// without deleting the record of it.
#[tokio::test]
async fn approval_states_are_stoppable() {
    for deny_first in [false, true] {
        let (app, _st) = common::setup_with_rollout_approval().await;
        let ds = fixture(&app, 2).await;
        let id = create_rollout(&app, ds).await["id"].as_i64().unwrap();

        if deny_first {
            assert_eq!(
                post_status(&app, &format!("/rest/v1/rollouts/{id}/deny")).await,
                StatusCode::NO_CONTENT
            );
        }

        assert_eq!(
            post_status(&app, &format!("/rest/v1/rollouts/{id}/stop")).await,
            StatusCode::OK
        );
        // Nothing was ever issued, so it settles straight to `stopped`.
        assert_eq!(
            get_json(&app, &format!("/rest/v1/rollouts/{id}")).await["status"],
            "stopped"
        );
    }
}
