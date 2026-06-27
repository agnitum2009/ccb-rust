use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 1;
const PAIRING_HASH_PREFIX: &str = "ccb-mobile-pairing-v1:";
const DEVICE_HASH_PREFIX: &str = "ccb-mobile-device-v1:";
const TERMINAL_HASH_PREFIX: &str = "ccb-mobile-terminal-v1:";
const DEFAULT_PAIRING_EXPIRES_SECONDS: i64 = 10 * 60;
const DEFAULT_TERMINAL_EXPIRES_SECONDS: i64 = 5 * 60;
const DEFAULT_DEVICE_SCOPE: &str = "view";

#[derive(Debug, Error)]
#[error("{message}")]
pub struct MobileGatewayPairingError {
    pub message: String,
    pub status_code: u16,
    pub reason: String,
}

impl MobileGatewayPairingError {
    fn new(message: impl Into<String>, status_code: u16, reason: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status_code,
            reason: reason.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, MobileGatewayPairingError>;

#[derive(Debug, Clone)]
pub struct AuthenticatedDevice {
    record: Value,
}

impl AuthenticatedDevice {
    pub fn device_id(&self) -> String {
        value_str(&self.record, "device_id")
    }

    pub fn scopes(&self) -> BTreeSet<String> {
        scope_set(self.record.get("scopes"))
    }

    pub fn public_payload(&self) -> Value {
        public_device(&self.record)
    }
}

#[derive(Debug, Clone)]
pub struct MobileGatewayPairingStore {
    mobile_dir: PathBuf,
}

impl MobileGatewayPairingStore {
    pub fn new(mobile_dir: impl Into<PathBuf>) -> Self {
        Self {
            mobile_dir: mobile_dir.into(),
        }
    }

    pub fn gateway_path(&self) -> PathBuf {
        self.mobile_dir.join("gateway.json")
    }

    pub fn devices_path(&self) -> PathBuf {
        self.mobile_dir.join("devices.json")
    }

    pub fn pairing_tokens_path(&self) -> PathBuf {
        self.mobile_dir.join("pairing-tokens.jsonl")
    }

    pub fn terminal_tokens_path(&self) -> PathBuf {
        self.mobile_dir.join("terminal-tokens.jsonl")
    }

    pub fn audit_path(&self) -> PathBuf {
        self.mobile_dir.join("audit.jsonl")
    }

    pub fn write_gateway_state(
        &self,
        project_id: &str,
        gateway_url: &str,
        route_provider: &str,
        capabilities: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Value> {
        let capabilities = capabilities
            .into_iter()
            .map(|item| item.as_ref().to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let payload = json!({
            "schema_version": SCHEMA_VERSION,
            "project_id": project_id,
            "gateway_url": gateway_url,
            "route_provider": route_provider,
            "capabilities": capabilities,
            "updated_at": iso_now(),
        });
        write_json(&self.gateway_path(), &payload)?;
        Ok(payload)
    }

    pub fn create_pairing_payload(
        &self,
        project_id: &str,
        gateway_url: &str,
        route_provider: Option<&str>,
        scopes: impl IntoIterator<Item = impl AsRef<str>>,
        expires_seconds: Option<i64>,
    ) -> Result<Value> {
        let now = Utc::now();
        let expires_seconds = expires_seconds
            .unwrap_or(DEFAULT_PAIRING_EXPIRES_SECONDS)
            .max(1);
        let expires_at = now + Duration::seconds(expires_seconds);
        let pairing_code = token_urlsafe(18);
        let pairing_id = random_id("pair");
        let route_provider = route_provider.unwrap_or("lan");
        let scope_list = scope_list_from_iter(scopes);
        let record = json!({
            "schema_version": SCHEMA_VERSION,
            "pairing_id": pairing_id,
            "project_id": project_id,
            "token_hash": token_hash(PAIRING_HASH_PREFIX, &pairing_code),
            "scopes": scope_list,
            "route_provider": route_provider,
            "gateway_url": gateway_url,
            "created_at": iso(now),
            "expires_at": iso(expires_at),
            "claimed_at": Value::Null,
            "claimed_by_device_id": Value::Null,
            "revoked_at": Value::Null,
        });
        append_jsonl(&self.pairing_tokens_path(), &record)?;
        self.append_audit(json!({
            "event": "pairing_token_created",
            "result": "ok",
            "project_id": project_id,
            "pairing_id": pairing_id,
            "scopes": scope_list,
        }))?;
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "pairing_id": pairing_id,
            "pairing_code": pairing_code,
            "project_id": project_id,
            "route_provider": route_provider,
            "gateway_url": gateway_url,
            "claim_endpoint": format!("{}/v1/pairing/claim", gateway_url.trim_end_matches('/')),
            "scopes": scope_list,
            "expires_at": iso(expires_at),
        }))
    }

    pub fn claim_pairing(
        &self,
        pairing_code: &str,
        device_name: &str,
        requested_device_id: Option<&str>,
    ) -> Result<Value> {
        let code = pairing_code.trim();
        if code.is_empty() {
            return Err(MobileGatewayPairingError::new(
                "pairing_code is required",
                400,
                "missing_code",
            ));
        }
        let name = if device_name.trim().is_empty() {
            "Mobile device"
        } else {
            device_name.trim()
        };
        let pairing_token_hash = token_hash(PAIRING_HASH_PREFIX, code);
        let mut pairings = self.pairing_state_by_id()?;
        let record = pairings
            .values_mut()
            .find(|item| value_str(item, "token_hash") == pairing_token_hash)
            .cloned();
        let Some(record) = record else {
            self.append_audit(json!({"event": "pairing_claim_denied", "result": "denied", "reason": "invalid_code"}))?;
            return Err(MobileGatewayPairingError::new(
                "invalid pairing_code",
                401,
                "invalid_code",
            ));
        };
        let pairing_id = value_str(&record, "pairing_id");
        let project_id = value_str(&record, "project_id");
        if value_present(record.get("revoked_at")) {
            self.audit_denied_pairing(&project_id, &pairing_id, "revoked")?;
            return Err(MobileGatewayPairingError::new(
                "pairing_code revoked",
                410,
                "revoked",
            ));
        }
        if value_present(record.get("claimed_at"))
            || value_present(record.get("claimed_by_device_id"))
        {
            self.audit_denied_pairing(&project_id, &pairing_id, "already_claimed")?;
            return Err(MobileGatewayPairingError::new(
                "pairing_code already claimed",
                409,
                "already_claimed",
            ));
        }
        if let Some(expires_at) = parse_utc(record.get("expires_at")) {
            if Utc::now() > expires_at {
                self.audit_denied_pairing(&project_id, &pairing_id, "expired")?;
                return Err(MobileGatewayPairingError::new(
                    "pairing_code expired",
                    410,
                    "expired",
                ));
            }
        }

        let device_id = clean_id(requested_device_id.unwrap_or(""));
        let device_id = if device_id.is_empty() {
            random_id("dev")
        } else {
            device_id
        };
        let mut devices = self.read_devices();
        if devices.iter().any(|item| {
            value_str(item, "device_id") == device_id && !value_present(item.get("revoked_at"))
        }) {
            self.append_audit(json!({
                "event": "pairing_claim_denied",
                "result": "denied",
                "project_id": project_id,
                "pairing_id": pairing_id,
                "device_id": device_id,
                "reason": "device_id_exists",
            }))?;
            return Err(MobileGatewayPairingError::new(
                "device_id already exists",
                409,
                "device_id_exists",
            ));
        }

        let now = Utc::now();
        let device_token = token_urlsafe(32);
        let scopes = scope_list(record.get("scopes"));
        let device_record = json!({
            "schema_version": SCHEMA_VERSION,
            "device_id": device_id,
            "name": name,
            "project_id": project_id,
            "pairing_id": pairing_id,
            "token_hash": token_hash(DEVICE_HASH_PREFIX, &device_token),
            "scopes": scopes,
            "route_provider": value_str_or(&record, "route_provider", "lan"),
            "gateway_url": value_str(&record, "gateway_url"),
            "created_at": iso(now),
            "last_seen_at": Value::Null,
            "revoked_at": Value::Null,
        });
        devices.retain(|item| value_str(item, "device_id") != device_id);
        devices.push(device_record.clone());
        write_json(
            &self.devices_path(),
            &json!({"schema_version": SCHEMA_VERSION, "devices": devices}),
        )?;

        let mut updated_record = record.clone();
        if let Some(obj) = updated_record.as_object_mut() {
            obj.insert("claimed_at".into(), Value::String(iso(now)));
            obj.insert(
                "claimed_by_device_id".into(),
                Value::String(device_id.clone()),
            );
        }
        append_jsonl(&self.pairing_tokens_path(), &updated_record)?;
        self.append_audit(json!({
            "event": "pairing_claimed",
            "result": "ok",
            "project_id": project_id,
            "pairing_id": pairing_id,
            "device_id": device_id,
            "scopes": scopes,
        }))?;
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "status": "ok",
            "device": public_device(&device_record),
            "device_token": device_token,
            "host_profile": {
                "host_id": project_id,
                "project_id": project_id,
                "device_id": device_id,
                "route_provider": value_str_or(&device_record, "route_provider", "lan"),
                "gateway_url": value_str(&device_record, "gateway_url"),
                "scopes": scopes,
            },
        }))
    }

    pub fn authenticate_device(
        &self,
        device_token: &str,
        required_scopes: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<AuthenticatedDevice> {
        let token = device_token.trim();
        if token.is_empty() {
            return Err(MobileGatewayPairingError::new(
                "device bearer token is required",
                401,
                "missing_token",
            ));
        }
        let token_hash = token_hash(DEVICE_HASH_PREFIX, token);
        let required = scope_set_from_iter(required_scopes);
        let mut devices = self.read_devices();
        for index in 0..devices.len() {
            if value_str(&devices[index], "token_hash") != token_hash {
                continue;
            }
            let device_id = value_str(&devices[index], "device_id");
            let project_id = value_str(&devices[index], "project_id");
            if value_present(devices[index].get("revoked_at")) {
                self.append_audit(json!({"event": "device_auth_denied", "result": "denied", "project_id": project_id, "device_id": device_id, "reason": "revoked"}))?;
                return Err(MobileGatewayPairingError::new(
                    "device token revoked",
                    401,
                    "revoked",
                ));
            }
            let scopes = scope_set(devices[index].get("scopes"));
            let missing = required.difference(&scopes).cloned().collect::<Vec<_>>();
            if !missing.is_empty() {
                self.append_audit(json!({"event": "device_auth_denied", "result": "denied", "project_id": project_id, "device_id": device_id, "reason": "missing_scope", "scopes": missing}))?;
                return Err(MobileGatewayPairingError::new(
                    "device scope denied",
                    403,
                    "missing_scope",
                ));
            }
            let mut updated = devices[index].clone();
            if let Some(obj) = updated.as_object_mut() {
                obj.insert("last_seen_at".into(), Value::String(iso_now()));
            }
            devices[index] = updated.clone();
            write_json(
                &self.devices_path(),
                &json!({"schema_version": SCHEMA_VERSION, "devices": devices}),
            )?;
            self.append_audit(json!({"event": "device_auth_ok", "result": "ok", "project_id": project_id, "device_id": device_id, "scopes": required.iter().cloned().collect::<Vec<_>>()}))?;
            return Ok(AuthenticatedDevice { record: updated });
        }
        self.append_audit(
            json!({"event": "device_auth_denied", "result": "denied", "reason": "invalid_token"}),
        )?;
        Err(MobileGatewayPairingError::new(
            "invalid device token",
            401,
            "invalid_token",
        ))
    }

    pub fn revoke_device(&self, device_id: &str, device_token: &str) -> Result<Value> {
        let requested = clean_id(device_id);
        if requested.is_empty() {
            return Err(MobileGatewayPairingError::new(
                "device_id is required",
                400,
                "missing_device_id",
            ));
        }
        let auth = self.authenticate_device(device_token, std::iter::empty::<&str>())?;
        if auth.device_id() != requested {
            self.append_audit(json!({
                "event": "device_revoke_denied",
                "result": "denied",
                "project_id": value_str(&auth.record, "project_id"),
                "device_id": auth.device_id(),
                "target_device_id": requested,
                "reason": "self_revoke_only",
            }))?;
            return Err(MobileGatewayPairingError::new(
                "device can only revoke itself in G2",
                403,
                "self_revoke_only",
            ));
        }
        self.revoke_device_record(&requested, Some(&auth.device_id()), "self_revoked")
    }

    pub fn list_devices(&self) -> Vec<Value> {
        self.read_devices()
            .iter()
            .map(public_device)
            .collect::<Vec<_>>()
    }

    pub fn create_terminal_handle(
        &self,
        project_id: &str,
        device_id: &str,
        target_epoch: i64,
        target_summary: Value,
        geometry: Value,
        expires_seconds: Option<i64>,
    ) -> Result<Value> {
        let now = Utc::now();
        let expires_at = now
            + Duration::seconds(
                expires_seconds
                    .unwrap_or(DEFAULT_TERMINAL_EXPIRES_SECONDS)
                    .max(1),
            );
        let terminal_id = random_id("term");
        let terminal_token = token_urlsafe(32);
        let record = json!({
            "schema_version": SCHEMA_VERSION,
            "terminal_id": terminal_id,
            "project_id": project_id,
            "device_id": device_id,
            "token_hash": token_hash(TERMINAL_HASH_PREFIX, &terminal_token),
            "created_at": iso(now),
            "expires_at": iso(expires_at),
            "target_epoch": target_epoch,
            "target_summary": object_or_empty(target_summary),
            "geometry": object_or_empty(geometry),
            "last_input_seq": 0,
            "last_output_seq": 0,
            "disconnected_at": Value::Null,
            "closed_at": Value::Null,
            "revoked_at": Value::Null,
        });
        append_jsonl(&self.terminal_tokens_path(), &record)?;
        self.append_audit(json!({
            "event": "terminal_token_created",
            "result": "ok",
            "project_id": project_id,
            "device_id": device_id,
            "terminal_id": terminal_id,
            "target_epoch": target_epoch,
        }))?;
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "terminal_id": terminal_id,
            "terminal_token": terminal_token,
            "expires_at": iso(expires_at),
            "target_epoch": target_epoch,
            "target_summary": record.get("target_summary").cloned().unwrap_or_else(|| json!({})),
        }))
    }

    pub fn authenticate_terminal_token(
        &self,
        terminal_id: &str,
        terminal_token: &str,
        resume_cursor: Option<i64>,
    ) -> Result<Value> {
        let requested = terminal_id.trim();
        let token = terminal_token.trim();
        if requested.is_empty() || token.is_empty() {
            return Err(MobileGatewayPairingError::new(
                "terminal token is required",
                401,
                "missing_terminal_token",
            ));
        }
        let token_hash = token_hash(TERMINAL_HASH_PREFIX, token);
        let record = self.terminal_state_by_id()?.remove(requested);
        let Some(record) = record else {
            self.append_audit(json!({"event": "terminal_auth_denied", "result": "denied", "terminal_id": requested, "reason": "invalid_token"}))?;
            return Err(MobileGatewayPairingError::new(
                "invalid terminal token",
                401,
                "invalid_token",
            ));
        };
        if value_str(&record, "token_hash") != token_hash {
            self.append_audit(json!({"event": "terminal_auth_denied", "result": "denied", "terminal_id": requested, "reason": "invalid_token"}))?;
            return Err(MobileGatewayPairingError::new(
                "invalid terminal token",
                401,
                "invalid_token",
            ));
        }
        self.validate_terminal_record(&record)?;
        let record = self.validate_resume_cursor(record, resume_cursor)?;
        self.append_audit(json!({
            "event": "terminal_auth_ok",
            "result": "ok",
            "project_id": value_str(&record, "project_id"),
            "device_id": value_str(&record, "device_id"),
            "terminal_id": requested,
            "resume_cursor": resume_cursor,
        }))?;
        Ok(record)
    }

    pub fn record_terminal_input_sequence(
        &self,
        terminal_id: &str,
        terminal_token: &str,
        sequence: i64,
    ) -> Result<Value> {
        let record =
            self.require_terminal_record(terminal_id, terminal_token, "terminal_input_denied")?;
        self.validate_terminal_record(&record)?;
        let last_seq = value_i64(record.get("last_input_seq"), 0);
        if sequence <= last_seq {
            self.append_audit(json!({
                "event": "terminal_input_denied",
                "result": "denied",
                "project_id": value_str(&record, "project_id"),
                "device_id": value_str(&record, "device_id"),
                "terminal_id": terminal_id,
                "reason": "replayed_sequence",
                "last_input_seq": last_seq,
                "sequence": sequence,
            }))?;
            return Err(MobileGatewayPairingError::new(
                "terminal input sequence replayed",
                409,
                "replayed_sequence",
            ));
        }
        let updated = with_field(record, "last_input_seq", json!(sequence));
        append_jsonl(&self.terminal_tokens_path(), &updated)?;
        self.append_audit(json!({
            "event": "terminal_input_accepted",
            "result": "ok",
            "project_id": value_str(&updated, "project_id"),
            "device_id": value_str(&updated, "device_id"),
            "terminal_id": terminal_id,
            "sequence": sequence,
        }))?;
        Ok(updated)
    }

    pub fn record_terminal_output_sequence(
        &self,
        terminal_id: &str,
        terminal_token: &str,
        sequence: i64,
    ) -> Result<Value> {
        let record =
            self.require_terminal_record(terminal_id, terminal_token, "terminal_output_denied")?;
        self.validate_terminal_record(&record)?;
        let last_seq = value_i64(record.get("last_output_seq"), 0);
        if sequence <= last_seq {
            return Ok(record);
        }
        let updated = with_field(record, "last_output_seq", json!(sequence));
        append_jsonl(&self.terminal_tokens_path(), &updated)?;
        Ok(updated)
    }

    pub fn mark_terminal_disconnected(
        &self,
        terminal_id: &str,
        terminal_token: &str,
        reason: Option<&str>,
    ) -> Result<Value> {
        let record = self.require_terminal_record(
            terminal_id,
            terminal_token,
            "terminal_disconnect_denied",
        )?;
        self.validate_terminal_record(&record)?;
        let reason = reason.unwrap_or("transport_disconnected");
        let updated = with_fields(
            record,
            [
                ("disconnected_at", json!(iso_now())),
                ("disconnected_reason", json!(reason)),
            ],
        );
        append_jsonl(&self.terminal_tokens_path(), &updated)?;
        self.append_audit(json!({
            "event": "terminal_disconnected",
            "result": "ok",
            "project_id": value_str(&updated, "project_id"),
            "device_id": value_str(&updated, "device_id"),
            "terminal_id": terminal_id,
            "reason": reason,
        }))?;
        Ok(updated)
    }

    pub fn close_terminal_handle(
        &self,
        terminal_id: &str,
        terminal_token: &str,
        reason: Option<&str>,
    ) -> Result<Value> {
        let record =
            self.require_terminal_record(terminal_id, terminal_token, "terminal_close_denied")?;
        if value_present(record.get("closed_at")) {
            return Ok(record);
        }
        let reason = reason.unwrap_or("client_closed");
        let updated = with_fields(
            record,
            [
                ("closed_at", json!(iso_now())),
                ("closed_reason", json!(reason)),
            ],
        );
        append_jsonl(&self.terminal_tokens_path(), &updated)?;
        self.append_audit(json!({
            "event": "terminal_closed",
            "result": "ok",
            "project_id": value_str(&updated, "project_id"),
            "device_id": value_str(&updated, "device_id"),
            "terminal_id": terminal_id,
            "reason": reason,
        }))?;
        Ok(updated)
    }

    pub fn revoke_device_locally(&self, device_id: &str, reason: Option<&str>) -> Result<Value> {
        let requested = clean_id(device_id);
        if requested.is_empty() {
            return Err(MobileGatewayPairingError::new(
                "device_id is required",
                400,
                "missing_device_id",
            ));
        }
        self.revoke_device_record(&requested, None, reason.unwrap_or("host_revoked"))
    }

    fn revoke_device_record(
        &self,
        device_id: &str,
        revoked_by_device_id: Option<&str>,
        reason: &str,
    ) -> Result<Value> {
        let now = iso_now();
        let mut devices = self.read_devices();
        for index in 0..devices.len() {
            if value_str(&devices[index], "device_id") != device_id {
                continue;
            }
            let mut updated = devices[index].clone();
            if !value_present(updated.get("revoked_at")) {
                if let Some(obj) = updated.as_object_mut() {
                    obj.insert("revoked_at".into(), Value::String(now.clone()));
                }
            }
            devices[index] = updated.clone();
            write_json(
                &self.devices_path(),
                &json!({"schema_version": SCHEMA_VERSION, "devices": devices}),
            )?;
            self.append_audit(json!({
                "event": "device_revoked",
                "result": "ok",
                "project_id": value_str(&updated, "project_id"),
                "device_id": device_id,
                "revoked_by_device_id": revoked_by_device_id,
                "reason": reason,
                "revoked_terminal_count": 0,
            }))?;
            return Ok(json!({
                "schema_version": SCHEMA_VERSION,
                "status": "revoked",
                "device": public_device(&updated),
                "revoked_terminal_count": 0,
            }));
        }
        Err(MobileGatewayPairingError::new(
            "device not found",
            404,
            "not_found",
        ))
    }

    fn pairing_state_by_id(&self) -> Result<HashMap<String, Value>> {
        let mut result = HashMap::new();
        for record in read_jsonl(&self.pairing_tokens_path())? {
            let pairing_id = value_str(&record, "pairing_id");
            if !pairing_id.is_empty() {
                result.insert(pairing_id, record);
            }
        }
        Ok(result)
    }

    fn read_devices(&self) -> Vec<Value> {
        let Ok(text) = fs::read_to_string(self.devices_path()) else {
            return Vec::new();
        };
        let Ok(payload) = serde_json::from_str::<Value>(&text) else {
            return Vec::new();
        };
        payload
            .get("devices")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.is_object())
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn terminal_state_by_id(&self) -> Result<HashMap<String, Value>> {
        let mut result = HashMap::new();
        for record in read_jsonl(&self.terminal_tokens_path())? {
            let terminal_id = value_str(&record, "terminal_id");
            if !terminal_id.is_empty() {
                result.insert(terminal_id, record);
            }
        }
        Ok(result)
    }

    fn require_terminal_record(
        &self,
        terminal_id: &str,
        terminal_token: &str,
        denied_event: &str,
    ) -> Result<Value> {
        let requested = terminal_id.trim();
        let token_hash = token_hash(TERMINAL_HASH_PREFIX, terminal_token.trim());
        let record = self.terminal_state_by_id()?.remove(requested);
        let Some(record) = record else {
            self.append_audit(json!({"event": denied_event, "result": "denied", "terminal_id": requested, "reason": "invalid_token"}))?;
            return Err(MobileGatewayPairingError::new(
                "invalid terminal token",
                401,
                "invalid_token",
            ));
        };
        if value_str(&record, "token_hash") != token_hash {
            self.append_audit(json!({"event": denied_event, "result": "denied", "terminal_id": requested, "reason": "invalid_token"}))?;
            return Err(MobileGatewayPairingError::new(
                "invalid terminal token",
                401,
                "invalid_token",
            ));
        }
        Ok(record)
    }

    fn validate_terminal_record(&self, record: &Value) -> Result<()> {
        let terminal_id = value_str(record, "terminal_id");
        let project_id = value_str(record, "project_id");
        let device_id = value_str(record, "device_id");
        if value_present(record.get("revoked_at")) {
            self.audit_terminal_denied(&project_id, &device_id, &terminal_id, "revoked")?;
            return Err(MobileGatewayPairingError::new(
                "terminal token revoked",
                401,
                "revoked",
            ));
        }
        if self.is_device_revoked(&device_id) {
            self.audit_terminal_denied(&project_id, &device_id, &terminal_id, "device_revoked")?;
            return Err(MobileGatewayPairingError::new(
                "terminal device revoked",
                401,
                "device_revoked",
            ));
        }
        if value_present(record.get("closed_at")) {
            self.audit_terminal_denied(&project_id, &device_id, &terminal_id, "closed")?;
            return Err(MobileGatewayPairingError::new(
                "terminal already closed",
                410,
                "closed",
            ));
        }
        if parse_utc(record.get("expires_at")).is_some_and(|expires_at| Utc::now() > expires_at) {
            self.audit_terminal_denied(&project_id, &device_id, &terminal_id, "expired")?;
            return Err(MobileGatewayPairingError::new(
                "terminal token expired",
                410,
                "expired",
            ));
        }
        Ok(())
    }

    fn validate_resume_cursor(&self, record: Value, resume_cursor: Option<i64>) -> Result<Value> {
        let disconnected = value_present(record.get("disconnected_at"));
        let last_output_seq = value_i64(record.get("last_output_seq"), 0);
        let terminal_id = value_str(&record, "terminal_id");
        let project_id = value_str(&record, "project_id");
        let device_id = value_str(&record, "device_id");
        if disconnected && resume_cursor.is_none() {
            self.append_audit(json!({
                "event": "terminal_resume_denied",
                "result": "denied",
                "project_id": project_id,
                "device_id": device_id,
                "terminal_id": terminal_id,
                "reason": "missing_resume_cursor",
                "last_output_seq": last_output_seq,
            }))?;
            return Err(MobileGatewayPairingError::new(
                "resume_cursor is required after terminal disconnect",
                409,
                "missing_resume_cursor",
            ));
        }
        if let Some(cursor) = resume_cursor {
            if cursor != last_output_seq {
                self.append_audit(json!({
                    "event": "terminal_resume_denied",
                    "result": "denied",
                    "project_id": project_id,
                    "device_id": device_id,
                    "terminal_id": terminal_id,
                    "reason": "stale_resume_cursor",
                    "last_output_seq": last_output_seq,
                    "resume_cursor": cursor,
                }))?;
                return Err(MobileGatewayPairingError::new(
                    "terminal resume cursor is stale",
                    409,
                    "stale_resume_cursor",
                ));
            }
            let updated = with_fields(
                record,
                [
                    ("disconnected_at", Value::Null),
                    ("resumed_at", json!(iso_now())),
                    ("last_resume_cursor", json!(cursor)),
                ],
            );
            append_jsonl(&self.terminal_tokens_path(), &updated)?;
            self.append_audit(json!({
                "event": "terminal_resume_ok",
                "result": "ok",
                "project_id": project_id,
                "device_id": device_id,
                "terminal_id": terminal_id,
                "resume_cursor": cursor,
            }))?;
            return Ok(updated);
        }
        Ok(record)
    }

    fn is_device_revoked(&self, device_id: &str) -> bool {
        !device_id.is_empty()
            && self.read_devices().iter().any(|record| {
                value_str(record, "device_id") == device_id
                    && value_present(record.get("revoked_at"))
            })
    }

    fn audit_terminal_denied(
        &self,
        project_id: &str,
        device_id: &str,
        terminal_id: &str,
        reason: &str,
    ) -> Result<()> {
        self.append_audit(json!({
            "event": "terminal_auth_denied",
            "result": "denied",
            "project_id": project_id,
            "device_id": device_id,
            "terminal_id": terminal_id,
            "reason": reason,
        }))
    }

    fn audit_denied_pairing(&self, project_id: &str, pairing_id: &str, reason: &str) -> Result<()> {
        self.append_audit(json!({
            "event": "pairing_claim_denied",
            "result": "denied",
            "project_id": project_id,
            "pairing_id": pairing_id,
            "reason": reason,
        }))
    }

    fn append_audit(&self, payload: Value) -> Result<()> {
        let mut entry = Map::new();
        entry.insert("schema_version".into(), json!(SCHEMA_VERSION));
        entry.insert("timestamp".into(), Value::String(iso_now()));
        if let Some(obj) = payload.as_object() {
            for (key, value) in obj {
                if !value.is_null() && value.as_str() != Some("") {
                    entry.insert(key.clone(), value.clone());
                }
            }
        }
        append_jsonl(&self.audit_path(), &Value::Object(entry))
    }
}

fn write_json(path: &Path, payload: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    let tmp = path.with_file_name(format!(
        ".{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp")
    ));
    let text = serde_json::to_string_pretty(payload).map_err(json_error)? + "\n";
    fs::write(&tmp, text).map_err(io_error)?;
    fs::rename(tmp, path).map_err(io_error)?;
    Ok(())
}

fn append_jsonl(path: &Path, payload: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    let mut text = serde_json::to_string(payload).map_err(json_error)?;
    text.push('\n');
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_error)?;
    file.write_all(text.as_bytes()).map_err(io_error)
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    let mut records = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let payload = serde_json::from_str::<Value>(trimmed).map_err(json_error)?;
        if payload.is_object() {
            records.push(payload);
        }
    }
    Ok(records)
}

fn token_hash(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{prefix}{value}").as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn public_device(record: &Value) -> Value {
    json!({
        "device_id": record.get("device_id").cloned().unwrap_or(Value::Null),
        "name": record.get("name").cloned().unwrap_or(Value::Null),
        "project_id": record.get("project_id").cloned().unwrap_or(Value::Null),
        "pairing_id": record.get("pairing_id").cloned().unwrap_or(Value::Null),
        "scopes": scope_list(record.get("scopes")),
        "route_provider": record.get("route_provider").cloned().unwrap_or(Value::Null),
        "gateway_url": record.get("gateway_url").cloned().unwrap_or(Value::Null),
        "created_at": record.get("created_at").cloned().unwrap_or(Value::Null),
        "last_seen_at": record.get("last_seen_at").cloned().unwrap_or(Value::Null),
        "revoked": value_present(record.get("revoked_at")),
        "revoked_at": record.get("revoked_at").cloned().unwrap_or(Value::Null),
    })
}

fn scope_list(value: Option<&Value>) -> Vec<String> {
    let scopes = scope_set(value);
    if scopes.is_empty() {
        vec![DEFAULT_DEVICE_SCOPE.to_string()]
    } else {
        scopes.into_iter().collect()
    }
}

fn scope_list_from_iter(scopes: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    let set = scope_set_from_iter(scopes);
    if set.is_empty() {
        vec![DEFAULT_DEVICE_SCOPE.to_string()]
    } else {
        set.into_iter().collect()
    }
}

fn scope_set(value: Option<&Value>) -> BTreeSet<String> {
    match value {
        Some(Value::String(text)) => scope_set_from_iter([text.as_str()]),
        Some(Value::Array(items)) => scope_set_from_iter(items.iter().map(|item| match item {
            Value::String(text) => text.as_str(),
            _ => "",
        })),
        _ => BTreeSet::new(),
    }
}

fn scope_set_from_iter(scopes: impl IntoIterator<Item = impl AsRef<str>>) -> BTreeSet<String> {
    scopes
        .into_iter()
        .map(|item| item.as_ref().trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn clean_id(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect()
}

fn object_or_empty(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        json!({})
    }
}

fn with_field(mut record: Value, key: &str, value: Value) -> Value {
    if let Some(obj) = record.as_object_mut() {
        obj.insert(key.to_string(), value);
    }
    record
}

fn with_fields<'a>(mut record: Value, fields: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    if let Some(obj) = record.as_object_mut() {
        for (key, value) in fields {
            obj.insert(key.to_string(), value);
        }
    }
    record
}

fn value_i64(value: Option<&Value>, fallback: i64) -> i64 {
    value.and_then(Value::as_i64).unwrap_or(fallback)
}

fn value_str(record: &Value, key: &str) -> String {
    record
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn value_str_or(record: &Value, key: &str, fallback: &str) -> String {
    let value = value_str(record, key);
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn value_present(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Null) | None => false,
        Some(Value::String(text)) => !text.is_empty(),
        Some(value) => !value.is_null(),
    }
}

fn parse_utc(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let text = value?.as_str()?;
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn token_urlsafe(parts: usize) -> String {
    let mut out = String::new();
    while out.len() < parts * 2 {
        out.push_str(&Uuid::new_v4().simple().to_string());
    }
    out.truncate(parts * 2);
    out
}

fn random_id(prefix: &str) -> String {
    let hex = Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &hex[..16])
}

fn iso_now() -> String {
    iso(Utc::now())
}

fn iso(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn io_error(error: std::io::Error) -> MobileGatewayPairingError {
    MobileGatewayPairingError::new(error.to_string(), 500, "io_error")
}

fn json_error(error: serde_json::Error) -> MobileGatewayPairingError {
    MobileGatewayPairingError::new(error.to_string(), 500, "json_error")
}
