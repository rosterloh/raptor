//! Quota enforcement (#14): each configured `[quota]` cap, plus the hawkBit
//! wire shape a violation produces.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use raptor::config::QuotaConfig;
use serde_json::json;
use tower::ServiceExt;

const BOUNDARY: &str = "raptorboundary";

fn multipart_upload(uri: &str, filename: &str, content: &[u8]) -> Request<Body> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post(uri)
        .header(header::AUTHORIZATION, common::mgmt_auth_header())
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap()
}

async fn create_module(app: &axum::Router, name: &str) -> i64 {
    common::body_json(
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

/// hawkBit answers a quota breach with 429 and
/// `hawkbit.server.error.quota.tooManyEntries` — verified against its own REST
/// tests, which assert `status().isTooManyRequests()`. The issue text's "403"
/// predates hawkBit 0.10.
#[tokio::test]
async fn a_breach_has_hawkbits_status_and_error_body() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_artifacts_per_software_module: 1,
        ..Default::default()
    })
    .await;
    let sm = create_module(&app, "fw").await;

    let ok = app
        .clone()
        .oneshot(multipart_upload(
            &format!("/rest/v1/softwaremodules/{sm}/artifacts"),
            "one.bin",
            b"first",
        ))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::CREATED);

    let resp = app
        .clone()
        .oneshot(multipart_upload(
            &format!("/rest/v1/softwaremodules/{sm}/artifacts"),
            "two.bin",
            b"second",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    let body = common::body_json(resp).await;
    assert_eq!(
        body["errorCode"],
        "hawkbit.server.error.quota.tooManyEntries"
    );
    assert_eq!(
        body["exceptionClass"],
        "org.eclipse.hawkbit.repository.exception.AssignmentQuotaExceededException"
    );
    // The message has to name the cap for an operator to act on it.
    assert!(
        body["message"].as_str().unwrap().contains("maximum is 1"),
        "{}",
        body["message"]
    );
}

/// hawkBit's `QuotaHelper` treats any limit <= 0 as unlimited; raptor's config
/// is unsigned, so 0 carries that whole meaning.
#[tokio::test]
async fn zero_disables_a_quota() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_artifacts_per_software_module: 0,
        ..Default::default()
    })
    .await;
    let sm = create_module(&app, "fw").await;
    for i in 0..5 {
        let resp = app
            .clone()
            .oneshot(multipart_upload(
                &format!("/rest/v1/softwaremodules/{sm}/artifacts"),
                &format!("f{i}.bin"),
                format!("content-{i}").as_bytes(),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED, "upload {i}");
    }
}

/// The default config must not reject anything a small deployment does — the
/// quotas are a runaway guard, not a workflow limit.
#[tokio::test]
async fn hawkbit_defaults_do_not_reject_ordinary_use() {
    let (app, _) = common::setup().await;
    let sm = create_module(&app, "fw").await;
    let resp = app
        .clone()
        .oneshot(multipart_upload(
            &format!("/rest/v1/softwaremodules/{sm}/artifacts"),
            "rootfs.img",
            b"payload",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    create_target(&app, "dev-1").await;
    let meta = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/dev-1/metadata",
            Some(json!([{"key": "site", "value": "plant-a"}])),
        ))
        .await
        .unwrap();
    assert_eq!(meta.status(), StatusCode::CREATED);
}

/// A batch is rejected whole rather than filled up to the cap and truncated.
#[tokio::test]
async fn a_metadata_batch_that_would_breach_is_rejected_entirely() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_metadata_entries_per_target: 2,
        ..Default::default()
    })
    .await;
    create_target(&app, "dev-1").await;

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/dev-1/metadata",
            Some(json!([
                {"key": "a", "value": "1"},
                {"key": "b", "value": "2"},
                {"key": "c", "value": "3"}
            ])),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // Nothing was written: the check runs before the inserts.
    let list = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/dev-1/metadata", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(list["total"], 0);
}

/// Metadata counts against what the owner already holds, not just the batch.
#[tokio::test]
async fn metadata_quota_counts_existing_entries() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_metadata_entries_per_target: 2,
        ..Default::default()
    })
    .await;
    create_target(&app, "dev-1").await;

    let post = |body: serde_json::Value| {
        let app = app.clone();
        async move {
            app.oneshot(common::req(
                "POST",
                "/rest/v1/targets/dev-1/metadata",
                Some(body),
            ))
            .await
            .unwrap()
            .status()
        }
    };
    assert_eq!(
        post(json!([{"key": "a", "value": "1"}])).await,
        StatusCode::CREATED
    );
    assert_eq!(
        post(json!([{"key": "b", "value": "2"}])).await,
        StatusCode::CREATED
    );
    // Two stored, cap of two: the third is one too many.
    assert_eq!(
        post(json!([{"key": "c", "value": "3"}])).await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn software_modules_per_distribution_set_is_capped() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_software_modules_per_distribution_set: 1,
        ..Default::default()
    })
    .await;
    let a = create_module(&app, "mod-a").await;
    let b = create_module(&app, "mod-b").await;

    // Rejected at creation, when the set would start over the cap.
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/distributionsets",
            Some(json!([{
                "name": "two-modules", "version": "1.0", "type": "os",
                "modules": [{"id": a}, {"id": b}]
            }])),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // ...and on the separate assign route, counting what the set already holds.
    let ds = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/distributionsets",
                Some(json!([{
                    "name": "one-module", "version": "1.0", "type": "os",
                    "modules": [{"id": a}]
                }])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/distributionsets/{ds}/assignedSM"),
            Some(json!([{"id": b}])),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn rollout_group_count_is_capped() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_rollout_groups_per_rollout: 2,
        ..Default::default()
    })
    .await;
    let sm = create_module(&app, "fw").await;
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
    for i in 0..4 {
        create_target(&app, &format!("dev-{i}")).await;
    }

    let create = |groups: i64, name: &str| {
        let app = app.clone();
        let body = json!({
            "name": name, "distributionSetId": ds,
            "targetFilterQuery": "controllerId==dev-*", "amountGroups": groups,
            "successCondition": {"condition": "THRESHOLD", "expression": "100"},
        });
        async move {
            app.oneshot(common::req("POST", "/rest/v1/rollouts", Some(body)))
                .await
                .unwrap()
                .status()
        }
    };
    assert_eq!(create(2, "within").await, StatusCode::CREATED);
    assert_eq!(create(3, "over").await, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn rollout_targets_per_group_is_capped() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_targets_per_rollout_group: 2,
        ..Default::default()
    })
    .await;
    let sm = create_module(&app, "fw").await;
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
    for i in 0..6 {
        create_target(&app, &format!("dev-{i}")).await;
    }

    let create = |groups: i64, name: &str| {
        let app = app.clone();
        let body = json!({
            "name": name, "distributionSetId": ds,
            "targetFilterQuery": "controllerId==dev-*", "amountGroups": groups,
            "successCondition": {"condition": "THRESHOLD", "expression": "100"},
        });
        async move {
            app.oneshot(common::req("POST", "/rest/v1/rollouts", Some(body)))
                .await
                .unwrap()
                .status()
        }
    };
    // 6 targets over 2 groups is 3 per group — one too many.
    assert_eq!(
        create(2, "too-few-groups").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // Splitting the same targets three ways brings each group to 2.
    assert_eq!(create(3, "enough-groups").await, StatusCode::CREATED);
}

// --- DDI-side quotas: what a device can report ------------------------------

/// Sets up one target with one active action, returning its id.
async fn action_fixture(app: &axum::Router) -> i64 {
    let sm = create_module(app, "fw").await;
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
    create_target(app, "d1").await;
    common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/d1/assignedDS",
                Some(json!({"id": ds})),
            ))
            .await
            .unwrap(),
    )
    .await["assignedActions"][0]["id"]
        .as_i64()
        .unwrap()
}

fn feedback(action_id: i64, execution: &str, details: serde_json::Value) -> Request<Body> {
    let uri = format!("/DEFAULT/controller/v1/d1/deploymentBase/{action_id}/feedback");
    let body = json!({
        "id": action_id.to_string(),
        "status": {"execution": execution, "result": {"finished": "none"}, "details": details}
    });
    Request::post(&uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// The unbounded-growth case the issue names: a device appending status rows
/// forever. Assignment writes one entry itself, so the cap is reached after
/// one more report.
#[tokio::test]
async fn a_device_cannot_report_status_entries_forever() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_status_entries_per_action: 2,
        ..Default::default()
    })
    .await;
    let aid = action_fixture(&app).await;

    let resp = app
        .clone()
        .oneshot(feedback(aid, "proceeding", json!(["still going"])))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(feedback(aid, "proceeding", json!(["and again"])))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

/// The carve-out that keeps the quota from bricking a device: a device that
/// has spent its status budget on progress reports must still be able to file
/// the terminal one, or its action stays active forever with no way to close
/// it. hawkBit checks the count "only for intermediate statuses".
#[tokio::test]
async fn a_device_at_its_status_cap_can_still_close_the_action() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_status_entries_per_action: 1,
        ..Default::default()
    })
    .await;
    let aid = action_fixture(&app).await;

    // Cap already reached by the entry assignment wrote: progress is refused.
    let resp = app
        .clone()
        .oneshot(feedback(aid, "proceeding", json!(["still working"])))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // ...but the terminal report goes through, and actually closes the action.
    let uri = format!("/DEFAULT/controller/v1/d1/deploymentBase/{aid}/feedback");
    let body = json!({
        "id": aid.to_string(),
        "status": {"execution": "closed", "result": {"finished": "success"}, "details": ["done"]}
    });
    let resp = app
        .clone()
        .oneshot(
            Request::post(&uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let action = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targets/d1/actions/{aid}"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(action["status"], "finished");
}

#[tokio::test]
async fn a_device_cannot_attach_unlimited_messages_to_one_status() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_messages_per_action_status: 2,
        ..Default::default()
    })
    .await;
    let aid = action_fixture(&app).await;

    let resp = app
        .clone()
        .oneshot(feedback(aid, "proceeding", json!(["one", "two"])))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(feedback(aid, "proceeding", json!(["one", "two", "three"])))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

/// The status-entry quota bounds what a *device* reports. raptor's own entries
/// — here the one recorded when an operator cancels — must still be written,
/// or a chatty device could stop the server recording why it was cancelled.
#[tokio::test]
async fn the_servers_own_status_entries_are_not_capped() {
    let (app, st) = common::setup_with_quota(QuotaConfig {
        max_status_entries_per_action: 1,
        ..Default::default()
    })
    .await;
    let aid = action_fixture(&app).await;

    // The device is already at its cap (assignment wrote entry #1).
    let resp = app
        .clone()
        .oneshot(feedback(aid, "proceeding", json!(["blocked"])))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // An operator cancel still records its own status entry.
    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/targets/d1/actions/{aid}"),
            None,
        ))
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "cancel rejected: {}",
        resp.status()
    );

    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    let entries = raptor::entity::action_status::Entity::find()
        .filter(raptor::entity::action_status::Column::ActionId.eq(aid))
        .count(&st.db)
        .await
        .unwrap();
    assert!(
        entries > 1,
        "server-side status entry was suppressed by the device quota ({entries} entries)"
    );
}

#[tokio::test]
async fn reported_attributes_are_capped() {
    let (app, _) = common::setup_with_quota(QuotaConfig {
        max_attribute_entries_per_target: 2,
        ..Default::default()
    })
    .await;
    create_target(&app, "d1").await;

    let put = |data: serde_json::Value, mode: &str| {
        let app = app.clone();
        let body = json!({"mode": mode, "data": data});
        async move {
            app.oneshot(
                Request::put("/DEFAULT/controller/v1/d1/configData")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        }
    };

    assert_eq!(
        put(json!({"a": "1", "b": "2"}), "merge").await,
        StatusCode::OK
    );
    // Two stored, cap of two: a third distinct key is one too many.
    assert_eq!(
        put(json!({"c": "3"}), "merge").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // `replace` starts from empty, so the same payload that breaches a merge
    // fits — it is not added to what is already there.
    assert_eq!(
        put(json!({"x": "1", "y": "2"}), "replace").await,
        StatusCode::OK
    );
}
