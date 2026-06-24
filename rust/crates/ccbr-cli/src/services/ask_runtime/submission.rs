//! Mirrors Python `lib/cli/services/ask_runtime/submission.py`.

use ccbr_agents::models::normalize_agent_name;
use ccbr_agents::rolepacks::{looks_like_role_id, normalize_role_id};
use ccbr_daemon::models::api_models::common::DeliveryScope;
use ccbr_daemon::models::api_models::messages::MessageEnvelope;
use ccbr_mailbox::targets::{non_agent_actors, normalize_actor_name};
use ccbr_storage::paths::PathLayout;
use ccbr_storage::text_artifacts::{
    artifact_stub, maybe_spill_text, write_text_artifact, TextArtifact,
};

use crate::context::CliContext;
use crate::models_mailbox::ParsedAskCommand;
use crate::services::ask_runtime::models::AskSummary;

const DEFAULT_REPLY_GUIDANCE: &str = "CCB reply guidance:
- Answer directly and concisely.
- Include only relevant conclusions, blockers, risks, evidence, and next actions.
- Avoid raw logs and background unless explicitly requested.";

const COMPACT_REPLY_GUIDANCE: &str = "CCB reply guidance:
- Distill aggressively and lead with the answer.
- Keep only details needed for this ask.
- Omit empty sections, raw logs, repeated context, and background unless essential.";

const SILENT_REPLY_GUIDANCE: &str = "CCB reply guidance:
- Silent-on-success requested.
- Reply with the shortest useful status.
- Include details only for failures, blockers, or required next actions.";

const GUIDANCE_MARKER: &str = "CCB reply guidance:";

const EXPLICIT_OUTPUT_HINTS: &[&str] = &[
    "output requirements",
    "reply format",
    "response format",
    "format:",
    "only reply",
    "reply only",
    "full report",
    "full output",
    "detailed report",
    "complete output",
    "include everything",
    "all details",
    "leave nothing out",
    "verbatim",
    "do not summarize",
    "do not abbreviate",
    "\u{5b8c}\u{6574}\u{8f93}\u{51fa}",
    "\u{4e0d}\u{8981}\u{603b}\u{7ed3}",
    "\u{4e0d}\u{8981}\u{538b}\u{7f29}",
    "\u{4e0d}\u{8981}\u{7cbe}\u{7b80}",
    "\u{4e0d}\u{8981}\u{7701}\u{7565}",
    "\u{9010}\u{5b57}\u{8fd4}\u{56de}",
    "\u{9010}\u{5b57}",
    "\u{539f}\u{6837}\u{8fd4}\u{56de}",
    "\u{4fdd}\u{7559}\u{539f}\u{6587}",
    "\u{5b8c}\u{6574}\u{65e5}\u{5fd7}",
    "\u{5b8c}\u{6574}\u{62a5}\u{544a}",
    "\u{8be6}\u{7ec6}\u{62a5}\u{544a}",
    "\u{5168}\u{6587}",
];

/// Submit an ask message through the mounted daemon with fully injected dependencies.
///
/// Mirrors Python `submit_ask`.
#[allow(clippy::too_many_arguments)]
pub fn submit_ask_with<C, L, S>(
    context: &CliContext,
    command: &ParsedAskCommand,
    load_project_config_fn: L,
    resolve_ask_sender_fn: S,
    invoke_mounted_daemon_fn: C,
) -> anyhow::Result<AskSummary>
where
    C: FnOnce(
        &CliContext,
        bool,
        &dyn Fn(&dyn SubmitClient) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<serde_json::Value>,
    L: FnOnce(&PathLayout) -> ccbr_agents::Result<ccbr_agents::config::ConfigLoadResult>,
    S: FnOnce(&CliContext, Option<&str>) -> String,
{
    let config = load_project_config_fn(&context.paths)?.config;
    let normalized_target = _resolve_target(&command.target, &config.agents)?;
    _validate_target(&normalized_target, &config.agents)?;
    let sender = resolve_ask_sender_fn(context, command.sender.as_deref());
    let normalized_sender = _normalize_sender(&sender)?;
    _validate_sender(&normalized_sender, &config.agents)?;

    let message_body = message_with_reply_guidance(
        &command.message,
        command.mode.as_deref().unwrap_or("ask"),
        command.compact,
        command.silence,
    );
    let (message_body, body_artifact) = _artifact_request_body(
        &context.paths,
        &message_body,
        &format!("{normalized_sender}-to-{normalized_target}"),
        command.artifact_request,
    )?;

    let payload = invoke_mounted_daemon_fn(context, true, &|client: &dyn SubmitClient| {
        let mut envelope = MessageEnvelope {
            project_id: context.project.project_id.clone(),
            to_agent: normalized_target.clone(),
            from_actor: normalized_sender.clone(),
            body: message_body.clone(),
            task_id: command.task_id.clone(),
            reply_to: command.reply_to.clone(),
            message_type: command.mode.clone().unwrap_or_else(|| "ask".into()),
            delivery_scope: _delivery_scope(&normalized_target),
            silence_on_success: command.silence,
            route_options: serde_json::json!(_route_options(command)),
            body_artifact: body_artifact.as_ref().map(|a| a.to_record().into()),
        };
        envelope.normalize().map_err(|e| anyhow::anyhow!(e))?;
        client.submit(&envelope)
    })?;
    Ok(_summary_from_payload(&context.project.project_id, payload))
}

/// Client trait used to submit a `MessageEnvelope`.
pub trait SubmitClient {
    fn submit(&self, envelope: &MessageEnvelope) -> anyhow::Result<serde_json::Value>;
}

fn _route_options(command: &ParsedAskCommand) -> serde_json::Map<String, serde_json::Value> {
    let mut options = serde_json::Map::new();
    if command.callback {
        options.insert("mode".into(), "callback".into());
    }
    if command.artifact_request {
        options.insert("artifact_request".into(), true.into());
    }
    if command.artifact_reply {
        options.insert("artifact_reply".into(), true.into());
    }
    options
}

fn _artifact_request_body(
    layout: &PathLayout,
    message_body: &str,
    owner_id: &str,
    force: bool,
) -> anyhow::Result<(String, Option<TextArtifact>)> {
    if force {
        let artifact =
            write_text_artifact(layout, message_body, "ask-request", owner_id, None, None)?;
        let stub = artifact_stub(
            "CCB ask request was stored as an artifact by --artifact-request.",
            &artifact,
            false,
        );
        return Ok((stub, Some(artifact)));
    }
    let (body, artifact) = maybe_spill_text(
        layout,
        message_body,
        "ask-request",
        owner_id,
        "CCB ask request is larger than 4 KiB and was stored as an artifact.",
        None,
        None,
        None,
    )?;
    Ok((body, artifact))
}

/// Append CCB reply guidance to an `ask` message body unless suppressed.
///
/// Mirrors Python `message_with_reply_guidance`.
pub fn message_with_reply_guidance(
    message: &str,
    message_type: &str,
    compact: bool,
    silence_on_success: bool,
) -> String {
    if message_type.trim().to_lowercase() != "ask" {
        return message.to_string();
    }
    if _has_explicit_output_guidance(message) {
        return message.to_string();
    }
    let guidance = if silence_on_success {
        SILENT_REPLY_GUIDANCE
    } else if compact {
        COMPACT_REPLY_GUIDANCE
    } else {
        DEFAULT_REPLY_GUIDANCE
    };
    format!("{}\n\n{}", message.trim_end(), guidance)
}

fn _has_explicit_output_guidance(message: &str) -> bool {
    let lowered = message.to_lowercase();
    if lowered.contains(&GUIDANCE_MARKER.to_lowercase()) {
        return true;
    }
    EXPLICIT_OUTPUT_HINTS
        .iter()
        .any(|hint| lowered.contains(hint))
}

fn _normalize_sender(value: &str) -> anyhow::Result<String> {
    normalize_actor_name(Some(value)).map_err(|e| anyhow::anyhow!(e))
}

fn _normalize_target(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    if normalized == "all" {
        return normalized;
    }
    if looks_like_role_id(&normalized) {
        return normalize_role_id(&normalized).unwrap_or(normalized);
    }
    normalize_agent_name(&normalized).unwrap_or_else(|_| normalized.clone())
}

fn _validate_target(
    target: &str,
    agents: &std::collections::HashMap<String, ccbr_agents::models::AgentSpec>,
) -> anyhow::Result<()> {
    if target != "all" && !agents.contains_key(target) {
        return Err(anyhow::anyhow!("unknown agent: {target}"));
    }
    Ok(())
}

fn _resolve_target(
    value: &str,
    agents: &std::collections::HashMap<String, ccbr_agents::models::AgentSpec>,
) -> anyhow::Result<String> {
    let normalized = _normalize_target(value);
    if normalized == "all" || agents.contains_key(&normalized) {
        return Ok(normalized);
    }
    if looks_like_role_id(&normalized) {
        let role_id = normalize_role_id(&normalized)?;
        let mut matches: Vec<String> = agents
            .iter()
            .filter(|(_, spec)| {
                spec.role
                    .as_ref()
                    .map(|r| r.trim().to_lowercase() == role_id)
                    .unwrap_or(false)
            })
            .map(|(name, _)| name.clone())
            .collect();
        matches.sort();
        match matches.len() {
            1 => return Ok(matches.into_iter().next().unwrap()),
            0 => {
                return Err(anyhow::anyhow!(
                    "role {role_id} is not bound to any configured agent; target the project-local agent name or add the role to config"
                ));
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "role {role_id} is bound to multiple agents: {}; target one agent name explicitly",
                    matches.join(", ")
                ));
            }
        }
    }
    Ok(normalized)
}

fn _validate_sender(
    sender: &str,
    agents: &std::collections::HashMap<String, ccbr_agents::models::AgentSpec>,
) -> anyhow::Result<()> {
    let non_agents = non_agent_actors();
    if non_agents.contains(&sender.to_string()) {
        if sender == "cmd" {
            return Err(anyhow::anyhow!("unknown sender agent: cmd"));
        }
        return Ok(());
    }
    if agents.contains_key(sender) {
        return Ok(());
    }
    Err(anyhow::anyhow!("unknown sender agent: {sender}"))
}

fn _delivery_scope(target: &str) -> DeliveryScope {
    if target.trim().to_lowercase() == "all" {
        DeliveryScope::Broadcast
    } else {
        DeliveryScope::Single
    }
}

fn _summary_from_payload(project_id: &str, payload: serde_json::Value) -> AskSummary {
    if payload.get("job_id").is_some() {
        let job = payload.clone();
        AskSummary {
            project_id: project_id.into(),
            submission_id: None,
            jobs: vec![job],
        }
    } else {
        AskSummary {
            project_id: project_id.into(),
            submission_id: payload
                .get("submission_id")
                .and_then(|v| v.as_str())
                .map(|s| s.into()),
            jobs: payload
                .get("jobs")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_with_reply_guidance_appends_default() {
        let body = message_with_reply_guidance("review the diff", "ask", false, false);
        assert!(body.starts_with("review the diff\n\nCCB reply guidance:"));
        assert!(body.contains("Answer directly and concisely."));
    }

    #[test]
    fn message_with_reply_guidance_appends_compact() {
        let body = message_with_reply_guidance("review the diff", "ask", true, false);
        assert!(body.contains("Distill aggressively and lead with the answer."));
    }

    #[test]
    fn message_with_reply_guidance_respects_explicit_output_requirements() {
        let body = message_with_reply_guidance(
            "review the diff\n\nOutput requirements:\n- Write a full report.",
            "ask",
            false,
            false,
        );
        assert_eq!(
            body,
            "review the diff\n\nOutput requirements:\n- Write a full report."
        );
    }

    #[test]
    fn message_with_reply_guidance_uses_silent_hint() {
        let body = message_with_reply_guidance("run smoke test", "ask", false, true);
        assert!(body.contains("Silent-on-success requested."));
    }

    #[test]
    fn message_with_reply_guidance_skips_non_ask_modes() {
        assert_eq!(
            message_with_reply_guidance("ship it", "notify", false, false),
            "ship it"
        );
    }
}
