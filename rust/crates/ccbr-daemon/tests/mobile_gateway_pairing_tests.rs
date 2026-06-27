use ccbr_daemon::mobile_gateway::pairing::MobileGatewayPairingStore;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn pairing_store_writes_python_named_state_files_and_claims_device() {
    let tmp = TempDir::new().unwrap();
    let store = MobileGatewayPairingStore::new(tmp.path());

    let gateway = store
        .write_gateway_state("proj-1", "http://127.0.0.1:8787", "lan", ["view"])
        .unwrap();
    assert_eq!(gateway["schema_version"], 1);
    assert!(tmp.path().join("gateway.json").exists());

    let pairing = store
        .create_pairing_payload(
            "proj-1",
            "http://127.0.0.1:8787",
            Some("lan"),
            ["view"],
            Some(600),
        )
        .unwrap();
    assert_eq!(pairing["schema_version"], 1);
    assert_eq!(pairing["project_id"], "proj-1");
    assert_eq!(
        pairing["claim_endpoint"],
        "http://127.0.0.1:8787/v1/pairing/claim"
    );
    assert!(pairing["pairing_code"].as_str().unwrap().len() >= 18);
    assert!(tmp.path().join("pairing-tokens.jsonl").exists());

    let claimed = store
        .claim_pairing(
            pairing["pairing_code"].as_str().unwrap(),
            "Phone",
            Some("dev-1!"),
        )
        .unwrap();
    assert_eq!(claimed["status"], "ok");
    assert_eq!(claimed["device"]["device_id"], "dev-1");
    assert_eq!(claimed["device"]["revoked"], false);
    assert!(claimed["device_token"].as_str().unwrap().len() >= 32);

    let devices = store.list_devices();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["name"], "Phone");
    assert!(tmp.path().join("devices.json").exists());
    assert!(tmp.path().join("audit.jsonl").exists());
}

#[test]
fn pairing_store_rejects_duplicate_claim_and_revoke_matches_python_shape() {
    let tmp = TempDir::new().unwrap();
    let store = MobileGatewayPairingStore::new(tmp.path());
    let pairing = store
        .create_pairing_payload("proj-1", "http://host", Some("relay"), ["view"], Some(600))
        .unwrap();
    let code = pairing["pairing_code"].as_str().unwrap();
    store.claim_pairing(code, "Phone", Some("dev_1")).unwrap();

    let err = store
        .claim_pairing(code, "Phone", Some("dev_2"))
        .unwrap_err();
    assert_eq!(err.status_code, 409);
    assert_eq!(err.reason, "already_claimed");

    let revoked = store.revoke_device_locally("dev_1", None).unwrap();
    assert_eq!(revoked["schema_version"], 1);
    assert_eq!(revoked["status"], "revoked");
    assert_eq!(revoked["device"]["revoked"], true);
    assert_eq!(revoked["revoked_terminal_count"], 0);

    let err = store.revoke_device_locally("missing", None).unwrap_err();
    assert_eq!(err.status_code, 404);
    assert_eq!(err.reason, "not_found");
}

#[test]
fn pairing_store_authenticates_python_style_device_tokens() {
    let tmp = TempDir::new().unwrap();
    let store = MobileGatewayPairingStore::new(tmp.path());
    let pairing = store
        .create_pairing_payload("proj-1", "http://host", Some("lan"), ["view"], Some(600))
        .unwrap();
    let claimed = store
        .claim_pairing(pairing["pairing_code"].as_str().unwrap(), "Phone", None)
        .unwrap();
    let token = claimed["device_token"].as_str().unwrap();

    let auth = store.authenticate_device(token, ["view"]).unwrap();
    assert_eq!(auth.device_id(), claimed["device"]["device_id"]);
    assert!(auth.scopes().contains("view"));
    assert_eq!(auth.public_payload()["last_seen_at"].is_string(), true);

    let err = store
        .authenticate_device("bad-token", ["view"])
        .unwrap_err();
    assert_eq!(err.status_code, 401);
    assert_eq!(err.reason, "invalid_token");
}

#[test]
fn pairing_store_reads_existing_python_devices_file() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("devices.json"),
        r#"{
  "schema_version": 1,
  "devices": [
    {
      "schema_version": 1,
      "device_id": "dev_py",
      "name": "Python phone",
      "project_id": "proj-py",
      "pairing_id": "pair_py",
      "token_hash": "sha256:x",
      "scopes": ["view"],
      "route_provider": "lan",
      "gateway_url": "http://host",
      "created_at": "2026-06-27T00:00:00Z",
      "last_seen_at": null,
      "revoked_at": null
    }
  ]
}
"#,
    )
    .unwrap();

    let store = MobileGatewayPairingStore::new(tmp.path());
    let devices: Vec<Value> = store.list_devices();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["device_id"], "dev_py");
    assert_eq!(devices[0]["revoked"], false);
}

#[test]
fn pairing_store_terminal_tokens_track_sequences_disconnect_and_resume() {
    let tmp = TempDir::new().unwrap();
    let store = MobileGatewayPairingStore::new(tmp.path());
    let handle = store
        .create_terminal_handle(
            "proj-1",
            "dev-1",
            7,
            serde_json::json!({"agent": "codex", "pane_id": "%1"}),
            serde_json::json!({"cols": 80, "rows": 24}),
            Some(600),
        )
        .unwrap();
    assert_eq!(handle["schema_version"], 1);
    assert_eq!(handle["target_epoch"], 7);
    assert_eq!(handle["target_summary"]["agent"], "codex");
    assert!(tmp.path().join("terminal-tokens.jsonl").exists());

    let terminal_id = handle["terminal_id"].as_str().unwrap();
    let token = handle["terminal_token"].as_str().unwrap();
    let auth = store
        .authenticate_terminal_token(terminal_id, token, None)
        .unwrap();
    assert_eq!(auth["last_input_seq"], 0);
    assert_eq!(auth["last_output_seq"], 0);

    let input = store
        .record_terminal_input_sequence(terminal_id, token, 1)
        .unwrap();
    assert_eq!(input["last_input_seq"], 1);
    let replay = store
        .record_terminal_input_sequence(terminal_id, token, 1)
        .unwrap_err();
    assert_eq!(replay.status_code, 409);
    assert_eq!(replay.reason, "replayed_sequence");

    let output = store
        .record_terminal_output_sequence(terminal_id, token, 3)
        .unwrap();
    assert_eq!(output["last_output_seq"], 3);
    let disconnected = store
        .mark_terminal_disconnected(terminal_id, token, None)
        .unwrap();
    assert!(disconnected["disconnected_at"].is_string());

    let missing_resume = store
        .authenticate_terminal_token(terminal_id, token, None)
        .unwrap_err();
    assert_eq!(missing_resume.status_code, 409);
    assert_eq!(missing_resume.reason, "missing_resume_cursor");
    let stale_resume = store
        .authenticate_terminal_token(terminal_id, token, Some(2))
        .unwrap_err();
    assert_eq!(stale_resume.status_code, 409);
    assert_eq!(stale_resume.reason, "stale_resume_cursor");
    let resumed = store
        .authenticate_terminal_token(terminal_id, token, Some(3))
        .unwrap();
    assert!(resumed["disconnected_at"].is_null());
    assert_eq!(resumed["last_resume_cursor"], 3);
}

#[test]
fn pairing_store_terminal_close_and_device_revoke_block_auth() {
    let tmp = TempDir::new().unwrap();
    let store = MobileGatewayPairingStore::new(tmp.path());
    let pairing = store
        .create_pairing_payload("proj-1", "http://host", Some("lan"), ["view"], Some(600))
        .unwrap();
    store
        .claim_pairing(
            pairing["pairing_code"].as_str().unwrap(),
            "Phone",
            Some("dev-1"),
        )
        .unwrap();
    let handle = store
        .create_terminal_handle(
            "proj-1",
            "dev-1",
            7,
            serde_json::json!({"agent": "codex"}),
            serde_json::json!({}),
            Some(600),
        )
        .unwrap();
    let terminal_id = handle["terminal_id"].as_str().unwrap();
    let token = handle["terminal_token"].as_str().unwrap();

    let closed = store
        .close_terminal_handle(terminal_id, token, Some("client_closed"))
        .unwrap();
    assert!(closed["closed_at"].is_string());
    let closed_err = store
        .authenticate_terminal_token(terminal_id, token, None)
        .unwrap_err();
    assert_eq!(closed_err.status_code, 410);
    assert_eq!(closed_err.reason, "closed");

    let handle2 = store
        .create_terminal_handle(
            "proj-1",
            "dev-1",
            7,
            serde_json::json!({"agent": "codex"}),
            serde_json::json!({}),
            Some(600),
        )
        .unwrap();
    store.revoke_device_locally("dev-1", None).unwrap();
    let device_revoked = store
        .authenticate_terminal_token(
            handle2["terminal_id"].as_str().unwrap(),
            handle2["terminal_token"].as_str().unwrap(),
            None,
        )
        .unwrap_err();
    assert_eq!(device_revoked.status_code, 401);
    assert_eq!(device_revoked.reason, "device_revoked");
}
