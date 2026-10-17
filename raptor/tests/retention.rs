//! Release retention: which sets a sweep retires, what goes with them, and
//! what must survive (newer releases, shared modules, other names).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use raptor::config::RetentionConfig;
use raptor::domain::retention::{Retired, run_sweep};
use raptor::entity::{action, distribution_set, rollout, software_module, target};
use raptor::state::AppState;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::json;
use tower::ServiceExt;

fn keep(n: u32) -> RetentionConfig {
    RetentionConfig {
        keep_releases: Some(n),
        ..Default::default()
    }
}

const BOUNDARY: &str = "raptorboundary";

/// A software module holding one artifact with `content`; returns (id, sha256).
async fn module(app: &axum::Router, name: &str, content: &[u8]) -> (i64, String) {
    let id = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/softwaremodules",
                Some(json!([{"name": name, "version": "1.0", "type": "os"}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    let resp = app
        .clone()
        .oneshot(
            Request::post(format!("/rest/v1/softwaremodules/{id}/artifacts"))
                .header(header::AUTHORIZATION, common::mgmt_auth_header())
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let sha = common::body_json(resp).await["hashes"]["sha256"]
        .as_str()
        .unwrap()
        .to_string();
    (id, sha)
}

async fn ds(app: &axum::Router, name: &str, version: &str, modules: &[i64]) -> i64 {
    let mods: Vec<_> = modules.iter().map(|m| json!({"id": m})).collect();
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/distributionsets",
                Some(json!([{"name": name, "version": version, "type": "os", "modules": mods}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap()
}

/// A release with its own module and artifact; returns (ds id, module id, sha256).
async fn release(app: &axum::Router, name: &str, version: &str) -> (i64, i64, String) {
    let label = format!("{name}-{version}");
    let (m, sha) = module(app, &label, label.as_bytes()).await;
    (ds(app, name, version, &[m]).await, m, sha)
}

async fn target(app: &axum::Router, cid: &str) {
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": cid}])),
        ))
        .await
        .unwrap();
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

/// Closes the action as finished and records its set as installed, standing
/// in for the DDI feedback flow (covered in ddi_feedback).
async fn install(st: &AppState, action_id: i64) {
    let a = action::Entity::find_by_id(action_id)
        .one(&st.db)
        .await
        .unwrap()
        .unwrap();
    let t = target::Entity::find_by_id(a.target_id)
        .one(&st.db)
        .await
        .unwrap()
        .unwrap();
    let mut tm: target::ActiveModel = t.into();
    tm.installed_ds_id = Set(Some(a.ds_id));
    tm.update_status = Set("in_sync".into());
    tm.update(&st.db).await.unwrap();
    let mut am: action::ActiveModel = a.into();
    am.status = Set("finished".into());
    am.active = Set(false);
    am.update(&st.db).await.unwrap();
}

async fn ds_exists(st: &AppState, id: i64) -> bool {
    distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await
        .unwrap()
        .is_some()
}

async fn module_exists(st: &AppState, id: i64) -> bool {
    software_module::Entity::find_by_id(id)
        .one(&st.db)
        .await
        .unwrap()
        .is_some()
}

async fn target_row(st: &AppState, cid: &str) -> target::Model {
    target::Entity::find()
        .filter(target::Column::ControllerId.eq(cid))
        .one(&st.db)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn unset_by_default_retires_nothing() {
    let (app, st) = common::setup().await;
    let (v1, ..) = release(&app, "fw", "1").await;
    release(&app, "fw", "2").await;

    assert_eq!(run_sweep(&st).await.unwrap(), Retired::default());
    assert!(ds_exists(&st, v1).await);
}

/// The point of the feature: per name, the newest N stay and everything older
/// goes along with its modules and the blobs on disk. Undeployed sets count
/// towards N, and other names are judged on their own.
#[tokio::test]
async fn keeps_the_newest_n_per_name_and_frees_the_rest() {
    let (app, st) = common::setup_with_retention(keep(2)).await;
    let (v1, m1, sha1) = release(&app, "fw", "1").await;
    let (v2, m2, sha2) = release(&app, "fw", "2").await;
    let (v3, m3, sha3) = release(&app, "fw", "3").await;
    let (v4, _, sha4) = release(&app, "fw", "4").await;
    let (other, _, _) = release(&app, "app", "1").await;

    let r = run_sweep(&st).await.unwrap();
    assert_eq!(
        r,
        Retired {
            sets: 2,
            modules: 2,
            blobs: 2
        }
    );
    for (id, m, sha) in [(v1, m1, &sha1), (v2, m2, &sha2)] {
        assert!(!ds_exists(&st, id).await);
        assert!(!module_exists(&st, m).await);
        assert!(!st.store.exists(sha).await.unwrap());
    }
    for id in [v3, v4, other] {
        assert!(ds_exists(&st, id).await);
    }
    assert!(module_exists(&st, m3).await);
    assert!(st.store.exists(&sha3).await.unwrap());
    assert!(st.store.exists(&sha4).await.unwrap());

    // Retired sets are gone over the API too, and a second sweep is a no-op.
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/distributionsets/{v1}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(run_sweep(&st).await.unwrap(), Retired::default());
}

/// A module shared with a kept release, and a blob shared by hash with one,
/// both survive.
#[tokio::test]
async fn shared_modules_and_blobs_survive() {
    let (app, st) = common::setup_with_retention(keep(1)).await;
    let (shared, shared_sha) = module(&app, "bootloader", b"boot").await;
    let (own, _) = module(&app, "rootfs-1", b"same bytes").await;
    let (dup, dup_sha) = module(&app, "rootfs-2", b"same bytes").await;
    let v1 = ds(&app, "fw", "1", &[shared, own]).await;
    let v2 = ds(&app, "fw", "2", &[shared, dup]).await;

    let r = run_sweep(&st).await.unwrap();
    assert_eq!(
        r,
        Retired {
            sets: 1,
            modules: 1,
            blobs: 0
        }
    );
    assert!(!ds_exists(&st, v1).await);
    assert!(ds_exists(&st, v2).await);
    assert!(!module_exists(&st, own).await);
    assert!(module_exists(&st, shared).await);
    assert!(st.store.exists(&shared_sha).await.unwrap());
    assert!(st.store.exists(&dup_sha).await.unwrap());
}

/// A deployed release goes with its rollout and actions, and no target is
/// left pointing at it: installed references clear, a pending assignment
/// falls back to what is installed, and status follows.
#[tokio::test]
async fn a_deployed_release_is_torn_down_and_targets_reset() {
    let (app, st) = common::setup_with_retention(keep(1)).await;
    let (old, ..) = release(&app, "fw", "1").await;
    let (new, ..) = release(&app, "fw", "2").await;
    for cid in ["dev-installed", "dev-pending", "dev-upgraded"] {
        target(&app, cid).await;
    }
    // Runs the old release, nothing else.
    let a = assign(&app, "dev-installed", old).await;
    install(&st, a).await;
    // Mid-way through getting the old release, nothing installed yet.
    assign(&app, "dev-pending", old).await;
    // Runs the new release but was then sent back to the old one.
    let a = assign(&app, "dev-upgraded", new).await;
    install(&st, a).await;
    assign(&app, "dev-upgraded", old).await;
    // And a rollout of the old release.
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/rollouts",
            Some(json!({
                "name": "r1",
                "distributionSetId": old,
                "targetFilterQuery": "controllerId==dev-*",
                "amountGroups": 1,
                "successCondition": {"condition": "THRESHOLD", "expression": "100"},
            })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    assert_eq!(run_sweep(&st).await.unwrap().sets, 1);
    assert!(!ds_exists(&st, old).await);
    assert_eq!(
        action::Entity::find()
            .filter(action::Column::DsId.eq(old))
            .count(&st.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(rollout::Entity::find().count(&st.db).await.unwrap(), 0);

    let t = target_row(&st, "dev-installed").await;
    assert_eq!((t.assigned_ds_id, t.installed_ds_id), (None, None));
    assert_eq!(t.update_status, "registered");
    let t = target_row(&st, "dev-pending").await;
    assert_eq!((t.assigned_ds_id, t.installed_ds_id), (None, None));
    assert_eq!(t.update_status, "registered");
    let t = target_row(&st, "dev-upgraded").await;
    assert_eq!(
        (t.assigned_ds_id, t.installed_ds_id),
        (Some(new), Some(new))
    );
    assert_eq!(t.update_status, "in_sync");

    // The device whose action vanished sees nothing to do on its next poll.
    let resp = app
        .clone()
        .oneshot(
            Request::get("/DEFAULT/controller/v1/dev-pending")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert!(body["_links"]["deploymentBase"].is_null());
}

/// Only valid sets count towards N, so an invalidated release never eats a
/// rollback slot; and too few valid releases means nothing goes.
#[tokio::test]
async fn invalid_sets_do_not_count_towards_n() {
    let (app, st) = common::setup_with_retention(keep(2)).await;
    let (v1, ..) = release(&app, "fw", "1").await;
    let (v2, ..) = release(&app, "fw", "2").await;
    let (v3, ..) = release(&app, "fw", "3").await;
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/distributionsets/{v3}/invalidate"),
            Some(json!({"actionCancelationType": "none"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Two valid releases left, two kept: nothing to retire.
    assert_eq!(run_sweep(&st).await.unwrap(), Retired::default());
    for id in [v1, v2, v3] {
        assert!(ds_exists(&st, id).await);
    }

    // A fourth release pushes v1 out; the invalid v3 is newer than the oldest
    // kept set, so it stays for now.
    let (v4, ..) = release(&app, "fw", "4").await;
    assert_eq!(run_sweep(&st).await.unwrap().sets, 1);
    assert!(!ds_exists(&st, v1).await);
    for id in [v2, v3, v4] {
        assert!(ds_exists(&st, id).await);
    }
}
