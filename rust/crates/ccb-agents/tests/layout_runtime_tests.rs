//! Mirrors Python `test/test_agents_layout_runtime.py`.

use ccb_agents::layout::{
    build_balanced_layout, iter_layout_names, parse_layout_spec, prune_layout,
};
use ccb_agents::models_runtime::config_runtime::validation::resolve_layout_spec;
use std::collections::HashMap;

#[test]
fn test_parse_layout_spec_roundtrip_with_parentheses() {
    let layout = parse_layout_spec("cmd; (agent1:codex, agent2:claude)").unwrap();
    assert_eq!(layout.render(), "cmd; agent1:codex, agent2:claude");
    assert_eq!(iter_layout_names(&layout), vec!["cmd", "agent1", "agent2"]);
}

#[test]
fn test_parse_layout_spec_roundtrip_with_worktree_workspace_marker() {
    let layout = parse_layout_spec("cmd; agent1:codex(worktree), agent2:claude").unwrap();
    assert_eq!(
        layout.render(),
        "cmd; agent1:codex(worktree), agent2:claude"
    );
    assert_eq!(iter_layout_names(&layout), vec!["cmd", "agent1", "agent2"]);
}

#[test]
fn test_parse_layout_spec_accepts_role_id_leaf_token() {
    let layout = parse_layout_spec("agent1:codex, agentroles.archi:codex").unwrap();
    let leaves = layout.iter_leaves();
    assert_eq!(leaves[1].name, "agentroles.archi");
    assert_eq!(leaves[1].provider.as_deref(), Some("codex"));
    assert_eq!(layout.render(), "agent1:codex, agentroles.archi:codex");
}

#[test]
fn test_parse_layout_spec_percent_token() {
    for (spec, expected_percent) in [
        ("debugger:agy@30", Some(30)),
        ("reviewer:claude@50", Some(50)),
        ("worker:codex(worktree)@40", Some(40)),
        ("debugger:agy", None),
    ] {
        let layout = parse_layout_spec(spec).unwrap();
        assert!(matches!(
            layout,
            ccb_agents::layout::LayoutNode::Leaf { .. }
        ));
        let leaf = &layout.iter_leaves()[0];
        assert_eq!(leaf.percent, expected_percent);
        assert_eq!(layout.render(), spec);
    }
}

#[test]
fn test_prune_layout_preserves_branch_shape_when_possible() {
    let layout = parse_layout_spec("cmd; (agent1:codex, agent2:claude)").unwrap();
    let pruned = prune_layout(&layout, &["cmd".into(), "agent2".into()]).unwrap();
    assert_eq!(pruned.render(), "cmd; agent2:claude");
}

#[test]
fn test_build_balanced_layout_adds_cmd_leaf_first() {
    let mut providers = HashMap::new();
    providers.insert("agent1".into(), "codex".into());
    providers.insert("agent2".into(), "claude".into());
    providers.insert("agent3".into(), "gemini".into());
    let mut modes = HashMap::new();
    modes.insert("agent2".into(), "worktree".into());

    let layout = build_balanced_layout(
        &["agent1", "agent2", "agent3"],
        Some(&providers),
        Some(&modes),
        true,
    );

    assert_eq!(
        layout.render(),
        "cmd, agent1:codex; agent2:claude(worktree), agent3:gemini"
    );
}

#[test]
fn test_parse_layout_spec_rejects_invalid_leaf_token() {
    let err = parse_layout_spec("cmd; ???").unwrap_err();
    assert!(err.to_string().contains("invalid layout token"));
}

#[test]
fn test_resolve_layout_spec_preserves_percent_token() {
    let rendered = resolve_layout_spec(
        &["agent1".into()],
        &HashMap::new(),
        true,
        "cmd; agent1:codex@65",
    )
    .unwrap();
    assert_eq!(rendered, "cmd; agent1:codex@65");
}
