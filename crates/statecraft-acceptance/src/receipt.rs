//! The receipt: evidence that a specific suite passed over specific bytes.
//!
//! Spec 005 section 3.4. Minted **only** when the suite passed over a candidate
//! whose HEAD did not move and whose work tree stayed clean.
//!
//! A receipt is **not a permission**, and nothing about it authorizes a later
//! effect on its own. That sentence is the reason this type has no method that
//! does anything but describe itself.

use crate::absence::{Absence, Recorded};
use crate::independence::SuiteResult;
use crate::judged::Judged;
use serde::{Deserialize, Serialize};

/// A minted receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    /// Which repository.
    pub repository: String,
    /// The base revision.
    pub base: String,
    /// The candidate sha.
    pub candidate: String,
    /// The ordered suite, each command with its exit code.
    pub suite: Vec<SuiteEntry>,
    /// The policy digest.
    pub policy_digest: String,
    /// This product's version.
    pub product_version: String,
    /// The spec-spine version.
    pub spec_spine_version: String,
    /// The adapter's version.
    pub adapter_version: String,
    /// Which harness revision the worker actually resolved.
    ///
    /// **This product's to record**, because only the thing that launched the
    /// worker can observe it. No harness package exists yet, so this reads
    /// `not-recorded` rather than being omitted: omitting it would make every
    /// receipt minted before the package silently unanswerable on the point, and
    /// the name-precedence trap makes the question a real one.
    pub harness_revision: Recorded<String>,
    /// The attempt this receipt belongs to.
    pub attempt: String,
    /// The authority-set paths the candidate touched.
    #[serde(default)]
    pub authority_paths_touched: Vec<String>,
}

/// One command in the receipt's ordered suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuiteEntry {
    /// The command.
    pub command: String,
    /// Its exit code.
    pub exit_code: i32,
}

/// Why a receipt was not minted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "reason")]
pub enum NoReceipt {
    /// The suite did not pass.
    SuiteDidNotPass {
        /// How many checks did not run, which is part of the outcome.
        unrun_checks: u32,
    },
    /// HEAD moved during the suite.
    HeadMoved,
    /// The work tree was dirty at the end of the suite.
    WorkTreeDirty {
        /// Which paths, named.
        dirty_paths: Vec<String>,
    },
    /// The candidate's diff touches the authority set.
    AuthorityChange {
        /// What a reader needs to know.
        note: String,
    },
}

/// Everything a receipt needs beyond what is being judged.
#[derive(Debug, Clone)]
pub struct MintContext {
    /// Which repository.
    pub repository: String,
    /// This product's version.
    pub product_version: String,
    /// The spec-spine version.
    pub spec_spine_version: String,
    /// The adapter's version.
    pub adapter_version: String,
    /// The attempt identity.
    pub attempt: String,
    /// The authority-set paths touched, from the authority evaluation.
    pub authority_paths_touched: Vec<String>,
}

/// Mint a receipt, or say why not.
///
/// The three refusals are checked before the pass, because each of them makes
/// "the suite passed over these bytes" untrue in a different way, and a reader
/// deserves the specific one.
pub fn mint(
    judged: &Judged,
    suite: &SuiteResult,
    context: &MintContext,
    authority_change: Option<String>,
) -> Result<Receipt, NoReceipt> {
    if let Some(note) = authority_change {
        return Err(NoReceipt::AuthorityChange { note });
    }
    if !judged.candidate.head_stable {
        return Err(NoReceipt::HeadMoved);
    }
    if !judged.candidate.work_tree_clean {
        return Err(NoReceipt::WorkTreeDirty {
            dirty_paths: judged.candidate.dirty_paths.clone(),
        });
    }
    if !suite.passed() {
        return Err(NoReceipt::SuiteDidNotPass {
            unrun_checks: suite.unrun_count(),
        });
    }

    Ok(Receipt {
        repository: context.repository.clone(),
        base: judged.base.sha.clone(),
        candidate: judged.candidate.sha.clone(),
        suite: suite
            .checks
            .iter()
            .map(|c| SuiteEntry {
                command: c.command.clone(),
                exit_code: c.exit_code.unwrap_or(-1),
            })
            .collect(),
        policy_digest: judged.policy.digest.clone(),
        product_version: context.product_version.clone(),
        spec_spine_version: context.spec_spine_version.clone(),
        adapter_version: context.adapter_version.clone(),
        // No harness package exists. Present, reading `not-recorded`.
        harness_revision: Recorded::Absent(Absence::NotRecorded),
        attempt: context.attempt.clone(),
        authority_paths_touched: context.authority_paths_touched.clone(),
    })
}

/// Whether a receipt still describes the branch's head.
///
/// A receipt for a head that is no longer the branch's is **`stale`**: a
/// reported state, not an error and not a pass.
pub fn freshness(receipt: &Receipt, branch_head: &str) -> Option<Absence> {
    if receipt.candidate == branch_head {
        None
    } else {
        Some(Absence::Stale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::independence::Check;
    use crate::judged::{Base, Candidate, Policy};

    fn judged(head_stable: bool, clean: bool) -> Judged {
        Judged {
            candidate: Candidate {
                sha: "c".repeat(40),
                work_tree_clean: clean,
                head_stable,
                dirty_paths: if clean {
                    vec![]
                } else {
                    vec!["src/leftover.rs".into()]
                },
            },
            base: Base {
                sha: "b".repeat(40),
            },
            policy: Policy {
                digest: "d".repeat(64),
            },
        }
    }

    fn context() -> MintContext {
        MintContext {
            repository: "statecraft-cli".into(),
            product_version: "0.0.0".into(),
            spec_spine_version: "0.18.0".into(),
            adapter_version: "1.0.0".into(),
            attempt: "run-1/1".into(),
            authority_paths_touched: vec![],
        }
    }

    fn passing() -> SuiteResult {
        SuiteResult::new(vec![Check::ran("make gate", 0, None)], None)
    }

    #[test]
    fn a_passing_suite_over_stable_clean_bytes_mints() {
        let r = mint(&judged(true, true), &passing(), &context(), None).unwrap();
        assert_eq!(r.suite[0].exit_code, 0);
        assert_eq!(r.policy_digest, "d".repeat(64));
    }

    #[test]
    fn the_harness_revision_is_present_reading_not_recorded_never_omitted() {
        let r = mint(&judged(true, true), &passing(), &context(), None).unwrap();
        assert_eq!(r.harness_revision, Recorded::Absent(Absence::NotRecorded));
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("harness_revision"), "present");
        assert!(json.contains("not-recorded"), "and reading not-recorded");
    }

    #[test]
    fn a_moved_head_mints_no_receipt() {
        assert_eq!(
            mint(&judged(false, true), &passing(), &context(), None),
            Err(NoReceipt::HeadMoved)
        );
    }

    #[test]
    fn a_dirty_work_tree_mints_no_receipt_and_names_the_paths() {
        match mint(&judged(true, false), &passing(), &context(), None) {
            Err(NoReceipt::WorkTreeDirty { dirty_paths }) => {
                assert_eq!(dirty_paths, ["src/leftover.rs"]);
            }
            other => panic!("expected a dirty work tree, got {other:?}"),
        }
    }

    #[test]
    fn an_authority_change_mints_no_receipt_even_when_the_suite_passes() {
        let r = mint(
            &judged(true, true),
            &passing(),
            &context(),
            Some("the candidate touches the check suite".into()),
        );
        assert!(matches!(r, Err(NoReceipt::AuthorityChange { .. })));
    }

    #[test]
    fn a_receipt_for_a_head_the_branch_moved_past_is_stale() {
        let r = mint(&judged(true, true), &passing(), &context(), None).unwrap();
        assert_eq!(freshness(&r, &r.candidate), None);
        assert_eq!(freshness(&r, &"f".repeat(40)), Some(Absence::Stale));
    }

    #[test]
    fn an_unrun_check_prevents_a_receipt_and_the_count_is_carried() {
        let suite = SuiteResult::new(
            vec![
                Check::ran("a", 0, None),
                Check::did_not_run("b", "never reached"),
            ],
            None,
        );
        match mint(&judged(true, true), &suite, &context(), None) {
            Err(NoReceipt::SuiteDidNotPass { unrun_checks }) => assert_eq!(unrun_checks, 1),
            other => panic!("expected no receipt, got {other:?}"),
        }
    }
}
