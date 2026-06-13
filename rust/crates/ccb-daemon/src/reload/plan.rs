use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadPlan {
    pub dry_run: bool,
    pub plan_class: String,
    pub added_agents: Vec<String>,
    pub removed_agents: Vec<String>,
    pub modified_agents: Vec<String>,
    pub unchanged_agents: Vec<String>,
    pub current_config_identity: Option<String>,
    pub new_config_identity: Option<String>,
    pub errors: Vec<String>,
}

impl ReloadPlan {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "dry_run": self.dry_run,
            "plan_class": self.plan_class,
            "added_agents": self.added_agents,
            "removed_agents": self.removed_agents,
            "modified_agents": self.modified_agents,
            "unchanged_agents": self.unchanged_agents,
            "current_config_identity": self.current_config_identity,
            "new_config_identity": self.new_config_identity,
            "errors": self.errors,
        })
    }

    pub fn is_noop(&self) -> bool {
        self.added_agents.is_empty()
            && self.removed_agents.is_empty()
            && self.modified_agents.is_empty()
            && self.errors.is_empty()
    }
}

pub fn build_reload_dry_run_plan(current_agents: &[String], new_agents: &[String]) -> ReloadPlan {
    let current_set: std::collections::HashSet<&String> = current_agents.iter().collect();
    let new_set: std::collections::HashSet<&String> = new_agents.iter().collect();

    let added: Vec<String> = new_agents
        .iter()
        .filter(|a| !current_set.contains(a))
        .cloned()
        .collect();
    let removed: Vec<String> = current_agents
        .iter()
        .filter(|a| !new_set.contains(a))
        .cloned()
        .collect();
    let unchanged: Vec<String> = current_agents
        .iter()
        .filter(|a| new_set.contains(a))
        .cloned()
        .collect();

    ReloadPlan {
        dry_run: true,
        plan_class: if added.is_empty() && removed.is_empty() {
            "noop"
        } else {
            "changes"
        }
        .into(),
        added_agents: added,
        removed_agents: removed,
        modified_agents: vec![],
        unchanged_agents: unchanged,
        current_config_identity: None,
        new_config_identity: None,
        errors: vec![],
    }
}
