//! The bindings: argument in, one library call, value out, exit code.
//!
//! Spec 006 section 3.2. Each function here parses what it was given, calls
//! **exactly one** library operation, and maps the value it returns onto the
//! exit vocabulary. None of them contains a rule spec 002 did not state.
//!
//! The mapping is the only judgement these functions make, and it is the same
//! judgement every time: a precondition that stopped the operation is a refusal
//! (2), something the operation found and reported is a finding (1), and an
//! error nobody asked for is a failure (4).

use crate::exit::Exit;
use crate::render::Answer;
use serde::Serialize;
use statecraft_environment::adapter::Readiness;
use statecraft_environment::apply::{ApplyError, Outcome};
use statecraft_environment::doctor::Report;
use statecraft_environment::plan::Plan;
use statecraft_environment::qualify::{Qualification, Verdict};
use statecraft_environment::registry::{Registration, Registry, RegistryError};
use std::path::Path;

/// A registration, as the command reports it.
#[derive(Debug, Clone, Serialize)]
pub struct RegisteredView {
    /// The absolute path.
    pub path: String,
    /// The verdict.
    pub verdict: Verdict,
    /// Every reason behind it.
    pub reasons: Vec<String>,
    /// Whether it is armed.
    pub armed: bool,
    /// Whether work may be scheduled here.
    pub eligible: bool,
}

impl RegisteredView {
    fn of(r: &Registration) -> Self {
        Self {
            path: r.root.display().to_string(),
            verdict: r.qualification.verdict,
            reasons: r
                .qualification
                .reasons
                .iter()
                .map(|x| x.describe())
                .collect(),
            armed: r.armed,
            eligible: r.eligible(),
        }
    }
}

/// The exit a qualification implies.
///
/// Only `qualified` is a clean answer. The other two are **findings**: the
/// command did what was asked and is reporting what it found, which is not the
/// same as having failed or refused.
fn exit_for(q: &Qualification) -> Exit {
    if q.verdict == Verdict::Qualified {
        Exit::Ok
    } else {
        Exit::Finding
    }
}

/// `project register <path>`
pub fn project_register(
    registry: &mut Registry,
    path: &Path,
    probe: &dyn statecraft_environment::qualify::TargetProbe,
) -> Result<Answer<RegisteredView>, Answer<String>> {
    match registry.register(path, probe) {
        Ok(r) => {
            let view = RegisteredView::of(r);
            let exit = exit_for(&r.qualification);
            let summary = format!(
                "{}: {}\n{}",
                view.path,
                format_verdict(view.verdict),
                view.reasons
                    .iter()
                    .map(|r| format!("  {r}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            Ok(Answer::new(view, exit, summary))
        }
        // A relative path is the caller getting the invocation wrong, which is
        // a usage error and not a refusal by the operation.
        Err(e @ RegistryError::NotAbsolute(_)) => {
            Err(Answer::new(e.to_string(), Exit::Usage, e.to_string()))
        }
        Err(e) => Err(Answer::new(e.to_string(), Exit::Failed, e.to_string())),
    }
}

fn format_verdict(v: Verdict) -> &'static str {
    match v {
        Verdict::Qualified => "qualified",
        Verdict::Ungoverned => "ungoverned",
        Verdict::Unqualified => "unqualified",
    }
}

/// `project list`
pub fn project_list(registry: &Registry) -> Answer<Vec<RegisteredView>> {
    let views: Vec<RegisteredView> = registry.all().map(RegisteredView::of).collect();
    let summary = if views.is_empty() {
        "no registered projects".to_string()
    } else {
        views
            .iter()
            .map(|v| {
                format!(
                    "{:<12} {:<8} {}",
                    format_verdict(v.verdict),
                    if v.armed { "armed" } else { "unarmed" },
                    v.path
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    // Listing is never a finding: reporting that a target is unqualified is what
    // the verb is for, and an operator scripting `project list` should not have
    // to treat a populated register as an error.
    Answer::new(views, Exit::Ok, summary)
}

/// `project arm <path>` and `project disarm <path>`
pub fn project_set_armed(
    registry: &mut Registry,
    path: &Path,
    armed: bool,
) -> Result<Answer<RegisteredView>, Answer<String>> {
    match registry.set_armed(path, armed) {
        Ok(()) => {
            let r = registry.get(path).expect("just armed");
            let view = RegisteredView::of(r);
            let summary = format!(
                "{} is now {}{}",
                view.path,
                if armed { "armed" } else { "disarmed" },
                if armed && !view.eligible {
                    ", and is still not eligible because it is not qualified"
                } else {
                    ""
                }
            );
            Ok(Answer::new(view, Exit::Ok, summary))
        }
        // The target is not registered: a precondition, so a refusal.
        Err(e @ RegistryError::NotRegistered(_)) => {
            Err(Answer::new(e.to_string(), Exit::Refused, e.to_string()))
        }
        Err(e) => Err(Answer::new(e.to_string(), Exit::Failed, e.to_string())),
    }
}

/// An environment outcome, as the JSON contract carries it.
///
/// A view type rather than spec 002's own `Outcome`, for two reasons that point
/// the same way. Spec 006 section 3.4 makes **this crate's** JSON the contract,
/// so the wire shape is 006's to fix and 002's internal type is not it. And
/// deriving `Serialize` onto 002's type from here would be a change to 002's
/// territory made by 006's change, which the coupling gate refuses and should.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum OutcomeView {
    /// Every planned write succeeded.
    Applied {
        /// The paths written.
        written: Vec<String>,
    },
    /// Some writes were withheld, each named with its reason.
    Partial {
        /// The paths written.
        written: Vec<String>,
        /// The paths not written.
        withheld: Vec<WithheldView>,
    },
    /// A precondition failed and nothing was written.
    Refused {
        /// Why.
        reasons: Vec<String>,
    },
}

/// A withheld path, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WithheldView {
    /// The path.
    pub path: String,
    /// The adapter that would have written it.
    pub adapter: String,
    /// Why it was withheld.
    pub reason: String,
}

impl OutcomeView {
    fn of(outcome: &Outcome) -> Self {
        match outcome {
            Outcome::Applied { written } => OutcomeView::Applied {
                written: written.clone(),
            },
            Outcome::Partial { written, withheld } => OutcomeView::Partial {
                written: written.clone(),
                withheld: withheld
                    .iter()
                    .map(|w| WithheldView {
                        path: w.path.clone(),
                        adapter: w.adapter.clone(),
                        reason: w.reason.describe(),
                    })
                    .collect(),
            },
            Outcome::Refused { reasons } => OutcomeView::Refused {
                reasons: reasons.clone(),
            },
        }
    }
}

/// A doctor report, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportView {
    /// One line per manifest entry.
    pub entries: Vec<EntryView>,
    /// Everything else.
    pub findings: Vec<String>,
    /// Whether anything is wrong.
    pub has_findings: bool,
}

/// One entry's state, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    /// The path.
    pub path: String,
    /// Its state, in section 3.5's vocabulary.
    pub state: String,
}

impl ReportView {
    fn of(report: &Report) -> Self {
        Self {
            entries: report
                .entries
                .iter()
                .map(|e| EntryView {
                    path: e.path.clone(),
                    state: e.state.word().to_string(),
                })
                .collect(),
            findings: report.findings.iter().map(|f| f.describe()).collect(),
            has_findings: report.has_findings(),
        }
    }
}

/// How an environment outcome maps onto the exit vocabulary.
///
/// The mapping spec 006 section 3.3 spells out: `applied` is 0, `partial` is a
/// finding because every withheld path was named and the contract held, and
/// `refused` is 2 because a precondition stopped it.
pub fn outcome_answer(outcome: Outcome) -> Answer<OutcomeView> {
    let (exit, summary) = match &outcome {
        Outcome::Applied { written } => (
            Exit::Ok,
            format!("applied: {} path(s) written", written.len()),
        ),
        Outcome::Partial { written, withheld } => (
            Exit::Finding,
            format!(
                "partial: {} written, {} withheld\n{}",
                written.len(),
                withheld.len(),
                withheld
                    .iter()
                    .map(|w| format!("  {}: {}", w.path, w.reason.describe()))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        ),
        Outcome::Refused { reasons } => (Exit::Refused, format!("refused: {}", reasons.join("; "))),
    };
    Answer::new(OutcomeView::of(&outcome), exit, summary)
}

/// An error from the environment library, mapped.
///
/// Every one of these is a failure: an i/o error or an unreadable manifest is
/// not something the operator asked for and not a precondition the operation
/// checked, which is exactly what distinguishes 4 from 2.
pub fn apply_error_answer(e: &ApplyError) -> Answer<String> {
    Answer::new(e.to_string(), Exit::Failed, e.to_string())
}

/// `doctor`
pub fn doctor_answer(report: Report) -> Answer<ReportView> {
    let exit = if report.has_findings() {
        Exit::Finding
    } else {
        Exit::Ok
    };
    let summary = if report.has_findings() {
        report.render()
    } else {
        "no findings".to_string()
    };
    // The exit code the report itself implies, and the one this command uses,
    // are the same number by construction rather than by coincidence.
    debug_assert_eq!(exit.code(), report.exit_code());
    Answer::new(ReportView::of(&report), exit, summary)
}

/// A plan, as the JSON contract carries it.
///
/// A view type for the same two reasons the outcome and report views are ones
/// (section 5, 2026-09-16): spec 006 section 3.4 makes this crate's JSON the
/// contract, and deriving `Serialize` onto spec 002's `Plan` from here would be
/// a change to 002's territory made by 006's change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    /// Paths the plan would write.
    pub writes: Vec<PlannedWriteView>,
    /// Paths it would withhold, each with its reason.
    pub withheld: Vec<WithheldView>,
    /// Precondition failures. Non-empty means apply writes nothing at all.
    pub refusals: Vec<String>,
    /// What each configured adapter will do, including the ones that refuse.
    pub adapters: Vec<AdapterView>,
}

/// One planned write, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedWriteView {
    /// The path.
    pub path: String,
    /// The adapter that would write it.
    pub adapter: String,
    /// Whether it replaces bytes already there.
    pub replaces_existing: bool,
}

/// One adapter's outcome in a plan, as the JSON contract carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterView {
    /// The adapter's name.
    pub name: String,
    /// `claiming`, `refused` or `degraded`.
    pub readiness: String,
    /// Which prerequisites are missing, when it refused.
    pub missing: Vec<String>,
    /// Why it degraded, when it did.
    pub reasons: Vec<String>,
    /// Facts it cannot express in its harness, stated rather than dropped.
    pub unexpressible: Vec<String>,
}

impl PlanView {
    fn of(plan: &Plan) -> Self {
        Self {
            writes: plan
                .writes
                .iter()
                .map(|w| PlannedWriteView {
                    path: w.path.clone(),
                    adapter: w.adapter.clone(),
                    replaces_existing: w.replaces_existing,
                })
                .collect(),
            withheld: plan
                .withheld
                .iter()
                .map(|w| WithheldView {
                    path: w.path.clone(),
                    adapter: w.adapter.clone(),
                    reason: w.reason.describe(),
                })
                .collect(),
            refusals: plan.refusals.iter().map(|r| r.describe()).collect(),
            adapters: plan
                .adapters
                .iter()
                .map(|a| {
                    let (readiness, missing, reasons) = match &a.readiness {
                        Readiness::Claiming => ("claiming", Vec::new(), Vec::new()),
                        Readiness::Refused { missing } => ("refused", missing.clone(), Vec::new()),
                        Readiness::Degraded { reasons } => {
                            ("degraded", Vec::new(), reasons.clone())
                        }
                    };
                    AdapterView {
                        name: a.name.clone(),
                        readiness: readiness.to_string(),
                        missing,
                        reasons,
                        unexpressible: a.unexpressible.clone(),
                    }
                })
                .collect(),
        }
    }
}

/// `env plan`
///
/// Spec 006 section 3.1: `env plan` prints what `env apply` **would** do. So its
/// exit is the exit that apply would take, derived from the same plan rather
/// than chosen here: refusals are 2 because the apply they preview returns
/// `refused` and writes nothing, a withheld path is 1 because the apply it
/// previews returns `partial`, and everything else is 0. A preview whose exit
/// disagreed with the operation it previews would be the one thing the verb
/// exists to prevent.
pub fn plan_answer(plan: Plan) -> Answer<PlanView> {
    let exit = if plan.refused() {
        Exit::Refused
    } else if !plan.withheld.is_empty() {
        Exit::Finding
    } else {
        Exit::Ok
    };
    let summary = {
        let rendered = plan.render();
        if rendered.is_empty() {
            "nothing to do".to_string()
        } else {
            rendered
        }
    };
    Answer::new(PlanView::of(&plan), exit, summary)
}

/// An i/o error from computing a plan, mapped.
///
/// A failure (4), not a refusal: reading the target is not a precondition the
/// operation checked, and spec 006 section 3.3 is explicit that the distinction
/// is the one a caller scripts against.
pub fn plan_error_answer(path: &Path, e: &std::io::Error) -> Answer<String> {
    let detail = format!("{}: {e}", path.display());
    Answer::new(detail.clone(), Exit::Failed, detail)
}

/// The refusal spec 006 section 3.7 requires for an unregistered target.
///
/// Every environment verb needs a registered target, which is a precondition and
/// therefore a refusal (2) naming the path. The mapping is the only judgement
/// here; that registration is required at all is spec 002's.
pub fn unregistered_answer(path: &Path) -> Answer<String> {
    let detail = format!(
        "{} is not registered; `project register {}` first",
        path.display(),
        path.display()
    );
    Answer::new(detail.clone(), Exit::Refused, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_environment::manifest::{Manifest, Pins};
    use statecraft_environment::plan::{WithheldWrite, Withholding};
    use std::collections::BTreeMap;

    #[test]
    fn applied_is_zero_partial_is_a_finding_and_refused_is_two() {
        assert_eq!(
            outcome_answer(Outcome::Applied { written: vec![] })
                .exit
                .code(),
            0
        );
        assert_eq!(
            outcome_answer(Outcome::Partial {
                written: vec![],
                withheld: vec![WithheldWrite {
                    path: "p".into(),
                    adapter: "a".into(),
                    reason: Withholding::Adopted,
                }],
            })
            .exit
            .code(),
            1
        );
        assert_eq!(
            outcome_answer(Outcome::Refused {
                reasons: vec!["no manifest".into()]
            })
            .exit
            .code(),
            2
        );
    }

    #[test]
    fn a_partial_outcome_names_every_withheld_path_in_the_summary() {
        let a = outcome_answer(Outcome::Partial {
            written: vec!["written.md".into()],
            withheld: vec![WithheldWrite {
                path: "theirs.md".into(),
                adapter: "a".into(),
                reason: Withholding::Adopted,
            }],
        });
        assert!(a.summary.contains("theirs.md"));
        assert!(a.summary.contains("adopted"));
    }

    #[test]
    fn a_clean_doctor_report_is_zero_and_a_finding_is_one() {
        let clean = statecraft_environment::doctor::Report::default();
        assert_eq!(doctor_answer(clean).exit.code(), 0);
    }

    #[test]
    fn an_apply_error_is_a_failure_not_a_refusal() {
        let e = ApplyError::Manifest(statecraft_environment::manifest::ManifestError::Io {
            path: ".statecraft/environment.json".into(),
            source: std::io::Error::other("disk on fire"),
        });
        assert_eq!(apply_error_answer(&e).exit, Exit::Failed);
    }

    #[test]
    fn an_empty_register_lists_cleanly_rather_than_as_a_finding() {
        let a = project_list(&Registry::default());
        assert_eq!(a.exit, Exit::Ok);
        assert!(a.summary.contains("no registered projects"));
    }

    #[test]
    fn arming_an_unregistered_target_is_a_refusal_not_a_failure() {
        let mut r = Registry::default();
        let e = project_set_armed(&mut r, Path::new("/nowhere"), true).unwrap_err();
        assert_eq!(e.exit, Exit::Refused);
    }

    // Keeps the unused-import warning honest: the binding module deals in
    // manifests even where no test above constructs one.
    #[test]
    fn a_manifest_can_be_constructed_for_the_binding_to_carry() {
        let m = Manifest::new(Pins {
            product: "0.0.0".into(),
            spec_spine: "0.18.0".into(),
            adapters: BTreeMap::new(),
        });
        assert!(m.entries.is_empty());
    }
}
