use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::error::{CompletionError, Result};
use crate::utils::first_non_empty;

pub const SCHEMA_VERSION: u32 = 2;

/// Priority map for reply candidates, matching Python `REPLY_PRIORITY`.
pub const REPLY_PRIORITY: [(ReplyCandidateKind, u32); 6] = [
    (ReplyCandidateKind::LastAgentMessage, 2),
    (ReplyCandidateKind::FinalAnswer, 3),
    (ReplyCandidateKind::AssistantFinal, 4),
    (ReplyCandidateKind::AssistantChunkMerged, 5),
    (ReplyCandidateKind::SessionReply, 6),
    (ReplyCandidateKind::FallbackText, 7),
];

fn validate_schema(record: &Map<String, serde_json::Value>, expected_type: &str) -> Result<()> {
    if record
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        != Some(SCHEMA_VERSION)
    {
        return Err(CompletionError::Validation(format!(
            "schema_version must be {SCHEMA_VERSION}"
        )));
    }
    if record.get("record_type").and_then(|v| v.as_str()) != Some(expected_type) {
        return Err(CompletionError::Validation(format!(
            "record_type must be {expected_type:?}"
        )));
    }
    Ok(())
}

fn normalize_provider(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CompletionError::Validation(
            "provider cannot be empty".into(),
        ));
    }
    Ok(trimmed.to_lowercase())
}

fn normalize_agent_name(value: &str) -> Result<String> {
    ccb_storage::path_helpers::normalize_agent_name(value)
        .map_err(|e| CompletionError::Validation(format!("invalid agent name: {e}")))
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStatus {
    Completed,
    Cancelled,
    Failed,
    Incomplete,
}

impl CompletionStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            CompletionStatus::Completed
                | CompletionStatus::Cancelled
                | CompletionStatus::Failed
                | CompletionStatus::Incomplete
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionConfidence {
    Exact,
    Observed,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionFamily {
    ProtocolTurn,
    StructuredResult,
    SessionBoundary,
    AnchoredSessionStability,
    TerminalTextQuiet,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionSourceKind {
    #[default]
    ProtocolEventStream,
    StructuredResultStream,
    SessionEventLog,
    SessionSnapshot,
    TerminalText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorFamily {
    FinalMessage,
    StructuredResult,
    SessionReply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionItemKind {
    AnchorSeen,
    AssistantChunk,
    AssistantFinal,
    ToolCall,
    ToolResult,
    Result,
    TurnBoundary,
    TurnAborted,
    CancelInfo,
    Error,
    PaneDead,
    SessionSnapshot,
    SessionMutation,
    SessionRotate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyCandidateKind {
    LastAgentMessage,
    FinalAnswer,
    AssistantFinal,
    AssistantChunkMerged,
    SessionReply,
    FallbackText,
}

impl ReplyCandidateKind {
    pub fn default_priority(&self) -> u32 {
        match self {
            ReplyCandidateKind::LastAgentMessage => 2,
            ReplyCandidateKind::FinalAnswer => 3,
            ReplyCandidateKind::AssistantFinal => 4,
            ReplyCandidateKind::AssistantChunkMerged => 5,
            ReplyCandidateKind::SessionReply => 6,
            ReplyCandidateKind::FallbackText => 7,
        }
    }
}

// ---------------------------------------------------------------------------
// CompletionCursor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionCursor {
    pub source_kind: CompletionSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opaque_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_no: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl CompletionCursor {
    pub fn new(source_kind: CompletionSourceKind, updated_at: impl Into<String>) -> Self {
        Self {
            source_kind,
            updated_at: Some(updated_at.into()),
            ..Default::default()
        }
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "completion_cursor",
            "source_kind": self.source_kind,
            "opaque_cursor": self.opaque_cursor,
            "session_path": self.session_path,
            "offset": self.offset,
            "line_no": self.line_no,
            "event_seq": self.event_seq,
            "updated_at": self.updated_at,
        })
    }

    pub fn from_record(record: &Map<String, serde_json::Value>) -> Result<Self> {
        validate_schema(record, "completion_cursor")?;
        Ok(Self {
            source_kind: serde_json::from_value(record["source_kind"].clone())?,
            opaque_cursor: record
                .get("opaque_cursor")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            session_path: record
                .get("session_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            offset: record.get("offset").and_then(|v| v.as_u64()),
            line_no: record.get("line_no").and_then(|v| v.as_u64()),
            event_seq: record.get("event_seq").and_then(|v| v.as_u64()),
            updated_at: record
                .get("updated_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    }
}

// ---------------------------------------------------------------------------
// CompletionItem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItem {
    pub kind: CompletionItemKind,
    pub timestamp: String,
    pub cursor: CompletionCursor,
    pub provider: String,
    pub agent_name: String,
    pub req_id: String,
    #[serde(default)]
    pub payload: Map<String, serde_json::Value>,
}

impl CompletionItem {
    pub fn new(
        kind: CompletionItemKind,
        timestamp: impl Into<String>,
        cursor: CompletionCursor,
        provider: impl Into<String>,
        agent_name: impl Into<String>,
        req_id: impl Into<String>,
    ) -> Result<Self> {
        let timestamp = timestamp.into();
        if timestamp.is_empty() {
            return Err(CompletionError::Validation(
                "timestamp cannot be empty".into(),
            ));
        }
        let provider = normalize_provider(&provider.into())?;
        let agent_name = normalize_agent_name(&agent_name.into())?;
        let req_id = req_id.into();
        if req_id.trim().is_empty() {
            return Err(CompletionError::Validation("req_id cannot be empty".into()));
        }
        Ok(Self {
            kind,
            timestamp,
            cursor,
            provider,
            agent_name,
            req_id,
            payload: Map::new(),
        })
    }

    pub fn with_payload(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.payload.insert(key.into(), value.into());
        self
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "completion_item",
            "kind": self.kind,
            "timestamp": self.timestamp,
            "cursor": self.cursor.to_record(),
            "provider": self.provider,
            "agent_name": self.agent_name,
            "req_id": self.req_id,
            "payload": self.payload,
        })
    }
}

// ---------------------------------------------------------------------------
// ReplyCandidate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyCandidate {
    pub kind: ReplyCandidateKind,
    pub text: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_turn_ref: Option<String>,
    pub priority: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CompletionCursor>,
}

impl ReplyCandidate {
    pub fn new(
        kind: ReplyCandidateKind,
        text: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Result<Self> {
        let text = text.into().trim().to_string();
        if text.is_empty() {
            return Err(CompletionError::Validation(
                "reply candidate text cannot be empty".into(),
            ));
        }
        let timestamp = timestamp.into();
        if timestamp.is_empty() {
            return Err(CompletionError::Validation(
                "reply candidate timestamp cannot be empty".into(),
            ));
        }
        Ok(Self {
            kind,
            text,
            timestamp,
            provider_turn_ref: None,
            priority: kind.default_priority(),
            cursor: None,
        })
    }

    pub fn with_provider_turn_ref(mut self, value: impl Into<String>) -> Self {
        self.provider_turn_ref = Some(value.into());
        self
    }

    pub fn with_cursor(mut self, cursor: CompletionCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "reply_candidate",
            "kind": self.kind,
            "text": self.text,
            "timestamp": self.timestamp,
            "provider_turn_ref": self.provider_turn_ref,
            "priority": self.priority,
            "cursor": self.cursor.as_ref().map(|c| c.to_record()),
        })
    }
}

// ---------------------------------------------------------------------------
// CompletionProfile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionProfile {
    pub provider: String,
    pub runtime_mode: ccb_agents::models::RuntimeMode,
    pub completion_family: CompletionFamily,
    pub completion_source_kind: CompletionSourceKind,
    pub supports_exact_completion: bool,
    pub supports_observed_completion: bool,
    pub supports_anchor_binding: bool,
    pub supports_reply_stability: bool,
    pub supports_terminal_reason: bool,
    pub selector_family: SelectorFamily,
}

impl CompletionProfile {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "completion_profile",
            "provider": self.provider,
            "runtime_mode": crate::utils::runtime_mode_to_string(&self.runtime_mode),
            "completion_family": self.completion_family,
            "completion_source_kind": self.completion_source_kind,
            "supports_exact_completion": self.supports_exact_completion,
            "supports_observed_completion": self.supports_observed_completion,
            "supports_anchor_binding": self.supports_anchor_binding,
            "supports_reply_stability": self.supports_reply_stability,
            "supports_terminal_reason": self.supports_terminal_reason,
            "selector_family": self.selector_family,
        })
    }
}

// ---------------------------------------------------------------------------
// CompletionRequestContext
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionRequestContext {
    pub req_id: String,
    pub agent_name: String,
    pub provider: String,
    pub timeout_s: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_text: Option<String>,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_s: f64,
}

fn default_poll_interval() -> f64 {
    0.5
}

impl CompletionRequestContext {
    pub fn new(
        req_id: impl Into<String>,
        agent_name: impl Into<String>,
        provider: impl Into<String>,
        timeout_s: f64,
    ) -> Result<Self> {
        let req_id = req_id.into();
        if req_id.trim().is_empty() {
            return Err(CompletionError::Validation("req_id cannot be empty".into()));
        }
        let agent_name = normalize_agent_name(&agent_name.into())?;
        let provider = normalize_provider(&provider.into())?;
        if timeout_s <= 0.0 {
            return Err(CompletionError::Validation(
                "timeout_s must be positive".into(),
            ));
        }
        Ok(Self {
            req_id,
            agent_name,
            provider,
            timeout_s,
            anchor_text: None,
            poll_interval_s: 0.5,
        })
    }

    pub fn with_anchor_text(mut self, value: impl Into<String>) -> Self {
        self.anchor_text = Some(value.into());
        self
    }

    pub fn with_poll_interval_s(mut self, value: f64) -> Self {
        self.poll_interval_s = value;
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.req_id.trim().is_empty() {
            return Err(CompletionError::Validation("req_id cannot be empty".into()));
        }
        if self.provider.trim().is_empty() {
            return Err(CompletionError::Validation(
                "provider cannot be empty".into(),
            ));
        }
        if self.timeout_s <= 0.0 {
            return Err(CompletionError::Validation(
                "timeout_s must be positive".into(),
            ));
        }
        if self.poll_interval_s <= 0.0 {
            return Err(CompletionError::Validation(
                "poll_interval_s must be positive".into(),
            ));
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "completion_request_context",
            "req_id": self.req_id,
            "agent_name": self.agent_name,
            "provider": self.provider,
            "timeout_s": self.timeout_s,
            "anchor_text": self.anchor_text,
            "poll_interval_s": self.poll_interval_s,
        })
    }
}

// ---------------------------------------------------------------------------
// CompletionState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompletionState {
    #[serde(default)]
    pub anchor_seen: bool,
    #[serde(default)]
    pub reply_started: bool,
    #[serde(default)]
    pub reply_stable: bool,
    #[serde(default)]
    pub tool_active: bool,
    #[serde(default)]
    pub subagent_activity_seen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reply_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reply_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_since: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_turn_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_cursor: Option<CompletionCursor>,
    #[serde(default)]
    pub terminal: bool,
}

impl CompletionState {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "completion_state",
            "anchor_seen": self.anchor_seen,
            "reply_started": self.reply_started,
            "reply_stable": self.reply_stable,
            "tool_active": self.tool_active,
            "subagent_activity_seen": self.subagent_activity_seen,
            "last_reply_hash": self.last_reply_hash,
            "last_reply_at": self.last_reply_at,
            "stable_since": self.stable_since,
            "provider_turn_ref": self.provider_turn_ref,
            "latest_cursor": self.latest_cursor.as_ref().map(|c| c.to_record()),
            "terminal": self.terminal,
        })
    }

    pub fn from_record(record: &Map<String, serde_json::Value>) -> Result<Self> {
        validate_schema(record, "completion_state")?;
        Ok(Self {
            anchor_seen: record
                .get("anchor_seen")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            reply_started: record
                .get("reply_started")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            reply_stable: record
                .get("reply_stable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            tool_active: record
                .get("tool_active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            subagent_activity_seen: record
                .get("subagent_activity_seen")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            last_reply_hash: record
                .get("last_reply_hash")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            last_reply_at: record
                .get("last_reply_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            stable_since: record
                .get("stable_since")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            provider_turn_ref: record
                .get("provider_turn_ref")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            latest_cursor: record
                .get("latest_cursor")
                .and_then(|v| v.as_object())
                .map(CompletionCursor::from_record)
                .transpose()?,
            terminal: record
                .get("terminal")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
    }
}

// ---------------------------------------------------------------------------
// CompletionDecision
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionDecision {
    pub terminal: bool,
    pub status: CompletionStatus,
    pub reason: Option<String>,
    pub confidence: Option<CompletionConfidence>,
    #[serde(default)]
    pub reply: String,
    #[serde(default)]
    pub anchor_seen: bool,
    #[serde(default)]
    pub reply_started: bool,
    #[serde(default)]
    pub reply_stable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_turn_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_cursor: Option<CompletionCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub diagnostics: Map<String, serde_json::Value>,
}

impl CompletionDecision {
    pub fn pending(cursor: Option<CompletionCursor>) -> Self {
        Self {
            terminal: false,
            status: CompletionStatus::Incomplete,
            reason: None,
            confidence: None,
            reply: String::new(),
            anchor_seen: false,
            reply_started: false,
            reply_stable: false,
            provider_turn_ref: None,
            source_cursor: cursor,
            finished_at: None,
            diagnostics: Map::new(),
        }
    }

    pub fn with_reply(&self, reply: impl Into<String>) -> Self {
        let reply = reply.into();
        let mut next = self.clone();
        if !reply.is_empty() {
            next.reply = reply;
        }
        next
    }

    pub fn validate(&self) -> Result<()> {
        if self.terminal {
            if self.reason.is_none() {
                return Err(CompletionError::Validation(
                    "terminal decisions require reason".into(),
                ));
            }
            if self.confidence.is_none() {
                return Err(CompletionError::Validation(
                    "terminal decisions require confidence".into(),
                ));
            }
            if self.finished_at.is_none() {
                return Err(CompletionError::Validation(
                    "terminal decisions require finished_at".into(),
                ));
            }
        } else {
            if self.status != CompletionStatus::Incomplete {
                return Err(CompletionError::Validation(
                    "non-terminal decisions must use status=incomplete".into(),
                ));
            }
            if self.reason.is_some() || self.confidence.is_some() || self.finished_at.is_some() {
                return Err(CompletionError::Validation(
                    "non-terminal decisions cannot set reason/confidence/finished_at".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "completion_decision",
            "terminal": self.terminal,
            "status": self.status,
            "reason": self.reason,
            "confidence": self.confidence,
            "reply": self.reply,
            "anchor_seen": self.anchor_seen,
            "reply_started": self.reply_started,
            "reply_stable": self.reply_stable,
            "provider_turn_ref": self.provider_turn_ref,
            "source_cursor": self.source_cursor.as_ref().map(|c| c.to_record()),
            "finished_at": self.finished_at,
            "diagnostics": self.diagnostics,
        })
    }

    pub fn from_record(record: &Map<String, serde_json::Value>) -> Result<Self> {
        validate_schema(record, "completion_decision")?;
        let decision = Self {
            terminal: record
                .get("terminal")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            status: serde_json::from_value(record["status"].clone())?,
            reason: record
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            confidence: record
                .get("confidence")
                .and_then(|v| v.as_str())
                .map(|s| serde_json::from_value(serde_json::Value::String(s.to_string())))
                .transpose()?,
            reply: record
                .get("reply")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            anchor_seen: record
                .get("anchor_seen")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            reply_started: record
                .get("reply_started")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            reply_stable: record
                .get("reply_stable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            provider_turn_ref: record
                .get("provider_turn_ref")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            source_cursor: record
                .get("source_cursor")
                .and_then(|v| v.as_object())
                .map(CompletionCursor::from_record)
                .transpose()?,
            finished_at: record
                .get("finished_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            diagnostics: record
                .get("diagnostics")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default(),
        };
        decision.validate()?;
        Ok(decision)
    }
}

// ---------------------------------------------------------------------------
// CompletionSnapshot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionSnapshot {
    pub job_id: String,
    pub agent_name: String,
    pub profile_family: CompletionFamily,
    pub state: CompletionState,
    pub latest_decision: CompletionDecision,
    #[serde(default)]
    pub latest_reply_preview: String,
    pub updated_at: String,
}

impl CompletionSnapshot {
    pub fn new(
        job_id: impl Into<String>,
        agent_name: impl Into<String>,
        profile_family: CompletionFamily,
        state: CompletionState,
        latest_decision: CompletionDecision,
        updated_at: impl Into<String>,
    ) -> Result<Self> {
        let job_id = job_id.into();
        if job_id.trim().is_empty() {
            return Err(CompletionError::Validation("job_id cannot be empty".into()));
        }
        let agent_name = normalize_agent_name(&agent_name.into())?;
        let updated_at = updated_at.into();
        if updated_at.is_empty() {
            return Err(CompletionError::Validation(
                "updated_at cannot be empty".into(),
            ));
        }
        latest_decision.validate()?;
        Ok(Self {
            job_id,
            agent_name,
            profile_family,
            state,
            latest_decision,
            latest_reply_preview: String::new(),
            updated_at,
        })
    }

    pub fn with_latest_reply_preview(mut self, value: impl Into<String>) -> Self {
        self.latest_reply_preview = value.into();
        self
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "completion_snapshot",
            "job_id": self.job_id,
            "agent_name": self.agent_name,
            "profile_family": self.profile_family,
            "state": self.state.to_record(),
            "latest_decision": self.latest_decision.to_record(),
            "latest_reply_preview": self.latest_reply_preview,
            "updated_at": self.updated_at,
        })
    }

    pub fn from_record(record: &Map<String, serde_json::Value>) -> Result<Self> {
        validate_schema(record, "completion_snapshot")?;
        let snapshot = Self {
            job_id: record["job_id"]
                .as_str()
                .ok_or_else(|| CompletionError::Validation("job_id must be a string".into()))?
                .to_string(),
            agent_name: normalize_agent_name(record["agent_name"].as_str().ok_or_else(|| {
                CompletionError::Validation("agent_name must be a string".into())
            })?)?,
            profile_family: serde_json::from_value(record["profile_family"].clone())?,
            state: CompletionState::from_record(
                record["state"]
                    .as_object()
                    .ok_or_else(|| CompletionError::Validation("state must be an object".into()))?,
            )?,
            latest_decision: CompletionDecision::from_record(
                record["latest_decision"].as_object().ok_or_else(|| {
                    CompletionError::Validation("latest_decision must be an object".into())
                })?,
            )?,
            latest_reply_preview: record
                .get("latest_reply_preview")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            updated_at: record["updated_at"]
                .as_str()
                .ok_or_else(|| CompletionError::Validation("updated_at must be a string".into()))?
                .to_string(),
        };
        Ok(snapshot)
    }
}

// ---------------------------------------------------------------------------
// Reply candidate extraction
// ---------------------------------------------------------------------------

pub fn reply_candidates_from_item(item: &CompletionItem) -> Vec<ReplyCandidate> {
    let mut candidates = Vec::new();
    let payload = &item.payload;
    let provider_turn_ref = first_non_empty(
        payload,
        &["turn_id", "provider_turn_ref", "message_id", "session_id"],
    );

    if let Some(text) = first_non_empty(payload, &["last_agent_message"]) {
        candidates.push(
            ReplyCandidate::new(ReplyCandidateKind::LastAgentMessage, text, &item.timestamp)
                .expect("text is non-empty")
                .with_provider_turn_ref(provider_turn_ref.clone().unwrap_or_default())
                .with_cursor(item.cursor.clone()),
        );
    }

    if item.kind == CompletionItemKind::Result {
        if let Some(text) =
            first_non_empty(payload, &["reply", "result_text", "final_answer", "text"])
        {
            candidates.push(
                ReplyCandidate::new(ReplyCandidateKind::FinalAnswer, text, &item.timestamp)
                    .expect("text is non-empty")
                    .with_provider_turn_ref(provider_turn_ref.clone().unwrap_or_default())
                    .with_cursor(item.cursor.clone()),
            );
        }
    }

    if item.kind == CompletionItemKind::AssistantFinal {
        if let Some(text) = first_non_empty(payload, &["text", "reply"]) {
            candidates.push(
                ReplyCandidate::new(ReplyCandidateKind::AssistantFinal, text, &item.timestamp)
                    .expect("text is non-empty")
                    .with_provider_turn_ref(provider_turn_ref.clone().unwrap_or_default())
                    .with_cursor(item.cursor.clone()),
            );
        }
    }

    if item.kind == CompletionItemKind::AssistantChunk {
        if let Some(text) = first_non_empty(payload, &["merged_text", "text", "reply"]) {
            candidates.push(
                ReplyCandidate::new(
                    ReplyCandidateKind::AssistantChunkMerged,
                    text,
                    &item.timestamp,
                )
                .expect("text is non-empty")
                .with_provider_turn_ref(provider_turn_ref.clone().unwrap_or_default())
                .with_cursor(item.cursor.clone()),
            );
        }
    }

    if item.kind == CompletionItemKind::SessionSnapshot
        || item.kind == CompletionItemKind::SessionMutation
    {
        if let Some(text) = first_non_empty(payload, &["reply", "content", "text"]) {
            candidates.push(
                ReplyCandidate::new(ReplyCandidateKind::SessionReply, text, &item.timestamp)
                    .expect("text is non-empty")
                    .with_provider_turn_ref(provider_turn_ref.clone().unwrap_or_default())
                    .with_cursor(item.cursor.clone()),
            );
        }
    }

    if let Some(text) = first_non_empty(payload, &["fallback_text"]) {
        candidates.push(
            ReplyCandidate::new(ReplyCandidateKind::FallbackText, text, &item.timestamp)
                .expect("text is non-empty")
                .with_provider_turn_ref(provider_turn_ref.unwrap_or_default())
                .with_cursor(item.cursor.clone()),
        );
    }

    candidates
}

// ---------------------------------------------------------------------------
// TargetKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    #[default]
    Agent,
}

// ---------------------------------------------------------------------------
// JobRecord
// ---------------------------------------------------------------------------

/// Request payload attached to a job record.
/// Mirrors Python `job.request`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JobRequest {
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub agent_name: String,
    pub provider: String,
    #[serde(default)]
    pub target_kind: TargetKind,
    /// Request payload for provider adapters.
    /// Mirrors Python `job.request`.
    #[serde(default)]
    pub request: JobRequest,
    /// Provider-specific options for this job.
    /// Mirrors Python `job.provider_options`.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub provider_options: Map<String, serde_json::Value>,
    /// Workspace path for provider execution.
    /// Mirrors Python `job.workspace_path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    /// Provider instance name for session resolution.
    /// Mirrors Python `job.provider_instance`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_instance: Option<String>,
}

impl JobRecord {
    pub fn new(
        job_id: impl Into<String>,
        agent_name: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            agent_name: agent_name.into(),
            provider: provider.into(),
            target_kind: TargetKind::Agent,
            request: JobRequest::default(),
            provider_options: Map::new(),
            workspace_path: None,
            provider_instance: None,
        }
    }

    pub fn with_request_body(mut self, body: impl Into<String>) -> Self {
        self.request.body = body.into();
        self
    }

    pub fn with_request_message_type(mut self, message_type: impl Into<String>) -> Self {
        self.request.message_type = Some(message_type.into());
        self
    }

    pub fn with_workspace_path(mut self, path: impl Into<String>) -> Self {
        self.workspace_path = Some(path.into());
        self
    }

    pub fn with_provider_instance(mut self, instance: impl Into<String>) -> Self {
        self.provider_instance = Some(instance.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CompletionValidationError;

    #[test]
    fn reply_priority_matches_python() {
        for (kind, expected) in REPLY_PRIORITY {
            assert_eq!(kind.default_priority(), expected);
        }
        assert_eq!(REPLY_PRIORITY.len(), 6);
    }

    #[test]
    fn validation_error_alias_exists() {
        // Compile-only check: Python `CompletionValidationError` is reachable.
        let _: CompletionValidationError = CompletionError::Validation("test".to_string());
    }
}
