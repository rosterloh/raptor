mod common;

use axum::http::StatusCode;
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
