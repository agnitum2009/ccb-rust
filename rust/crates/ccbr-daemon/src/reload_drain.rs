//! Mirrors Python `lib/ccbd/reload_drain.py`.

use crate::models::api_models::common::SCHEMA_VERSION;
use ccbr_storage::json::JsonStore;
use ccbr_storage::paths::PathLayout;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const QUEUE_RECORD_TYPE: &str = "ccbd_reload_drain_queue";
const INTENT_KINDS: &[&str] = &["unload", "replace"];
const PHASES: &[&str] = &[
    "pending_unload",
    "pending_replace",
    "draining",
    "retiring",
    "retired",
    "rejected",
];
const STATUSES: &[&str] = &[
    "pending",
    "waiting",
    "idle_ready",
    "timed_out",
    "rejected_queue_full",
    "retired",
];
const TERMINAL_STATUSES: &[&str] = &["timed_out", "rejected_queue_full", "retired"];

/// Bounds for the drain queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainBounds {
    pub max_pending: usize,
    pub timeout_s: f64,
    pub max_age_s: f64,
}

impl Default for DrainBounds {
    fn default() -> Self {
        Self {
            max_pending: 16,
            timeout_s: 300.0,
            max_age_s: 900.0,
        }
    }
}

impl DrainBounds {
    pub fn new(max_pending: usize, timeout_s: f64, max_age_s: f64) -> Self {
        if max_pending == 0 {
            panic!("max_pending must be positive");
        }
        if timeout_s <= 0.0 {
            panic!("timeout_s must be positive");
        }
        if max_age_s <= 0.0 {
            panic!("max_age_s must be positive");
        }
        Self {
            max_pending,
            timeout_s,
            max_age_s,
        }
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "max_pending": self.max_pending,
            "timeout_s": self.timeout_s,
            "max_age_s": self.max_age_s,
        })
    }

    pub fn from_record(record: Option<&serde_json::Map<String, serde_json::Value>>) -> Self {
        let payload = record.cloned().unwrap_or_default();
        let max_pending = payload
            .get("max_pending")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(16);
        let timeout_s = payload
            .get("timeout_s")
            .and_then(|v| v.as_f64())
            .unwrap_or(300.0);
        let max_age_s = payload
            .get("max_age_s")
            .and_then(|v| v.as_f64())
            .unwrap_or(900.0);
        Self::new(max_pending, timeout_s, max_age_s)
    }
}

/// A single drain intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainIntent {
    pub intent_id: String,
    pub intent_kind: String,
    pub agent_name: String,
    pub created_at_s: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_config_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_config_signature: Option<String>,
}

impl DrainIntent {
    pub fn new(
        intent_id: impl Into<String>,
        intent_kind: impl Into<String>,
        agent_name: impl Into<String>,
        created_at_s: f64,
        reason: Option<String>,
        old_config_signature: Option<String>,
        new_config_signature: Option<String>,
    ) -> Self {
        let intent_id = intent_id.into();
        let intent_kind = intent_kind.into();
        let agent_name = agent_name.into();
        if intent_id.trim().is_empty() {
            panic!("intent_id cannot be empty");
        }
        if !INTENT_KINDS.contains(&intent_kind.as_str()) {
            panic!("invalid drain intent kind: {intent_kind:?}");
        }
        if agent_name.trim().is_empty() {
            panic!("agent_name cannot be empty");
        }
        Self {
            intent_id,
            intent_kind,
            agent_name,
            created_at_s,
            reason,
            old_config_signature,
            new_config_signature,
        }
    }

    pub fn initial_phase(&self) -> String {
        format!("pending_{}", self.intent_kind)
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "intent_id": self.intent_id,
            "intent_kind": self.intent_kind,
            "agent_name": self.agent_name,
            "created_at_s": self.created_at_s,
            "reason": self.reason,
            "old_config_signature": self.old_config_signature,
            "new_config_signature": self.new_config_signature,
        })
    }

    pub fn from_record(record: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::new(
            record
                .get("intent_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("intent_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("agent_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("created_at_s")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            clean_text(record.get("reason").and_then(|v| v.as_str())),
            clean_text(record.get("old_config_signature").and_then(|v| v.as_str())),
            clean_text(record.get("new_config_signature").and_then(|v| v.as_str())),
        )
    }
}

/// A drain record in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainRecord {
    pub intent: DrainIntent,
    pub phase: String,
    pub status: String,
    pub created_at_s: f64,
    pub updated_at_s: f64,
    pub deadline_at_s: f64,
    pub max_age_deadline_at_s: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub busy: Option<bool>,
    pub transition_count: u32,
}

impl DrainRecord {
    /// Arity mirrors the Python `DrainRecord.__init__` constructor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        intent: DrainIntent,
        phase: impl Into<String>,
        status: impl Into<String>,
        created_at_s: f64,
        updated_at_s: f64,
        deadline_at_s: f64,
        max_age_deadline_at_s: f64,
        reason: Option<String>,
        busy: Option<bool>,
        transition_count: u32,
    ) -> Self {
        let phase = phase.into();
        let status = status.into();
        if !PHASES.contains(&phase.as_str()) {
            panic!("invalid drain phase: {phase:?}");
        }
        if !STATUSES.contains(&status.as_str()) {
            panic!("invalid drain status: {status:?}");
        }
        Self {
            intent,
            phase,
            status,
            created_at_s,
            updated_at_s,
            deadline_at_s,
            max_age_deadline_at_s,
            reason,
            busy,
            transition_count,
        }
    }

    pub fn terminal(&self) -> bool {
        TERMINAL_STATUSES.contains(&self.status.as_str())
    }

    pub fn with_transition(
        &self,
        phase: impl Into<String>,
        status: impl Into<String>,
        now_s: f64,
        reason: impl Into<String>,
        busy: Option<bool>,
    ) -> Self {
        Self::new(
            self.intent.clone(),
            phase,
            status,
            self.created_at_s,
            now_s,
            self.deadline_at_s,
            self.max_age_deadline_at_s,
            Some(reason.into()),
            busy,
            self.transition_count + 1,
        )
    }

    pub fn pending(intent: &DrainIntent, bounds: &DrainBounds, now_s: f64) -> Self {
        Self::new(
            intent.clone(),
            intent.initial_phase(),
            "pending",
            now_s,
            now_s,
            now_s + bounds.timeout_s,
            intent.created_at_s + bounds.max_age_s,
            intent.reason.clone(),
            None,
            0,
        )
    }

    pub fn rejected_queue_full(intent: &DrainIntent, bounds: &DrainBounds, now_s: f64) -> Self {
        Self::new(
            intent.clone(),
            "rejected",
            "rejected_queue_full",
            now_s,
            now_s,
            now_s + bounds.timeout_s,
            intent.created_at_s + bounds.max_age_s,
            Some("pending drain queue is full".to_string()),
            None,
            0,
        )
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "intent": self.intent.to_record(),
            "phase": self.phase,
            "status": self.status,
            "created_at_s": self.created_at_s,
            "updated_at_s": self.updated_at_s,
            "deadline_at_s": self.deadline_at_s,
            "max_age_deadline_at_s": self.max_age_deadline_at_s,
            "reason": self.reason,
            "busy": self.busy,
            "transition_count": self.transition_count,
        })
    }

    pub fn from_record(record: &serde_json::Map<String, serde_json::Value>) -> Self {
        let intent = if let Some(serde_json::Value::Object(obj)) = record.get("intent") {
            DrainIntent::from_record(obj)
        } else {
            DrainIntent::new("unknown", "unload", "unknown", 0.0, None, None, None)
        };
        Self::new(
            intent,
            record
                .get("phase")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("created_at_s")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            record
                .get("updated_at_s")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            record
                .get("deadline_at_s")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            record
                .get("max_age_deadline_at_s")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            clean_text(record.get("reason").and_then(|v| v.as_str())),
            record.get("busy").and_then(|v| v.as_bool()),
            record
                .get("transition_count")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32)
                .unwrap_or(0),
        )
    }
}

/// Result of enqueueing an intent.
#[derive(Debug, Clone)]
pub struct DrainQueueResult {
    pub queue: DrainQueue,
    pub record: DrainRecord,
    pub accepted: bool,
}

/// Predicate used to decide whether a record is busy.
pub type BusyPredicate = Box<dyn Fn(&DrainRecord) -> bool>;

/// A queue of drain records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainQueue {
    pub bounds: DrainBounds,
    pub records: Vec<DrainRecord>,
}

impl DrainQueue {
    pub fn empty(bounds: Option<DrainBounds>) -> Self {
        Self {
            bounds: bounds.unwrap_or_default(),
            records: Vec::new(),
        }
    }

    pub fn pending_count(&self) -> usize {
        self.records.iter().filter(|r| !r.terminal()).count()
    }

    pub fn enqueue(&self, intent: &DrainIntent, now_s: f64) -> DrainQueueResult {
        if now_s > intent.created_at_s + self.bounds.max_age_s {
            let expired = DrainRecord::pending(intent, &self.bounds, now_s).with_transition(
                "draining",
                "timed_out",
                now_s,
                "intent age exceeds max_age_s",
                None,
            );
            return DrainQueueResult {
                queue: self.clone(),
                record: expired,
                accepted: false,
            };
        }
        if self.pending_count() >= self.bounds.max_pending {
            let rejected = DrainRecord::rejected_queue_full(intent, &self.bounds, now_s);
            return DrainQueueResult {
                queue: self.clone(),
                record: rejected,
                accepted: false,
            };
        }
        let record = DrainRecord::pending(intent, &self.bounds, now_s);
        let mut queue = self.clone();
        queue.records.push(record.clone());
        DrainQueueResult {
            queue,
            record,
            accepted: true,
        }
    }

    pub fn replace_record(&self, updated: &DrainRecord) -> Self {
        let mut records = self.records.clone();
        let mut replaced = false;
        for record in records.iter_mut() {
            if record.intent.intent_id == updated.intent.intent_id {
                *record = updated.clone();
                replaced = true;
                break;
            }
        }
        if !replaced {
            return self.clone();
        }
        Self {
            records,
            ..self.clone()
        }
    }

    pub fn active_records_for(&self, agent_name: &str) -> Vec<&DrainRecord> {
        self.records
            .iter()
            .filter(|r| r.intent.agent_name == agent_name && !r.terminal())
            .collect()
    }

    pub fn blocks_new_work_for(&self, agent_name: &str) -> bool {
        !self.active_records_for(agent_name).is_empty()
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": QUEUE_RECORD_TYPE,
            "bounds": self.bounds.to_record(),
            "records": self.records.iter().map(|r| r.to_record()).collect::<Vec<_>>(),
        })
    }

    pub fn from_record(record: &serde_json::Map<String, serde_json::Value>) -> Self {
        if record.get("schema_version") != Some(&serde_json::json!(SCHEMA_VERSION)) {
            panic!("schema_version must be {SCHEMA_VERSION}");
        }
        if record.get("record_type") != Some(&serde_json::json!(QUEUE_RECORD_TYPE)) {
            panic!("record_type must be '{QUEUE_RECORD_TYPE}'");
        }
        let bounds = if let Some(serde_json::Value::Object(obj)) = record.get("bounds") {
            DrainBounds::from_record(Some(obj))
        } else {
            DrainBounds::default()
        };
        let records: Vec<DrainRecord> = record
            .get("records")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        if let serde_json::Value::Object(obj) = v {
                            Some(DrainRecord::from_record(obj))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { bounds, records }
    }
}

/// Plan the next state transition for a drain record.
pub fn plan_drain_transition(
    record: &DrainRecord,
    now_s: f64,
    is_busy: &dyn Fn(&DrainRecord) -> bool,
) -> DrainRecord {
    if record.terminal() {
        return record.clone();
    }
    if record.status == "idle_ready" {
        return record.clone();
    }
    if now_s >= record.max_age_deadline_at_s {
        return record.with_transition(
            "draining",
            "timed_out",
            now_s,
            "drain max_age_s exceeded",
            None,
        );
    }
    if now_s >= record.deadline_at_s {
        return record.with_transition(
            "draining",
            "timed_out",
            now_s,
            "drain timeout_s exceeded",
            None,
        );
    }
    let busy = is_busy(record);
    if busy {
        record.with_transition(
            "draining",
            "waiting",
            now_s,
            "agent is busy; drain remains bounded and pending",
            Some(true),
        )
    } else {
        record.with_transition(
            "retiring",
            "idle_ready",
            now_s,
            "agent is idle and ready for retire step",
            Some(false),
        )
    }
}

/// Retire a record that is idle_ready.
pub fn retire_record(record: &DrainRecord, now_s: f64) -> DrainRecord {
    if record.status == "retired" {
        return record.clone();
    }
    if record.status != "idle_ready" {
        return record.clone();
    }
    record.with_transition(
        "retired",
        "retired",
        now_s,
        "record retired; Phase 4 performs no runtime or tmux mutation",
        Some(false),
    )
}

/// Persistent store for the drain queue.
pub struct DrainQueueStore {
    layout: PathLayout,
    bounds: DrainBounds,
    store: JsonStore,
}

impl DrainQueueStore {
    pub fn new(layout: &PathLayout, bounds: Option<DrainBounds>) -> Self {
        Self {
            layout: layout.clone(),
            bounds: bounds.unwrap_or_default(),
            store: JsonStore::new(),
        }
    }

    pub fn load(&self) -> Result<DrainQueue, crate::DaemonError> {
        let path = self.layout.ccbd_reload_drain_path();
        if !path.exists() {
            return Ok(DrainQueue::empty(Some(self.bounds.clone())));
        }
        let value: serde_json::Value = self.store.load(&path)?;
        if let serde_json::Value::Object(obj) = value {
            Ok(DrainQueue::from_record(&obj))
        } else {
            Ok(DrainQueue::empty(Some(self.bounds.clone())))
        }
    }

    pub fn save(&self, queue: &DrainQueue) -> Result<(), crate::DaemonError> {
        let path = self.layout.ccbd_reload_drain_path();
        self.store.save(&path, &queue.to_record())?;
        Ok(())
    }
}

/// Build drain intent suggestions from reload operations.
pub fn drain_intent_suggestions_for_reload_operations(
    operations: &[HashMap<String, serde_json::Value>],
    old_config_signature: Option<&str>,
    new_config_signature: Option<&str>,
) -> Vec<HashMap<String, serde_json::Value>> {
    let old_signature = clean_text(old_config_signature);
    let new_signature = clean_text(new_config_signature);
    let mut suggestions = Vec::new();
    for operation in operations {
        let op = operation
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let (intent_kind, initial_phase) = match op {
            "remove_agent" => ("unload", "pending_unload"),
            "replace_agent" => ("replace", "pending_replace"),
            _ => continue,
        };
        let agent_name = clean_text(operation.get("agent").and_then(|v| v.as_str()));
        if agent_name.is_none() {
            continue;
        }
        let agent_name = agent_name.unwrap();
        let reason = clean_text(operation.get("reason").and_then(|v| v.as_str()));
        let intent_id = stable_intent_id(
            intent_kind,
            &agent_name,
            old_signature.as_deref(),
            new_signature.as_deref(),
            reason.as_deref(),
        );
        let mut suggestion = HashMap::new();
        suggestion.insert("intent_id".to_string(), serde_json::json!(intent_id));
        suggestion.insert("intent_kind".to_string(), serde_json::json!(intent_kind));
        suggestion.insert("agent".to_string(), serde_json::json!(agent_name));
        suggestion.insert(
            "initial_phase".to_string(),
            serde_json::json!(initial_phase),
        );
        suggestion.insert("dry_run_only".to_string(), serde_json::json!(true));
        suggestion.insert("reason".to_string(), serde_json::json!(reason));
        suggestions.push(suggestion);
    }
    suggestions
}

fn stable_intent_id(
    intent_kind: &str,
    agent_name: &str,
    old_config_signature: Option<&str>,
    new_config_signature: Option<&str>,
    reason: Option<&str>,
) -> String {
    let material = [
        intent_kind,
        agent_name,
        old_config_signature.unwrap_or(""),
        new_config_signature.unwrap_or(""),
        reason.unwrap_or(""),
    ]
    .join("\0");
    let hash = Sha256::digest(material.as_bytes());
    format!("drain_{}", hex::encode(&hash[..8]))
}

fn clean_text(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or("").trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_intent() -> DrainIntent {
        DrainIntent::new(
            "intent-1",
            "unload",
            "claude",
            1000.0,
            Some("removing".to_string()),
            Some("old-sig".to_string()),
            Some("new-sig".to_string()),
        )
    }

    fn sample_bounds() -> DrainBounds {
        DrainBounds::new(2, 10.0, 60.0)
    }

    #[test]
    fn test_drain_bounds_defaults() {
        let bounds = DrainBounds::default();
        assert_eq!(bounds.max_pending, 16);
        assert_eq!(bounds.timeout_s, 300.0);
        assert_eq!(bounds.max_age_s, 900.0);
    }

    #[test]
    #[should_panic(expected = "max_pending must be positive")]
    fn test_drain_bounds_zero_max_pending_panics() {
        DrainBounds::new(0, 1.0, 1.0);
    }

    #[test]
    fn test_drain_intent_initial_phase() {
        let intent = sample_intent();
        assert_eq!(intent.initial_phase(), "pending_unload");
    }

    #[test]
    fn test_drain_record_terminal() {
        let record = DrainRecord::pending(&sample_intent(), &sample_bounds(), 1000.0);
        assert!(!record.terminal());
        let timed_out = record.with_transition("draining", "timed_out", 1001.0, "timeout", None);
        assert!(timed_out.terminal());
    }

    #[test]
    fn test_enqueue_accepted() {
        let queue = DrainQueue::empty(Some(sample_bounds()));
        let intent = sample_intent();
        let result = queue.enqueue(&intent, 1000.0);
        assert!(result.accepted);
        assert_eq!(result.record.status, "pending");
        assert_eq!(result.queue.pending_count(), 1);
    }

    #[test]
    fn test_enqueue_rejected_when_full() {
        let queue = DrainQueue::empty(Some(DrainBounds::new(1, 10.0, 60.0)));
        let intent1 = DrainIntent::new("i1", "unload", "a", 1000.0, None, None, None);
        let intent2 = DrainIntent::new("i2", "unload", "b", 1000.0, None, None, None);
        let first = queue.enqueue(&intent1, 1000.0);
        assert!(first.accepted);
        let second = first.queue.enqueue(&intent2, 1000.0);
        assert!(!second.accepted);
        assert_eq!(second.record.status, "rejected_queue_full");
    }

    #[test]
    fn test_enqueue_expired() {
        let queue = DrainQueue::empty(Some(sample_bounds()));
        let intent = DrainIntent::new("i1", "unload", "a", 900.0, None, None, None);
        let result = queue.enqueue(&intent, 1000.0);
        assert!(!result.accepted);
        assert_eq!(result.record.status, "timed_out");
    }

    #[test]
    fn test_replace_record() {
        let queue = DrainQueue::empty(Some(sample_bounds()));
        let intent = sample_intent();
        let enqueued = queue.enqueue(&intent, 1000.0);
        let record =
            enqueued
                .record
                .with_transition("draining", "waiting", 1001.0, "busy", Some(true));
        let replaced = enqueued.queue.replace_record(&record);
        assert_eq!(replaced.records[0].status, "waiting");
        assert_eq!(replaced.records[0].transition_count, 1);
    }

    #[test]
    fn test_blocks_new_work_for() {
        let queue = DrainQueue::empty(Some(sample_bounds()));
        let intent = sample_intent();
        let enqueued = queue.enqueue(&intent, 1000.0);
        assert!(enqueued.queue.blocks_new_work_for("claude"));
        assert!(!enqueued.queue.blocks_new_work_for("codex"));
    }

    #[test]
    fn test_plan_drain_transition_idle_ready_unchanged() {
        let record = DrainRecord::pending(&sample_intent(), &sample_bounds(), 1000.0)
            .with_transition("retiring", "idle_ready", 1001.0, "ready", Some(false));
        let next = plan_drain_transition(&record, 1002.0, &|_| true);
        assert_eq!(next.status, "idle_ready");
        assert_eq!(next.transition_count, record.transition_count);
    }

    #[test]
    fn test_plan_drain_transition_busy() {
        let record = DrainRecord::pending(&sample_intent(), &sample_bounds(), 1000.0);
        let next = plan_drain_transition(&record, 1001.0, &|_| true);
        assert_eq!(next.status, "waiting");
        assert_eq!(next.busy, Some(true));
    }

    #[test]
    fn test_plan_drain_transition_idle() {
        let record = DrainRecord::pending(&sample_intent(), &sample_bounds(), 1000.0);
        let next = plan_drain_transition(&record, 1001.0, &|_| false);
        assert_eq!(next.status, "idle_ready");
        assert_eq!(next.busy, Some(false));
    }

    #[test]
    fn test_plan_drain_transition_timeout() {
        let record =
            DrainRecord::pending(&sample_intent(), &DrainBounds::new(1, 5.0, 60.0), 1000.0);
        let next = plan_drain_transition(&record, 1006.0, &|_| false);
        assert_eq!(next.status, "timed_out");
    }

    #[test]
    fn test_retire_record() {
        let record = DrainRecord::pending(&sample_intent(), &sample_bounds(), 1000.0)
            .with_transition("retiring", "idle_ready", 1001.0, "ready", Some(false));
        let retired = retire_record(&record, 1002.0);
        assert_eq!(retired.status, "retired");
        assert_eq!(retired.phase, "retired");
    }

    #[test]
    fn test_drain_intent_suggestions_for_reload_operations() {
        let operations = vec![
            {
                let mut op = HashMap::new();
                op.insert("op".to_string(), serde_json::json!("remove_agent"));
                op.insert("agent".to_string(), serde_json::json!("claude"));
                op.insert("reason".to_string(), serde_json::json!("deprecated"));
                op
            },
            {
                let mut op = HashMap::new();
                op.insert("op".to_string(), serde_json::json!("add_agent"));
                op.insert("agent".to_string(), serde_json::json!("new"));
                op
            },
        ];
        let suggestions =
            drain_intent_suggestions_for_reload_operations(&operations, Some("old"), Some("new"));
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0]["intent_kind"], "unload");
        assert_eq!(suggestions[0]["agent"], "claude");
        assert_eq!(suggestions[0]["dry_run_only"], true);
    }

    #[test]
    fn test_queue_roundtrip_through_record() {
        let queue = DrainQueue::empty(Some(sample_bounds()));
        let intent = sample_intent();
        let enqueued = queue.enqueue(&intent, 1000.0);
        let record = enqueued.queue.to_record();
        let obj = record.as_object().unwrap();
        let loaded = DrainQueue::from_record(obj);
        assert_eq!(loaded.records.len(), 1);
        assert_eq!(loaded.records[0].intent.agent_name, "claude");
    }
}
