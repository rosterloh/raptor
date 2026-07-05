mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
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
