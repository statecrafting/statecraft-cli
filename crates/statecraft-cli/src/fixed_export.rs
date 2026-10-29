//! Spec 022's command bindings (section 3.11), through its additive edge on
//! this crate.
//!
//! Every operation is a library's: preparation and custody are
//! `statecraft-run`'s, selection, admission and the fake are
//! `statecraft-adapter`'s, the judgment is `statecraft-acceptance`'s. This
//! module joins them for one attempt and renders their values.
//!
//! - [`prepare`] refuses from the subjects alone, then writes the spec 003
//!   intent with the manifest and task, then the workflow's `prepared`
//!   version. Repeating it with the same inputs reports the existing record.
//! - [`execute`] selects the adapter the workflow's policy names, admits the
//!   request before anything runs, runs the pilot's fake inside a denial
//!   boundary, records every output, and concludes the attempt.
//! - [`submit_review`] and [`submit_correction`] append spec 021 records
//!   only for a reviewer the trusted-base policy grants, never for the
//!   result's provider or adapter.
//! - [`accept`] judges the workflow beside spec 021's judgment and appends
//!   the result; `run show` renders [`WorkflowView`] without recomputing it.
//!
//! No binding applies an artifact or publishes, and none is a command line
//! verb: the only adapter that declares the profile is the pilot's fake
//! (section 5, 2026-10-28).

use std::path::Path;

use serde::Serialize;

use statecraft_acceptance::documentation_acceptance::{self as da, Judgment};
use statecraft_acceptance::fixed_export::{self as judge, WorkflowJudgment};
use statecraft_adapter::fixed_export::{self as fx, Declaration, Denial, Step};
use statecraft_environment::time::{Clock, rfc3339_utc};
use statecraft_run::context::{self, Manifest};
use statecraft_run::documentation::review::{self, Correction, Review};
use statecraft_run::documentation::workflow::{self, AdapterRef, Workflow, WorkflowState};
use statecraft_run::documentation::{
    self as doc, Binding, DocumentationTask, Projection, ResultProjection, Skill,
};
use statecraft_run::record::Chain;
use statecraft_run::session::{self, IntentDetail, Session};

fn open(home: &Path, root: &Path) -> Result<Chain, String> {
    Chain::open(home, root)
        .map(|(c, _)| c)
        .map_err(|e| e.to_string())
}

/// What [`prepare`] did.
#[derive(Debug)]
pub struct Prepared {
    /// The workflow's `prepared` version, new or existing.
    pub workflow: Workflow,
    /// The attempt's session, when this call began it; `None` when the same
    /// inputs were already prepared.
    pub session: Option<Session>,
}

/// Section 3.2: prepare `task`'s attempt from fixed exports, or refuse
/// before anything is written.
#[allow(clippy::too_many_arguments)]
pub fn prepare(
    root: &Path,
    home: &Path,
    base: &str,
    manifest: &Manifest,
    task: &DocumentationTask,
    skill: Option<&[u8]>,
    adapter: &AdapterRef,
    clock: &dyn Clock,
) -> Result<Prepared, String> {
    let project = root.display().to_string();
    let at = rfc3339_utc(clock.now_unix());
    let mut chain = open(home, root)?;
    if let Some(found) = workflow::current(&chain, &task.run_id, task.attempt) {
        // An unreadable record binds nothing and is never read as absent.
        let candidate = found
            .map_err(|e| format!("workflow-unreadable: {e}"))?
            .candidate;
        return workflow::prepare(
            home, &mut chain, &project, &candidate, manifest, task, skill, adapter, &at,
        )
        .map(|p| Prepared {
            workflow: p.workflow,
            session: None,
        })
        .map_err(|e| e.to_string());
    }
    workflow::subjects(home, manifest, task, skill, adapter).map_err(|e| e.to_string())?;
    let session = session::begin_with(
        &mut chain,
        root,
        &task.run_id,
        base,
        clock,
        &IntentDetail {
            context_manifest: Some(manifest),
            documentation_task: Some(task),
            ..IntentDetail::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let prepared = workflow::prepare(
        home,
        &mut chain,
        &project,
        &session.base_commit,
        manifest,
        task,
        skill,
        adapter,
        &at,
    )
    .map_err(|e| e.to_string())?;
    Ok(Prepared {
        workflow: prepared.workflow,
        session: Some(session),
    })
}

fn recorded(
    chain: &Chain,
    run_id: &str,
    attempt: u32,
) -> Result<(Workflow, Manifest, DocumentationTask), String> {
    let w = workflow::current(chain, run_id, attempt)
        .ok_or("workflow-not-recorded")?
        .map_err(|e| format!("workflow-unreadable: {e}"))?;
    let manifest = context::recorded_manifest(chain, run_id, attempt)?
        .ok_or("the intent records no manifest")?;
    let task = doc::recorded_task(chain, run_id, attempt)?.ok_or("the intent records no task")?;
    Ok((w, manifest, task))
}

fn conclude(
    chain: &mut Chain,
    root: &Path,
    session: &Session,
    outcome: statecraft_run::attempt::Outcome,
    accounting: &statecraft_run::refusal::Accounting,
    detail: serde_json::Value,
    clock: &dyn Clock,
) -> Result<(), String> {
    session::conclude(chain, root, session, outcome, accounting, detail, clock)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Sections 3.3 and 3.4: run the prepared attempt's fake adapter under
/// `script`, record every output, and conclude the attempt. A selection or
/// admission refusal concludes it `refused` before anything runs.
pub fn execute(
    root: &Path,
    home: &Path,
    session: &Session,
    available: &[Declaration],
    script: &[Step],
    clock: &dyn Clock,
) -> Result<Workflow, String> {
    use statecraft_run::attempt::Outcome;
    use statecraft_run::refusal::{Accounting, RefusalEvent};

    let at = rfc3339_utc(clock.now_unix());
    let mut chain = open(home, root)?;
    let (prepared, manifest, task) = recorded(&chain, &session.run_id, session.attempt)?;
    // An attempt executes once, while it is live: a concluded attempt, or a
    // workflow past `prepared`, would otherwise get a second outcome.
    if attempt_of(&chain, &session.run_id, session.attempt)?
        .outcome
        .is_some()
    {
        return Err(format!(
            "workflow: run {} attempt {} is already concluded",
            session.run_id, session.attempt
        ));
    }
    if prepared.state() != Some(WorkflowState::Prepared) {
        return Err(format!(
            "workflow: run {} attempt {} is {}, not prepared",
            session.run_id,
            session.attempt,
            prepared.state().map(|s| word(&s)).unwrap_or_default()
        ));
    }
    let refuse = |chain: &mut Chain, why: String| {
        let mut accounting = Accounting::without_process();
        accounting.observe(RefusalEvent {
            guard: "fixed-export-admission".to_string(),
            detail: why.clone(),
        });
        conclude(
            chain,
            root,
            session,
            Outcome::Refused,
            &accounting,
            serde_json::json!({ "fixedExportRefusal": why }),
            clock,
        )?;
        Err(why)
    };
    let policy = match &prepared.subjects.adapter {
        Some(p) => p.clone(),
        None => return refuse(&mut chain, "the workflow names no adapter".into()),
    };
    let selected = match fx::select(&policy, available) {
        Ok(s) => s.clone(),
        Err(e) => return refuse(&mut chain, e.to_string()),
    };
    let read = |c: &str| context::read_retained(home, c).ok().flatten();
    let skill = match &task.skill {
        Skill::NotRequired(_) => None,
        Skill::Pinned(p) => read(&p.digest),
    };
    let context_bundle =
        statecraft_adapter::context::Bundle::assemble(&manifest, read).map_err(|e| e.to_string());
    let documentation_bundle =
        statecraft_adapter::documentation::Bundle::assemble(&task, skill.as_deref())
            .map_err(|e| e.to_string());
    let (context_bundle, documentation_bundle) = match (context_bundle, documentation_bundle) {
        (Ok(c), Ok(d)) => (c, d),
        (Err(e), _) | (_, Err(e)) => return refuse(&mut chain, e),
    };
    let request = statecraft_adapter::protocol::Request {
        workspace: session.workspace.path.clone(),
        base_commit: session.base_commit.clone(),
        prompt: task.objective.clone().into_bytes(),
        capabilities: Default::default(),
        deadline_seconds: 60,
        attempt: statecraft_adapter::protocol::AttemptIdentity {
            run_id: session.run_id.clone(),
            number: session.attempt,
        },
        context: Some(context_bundle),
        documentation: Some(documentation_bundle),
    };
    if let Err(e) = fx::admit(&request, &prepared, &manifest, &task, &selected) {
        return refuse(&mut chain, e.to_string());
    }
    let denial = Denial::new();
    let lines = fx::run_fake(&request, &task, script, &denial);
    let (adapter, provider) = fx::fake_parties();
    let binding = Binding::of(&task, adapter, provider);
    let w = fx::record(
        home, &mut chain, &prepared, &manifest, &task, &selected, &binding, &lines, &denial, &at,
    )
    .map_err(|e| e.to_string())?;
    let mut accounting = Accounting::without_process();
    for effect in denial.attempted() {
        accounting.observe(RefusalEvent {
            guard: fx::BOUNDARY_GUARD.to_string(),
            detail: effect.describe(),
        });
    }
    let terminal = w.subjects.raw_result.is_some();
    conclude(
        &mut chain,
        root,
        session,
        if terminal {
            Outcome::Completed
        } else {
            Outcome::Failed
        },
        &accounting,
        serde_json::json!({ "fixedExportWorkflow": w.workflow_digest }),
        clock,
    )?;
    Ok(w)
}

/// The trusted-base acceptance policy for the attempt, read at its base
/// commit, never at the working tree.
fn policy(root: &Path, base: &str) -> Result<da::Policy, String> {
    let base = crate::documentation::object_id(base)
        .ok_or_else(|| format!("the trusted base {base:?} is not an object id"))?;
    let bytes =
        crate::documentation::git_out(root, &["show", &format!("{base}:{}", da::POLICY_PATH)])
            .ok_or_else(|| format!("{} is absent at the trusted base {base}", da::POLICY_PATH))?;
    da::Policy::decode(&bytes).map(|(p, _)| p)
}

/// The adapter and provider names the attempt's bound result repeats. Fails
/// closed: a record is reviewed only once its result can be read, so the
/// exclusion of its provider and adapter is never skipped by an absence.
fn result_parties(
    chain: &Chain,
    home: &Path,
    task: &DocumentationTask,
) -> Result<Vec<String>, String> {
    let Projection::Recorded {
        result: ResultProjection::Recorded(r),
        ..
    } = doc::project(chain, &task.run_id, task.attempt)
    else {
        return Err("authority: the attempt has no single recorded result".into());
    };
    let bytes = context::read_retained(home, &r.raw.custody_digest)
        .map_err(|e| format!("authority: the result is unreadable: {e}"))?
        .ok_or("authority: the result's bytes are not retained")?;
    let result = doc::DocumentationResult::decode(&bytes, task)
        .map_err(|e| format!("authority: the result does not decode: {e}"))?;
    Ok(vec![result.adapter.name, result.provider.name])
}

/// Whether `t` is exactly `YYYY-MM-DDTHH:MM:SSZ`, the one form in which two
/// instants compare as strings. Any other spelling of a grant's `notAfter`
/// grants nothing.
fn utc_seconds(t: &str) -> bool {
    let b = t.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

/// Section 3.7: who may append a record, under the trusted-base policy.
#[allow(clippy::too_many_arguments)]
fn authorized(
    root: &Path,
    home: &Path,
    chain: &Chain,
    attempt: &statecraft_run::Attempt,
    task: &DocumentationTask,
    w: &Workflow,
    identity: &str,
    authority: Option<review::Authority>,
    now: &str,
) -> Result<(), String> {
    let mut parties = result_parties(chain, home, task)?;
    if let Some(a) = &w.subjects.adapter {
        parties.push(a.name.clone());
    }
    if parties.iter().any(|p| p == identity) {
        return Err(format!(
            "authority: {identity} is the result's provider or adapter"
        ));
    }
    let policy = policy(root, &attempt.base_commit).map_err(|e| format!("authority: {e}"))?;
    let granted = policy.reviewers.iter().any(|g| {
        g.identity == identity
            && authority.is_none_or(|a| a == g.authority)
            && g.not_after
                .as_deref()
                .is_none_or(|n| utc_seconds(n) && now <= n)
    });
    if granted {
        Ok(())
    } else {
        Err(format!(
            "authority: the trusted-base policy grants {identity} no such authority"
        ))
    }
}

fn attempt_of(chain: &Chain, run_id: &str, number: u32) -> Result<statecraft_run::Attempt, String> {
    session::runs(chain)
        .into_iter()
        .find(|r| r.id == run_id)
        .and_then(|r| r.attempts.into_iter().find(|a| a.number == number))
        .ok_or_else(|| format!("run {run_id} has no attempt {number}"))
}

/// Section 3.7: append an authorized reviewer's closed review, then the
/// workflow's `reviewed` version. Raw output and artifacts stay as they are.
pub fn submit_review(
    root: &Path,
    home: &Path,
    run_id: &str,
    attempt: u32,
    bytes: &[u8],
    clock: &dyn Clock,
) -> Result<Workflow, String> {
    let at = rfc3339_utc(clock.now_unix());
    let mut chain = open(home, root)?;
    let (w, _, task) = recorded(&chain, run_id, attempt)?;
    let r = Review::decode(bytes).map_err(|e| e.to_string())?;
    if r.subject.run_id != run_id || r.subject.attempt != attempt {
        return Err("binding: the review names another attempt".into());
    }
    if r.subject.task_digest != task.task_digest {
        return Err("binding: the review names another task".into());
    }
    if w.subjects.reviews.contains(&r.review_digest) {
        return Err(format!("duplicate: review {} is recorded", r.review_digest));
    }
    let a = attempt_of(&chain, run_id, attempt)?;
    authorized(
        root,
        home,
        &chain,
        &a,
        &task,
        &w,
        &r.reviewer.identity,
        Some(r.reviewer.authority),
        &at,
    )?;
    review::record_review(&mut chain, &r, &at).map_err(|e| e.to_string())?;
    workflow::advance(
        &mut chain,
        &w,
        WorkflowState::Reviewed,
        &format!("review by {}", r.reviewer.identity),
        &at,
        |w| w.subjects.reviews.push(r.review_digest.clone()),
    )
    .map_err(|e| e.to_string())
}

/// Section 3.8: keep a correction's new bytes, append an authorized
/// author's correction, then the workflow's `corrected` version. Nothing
/// earlier is overwritten.
pub fn submit_correction(
    root: &Path,
    home: &Path,
    run_id: &str,
    attempt: u32,
    bytes: &[u8],
    artifacts: &[&[u8]],
    clock: &dyn Clock,
) -> Result<Workflow, String> {
    let at = rfc3339_utc(clock.now_unix());
    let mut chain = open(home, root)?;
    let (w, _, task) = recorded(&chain, run_id, attempt)?;
    let c = Correction::decode(bytes).map_err(|e| e.to_string())?;
    if c.subject.run_id != run_id || c.subject.attempt != attempt {
        return Err("binding: the correction names another attempt".into());
    }
    if c.subject.task_digest != task.task_digest {
        return Err("binding: the correction names another task".into());
    }
    if w.subjects.corrections.contains(&c.correction_digest) {
        return Err(format!(
            "duplicate: correction {} is recorded",
            c.correction_digest
        ));
    }
    let a = attempt_of(&chain, run_id, attempt)?;
    authorized(root, home, &chain, &a, &task, &w, &c.author, None, &at)?;
    let mut kept = Vec::new();
    for bytes in artifacts {
        kept.push(context::retain(home, bytes).map_err(|e| e.to_string())?);
    }
    for changed in &c.changed_artifacts {
        if !kept.contains(&changed.digest) {
            return Err(format!(
                "not-retained: {} {} has no supplied bytes",
                changed.logical_path, changed.digest
            ));
        }
    }
    review::record_correction(&mut chain, &c, &at).map_err(|e| e.to_string())?;
    workflow::advance(
        &mut chain,
        &w,
        WorkflowState::Corrected,
        &c.reason,
        &at,
        |w| w.subjects.corrections.push(c.correction_digest.clone()),
    )
    .map_err(|e| e.to_string())
}

/// What `accept` shows under `fixedExportAcceptance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum AcceptanceView {
    /// The attempt has no fixed-export workflow.
    NotRecorded,
    /// The judgment could not be made or recorded.
    Unrecorded {
        /// Why.
        reason: String,
    },
    /// The judgment, recorded.
    Judged {
        /// The workflow judgment.
        judgment: Box<WorkflowJudgment>,
    },
}

/// Section 3.9: judge the attempt's workflow beside spec 021's judgment of
/// its current subject, and append both the judgment and the workflow's
/// `judged` version. An attempt with no workflow records nothing.
pub fn accept(
    root: &Path,
    home: &Path,
    run_id: &str,
    attempt: u32,
    documentation: Option<&Judgment>,
    clock: &dyn Clock,
) -> AcceptanceView {
    let Ok(mut chain) = open(home, root) else {
        return AcceptanceView::Unrecorded {
            reason: "the record could not be opened".into(),
        };
    };
    let Some(current) = workflow::current(&chain, run_id, attempt) else {
        return AcceptanceView::NotRecorded;
    };
    let Some(documentation) = documentation else {
        return AcceptanceView::Unrecorded {
            reason: "documentation-not-judged: spec 021's judgment was not recorded".into(),
        };
    };
    let task = match doc::recorded_task(&chain, run_id, attempt) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return AcceptanceView::Unrecorded {
                reason: "the intent records no task".into(),
            };
        }
        Err(e) => return AcceptanceView::Unrecorded { reason: e },
    };
    let retained = |d: &str| context::read_retained(home, d).ok().flatten();
    let judgment = judge::judge(
        current.as_ref().map_err(String::as_str),
        &task,
        documentation.clone(),
        &retained,
    );
    // A judgment is shown as `judged` only once it is recorded, and an
    // unreadable workflow has no version to record it against.
    let w = match current {
        Ok(w) => w,
        Err(e) => {
            return AcceptanceView::Unrecorded {
                reason: format!("workflow-unreadable: {e}"),
            };
        }
    };
    let now = rfc3339_utc(clock.now_unix());
    match judge::record(&mut chain, &w, &judgment, &now) {
        Ok(_) => AcceptanceView::Judged {
            judgment: Box::new(judgment),
        },
        Err(e) => AcceptanceView::Unrecorded {
            reason: e.to_string(),
        },
    }
}

/// The workflow as `run show` renders it, read from the record alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum WorkflowView {
    /// No workflow was recorded; every historical attempt renders this.
    NotRecorded,
    /// The latest version could not be read.
    Unreadable {
        /// Why.
        reason: String,
    },
    /// The latest version, and how many versions precede it.
    Recorded {
        /// The latest version.
        workflow: Box<Workflow>,
        /// How many versions the record holds.
        versions: usize,
    },
}

/// The attempt's workflow, for `run show`. Nothing is recomputed.
pub fn view(chain: &Chain, run_id: &str, attempt: u32) -> WorkflowView {
    let versions = workflow::versions(chain, run_id, attempt);
    let n = versions.len();
    match versions.into_iter().last() {
        None => WorkflowView::NotRecorded,
        Some(Err(reason)) => WorkflowView::Unreadable { reason },
        Some(Ok(w)) => WorkflowView::Recorded {
            workflow: Box::new(w),
            versions: n,
        },
    }
}

/// A value whose JSON form is one word, as that word.
fn word<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The human lines for one attempt's workflow; empty when none was recorded.
pub fn describe(attempt: u32, view: &WorkflowView) -> String {
    match view {
        WorkflowView::NotRecorded => String::new(),
        WorkflowView::Unreadable { reason } => {
            format!("attempt {attempt} fixed-export workflow unreadable: {reason}\n")
        }
        WorkflowView::Recorded { workflow, versions } => {
            let mut out = format!(
                "attempt {attempt} {} {} {}, {versions} version(s)\n",
                workflow.profile,
                workflow
                    .state()
                    .map(|s| word(&s))
                    .unwrap_or_else(|| "not-recorded".into()),
                workflow.workflow_digest,
            );
            for t in &workflow.transitions {
                out.push_str(&format!("  {} {}: {}\n", t.at, word(&t.state), t.reason));
            }
            for f in &workflow.findings {
                out.push_str(&format!("  finding {f}\n"));
            }
            for a in &workflow.subjects.acceptances {
                out.push_str(&format!("  judgment {a}\n"));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::utc_seconds;

    #[test]
    fn only_the_z_form_of_an_instant_is_compared() {
        assert!(utc_seconds("2026-12-31T00:00:00Z"));
        for bad in [
            "2026-12-31T00:00:00+00:00",
            "2026-12-31T00:00:00.000Z",
            "2026-12-31",
            "",
            "9999-99-99T99:99:99z",
        ] {
            assert!(!utc_seconds(bad), "{bad}");
        }
    }
}
