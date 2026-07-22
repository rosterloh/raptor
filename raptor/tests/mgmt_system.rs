mod common;

use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn tenant_configs_read() {
    let (app, _) = common::setup().await;

    let all = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/system/configs", None))
            .await
            .unwrap(),
    )
    .await;
    // pollingTime is the config-file default; every value is global.
    assert_eq!(all["pollingTime"]["value"], "00:05:00");
    assert_eq!(all["pollingTime"]["global"], true);
    // the test config sets a gateway token
    assert_eq!(all["authentication.gatewaytoken.enabled"]["value"], true);

    // single key
    let one = common::body_json(
        app.clone()
            .oneshot(common::req(
                "GET",
                "/rest/v1/system/configs/user.confirmation.flow.enabled",
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(one["value"], false);

    // unknown key -> 404
    let resp = app
        .oneshot(common::req(
            "GET",
            "/rest/v1/system/configs/does.not.exist",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tenant_config_writes_forbidden() {
    let (app, _) = common::setup().await;
    for method in ["PUT", "DELETE"] {
        let body = (method == "PUT").then(|| json!({"value": "00:01:00"}));
        let resp = app
            .clone()
            .oneshot(common::req(
                method,
                "/rest/v1/system/configs/pollingTime",
                body,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{method}");
    }
}

#[tokio::test]
async fn system_statistics_counts() {
    let (app, _) = common::setup().await;

    // seed a module, a complete DS, and two targets; assign one.
    let sm = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/softwaremodules",
                Some(json!([{"name": "rootfs", "version": "1.0", "type": "os"}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    let ds = common::body_json(
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/distributionsets",
                Some(json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}])),
            ))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_i64()
        .unwrap();
    for cid in ["dev-0", "dev-1"] {
        app.clone()
            .oneshot(common::req(
                "POST",
                "/rest/v1/targets",
                Some(json!([{"controllerId": cid}])),
            ))
            .await
            .unwrap();
    }
    app.clone()
        .oneshot(common::req(
            "POST",
            "/rest/v1/targets/dev-0/assignedDS",
            Some(json!({"id": ds, "type": "forced"})),
        ))
        .await
        .unwrap();

    let s = common::body_json(
        app.clone()
            .oneshot(common::req("GET", "/rest/v1/system/statistics", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(s["totalTargets"], 2);
    assert_eq!(s["totalDistributionSets"], 1);
    assert_eq!(s["totalSoftwareModules"], 1);
    assert_eq!(s["totalActions"], 1);
    assert_eq!(s["activeActions"], 1);
    // one target pending (assigned), one still unknown (never polled)
    assert_eq!(s["targetsByStatus"]["pending"], 1);
    assert_eq!(s["targetsByStatus"]["unknown"], 1);
}
