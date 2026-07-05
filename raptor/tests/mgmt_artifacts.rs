mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
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
    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/softwaremodules",
            Some(json!([{"name": name, "version": "1.0", "type": "os"}])),
        ))
        .await
        .unwrap();
    common::body_json(resp).await[0]["id"].as_i64().unwrap()
}

#[tokio::test]
async fn upload_hashes_and_lists_artifact() {
    let (app, _) = common::setup().await;
    let id = create_module(&app, "rootfs").await;

    let resp = app
        .clone()
        .oneshot(multipart_upload(
            &format!("/rest/v1/softwaremodules/{id}/artifacts"),
            "fw.bin",
            b"hello world",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let a = common::body_json(resp).await;
    assert_eq!(a["providedFilename"], "fw.bin");
    assert_eq!(a["size"], 11);
    assert_eq!(a["hashes"]["md5"], "5eb63bbbe01eeed093cb22bb8f5acdc3");
    assert_eq!(
        a["hashes"]["sha256"],
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );

    // list is a bare array
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/softwaremodules/{id}/artifacts"),
            None,
        ))
        .await
        .unwrap();
    let list = common::body_json(resp).await;
    assert!(list.is_array());
    assert_eq!(list.as_array().unwrap().len(), 1);

    // download round-trips content
    let aid = a["id"].as_i64().unwrap();
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            &format!("/rest/v1/softwaremodules/{id}/artifacts/{aid}/download"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(&bytes[..], b"hello world");

    // duplicate filename conflicts
    let resp = app
        .clone()
        .oneshot(multipart_upload(
            &format!("/rest/v1/softwaremodules/{id}/artifacts"),
            "fw.bin",
            b"other",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn blob_refcounting_on_delete() {
    let (app, state) = common::setup().await;
    let m1 = create_module(&app, "m1").await;
    let m2 = create_module(&app, "m2").await;
    let sha256 = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

    let a1 = common::body_json(
        app.clone()
            .oneshot(multipart_upload(
                &format!("/rest/v1/softwaremodules/{m1}/artifacts"),
                "a.bin",
                b"hello world",
            ))
            .await
            .unwrap(),
    )
    .await;
    let _a2 = common::body_json(
        app.clone()
            .oneshot(multipart_upload(
                &format!("/rest/v1/softwaremodules/{m2}/artifacts"),
                "b.bin",
                b"hello world",
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(state.store.path_for(sha256).exists());

    // delete first reference: blob stays
    let aid1 = a1["id"].as_i64().unwrap();
    app.clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/softwaremodules/{m1}/artifacts/{aid1}"),
            None,
        ))
        .await
        .unwrap();
    assert!(state.store.path_for(sha256).exists());

    // delete the module holding the second reference: blob goes
    app.clone()
        .oneshot(common::req(
            "DELETE",
            &format!("/rest/v1/softwaremodules/{m2}"),
            None,
        ))
        .await
        .unwrap();
    assert!(!state.store.path_for(sha256).exists());
}

#[tokio::test]
async fn large_body_rejected_on_json_routes() {
    let (app, _) = common::setup().await;

    // Create a 3 MiB junk description string to exceed the default limit (typically 2 MiB)
    let large_description = "x".repeat(3 * 1024 * 1024);
    let payload = serde_json::json!([{
        "name": "test_module",
        "version": "1.0",
        "type": "os",
        "description": large_description
    }]);

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/softwaremodules",
            Some(payload),
        ))
        .await
        .unwrap();

    // The body should be rejected for being too large before the handler runs
    // Axum may return 413 Payload Too Large or 400 Bad Request depending on extractor
    let status = resp.status();
    assert!(
        status.is_client_error(),
        "Expected 4xx client error, got {} ({})",
        status.as_u16(),
        status.canonical_reason().unwrap_or("unknown")
    );
    assert_ne!(status, StatusCode::OK, "Large body should not succeed");
    assert_ne!(status, StatusCode::CREATED, "Large body should not succeed");
    assert_ne!(
        status,
        StatusCode::CONFLICT,
        "Large body should not succeed"
    );
    assert_ne!(
        status,
        StatusCode::NOT_FOUND,
        "Large body should not succeed"
    );
}
