use ccbr_daemon::mobile_gateway::service::{
    MobileGatewayProject, MobileGatewayProjectClient, MobileGatewayProjectRegistry,
    MobileGatewayService,
};
use serde_json::{json, Value};
use tempfile::TempDir;

#[derive(Clone)]
struct FakeClient {
    ping: Result<Value, String>,
    view: Result<Value, String>,
}

impl MobileGatewayProjectClient for FakeClient {
    fn ping(&self) -> Result<Value, String> {
        self.ping.clone()
    }

    fn project_view(&self) -> Result<Value, String> {
        self.view.clone()
    }
}

fn ok_client() -> FakeClient {
    FakeClient {
        ping: Ok(json!({
            "project_id": "proj-1",
            "mount_state": "mounted",
            "health": "ok",
            "namespace_epoch": 7,
            "namespace_ui_attachable": true,
        })),
        view: Ok(json!({
            "view": {
                "project": {"id": "proj-1"},
                "namespace": {
                    "epoch": 7,
                    "socket_path": "/run/private.sock",
                    "session_name": "private-session",
                    "visible": true
                }
            },
            "cache": {"ttl_ms": 1000}
        })),
    }
}

#[test]
fn mobile_gateway_health_matches_python_ok_and_degraded_shapes() {
    let tmp = TempDir::new().unwrap();
    let service =
        MobileGatewayService::current_project("proj-1", tmp.path(), ok_client(), Some(tmp.path()))
            .unwrap();
    let health = service.health_payload();
    assert_eq!(health["schema_version"], 1);
    assert_eq!(health["status"], "ok");
    assert_eq!(health["project_id"], "proj-1");
    assert_eq!(health["ccbd"]["reachable"], true);
    assert_eq!(health["ccbd"]["namespace_epoch"], 7);
    assert!(health["capabilities"]
        .as_array()
        .unwrap()
        .contains(&json!("pairing")));

    let bad = FakeClient {
        ping: Err("socket down".to_string()),
        view: Ok(json!({})),
    };
    let degraded = MobileGatewayService::current_project("proj-1", tmp.path(), bad, None)
        .unwrap()
        .health_payload();
    assert_eq!(degraded["status"], "degraded");
    assert_eq!(degraded["ccbd"]["reachable"], false);
    assert_eq!(degraded["ccbd"]["error"], "socket down");
}

#[test]
fn mobile_gateway_projects_payload_reports_registry_health() {
    let tmp = TempDir::new().unwrap();
    let registry = MobileGatewayProjectRegistry::new(vec![
        MobileGatewayProject::new("proj-1", tmp.path().join("repo-a"), None, ok_client()).unwrap(),
        MobileGatewayProject::new(
            "proj-2",
            tmp.path().join("repo-b"),
            Some("Repo B".to_string()),
            FakeClient {
                ping: Err("no daemon".to_string()),
                view: Ok(json!({})),
            },
        )
        .unwrap(),
    ])
    .unwrap();
    let service = MobileGatewayService::with_registry(
        "host-1",
        tmp.path(),
        registry,
        "loopback_server_registry",
        None,
    );

    let payload = service.projects_payload();
    let projects = payload["projects"].as_array().unwrap();
    assert_eq!(payload["schema_version"], 1);
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0]["id"], "proj-1");
    assert_eq!(projects[0]["display_name"], "repo-a");
    assert_eq!(projects[0]["health"], "ok");
    assert_eq!(projects[1]["display_name"], "Repo B");
    assert_eq!(projects[1]["health"], "unreachable");
    assert_eq!(projects[1]["mount_state"], "unavailable");
    assert_eq!(projects[1]["error"], "project unavailable");
}

#[test]
fn mobile_gateway_project_view_redacts_namespace_private_fields() {
    let tmp = TempDir::new().unwrap();
    let service =
        MobileGatewayService::current_project("proj-1", tmp.path(), ok_client(), None).unwrap();

    let payload = service.project_view_payload("proj-1").unwrap();
    let namespace = payload["view"]["namespace"].as_object().unwrap();
    assert_eq!(namespace.get("visible"), Some(&json!(true)));
    assert!(!namespace.contains_key("socket_path"));
    assert!(!namespace.contains_key("session_name"));

    let err = service.project_view_payload("missing").unwrap_err();
    assert_eq!(err.status_code, 404);
    assert_eq!(err.message, "unknown project");
}

#[test]
fn mobile_gateway_dispatch_claims_pairing_and_authenticates_view_routes() {
    let tmp = TempDir::new().unwrap();
    let service =
        MobileGatewayService::current_project("proj-1", tmp.path(), ok_client(), Some(tmp.path()))
            .unwrap();
    let pairing = service
        .create_pairing_payload("http://127.0.0.1:8787", Some("lan"), ["view"], Some(600))
        .unwrap();

    let (claim_status, claimed) = service
        .dispatch_post(
            "/v1/pairing/claim",
            &json!({
                "pairing_code": pairing["pairing_code"],
                "device_name": "Phone",
                "device_id": "dev-1",
            }),
            None,
        )
        .unwrap();
    assert_eq!(claim_status, 201);
    let token = claimed["device_token"].as_str().unwrap();

    let unauth = service
        .dispatch_get("/v1/projects/proj-1/view", None)
        .unwrap_err();
    assert_eq!(unauth.status_code, 401);

    let (view_status, view) = service
        .dispatch_get("/v1/projects/proj-1/view", Some(token))
        .unwrap();
    assert_eq!(view_status, 200);
    assert!(!view["view"]["namespace"]
        .as_object()
        .unwrap()
        .contains_key("socket_path"));

    let (me_status, me) = service.dispatch_get("/v1/devices/me", Some(token)).unwrap();
    assert_eq!(me_status, 200);
    assert_eq!(me["status"], "ok");
    assert_eq!(me["device"]["device_id"], "dev-1");
}

#[test]
fn mobile_gateway_dispatch_self_revoke_preserves_python_error_boundary() {
    let tmp = TempDir::new().unwrap();
    let service =
        MobileGatewayService::current_project("proj-1", tmp.path(), ok_client(), Some(tmp.path()))
            .unwrap();
    let pairing = service
        .create_pairing_payload("http://127.0.0.1:8787", Some("lan"), ["view"], Some(600))
        .unwrap();
    let (_, claimed) = service
        .dispatch_post(
            "/v1/pairing/claim",
            &json!({
                "pairing_code": pairing["pairing_code"],
                "device_name": "Phone",
                "device_id": "dev-1",
            }),
            None,
        )
        .unwrap();
    let token = claimed["device_token"].as_str().unwrap();

    let wrong_device = service
        .dispatch_post("/v1/devices/dev-2/revoke", &json!({}), Some(token))
        .unwrap_err();
    assert_eq!(wrong_device.status_code, 403);
    assert_eq!(wrong_device.message, "device can only revoke itself in G2");

    let (status, revoked) = service
        .dispatch_post("/v1/devices/dev-1/revoke", &json!({}), Some(token))
        .unwrap();
    assert_eq!(status, 200);
    assert_eq!(revoked["status"], "revoked");
    assert_eq!(revoked["device"]["revoked"], true);

    let revoked_token = service
        .dispatch_get("/v1/devices/me", Some(token))
        .unwrap_err();
    assert_eq!(revoked_token.status_code, 401);
    assert_eq!(revoked_token.message, "device token revoked");
}
