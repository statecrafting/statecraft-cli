//! Running the suite, and folding one run's account out of the record.
//!
//! Added by spec `009`'s additive `extends` edge on this crate. Spec 009 section
//! 3.5: a binding that needs a library entry point gets it in the library that
//! owns the behavior, and both entry points here are behavior spec `005` owns.
//!
//! # Where the suite comes from
//!
//! Spec 005 section 3.2: "the suite is run by this product, in the prepared
//! workspace, from the instructions read **at the base**." The instructions a
//! spec declares are its `## Verification` block, and the thing that reads them
//! is `spec-spine verify`, which emits a structured verdict. So the suite is
//! asked of spec-spine and its report is read, which is rule 2 of that same
//! section and spec 001 section 3.2's boundary: this product never answers a
//! specification question itself.
//!
//! # The fold
//!
//! [`fold`] reads spec 005 section 3.9's reviewable account out of the run
//! record and **nothing else** (spec 009 section 3.3). Every value carries the
//! record it came from, and a value the record does not carry is one of the
//! three names for absence rather than a default.

use crate::absence::{Absence, Recorded, Statement};
use crate::authority::{CorpusVerdict, Verdict as AuthorityVerdict};
use crate::dimensions::{
    Admission, Dimensions, Integrity, IssuerTrust, RefusalCode, Signature, SubjectBinding,
};
use crate::independence::{Check, SuiteResult};
use crate::judged::NotAttemptedReason;
use crate::outcome::{Acceptance, ReviewableOutcome, Sourced};
use serde::Deserialize;
use statecraft_run::attempt::Outcome;
use statecraft_run::record::{Entry, Kind};
use std::path::Path;

/// Where the suite result comes from.
///
/// A trait so a test does not need a corpus on disk, and so the one real
/// implementation is the only place a command line is spelled.
pub trait SuiteSource {
    /// Run the declared acceptance for one spec in a prepared workspace.
    fn run_suite(&self, workspace: &Path, spec_id: &str) -> SuiteResult;
}

/// The real source: `spec-spine verify <spec> --json`, in the workspace.
#[derive(Debug, Clone)]
pub struct SpecSpineVerify {
    /// The binary to run.
    pub binary: String,
}

impl Default for SpecSpineVerify {
    fn default() -> Self {
        Self {
            binary: "spec-spine".into(),
        }
    }
}

/// The verdict envelope `verify --json` emits (spec-spine's spec 037).
#[derive(Debug, Clone, Deserialize)]
struct VerifyEnvelope {
    #[serde(rename = "exitCode")]
    exit_code: i32,
    report: VerifyReport,
}

#[derive(Debug, Clone, Deserialize)]
struct VerifyReport {
    #[serde(default)]
    declared: bool,
    outcome: String,
    #[serde(default)]
    ran: u32,
    #[serde(default)]
    total: u32,
}

impl SuiteSource for SpecSpineVerify {
    fn run_suite(&self, workspace: &Path, spec_id: &str) -> SuiteResult {
        let command = format!("spec-spine verify {spec_id} --json");
        let output = std::process::Command::new(&self.binary)
            .args(["verify", spec_id, "--json"])
            .current_dir(workspace)
            .output();

        let Ok(output) = output else {
            // Section 3.2 rule 3: a check that did not run is `unknown`, never
            // a pass, and never a failure either: nothing was judged.
            return SuiteResult::new(
                vec![Check::did_not_run(
                    &command,
                    "spec-spine could not be run in the prepared workspace",
                )],
                None,
            );
        };

        let text = String::from_utf8_lossy(&output.stdout);
        // The envelope is the last JSON object on stdout: the verb prints its
        // human lines first. Parsing from the first `{` of the last line that
        // starts one is how the envelope is found without reading the prose.
        let envelope: Option<VerifyEnvelope> = text
            .lines()
            .rev()
            .find_map(|_| None::<VerifyEnvelope>)
            .or_else(|| {
                let start = text.find("{\n  \"exitCode\"")?;
                serde_json::from_str(&text[start..]).ok()
            });

        let Some(envelope) = envelope else {
            // Section 3.10: a tool that should have emitted a structured report
            // and did not is `unknown` for what the report would have carried.
            // The exit code is retained and does not decide.
            let code = output.status.code().unwrap_or(-1);
            return SuiteResult::new(vec![Check::missing_report(&command, code)], None);
        };

        if !envelope.report.declared {
            return SuiteResult::new(
                vec![Check::did_not_run(
                    &command,
                    &format!("{spec_id} declares no acceptance, so nothing was run"),
                )],
                None,
            );
        }

        // Rule 2: the structured report is what is read. `outcome` is
        // spec-spine's own verdict; the exit code is recorded beside it.
        let structured_pass = Some(envelope.report.outcome == "passed");
        let mut checks = vec![Check::ran(&command, envelope.exit_code, structured_pass)];
        if envelope.report.ran < envelope.report.total {
            checks.push(Check::did_not_run(
                &format!("{command} (the remaining declared commands)"),
                &format!(
                    "{} of {} declared commands ran; the rest are unknown, which is never a pass",
                    envelope.report.ran, envelope.report.total
                ),
            ));
        }
        SuiteResult::new(checks, None)
    }
}

/// Fold one run's reviewable account out of the record.
///
/// Reads the chain's entries and nothing else. `run_id` selects; everything else
/// comes from the entries, each value naming the record it was folded from.
pub fn fold(run_id: &str, entries: &[Entry]) -> ReviewableOutcome {
    let mine: Vec<&Entry> = entries.iter().filter(|e| e.run_id == run_id).collect();
    let outcome_entry = mine.iter().rev().find(|e| e.kind == Kind::Outcome).copied();
    let accounting_entry = mine
        .iter()
        .rev()
        .find(|e| e.kind == Kind::Accounting)
        .copied();
    let reconciliation = mine
        .iter()
        .rev()
        .find(|e| e.kind == Kind::Reconciliation)
        .copied();

    let record_name = |e: Option<&Entry>| match e {
        Some(e) => format!("{}#{}/{}", e.subject, e.run_id, e.attempt),
        None => Absence::NotRecorded.word().to_string(),
    };
    let outcome_record = record_name(outcome_entry);

    // Spec 009 section 3.3: an inspection of a run whose intent has no outcome
    // reports the reconciliation state of spec 003 section 3.6, and `unknown`
    // is shown as `unknown`. It is never an inferred outcome.
    let independent = match (outcome_entry, reconciliation) {
        (Some(e), _) => e
            .detail
            .get("outcome")
            .and_then(|v| v.as_str())
            .unwrap_or(Absence::NotRecorded.word())
            .to_string(),
        (None, Some(r)) => format!(
            "no outcome recorded; reconciliation says {}",
            r.detail
                .get("verdict")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        ),
        (None, None) => format!(
            "no outcome recorded and no reconciliation; {}",
            Absence::Stale.word()
        ),
    };

    let strings = |e: Option<&Entry>, key: &str| -> Vec<String> {
        e.and_then(|e| e.detail.get(key))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };

    // The agent's claim, kept in a field named for a claim and never read as a
    // result (spec 005 section 3.2 rule 1). A narrative is marked as one.
    let claim = match outcome_entry
        .and_then(|e| e.detail.get("agentClaim"))
        .and_then(|v| v.as_str())
    {
        Some(text) => Recorded::Present(Statement::Narrative {
            text: text.to_string(),
        }),
        None => Recorded::Absent(Absence::NotRecorded),
    };

    let acceptance = match outcome_entry
        .and_then(|e| e.detail.get("outcome"))
        .and_then(|v| v.as_str())
        .and_then(word_to_outcome)
    {
        // Acceptance is `accept`'s answer, not `run`'s. An inspection of a run
        // nobody has accepted says exactly that, by name.
        Some(Outcome::Completed) => Acceptance::None {
            reason: crate::judged::NoAcceptance::SuiteDidNotRun { unrun_checks: 0 },
        },
        Some(other) => Acceptance::NotAttempted {
            reason: not_attempted(other),
            refusal_count: accounting_entry
                .and_then(|e| e.detail.get("count"))
                .and_then(|v| v.as_u64())
                .map(|c| c as u32),
        },
        None => Acceptance::None {
            reason: crate::judged::NoAcceptance::CandidateUnidentified {
                detail: "the record carries no outcome for this run".to_string(),
            },
        },
    };

    ReviewableOutcome {
        run_id: run_id.to_string(),
        posture: outcome_entry
            .and_then(|e| e.detail.get("posture"))
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .map(|posture| Recorded::Present(Sourced::new(posture, &outcome_record)))
            .unwrap_or(Recorded::Absent(Absence::NotRecorded)),
        claim,
        independent_result: Sourced::new(independent, &outcome_record),
        requested: Sourced::new(strings(outcome_entry, "requested"), &outcome_record),
        applied: Sourced::new(strings(outcome_entry, "applied"), &outcome_record),
        dimensions: Sourced::new(unrecorded_dimensions(), &record_name(None)),
        admission: Sourced::new(unrecorded_admission(), &record_name(None)),
        authority: Sourced::new(unrecorded_authority(), &record_name(None)),
        acceptance: Sourced::new(acceptance, &outcome_record),
        receipt_freshness: Recorded::Absent(Absence::NotRecorded),
        refusals: Sourced::new(
            strings(accounting_entry, "sample_guards"),
            &record_name(accounting_entry),
        ),
    }
}

fn word_to_outcome(word: &str) -> Option<Outcome> {
    Outcome::all().into_iter().find(|o| o.word() == word)
}

fn not_attempted(outcome: Outcome) -> NotAttemptedReason {
    match crate::judged::eligibility(outcome) {
        Ok(()) => NotAttemptedReason::AttemptFailed,
        Err(reason) => reason,
    }
}

/// The four dimensions as an unaccepted run carries them.
///
/// Each one absent rather than defaulted to a pass: spec 005 section 3.8's whole
/// point is that no name for absence reads as success, and a run that has not
/// been accepted has produced no evidence to judge.
fn unrecorded_dimensions() -> Dimensions {
    Dimensions {
        integrity: Integrity::Unknown,
        signature: Signature::Unknown,
        issuer_trust: IssuerTrust::Unknown,
        subject_binding: SubjectBinding::Unknown,
    }
}

/// Admission for a run nobody has accepted: refused, naming what is missing.
///
/// Spec 005 section 3.8 keeps admission apart from the dimensions, and an
/// unaccepted run has no evidence at all, which `IncompleteEvidence` says by
/// name rather than leaving the field to read as an admission.
fn unrecorded_admission() -> Admission {
    let reason = RefusalCode::IncompleteEvidence {
        missing: vec!["acceptance has not been run for this run".to_string()],
    };
    Admission::Refuse {
        reasons: vec![reason.clone()],
        reason,
    }
}

fn unrecorded_authority() -> AuthorityVerdict {
    AuthorityVerdict {
        repository_members_touched: Vec::new(),
        environment_manifest_touched: false,
        corpus_members: CorpusVerdict::Absent(Absence::NotRecorded),
        corpus_classes: Vec::new(),
        prior_policy_required: Recorded::Absent(Absence::NotRecorded),
        authority_change: false,
        may_accept_on_own_suite: false,
        note: "no acceptance has been run for this run, so no authority-set verdict exists; \
               `accept` is what produces one"
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use statecraft_run::record::Identity;

    fn entry(
        kind: Kind,
        run_id: &str,
        attempt: u32,
        subject: &str,
        detail: serde_json::Value,
    ) -> Entry {
        Entry {
            kind,
            run_id: run_id.to_string(),
            attempt,
            subject: subject.to_string(),
            // Mechanical: spec 003 section 3.3.1 added the field, and this
            // helper's records carry no identity. Nothing 005 requires changes.
            effect_id: Identity::Absent,
            idempotency_key: None,
            detail,
        }
    }

    #[test]
    fn a_completed_run_folds_an_account_that_claims_no_acceptance() {
        let entries = vec![
            entry(
                Kind::Intent,
                "r1",
                1,
                "prepare-workspace",
                json!({"baseCommit": "abc"}),
            ),
            entry(
                Kind::Outcome,
                "r1",
                1,
                "attempt",
                json!({"outcome": "completed", "requested": ["turn-limit"], "applied": ["turn-limit"]}),
            ),
        ];
        let account = fold("r1", &entries);
        assert_eq!(account.independent_result.value, "completed");
        assert_eq!(account.requested.value, ["turn-limit"]);
        assert_eq!(account.posture, Recorded::Absent(Absence::NotRecorded));
        assert_eq!(
            serde_json::to_value(&account).unwrap()["posture"],
            "not-recorded"
        );
        assert!(account.render().contains("posture: not-recorded"));
        // Every value names the record it came from.
        assert!(account.independent_result.from_record.contains("attempt"));
        // And acceptance is absent BY NAME, never implied by a completed run.
        assert_eq!(account.acceptance.value.word(), "no-acceptance");
        assert!(!account.has_receipt());
    }

    #[test]
    fn a_refused_run_folds_not_attempted_with_the_refusal_count() {
        let entries = vec![
            entry(
                Kind::Outcome,
                "r1",
                1,
                "attempt",
                json!({"outcome": "refused"}),
            ),
            entry(
                Kind::Accounting,
                "r1",
                1,
                "refusals",
                json!({"count": 3, "sample_guards": ["permission-deny-rule/Bash"]}),
            ),
        ];
        let account = fold("r1", &entries);
        match &account.acceptance.value {
            Acceptance::NotAttempted {
                reason,
                refusal_count,
            } => {
                assert_eq!(reason.word(), "attempt-refused");
                assert_eq!(*refusal_count, Some(3));
            }
            other => panic!("expected not-attempted, got {other:?}"),
        }
        assert_eq!(account.refusals.value, ["permission-deny-rule/Bash"]);
    }

    #[test]
    fn an_intent_with_no_outcome_shows_the_reconciliation_state_and_never_infers_one() {
        let entries = vec![entry(
            Kind::Intent,
            "r1",
            1,
            "prepare-workspace",
            json!({"baseCommit": "abc"}),
        )];
        let account = fold("r1", &entries);
        assert!(
            account
                .independent_result
                .value
                .contains("no outcome recorded")
        );
        for word in ["completed", "failed", "refused"] {
            assert!(
                !account.independent_result.value.contains(word),
                "no outcome may be inferred, and `{word}` appeared"
            );
        }
    }

    #[test]
    fn a_reconciled_intent_shows_unknown_as_unknown() {
        let entries = vec![
            entry(Kind::Intent, "r1", 1, "prepare-workspace", json!({})),
            entry(
                Kind::Reconciliation,
                "r1",
                1,
                "reconciliation",
                json!({"verdict": "unknown"}),
            ),
        ];
        let account = fold("r1", &entries);
        assert!(account.independent_result.value.contains("unknown"));
    }

    #[test]
    fn the_four_dimensions_of_an_unaccepted_run_are_absent_and_never_a_pass() {
        let entries = vec![entry(
            Kind::Outcome,
            "r1",
            1,
            "attempt",
            json!({"outcome": "completed"}),
        )];
        let account = fold("r1", &entries);
        let d = &account.dimensions.value;
        assert_eq!(d.integrity, Integrity::Unknown);
        assert_eq!(d.signature, Signature::Unknown);
        assert_eq!(d.issuer_trust, IssuerTrust::Unknown);
        assert_eq!(d.subject_binding, SubjectBinding::Unknown);
        assert!(matches!(account.admission.value, Admission::Refuse { .. }));
        // And the authority verdict is `not-recorded`, never a pass.
        assert!(!account.authority.value.may_accept_on_own_suite);
    }

    #[test]
    fn a_fold_ignores_every_other_runs_records() {
        let entries = vec![
            entry(
                Kind::Outcome,
                "r1",
                1,
                "attempt",
                json!({"outcome": "completed"}),
            ),
            entry(
                Kind::Outcome,
                "r2",
                1,
                "attempt",
                json!({"outcome": "failed"}),
            ),
        ];
        assert_eq!(fold("r1", &entries).independent_result.value, "completed");
        assert_eq!(fold("r2", &entries).independent_result.value, "failed");
    }

    #[test]
    fn an_agent_claim_is_kept_in_a_field_named_for_a_claim() {
        let entries = vec![entry(
            Kind::Outcome,
            "r1",
            1,
            "attempt",
            json!({"outcome": "completed", "agentClaim": "I finished the work"}),
        )];
        let account = fold("r1", &entries);
        match &account.claim {
            Recorded::Present(Statement::Narrative { text }) => {
                assert_eq!(text, "I finished the work");
            }
            other => panic!("a claim is a narrative, not a result: {other:?}"),
        }
        assert!(account.render().contains("not a result"));
    }
}
