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
