//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/topology_plan.py`.

use ccb_agents::models::ProjectConfig;
use serde::{Deserialize, Serialize};

/// Topology plan describing how a config maps onto the project namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceTopologyPlan {
    pub config: ProjectConfig,
    pub ccbd_socket_path: String,
    pub project_root: String,
}

/// Build a namespace topology plan for a config.
pub fn build_namespace_topology_plan(
    config: &ProjectConfig,
    ccbd_socket_path: String,
    project_root: String,
) -> NamespaceTopologyPlan {
    NamespaceTopologyPlan {
        config: config.clone(),
        ccbd_socket_path,
        project_root,
    }
}
