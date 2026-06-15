# ProjectView Owns Provider Activity

Date: 2026-05-27

## Context

The sidebar currently renders `ccbd project_view`. Provider-native hooks and
session logs can produce more accurate manual-pane activity than pane text, but
letting the Rust sidebar read provider files would create a second state path.

## Decision

Provider-native activity is advisory evidence owned by `ccbd`. Provider hooks
write agent-scoped activity artifacts, `project_view` validates and merges that
evidence, and `ccb-agent-sidebar` renders only the resulting ProjectView rows.

The Rust sidebar must not read provider activity files directly.

## Consequences

- State precedence stays centralized in Python `project_view`.
- Provider activity can be tested without coupling Rust rendering to provider
  runtime layouts.
- Wrong-agent, wrong-pane, wrong-generation, and stale artifacts can be rejected
  before UI rendering.
- Sidebar behavior remains project-scoped and isolated from global tmux or
  provider configuration.
