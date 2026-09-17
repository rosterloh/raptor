//! Dynamic rollouts (#18): a trailing group that keeps absorbing targets which
//! start matching the filter after the rollout was created.
//!
//! The static-rollout behaviour these must not disturb is covered by
//! `mgmt_rollouts.rs`; everything here is about the dynamic group.

mod common;

use axum::http::StatusCode;
use raptor::domain::rollout::evaluate_rollouts;
use raptor::entity::action;
use raptor::state::AppState;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use tower::ServiceExt;

/// Complete DS plus `n` targets named dev-0..dev-(n-1). Returns the DS id.
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
        register(app, &format!("dev-{i}")).await;
    }
    ds
}

async fn register(app: &axum::Router, cid: &str) {
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": cid}])),
        ))
        .await
        .unwrap();
}

fn dynamic_body(ds: i64, groups: i64, template: Option<serde_json::Value>) -> serde_json::Value {
    let mut body = json!({
        "name": "r1",
        "distributionSetId": ds,
        "targetFilterQuery": "controllerId==dev-*",
        "amountGroups": groups,
        "successCondition": {"condition": "THRESHOLD", "expression": "100"},
        "dynamic": true,
    });
    if let Some(t) = template {
        body["dynamicGroupTemplate"] = t;
    }
    body
}

async fn create(app: &axum::Router, body: serde_json::Value) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(common::req("POST", "/rest/v1/rollouts", Some(body)))
            .await
            .unwrap(),
    )
    .await
}

async fn groups(app: &axum::Router, id: i64) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{id}/deploygroups"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await
}

async fn rollout(app: &axum::Router, id: i64) -> serde_json::Value {
    common::body_json(
        app.clone()
            .oneshot(common::req("GET", &format!("/rest/v1/rollouts/{id}"), None))
            .await
            .unwrap(),
    )
    .await
}

async fn start(app: &axum::Router, id: i64) {
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
}

/// Finishes every active action of a group, so the evaluator sees its outcome.
async fn finish_group_actions(st: &AppState, group_id: i64) {
    let actions = action::Entity::find()
        .filter(action::Column::RolloutGroupId.eq(group_id))
        .filter(action::Column::Active.eq(true))
        .all(&st.db)
        .await
        .unwrap();
    for a in actions {
        let mut am: action::ActiveModel = a.into();
        am.status = Set("finished".into());
        am.active = Set(false);
        am.update(&st.db).await.unwrap();
    }
}

async fn action_count(app: &axum::Router) -> i64 {
    common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/actions", None))
            .await
            .unwrap(),
    )
    .await["total"]
        .as_i64()
        .unwrap()
}

#[tokio::test]
async fn dynamic_rollout_gets_a_trailing_group() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let r = create(&app, dynamic_body(ds, 2, None)).await;
    assert_eq!(r["dynamic"], true);
    let id = r["id"].as_i64().unwrap();

    let g = groups(&app, id).await;
    assert_eq!(g["total"], 3, "two static groups plus the dynamic one");
    assert_eq!(g["content"][0]["dynamic"], false);
    assert_eq!(g["content"][1]["dynamic"], false);
    assert_eq!(g["content"][2]["dynamic"], true);
    assert_eq!(g["content"][2]["name"], "group-3");
    assert_eq!(
        g["content"][2]["totalTargets"], 0,
        "the trailing group starts empty"
    );
    // The static split is unchanged by the extra group.
    assert_eq!(g["content"][0]["totalTargets"], 2);
    assert_eq!(g["content"][1]["totalTargets"], 2);
}

#[tokio::test]
async fn a_static_rollout_has_no_dynamic_group() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let mut body = dynamic_body(ds, 2, None);
    body["dynamic"] = json!(false);
    let r = create(&app, body).await;
    assert_eq!(r["dynamic"], false);
    let g = groups(&app, r["id"].as_i64().unwrap()).await;
    assert_eq!(g["total"], 2);
    assert_eq!(g["content"][1]["dynamic"], false);
}

#[tokio::test]
async fn the_template_names_and_sizes_the_dynamic_group() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let r = create(
        &app,
        dynamic_body(ds, 2, Some(json!({"nameSuffix": "-dyn", "targetCount": 5}))),
    )
    .await;
    let g = groups(&app, r["id"].as_i64().unwrap()).await;
    assert_eq!(g["content"][2]["name"], "group-3-dyn");
}

#[tokio::test]
async fn a_template_without_dynamic_is_rejected() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let mut body = dynamic_body(ds, 2, Some(json!({"targetCount": 5})));
    body["dynamic"] = json!(false);
    let resp = app
        .clone()
        .oneshot(common::req("POST", "/rest/v1/rollouts", Some(body)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let e = common::body_json(resp).await;
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("dynamicGroupTemplate"),
        "{e}"
    );
}

#[tokio::test]
async fn a_zero_target_count_is_rejected() {
    let (app, _) = common::setup().await;
    let ds = fixture(&app, 4).await;
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/rollouts",
            Some(dynamic_body(ds, 2, Some(json!({"targetCount": 0})))),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// The point of the feature: a device that registers after the rollout started
/// is deployed to without anyone creating a second rollout.
#[tokio::test]
async fn a_target_registered_after_start_is_absorbed_and_deployed_to() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = create(&app, dynamic_body(ds, 1, None)).await;
    let id = r["id"].as_i64().unwrap();
    start(&app, id).await;
    assert_eq!(action_count(&app).await, 2, "the static group deployed");

    // Static group done -> the dynamic group takes over.
    let g = groups(&app, id).await;
    let static_gid = g["content"][0]["id"].as_i64().unwrap();
    finish_group_actions(&st, static_gid).await;
    evaluate_rollouts(&st).await.unwrap();
    let g = groups(&app, id).await;
    assert_eq!(g["content"][1]["status"], "running");

    // A device nobody knew about at creation time.
    register(&app, "dev-late").await;
    evaluate_rollouts(&st).await.unwrap();

    let g = groups(&app, id).await;
    assert_eq!(
        g["content"][1]["totalTargets"], 1,
        "the newcomer joined the dynamic group"
    );
    assert_eq!(action_count(&app).await, 3, "and was deployed to");
    assert_eq!(
        rollout(&app, id).await["totalTargets"],
        3,
        "the rollout's own total grew with it"
    );
}

/// A target already carried by an earlier group must not be picked up again.
#[tokio::test]
async fn targets_already_in_the_rollout_are_not_absorbed_twice() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = create(&app, dynamic_body(ds, 1, None)).await;
    let id = r["id"].as_i64().unwrap();
    start(&app, id).await;

    let g = groups(&app, id).await;
    finish_group_actions(&st, g["content"][0]["id"].as_i64().unwrap()).await;
    for _ in 0..3 {
        evaluate_rollouts(&st).await.unwrap();
    }

    let g = groups(&app, id).await;
    assert_eq!(
        g["content"][1]["totalTargets"], 0,
        "the two originals stayed in the static group"
    );
    assert_eq!(action_count(&app).await, 2);
}

/// A newcomer that arrives while an earlier group is still running joins the
/// trailing group straight away, but waits its turn for a deployment — the
/// same deal a target in the last static group gets.
#[tokio::test]
async fn a_newcomer_waits_its_turn_while_an_earlier_group_runs() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = create(&app, dynamic_body(ds, 2, None)).await;
    let id = r["id"].as_i64().unwrap();
    start(&app, id).await;
    assert_eq!(action_count(&app).await, 1, "only group-1 deployed");

    register(&app, "dev-late").await;
    evaluate_rollouts(&st).await.unwrap();

    let g = groups(&app, id).await;
    assert_eq!(g["content"][2]["status"], "ready");
    assert_eq!(
        g["content"][2]["totalTargets"], 1,
        "absorbed into the trailing group already"
    );
    assert_eq!(action_count(&app).await, 1, "but not deployed to yet");

    // Both static groups complete; the dynamic group's turn comes and the
    // newcomer it has been holding is deployed to.
    finish_group_actions(&st, g["content"][0]["id"].as_i64().unwrap()).await;
    evaluate_rollouts(&st).await.unwrap();
    let g = groups(&app, id).await;
    finish_group_actions(&st, g["content"][1]["id"].as_i64().unwrap()).await;
    evaluate_rollouts(&st).await.unwrap();

    let g = groups(&app, id).await;
    assert_eq!(g["content"][2]["status"], "running");
    assert_eq!(action_count(&app).await, 3);
}

/// hawkBit never completes the trailing dynamic group, so the rollout keeps
/// running even when every target it has deployed to has succeeded.
#[tokio::test]
async fn a_dynamic_rollout_does_not_finish_itself() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let r = create(&app, dynamic_body(ds, 1, None)).await;
    let id = r["id"].as_i64().unwrap();
    start(&app, id).await;

    let g = groups(&app, id).await;
    finish_group_actions(&st, g["content"][0]["id"].as_i64().unwrap()).await;
    evaluate_rollouts(&st).await.unwrap();

    // Absorb a newcomer and let it succeed too: 100% of everything deployed.
    register(&app, "dev-late").await;
    evaluate_rollouts(&st).await.unwrap();
    let g = groups(&app, id).await;
    let dyn_gid = g["content"][1]["id"].as_i64().unwrap();
    finish_group_actions(&st, dyn_gid).await;
    evaluate_rollouts(&st).await.unwrap();

    assert_eq!(
        rollout(&app, id).await["status"],
        "running",
        "a dynamic rollout ends only when an operator stops it"
    );
    let g = groups(&app, id).await;
    assert_eq!(g["content"][1]["status"], "running");
}

/// The equivalent static rollout does finish — the guard above must not have
/// turned the ordinary completion path off.
#[tokio::test]
async fn the_same_rollout_without_dynamic_still_finishes() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 2).await;
    let mut body = dynamic_body(ds, 1, None);
    body["dynamic"] = json!(false);
    let r = create(&app, body).await;
    let id = r["id"].as_i64().unwrap();
    start(&app, id).await;

    let g = groups(&app, id).await;
    finish_group_actions(&st, g["content"][0]["id"].as_i64().unwrap()).await;
    evaluate_rollouts(&st).await.unwrap();

    assert_eq!(rollout(&app, id).await["status"], "finished");
}

/// Once the trailing group reaches its capacity, the next one is created
/// behind it rather than letting a single group grow without bound.
#[tokio::test]
async fn a_full_dynamic_group_rolls_over_to_the_next() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 1).await;
    let r = create(&app, dynamic_body(ds, 1, Some(json!({"targetCount": 2})))).await;
    let id = r["id"].as_i64().unwrap();
    start(&app, id).await;
    let g = groups(&app, id).await;
    finish_group_actions(&st, g["content"][0]["id"].as_i64().unwrap()).await;
    evaluate_rollouts(&st).await.unwrap();

    // Two newcomers exactly fill the dynamic group's capacity.
    register(&app, "dev-a").await;
    register(&app, "dev-b").await;
    evaluate_rollouts(&st).await.unwrap();
    let g = groups(&app, id).await;
    assert_eq!(g["total"], 2, "still just the static and the dynamic group");
    assert_eq!(g["content"][1]["totalTargets"], 2, "filled to capacity");

    // The next sweep notices it is full and opens another.
    evaluate_rollouts(&st).await.unwrap();
    let g = groups(&app, id).await;
    assert_eq!(g["total"], 3);
    assert_eq!(g["content"][2]["dynamic"], true);
    assert_eq!(g["content"][2]["totalTargets"], 0);
    assert_eq!(
        g["content"][2]["name"], "group-3",
        "numbering continues past the groups already created"
    );
}

/// The suffix chosen at creation time is carried by every later dynamic group,
/// which is how hawkBit avoids storing it.
#[tokio::test]
async fn rollover_keeps_the_configured_name_suffix() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 1).await;
    let r = create(
        &app,
        dynamic_body(ds, 1, Some(json!({"nameSuffix": "-dyn", "targetCount": 1}))),
    )
    .await;
    let id = r["id"].as_i64().unwrap();
    start(&app, id).await;
    let g = groups(&app, id).await;
    finish_group_actions(&st, g["content"][0]["id"].as_i64().unwrap()).await;
    evaluate_rollouts(&st).await.unwrap();

    register(&app, "dev-a").await;
    evaluate_rollouts(&st).await.unwrap();
    evaluate_rollouts(&st).await.unwrap();

    let g = groups(&app, id).await;
    assert_eq!(g["total"], 3);
    assert_eq!(g["content"][1]["name"], "group-2-dyn");
    assert_eq!(g["content"][2]["name"], "group-3-dyn");
}

/// Withdrawing the distribution set has to stop the rollout pulling more
/// devices onto it — otherwise invalidation leaks through the one path that
/// keeps assigning on its own.
#[tokio::test]
async fn an_invalidated_distribution_set_stops_the_absorbing() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 1).await;
    let r = create(&app, dynamic_body(ds, 1, None)).await;
    let id = r["id"].as_i64().unwrap();
    start(&app, id).await;
    let g = groups(&app, id).await;
    finish_group_actions(&st, g["content"][0]["id"].as_i64().unwrap()).await;
    evaluate_rollouts(&st).await.unwrap();

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/distributionsets/{ds}/invalidate"),
            Some(json!({"actionCancelationType": "none", "cancelRollouts": false})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    register(&app, "dev-late").await;
    // The sweep must stay healthy rather than erroring on the invalid DS.
    evaluate_rollouts(&st).await.unwrap();

    let g = groups(&app, id).await;
    assert_eq!(
        g["content"][1]["totalTargets"], 0,
        "no device is drawn onto a withdrawn release"
    );
    assert_eq!(action_count(&app).await, 1);
}
