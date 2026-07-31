mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn create_list_update_delete_target() {
    let (app, _) = common::setup().await;

    let resp = app.clone().oneshot(common::req("POST", "/rest/v1/targets",
        Some(json!([{"controllerId": "dev-1"}, {"controllerId": "dev-2", "name": "Device 2", "securityToken": "tok2"}])))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = common::body_json(resp).await;
    assert_eq!(body[0]["controllerId"], "dev-1");
    assert_eq!(body[0]["name"], "dev-1"); // defaults to controllerId
    assert_eq!(body[0]["updateStatus"], "unknown");
    assert_eq!(body[0]["securityToken"].as_str().unwrap().len(), 32);
    assert_eq!(body[1]["securityToken"], "tok2");
    assert!(body[0]["pollStatus"].is_null());

    // FIQL filter
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            "/rest/v1/targets?q=controllerId==dev-2",
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["content"][0]["name"], "Device 2");

    // update
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            "/rest/v1/targets/dev-1",
            Some(json!({"description": "first device"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["description"], "first device");

    // attributes empty map
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            "/rest/v1/targets/dev-1/attributes",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(common::body_json(resp).await, json!({}));

    // delete
    assert_eq!(
        app.clone()
            .oneshot(common::req("DELETE", "/rest/v1/targets/dev-1", None))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/dev-1", None))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn duplicate_controller_id_conflicts() {
    let (app, _) = common::setup().await;
    let t = json!([{"controllerId": "dup"}]);
    assert_eq!(
        app.clone()
            .oneshot(common::req("POST", "/rest/v1/targets", Some(t.clone())))
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        app.clone()
            .oneshot(common::req("POST", "/rest/v1/targets", Some(t)))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn partial_write_prevented_on_bulk_conflict() {
    let (app, _) = common::setup().await;

    // Create "already-there" beforehand
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "already-there"}])),
        ))
        .await
        .unwrap();

    // POST array with brand-new and already-there (conflict)
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "brand-new"}, {"controllerId": "already-there"}])),
        ))
        .await
        .unwrap();

    // Should return 409 because second item conflicts
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Verify brand-new was NOT created (no partial write)
    let check = app
        .clone()
        .oneshot(common::req("GET", "/rest/v1/targets/brand-new", None))
        .await
        .unwrap();
    assert_eq!(check.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn duplicate_within_request_rejected() {
    let (app, _) = common::setup().await;

    // POST array with two identical brand-new controllerIds (duplicate within same request)
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "dup-in-req"}, {"controllerId": "dup-in-req"}])),
        ))
        .await
        .unwrap();

    // Should return 409 Conflict, not 500
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Verify nothing was persisted (404 on GET)
    let check = app
        .clone()
        .oneshot(common::req("GET", "/rest/v1/targets/dup-in-req", None))
        .await
        .unwrap();
    assert_eq!(check.status(), StatusCode::NOT_FOUND);
}

/// The target list carries the installed/assigned set and the target's tags
/// inline, so a console can render them without a request per row (#70).
#[tokio::test]
async fn target_list_embeds_set_and_tag_refs() {
    let (app, _) = common::setup().await;

    let sm = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/softwaremodules",
                Some(json!([{"name": "rootfs", "version": "3.4.0", "type": "os"}])),
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
                    json!([{"name": "gw-linux", "version": "2026.07", "type": "os", "modules": [{"id": sm}]}]),
                ),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let tag = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targettags",
                Some(json!([{"name": "linux-gw", "colour": "#4f9cf9"}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();

    for cid in ["gw-a", "gw-b"] {
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets",
                Some(json!([{"controllerId": cid}])),
            ))
            .await
            .unwrap();
    }
    // Assign the set to one of them, and tag it.
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/gw-a/assignedDS",
            Some(json!({"id": ds, "type": "forced"})),
        ))
        .await
        .unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            &format!("/rest/v1/targettags/{tag}/assigned/gw-a"),
            None,
        ))
        .await
        .unwrap();

    let list = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets", None))
            .await
            .unwrap(),
    )
    .await;
    let row = |cid: &str| {
        list["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["controllerId"] == cid)
            .cloned()
            .unwrap()
    };

    let a = row("gw-a");
    assert_eq!(a["assignedDs"]["id"], ds);
    assert_eq!(a["assignedDs"]["name"], "gw-linux");
    assert_eq!(a["assignedDs"]["version"], "2026.07");
    // The type key, not the numeric id — it is what says whether a device class
    // can install this set at all.
    assert_eq!(a["assignedDs"]["type"], "os");
    assert_eq!(a["tags"][0]["name"], "linux-gw");
    assert_eq!(a["tags"][0]["colour"], "#4f9cf9");
    // Nothing installed until the device reports it, and that is distinct from
    // "no assignment" — the field is absent rather than null-ish.
    assert!(a.get("installedDs").is_none());

    // The untouched target keeps the pre-#70 shape: absent, not empty objects.
    let b = row("gw-b");
    assert!(b.get("assignedDs").is_none());
    assert!(b.get("installedDs").is_none());
    assert!(b.get("tags").is_none());

    // The single-target read returns the same shape, so a consumer does not have
    // to special-case it.
    let one = common::body_json(
        app.oneshot(common::req("GET", "/rest/v1/targets/gw-a", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(one["assignedDs"], a["assignedDs"]);
    assert_eq!(one["tags"], a["tags"]);
}
