//! Mirrors Python `lib/provider_backends/pane_quiet_support/reader.py`.

use regex::Regex;
use std::fmt::Debug;

/// Backend capable of reading pane content.
///
/// Mirrors the small duck-typed backend interface used by `PaneSnapshotReader`.
pub trait PaneContentBackend: Debug {
    fn get_pane_content(&self, pane_id: &str, lines: usize) -> Option<String>;
}

impl<B: PaneContentBackend> PaneContentBackend for &B {
    fn get_pane_content(&self, pane_id: &str, lines: usize) -> Option<String> {
        (*self).get_pane_content(pane_id, lines)
    }
}

/// Reader that snapshots pane content, stripping ANSI escapes.
///
/// Mirrors Python `PaneSnapshotReader`.
#[derive(Debug)]
pub struct PaneSnapshotReader<B: PaneContentBackend> {
    pub backend: B,
    pub pane_id: String,
    pub lines: usize,
}

impl<B: PaneContentBackend> PaneSnapshotReader<B> {
    pub fn new(backend: B, pane_id: impl Into<String>, lines: usize) -> Self {
        Self {
            backend,
            pane_id: pane_id.into(),
            lines,
        }
    }

    /// Capture the current pane content with ANSI escapes removed.
    pub fn snapshot(&self) -> String {
        let content = self
            .backend
            .get_pane_content(&self.pane_id, self.lines)
            .unwrap_or_default();
        strip_ansi(&content)
    }
}

fn ansi_re() -> Regex {
    Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").unwrap()
}

/// Strip ANSI escape sequences from text.
pub fn strip_ansi(text: &str) -> String {
    ansi_re().replace_all(text, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestBackend {
        text: String,
    }

    impl PaneContentBackend for TestBackend {
        fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Option<String> {
            Some(self.text.clone())
        }
    }

    #[test]
    fn test_snapshot_strips_ansi() {
        let reader = PaneSnapshotReader::new(
            TestBackend {
                text: "\x1b[32mhello\x1b[0m world".to_string(),
            },
            "%1",
            200,
        );
        assert_eq!(reader.snapshot(), "hello world");
    }

    #[test]
    fn test_snapshot_returns_empty_when_backend_fails() {
        #[derive(Debug)]
        struct EmptyBackend;
        impl PaneContentBackend for EmptyBackend {
            fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Option<String> {
                None
            }
        }
        let reader = PaneSnapshotReader::new(EmptyBackend, "%1", 200);
        assert_eq!(reader.snapshot(), "");
    }
}
