mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

const BOUNDARY: &str = "raptorboundary";

fn upload(uri: &str, filename: &str, content: &[u8]) -> Request<Body> {
    use axum::http::header;
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

/// Fixture: os module w/ artifact "fw.bin" (b"hello world"), ds "stable:1.0", target d1, forced assignment.
/// Returns (module_id, action_id).
async fn deploy_fixture(app: &axum::Router) -> (i64, i64) {
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
    app.clone()
        .oneshot(upload(
            &format!("/rest/v1/softwaremodules/{sm}/artifacts"),
            "fw.bin",
            b"hello world",
        ))
        .await
        .unwrap();
    let ds = common::body_json(app.clone().oneshot(common::req("POST", "/rest/v1/distributionsets",
        Some(json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}])))).await.unwrap()).await[0]["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "d1"}])),
        ))
        .await
        .unwrap();
    let r = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets/d1/assignedDS",
                Some(json!({"id": ds, "type": "forced"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    (sm, r["assignedActions"][0]["id"].as_i64().unwrap())
}

/// TLS-only deployment: `download-http` has no plain-HTTP address to point at,
/// so it reuses the https base rather than advertising a closed port.
#[tokio::test]
async fn tls_only_keeps_both_link_families_on_https() {
    let (_, state) = common::setup().await;
    let mut cfg = state.cfg.clone();
    cfg.url = Some("https://ota.example.com".into());
    let state = raptor::state::AppState::new(state.db.clone(), cfg, state.store.clone());
    let app = raptor::app::build_app(state);
    let (sm, action_id) = deploy_fixture(&app).await;

    let body = common::body_json(
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
    .await;
    let l = &body["deployment"]["chunks"][0]["artifacts"][0]["_links"];
    let https = format!(
        "https://ota.example.com/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin"
    );
    assert_eq!(l["download"]["href"], https);
    assert_eq!(l["download-http"]["href"], https);
}

/// With a plain-HTTP address configured, `download-http` becomes the http
/// variant and `download` stays https — hawkBit's convention, and what lets an
/// operator hand device downloads to a plain-HTTP frontend.
#[tokio::test]
async fn artifact_http_url_splits_the_download_links_by_scheme() {
    let (_, state) = common::setup().await;
    let mut cfg = state.cfg.clone();
    cfg.url = Some("https://ota.example.com".into());
    cfg.ddi.artifact_http_url = Some("http://dl.example.com:8080/".into());
    let state = raptor::state::AppState::new(state.db.clone(), cfg, state.store.clone());
    let app = raptor::app::build_app(state);
    let (sm, action_id) = deploy_fixture(&app).await;

    let body = common::body_json(
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
    .await;
    let l = &body["deployment"]["chunks"][0]["artifacts"][0]["_links"];
    let path = format!("DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin");
    // trailing slash on the configured URL must not double up
    assert_eq!(
        l["download-http"]["href"],
        format!("http://dl.example.com:8080/{path}")
    );
    assert_eq!(
        l["md5sum-http"]["href"],
        format!("http://dl.example.com:8080/{path}.MD5SUM")
    );
    assert_eq!(
        l["download"]["href"],
        format!("https://ota.example.com/{path}")
    );
    assert_eq!(
        l["md5sum"]["href"],
        format!("https://ota.example.com/{path}.MD5SUM")
    );

    // the artifact listing endpoint agrees with deploymentBase
    let listed = common::body_json(
        app.clone()
            .oneshot(
                Request::get(format!(
                    "/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        listed[0]["_links"]["download-http"]["href"],
        format!("http://dl.example.com:8080/{path}")
    );
}

#[tokio::test]
async fn deployment_base_matches_hawkbit_shape() {
    let (app, _) = common::setup().await;
    let (sm, action_id) = deploy_fixture(&app).await;

    let resp = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/DEFAULT/controller/v1/d1/deploymentBase/{action_id}"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;

    // golden shape check (actionHistory checked loosely)
    let ddi = "http://localhost:8080/DEFAULT/controller/v1/d1";
    assert_eq!(body["id"], action_id.to_string());
    assert_eq!(body["deployment"]["download"], "forced");
    assert_eq!(body["deployment"]["update"], "forced");
    let chunk = &body["deployment"]["chunks"][0];
    assert_eq!(chunk["part"], "os");
    assert_eq!(chunk["version"], "1.0");
    assert_eq!(chunk["name"], "fw");
    let art = &chunk["artifacts"][0];
    assert_eq!(art["filename"], "fw.bin");
    assert_eq!(art["size"], 11);
    assert_eq!(
        art["hashes"]["sha1"],
        "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"
    );
    assert_eq!(art["hashes"]["md5"], "5eb63bbbe01eeed093cb22bb8f5acdc3");
    assert_eq!(
        art["hashes"]["sha256"],
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
    assert_eq!(
        art["_links"]["download-http"]["href"],
        format!("{ddi}/softwaremodules/{sm}/artifacts/fw.bin")
    );
    assert_eq!(
        art["_links"]["md5sum-http"]["href"],
        format!("{ddi}/softwaremodules/{sm}/artifacts/fw.bin.MD5SUM")
    );

    // wrong controller -> 404
    let resp = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/DEFAULT/controller/v1/other/deploymentBase/{action_id}"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn action_history_messages_ordered_newest_first() {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    let (app, state) = common::setup().await;
    let (_, action_id) = deploy_fixture(&app).await;

    // Insert two action_status rows (proceeding, then download in chronological order)
    // They will have sequential IDs, so second will have higher ID (newer)
    let status1 = raptor::entity::action_status::ActiveModel {
        action_id: Set(action_id),
        status: Set("proceeding".into()),
        created_at: Set(1000),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();

    let status2 = raptor::entity::action_status::ActiveModel {
        action_id: Set(action_id),
        status: Set("download".into()),
        created_at: Set(2000),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();

    // Insert messages for status1: m1, m2 (in that order, so m2 has higher ID)
    raptor::entity::action_status_message::ActiveModel {
        action_status_id: Set(status1.id),
        message: Set("m1".into()),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();

    raptor::entity::action_status_message::ActiveModel {
        action_status_id: Set(status1.id),
        message: Set("m2".into()),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();

    // Insert messages for status2: m3, m4 (in that order, so m4 has higher ID)
    raptor::entity::action_status_message::ActiveModel {
        action_status_id: Set(status2.id),
        message: Set("m3".into()),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();

    raptor::entity::action_status_message::ActiveModel {
        action_status_id: Set(status2.id),
        message: Set("m4".into()),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();

    // GET deploymentBase
    let resp = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/DEFAULT/controller/v1/d1/deploymentBase/{action_id}"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;

    // Assert latest status is "DOWNLOAD" (uppercased, from status2 which has higher ID)
    assert_eq!(body["actionHistory"]["status"], "DOWNLOAD");

    // Assert messages are strictly newest-first: m4, m3, m2, m1
    // status2 messages (m4, m3) then status1 messages (m2, m1)
    let messages = &body["actionHistory"]["messages"];
    assert_eq!(messages.as_array().map(|a| a.len()), Some(4));
    assert_eq!(messages[0], "m4");
    assert_eq!(messages[1], "m3");
    assert_eq!(messages[2], "m2");
    assert_eq!(messages[3], "m1");
}
