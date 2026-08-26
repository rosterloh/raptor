//! Polling time overrides (#91): `[ddi] polling_interval` extended into
//! hawkBit's `pollingTime` grammar — a default interval plus ordered
//! `<RSQL> -> <interval>` override rules, each accepting an optional `~NN%`
//! jitter. See docs/src/reference/configuration.md for the grammar and
//! raptor/src/config.rs for the parser.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

fn ddi_get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap() // common config: anonymous=true
}

/// Builds an app whose `[ddi] polling_interval` is the given raw string,
/// otherwise identical to `common::test_config`.
async fn app_with_polling_interval(
    polling_interval: &str,
) -> (axum::Router, raptor::state::AppState) {
    let (_, state) = common::setup().await;
    let mut cfg = state.cfg.clone();
    cfg.ddi.polling_interval = polling_interval.to_string();
    let state = raptor::state::AppState::new(state.db.clone(), cfg, state.store.clone());
    (raptor::app::build_app(state.clone()), state)
}

async fn create_target(app: &axum::Router, cid: &str, group: Option<&str>) {
    let mut body = json!({"controllerId": cid});
    if let Some(g) = group {
        body["group"] = json!(g);
    }
    let resp = app
        .clone()
        .oneshot(common::req("POST", "/rest/v1/targets", Some(json!([body]))))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn override_rule_matches_by_group_others_fall_through_to_default() {
    let (app, _) = app_with_polling_interval("00:05:00, group==eu -> 00:00:30").await;
    create_target(&app, "eu-dev", Some("eu")).await;
    create_target(&app, "us-dev", Some("us")).await;

    let eu = common::body_json(
        app.clone()
            .oneshot(ddi_get("/DEFAULT/controller/v1/eu-dev"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(eu["config"]["polling"]["sleep"], "00:00:30");

    let us = common::body_json(
        app.clone()
            .oneshot(ddi_get("/DEFAULT/controller/v1/us-dev"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(us["config"]["polling"]["sleep"], "00:05:00");
}

#[tokio::test]
async fn rules_evaluate_in_order_first_match_wins() {
    // dev-1 matches both rules; the first-listed one must win even though
    // both are individually valid matches.
    let (app, _) = app_with_polling_interval(
        "00:05:00, group==eu -> 00:01:00, controllerId=='dev-1' -> 00:02:00",
    )
    .await;
    create_target(&app, "dev-1", Some("eu")).await;

    let body = common::body_json(
        app.clone()
            .oneshot(ddi_get("/DEFAULT/controller/v1/dev-1"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["config"]["polling"]["sleep"], "00:01:00");
}

#[tokio::test]
async fn no_rule_matches_falls_through_to_default() {
    let (app, _) = app_with_polling_interval("00:07:00, group==eu -> 00:00:30").await;
    create_target(&app, "dev-1", None).await;

    let body = common::body_json(
        app.clone()
            .oneshot(ddi_get("/DEFAULT/controller/v1/dev-1"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["config"]["polling"]["sleep"], "00:07:00");
}

#[tokio::test]
async fn jitter_stays_within_configured_bounds_and_actually_varies() {
    let (app, _) = app_with_polling_interval("00:10:00~50%").await;
    create_target(&app, "dev-1", None).await;

    let mut seen = std::collections::HashSet::new();
    for _ in 0..30 {
        let body = common::body_json(
            app.clone()
                .oneshot(ddi_get("/DEFAULT/controller/v1/dev-1"))
                .await
                .unwrap(),
        )
        .await;
        let sleep = body["config"]["polling"]["sleep"]
            .as_str()
            .unwrap()
            .to_string();
        let parts: Vec<u64> = sleep.split(':').map(|p| p.parse().unwrap()).collect();
        let secs = parts[0] * 3600 + parts[1] * 60 + parts[2];
        assert!(
            (300..=900).contains(&secs),
            "{sleep} outside ±50% of 00:10:00"
        );
        seen.insert(sleep);
    }
    assert!(seen.len() > 1, "jitter never varied across 30 polls");
}

/// hawkBit's own PR examples write RSQL with spaces around the operator
/// (`group == 'eu'`); raptor's FIQL dialect doesn't tolerate that (`group==eu`
/// is required — see `Config::polling_schedule`'s doc comment). A rule
/// written that way fails to compile at poll time, is logged, and skipped —
/// not a 500 — falling through to whatever rule (or the default) comes next.
/// `main.rs`'s startup validation is what's meant to catch this before it
/// ever reaches a device.
#[tokio::test]
async fn hawkbit_style_whitespace_in_rsql_is_not_supported_and_falls_through_safely() {
    let (app, _) = app_with_polling_interval("00:05:00, group == 'eu' -> 00:00:30").await;
    create_target(&app, "eu-dev", Some("eu")).await;

    let body = common::body_json(
        app.clone()
            .oneshot(ddi_get("/DEFAULT/controller/v1/eu-dev"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["config"]["polling"]["sleep"], "00:05:00");
}

/// A malformed `polling_interval` must not 500 every device's poll — startup
/// validation (main.rs) is meant to catch this before the server ever
/// accepts traffic, but the handler itself falls back to the pre-override
/// default (5 minutes, no rules) rather than propagating the parse error.
#[tokio::test]
async fn malformed_config_falls_back_instead_of_failing_the_poll() {
    let (app, _) = app_with_polling_interval("not a valid pollingTime").await;

    let resp = app
        .clone()
        .oneshot(ddi_get("/DEFAULT/controller/v1/dev-1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = common::body_json(resp).await;
    assert_eq!(body["config"]["polling"]["sleep"], "00:05:00");
}
