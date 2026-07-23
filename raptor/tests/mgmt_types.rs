mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

/// Look up a seeded type's id by key from a list endpoint.
async fn type_id_by_key(app: &axum::Router, path: &str, key: &str) -> i64 {
    let body = common::body_json(
        app.clone()
            .oneshot(common::req("GET", path, None))
            .await
            .unwrap(),
    )
    .await;
    body["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == key)
        .unwrap_or_else(|| panic!("type {key} not found in {path}"))["id"]
        .as_i64()
        .unwrap()
}

async fn post(
    app: &axum::Router,
    path: &str,
    body: serde_json::Value,
) -> axum::http::Response<axum::body::Body> {
    app.clone()
        .oneshot(common::req("POST", path, Some(body)))
        .await
        .unwrap()
}

// --------------------------------------------------------------------------
// Software module types
// --------------------------------------------------------------------------

#[tokio::test]
async fn sm_type_crud_and_delete_in_use_conflicts() {
    let (app, _) = common::setup().await;

    // Create
    let resp = post(
        &app,
        "/rest/v1/softwaremoduletypes",
        json!([{"key": "container", "name": "Container", "maxAssignments": 3}]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = common::body_json(resp).await;
    let id = created[0]["id"].as_i64().unwrap();
    assert_eq!(created[0]["key"], "container");
    assert_eq!(created[0]["maxAssignments"], 3);
    assert_eq!(created[0]["deleted"], false);

    // Duplicate key -> 409
    let resp = post(
        &app,
        "/rest/v1/softwaremoduletypes",
        json!([{"key": "container", "name": "Dup"}]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Update description
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            &format!("/rest/v1/softwaremoduletypes/{id}"),
            Some(json!({"description": "OCI images"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["description"], "OCI images");

    // A module of this type blocks deletion
    post(
        &app,
        "/rest/v1/softwaremodules",
        json!([{"name": "img", "version": "1", "type": "container"}]),
    )
    .await;
    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/softwaremoduletypes/{id}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // An unused type deletes fine
    let unused = common::body_json(
        post(
            &app,
            "/rest/v1/softwaremoduletypes",
            json!([{"key": "data", "name": "Data"}]),
        )
        .await,
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/softwaremoduletypes/{unused}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// --------------------------------------------------------------------------
// Distribution set types: composition drives `complete`
// --------------------------------------------------------------------------

#[tokio::test]
async fn ds_type_composition_derives_complete() {
    let (app, _) = common::setup().await;
    let firmware = type_id_by_key(&app, "/rest/v1/softwaremoduletypes", "firmware").await;
    let application = type_id_by_key(&app, "/rest/v1/softwaremoduletypes", "application").await;

    // New DS type: firmware mandatory, application optional.
    let resp = post(
        &app,
        "/rest/v1/distributionsettypes",
        json!([{
            "key": "kernel", "name": "Kernel",
            "mandatorymodules": [{"id": firmware}],
            "optionalmodules": [{"id": application}]
        }]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let kernel_id = common::body_json(resp).await[0]["id"].as_i64().unwrap();

    // Sub-resource lists reflect the composition.
    let mand = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/distributionsettypes/{kernel_id}/mandatorymoduletypes"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(mand["total"], 1);
    assert_eq!(mand["content"][0]["key"], "firmware");
    let opt = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/distributionsettypes/{kernel_id}/optionalmoduletypes"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(opt["total"], 1);
    assert_eq!(opt["content"][0]["key"], "application");

    // A firmware module satisfies the mandatory type -> complete.
    let fw = common::body_json(
        post(
            &app,
            "/rest/v1/softwaremodules",
            json!([{"name": "fw", "version": "1", "type": "firmware"}]),
        )
        .await,
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let complete = common::body_json(
        post(
            &app,
            "/rest/v1/distributionsets",
            json!([{"name": "k", "version": "1", "type": "kernel", "modules": [{"id": fw}]}]),
        )
        .await,
    )
    .await;
    assert_eq!(complete[0]["complete"], true);

    // Without the mandatory firmware module -> incomplete, even with an
    // (optional) application module present.
    let appmod = common::body_json(
        post(
            &app,
            "/rest/v1/softwaremodules",
            json!([{"name": "a", "version": "1", "type": "application"}]),
        )
        .await,
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let incomplete = common::body_json(
        post(
            &app,
            "/rest/v1/distributionsets",
            json!([{"name": "k2", "version": "1", "type": "kernel", "modules": [{"id": appmod}]}]),
        )
        .await,
    )
    .await;
    assert_eq!(incomplete[0]["complete"], false);
}

#[tokio::test]
async fn ds_type_delete_in_use_conflicts() {
    let (app, _) = common::setup().await;
    let resp = post(
        &app,
        "/rest/v1/distributionsettypes",
        json!([{"key": "custom", "name": "Custom"}]),
    )
    .await;
    let id = common::body_json(resp).await[0]["id"].as_i64().unwrap();

    // A DS of this type (custom has no mandatory modules -> trivially complete).
    post(
        &app,
        "/rest/v1/distributionsets",
        json!([{"name": "d", "version": "1", "type": "custom"}]),
    )
    .await;
    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/distributionsettypes/{id}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// --------------------------------------------------------------------------
// Target types: compatibility constrains assignment
// --------------------------------------------------------------------------

async fn complete_os_ds(app: &axum::Router, name: &str) -> i64 {
    let sm = common::body_json(
        post(
            app,
            "/rest/v1/softwaremodules",
            json!([{"name": format!("{name}-os"), "version": "1", "type": "os"}]),
        )
        .await,
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    common::body_json(
        post(
            app,
            "/rest/v1/distributionsets",
            json!([{"name": name, "version": "1", "type": "os", "modules": [{"id": sm}]}]),
        )
        .await,
    )
    .await[0]["id"]
        .as_i64()
        .unwrap()
}

#[tokio::test]
async fn typed_target_rejects_incompatible_ds() {
    let (app, _) = common::setup().await;
    let os_ds_type = type_id_by_key(&app, "/rest/v1/distributionsettypes", "os").await;

    // Target type compatible only with the "os" DS type.
    let tt = common::body_json(
        post(
            &app,
            "/rest/v1/targettypes",
            json!([{"name": "gateway", "colour": "#ff0000",
                    "compatibledistributionsettypes": [{"id": os_ds_type}]}]),
        )
        .await,
    )
    .await;
    let tt_id = tt[0]["id"].as_i64().unwrap();
    assert_eq!(tt[0]["deleted"], false);

    // Compatibility sub-resource lists the os DS type.
    let compat = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                &format!("/rest/v1/targettypes/{tt_id}/compatibledistributionsettypes"),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(compat["total"], 1);
    assert_eq!(compat["content"][0]["key"], "os");

    // Typed target.
    post(
        &app,
        "/rest/v1/targets",
        json!([{"controllerId": "gw-1", "targetType": tt_id}]),
    )
    .await;
    let t = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/targets/gw-1", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(t["targetType"], tt_id);

    // Compatible os DS assigns fine.
    let os_ds = complete_os_ds(&app, "os-set").await;
    let resp = post(
        &app,
        "/rest/v1/targets/gw-1/assignedDS",
        json!({"id": os_ds}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["assigned"], 1);

    // An app-typed DS is incompatible -> rejected.
    let appmod = common::body_json(
        post(
            &app,
            "/rest/v1/softwaremodules",
            json!([{"name": "a", "version": "1", "type": "application"}]),
        )
        .await,
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let app_ds = common::body_json(
        post(
            &app,
            "/rest/v1/distributionsets",
            json!([{"name": "app-set", "version": "1", "type": "app", "modules": [{"id": appmod}]}]),
        )
        .await,
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let resp = post(
        &app,
        "/rest/v1/targets/gw-1/assignedDS",
        json!({"id": app_ds}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn target_type_assign_unassign_and_delete_in_use() {
    let (app, _) = common::setup().await;
    let tt_id =
        common::body_json(post(&app, "/rest/v1/targettypes", json!([{"name": "sensor"}])).await)
            .await[0]["id"]
            .as_i64()
            .unwrap();

    post(&app, "/rest/v1/targets", json!([{"controllerId": "s-1"}])).await;

    // Assign the type via the sub-resource.
    let resp = post(
        &app,
        "/rest/v1/targets/s-1/targettype",
        json!({"id": tt_id}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Type is now in use -> delete 409.
    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/targettypes/{tt_id}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Unassign, then deletion succeeds.
    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            "/rest/v1/targets/s-1/targettype",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/targettypes/{tt_id}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
