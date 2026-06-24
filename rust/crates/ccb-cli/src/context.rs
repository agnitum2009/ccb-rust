//! Mirrors Python `lib/cli/context.py`.
//!
//! CLI execution context: resolves project root, builds path layout,
//! and bundles everything needed to execute a parsed command.
//! 1:1 alignment with Python class.

use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use thiserror::Error;

use ccb_agents::workspace::ProjectContext;
use ccb_storage::paths::PathLayout;

use crate::models::ParsedCommand;

/// Errors that can occur while building CLI context.
#[derive(Error, Debug)]
pub enum CliContextError {
    #[error("could not determine project root from {cwd}")]
    NoProjectRoot { cwd: PathBuf },
    #[error("path layout error: {0}")]
    PathLayout(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Fully resolved CLI execution context.
///
/// Mirrors Python `CliContext` dataclass.
#[derive(Debug, Clone)]
pub struct CliContext {
    pub command: ParsedCommand,
    pub cwd: PathBuf,
    pub project: ProjectContext,
    pub paths: PathLayout,
}

/// Builder for `CliContext`.
///
/// Mirrors Python `build_cli_context()` factory.
#[derive(Clone)]
pub struct CliContextBuilder {
    command: ParsedCommand,
    cwd: Option<PathBuf>,
}

impl CliContextBuilder {
    pub fn new(command: ParsedCommand) -> Self {
        Self { command, cwd: None }
    }

    pub fn cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// Build the context by resolving project root and path layout.
    pub fn build(self) -> Result<CliContext, CliContextError> {
        let cwd = self
            .cwd
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        // Use explicit project from command, or discover from cwd.
        let project_root = if let Some(proj) = self.command.project() {
            PathBuf::from(proj)
        } else {
            find_project_root(&cwd)
                .ok_or_else(|| CliContextError::NoProjectRoot { cwd: cwd.clone() })?
        };

        let project_id = project_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "default".to_string());

        let project = ProjectContext::new(&project_root, &project_id);
        let utf8_root = Utf8PathBuf::from_path_buf(project_root.clone())
            .unwrap_or_else(|p| Utf8PathBuf::from(p.to_string_lossy().as_ref()));
        let paths = PathLayout::new(utf8_root);

        Ok(CliContext {
            command: self.command,
            cwd,
            project,
            paths,
        })
    }
}

/// Walk up from `start` looking for a `.ccb` directory marker.
///
/// Mirrors Python `find_project_root`.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".ccb").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
