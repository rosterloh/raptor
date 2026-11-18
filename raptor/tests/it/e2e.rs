//! Full update cycle driven by Collabora's `hawkbit` DDI client crate —
//! the drop-in compatibility proof. Runs a real server on a loopback port.
use crate::common;

use hawkbit::ddi::{Client, ClientAuthorization, Execution, Finished};
use serde_json::json;

#[tokio::test]
async fn full_update_cycle_with_hawkbit_client() {
    let (base, http, ds) = common::serve_seeded("e2e-dev", "e2e-token").await;
    let auth = |b: reqwest::RequestBuilder| b.basic_auth("admin", Some(common::TEST_PASSWORD));

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
