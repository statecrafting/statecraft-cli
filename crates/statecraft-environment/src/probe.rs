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

/// Which of the two readings spec-spine's staleness answer carries.
///
/// Contract 4 of spec 002 section 3.23 gives exit 2 two readings, stale or an
/// unresolved claim, and the translation keeps them apart in the text rather
/// than in the code. The producer's own words are the only place they differ.
/// From spec-spine 0.26.0 a stale tree exits 1 (spec 002 section 5,
/// 2026-09-25, "both exit tables"), and the same words say so.
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
    read_check(output.status.code(), &status_word(&output.status), &text)
}

/// Read one `check` answer by its exit code and the producer's words, under
/// either of spec-spine's exit tables (see [`names_stale_only`]).
pub fn read_check(code: Option<i32>, status: &str, text: &str) -> CheckAnswer {
    let detail = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no output")
        .to_string();
    match code {
        Some(0) => CheckAnswer::Fresh,
        // Under spec-spine 0.26.0's table a stale tree is 1, beside a corpus
        // that does not validate, and the report lines say which.
        Some(1) if names_stale_only(text) => CheckAnswer::Stale {
            reading: StaleReading::Stale,
            detail,
        },
        Some(1) => CheckAnswer::DoesNotValidate { detail },
        // Under 0.26.0's table 2 is a refusal to judge, a pin not met among
        // them; under 0.25.0's it is stale. The refusal names itself.
        Some(2) if names_refusal(text) => CheckAnswer::NotPerformed {
            status: status.to_string(),
            detail,
        },
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
            status: status.to_string(),
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

/// Whether spec-spine's words report a stale tree and nothing else.
///
/// Spec-spine has two exit tables among the releases a repository may pin
/// (spec 002 section 5, 2026-09-25, "both exit tables"): below 0.26.0 stale is
/// exit 2, and from 0.26.0 it is exit 1, beside a corpus that does not
/// validate. The code alone cannot say which table answered, so an answer is
/// read by the producer's own report lines, which name each half: `STALE` on
/// `check`, "is stale" on a registry query. A report that also names an
/// invalid corpus or an unresolved claim is not stale only, because
/// regenerating would not cure it. From 0.27.0 the guarded readers report an
/// unresolved claim as `validation failed` at exit 1 (spec-spine's 145), which
/// is named here so that no later wording beside it reads as stale.
pub fn names_stale_only(text: &str) -> bool {
    (text.contains(": STALE") || text.contains("is stale"))
        && !text.contains("INVALID")
        && !text.contains("UNRESOLVED CLAIM")
        && !text.contains("but REFUSED")
        && !text.contains("validation failed")
}

/// Whether spec-spine's words report a refusal to judge at all: a pin the
/// running version does not satisfy, invalid configuration, or a containment
/// refusal. From 0.26.0 each exits 2; a pin not met and a containment refusal
/// say `refused:`, and invalid configuration says `config error:` (measured
/// under 0.26.0 and 0.27.0, spec 002 section 5, 2026-09-25, "config error is
/// a refusal"). Below 0.26.0 they exited 3, and a pin not met is worded the
/// same under both.
pub fn names_refusal(text: &str) -> bool {
    text.contains("spec-spine: refused:")
        || text.contains("spec-spine: config error:")
        || names_pin_refusal(text)
}

/// Whether spec-spine's words report a version pin the running binary does not
/// satisfy (0.25.0: exit 3 "config error"; 0.26.0: exit 2 "refused").
pub fn names_pin_refusal(text: &str) -> bool {
    text.contains("requires spec-spine")
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

    // Recorded 2026-09-25 from the published 0.25.0 and 0.26.0 on a clone of
    // this repository: one appended line in a spec.md, and a pin of =0.1.0.
    const STALE_025_026: &str = "spec-registry: STALE\n1 stale shard(s):\n  modified \
        003-work-and-run-semantics.json\ncodebase-index: STALE (run `spec-spine index`)\n";
    const PIN_025: &str = "spec-spine: config error: this repository requires spec-spine =0.1.0 \
        (spec-spine.toml [meta] required_version); running 0.25.0.";
    const PIN_026: &str = "spec-spine: refused: this repository requires spec-spine =0.1.0 \
        (spec-spine.toml [meta] required_version); running 0.26.0.";
    // Recorded 2026-09-25 from the published 0.26.0 and 0.27.0 on clones of
    // this repository: an unknown table in spec-spine.toml (both, exit 2), a
    // link leaving the repository, and `specs_dir = "C:specs"` (0.27.0, exit 2;
    // spec-spine's 144), and an unresolved claim at a guarded reader (0.27.0,
    // exit 1; spec-spine's 145).
    const CONFIG_026_027: &str = "spec-spine: config error: TOML parse error at line 134, \
        column 2\n    |\n134 | [nonsense]\n    |  ^^^^^^^^\n";
    const LINK_027: &str = "spec-spine: refused: refused to read the repository: \
        'docs-outside' is a link to /tmp/outside, outside it (spec 144). A governed read \
        through it would judge content the repository does not hold; remove the link or \
        point it inside the repository";
    const LAYOUT_027: &str = "spec-spine: config error: layout.specs_dir 'C:specs' must name \
        a directory inside the repository, and it contains a ':', a drive, drive-relative or \
        stream form on Windows (spec 144). Use a relative path of plain segments";
    const UNRESOLVED_027: &str = "spec-spine: validation failed: 1 violation(s)\n  I-004 \
        [crates/statecraft-acceptance/src/missing.rs] unresolved claim, not staleness: spec \
        '005-acceptance-and-evidence' file unit 'crates/statecraft-acceptance/src/missing.rs' \
        does not exist; regenerating the index does not clear it, because the claim is \
        recomputed from the corpus on every run\n";

    #[test]
    fn a_configuration_or_containment_refusal_is_never_stale() {
        for (code, words) in [
            (2, CONFIG_026_027),
            (2, LINK_027),
            (2, LAYOUT_027),
            // 0.25.0 spent 3 on the same configuration error.
            (3, CONFIG_026_027),
        ] {
            assert!(names_refusal(words), "{words}");
            assert!(!names_stale_only(words), "{words}");
            assert!(
                matches!(
                    read_check(Some(code), &format!("exit {code}"), words),
                    CheckAnswer::NotPerformed { .. }
                ),
                "exit {code}: {words}"
            );
        }
    }

    #[test]
    fn an_unresolved_claim_at_a_guarded_reader_is_neither_stale_nor_a_refusal() {
        assert!(!names_stale_only(UNRESOLVED_027));
        assert!(!names_refusal(UNRESOLVED_027));
        assert!(matches!(
            read_check(Some(1), "exit 1", UNRESOLVED_027),
            CheckAnswer::DoesNotValidate { .. }
        ));
        // Beside a stale line it is still not stale only.
        let both = format!("codebase-index: STALE (run `spec-spine index`)\n{UNRESOLVED_027}");
        assert!(!names_stale_only(&both));
    }

    /// A real repository with a real link leaving it, answered by a stand-in
    /// that prints what the published 0.27.0 printed for that tree. The
    /// stand-in is labelled as one: the published binary's own answer over a
    /// link is asserted in `statecraft-home`'s negative cases once the pin is
    /// 0.27.0, because 0.26.0 follows the link.
    #[cfg(unix)]
    #[test]
    fn a_repository_with_a_link_leaving_it_is_a_read_not_performed() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let outside = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        symlink(outside.path(), repo.path().join("docs-outside")).unwrap();
        let bin = repo.path().join("spec-spine-0.27.0-stand-in");
        std::fs::write(
            &bin,
            "#!/bin/sh\ncase \"$*\" in\n  'check --help') exit 0 ;;\n  check) \
             printf '%s\\n' \"spec-spine: refused: refused to read the repository: \
             'docs-outside' is a link to $(readlink docs-outside), outside it (spec 144).\" >&2; \
             exit 2 ;;\nesac\nexit 3\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        match run_check(bin.to_str().unwrap(), repo.path()) {
            CheckAnswer::NotPerformed { status, detail } => {
                assert_eq!(status, "exit 2");
                assert!(detail.contains("outside it"), "{detail}");
            }
            other => panic!("a link leaving the repository read as {other:?}"),
        }
    }

    #[test]
    fn both_exit_tables_read_to_the_same_answers() {
        let stale = |a: &CheckAnswer| {
            matches!(
                a,
                CheckAnswer::Stale {
                    reading: StaleReading::Stale,
                    ..
                }
            )
        };
        let not_performed = |a: &CheckAnswer| matches!(a, CheckAnswer::NotPerformed { .. });
        // Stale: 2 under 0.25.0, 1 under 0.26.0, the same words.
        assert!(stale(&read_check(Some(2), "exit 2", STALE_025_026)));
        assert!(stale(&read_check(Some(1), "exit 1", STALE_025_026)));
        // A pin not met: 3 under 0.25.0, 2 under 0.26.0; never stale.
        assert!(not_performed(&read_check(Some(3), "exit 3", PIN_025)));
        assert!(not_performed(&read_check(Some(2), "exit 2", PIN_026)));
        // 0.26.0's failed (4) and usage (3) are reads not performed.
        assert!(not_performed(&read_check(
            Some(4),
            "exit 4",
            "internal error"
        )));
        // Exit 1 that names an invalid corpus, even beside a stale half, is not
        // stale only: regenerating would not cure it.
        let invalid = "spec-registry: INVALID\ncodebase-index: STALE\n";
        assert!(matches!(
            read_check(Some(1), "exit 1", invalid),
            CheckAnswer::DoesNotValidate { .. }
        ));
        assert!(matches!(
            read_check(Some(1), "exit 1", "spec-spine: validation failed"),
            CheckAnswer::DoesNotValidate { .. }
        ));
        // 0.25.0's unresolved reading of 2 is unchanged.
        assert!(matches!(
            read_check(Some(2), "exit 2", "codebase-index: UNRESOLVED CLAIM"),
            CheckAnswer::Stale {
                reading: StaleReading::UnresolvedClaim,
                ..
            }
        ));
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
