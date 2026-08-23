//! Target groups (#89): hawkBit's single-valued organisational placement for a
//! target, set through the normal create/update bodies and filterable with
//! `q=group==`.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn group_is_stored_on_create_and_echoed() {
    let (app, _) = common::setup().await;

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "dev-1", "group": "plant-a/line-3"}])),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = common::body_json(resp).await;
    assert_eq!(body[0]["group"], "plant-a/line-3");

    // and survives a read back
    let resp = app
        .clone()
        .oneshot(common::req("GET", "/rest/v1/targets/dev-1", None))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["group"], "plant-a/line-3");
}

#[tokio::test]
async fn a_target_without_a_group_omits_the_field_entirely() {
    // The field must not appear at all when unset — a client written against
    // the previous shape sees byte-identical JSON.
    let (app, _) = common::setup().await;

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "dev-1"}])),
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert!(
        body[0].get("group").is_none(),
        "unset group must be omitted, got {}",
        body[0]
    );

    let resp = app
        .clone()
        .oneshot(common::req("GET", "/rest/v1/targets/dev-1", None))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert!(body.get("group").is_none(), "got {body}");
}

#[tokio::test]
async fn put_moves_a_target_between_groups_and_leaves_it_alone_when_omitted() {
    let (app, _) = common::setup().await;

    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "dev-1", "group": "plant-a"}])),
        ))
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            "/rest/v1/targets/dev-1",
            Some(json!({"group": "plant-b/line-1"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["group"], "plant-b/line-1");

    // A PUT that doesn't mention the group must not clear it.
    let resp = app
        .clone()
        .oneshot(common::req(
            "PUT",
            "/rest/v1/targets/dev-1",
            Some(json!({"name": "renamed"})),
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["name"], "renamed");
    assert_eq!(body["group"], "plant-b/line-1");
}

#[tokio::test]
async fn fiql_filters_by_group_exactly_and_by_hierarchical_prefix() {
    let (app, _) = common::setup().await;

    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([
                {"controllerId": "a1", "group": "plant-a/line-1"},
                {"controllerId": "a2", "group": "plant-a/line-2"},
                {"controllerId": "b1", "group": "plant-b/line-1"},
                {"controllerId": "n1"}
            ])),
        ))
        .await
        .unwrap();

    let cids = |body: &serde_json::Value| {
        let mut v: Vec<String> = body["content"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["controllerId"].as_str().unwrap().to_string())
            .collect();
        v.sort();
        v
    };

    // exact match
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            "/rest/v1/targets?q=group==plant-a/line-1",
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(cids(&body), vec!["a1"]);

    // hierarchical prefix, via the shared `*` -> LIKE path
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            "/rest/v1/targets?q=group==plant-a/*",
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 2);
    assert_eq!(cids(&body), vec!["a1", "a2"]);

    // composes with the other target fields, which is the point of putting it
    // in the shared compiler rather than special-casing the list endpoint
    let resp = app
        .clone()
        .oneshot(common::req(
            "GET",
            "/rest/v1/targets?q=group==plant-a/*;updateStatus==unknown",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(common::body_json(resp).await["total"], 2);
}

#[tokio::test]
async fn saved_target_filters_accept_a_group_query() {
    // Saved filters and rollouts route through the same FIQL compiler as the
    // list, so `group==` has to mean the same thing in all three.
    let (app, _) = common::setup().await;

    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(json!([
                {"controllerId": "a1", "group": "plant-a/line-1"},
                {"controllerId": "b1", "group": "plant-b/line-1"}
            ])),
        ))
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targetfilters",
            Some(json!({"name": "plant-a", "query": "group==plant-a/*"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}
