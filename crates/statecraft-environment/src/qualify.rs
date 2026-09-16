//! Registration's read-only verdict.
//!
//! Spec 002 section 3.1. `project register` records an absolute path and
//! evaluates a qualification that writes NOTHING inside the target. The verdict
//! is never a bare boolean: an operator who is told "no" without reasons cannot
//! act, and a verdict that collapses "not a git repository" into "no corpus"
//! sends them to the wrong fix.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Why a target qualified, or did not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// The path is a git work tree.
    GitWorkTree,
    /// The path is not a git work tree.
    NotAGitWorkTree,
    /// A base revision resolves.
    BaseRevisionResolves,
    /// No base revision resolves (an empty repository, typically).
    NoBaseRevision,
    /// A spec-spine corpus is present and compiles.
    CorpusCompiles,
    /// A corpus is present and does not compile.
    CorpusDoesNotCompile {
        /// What the compiler said, trimmed to its first line.
        detail: String,
    },
    /// No spec-spine corpus is present.
    NoCorpus,
}

impl Reason {
    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Reason::GitWorkTree => "is a git work tree".into(),
            Reason::NotAGitWorkTree => "is not a git work tree".into(),
            Reason::BaseRevisionResolves => "a base revision resolves".into(),
            Reason::NoBaseRevision => "no base revision resolves".into(),
            Reason::CorpusCompiles => "a spec-spine corpus is present and compiles".into(),
            Reason::CorpusDoesNotCompile { detail } => {
                format!("a spec-spine corpus is present and does not compile: {detail}")
            }
            Reason::NoCorpus => "no spec-spine corpus is present".into(),
        }
    }
}

/// The three verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// A git work tree, a resolvable base revision, and a corpus that compiles.
    Qualified,
    /// A git work tree with no corpus. Visible, never scheduled.
    Ungoverned,
    /// Not a git work tree, or a corpus that does not compile.
    Unqualified,
}

impl Verdict {
    /// Whether work may ever be scheduled against a target with this verdict.
    ///
    /// Only `qualified` may. `ungoverned` and `unqualified` are both visible and
    /// both unschedulable, which is the distinction section 3.1 draws: being
    /// listed is not being eligible.
    pub fn schedulable(self) -> bool {
        matches!(self, Verdict::Qualified)
    }
}

/// A verdict and the reasons behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qualification {
    /// The verdict.
    pub verdict: Verdict,
    /// Every observation that produced it, in the order they were made.
    pub reasons: Vec<Reason>,
}

/// What can be observed about a candidate target.
///
/// A trait because the real implementation shells out to `git` and to
/// `spec-spine`, and because spec 001 forbids this product from becoming a
/// second spec compiler: the corpus question is answered by asking spec-spine,
/// never by reading `.derived/` or parsing a spec.
pub trait TargetProbe {
    /// True when the path is a git work tree.
    fn is_git_work_tree(&self, path: &Path) -> bool;
    /// True when a base revision resolves in that work tree.
    fn has_base_revision(&self, path: &Path) -> bool;
    /// Whether a corpus is present, and whether it compiles.
    fn corpus(&self, path: &Path) -> CorpusState;
}

/// What a probe found out about the corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusState {
    /// No corpus at all.
    Absent,
    /// A corpus that compiles.
    Compiles,
    /// A corpus that does not compile, with the compiler's first line.
    Broken(String),
}

/// Evaluate a target. Reads only; writes nothing anywhere.
pub fn qualify(path: &Path, probe: &dyn TargetProbe) -> Qualification {
    let mut reasons = Vec::new();

    if !probe.is_git_work_tree(path) {
        reasons.push(Reason::NotAGitWorkTree);
        return Qualification {
            verdict: Verdict::Unqualified,
            reasons,
        };
    }
    reasons.push(Reason::GitWorkTree);

    // The corpus question is asked before the base-revision one only in the
    // report's ordering; both are evaluated, because an operator fixing a
    // target wants every reason at once rather than one per attempt.
    let base = probe.has_base_revision(path);
    reasons.push(if base {
        Reason::BaseRevisionResolves
    } else {
        Reason::NoBaseRevision
    });

    match probe.corpus(path) {
        CorpusState::Absent => {
            reasons.push(Reason::NoCorpus);
            Qualification {
                // A git work tree with no corpus is `ungoverned`, NOT
                // `unqualified`: nothing is wrong with it, it is simply not a
                // thing this product may drive.
                verdict: Verdict::Ungoverned,
                reasons,
            }
        }
        CorpusState::Broken(detail) => {
            reasons.push(Reason::CorpusDoesNotCompile { detail });
            Qualification {
                verdict: Verdict::Unqualified,
                reasons,
            }
        }
        CorpusState::Compiles => {
            reasons.push(Reason::CorpusCompiles);
            Qualification {
                verdict: if base {
                    Verdict::Qualified
                } else {
                    // A corpus that compiles but no base revision to diff
                    // against: the coupling gate would have nothing to compare,
                    // so this is not qualified.
                    Verdict::Unqualified
                },
                reasons,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe {
        git: bool,
        base: bool,
        corpus: CorpusState,
    }

    impl TargetProbe for Probe {
        fn is_git_work_tree(&self, _: &Path) -> bool {
            self.git
        }
        fn has_base_revision(&self, _: &Path) -> bool {
            self.base
        }
        fn corpus(&self, _: &Path) -> CorpusState {
            self.corpus.clone()
        }
    }

    fn run(git: bool, base: bool, corpus: CorpusState) -> Qualification {
        qualify(Path::new("/x"), &Probe { git, base, corpus })
    }

    #[test]
    fn a_governed_repository_qualifies() {
        let q = run(true, true, CorpusState::Compiles);
        assert_eq!(q.verdict, Verdict::Qualified);
        assert!(q.verdict.schedulable());
    }

    #[test]
    fn a_path_that_is_not_a_git_work_tree_is_unqualified_with_that_reason() {
        let q = run(false, false, CorpusState::Compiles);
        assert_eq!(q.verdict, Verdict::Unqualified);
        assert_eq!(q.reasons, [Reason::NotAGitWorkTree]);
        assert!(!q.verdict.schedulable());
    }

    #[test]
    fn a_git_repository_with_no_corpus_is_ungoverned_not_unqualified() {
        let q = run(true, true, CorpusState::Absent);
        assert_eq!(q.verdict, Verdict::Ungoverned);
        assert!(q.reasons.contains(&Reason::NoCorpus));
        assert!(!q.verdict.schedulable());
    }

    #[test]
    fn a_corpus_that_does_not_compile_is_unqualified_and_carries_the_detail() {
        let q = run(true, true, CorpusState::Broken("E-001 at specs/1".into()));
        assert_eq!(q.verdict, Verdict::Unqualified);
        assert!(q.reasons.iter().any(
            |r| matches!(r, Reason::CorpusDoesNotCompile { detail } if detail.contains("E-001"))
        ));
    }

    #[test]
    fn a_compiling_corpus_with_no_base_revision_does_not_qualify() {
        let q = run(true, false, CorpusState::Compiles);
        assert_eq!(q.verdict, Verdict::Unqualified);
        assert!(q.reasons.contains(&Reason::NoBaseRevision));
    }

    #[test]
    fn every_verdict_reports_reasons_never_a_bare_boolean() {
        for q in [
            run(true, true, CorpusState::Compiles),
            run(true, true, CorpusState::Absent),
            run(false, false, CorpusState::Absent),
        ] {
            assert!(!q.reasons.is_empty());
        }
    }
}
