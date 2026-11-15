#![allow(clippy::unwrap_used)] // helpers outside #[test] fns aren't covered by allow-unwrap-in-tests
mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

async fn probe_app(anonymous: bool) -> axum::Router {
    let (_, state) = common::setup().await;
    // common::setup() config has anonymous=true; rebuild cfg for the strict case
    let state = if anonymous {
        state
    } else {
        common::setup_with_anonymous(false).await
    };
    axum::Router::new()
        .route(
            "/{tenant}/controller/v1/{controllerId}/probe",
            axum::routing::get(|| async { "ok" }),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            raptor::auth::ddi::ddi_auth,
        ))
        .with_state(state)
}

fn get(uri: &str, auth: Option<&str>) -> Request<Body> {
    let mut b = Request::get(uri);
    if let Some(a) = auth {
        b = b.header(header::AUTHORIZATION, a);
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn anonymous_mode_allows_unauthenticated() {
    let app = probe_app(true).await;
    let resp = app
        .oneshot(get("/DEFAULT/controller/v1/dev-1/probe", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn strict_mode_rejects_unauthenticated() {
    let app = probe_app(false).await;
    let resp = app
        .oneshot(get("/DEFAULT/controller/v1/dev-1/probe", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn gateway_token_accepted_wrong_rejected() {
    let app = probe_app(false).await;
    // "gw-token" comes from common::test_config
    let resp = app
        .clone()
        .oneshot(get(
            "/DEFAULT/controller/v1/dev-1/probe",
            Some("GatewayToken gw-token"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .oneshot(get(
            "/DEFAULT/controller/v1/dev-1/probe",
            Some("GatewayToken nope"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn target_token_verified_against_db() {
    let state = common::setup_with_anonymous(false).await;
    let app = axum::Router::new()
        .route(
            "/{tenant}/controller/v1/{controllerId}/probe",
            axum::routing::get(|| async { "ok" }),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            raptor::auth::ddi::ddi_auth,
        ))
        .with_state(state.clone());
    // create target with known token via the mgmt layer's entity directly
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    raptor::entity::target::ActiveModel {
        controller_id: Set("dev-t".into()),
        name: Set("dev-t".into()),
        security_token: Set("s3cret".into()),
        update_status: Set("unknown".into()),
        created_at: Set(1),
        updated_at: Set(1),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .unwrap();

    assert_eq!(
        app.clone()
            .oneshot(get(
                "/DEFAULT/controller/v1/dev-t/probe",
                Some("TargetToken s3cret")
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(get(
                "/DEFAULT/controller/v1/dev-t/probe",
                Some("TargetToken wrong")
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.oneshot(get(
            "/DEFAULT/controller/v1/ghost/probe",
            Some("TargetToken s3cret")
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- [ddi.header_auth]: proxy-forwarded certificate identity ---

fn header_auth(proxies: &[&str], auto_register: bool) -> raptor::config::HeaderAuthConfig {
    raptor::config::HeaderAuthConfig {
        enabled: true,
        trusted_authorities: vec!["ca-a".into()],
        trusted_proxies: proxies.iter().map(|p| p.parse().unwrap()).collect(),
        auto_register,
        ..Default::default()
    }
}

/// A poll of `cid` carrying `headers`, arriving from socket peer `peer`.
fn poll(cid: &str, headers: &[(&str, &str)], peer: Option<&str>) -> Request<Body> {
    let mut b = Request::get(format!("/DEFAULT/controller/v1/{cid}"));
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let mut req = b.body(Body::empty()).unwrap();
    if let Some(p) = peer {
        req.extensions_mut()
            .insert(axum::extract::ConnectInfo::<std::net::SocketAddr>(
                p.parse().unwrap(),
            ));
    }
    req
}

async fn status(app: &axum::Router, req: Request<Body>) -> StatusCode {
    app.clone().oneshot(req).await.unwrap().status()
}

async fn target_exists(app: &axum::Router, cid: &str) -> bool {
    status(
        app,
        common::req("GET", &format!("/rest/v1/targets/{cid}"), None),
    )
    .await
        == StatusCode::OK
}

#[tokio::test]
async fn header_auth_registers_matching_controller() {
    let (app, _) = common::setup_with_header_auth(header_auth(&[], true)).await;
    let h = [("X-Controller-Id", "dev-10"), ("X-Authority", "CA-A")];
    assert_eq!(status(&app, poll("dev-10", &h, None)).await, StatusCode::OK);
    assert!(target_exists(&app, "dev-10").await);
}

#[tokio::test]
async fn header_auth_accepts_indexed_authority_headers() {
    let (app, _) = common::setup_with_header_auth(header_auth(&[], true)).await;
    let h = [("X-Controller-Id", "dev-1"), ("X-Authority-1", "ca-a")];
    assert_eq!(status(&app, poll("dev-1", &h, None)).await, StatusCode::OK);
}

#[tokio::test]
async fn header_auth_rejects_mismatch_and_untrusted_authority() {
    let (app, _) = common::setup_with_header_auth(header_auth(&[], true)).await;
    let wrong_id = [("X-Controller-Id", "dev-2"), ("X-Authority", "ca-a")];
    assert_eq!(
        status(&app, poll("dev-1", &wrong_id, None)).await,
        StatusCode::UNAUTHORIZED
    );
    let wrong_ca = [("X-Controller-Id", "dev-1"), ("X-Authority", "ca-evil")];
    assert_eq!(
        status(&app, poll("dev-1", &wrong_ca, None)).await,
        StatusCode::UNAUTHORIZED
    );
    assert!(!target_exists(&app, "dev-1").await);
}

#[tokio::test]
async fn header_auth_off_ignores_headers() {
    let (app, _) = common::setup_with_header_auth(Default::default()).await;
    let h = [("X-Controller-Id", "dev-1"), ("X-Authority", "ca-a")];
    assert_eq!(
        status(&app, poll("dev-1", &h, None)).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn header_auth_miss_falls_through_to_gateway_token() {
    let (app, _) = common::setup_with_header_auth(header_auth(&[], true)).await;
    let h = [
        ("X-Controller-Id", "gw-1"),
        ("X-Authority", "ca-a"),
        ("Authorization", "GatewayToken gw-token"),
    ];
    assert_eq!(status(&app, poll("dev-7", &h, None)).await, StatusCode::OK);
}

#[tokio::test]
async fn header_auth_honours_only_trusted_proxies() {
    let (app, _) = common::setup_with_header_auth(header_auth(&["10.0.0.5"], true)).await;
    let h = [("X-Controller-Id", "dev-1"), ("X-Authority", "ca-a")];
    assert_eq!(
        status(&app, poll("dev-1", &h, Some("10.0.0.9:4000"))).await,
        StatusCode::UNAUTHORIZED,
        "direct client forging headers"
    );
    assert_eq!(
        status(&app, poll("dev-1", &h, None)).await,
        StatusCode::UNAUTHORIZED,
        "unknown peer fails closed"
    );
    assert_eq!(
        status(&app, poll("dev-1", &h, Some("10.0.0.5:4000"))).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn header_auth_without_auto_register_needs_existing_target() {
    let (app, _) = common::setup_with_header_auth(header_auth(&[], false)).await;
    let h = [("X-Controller-Id", "dev-11"), ("X-Authority", "ca-a")];
    assert_eq!(
        status(&app, poll("dev-11", &h, None)).await,
        StatusCode::UNAUTHORIZED
    );
    assert!(!target_exists(&app, "dev-11").await);
    // A pre-created target authenticates as before.
    status(
        &app,
        common::req(
            "POST",
            "/rest/v1/targets",
            Some(serde_json::json!([{"controllerId": "dev-11"}])),
        ),
    )
    .await;
    assert_eq!(status(&app, poll("dev-11", &h, None)).await, StatusCode::OK);
}
