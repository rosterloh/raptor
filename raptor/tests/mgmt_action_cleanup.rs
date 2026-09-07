//! Automatic action cleanup (#134): what gets deleted, what is protected, and
//! that reclaiming an action's history does not rewrite a rollout's outcome.

mod common;

use axum::http::StatusCode;
use raptor::config::CleanupConfig;
use raptor::domain::cleanup::cleanup_actions;
use raptor::entity::{action, action_status, action_status_message};
use raptor::state::AppState;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::json;
use tower::ServiceExt;

fn enabled(days: u64) -> CleanupConfig {
    CleanupConfig {
        enabled: true,
        action_expiry_days: days,
        ..Default::default()
    }
}

/// A complete DS plus `n` targets, returning the DS id.
async fn fixture(app: &axum::Router, n: usize) -> i64 {
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
                Some(
                    json!([{"name": "d", "version": "1.0", "type": "os", "modules": [{"id": sm}]}]),
                ),
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

async fn assign(app: &axum::Router, cid: &str, ds: i64) -> i64 {
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                &format!("/rest/v1/targets/{cid}/assignedDS"),
                Some(json!({"id": ds})),
            ))
            .await
            .unwrap(),
    )
    .await["assignedActions"][0]["id"]
        .as_i64()
        .unwrap()
}

/// Closes an action and backdates it, standing in for the passage of time.
async fn close_and_age(st: &AppState, action_id: i64, status: &str, days_ago: i64) {
    let a = action::Entity::find_by_id(action_id)
        .one(&st.db)
        .await
        .unwrap()
        .unwrap();
    let mut am: action::ActiveModel = a.into();
    am.status = Set(status.into());
    am.active = Set(false);
    am.updated_at = Set(raptor::util::now_ms() - days_ago * 24 * 60 * 60 * 1000);
    am.update(&st.db).await.unwrap();
}

async fn action_count(st: &AppState) -> u64 {
    action::Entity::find().count(&st.db).await.unwrap()
}

#[tokio::test]
async fn disabled_by_default_deletes_nothing() {
    let (app, st) = common::setup().await;
    let ds = fixture(&app, 1).await;
    let aid = assign(&app, "dev-0", ds).await;
    close_and_age(&st, aid, "finished", 365).await;

    assert_eq!(cleanup_actions(&st).await.unwrap(), 0);
    assert_eq!(action_count(&st).await, 1);
}

/// The point of the feature: an old closed action goes, and takes its status
/// history with it. raptor's foreign keys have no `ON DELETE CASCADE` (and
/// SQLite would not enforce one), so orphans are a real risk, not a
/// hypothetical — this asserts none are left.
#[tokio::test]
async fn an_expired_action_goes_with_its_whole_status_history() {
    let (app, st) = common::setup_with_cleanup(enabled(30)).await;
    let ds = fixture(&app, 1).await;
    let aid = assign(&app, "dev-0", ds).await;

    // Give it some history to reclaim, messages included.
    raptor::domain::deployment::add_action_status(
        &st.db,
        aid,
        "proceeding",
        &["step one".into(), "step two".into()],
    )
    .await
    .unwrap();
    close_and_age(&st, aid, "finished", 31).await;

    let status_ids: Vec<i64> = action_status::Entity::find()
        .filter(action_status::Column::ActionId.eq(aid))
        .all(&st.db)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert!(!status_ids.is_empty());

    assert_eq!(cleanup_actions(&st).await.unwrap(), 1);
    assert_eq!(action_count(&st).await, 0);
    assert_eq!(
        action_status::Entity::find()
            .filter(action_status::Column::ActionId.eq(aid))
            .count(&st.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        action_status_message::Entity::find()
            .filter(action_status_message::Column::ActionStatusId.is_in(status_ids))
            .count(&st.db)
            .await
            .unwrap(),
        0,
        "status messages were orphaned rather than deleted"
    );
}

#[tokio::test]
async fn actions_inside_the_retention_window_are_kept() {
    let (app, st) = common::setup_with_cleanup(enabled(30)).await;
    let ds = fixture(&app, 2).await;
    let old = assign(&app, "dev-0", ds).await;
    let recent = assign(&app, "dev-1", ds).await;
    close_and_age(&st, old, "finished", 31).await;
    close_and_age(&st, recent, "finished", 29).await;

    assert_eq!(cleanup_actions(&st).await.unwrap(), 1);
    assert!(
        action::Entity::find_by_id(recent)
            .one(&st.db)
            .await
            .unwrap()
            .is_some(),
        "an action inside the window was deleted"
    );
}

/// Status list and active-ness are separate gates: an action can carry a
/// listed status while still being live, and deleting it would strand the
/// device holding an id the server no longer knows.
#[tokio::test]
async fn an_active_action_is_never_eligible() {
    let (app, st) = common::setup_with_cleanup(CleanupConfig {
        enabled: true,
        action_expiry_days: 30,
        // deliberately lists a status an active action can hold
        action_statuses: vec!["finished".into(), "running".into()],
        ..Default::default()
    })
    .await;
    let ds = fixture(&app, 1).await;
    let aid = assign(&app, "dev-0", ds).await;

    // Old enough, listed status — but still active.
    let a = action::Entity::find_by_id(aid)
        .one(&st.db)
        .await
        .unwrap()
        .unwrap();
    assert!(a.active);
    let mut am: action::ActiveModel = a.into();
    am.updated_at = Set(raptor::util::now_ms() - 365 * 24 * 60 * 60 * 1000);
    am.update(&st.db).await.unwrap();

    assert_eq!(cleanup_actions(&st).await.unwrap(), 0);
    assert_eq!(action_count(&st).await, 1);
}

#[tokio::test]
async fn only_the_configured_statuses_are_deleted() {
    let (app, st) = common::setup_with_cleanup(CleanupConfig {
        enabled: true,
        action_expiry_days: 30,
        action_statuses: vec!["error".into()],
        ..Default::default()
    })
    .await;
    let ds = fixture(&app, 2).await;
    let failed = assign(&app, "dev-0", ds).await;
    let done = assign(&app, "dev-1", ds).await;
    close_and_age(&st, failed, "error", 31).await;
    close_and_age(&st, done, "finished", 31).await;

    assert_eq!(cleanup_actions(&st).await.unwrap(), 1);
    assert!(
        action::Entity::find_by_id(done)
            .one(&st.db)
            .await
            .unwrap()
            .is_some(),
        "a status outside the configured set was deleted"
    );
}

/// The hazard this feature had to solve. A rollout's progress is derived by
/// counting actions, so deleting a finished one would walk its targets back to
/// `scheduled` — a long-completed rollout reporting as though it never ran.
/// hawkBit has exactly this drift and tolerates it; raptor must not.
#[tokio::test]
async fn cleanup_does_not_rewrite_a_finished_rollouts_progress() {
    let (app, st) = common::setup_with_cleanup(enabled(30)).await;
    let ds = fixture(&app, 2).await;

    let rollout = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/rollouts",
                Some(json!({
                    "name": "r1", "distributionSetId": ds,
                    "targetFilterQuery": "controllerId==dev-*", "amountGroups": 1,
                    "successCondition": {"condition": "THRESHOLD", "expression": "100"},
                })),
            ))
            .await
            .unwrap(),
    )
    .await;
    let rid = rollout["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/rollouts/{rid}/start"),
            None,
        ))
        .await
        .unwrap();

    // Finish every action the rollout issued, then let the evaluator settle it.
    for a in action::Entity::find()
        .filter(action::Column::RolloutId.eq(rid))
        .all(&st.db)
        .await
        .unwrap()
    {
        close_and_age(&st, a.id, "finished", 31).await;
    }
    raptor::domain::rollout::evaluate_rollouts(&st)
        .await
        .unwrap();

    let before = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{rid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(before["totalTargetsPerStatus"]["finished"], 2);

    // Reclaim the history.
    assert_eq!(cleanup_actions(&st).await.unwrap(), 2);
    assert_eq!(action_count(&st).await, 0);

    let after = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{rid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        after["totalTargetsPerStatus"], before["totalTargetsPerStatus"],
        "cleanup changed the rollout's reported progress"
    );
    assert_eq!(after["totalTargetsPerStatus"]["scheduled"], 0);

    // The per-group view is derived separately and must agree.
    let groups = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/rollouts/{rid}/deploygroups"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(groups["content"][0]["totalTargetsPerStatus"]["finished"], 2);
    assert_eq!(
        groups["content"][0]["totalTargetsPerStatus"]["scheduled"],
        0
    );
}

/// A sweep chains batches, so a backlog larger than one batch still drains —
/// and stops cleanly once there is nothing left rather than spinning.
#[tokio::test]
async fn a_sweep_drains_more_than_one_batch_and_then_stops() {
    let (app, st) = common::setup_with_cleanup(enabled(30)).await;
    let ds = fixture(&app, 3).await;
    for i in 0..3 {
        let aid = assign(&app, &format!("dev-{i}"), ds).await;
        close_and_age(&st, aid, "finished", 31).await;
    }

    assert_eq!(raptor::domain::cleanup::run_sweep(&st).await.unwrap(), 3);
    assert_eq!(action_count(&st).await, 0);
    // Nothing left: a second sweep is a no-op, not an error.
    assert_eq!(raptor::domain::cleanup::run_sweep(&st).await.unwrap(), 0);
}

/// The two hawkBit tenant-config keys must read as "off" when cleanup is
/// disabled — reporting a retention window would tell a client it is running.
#[tokio::test]
async fn tenant_config_reports_the_cleanup_keys() {
    let get = |app: axum::Router, key: &str| {
        let uri = format!("/rest/v1/system/configs/{key}");
        async move {
            let resp = app.oneshot(common::req("GET", &uri, None)).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            common::body_json(resp).await["value"].clone()
        }
    };

    let (app, _) = common::setup().await;
    assert_eq!(get(app.clone(), "action.cleanup.auto.expiry").await, -1);
    assert_eq!(get(app, "action.cleanup.auto.status").await, "");

    let (app, _) = common::setup_with_cleanup(enabled(30)).await;
    // hawkBit expresses the expiry in milliseconds.
    assert_eq!(
        get(app.clone(), "action.cleanup.auto.expiry").await,
        30i64 * 24 * 60 * 60 * 1000
    );
    assert_eq!(
        get(app, "action.cleanup.auto.status").await,
        "FINISHED,ERROR,CANCELED"
    );
}
