//! Full update cycle driven by Collabora's `hawkbit` DDI client crate —
//! the drop-in compatibility proof. Runs a real server on a loopback port.
mod common;

use hawkbit::ddi::{Client, ClientAuthorization, Execution, Finished};
use serde_json::json;

/// Binds a real TCP listener first (so we know the port), then builds the app
/// with cfg.url pointing at that address, so `_links` in DDI responses are dialable.
async fn spawn_server() -> (String, reqwest::Client) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let (app, _state) = common::setup_with_url(&base).await;
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, reqwest::Client::new())
}

#[tokio::test]
async fn full_update_cycle_with_hawkbit_client() {
    let (base, http) = spawn_server().await;
    let auth = |b: reqwest::RequestBuilder| b.basic_auth("admin", Some(common::TEST_PASSWORD));

    // seed: module + artifact + ds + target
    let sm = auth(http.post(format!("{base}/rest/v1/softwaremodules")))
        .json(&json!([{"name": "fw", "version": "1.0", "type": "os"}]))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()[0]["id"]
        .as_i64()
        .unwrap();
    let part = reqwest::multipart::Part::bytes(b"raptor-e2e-payload".to_vec()).file_name("fw.bin");
    auth(http.post(format!("{base}/rest/v1/softwaremodules/{sm}/artifacts")))
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let ds = auth(http.post(format!("{base}/rest/v1/distributionsets")))
        .json(&json!([{"name": "stable", "version": "1.0", "type": "os", "modules": [{"id": sm}]}]))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()[0]["id"]
        .as_i64()
        .unwrap();
    auth(http.post(format!("{base}/rest/v1/targets")))
        .json(&json!([{"controllerId": "e2e-dev", "securityToken": "e2e-token"}]))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // device: no update pending yet
    let auth_method = ClientAuthorization::TargetToken("e2e-token".into());
    let client = Client::new(&base, "DEFAULT", "e2e-dev", auth_method, None, None, None).unwrap();
    let reply = client.poll().await.unwrap();
    assert!(reply.update().is_none());

    // operator assigns
    auth(http.post(format!("{base}/rest/v1/targets/e2e-dev/assignedDS")))
        .json(&json!({"id": ds, "type": "forced"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // device: sees update, downloads, verifies, reports success
    let reply = client.poll().await.unwrap();
    let update = reply.update().expect("deploymentBase link expected");
    let update = update.fetch().await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let artifacts = update.download(dir.path()).await.unwrap();
    assert!(!artifacts.is_empty());
    update
        .send_feedback(Execution::Closed, Finished::Success, vec!["installed"])
        .await
        .unwrap();

    // server: target in_sync, action finished
    let t = auth(http.get(format!("{base}/rest/v1/targets/e2e-dev")))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(t["updateStatus"], "in_sync");
    let actions = auth(http.get(format!("{base}/rest/v1/targets/e2e-dev/actions")))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(actions["content"][0]["status"], "finished");
}
