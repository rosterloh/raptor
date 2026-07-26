mod common;

use axum::body::Body;
use axum::http::{Response, StatusCode};
use serde_json::json;
use tower::ServiceExt;

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> Response<Body> {
    app.clone()
        .oneshot(common::req(method, uri, body))
        .await
        .unwrap()
}

async fn json_of(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> serde_json::Value {
    common::body_json(call(app, method, uri, body).await).await
}

/// Creates `n` targets named dev-1..dev-n.
async fn make_targets(app: &axum::Router, cids: &[&str]) {
    let body: Vec<_> = cids.iter().map(|c| json!({"controllerId": c})).collect();
    let resp = call(app, "POST", "/rest/v1/targets", Some(json!(body))).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
}

async fn make_ds(app: &axum::Router, name: &str, version: &str) -> i64 {
    let body = json_of(
        app,
        "POST",
        "/rest/v1/distributionsets",
        Some(json!([{"name": name, "version": version, "type": "os"}])),
    )
    .await;
    body[0]["id"].as_i64().unwrap()
}

async fn make_target_tag(app: &axum::Router, name: &str) -> i64 {
    let resp = call(
        app,
        "POST",
        "/rest/v1/targettags",
        Some(json!([{"name": name}])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    common::body_json(resp).await[0]["id"].as_i64().unwrap()
}

async fn make_ds_tag(app: &axum::Router, name: &str) -> i64 {
    let resp = call(
        app,
        "POST",
        "/rest/v1/distributionsettags",
        Some(json!([{"name": name}])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    common::body_json(resp).await[0]["id"].as_i64().unwrap()
}

// --------------------------------------------------------------------------
// Target tag CRUD
// --------------------------------------------------------------------------

#[tokio::test]
async fn target_tag_crud() {
    let (app, _) = common::setup().await;

    let resp = call(
        &app,
        "POST",
        "/rest/v1/targettags",
        Some(json!([{"name": "beta", "description": "early access", "colour": "#00ff00"}])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = common::body_json(resp).await;
    let id = created[0]["id"].as_i64().unwrap();
    assert_eq!(created[0]["name"], "beta");
    assert_eq!(created[0]["description"], "early access");
    assert_eq!(created[0]["colour"], "#00ff00");
    assert!(created[0]["createdAt"].as_i64().unwrap() > 0);
    assert_eq!(
        created[0]["_links"]["assignedTargets"]["href"],
        format!("/rest/v1/targettags/{id}/assigned")
    );

    // duplicate name -> 409
    let resp = call(
        &app,
        "POST",
        "/rest/v1/targettags",
        Some(json!([{"name": "beta"}])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // get one
    let one = json_of(&app, "GET", &format!("/rest/v1/targettags/{id}"), None).await;
    assert_eq!(one["name"], "beta");

    // update name + colour
    let resp = call(
        &app,
        "PUT",
        &format!("/rest/v1/targettags/{id}"),
        Some(json!({"name": "canary", "colour": "#ff0000"})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let updated = common::body_json(resp).await;
    assert_eq!(updated["name"], "canary");
    assert_eq!(updated["colour"], "#ff0000");
    assert_eq!(updated["description"], "early access"); // untouched

    // rename onto an existing tag -> 409
    make_target_tag(&app, "stable").await;
    let resp = call(
        &app,
        "PUT",
        &format!("/rest/v1/targettags/{id}"),
        Some(json!({"name": "stable"})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // list: paged envelope, FIQL + sort
    let list = json_of(&app, "GET", "/rest/v1/targettags", None).await;
    assert_eq!(list["total"], 2);
    let list = json_of(&app, "GET", "/rest/v1/targettags?q=name==canary", None).await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["content"][0]["id"], id);
    let list = json_of(&app, "GET", "/rest/v1/targettags?sort=name:DESC", None).await;
    assert_eq!(list["content"][0]["name"], "stable");
    let list = json_of(&app, "GET", "/rest/v1/targettags?offset=1&limit=1", None).await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["size"], 1);

    // unknown field -> 400, unknown id -> 404
    let resp = call(&app, "GET", "/rest/v1/targettags?q=bogus==1", None).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = call(&app, "GET", "/rest/v1/targettags/9999", None).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // delete
    let resp = call(&app, "DELETE", &format!("/rest/v1/targettags/{id}"), None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = call(&app, "GET", &format!("/rest/v1/targettags/{id}"), None).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --------------------------------------------------------------------------
// Target tag assignment
// --------------------------------------------------------------------------

#[tokio::test]
async fn target_tag_assign_and_unassign() {
    let (app, _) = common::setup().await;
    make_targets(&app, &["dev-1", "dev-2", "dev-3"]).await;
    let tag = make_target_tag(&app, "beta").await;

    // single assign
    let resp = call(
        &app,
        "POST",
        &format!("/rest/v1/targettags/{tag}/assigned/dev-1"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["controllerId"], "dev-1");

    // re-assigning is a no-op, not a conflict
    let resp = call(
        &app,
        "POST",
        &format!("/rest/v1/targettags/{tag}/assigned/dev-1"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // bulk assign
    let resp = call(
        &app,
        "POST",
        &format!("/rest/v1/targettags/{tag}/assigned"),
        Some(json!(["dev-2", "dev-3"])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await.as_array().unwrap().len(), 2);

    let assigned = json_of(
        &app,
        "GET",
        &format!("/rest/v1/targettags/{tag}/assigned"),
        None,
    )
    .await;
    assert_eq!(assigned["total"], 3);

    // paging and FIQL narrow the assigned list
    let assigned = json_of(
        &app,
        "GET",
        &format!("/rest/v1/targettags/{tag}/assigned?q=controllerId==dev-2"),
        None,
    )
    .await;
    assert_eq!(assigned["total"], 1);
    assert_eq!(assigned["content"][0]["controllerId"], "dev-2");

    // single unassign
    let resp = call(
        &app,
        "DELETE",
        &format!("/rest/v1/targettags/{tag}/assigned/dev-1"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    // ...and again -> 404, the assignment is gone
    let resp = call(
        &app,
        "DELETE",
        &format!("/rest/v1/targettags/{tag}/assigned/dev-1"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // bulk unassign
    let resp = call(
        &app,
        "DELETE",
        &format!("/rest/v1/targettags/{tag}/assigned"),
        Some(json!(["dev-2", "dev-3"])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let assigned = json_of(
        &app,
        "GET",
        &format!("/rest/v1/targettags/{tag}/assigned"),
        None,
    )
    .await;
    assert_eq!(assigned["total"], 0);

    // unknown target / unknown tag
    let resp = call(
        &app,
        "POST",
        &format!("/rest/v1/targettags/{tag}/assigned/nope"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = call(
        &app,
        "POST",
        "/rest/v1/targettags/9999/assigned/dev-1",
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_a_target_tag_keeps_its_targets() {
    let (app, _) = common::setup().await;
    make_targets(&app, &["dev-1"]).await;
    let tag = make_target_tag(&app, "beta").await;
    call(
        &app,
        "POST",
        &format!("/rest/v1/targettags/{tag}/assigned/dev-1"),
        None,
    )
    .await;

    let resp = call(&app, "DELETE", &format!("/rest/v1/targettags/{tag}"), None).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // target survives, and its (now removed) assignment doesn't linger
    let resp = call(&app, "GET", "/rest/v1/targets/dev-1", None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let targets = json_of(&app, "GET", "/rest/v1/targets", None).await;
    assert_eq!(targets["total"], 1);

    // a fresh tag reusing the name starts empty
    let tag = make_target_tag(&app, "beta").await;
    let assigned = json_of(
        &app,
        "GET",
        &format!("/rest/v1/targettags/{tag}/assigned"),
        None,
    )
    .await;
    assert_eq!(assigned["total"], 0);
}

// --------------------------------------------------------------------------
// `tag==` FIQL on targets
// --------------------------------------------------------------------------

#[tokio::test]
async fn target_list_filters_by_tag() {
    let (app, _) = common::setup().await;
    make_targets(&app, &["dev-1", "dev-2", "dev-3"]).await;
    let beta = make_target_tag(&app, "beta").await;
    let stable = make_target_tag(&app, "stable").await;
    call(
        &app,
        "POST",
        &format!("/rest/v1/targettags/{beta}/assigned"),
        Some(json!(["dev-1", "dev-2"])),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/rest/v1/targettags/{stable}/assigned"),
        Some(json!(["dev-2"])),
    )
    .await;

    let cids = |v: &serde_json::Value| -> Vec<String> {
        let mut out: Vec<String> = v["content"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["controllerId"].as_str().unwrap().to_string())
            .collect();
        out.sort();
        out
    };

    let r = json_of(&app, "GET", "/rest/v1/targets?q=tag==beta", None).await;
    assert_eq!(r["total"], 2);
    assert_eq!(cids(&r), ["dev-1", "dev-2"]);

    // negation means "not tagged beta" — dev-2 carries another tag too and is
    // still excluded.
    let r = json_of(&app, "GET", "/rest/v1/targets?q=tag!=beta", None).await;
    assert_eq!(r["total"], 1);
    assert_eq!(cids(&r), ["dev-3"]);

    // wildcards, lists and combinations with plain columns
    let r = json_of(&app, "GET", "/rest/v1/targets?q=tag==bet*", None).await;
    assert_eq!(r["total"], 2);
    let r = json_of(&app, "GET", "/rest/v1/targets?q=tag=in=(beta,stable)", None).await;
    assert_eq!(r["total"], 2);
    let r = json_of(&app, "GET", "/rest/v1/targets?q=tag=out=(beta)", None).await;
    assert_eq!(cids(&r), ["dev-3"]);
    let r = json_of(
        &app,
        "GET",
        "/rest/v1/targets?q=tag==beta;controllerId==dev-1",
        None,
    )
    .await;
    assert_eq!(r["total"], 1);
    let r = json_of(
        &app,
        "GET",
        "/rest/v1/targets?q=tag==stable,controllerId==dev-3",
        None,
    )
    .await;
    assert_eq!(cids(&r), ["dev-2", "dev-3"]);

    // an unknown tag matches nothing; ordering operators are rejected
    let r = json_of(&app, "GET", "/rest/v1/targets?q=tag==nope", None).await;
    assert_eq!(r["total"], 0);
    let resp = call(&app, "GET", "/rest/v1/targets?q=tag=gt=beta", None).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // paging still counts only matching rows
    let r = json_of(&app, "GET", "/rest/v1/targets?q=tag==beta&limit=1", None).await;
    assert_eq!(r["total"], 2);
    assert_eq!(r["size"], 1);
}

#[tokio::test]
async fn target_filters_accept_tag_queries() {
    let (app, _) = common::setup().await;
    make_targets(&app, &["dev-1"]).await;
    let beta = make_target_tag(&app, "beta").await;
    call(
        &app,
        "POST",
        &format!("/rest/v1/targettags/{beta}/assigned/dev-1"),
        None,
    )
    .await;

    // A saved filter compiles the same field set as the target list.
    let resp = call(
        &app,
        "POST",
        "/rest/v1/targetfilters",
        Some(json!({"name": "betas", "query": "tag==beta"})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// --------------------------------------------------------------------------
// Distribution set tags
// --------------------------------------------------------------------------

#[tokio::test]
async fn ds_tag_crud_assign_and_filter() {
    let (app, _) = common::setup().await;
    let ds1 = make_ds(&app, "release", "1.0").await;
    let ds2 = make_ds(&app, "release", "2.0").await;
    make_ds(&app, "release", "3.0").await;

    let resp = call(
        &app,
        "POST",
        "/rest/v1/distributionsettags",
        Some(json!([{"name": "qa", "colour": "#0000ff"}])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let tag = common::body_json(resp).await[0]["id"].as_i64().unwrap();
    assert_eq!(
        json_of(
            &app,
            "GET",
            &format!("/rest/v1/distributionsettags/{tag}"),
            None
        )
        .await["colour"],
        "#0000ff"
    );

    // duplicate name -> 409
    let resp = call(
        &app,
        "POST",
        "/rest/v1/distributionsettags",
        Some(json!([{"name": "qa"}])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // update
    let resp = call(
        &app,
        "PUT",
        &format!("/rest/v1/distributionsettags/{tag}"),
        Some(json!({"description": "release candidates"})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        common::body_json(resp).await["description"],
        "release candidates"
    );

    // assign single + bulk
    let resp = call(
        &app,
        "POST",
        &format!("/rest/v1/distributionsettags/{tag}/assigned/{ds1}"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(common::body_json(resp).await["id"], ds1);
    let resp = call(
        &app,
        "POST",
        &format!("/rest/v1/distributionsettags/{tag}/assigned"),
        Some(json!([ds2])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let assigned = json_of(
        &app,
        "GET",
        &format!("/rest/v1/distributionsettags/{tag}/assigned"),
        None,
    )
    .await;
    assert_eq!(assigned["total"], 2);
    // full DS shape, modules included
    assert!(assigned["content"][0]["modules"].is_array());

    // the assigned list takes the usual sort/q parameters too
    let assigned = json_of(
        &app,
        "GET",
        &format!("/rest/v1/distributionsettags/{tag}/assigned?sort=version:DESC&q=version==2.0"),
        None,
    )
    .await;
    assert_eq!(assigned["total"], 1);
    assert_eq!(assigned["content"][0]["version"], "2.0");

    // FIQL on the DS list
    let r = json_of(&app, "GET", "/rest/v1/distributionsets?q=tag==qa", None).await;
    assert_eq!(r["total"], 2);
    let r = json_of(&app, "GET", "/rest/v1/distributionsets?q=tag!=qa", None).await;
    assert_eq!(r["total"], 1);
    assert_eq!(r["content"][0]["version"], "3.0");
    let r = json_of(
        &app,
        "GET",
        "/rest/v1/distributionsets?q=tag==qa;version==1.0",
        None,
    )
    .await;
    assert_eq!(r["total"], 1);

    // unassign
    let resp = call(
        &app,
        "DELETE",
        &format!("/rest/v1/distributionsettags/{tag}/assigned/{ds1}"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = call(
        &app,
        "DELETE",
        &format!("/rest/v1/distributionsettags/{tag}/assigned"),
        Some(json!([ds2])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let r = json_of(&app, "GET", "/rest/v1/distributionsets?q=tag==qa", None).await;
    assert_eq!(r["total"], 0);

    // unknown DS -> 404
    let resp = call(
        &app,
        "POST",
        &format!("/rest/v1/distributionsettags/{tag}/assigned/9999"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_a_ds_tag_keeps_its_distribution_sets() {
    let (app, _) = common::setup().await;
    let ds = make_ds(&app, "release", "1.0").await;
    let tag = make_ds_tag(&app, "qa").await;
    call(
        &app,
        "POST",
        &format!("/rest/v1/distributionsettags/{tag}/assigned/{ds}"),
        None,
    )
    .await;

    let resp = call(
        &app,
        "DELETE",
        &format!("/rest/v1/distributionsettags/{tag}"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = call(
        &app,
        "GET",
        &format!("/rest/v1/distributionsets/{ds}"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        json_of(&app, "GET", "/rest/v1/distributionsets", None).await["total"],
        1
    );
}

#[tokio::test]
async fn tag_endpoints_require_auth() {
    let (app, _) = common::setup().await;
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/rest/v1/targettags")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
