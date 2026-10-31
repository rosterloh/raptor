mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
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

async fn get_range(app: &axum::Router, uri: &str, range: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::get(uri)
                .header(header::RANGE, range)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn suffix_range_serves_tail() {
    let (app, _) = common::setup().await;
    let (sm, _) = fixture(&app).await;
    let uri = format!("/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin");

    let resp = get_range(&app, &uri, "bytes=-5").await;
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
}

/// Golden `multipart/byteranges` body (RFC 9110 §14.6): this is what
/// SWUpdate's delta handler parses when fetching zchunk chunks.
#[tokio::test]
async fn multi_range_serves_multipart_byteranges() {
    let (app, _) = common::setup().await;
    let (sm, _) = fixture(&app).await;
    let uri = format!("/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin");

    // the overlapping last two coalesce into "wor"
    let resp = get_range(&app, &uri, "bytes=0-1, 6-7, 7-8").await;
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert!(resp.headers().get("content-range").is_none());
    let ctype = resp.headers()["content-type"].to_str().unwrap().to_owned();
    let boundary = ctype
        .strip_prefix("multipart/byteranges; boundary=")
        .expect("multipart content type")
        .to_owned();
    let declared: usize = resp.headers()["content-length"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(bytes.len(), declared);
    let expected = format!(
        "--{boundary}\r\nContent-Type: application/octet-stream\r\nContent-Range: bytes 0-1/11\r\n\r\nhe\
         \r\n--{boundary}\r\nContent-Type: application/octet-stream\r\nContent-Range: bytes 6-8/11\r\n\r\nwor\
         \r\n--{boundary}--\r\n"
    );
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), expected);
}

#[tokio::test]
async fn multi_range_that_coalesces_to_one_is_single_part() {
    let (app, _) = common::setup().await;
    let (sm, _) = fixture(&app).await;
    let uri = format!("/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin");

    let resp = get_range(&app, &uri, "bytes=0-4,5-10").await;
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get("content-range").unwrap(),
        "bytes 0-10/11"
    );
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/octet-stream"
    );
}

#[tokio::test]
async fn malformed_range_is_ignored() {
    let (app, _) = common::setup().await;
    let (sm, _) = fixture(&app).await;
    let uri = format!("/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw.bin");

    for range in ["bytes=5-2", "items=0-1"] {
        let resp = get_range(&app, &uri, range).await;
        assert_eq!(resp.status(), StatusCode::OK, "{range}");
        let bytes = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(&bytes[..], b"hello world");
    }
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

#[tokio::test]
async fn legacy_unsafe_filename_still_downloads() {
    use raptor::entity::artifact;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr};

    let (app, state) = common::setup().await;
    let (sm, _) = fixture(&app).await;
    // Stored before upload validation, which now rejects such names.
    artifact::Entity::update_many()
        .col_expr(artifact::Column::Filename, Expr::value("fw\u{1}\".bin"))
        .filter(artifact::Column::Filename.eq("fw.bin"))
        .exec(&state.db)
        .await
        .unwrap();
    let aid = artifact::Entity::find()
        .one(&state.db)
        .await
        .unwrap()
        .unwrap()
        .id;

    for uri in [
        format!("/DEFAULT/controller/v1/d1/softwaremodules/{sm}/artifacts/fw%01%22.bin"),
        format!("/rest/v1/softwaremodules/{sm}/artifacts/{aid}/download"),
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::get(&uri)
                    .header(header::AUTHORIZATION, common::mgmt_auth_header())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{uri}");
        assert_eq!(
            resp.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            "attachment; filename=\"fw__.bin\""
        );
    }
}
