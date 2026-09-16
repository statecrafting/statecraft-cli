//! The real probes: `git` and `spec-spine`, asked rather than reimplemented.
//!
//! Spec 001 section 3.2 makes this a hard boundary. Specification semantics
//! belong to spec-spine, and this product is a consumer of its supported
//! commands and its structured reports: never a second compiler, and never an
//! ad-hoc read of `.derived/`. So "does this corpus compile?" is answered by
//! running `spec-spine check`, and the answer is its exit status.

use crate::adapter::HarnessProbe;
use crate::qualify::{CorpusState, TargetProbe};
use std::path::Path;
use std::process::Command;

/// A probe that shells out to the real tools.
#[derive(Debug, Clone)]
pub struct CommandProbe {
    /// The `git` binary to run.
    pub git: String,
    /// The `spec-spine` binary to run.
    pub spec_spine: String,
}

impl Default for CommandProbe {
    fn default() -> Self {
        Self {
            git: "git".into(),
            spec_spine: "spec-spine".into(),
        }
    }
}

impl CommandProbe {
    fn run(&self, program: &str, dir: &Path, args: &[&str]) -> Option<std::process::Output> {
        Command::new(program)
            .args(args)
            .current_dir(dir)
            .output()
            .ok()
    }
}

impl TargetProbe for CommandProbe {
    fn is_git_work_tree(&self, path: &Path) -> bool {
        // `rev-parse --is-inside-work-tree` prints `true` and exits 0 inside a
        // work tree, and fails outside one. A bare repository prints `false`,
        // which is why the output is read rather than only the exit status: a
        // bare repository has no work tree to install an environment into.
        self.run(&self.git, path, &["rev-parse", "--is-inside-work-tree"])
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
            .unwrap_or(false)
    }

    fn has_base_revision(&self, path: &Path) -> bool {
        self.run(&self.git, path, &["rev-parse", "--verify", "HEAD"])
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn corpus(&self, path: &Path) -> CorpusState {
        if !path.join("spec-spine.toml").exists() && !path.join("specs").is_dir() {
            return CorpusState::Absent;
        }
        match self.run(&self.spec_spine, path, &["check"]) {
            None => CorpusState::Broken("spec-spine is not runnable".into()),
            Some(o) if o.status.success() => CorpusState::Compiles,
            Some(o) => {
                // Both streams, because a refusal can land on either, and the
                // first non-empty line is the one an operator needs.
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stderr),
                    String::from_utf8_lossy(&o.stdout)
                );
                let first = text
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("spec-spine check failed with no output")
                    .to_string();
                CorpusState::Broken(first)
            }
        }
    }
}

/// A harness probe that asks whether a directory or file is present.
///
/// Deliberately thin. Detecting a harness properly means knowing that harness,
/// which is an adapter's job (section 3.9), not this crate's. This exists so a
/// caller can wire something real without every caller inventing the same
/// directory test.
#[derive(Debug, Clone)]
pub struct MarkerProbe {
    root: std::path::PathBuf,
    markers: Vec<(String, String)>,
    prerequisites: Vec<(String, String, String)>,
}

impl MarkerProbe {
    /// A probe rooted at a target repository.
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            root: root.into(),
            markers: Vec::new(),
            prerequisites: Vec::new(),
        }
    }

    /// A harness is present when `marker` exists inside the target.
    #[must_use]
    pub fn harness_marker(mut self, harness: &str, marker: &str) -> Self {
        self.markers.push((harness.into(), marker.into()));
        self
    }

    /// A prerequisite is satisfied when `marker` exists inside the target.
    #[must_use]
    pub fn prerequisite_marker(mut self, harness: &str, prerequisite: &str, marker: &str) -> Self {
        self.prerequisites
            .push((harness.into(), prerequisite.into(), marker.into()));
        self
    }
}

impl HarnessProbe for MarkerProbe {
    fn harness_present(&self, harness: &str) -> bool {
        self.markers
            .iter()
            .filter(|(h, _)| h == harness)
            .any(|(_, m)| crate::claimant::resolve(&self.root, m).exists())
    }

    fn prerequisite_satisfied(&self, harness: &str, prerequisite: &str) -> bool {
        self.prerequisites
            .iter()
            .filter(|(h, p, _)| h == harness && p == prerequisite)
            .any(|(_, _, m)| crate::claimant::resolve(&self.root, m).exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_that_is_not_a_repository_is_not_a_work_tree() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!CommandProbe::default().is_git_work_tree(dir.path()));
    }

    #[test]
    fn a_directory_with_no_corpus_reports_absent_without_running_spec_spine() {
        let dir = tempfile::tempdir().unwrap();
        let probe = CommandProbe {
            git: "git".into(),
            // A binary that does not exist: reaching it would be the bug.
            spec_spine: "spec-spine-that-does-not-exist".into(),
        };
        assert_eq!(probe.corpus(dir.path()), CorpusState::Absent);
    }

    #[test]
    fn a_corpus_whose_tool_cannot_run_is_broken_not_absent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("spec-spine.toml"), "").unwrap();
        let probe = CommandProbe {
            git: "git".into(),
            spec_spine: "spec-spine-that-does-not-exist".into(),
        };
        assert!(matches!(probe.corpus(dir.path()), CorpusState::Broken(_)));
    }

    #[test]
    fn a_marker_probe_finds_a_harness_by_its_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        let probe = MarkerProbe::new(dir.path()).harness_marker("claude-code", ".claude");
        assert!(probe.harness_present("claude-code"));
        assert!(!probe.harness_present("other"));
    }

    #[test]
    fn a_prerequisite_marker_is_separate_from_the_harness_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        let probe = MarkerProbe::new(dir.path())
            .harness_marker("claude-code", ".claude")
            .prerequisite_marker("claude-code", "loads-pointer", ".claude/settings.json");
        assert!(probe.harness_present("claude-code"));
        assert!(!probe.prerequisite_satisfied("claude-code", "loads-pointer"));
    }
}
