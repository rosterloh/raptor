#![allow(clippy::unwrap_used)]
mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{TEST_PASSWORD, body_json, insert_user, req, req_as, setup};
use serde_json::json;
use tower::ServiceExt;

async fn send(app: &axum::Router, r: Request<Body>) -> axum::http::Response<Body> {
    app.clone().oneshot(r).await.unwrap()
}

/// Logs in through `POST /rest/v1/login` and returns the session cookie.
async fn login(app: &axum::Router, user: &str, pass: &str) -> Option<String> {
    let r = Request::post("/rest/v1/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": user, "password": pass}).to_string(),
        ))
        .unwrap();
    let resp = send(app, r).await;
    let c = resp.headers().get(header::SET_COOKIE)?.to_str().ok()?;
    Some(c.split(';').next()?.to_string())
}

fn with_cookie(cookie: &str, method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap()
}

#[tokio::test]
async fn crud_and_error_paths() {
    let (app, st) = setup().await;
    let resp = send(
        &app,
        req(
            "POST",
            "/rest/v1/users",
            Some(json!({"username": "alice", "password": "pw", "role": "read_only"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let alice = body_json(resp).await;
    assert_eq!(alice["role"], "read_only");
    assert_eq!(alice["source"], "local");
    assert!(alice.get("password").is_none() && alice.get("passwordHash").is_none());
    let id = alice["id"].as_i64().unwrap();

    for (body, want) in [
        (
            json!({"username": "alice", "password": "pw", "role": "admin"}),
            StatusCode::CONFLICT,
        ),
        (
            json!({"username": "admin", "password": "pw", "role": "admin"}),
            StatusCode::CONFLICT,
        ),
        (
            json!({"username": "bob", "password": "pw", "role": "operator"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"username": "bob", "password": "", "role": "admin"}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let resp = send(&app, req("POST", "/rest/v1/users", Some(body.clone()))).await;
        assert_eq!(resp.status(), want, "{body}");
    }

    let oidc = insert_user(&st, "carol", "admin", "oidc").await;
    let list = body_json(send(&app, req("GET", "/rest/v1/users?q=source==oidc", None)).await).await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["content"][0]["username"], "carol");

    let resp = send(
        &app,
        req(
            "PUT",
            &format!("/rest/v1/users/{oidc}"),
            Some(json!({"password": "x"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = send(
        &app,
        req(
            "PUT",
            &format!("/rest/v1/users/{id}"),
            Some(json!({"role": "admin"})),
        ),
    )
    .await;
    assert_eq!(body_json(resp).await["role"], "admin");

    let resp = send(&app, req("DELETE", &format!("/rest/v1/users/{id}"), None)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = send(&app, req("GET", &format!("/rest/v1/users/{id}"), None)).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// One API-created user (real argon2 cost) logs in by Basic and by session.
#[tokio::test]
async fn local_user_logs_in_with_basic_and_session() {
    let (app, _st) = setup().await;
    let resp = send(
        &app,
        req(
            "POST",
            "/rest/v1/users",
            Some(json!({"username": "dave", "password": "s3cret", "role": "admin"})),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let basic = Request::get("/rest/v1/targets")
        .header(
            header::AUTHORIZATION,
            format!("Basic {}", base64_enc("dave:s3cret")),
        )
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, basic).await.status(), StatusCode::OK);

    assert!(login(&app, "dave", "wrong").await.is_none());
    let cookie = login(&app, "dave", "s3cret").await.unwrap();
    let info = body_json(send(&app, with_cookie(&cookie, "GET", "/rest/v1/userinfo")).await).await;
    assert_eq!(info["username"], "dave");
    assert_eq!(info["role"], "admin");
}

fn base64_enc(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(s)
}

#[tokio::test]
async fn config_admin_unchanged_and_reported_as_admin() {
    let (app, _st) = setup().await;
    let info = body_json(send(&app, req("GET", "/rest/v1/userinfo", None)).await).await;
    assert_eq!(info["tenant"], "DEFAULT");
    assert_eq!(info["username"], "admin");
    assert_eq!(info["role"], "admin");
    let perms = info["permissions"].as_array().unwrap();
    assert!(perms.iter().any(|p| p == "CREATE_TARGET"));
    assert!(perms.iter().any(|p| p == "READ_TARGET_SECURITY_TOKEN"));
}

#[tokio::test]
async fn read_only_reads_but_cannot_write() {
    let (app, st) = setup().await;
    send(
        &app,
        req(
            "POST",
            "/rest/v1/targets",
            Some(json!([{"controllerId": "dev-1"}])),
        ),
    )
    .await;
    insert_user(&st, "viewer", "read_only", "local").await;

    let resp = send(&app, req_as("viewer", "GET", "/rest/v1/targets", None)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp).await;
    assert_eq!(list["total"], 1);
    assert!(list["content"][0].get("securityToken").is_none());
    let one = body_json(
        send(
            &app,
            req_as("viewer", "GET", "/rest/v1/targets/dev-1", None),
        )
        .await,
    )
    .await;
    assert!(one.get("securityToken").is_none());
    let admin_view = body_json(send(&app, req("GET", "/rest/v1/targets/dev-1", None)).await).await;
    assert!(admin_view["securityToken"].is_string());

    let resp = send(
        &app,
        req_as(
            "viewer",
            "POST",
            "/rest/v1/targets/dev-1/assignedDS",
            Some(json!([{"id": 1}])),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = body_json(resp).await;
    assert_eq!(
        body["errorCode"],
        "hawkbit.server.error.insufficientPermission"
    );
    assert_eq!(
        body["exceptionClass"],
        "org.eclipse.hawkbit.repository.exception.InsufficientPermissionException"
    );

    let resp = send(&app, req_as("viewer", "GET", "/rest/v1/users", None)).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let info =
        body_json(send(&app, req_as("viewer", "GET", "/rest/v1/userinfo", None)).await).await;
    assert_eq!(info["role"], "read_only");
    let perms = info["permissions"].as_array().unwrap();
    assert!(perms.iter().any(|p| p == "READ_TARGET"));
    assert!(!perms.iter().any(|p| p == "CREATE_TARGET"));
    assert!(!perms.iter().any(|p| p == "READ_TARGET_SECURITY_TOKEN"));

    // Logout stays open to a viewer.
    let cookie = login(&app, "viewer", TEST_PASSWORD).await.unwrap();
    let resp = send(&app, with_cookie(&cookie, "POST", "/rest/v1/logout")).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

/// Every Management API path in the source, tried with every write method as a
/// viewer: the gate must answer before any handler, so a new write route is
/// covered without anyone remembering to add it here.
#[tokio::test]
async fn read_only_is_refused_every_write_route() {
    let (app, st) = setup().await;
    insert_user(&st, "viewer", "read_only", "local").await;
    let cookie = login(&app, "viewer", TEST_PASSWORD).await.unwrap();

    let mut paths = std::collections::BTreeSet::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api/mgmt");
    let mut dirs = vec![root];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
                continue;
            }
            let src = std::fs::read_to_string(&p).unwrap();
            for chunk in src.split(".route(").skip(1) {
                let lit = chunk
                    .trim_start()
                    .strip_prefix('"')
                    .and_then(|c| c.split('"').next());
                if let Some(path) = lit.filter(|l| l.starts_with("/rest/v1/")) {
                    paths.insert(path.to_string());
                }
            }
        }
    }
    paths.remove("/rest/v1/login");
    paths.remove("/rest/v1/logout");
    assert!(paths.len() > 30, "route scan found only {paths:?}");

    let mut refused = 0;
    for path in &paths {
        // Fill `{param}` segments with a placeholder.
        let uri: String = path
            .split('/')
            .map(|s| if s.starts_with('{') { "1" } else { s })
            .collect::<Vec<_>>()
            .join("/");
        for m in ["POST", "PUT", "PATCH", "DELETE"] {
            let resp = send(&app, with_cookie(&cookie, m, &uri)).await;
            match resp.status() {
                StatusCode::FORBIDDEN => refused += 1,
                // Method not routed: no handler could run.
                StatusCode::METHOD_NOT_ALLOWED => {}
                s => panic!("{m} {uri} as read_only gave {s}"),
            }
        }
    }
    assert!(refused > 30, "only {refused} writes were refused");
}

#[tokio::test]
async fn disable_delete_and_role_change_end_sessions() {
    let (app, st) = setup().await;
    for (name, change) in [
        ("u1", Some(json!({"enabled": false}))),
        ("u2", Some(json!({"role": "read_only"}))),
        ("u3", None),
    ] {
        let id = insert_user(&st, name, "admin", "local").await;
        let cookie = login(&app, name, TEST_PASSWORD).await.unwrap();
        let ok = send(&app, with_cookie(&cookie, "GET", "/rest/v1/session")).await;
        assert_eq!(ok.status(), StatusCode::NO_CONTENT);
        let uri = format!("/rest/v1/users/{id}");
        let resp = match change {
            Some(b) => send(&app, req("PUT", &uri, Some(b))).await,
            None => send(&app, req("DELETE", &uri, None)).await,
        };
        assert_eq!(resp.status(), StatusCode::OK, "{name}");
        let gone = send(&app, with_cookie(&cookie, "GET", "/rest/v1/session")).await;
        assert_eq!(gone.status(), StatusCode::UNAUTHORIZED, "{name}");
    }
    // A disabled user cannot log in again either.
    assert!(login(&app, "u1", TEST_PASSWORD).await.is_none());
}

#[tokio::test]
async fn admin_cannot_lock_themselves_out() {
    let (app, st) = setup().await;
    let id = insert_user(&st, "alice", "admin", "local").await;
    let uri = format!("/rest/v1/users/{id}");
    for (m, body) in [
        ("PUT", Some(json!({"role": "read_only"}))),
        ("PUT", Some(json!({"enabled": false}))),
        ("DELETE", None),
    ] {
        let resp = send(&app, req_as("alice", m, &uri, body)).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{m}");
    }
    // Changing her own password is fine.
    let resp = send(
        &app,
        req_as("alice", "PUT", &uri, Some(json!({"password": "new"}))),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn oidc_user_has_no_password_login() {
    let (app, st) = setup().await;
    insert_user(&st, "carol", "admin", "oidc").await;
    let resp = send(&app, req_as("carol", "GET", "/rest/v1/targets", None)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(login(&app, "carol", TEST_PASSWORD).await.is_none());
}
