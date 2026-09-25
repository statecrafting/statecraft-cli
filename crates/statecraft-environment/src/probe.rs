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
        // Spec 002 section 3.23: spec-spine's answer is translated, never
        // passed through. A corpus that does not validate and a stale tree are
        // both findings about the corpus; a read that was not performed and a
        // binary that is absent or lacks the verb establish nothing about it.
        let answer = run_check(&self.spec_spine, path);
        match answer {
            CheckAnswer::Fresh => CorpusState::Compiles,
            CheckAnswer::DoesNotValidate { .. } | CheckAnswer::Stale { .. } => {
                CorpusState::Broken(answer.describe())
            }
            CheckAnswer::NotPerformed { .. } => CorpusState::CheckNotPerformed(answer.describe()),
            CheckAnswer::Unavailable { .. } => CorpusState::CheckUnavailable(answer.describe()),
        }
    }
}

/// Which of the two readings spec-spine's exit 2 carries.
///
/// Contract 4 of spec 002 section 3.23 gives exit 2 two readings, stale or an
/// unresolved claim, and the translation keeps them apart in the text rather
/// than in the code. The producer's own words are the only place they differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleReading {
    /// The committed shards are behind the tree; regenerating cures it.
    Stale,
    /// A claim does not resolve; regenerating does not cure it.
    UnresolvedClaim,
    /// The producer's text names neither.
    Undistinguished,
}

/// Why no answer about the corpus could be had at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailability {
    /// The binary could not be run.
    Absent,
    /// The binary runs and does not carry `check` (contract 5).
    LacksVerb,
    /// Candidates exist and none may judge: an override the repository does
    /// not admit or that names no executable, or no compatible convention
    /// candidate (contract 2 as amended; spec 002 section 5, 2026-09-25).
    NotSelected,
}

/// What `spec-spine check` answered, in spec-spine's vocabulary.
///
/// Spec 002 section 3.23, "Two exit vocabularies, and no numeric passthrough
/// between them". [`CheckAnswer::exit_code`] is the translation into spec 006
/// section 3.3's vocabulary, and it is the only place this product turns a
/// `check` answer into a code of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckAnswer {
    /// Exit 0: fresh.
    Fresh,
    /// Exit 1: the corpus does not validate.
    DoesNotValidate {
        /// The producer's first line.
        detail: String,
    },
    /// Exit 2: stale, or an unresolved claim.
    Stale {
        /// Which reading, as far as the producer's text says.
        reading: StaleReading,
        /// The producer's first line.
        detail: String,
    },
    /// Exit 3, or any end outside spec-spine's four answers: the read was not
    /// performed, and nothing about the corpus was established.
    NotPerformed {
        /// How `check` ended: `exit N`, or `signal`.
        status: String,
        /// The producer's first line.
        detail: String,
    },
    /// The binary is absent, or lacks the verb: a precondition not met.
    Unavailable {
        /// Which.
        why: Unavailability,
        /// What was observed.
        detail: String,
    },
}

impl CheckAnswer {
    /// This product's exit code for the answer (spec 006 section 3.3), by
    /// section 3.23's translation table: 0 to 0, 1 and 2 to 1, 3 to 4, and an
    /// absent binary or a missing verb to 2.
    pub fn exit_code(&self) -> i32 {
        match self {
            CheckAnswer::Fresh => 0,
            CheckAnswer::DoesNotValidate { .. } | CheckAnswer::Stale { .. } => 1,
            CheckAnswer::Unavailable { .. } => 2,
            CheckAnswer::NotPerformed { .. } => 4,
        }
    }

    /// A one-line rendering that says which answer this was.
    pub fn describe(&self) -> String {
        match self {
            CheckAnswer::Fresh => "spec-spine check: fresh".to_string(),
            CheckAnswer::DoesNotValidate { detail } => {
                format!("spec-spine check found a corpus that does not validate: {detail}")
            }
            CheckAnswer::Stale { reading, detail } => match reading {
                StaleReading::Stale => format!(
                    "spec-spine check found the committed artifacts stale, which regenerating \
                     cures: {detail}"
                ),
                StaleReading::UnresolvedClaim => format!(
                    "spec-spine check found an unresolved claim, which regenerating does not \
                     cure: {detail}"
                ),
                StaleReading::Undistinguished => format!(
                    "spec-spine check exited 2, stale or an unresolved claim, and its text does \
                     not say which: {detail}"
                ),
            },
            CheckAnswer::NotPerformed { status, detail } => format!(
                "spec-spine check did not perform its read ({status}), so nothing about the \
                 corpus was established: {detail}"
            ),
            CheckAnswer::Unavailable { why, detail } => match why {
                Unavailability::Absent => {
                    format!("spec-spine is not available, so nothing was checked: {detail}")
                }
                Unavailability::LacksVerb => {
                    format!("spec-spine does not carry `check`, so nothing was checked: {detail}")
                }
                Unavailability::NotSelected => format!(
                    "no spec-spine the repository admits was selected, so nothing was checked: \
                     {detail}"
                ),
            },
        }
    }
}

/// Run `check` in `dir` and read its answer.
///
/// Contract 5 of spec 002 section 3.23 first: the verb is established with
/// `check --help` before `check`'s exit code is read, because a binary that
/// lacks the verb spends a code of its own on the unknown subcommand (`clap`'s
/// 2, which would read as stale; spec-spine 0.23.0's 3, which would read as a
/// read not performed).
pub fn run_check(program: &str, dir: &Path) -> CheckAnswer {
    if let Some(unavailable) = check_unavailable(program, dir) {
        return unavailable;
    }
    let output = match Command::new(program).arg("check").current_dir(dir).output() {
        Ok(o) => o,
        Err(e) => {
            return CheckAnswer::Unavailable {
                why: Unavailability::Absent,
                detail: format!("{program}: {e}"),
            };
        }
    };
    // Both streams, because an answer can land on either, and the first
    // non-empty line is the one an operator needs.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let detail = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no output")
        .to_string();
    match output.status.code() {
        Some(0) => CheckAnswer::Fresh,
        Some(1) => CheckAnswer::DoesNotValidate { detail },
        Some(2) => {
            let lower = text.to_lowercase();
            let reading = if lower.contains("unresolved") {
                StaleReading::UnresolvedClaim
            } else if lower.contains("stale") {
                StaleReading::Stale
            } else {
                StaleReading::Undistinguished
            };
            CheckAnswer::Stale { reading, detail }
        }
        _ => CheckAnswer::NotPerformed {
            status: status_word(&output.status),
            detail,
        },
    }
}

/// Contract 5 alone: `check --help`, and the answer it implies when the binary
/// is absent or does not carry the verb. `None` means the verb is there.
pub fn check_unavailable(program: &str, dir: &Path) -> Option<CheckAnswer> {
    match Command::new(program)
        .args(["check", "--help"])
        .current_dir(dir)
        .output()
    {
        Err(e) => Some(CheckAnswer::Unavailable {
            why: Unavailability::Absent,
            detail: format!("{program}: {e}"),
        }),
        Ok(o) if !o.status.success() => Some(CheckAnswer::Unavailable {
            why: Unavailability::LacksVerb,
            detail: format!("`{program} check --help` ended {}", status_word(&o.status)),
        }),
        Ok(_) => None,
    }
}

fn status_word(status: &std::process::ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| "signal".to_string(), |c| format!("exit {c}"))
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

    // A corpus is there and the tool that judges it is not: not `Absent`
    // (ungoverned), and, by section 3.23's translation, not a verdict about
    // the corpus either. It is the precondition a register refuses on.
    #[test]
    fn a_corpus_whose_tool_cannot_run_is_unavailable_not_absent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("spec-spine.toml"), "").unwrap();
        let probe = CommandProbe {
            git: "git".into(),
            spec_spine: "spec-spine-that-does-not-exist".into(),
        };
        assert!(matches!(
            probe.corpus(dir.path()),
            CorpusState::CheckUnavailable(_)
        ));
        let answer = run_check("spec-spine-that-does-not-exist", dir.path());
        assert!(matches!(
            answer,
            CheckAnswer::Unavailable {
                why: Unavailability::Absent,
                ..
            }
        ));
        assert_eq!(answer.exit_code(), 2);
    }

    #[test]
    fn the_translation_table_is_the_one_section_3_23_states() {
        let detail = || "d".to_string();
        let rows = [
            (CheckAnswer::Fresh, 0),
            (CheckAnswer::DoesNotValidate { detail: detail() }, 1),
            (
                CheckAnswer::Stale {
                    reading: StaleReading::Stale,
                    detail: detail(),
                },
                1,
            ),
            (
                CheckAnswer::Stale {
                    reading: StaleReading::UnresolvedClaim,
                    detail: detail(),
                },
                1,
            ),
            (
                CheckAnswer::NotPerformed {
                    status: "exit 3".into(),
                    detail: detail(),
                },
                4,
            ),
            (
                CheckAnswer::Unavailable {
                    why: Unavailability::Absent,
                    detail: detail(),
                },
                2,
            ),
            (
                CheckAnswer::Unavailable {
                    why: Unavailability::LacksVerb,
                    detail: detail(),
                },
                2,
            ),
        ];
        for (answer, code) in rows {
            assert_eq!(answer.exit_code(), code, "{answer:?}");
        }
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
