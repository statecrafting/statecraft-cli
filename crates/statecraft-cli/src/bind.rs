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
use crate::render::{Answer, ErrorKind};
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
        // Spec 002 section 3.23's translation of `spec-spine check`: an absent
        // binary or a missing verb is a refusal (2), and a read the producer
        // did not perform is a failure (4). Either way nothing is recorded.
        Err(e @ RegistryError::CorpusCheckUnavailable { .. }) => {
            Err(Answer::new(e.to_string(), Exit::Refused, e.to_string()))
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
            Err(Answer::new(e.to_string(), Exit::Refused, e.to_string())
                .with_kind(ErrorKind::NotFound))
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
    /// Who holds the path, where it was withheld because someone does: `user`,
    /// a package identity, or `not-recorded`. Spec 002 section 3.21 part 1: a
    /// `foreign` finding names an owner, not only a path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

impl WithheldView {
    fn of(w: &statecraft_environment::plan::WithheldWrite) -> Self {
        Self {
            path: w.path.clone(),
            adapter: w.adapter.clone(),
            reason: w.reason.describe(),
            owner: w.reason.claimant().map(|c| c.owner()),
        }
    }
}

impl OutcomeView {
    fn of(outcome: &Outcome) -> Self {
        match outcome {
            Outcome::Applied { written } => OutcomeView::Applied {
                written: written.clone(),
            },
            Outcome::Partial { written, withheld } => OutcomeView::Partial {
                written: written.clone(),
                withheld: withheld.iter().map(WithheldView::of).collect(),
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
    /// Information that is not a finding (spec 002 section 5, 2026-09-24,
    /// provenance): an unpinned project, a producer recorded before
    /// provenance, what an authored input's seed comparison means.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
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
            notes: report.notes.clone(),
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

/// An apply or upgrade given the operator's per-path consents, as the JSON
/// contract carries it: the outcome, plus what became of every named path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentedView {
    /// The outcome, with the fields [`OutcomeView`] carries.
    #[serde(flatten)]
    pub outcome: OutcomeView,
    /// Every path named with `--replace`, and what became of it. Empty when
    /// nothing was named.
    pub named: Vec<statecraft_environment::replace::Named>,
    /// Staged files an interrupted earlier replacement left, removed before
    /// this one staged anything.
    pub swept: Vec<String>,
}

/// Map an apply that carried per-path consents (spec 002 section 3.4).
///
/// The exit is the outcome's: a stale plan or a path this product may not
/// replace is a refusal (2) and nothing was written; a path already holding
/// the replacement is not a finding, so a repeated request that changes nothing
/// exits 0.
pub fn consented_answer(
    consented: statecraft_environment::apply::Consented,
) -> Answer<ConsentedView> {
    let base = outcome_answer(consented.outcome.clone());
    let mut summary = base.summary.clone();
    if !consented.outcome.refused() {
        for n in &consented.named {
            let line = match n {
                statecraft_environment::replace::Named::Replace(r) => {
                    format!("replaced {} ({} to {})", r.path, r.found, r.replacement)
                }
                other => other.describe(),
            };
            summary.push('\n');
            summary.push_str(&line);
        }
        for s in &consented.swept {
            summary.push_str(&format!("\nswept {s}, left by an interrupted replacement"));
        }
    }
    Answer::new(
        ConsentedView {
            outcome: OutcomeView::of(&consented.outcome),
            named: consented.named,
            swept: consented.swept,
        },
        base.exit,
        summary,
    )
}

/// A removal, as the JSON contract carries it: the outcome, plus notes that
/// are not findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalView {
    /// The outcome, with the fields [`OutcomeView`] carries.
    #[serde(flatten)]
    pub outcome: OutcomeView,
    /// Observations that change no exit: a bridge line with no record, which
    /// is not this product's, and a record dropped because its line was
    /// already taken back.
    pub notes: Vec<String>,
}

/// Map a removal (spec 002 sections 3.6 and 3.13 rule 4). The exit is the
/// outcome's; a note never changes it.
pub fn removal_answer(removal: statecraft_environment::apply::Removal) -> Answer<RemovalView> {
    let base = outcome_answer(removal.outcome.clone());
    let mut summary = base.summary.clone();
    for n in &removal.notes {
        summary.push_str(&format!("\nnote: {n}"));
    }
    Answer::new(
        RemovalView {
            outcome: OutcomeView::of(&removal.outcome),
            notes: removal.notes,
        },
        base.exit,
        summary,
    )
}

/// The `--replace` arguments of an environment verb, parsed strictly.
///
/// `env plan` takes `--replace <file>`, and `env apply` and `env upgrade` take
/// `--replace <file>=<plan-id>`, repeatable, one per path. Nothing is inferred:
/// a path is replaced only when it is named here. Any other argument after the
/// target, a value that is itself an option, a consent given to `env plan` and
/// a path given to `env apply` without an identity are each a usage error
/// (`Err`). Whether the verb takes `--replace` at all is the caller's check.
pub fn replace_arguments(
    consenting: bool,
    rest: &[String],
) -> Result<Vec<statecraft_environment::replace::Consent>, String> {
    let is_id = |id: &str| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit());
    let mut out = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] != "--replace" {
            return Err(format!(
                "unexpected argument `{}`; the only option after the target is --replace",
                rest[i]
            ));
        }
        let Some(value) = rest.get(i + 1) else {
            return Err("--replace needs a path".to_string());
        };
        if value.is_empty() || value.starts_with('-') {
            return Err(format!("--replace needs a path, not `{value}`"));
        }
        if consenting {
            let Some((path, id)) = value.rsplit_once('=') else {
                return Err(format!(
                    "--replace {value}: give <file>=<plan-id>, the identity `env plan --replace {value}` reported"
                ));
            };
            if path.is_empty() || !is_id(id) {
                return Err(format!(
                    "--replace {value}: `{id}` is not a plan identity (64 hexadecimal digits) after a path"
                ));
            }
            out.push(statecraft_environment::replace::Consent {
                path: path.to_string(),
                plan_id: id.to_ascii_lowercase(),
            });
        } else {
            if value.rsplit_once('=').is_some_and(|(_, id)| is_id(id)) {
                return Err(format!(
                    "--replace {value}: env plan takes the path alone; the identity is what it reports"
                ));
            }
            out.push(statecraft_environment::replace::Consent {
                path: value.clone(),
                plan_id: String::new(),
            });
        }
        i += 2;
    }
    Ok(out)
}

/// An error from the environment library, mapped.
///
/// Every one of these is a failure: an i/o error or an unreadable manifest is
/// not something the operator asked for and not a precondition the operation
/// checked, which is exactly what distinguishes 4 from 2.
pub fn apply_error_answer(e: &ApplyError) -> Answer<String> {
    use statecraft_environment::manifest::ManifestError;
    // Another writer held the manifest lock past the wait, or changed the
    // manifest since it was read: a precondition, and nothing was written,
    // so a refusal (2) under section 3.3, not a failure.
    let exit = match e {
        ApplyError::Manifest(ManifestError::Busy { .. } | ManifestError::Changed { .. }) => {
            Exit::Refused
        }
        _ => Exit::Failed,
    };
    Answer::new(e.to_string(), exit, e.to_string())
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
    } else if report.notes.is_empty() {
        "no findings".to_string()
    } else {
        format!("no findings\n{}", report.render())
    };
    // The exit code the report itself implies, and the one this command uses,
    // are the same number by construction rather than by coincidence.
    debug_assert_eq!(exit.code(), report.exit_code());
    Answer::new(ReportView::of(&report), exit, summary)
}

/// What `doctor --remote` was asked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteAsk {
    /// The commit the CI and review results are asked about; absent, `HEAD`.
    pub head: Option<String>,
}

/// Split `--remote [--head <sha>]` out of `doctor`'s arguments. `--head`
/// without `--remote`, a repeated flag, or a head that is not a hexadecimal
/// commit id is a usage error.
pub fn remote_arguments(rest: &[String]) -> Result<(Option<RemoteAsk>, Vec<String>), String> {
    let mut remote = false;
    let mut head: Option<String> = None;
    let mut others = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--remote" if !remote => remote = true,
            "--head" if head.is_none() => {
                let Some(value) = rest.get(i + 1) else {
                    return Err("--head needs a commit id".to_string());
                };
                let ok =
                    (7..=64).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_hexdigit());
                if !ok {
                    return Err(format!("--head `{value}` is not a commit id"));
                }
                head = Some(value.to_ascii_lowercase());
                i += 1;
            }
            "--remote" | "--head" => return Err(format!("{} given twice", rest[i])),
            _ => others.push(rest[i].clone()),
        }
        i += 1;
    }
    if head.is_some() && !remote {
        return Err("--head is asked with --remote".to_string());
    }
    Ok((remote.then_some(RemoteAsk { head }), others))
}

/// `doctor --remote`: the diagnostic, and the setup profile's six results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorRemoteView {
    /// The local diagnostic, as `doctor` reports it.
    pub report: ReportView,
    /// The six results, reported separately.
    pub setup: statecraft_home::setup::Results,
    /// Whether all six are satisfied.
    pub setup_complete: bool,
}

/// `doctor --remote`. The exit is the diagnostic's: a result that is not
/// satisfied is reported, and is not a finding about the local tree.
pub fn doctor_remote_answer(
    report: Report,
    results: statecraft_home::setup::Results,
) -> Answer<DoctorRemoteView> {
    let exit = if report.has_findings() {
        Exit::Finding
    } else {
        Exit::Ok
    };
    let mut summary = if report.has_findings() {
        report.render()
    } else {
        "no findings\n".to_string()
    };
    for (name, outcome) in statecraft_home::setup::results_rows(&results) {
        summary.push_str(&format!(
            "result {name:<22} {} ({})\n",
            outcome.state.word(),
            outcome.detail
        ));
    }
    let view = DoctorRemoteView {
        report: ReportView::of(&report),
        setup_complete: results.complete(),
        setup: results,
    };
    Answer::new(view, exit, summary.trim_end().to_string())
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
    /// The paths the operator named with `--replace`, each replaceable, with
    /// its plan identity, or already satisfied (spec 002 section 3.4). A named
    /// path that is neither is in `refusals`. Empty when nothing was named.
    pub named: Vec<statecraft_environment::replace::Named>,
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
            withheld: plan.withheld.iter().map(WithheldView::of).collect(),
            refusals: plan.refusals.iter().map(|r| r.describe()).collect(),
            named: plan.named.clone(),
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
    Answer::new(detail.clone(), Exit::Refused, detail).with_kind(ErrorKind::NotFound)
}

/// The refusal a path gets when two registrations name its directory.
///
/// Spec 003 section 3.1.4 rule 7 is one lock per repository, and each stored
/// root keys its own lock, chain and journal. Choosing one of the two would
/// leave the other spelling a second lock for the same repository, so both are
/// refused until one registration names it.
pub fn same_directory_answer(path: &Path, roots: &[std::path::PathBuf]) -> Answer<String> {
    let listed: Vec<String> = roots.iter().map(|r| r.display().to_string()).collect();
    let detail = format!(
        "{} names a directory registered under more than one path ({}); each would key its own \
         lock, run record and override journal for one repository, so nothing was read or written",
        path.display(),
        listed.join(", ")
    );
    Answer::new(detail.clone(), Exit::Refused, detail)
}

/// The refusal an unarmed target gets from a verb that would drive it.
///
/// Spec 002 section 3.1: a repository is armed separately from being
/// registered, and arming is what consents to being driven. Registration makes
/// a target visible; it does not make it drivable. So a verb that would drive
/// an unarmed target has an unmet precondition, which spec 006 section 3.3
/// makes a refusal (2) and not a finding.
///
/// The mapping is the only judgement here, exactly as it is for
/// [`unregistered_answer`]: that consent is required at all is spec 002's, and
/// [`statecraft_environment::registry::Registration::armed`] is where it is
/// recorded. This says nothing about qualification, which is the registration's
/// other and independent condition.
pub fn unarmed_answer(path: &Path) -> Answer<String> {
    let detail = format!(
        "{} is registered but not armed; `project arm {}` consents to it being driven",
        path.display(),
        path.display()
    );
    Answer::new(detail.clone(), Exit::Refused, detail)
}

/// A managed run refused under spec 002 section 3.25, before any attempt.
pub fn harness_refused_answer(path: &Path, reason: &str) -> Answer<String> {
    let detail = format!(
        "{} is not driven: {reason}. `harness show {}` inspects it and `harness upgrade {}` is \
         the explicit act that changes the requirement",
        path.display(),
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

    // A writer that timed out on the manifest lock wrote nothing: refused (2),
    // not failed (4). Likewise a manifest changed since it was read.
    #[test]
    fn a_writer_that_could_not_take_the_lock_is_refused_not_failed() {
        use statecraft_environment::manifest::ManifestError;
        let busy = ApplyError::Manifest(ManifestError::Busy { root: "/r".into() });
        assert_eq!(apply_error_answer(&busy).exit.code(), 2);
        let changed = ApplyError::Manifest(ManifestError::Changed {
            path: "p".into(),
            expected: "a".into(),
            found: "b".into(),
        });
        assert_eq!(apply_error_answer(&changed).exit.code(), 2);
        let io = ApplyError::Io {
            path: "p".into(),
            source: std::io::Error::other("x"),
        };
        assert_eq!(apply_error_answer(&io).exit.code(), 4);
    }

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
            producer: None,
        });
        assert!(m.entries.is_empty());
    }
}
