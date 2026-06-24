//! Mirrors Python `lib/ccbrd/reload_transaction_context.py`.

use crate::reload_apply_models::ServiceGraph;
use serde_json::Value;

/// Context captured at the start of a reload publish transaction.
#[derive(Debug, Clone)]
pub struct TransactionContext {
    pub old_graph: ServiceGraph,
    pub new_graph: ServiceGraph,
    pub old_graph_version: Option<String>,
    pub new_graph_version: Option<String>,
    pub old_config_signature: String,
    pub new_config_signature: String,
    pub namespace_patch: Option<Value>,
    pub runtime_mount: Option<Value>,
}

/// Keyword-style arguments reused by every transaction result constructor.
#[derive(Debug, Clone)]
pub struct TransactionResultKwargs {
    pub old_graph_version: Option<String>,
    pub new_graph_version: Option<String>,
    pub old_config_signature: Option<String>,
    pub new_config_signature: Option<String>,
    pub namespace_patch: Option<Value>,
    pub runtime_mount: Option<Value>,
}

impl TransactionContext {
    /// Return the common result kwargs derived from this context.
    pub fn result_kwargs(&self) -> TransactionResultKwargs {
        TransactionResultKwargs {
            old_graph_version: self.old_graph_version.clone(),
            new_graph_version: self.new_graph_version.clone(),
            old_config_signature: Some(self.old_config_signature.clone()),
            new_config_signature: Some(self.new_config_signature.clone()),
            namespace_patch: self.namespace_patch.clone(),
            runtime_mount: self.runtime_mount.clone(),
        }
    }
}

/// Build a transaction context from the old/new graphs and stage results.
pub fn transaction_context(
    old_graph: &ServiceGraph,
    new_graph: &ServiceGraph,
    namespace_patch: Option<Value>,
    runtime_mount: Option<Value>,
) -> TransactionContext {
    TransactionContext {
        old_graph: old_graph.clone(),
        new_graph: new_graph.clone(),
        old_graph_version: old_graph.version.clone(),
        new_graph_version: new_graph.version.clone(),
        old_config_signature: old_graph.config_signature.clone(),
        new_config_signature: new_graph.config_signature.clone(),
        namespace_patch,
        runtime_mount,
    }
}
