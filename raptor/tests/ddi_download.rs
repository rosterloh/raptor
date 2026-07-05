mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

const BOUNDARY: &str = "raptorboundary";

fn upload(uri: &str, filename: &str, content: &[u8]) -> Request<Body> {
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

async fn fixture(app: &axum::Router) -> (i64, i64) {
    let sm = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/softwaremodules",
                Some(json!([{"name": "fw", "version": "1", "type": "os"}])),
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
    let ds = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/distributionsets",
                Some(json!([{"name": "r", "version": "1", "type": "os", "modules": [{"id": sm}]}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
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
                Some(json!({"id": ds})),
            ))
            .await
            .unwrap(),
    )
    .await;
    (sm, r["assignedActions"][0]["id"].as_i64().unwrap())
}

#[tokio::test]
async fn full_download_and_md5sum() {
    let (app, _) = common::setup().await;
    let (sm, _) = fixture(&app).await;

    let resp = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("accept-ranges").unwrap(), "bytes");
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(&bytes[..], b"hello world");

    let resp = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin.MD5SUM"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(&bytes[..], b"5eb63bbbe01eeed093cb22bb8f5acdc3  fw.bin\n");
}

#[tokio::test]
async fn range_download_resumes() {
    let (app, _) = common::setup().await;
    let (sm, _) = fixture(&app).await;
    let uri = format!("/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin");

    // open-ended range
    let resp = app
        .clone()
        .oneshot(
            Request::get(&uri)
                .header(header::RANGE, "bytes=6-")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get("content-range").unwrap(),
        "bytes 6-10/11"
    );
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(&bytes[..], b"world");

    // bounded range
    let resp = app
        .clone()
        .oneshot(
            Request::get(&uri)
                .header(header::RANGE, "bytes=0-4")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(&bytes[..], b"hello");

    // unsatisfiable
    let resp = app
        .clone()
        .oneshot(
            Request::get(&uri)
                .header(header::RANGE, "bytes=99-")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
}

#[tokio::test]
async fn installed_base_serves_finished_deployment() {
    let (app, _) = common::setup().await;
    let (_, aid) = fixture(&app).await;

    // not finished yet -> 404
    let resp = app
        .clone()
        .oneshot(
            Request::get(format!("/DEFAULT/controller/v1/d1/installedBase/{aid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // finish it
    let fb = json!({"status": {"execution": "closed", "result": {"finished": "success"}}});
    app.clone()
        .oneshot(
            Request::post(format!(
                "/DEFAULT/controller/v1/d1/deploymentBase/{aid}/feedback"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(fb.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::get(format!("/DEFAULT/controller/v1/d1/installedBase/{aid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["id"], aid.to_string());
    assert_eq!(body["deployment"]["chunks"][0]["name"], "fw");
}

#[tokio::test]
async fn artifact_list_matches_deployment_base_shape() {
    let (app, _) = common::setup().await;
    let (sm, aid) = fixture(&app).await;

    // get artifact list
    let resp = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list_body = common::body_json(resp).await;
    assert!(list_body.is_array());
    let list_artifact = &list_body[0];

    // get deploymentBase
    let resp = app
        .clone()
        .oneshot(
            Request::get(format!("/DEFAULT/controller/v1/d1/deploymentBase/{aid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let deployment = common::body_json(resp).await;
    let deployment_artifact = &deployment["deployment"]["chunks"][0]["artifacts"][0];

    // on http config, both should be identical
    assert_eq!(
        list_artifact, deployment_artifact,
        "artifact JSON shape must be identical on http"
    );
}
