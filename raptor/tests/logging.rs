//! What reaches the log at the default `info` level: management API writes
//! and domain events, but not reads or device polls, which would drown them.

mod common;

use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, SubscriberExt};

/// Records every INFO-or-above event as `field=value` pairs.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
    fn on_event(&self, e: &tracing::Event<'_>, _: Context<'_, S>) {
        if *e.metadata().level() > tracing::Level::INFO {
            return;
        }
        struct Fields(String);
        impl Visit for Fields {
            fn record_debug(&mut self, f: &Field, v: &dyn std::fmt::Debug) {
                self.0.push_str(&format!("{}={v:?} ", f.name()));
            }
        }
        let mut fields = Fields(String::new());
        e.record(&mut fields);
        self.0.lock().unwrap().push(fields.0);
    }
}

impl Capture {
    fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn mgmt_writes_are_logged_and_reads_are_not() {
    let (app, _st) = common::setup().await;
    let cap = Capture::default();
    let _guard = tracing::subscriber::set_default(tracing_subscriber::registry().with(cap.clone()));

    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets",
            Some(serde_json::json!([{"controllerId": "dev-1"}])),
        ))
        .await
        .unwrap();
    app.clone()
        .oneshot(common::req("GET", "/rest/v1/targets", None))
        .await
        .unwrap();

    let lines = cap.lines();
    assert!(
        lines.iter().any(|l| l.contains("method=POST")
            && l.contains("path=\"/rest/v1/targets\"")
            && l.contains("status=201")),
        "{lines:#?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("method=GET")),
        "reads must stay below info: {lines:#?}"
    );
}

#[tokio::test]
async fn device_polls_stay_below_info_but_registration_is_logged() {
    let st = common::setup_with_anonymous(true).await;
    let app = raptor::app::build_app(st);
    let cap = Capture::default();
    let _guard = tracing::subscriber::set_default(tracing_subscriber::registry().with(cap.clone()));

    for _ in 0..2 {
        app.clone()
            .oneshot(
                axum::http::Request::get("/DEFAULT/controller/v1/dev-9")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    let lines = cap.lines();
    let registered: Vec<_> = lines
        .iter()
        .filter(|l| l.contains("target registered") && l.contains("dev-9"))
        .collect();
    assert_eq!(registered.len(), 1, "{lines:#?}");
    assert!(
        !lines.iter().any(|l| l.contains("controller/v1")),
        "polls must stay below info: {lines:#?}"
    );
}
