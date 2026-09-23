//! The `work`, `run` and `accept` bindings.
//!
//! Added by spec 006's additive `extends` edge on this crate. Same rule as
//! [`crate::bind`]: arguments in, **one** library operation, a value out, an exit
//! code. Spec 006 section 3.11 sharpens it for this slice: where a verb needed
//! something the owning library did not expose, the entry point was added
//! **there** (`statecraft_run::session`, `statecraft_run::work::Eligibility`,
//! `statecraft_acceptance::suite`) and these functions call it.
//!
//! Nothing here derives an answer an owning crate could have returned. The view
//! types are the exception the crate already has a reason for: spec 006 section
//! 3.4 makes this crate's JSON the contract, so the wire shape is 006's and an
//! owning crate's internal type is not it.

use crate::exit::Exit;
use crate::render::Answer;
use serde::Serialize;
use statecraft_acceptance::judged::NoAcceptance;
use statecraft_acceptance::outcome::{
    Acceptance, RecordedPosture, ReviewableOutcome, render_posture,
};
use statecraft_run::attempt::{Outcome, Run};
use statecraft_run::report::ReportError;
use statecraft_run::session::{Concluded, SessionError};
use statecraft_run::work::{Eligibility, WorkList};

/// A ready set, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkListView {
    /// Eligible units.
    pub eligible: Vec<WorkRowView>,
    /// Ready specs the policy excluded, each with its reason.
    pub excluded: Vec<ExcludedView>,
    /// Whether the lifecycle policy was declared by the target or defaulted.
    pub policy_declared: bool,
    /// A disagreement between the target's declaration and what this product
    /// holds. The declaration still won.
    pub policy_disagreement: Option<String>,
    /// The spec-spine version whose reports this was computed from.
    pub spec_spine_version: String,
}

/// One eligible unit, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkRowView {
    /// The spec id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its status.
    pub status: String,
    /// **Which report field this row came from.**
    ///
    /// Spec 006 section 3.8 rule 1: a reader must never have to guess whether
    /// `ready` and `status` came from one answer. They did not, so every row
    /// says where each half came from.
    pub from_field: String,
    /// The status field's own source, which is the other report.
    pub status_from_field: String,
    /// Present when only an operator override made this eligible.
    pub admitted_by_override: Option<String>,
}

/// One excluded spec, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedView {
    /// The spec id.
    pub id: String,
    /// Its status, or `unknown` when the lifecycle report does not carry it.
    pub status: String,
    /// Why it was excluded.
    pub reason: String,
}

/// Where the `status` half of every row comes from.
pub const STATUS_FIELD: &str = "registry list --json: status";

impl WorkListView {
    fn of(list: &WorkList) -> Self {
        Self {
            eligible: list
                .eligible
                .iter()
                .map(|i| WorkRowView {
                    id: i.id.clone(),
                    title: i.title.clone(),
                    status: i.status.clone(),
                    from_field: i.from_field.clone(),
                    status_from_field: STATUS_FIELD.to_string(),
                    admitted_by_override: i
                        .admitted_by_override
                        .as_ref()
                        .map(|o| format!("{}: {}", o.operator, o.reason)),
                })
                .collect(),
            excluded: list
                .excluded
                .iter()
                .map(|e| ExcludedView {
                    id: e.id.clone(),
                    status: e.status.clone(),
                    reason: e.reason.clone(),
                })
                .collect(),
            policy_declared: !list.policy_was_defaulted(),
            policy_disagreement: list.policy_disagreement.clone(),
            spec_spine_version: list.spec_spine_version.clone(),
        }
    }
}

/// `work list <path>`
///
/// Spec 006 section 3.10: an empty ready set is **0**. It is an answer to a
/// question that was asked and answered, not a finding.
pub fn work_list_answer(list: WorkList) -> Answer<WorkListView> {
    let view = WorkListView::of(&list);
    let mut summary = String::new();
    if view.eligible.is_empty() && view.excluded.is_empty() {
        summary.push_str("no ready specs\n");
    }
    for row in &view.eligible {
        summary.push_str(&format!(
            "eligible {:<34} {:<10} [{}] [{}]\n",
            row.id, row.status, row.from_field, row.status_from_field
        ));
        if let Some(o) = &row.admitted_by_override {
            summary.push_str(&format!("  admitted by override: {o}\n"));
        }
    }
    for row in &view.excluded {
        summary.push_str(&format!(
            "excluded {:<34} {:<10} {}\n",
            row.id, row.status, row.reason
        ));
    }
    if let Some(d) = &view.policy_disagreement {
        summary.push_str(&format!("policy disagreement: {d}\n"));
    }
    summary.push_str(&format!(
        "policy {} | spec-spine {}\n",
        if view.policy_declared {
            "declared by the target"
        } else {
            "defaulted"
        },
        view.spec_spine_version
    ));
    Answer::new(view, Exit::Ok, summary)
}

/// `work show <path> <id>`
///
/// Spec 006 section 3.10: a spec excluded by spec 003 section 3.1.1 is **1**. The
/// reason is the answer, and it is not a failure.
pub fn work_show_answer(eligibility: Eligibility) -> Answer<Eligibility> {
    let exit = if eligibility.schedulable() {
        Exit::Ok
    } else {
        Exit::Finding
    };
    let summary = format!("{}\n", eligibility.describe());
    Answer::new(eligibility, exit, summary)
}

/// A report gap, mapped.
///
/// Spec 006 section 3.10 and spec 003 section 3.8: a report lacking a field a
/// verb needs is **2**, naming the field and the spec-spine version. Never a
/// locally derived substitute, and nothing was done.
pub fn report_error_answer(e: &ReportError) -> Answer<String> {
    let exit = match e {
        // A missing field is a precondition that stopped the operation.
        ReportError::MissingField { .. } => Exit::Refused,
        // A corpus that does not compile is the target's state, reported as
        // itself: the operation ran and found it.
        ReportError::CorpusDoesNotCompile { .. } => Exit::Finding,
        // spec-spine absent is a precondition, and nothing was done: spec 001
        // section 3.2 makes it the only thing that may answer a specification
        // question here, so without it there is no answer to have. An operator
        // can install it, which is what distinguishes 2 from 4.
        ReportError::NotRunnable { .. } => Exit::Refused,
        // A report this build cannot read is nobody's request.
        ReportError::Unreadable { .. } => Exit::Failed,
    };
    Answer::new(e.to_string(), exit, e.to_string())
}

/// A concluded attempt, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcludedView {
    /// The posture folded from this attempt's durable outcome record.
    pub posture: RecordedPosture,
    /// The run.
    pub run_id: String,
    /// The attempt number.
    pub attempt: u32,
    /// The outcome the supervisor decided.
    pub outcome: String,
    /// What the adapter classified its own termination as, kept as a claim.
    pub adapter_claimed: String,
    /// How many refusals the supervisor counted.
    pub refusals: u32,
    /// Whether the base revision moved while the attempt ran.
    pub base_moved: bool,
    /// The workspace retained for the next attempt.
    pub workspace_retained: String,
    /// The attempt's startup evidence (spec 006 section 3.11.3).
    pub startup: statecraft_home::launch::RunStartup,
}

impl ConcludedView {
    fn of(
        c: &Concluded,
        posture: RecordedPosture,
        startup: statecraft_home::launch::RunStartup,
    ) -> Self {
        Self {
            posture,
            run_id: c.run_id.clone(),
            attempt: c.attempt,
            outcome: c.outcome.word().to_string(),
            adapter_claimed: c.adapter_claimed.word().to_string(),
            refusals: c.refusals,
            base_moved: c.base_moved,
            workspace_retained: c.workspace_retained.clone(),
            startup,
        }
    }
}

/// `run <path> <id>`
///
/// Spec 006 section 3.10, stated twice there and once more here: **0 means the
/// attempt reached its own end, and carries no acceptance claim whatever.** A
/// caller that wants an acceptance runs `accept` and reads its code.
pub fn run_answer(concluded: Concluded) -> Answer<ConcludedView> {
    run_answer_with_posture(
        concluded,
        statecraft_acceptance::absence::Recorded::Absent(
            statecraft_acceptance::absence::Absence::NotRecorded,
        ),
        statecraft_home::launch::RunStartup::unmanaged(),
    )
}

/// Render a concluded attempt beside the posture read from its durable record.
pub fn run_answer_with_posture(
    concluded: Concluded,
    posture: RecordedPosture,
    startup: statecraft_home::launch::RunStartup,
) -> Answer<ConcludedView> {
    let exit = match concluded.outcome {
        // Spec 006 section 3.11.3: a launch whose record was not stored is a
        // failure nobody asked for, whatever the attempt's outcome.
        _ if startup.not_stored() => Exit::Failed,
        Outcome::Completed => Exit::Ok,
        // The operation ran and reports an outcome that is not clean. Nothing
        // about the product failed.
        Outcome::Failed | Outcome::Refused | Outcome::Interrupted | Outcome::Cancelled => {
            Exit::Finding
        }
    };
    let mut summary = format!(
        "run {} attempt {}: {}\n",
        concluded.run_id,
        concluded.attempt,
        concluded.outcome.word()
    );
    if concluded.adapter_claimed != concluded.outcome {
        summary.push_str(&format!(
            "  the adapter claimed {} and the supervisor decided {}\n",
            concluded.adapter_claimed.word(),
            concluded.outcome.word()
        ));
    }
    if concluded.refusals > 0 {
        summary.push_str(&format!("  refusals counted: {}\n", concluded.refusals));
    }
    if concluded.base_moved {
        summary.push_str("  the base revision moved during the attempt\n");
    }
    if concluded.outcome == Outcome::Completed {
        summary.push_str("  completed says nothing about acceptance; run `accept` for that\n");
    }
    summary.push_str(&render_posture(&posture));
    summary.push_str(&startup.describe());
    Answer::new(
        ConcludedView::of(&concluded, posture, startup),
        exit,
        summary,
    )
}

/// A session that could not begin or conclude, mapped.
pub fn session_error_answer(e: &SessionError) -> Answer<String> {
    let exit = match e {
        // A live attempt is a precondition, named (spec 003 section 3.7).
        SessionError::LiveAttempt { .. } => Exit::Refused,
        // An unresolvable base or an occupied path is also a precondition.
        SessionError::Workspace(_) => Exit::Refused,
        // A record that could not be written is nobody's request.
        SessionError::Record(_) => Exit::Failed,
    };
    Answer::new(e.to_string(), exit, e.to_string())
}

/// A run refused because an attempt is live, with what that attempt's launch
/// records establish (spec 002 section 3.32 rule 24).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAttemptView {
    /// The refusal, as spec 003 section 3.7 words it.
    pub refusal: String,
    /// The live run.
    pub run_id: String,
    /// Its live attempt.
    pub attempt: u32,
    /// The launch state its records establish, where it has any.
    pub launch_state: Option<String>,
    /// Why, in order.
    pub reasons: Vec<String>,
    /// What to do next.
    pub next: String,
}

/// `run` refused because an attempt is live. Exit 2, as for any live attempt;
/// the answer adds what the attempt's launch records establish and infers no
/// outcome.
pub fn live_attempt_answer(
    e: &SessionError,
    root: &str,
    startup: Option<statecraft_home::launch::AttemptStartup>,
) -> Answer<LiveAttemptView> {
    let (run_id, attempt) = match e {
        SessionError::LiveAttempt { run_id, attempt } => (run_id.clone(), *attempt),
        _ => (String::new(), 0),
    };
    let inspect = format!("startup show {root} {run_id} --attempt {attempt}");
    let next = match startup.as_ref().and_then(|s| s.next.clone()) {
        Some(next) => format!("read `{inspect}`; {next}"),
        None => format!(
            "read `{inspect}`; the attempt stays live until it is reconciled (spec 003 section \
             3.6), and this build has no verb that reconciles it"
        ),
    };
    let view = LiveAttemptView {
        refusal: e.to_string(),
        run_id,
        attempt,
        launch_state: startup.as_ref().map(|s| s.verdict.word().to_string()),
        reasons: startup.map(|s| s.reasons).unwrap_or_default(),
        next,
    };
    let mut summary = format!("{}\n", view.refusal);
    if let Some(state) = &view.launch_state {
        summary.push_str(&format!("  launch state: {state}\n"));
    }
    for reason in &view.reasons {
        summary.push_str(&format!("  - {reason}\n"));
    }
    summary.push_str(&format!("  next: {}\n", view.next));
    Answer::new(view, Exit::Refused, summary)
}

/// Every run for a target, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunListView {
    /// One row per run.
    pub runs: Vec<RunRowView>,
}

/// One run's attempts, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRowView {
    /// The run id.
    pub id: String,
    /// One entry per attempt, oldest first.
    pub attempts: Vec<AttemptRowView>,
}

/// One attempt, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRowView {
    /// Its number within the run.
    pub number: u32,
    /// The base it resolved at its start.
    pub base_commit: String,
    /// Its outcome, or `null` while it is live.
    pub outcome: Option<String>,
}

/// `run list <path>`
///
/// A fold of the record and nothing else (spec 006 section 3.9). An empty
/// register of runs is 0, for the same reason an empty ready set is.
pub fn run_list_answer(runs: Vec<Run>) -> Answer<RunListView> {
    let view = RunListView {
        runs: runs
            .iter()
            .map(|r| RunRowView {
                id: r.id.clone(),
                attempts: r
                    .attempts
                    .iter()
                    .map(|a| AttemptRowView {
                        number: a.number,
                        base_commit: a.base_commit.clone(),
                        outcome: a.outcome.map(|o| o.word().to_string()),
                    })
                    .collect(),
            })
            .collect(),
    };
    let mut summary = String::new();
    if view.runs.is_empty() {
        summary.push_str("no runs recorded\n");
    }
    for run in &view.runs {
        for a in &run.attempts {
            summary.push_str(&format!(
                "{:<20} attempt {:<3} {:<12} base {}\n",
                run.id,
                a.number,
                a.outcome.clone().unwrap_or_else(|| "live".to_string()),
                a.base_commit
            ));
        }
    }
    Answer::new(view, Exit::Ok, summary)
}

/// `run show <path> <run>`
///
/// The reviewable outcome of spec 005 section 3.9, folded from the record. Every
/// value names the record it came from, which is why the account is returned
/// whole rather than reshaped here.
pub fn run_show_answer(account: ReviewableOutcome) -> Answer<ReviewableOutcome> {
    // Reporting an account is what the verb is for, whatever the account says,
    // so this is 0 unless the run is not in the record at all, which the caller
    // checks before folding.
    let summary = account.render();
    Answer::new(account, Exit::Ok, summary)
}

/// The refusal for a run id the record does not carry.
pub fn no_such_run_answer(run_id: &str) -> Answer<String> {
    let detail =
        format!("the record carries no run `{run_id}`; `run list` names every run it does carry");
    Answer::new(detail.clone(), Exit::Refused, detail)
}

/// `accept <path> <run>`
///
/// Spec 006 section 3.10, four rows at once. **Every unsuccessful answer is a
/// finding, not a failure and not a silent zero**, except the one precondition
/// that stopped the operation before it judged anything:
///
/// - the attempt outcome is not `completed`: `not-attempted` with the reason
///   named, exit 1;
/// - the suite failed: a finding with no receipt, exit 1;
/// - the suite never ran: no acceptance and the unrun checks counted, exit 1,
///   which is neither a pass nor a fail;
/// - the policy digest could not be computed at the base: refused, exit 2, and
///   the reason is recorded.
pub fn accept_answer(acceptance: Acceptance) -> Answer<Acceptance> {
    let exit = match &acceptance {
        Acceptance::Accepted { .. } => Exit::Ok,
        Acceptance::Failed { .. } => Exit::Finding,
        Acceptance::NotAttempted { .. } => Exit::Finding,
        Acceptance::None { reason } => match reason {
            // A digest nobody can compute identifies no policy, so nothing was
            // judged: that is a precondition, which is 2.
            NoAcceptance::PolicyDigestUncomputable { .. } => Exit::Refused,
            NoAcceptance::CandidateUnidentified { .. } | NoAcceptance::BaseUnidentified { .. } => {
                Exit::Refused
            }
            // The suite never ran. The operation ran and reports that, which is
            // a finding: not a pass and not a fail.
            NoAcceptance::SuiteDidNotRun { .. } => Exit::Finding,
        },
    };

    let mut summary = format!("acceptance: {}\n", acceptance.word());
    match &acceptance {
        Acceptance::Accepted { receipt } => {
            summary.push_str(&format!(
                "  receipt over candidate {} at base {}, {} check(s)\n",
                receipt.candidate,
                receipt.base,
                receipt.suite.len()
            ));
            summary.push_str(
                "  a receipt is evidence that a suite passed over these bytes; it is not a \
                 permission and nothing here publishes\n",
            );
        }
        Acceptance::Failed { reason } => {
            summary.push_str(&format!("  no receipt: {reason:?}\n"));
        }
        Acceptance::NotAttempted {
            reason,
            refusal_count,
        } => {
            summary.push_str(&format!("  reason: {}\n", reason.word()));
            if let Some(count) = refusal_count {
                summary.push_str(&format!("  refusals counted: {count}\n"));
            }
            summary.push_str("  no receipt\n");
        }
        Acceptance::None { reason } => {
            summary.push_str(&format!("  {reason:?}\n"));
        }
    }
    Answer::new(acceptance, exit, summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_run::policy::{Overrides, Policy};
    use statecraft_run::report::{CorpusReport, ReadySpec, SpecLifecycle};
    use statecraft_run::work::select;

    fn list(ready: &[(&str, &str)]) -> WorkList {
        select(
            &CorpusReport {
                spec_spine_version: "0.20.0".into(),
                ready: ready
                    .iter()
                    .map(|(id, _)| ReadySpec {
                        id: (*id).to_string(),
                        title: "t".into(),
                    })
                    .collect(),
                lifecycle: ready
                    .iter()
                    .map(|(id, status)| SpecLifecycle {
                        id: (*id).to_string(),
                        status: (*status).to_string(),
                        implementation: Some("pending".into()),
                    })
                    .collect(),
            },
            &Policy::default_policy(),
            &Overrides::none(),
            None,
        )
    }

    #[test]
    fn an_empty_ready_set_is_zero_and_not_a_finding() {
        let a = work_list_answer(list(&[]));
        assert_eq!(a.exit, Exit::Ok);
        assert!(a.summary.contains("no ready specs"));
    }

    #[test]
    fn every_row_names_the_report_each_half_came_from() {
        let a = work_list_answer(list(&[("010", "approved")]));
        let row = &a.value.eligible[0];
        assert!(row.from_field.contains("registry plan"));
        assert_eq!(row.status_from_field, STATUS_FIELD);
        // And the human rendering carries both, so the two are never conflated
        // by a reader either.
        assert!(a.summary.contains("registry plan"));
        assert!(a.summary.contains("registry list"));
    }

    #[test]
    fn a_draft_is_listed_as_excluded_with_its_reason_and_is_never_scheduled() {
        let a = work_list_answer(list(&[("011", "draft")]));
        assert_eq!(a.exit, Exit::Ok);
        assert!(a.value.eligible.is_empty());
        assert_eq!(a.value.excluded.len(), 1);
        assert_eq!(a.value.excluded[0].status, "draft");
    }

    #[test]
    fn a_ready_spec_the_lifecycle_report_does_not_carry_is_excluded_as_unknown() {
        let report = CorpusReport {
            spec_spine_version: "0.20.0".into(),
            ready: vec![ReadySpec {
                id: "012".into(),
                title: "t".into(),
            }],
            lifecycle: vec![],
        };
        let a = work_list_answer(select(
            &report,
            &Policy::default_policy(),
            &Overrides::none(),
            None,
        ));
        assert_eq!(a.value.excluded[0].status, "unknown");
        assert!(a.value.excluded[0].reason.contains("status is unknown"));
    }

    #[test]
    fn work_show_on_an_excluded_spec_is_a_finding_and_not_a_failure() {
        let a = work_show_answer(list(&[("011", "draft")]).eligibility_of("011"));
        assert_eq!(a.exit, Exit::Finding);
        assert_ne!(a.exit, Exit::Failed);
    }

    #[test]
    fn a_missing_report_field_refuses_naming_the_field_and_the_version() {
        let e = ReportError::MissingField {
            command: "registry plan --json".into(),
            field: "status".into(),
            version: "0.20.0".into(),
        };
        let a = report_error_answer(&e);
        assert_eq!(a.exit, Exit::Refused);
        assert!(a.summary.contains("status"));
        assert!(a.summary.contains("0.20.0"));
    }

    #[test]
    fn an_absent_spec_spine_is_a_precondition_and_a_report_it_cannot_read_is_a_failure() {
        let absent = report_error_answer(&ReportError::NotRunnable {
            path: "/t".into(),
            detail: "no such file".into(),
        });
        assert_eq!(absent.exit, Exit::Refused);
        assert!(absent.summary.contains("spec-spine"));

        let unreadable = report_error_answer(&ReportError::Unreadable {
            command: "registry plan --json".into(),
            version: "0.20.0".into(),
            detail: "not the JSON this build reads".into(),
        });
        assert_eq!(unreadable.exit, Exit::Failed);
    }

    #[test]
    fn a_completed_run_is_zero_and_says_so_carries_no_acceptance_claim() {
        let a = run_answer(Concluded {
            run_id: "r1".into(),
            attempt: 1,
            outcome: Outcome::Completed,
            adapter_claimed: Outcome::Completed,
            refusals: 0,
            base_moved: false,
            workspace_retained: "/w".into(),
        });
        assert_eq!(a.exit, Exit::Ok);
        assert!(a.summary.contains("says nothing about acceptance"));
    }

    #[test]
    fn a_refused_run_is_a_finding_and_retains_the_adapters_disagreeing_claim() {
        let a = run_answer(Concluded {
            run_id: "r1".into(),
            attempt: 1,
            outcome: Outcome::Refused,
            adapter_claimed: Outcome::Completed,
            refusals: 2,
            base_moved: false,
            workspace_retained: "/w".into(),
        });
        assert_eq!(a.exit, Exit::Finding);
        assert_eq!(a.value.adapter_claimed, "completed");
        assert_eq!(a.value.outcome, "refused");
        assert!(a.summary.contains("refusals counted: 2"));
    }

    #[test]
    fn a_live_attempt_refuses_the_second_run_rather_than_failing_it() {
        let a = session_error_answer(&SessionError::LiveAttempt {
            run_id: "r1".into(),
            attempt: 1,
        });
        assert_eq!(a.exit, Exit::Refused);
        assert!(a.summary.contains("r1"));
    }

    #[test]
    fn a_live_attempt_shows_as_live_and_never_as_an_outcome() {
        let mut run = Run::new("r1");
        run.append_attempt("abc");
        let a = run_list_answer(vec![run]);
        assert_eq!(a.value.runs[0].attempts[0].outcome, None);
        assert!(a.summary.contains("live"));
    }

    #[test]
    fn an_attempt_that_is_not_completed_is_a_finding_with_no_receipt() {
        let a = accept_answer(Acceptance::NotAttempted {
            reason: statecraft_acceptance::judged::NotAttemptedReason::AttemptRefused,
            refusal_count: Some(2),
        });
        assert_eq!(a.exit, Exit::Finding);
        assert!(a.summary.contains("attempt-refused"));
        assert!(a.summary.contains("refusals counted: 2"));
        assert!(a.summary.contains("no receipt"));
        assert!(a.value.receipt().is_none());
    }

    #[test]
    fn a_suite_that_never_ran_is_a_finding_and_neither_a_pass_nor_a_fail() {
        let a = accept_answer(Acceptance::None {
            reason: NoAcceptance::SuiteDidNotRun { unrun_checks: 3 },
        });
        assert_eq!(a.exit, Exit::Finding);
        assert_ne!(a.exit, Exit::Ok);
        assert_ne!(a.exit, Exit::Failed);
        assert!(a.summary.contains("3"));
    }

    #[test]
    fn an_uncomputable_policy_digest_refuses_and_the_reason_is_recorded() {
        let a = accept_answer(Acceptance::None {
            reason: NoAcceptance::PolicyDigestUncomputable {
                detail: "the base does not resolve".into(),
            },
        });
        assert_eq!(a.exit, Exit::Refused);
        assert!(a.summary.contains("does not resolve"));
    }

    #[test]
    fn a_failed_suite_is_a_finding_with_no_receipt() {
        let a = accept_answer(Acceptance::Failed {
            reason: statecraft_acceptance::receipt::NoReceipt::SuiteDidNotPass { unrun_checks: 0 },
        });
        assert_eq!(a.exit, Exit::Finding);
        assert!(a.value.receipt().is_none());
    }

    #[test]
    fn a_run_the_record_does_not_carry_is_a_refusal() {
        let a = no_such_run_answer("nope");
        assert_eq!(a.exit, Exit::Refused);
        assert!(a.summary.contains("nope"));
    }
}

/// Why `startup trial` refused before an attempt was appended (spec 006
/// section 3.11.4, exit 2).
pub fn trial_refused_answer(reason: &str) -> Answer<serde_json::Value> {
    Answer::new(
        serde_json::json!({ "operation": "trial-refused", "reason": reason }),
        Exit::Refused,
        format!("startup trial refused: {reason}\nNothing was launched.\n"),
    )
}

/// What `startup trial` answers once its attempt is concluded.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialView {
    /// The trial's run.
    pub run_id: String,
    /// Its attempt.
    pub attempt: u32,
    /// The attempt's outcome.
    pub outcome: String,
    /// The attempt's startup records, read back, with the trial's section.
    pub startup: Option<statecraft_home::launch::AttemptStartup>,
    /// Why something was not stored or not read, where it was not.
    pub not_stored: Option<String>,
}

/// Map a concluded trial onto spec 006 section 3.11.4's codes.
pub fn trial_answer(
    concluded: &Concluded,
    shown: Result<statecraft_home::launch::AttemptStartup, statecraft_home::launch::NotRead>,
    sentinel_placed: bool,
    not_stored: Option<String>,
) -> Answer<TrialView> {
    use statecraft_home::trial::TrialVerdict;
    let mut not_stored = not_stored;
    let startup = match shown {
        Ok(s) => Some(s),
        Err(e) => {
            not_stored.get_or_insert(format!("the trial's records could not be read back: {e}"));
            None
        }
    };
    let trial = startup.as_ref().and_then(|s| s.trial.as_ref());
    if sentinel_placed && not_stored.is_none() {
        match trial {
            None => {
                not_stored = Some("the trial's record was written and is not there".to_string());
            }
            Some(t) if !t.agrees => {
                not_stored = Some(
                    "the trial's judgement read back from disk differs from the one written"
                        .to_string(),
                );
            }
            Some(_) => {}
        }
    }
    let exit = match (&not_stored, trial.map(|t| t.judgement.verdict)) {
        (Some(_), _) => Exit::Failed,
        (None, Some(TrialVerdict::Established)) => Exit::Ok,
        _ => Exit::Finding,
    };
    let mut summary = format!(
        "startup trial: run {} attempt {}: {}\n",
        concluded.run_id,
        concluded.attempt,
        concluded.outcome.word()
    );
    if !sentinel_placed {
        summary
            .push_str("  no sentinel was placed and no session was started; the trial is spent\n");
    }
    if let Some(s) = &startup {
        summary.push_str(&s.describe());
    }
    if let Some(why) = &not_stored {
        summary.push_str(&format!("NOT STORED: {why}\n"));
    }
    Answer::new(
        TrialView {
            run_id: concluded.run_id.clone(),
            attempt: concluded.attempt,
            outcome: concluded.outcome.word().to_string(),
            startup,
            not_stored,
        },
        exit,
        summary,
    )
}
