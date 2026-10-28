//! Callable application operations (spec 040).
//!
//! Each operation performs its own authoritative admission (registration
//! resolution, locks, journal and chain reads) and returns a
//! [`Response`]: the same `render::Answer` the verb renders, with its value
//! erased to JSON, which is what the family envelope serializes anyway
//! (`Answer::envelope` converts the value and renders it in canonical key
//! order), so erasing changes no byte. Operations never print, choose a
//! format, parse argv or exit. The binary keeps those, and its adapters call
//! these functions and render what they return.
//!
//! A typed caller supplies operands and the home and registry, never a
//! pre-resolved registration: the root a record is keyed by is resolved here.
//! Operand checks the verb made after resolving the target are made after
//! resolving it here too, so the order of refusals is the verb's.

use serde_json::Value;
use statecraft_adapter::witness::posture::Requirement;
use statecraft_environment::registry::Registry;
use statecraft_environment::time::Clock;
use statecraft_run::record::Chain;
use std::path::{Path, PathBuf};

use crate::exit::Exit;
use crate::render::Answer;
use crate::{adapters, bind, slice};

/// What an operation answers: a family-envelope answer.
pub type Response = Answer<Value>;

/// An operand an operation cannot use: the binary renders it as usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage(pub String);

/// What every operation is given: the product home, the register it resolves
/// targets against, and the clock any record it writes is stamped with.
pub struct Context<'a> {
    /// The product home.
    pub home: &'a Path,
    /// The register.
    pub registry: &'a Registry,
    /// The clock.
    pub clock: &'a dyn Clock,
}

/// A failure no request asked for: a product store that could not be read or
/// written. Its kind is the exit's default, `io`.
pub fn failed(detail: &str) -> Response {
    Answer::new(detail.to_string(), Exit::Failed, detail).erase()
}

/// The registration a typed path names, as the root it was stored under
/// (spec 003 section 3.1.4 rule 7): every record is keyed by the stored root.
pub fn registered(registry: &Registry, typed: &Path) -> Result<PathBuf, Response> {
    use statecraft_run::repository::{Unresolved, resolve};
    match resolve(registry, typed) {
        Ok(registration) => Ok(registration.root.clone()),
        Err(Unresolved::NotRegistered) => Err(bind::unregistered_answer(typed).erase()),
        Err(Unresolved::SameDirectory(roots)) => {
            Err(bind::same_directory_answer(typed, &roots).erase())
        }
    }
}

/// As [`registered`], for an operation that does not require registration:
/// an unregistered path is used as typed, a registered one as stored.
pub fn stored_or_typed(registry: &Registry, typed: &Path) -> Result<PathBuf, Response> {
    use statecraft_run::repository::{Unresolved, resolve};
    match resolve(registry, typed) {
        Ok(registration) => Ok(registration.root.clone()),
        Err(Unresolved::NotRegistered) => Ok(typed.to_path_buf()),
        Err(Unresolved::SameDirectory(roots)) => {
            Err(bind::same_directory_answer(typed, &roots).erase())
        }
    }
}

/// The clock a verb adapter gives its operation: the system clock.
///
/// In a debug build, `STATECRAFT_TEST_CLOCK` (whole seconds since the Unix
/// epoch) fixes it instead, so spec 040's differential tests can control the
/// one input a verb and its operation would otherwise disagree on. A release
/// build never reads the variable.
pub fn process_clock() -> Box<dyn Clock> {
    #[cfg(debug_assertions)]
    if let Some(at) = std::env::var("STATECRAFT_TEST_CLOCK")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
    {
        return Box::new(statecraft_environment::time::FixedClock(at));
    }
    Box::new(statecraft_environment::time::SystemClock)
}

fn now(clock: &dyn Clock) -> String {
    statecraft_environment::time::rfc3339_utc(clock.now_unix())
}

// ------------------------------------------------------------------ accept

/// `accept <path> <run-id>`: judge a run's last attempt (spec 005).
///
/// Registration is resolved first; a missing run id is usage only after it,
/// as the verb orders them.
pub fn accept(ctx: &Context<'_>, path: &Path, run_id: Option<&str>) -> Result<Response, Usage> {
    accept_with_context(ctx, path, run_id, None)
}

/// [`accept`], with the spec 019 `accept --context` file: the current
/// checkouts, the claimed citations and the evidence admissions the context
/// checks read. Without one, every check that needs a checkout reports its
/// absence and none is guessed.
pub fn accept_with_context(
    ctx: &Context<'_>,
    path: &Path,
    run_id: Option<&str>,
    context: Option<&Path>,
) -> Result<Response, Usage> {
    let root = match registered(ctx.registry, path) {
        Ok(root) => root,
        Err(answer) => return Ok(answer),
    };
    let Some(run_id) = run_id else {
        return Err(Usage(
            "usage: accept <path> <run-id> [--context <file>]".to_string(),
        ));
    };
    let supplied = match context.map(crate::context::read_accept_file) {
        None => crate::context::AcceptFile::default(),
        Some(Ok(f)) => f,
        Some(Err(e)) => return Err(Usage(format!("accept --context: {e}"))),
    };
    Ok(accept_registered(&root, ctx.home, run_id, &supplied))
}

fn accept_registered(
    root: &Path,
    home: &Path,
    run_id: &str,
    supplied: &crate::context::AcceptFile,
) -> Response {
    let (chain, _) = match Chain::open(home, root) {
        Ok(c) => c,
        Err(e) => return failed(&e.to_string()),
    };
    let Some(run) = statecraft_run::session::runs(&chain)
        .into_iter()
        .find(|r| r.id == run_id)
    else {
        return slice::no_such_run_answer(run_id).erase();
    };
    let Some(attempt) = run.attempts.last().cloned() else {
        return slice::no_such_run_answer(run_id).erase();
    };

    // Exactly one of spec 003's five outcomes is eligible, and the other four
    // are recorded reasons rather than blanks (spec 005 section 3.1.1).
    let Some(outcome) = attempt.outcome else {
        return slice::accept_answer(statecraft_acceptance::outcome::Acceptance::None {
            reason: statecraft_acceptance::judged::NoAcceptance::CandidateUnidentified {
                detail: format!(
                    "run {run_id} attempt {} is live, so nothing identifies stable candidate bytes",
                    attempt.number
                ),
            },
        })
        .erase();
    };
    if let Err(reason) = statecraft_acceptance::judged::eligibility(outcome) {
        let refusal_count = chain
            .entries()
            .iter()
            .rev()
            .find(|e| e.run_id == run_id && e.kind == statecraft_run::record::Kind::Accounting)
            .and_then(|e| e.detail.get("count"))
            .and_then(|v| v.as_u64())
            .map(|c| c as u32);
        return slice::accept_answer(statecraft_acceptance::outcome::Acceptance::NotAttempted {
            reason,
            refusal_count,
        })
        .erase();
    }

    // Spec 005 section 3.18: the contract the attempt was bound to, compared
    // with the producer's resolution now, before anything else is observed.
    let judge = match crate::judge::report_source(root) {
        Ok(judge) => judge,
        Err(e) => return slice::report_error_answer(&e).erase(),
    };
    let contract = statecraft_acceptance::contract::check(
        &chain.entries(),
        run_id,
        attempt.number,
        root,
        &judge,
    );
    if contract.word == statecraft_acceptance::contract::Word::Stale {
        return slice::contract_stale_answer(&contract).erase();
    }
    if contract.word.moved() {
        return slice::accept_answer_with(
            statecraft_acceptance::outcome::Acceptance::None {
                reason: statecraft_acceptance::judged::NoAcceptance::ContractMoved {
                    comparison: Box::new(contract.clone()),
                },
            },
            Some(contract),
        )
        .erase();
    }

    // Spec 014 section 3.5: where a policy requires wire evidence, anything
    // but an admitted artifact or a permitted absence is ineligible, with its
    // reason, before the suite runs. The attempt outcome is not rewritten.
    let wire = statecraft_acceptance::wire::eligibility(
        &chain,
        &statecraft_run::wire::Target {
            run_id: run_id.to_string(),
            attempt: attempt.number,
        },
    );
    if let Some(reason) = &wire.ineligible {
        return slice::accept_answer_with(
            statecraft_acceptance::outcome::Acceptance::None {
                reason: statecraft_acceptance::judged::NoAcceptance::WireEvidenceIneligible {
                    state: wire.state.to_string(),
                    evidence: reason.word().to_string(),
                },
            },
            Some(contract),
        )
        .erase();
    }

    // Spec 019 section 3.8: the context checks under the policy the intent
    // recorded. A required fact that does not hold is ineligible, with every
    // reason, before the suite runs; the attempt outcome is not rewritten.
    let judgment = crate::context::judge(root, home, &chain, run_id, attempt.number, supplied);
    // Spec 020 section 3.9: the documentation inputs, beside the verdict and
    // never part of it; documentation acceptance is `not-declared`.
    let mut documentation =
        statecraft_acceptance::documentation::inputs(&chain, home, run_id, attempt.number);
    // Spec 021 section 3.10: the independent judgment, recorded, beside the
    // verdict and never part of it.
    let documentation_acceptance =
        crate::documentation::accept(root, home, &chain, &attempt, run_id, supplied);
    if let (
        statecraft_acceptance::documentation::Inputs::Declared(d),
        crate::documentation::AcceptanceView::Judged { .. },
    ) = (&mut documentation, &documentation_acceptance)
    {
        d.acceptance = statecraft_acceptance::documentation::Acceptance::Judged;
    }
    // Spec 022 section 3.9: the workflow's judgment, beside spec 021's and
    // never better than it.
    let fixed_export_acceptance = crate::fixed_export::accept(
        root,
        home,
        run_id,
        attempt.number,
        match &documentation_acceptance {
            crate::documentation::AcceptanceView::Judged { judgment } => Some(judgment),
            _ => None,
        },
        &statecraft_environment::time::SystemClock,
    );
    if let Some(reasons) = judgment.ineligible() {
        let mut answer = slice::accept_answer_with(
            statecraft_acceptance::outcome::Acceptance::None {
                reason: statecraft_acceptance::judged::NoAcceptance::ContextIneligible { reasons },
            },
            Some(contract),
        );
        answer.value.context_manifest = judgment;
        answer.value.documentation = documentation;
        answer.value.documentation_acceptance = documentation_acceptance;
        answer.value.fixed_export_acceptance = fixed_export_acceptance;
        return answer.erase();
    }

    let workspace = statecraft_run::workspace::workspace_path(root, run_id);
    let context = crate::accept::Context {
        repository: root.display().to_string(),
        spec_spine_version: adapters::observed(root)
            .spec_spine
            .unwrap_or_else(|| "not-recorded".to_string()),
        adapter_version: statecraft_adapter_claude_code::manifest().version,
        attempt: format!("{run_id}/{}", attempt.number),
        spec_id: run_id.to_string(),
    };
    let (acceptance, _suite, _freshness, boundary) = crate::accept::judge_with_diagnostic(
        root,
        home,
        &workspace,
        &attempt.base_commit,
        // The attempt concluded `completed`, which spec 003 section 3.8 makes
        // impossible when the base moved: a moved base is `interrupted`.
        true,
        &context,
    );
    let mut answer = slice::accept_answer_with_boundary(acceptance, Some(contract), boundary);
    answer.value.context_manifest = judgment;
    answer.value.documentation = documentation;
    answer.value.documentation_acceptance = documentation_acceptance;
    answer.value.fixed_export_acceptance = fixed_export_acceptance;
    answer.erase()
}

// -------------------------------------------------------- override recover

/// A recovery choice and the state it is made against (spec 003 section
/// 3.1.5 rule 5).
pub struct RecoverChoice<'a> {
    /// The choice.
    pub choice: statecraft_run::overrides::Choice,
    /// The journal digest it was made against, or `absent`.
    pub journal: &'a str,
    /// The state-authority digest, or `absent`.
    pub authority: &'a str,
    /// Who chose.
    pub operator: &'a str,
    /// Why.
    pub reason: &'a str,
}

/// `override recover <path> [<choice> ...]`: report both files, or record the
/// operator's choice against the state the digests name.
pub fn override_recover(
    ctx: &Context<'_>,
    path: &Path,
    choice: Option<RecoverChoice<'_>>,
) -> Response {
    let root = match registered(ctx.registry, path) {
        Ok(root) => root,
        Err(answer) => return answer,
    };
    let Some(choice) = choice else {
        // The report takes the lock when it is free, so what it shows is not
        // a write caught half way; while another process holds it, an
        // interrupted state is a write in progress.
        let held = match statecraft_run::lock::try_acquire(ctx.home, &root) {
            Ok(held) => Some(held),
            Err(statecraft_run::lock::LockError::Busy { .. }) => None,
            Err(e) => return failed(&e.to_string()),
        };
        return match statecraft_run::overrides::inspect(ctx.home, &root) {
            Ok(found) => {
                slice::recover_report_answer(&root.display().to_string(), &found, held.is_none())
                    .erase()
            }
            Err(e) => slice::journal_error_answer(&e).erase(),
        };
    };
    let at = now(ctx.clock);
    slice::recover_answer(statecraft_run::overrides::recover(
        &statecraft_run::overrides::RecoverRequest {
            home: ctx.home,
            target: &root,
            choice: choice.choice,
            journal: choice.journal,
            authority: choice.authority,
            operator: choice.operator,
            reason: choice.reason,
            at: &at,
        },
    ))
    .erase()
}

// ----------------------------------------------------------- run reconcile

/// The operands of `run reconcile`, as typed (spec 003 section 3.6.1).
pub struct ReconcileOperands<'a> {
    /// The target path, as typed.
    pub path: &'a Path,
    /// The run.
    pub run_id: &'a str,
    /// The attempt number, as typed.
    pub attempt: &'a str,
    /// `confirmed`, `absent` or `unknown`.
    pub finding: &'a str,
    /// The launch state the operator inspected.
    pub inspected: &'a str,
    /// Who reconciled.
    pub operator: &'a str,
    /// Why.
    pub reason: &'a str,
    /// Evidence files, hashed as read and never copied.
    pub evidence: &'a [String],
}

/// `run reconcile`: read the launch records, never launch, and record the
/// operator's finding (spec 003 section 3.6.1).
pub fn run_reconcile(ctx: &Context<'_>, op: &ReconcileOperands<'_>) -> Result<Response, Usage> {
    use statecraft_run::reconcile::{EvidenceFile, LaunchState, Observation, Request};
    use statecraft_run::recovery::Verdict;
    let usage = || {
        Usage(
            "usage: run reconcile <path> <run-id> <attempt> <confirmed|absent|unknown> \
             <launch-state> <operator> <reason...> [--evidence <file>]..."
                .to_string(),
        )
    };
    // Filed under the stored root, like every other verb that keys a record,
    // so another spelling of a registered path reads the same chain.
    let root = match stored_or_typed(ctx.registry, op.path) {
        Ok(root) => root,
        Err(answer) => return Ok(answer),
    };
    let run_id = op.run_id;
    let Ok(attempt) = op.attempt.parse::<u32>() else {
        return Err(usage());
    };
    let finding = match op.finding {
        "confirmed" => Verdict::Confirmed,
        "absent" => Verdict::Absent,
        "unknown" => Verdict::Unknown,
        _ => return Err(usage()),
    };
    let inspected = op.inspected;
    if LaunchState::from_word(inspected).is_none() {
        return Err(usage());
    }
    if ctx.registry.get(&root).is_none() {
        return Ok(bind::unregistered_answer(&root).erase());
    }
    let home = ctx.home;
    let mut evidence = Vec::new();
    for p in op.evidence {
        // Hashed as it is read, never held whole: its content is not copied and
        // not read for a decision (spec 003 section 3.6.1 rule 2).
        match std::fs::File::open(p).and_then(statecraft_environment::digest::digest_reader) {
            Ok((sha256, bytes)) => evidence.push(EvidenceFile {
                path: p.clone(),
                bytes,
                sha256,
            }),
            Err(e) => {
                return Ok(slice::reconcile_refused_answer(&format!(
                    "the evidence file {p} could not be read: {e}; nothing was written"
                ))
                .erase());
            }
        }
    }

    // Rule 1: a run still supervising holds the lock.
    let _held = match statecraft_run::lock::try_acquire(home, &root) {
        Ok(held) => held,
        Err(e @ statecraft_run::lock::LockError::Busy { .. }) => {
            return Ok(slice::lock_busy_answer(&e.to_string()).erase());
        }
        Err(e) => return Ok(failed(&e.to_string())),
    };
    let (mut chain, _) = match Chain::open(home, &root) {
        Ok(c) => c,
        Err(e) => return Ok(failed(&e.to_string())),
    };

    // Rule 2: read the launch records; never launch, signal or replay.
    // Rule 3: the attempt's gate log is read whatever the manifest says, and a
    // log that exists and cannot be read fails rather than reading as
    // "nothing released".
    //
    // Spec 002 section 3.37 and 003 section 3.6.1's note: the records are in
    // the product home, or inside the target where they were written before
    // that section, never read from both; the gate log is the supervisor's
    // copy, or the exchange directory's for an attempt it did not conclude,
    // and it is child-attested either way.
    let identity = statecraft_home::launch::AttemptIdentity {
        run_id: run_id.to_string(),
        attempt,
    };
    let places = statecraft_home::launch::Places::of(home, &root);
    let (gate_log_path, gate) =
        match statecraft_home::launch::read_gate_log_checked(&places, &identity) {
            Ok((paths, gate)) => (
                gate.as_ref().map_or_else(
                    || {
                        match paths.placement {
                            statecraft_home::launch::Placement::Home => {
                                identity.gate_log_path(&places)
                            }
                            statecraft_home::launch::Placement::Target => {
                                paths.records.join(statecraft_home::launch::files::GATE_LOG)
                            }
                        }
                        .display()
                        .to_string()
                    },
                    |g| g.path.clone(),
                ),
                gate.map(|g| g.lines),
            ),
            Err(statecraft_home::launch::NotRead::NoSuchAttempt(why)) => {
                return Ok(
                    slice::reconcile_refused_answer(&format!("{why}; nothing was written")).erase(),
                );
            }
            Err(e) => return Ok(failed(&format!("{e}; nothing was written"))),
        };
    let gate_released_tool_call = gate
        .as_ref()
        .is_some_and(|g| g.iter().any(|w| w == "admitted"));
    let observation = match statecraft_environment::manifest::Manifest::read(&root) {
        Err(e) => return Ok(failed(&e.to_string())),
        Ok(None) => Observation {
            launch_state: LaunchState::Unrecorded,
            gate_released_tool_call,
            confirmed_pid: None,
            pid_exists: None,
            files: vec![gate_log_path],
        },
        Ok(Some(_)) => {
            let facts: Vec<statecraft_home::launch::AttemptFact> =
                statecraft_run::session::runs(&chain)
                    .into_iter()
                    .find(|r| r.id == run_id)
                    .map(|r| {
                        r.attempts
                            .iter()
                            .map(|a| statecraft_home::launch::AttemptFact {
                                number: a.number,
                                outcome: a.outcome.map(|o| o.word().to_string()),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            match statecraft_home::launch::inspect(&places, run_id, Some(attempt), &facts) {
                Ok(shown) => {
                    let pid = shown.launched.as_ref().map(|l| l.pid);
                    Observation {
                        launch_state: observed_launch_state(shown.verdict),
                        gate_released_tool_call,
                        confirmed_pid: pid,
                        pid_exists: pid.map(statecraft_run::reconcile::process_exists),
                        files: vec![
                            shown.intent_path.clone(),
                            shown.launched_path.clone(),
                            shown.admission_path.clone(),
                            shown.record_path.clone(),
                            gate_log_path,
                        ],
                    }
                }
                Err(statecraft_home::launch::NotRead::NoSuchAttempt(why)) => {
                    return Ok(slice::reconcile_refused_answer(&format!(
                        "{why}; nothing was written"
                    ))
                    .erase());
                }
                Err(e) => return Ok(failed(&e.to_string())),
            }
        }
    };
    let at = now(ctx.clock);
    let request = Request {
        run_id,
        attempt,
        finding,
        inspected,
        operator: op.operator,
        reason: op.reason,
        evidence,
        at: &at,
    };
    let audit = match statecraft_run::tamper::read_audit(home, &root) {
        Ok(found) => found
            .into_iter()
            .filter(|f| f.run_id == run_id && f.attempt == attempt)
            .collect(),
        Err(e) => return Ok(failed(&e)),
    };
    Ok(slice::reconcile_answer(
        statecraft_run::reconcile::reconcile(&mut chain, &request, observation),
        audit,
    )
    .erase())
}

/// Spec 002's launch verdict as the reconciliation's observed launch state,
/// word for word and with no fallback (spec 003 section 3.6.1 rule 3).
fn observed_launch_state(
    verdict: statecraft_home::launch::Verdict,
) -> statecraft_run::reconcile::LaunchState {
    use statecraft_home::launch::Verdict as V;
    use statecraft_run::reconcile::LaunchState as S;
    match verdict {
        V::NotLaunched => S::NotLaunched,
        V::LaunchUnknown => S::LaunchUnknown,
        V::OutcomeUnknown => S::OutcomeUnknown,
        V::SpawnFailed => S::SpawnFailed,
        V::Interrupted => S::Interrupted,
        V::Mismatched | V::NotAdmitted | V::Unverified | V::Qualified => {
            S::Completed(verdict.word().to_string())
        }
    }
}

// --------------------------------------------------------------------- run

/// `run <path> <spec-id>`: one attempt at one unit of work (spec 003, through
/// spec 006 section 3.11).
///
/// Admission is the verb's, in its order: registration, then arming (spec 002
/// section 3.1), then the repository lock, held through the attempt (spec 003
/// section 3.1.5 rule 8), then the corpus report, the policy and the override
/// journal read under that lock. A missing spec id is usage only after those,
/// as the verb orders it. An unregistered or unarmed target refuses before
/// any workspace, appended attempt or provider.
pub fn run(ctx: &Context<'_>, path: &Path, spec_id: Option<&str>) -> Result<Response, Usage> {
    run_from(ctx, path, spec_id, None, Requirement::DEFAULT, None)
}

/// [`run`], with witness capture requested or required (spec 013 section 7,
/// 2026-10-17): `run --witness` and `run --require-witness`; and with the
/// spec 019 task file `run --context` names, when it names one.
pub fn run_witnessed(
    ctx: &Context<'_>,
    path: &Path,
    spec_id: Option<&str>,
    witness: Requirement,
    context: Option<&Path>,
) -> Result<Response, Usage> {
    run_from(ctx, path, spec_id, None, witness, context)
}

/// Where an admitted spec 027 proposal enters the run path: the commit its
/// fresh observation read, and the proposal the intent names.
pub struct Origin<'a> {
    /// The commit the workspace is prepared at.
    pub commit: &'a str,
    /// The admitting proposal.
    pub proposal: &'a str,
}

/// [`run`], entered by an admitted spec 027 proposal (section 3.6 step 3):
/// the same admission (registration, arming, the repository lock, the
/// report, the policy and the journal, none of them bypassed or pre-decided),
/// with the workspace prepared at the observed commit and the proposal named
/// in the intent.
pub fn run_admitted_proposal(
    ctx: &Context<'_>,
    path: &Path,
    spec_id: &str,
    origin: &Origin<'_>,
) -> Response {
    match run_from(
        ctx,
        path,
        Some(spec_id),
        Some(origin),
        Requirement::DEFAULT,
        None,
    ) {
        Ok(answer) => answer,
        Err(Usage(detail)) => failed(&detail),
    }
}

fn run_from(
    ctx: &Context<'_>,
    path: &Path,
    spec_id: Option<&str>,
    origin: Option<&Origin<'_>>,
    witness: Requirement,
    context: Option<&Path>,
) -> Result<Response, Usage> {
    let root = match registered(ctx.registry, path) {
        Ok(root) => root,
        Err(answer) => return Ok(answer),
    };
    let registration = ctx.registry.get(&root).expect("resolved above");
    if !registration.armed {
        return Ok(bind::unarmed_answer(&root).erase());
    }
    let held = match statecraft_run::lock::try_acquire(ctx.home, &root) {
        Ok(held) => held,
        Err(e @ statecraft_run::lock::LockError::Busy { .. }) => {
            return Ok(slice::lock_busy_answer(&e.to_string()).erase());
        }
        Err(e) => return Ok(failed(&e.to_string())),
    };
    let report = match crate::judge::report_source(&root).and_then(|cli| {
        use statecraft_run::report::ReportSource;
        cli.corpus_report(&root)
    }) {
        Ok(report) => report,
        Err(e) => return Ok(slice::report_error_answer(&e).erase()),
    };
    let (policy, disagreement) =
        statecraft_run::policy::resolve(&root, &statecraft_run::policy::NoDeclarationFiled, None);
    // Spec 003 section 3.1.4 rule 3 and 3.1.5 rule 3: a journal that does not
    // read or verify is a failure, never an empty set of overrides.
    let overrides = match statecraft_run::overrides::read_held(ctx.home, &root, &held) {
        Ok(journal) => journal.overrides(),
        Err(e) => return Ok(slice::journal_error_answer(&e).erase()),
    };
    let work = statecraft_run::work::select(&report, &policy, &overrides, disagreement);
    let Some(spec_id) = spec_id else {
        return Err(Usage(
            "usage: run <path> <spec-id> [--witness | --require-witness]".to_string(),
        ));
    };
    Ok(run_admitted(
        ctx, &root, spec_id, work, &report, held, origin, witness, context,
    ))
}

/// The attempt itself, once [`run`] has admitted it.
#[allow(clippy::too_many_arguments)]
fn run_admitted(
    ctx: &Context<'_>,
    root: &Path,
    spec_id: &str,
    work: statecraft_run::work::WorkList,
    report: &statecraft_run::report::CorpusReport,
    held: statecraft_run::lock::Held,
    origin: Option<&Origin<'_>>,
    witness: Requirement,
    context: Option<&Path>,
) -> Response {
    // A unit of work the policy did not admit is never run, and the reason is
    // the answer (spec 003 section 3.1.1).
    let eligibility = work.eligibility_of(spec_id);
    let statecraft_run::work::Eligibility::Eligible(item) = &eligibility else {
        return slice::work_show_answer(eligibility).erase();
    };
    // Spec 003 section 3.1.4 rule 5: how it was admitted goes into the intent.
    let admission = item.admission(&work.policy);
    // Spec 004 section 3.17 rule 4: the command coverage at planning, before
    // anything is appended; each refusal here creates no process.
    let planned = match crate::coverage::at_planning(root, spec_id) {
        Ok(planned) => planned,
        Err(why) => return crate::coverage::planning_refused_answer(&why, None).erase(),
    };
    if let Some(why) = planned.refusal() {
        return crate::coverage::planning_refused_answer(&why, Some(&planned)).erase();
    }
    // Spec 003 section 3.1.3: the contract is resolved by the producer and
    // written with the intent, before any effect. It is evidence, not a gate.
    let judge = match crate::judge::report_source(root) {
        Ok(judge) => judge,
        Err(e) => return slice::report_error_answer(&e).erase(),
    };
    let contract =
        statecraft_run::contract::bind(&judge, root, spec_id, report.lifecycle_of(spec_id));
    let mut plan = crate::attempt::AttemptPlan::work_order(spec_id, admission, planned, held);
    plan.witness = witness;
    plan.context = context.map(Path::to_path_buf);
    if let Some(origin) = origin {
        plan.base = origin.commit.to_string();
        plan.proposal = Some(origin.proposal.to_string());
    }
    crate::launch::launch_attempt(ctx.clock, root, ctx.home, spec_id, plan, contract, None)
}

// ----------------------------------------------------------- startup trial

/// What `startup trial` was told: the origins the operator stated (exactly
/// one is admissible) and the deadline in seconds.
pub struct TrialOperands {
    /// Each origin stated, in order.
    pub stated: Vec<statecraft_home::trial::Origin>,
    /// The deadline, in seconds.
    pub deadline: u64,
}

/// `startup trial <path> (--provider-session | --synthetic) [--deadline <s>]`
/// (spec 006 section 3.11.4, spec 002 section 3.33).
///
/// The trial drives a session, so it has `run`'s preconditions, a registered
/// and armed target, judged first. `None` operands are ones the caller could
/// not read; they are usage only after those, as the verb orders them. Every
/// refusal here comes before an attempt is appended, so a refused trial
/// spends nothing.
pub fn startup_trial(
    ctx: &Context<'_>,
    path: &Path,
    operands: Option<TrialOperands>,
) -> Result<Response, Usage> {
    use statecraft_home::trial;
    let root = match registered(ctx.registry, path) {
        Ok(root) => root,
        Err(answer) => return Ok(answer),
    };
    let registration = ctx.registry.get(&root).expect("resolved above");
    if !registration.armed {
        return Ok(bind::unarmed_answer(&root).erase());
    }
    let Some(operands) = operands else {
        return Err(Usage(format!(
            "usage: startup trial{}",
            crate::manage::usage(crate::commands::Verb::StartupTrial)
        )));
    };
    let refused = |why: &str| Ok(slice::trial_refused_answer(why).erase());
    let origin = match operands.stated.as_slice() {
        [one] => *one,
        [] => {
            return refused(
                "the trial runs a provider session or a local fake, and the operator must                      say which: pass --provider-session or --synthetic",
            );
        }
        _ => return refused("--provider-session and --synthetic contradict each other; pass one"),
    };
    let deadline = operands.deadline;
    if let Some(why) = trial::refuses_deadline(deadline) {
        return refused(&why);
    }
    let manifest = match statecraft_environment::manifest::Manifest::read(&root) {
        Ok(Some(m)) => m,
        Ok(None) => {
            return refused(&format!(
                "{} holds no manifest, so it is not a managed project",
                root.display()
            ));
        }
        Err(e) => return Ok(failed(&e.to_string())),
    };
    let layout = statecraft_home::home::Layout::new(ctx.home);
    let standing = statecraft_home::required::evaluate(&layout, &manifest, None);
    if matches!(standing, statecraft_home::required::Standing::Unrequired) {
        return refused(
            "the project commits no harness requirement, so a run is not gated and a trial                  could not answer its question; commit one with `harness upgrade` first",
        );
    }
    if let Some(reason) = standing.refuses_a_run() {
        return Ok(bind::harness_refused_answer(&root, &reason).erase());
    }
    match Chain::open(ctx.home, &root) {
        Ok((chain, _)) => {
            if let Some(run) = statecraft_run::session::runs(&chain)
                .into_iter()
                .find(|r| r.id == trial::RUN_ID && !r.attempts.is_empty())
            {
                return refused(&format!(
                    "this project's trial is spent: run {} holds {} attempt(s). A trial \
                     happens once per project, and an uncertain one is never replayed; \
                     inspect it with `startup show {} {}`",
                    trial::RUN_ID,
                    run.attempts.len(),
                    root.display(),
                    trial::RUN_ID
                ));
            }
        }
        Err(e) => return Ok(failed(&e.to_string())),
    }
    if root.join(trial::SENTINEL).exists() {
        return refused(&format!(
            "{} already exists in the project, so the sentinel's nonce could not be the \
             only copy",
            trial::SENTINEL
        ));
    }
    Ok(crate::launch::launch_attempt(
        ctx.clock,
        &root,
        ctx.home,
        trial::RUN_ID,
        crate::attempt::AttemptPlan::trial(deadline),
        statecraft_run::contract::Binding::not_a_unit_of_work(),
        Some(crate::attempt::TrialRun {
            origin,
            deadline_seconds: deadline,
            sentinel: None,
            probed_version: None,
            settings_written: None,
            entries: Vec::new(),
            decision: None,
            process: statecraft_home::trial::ProcessEnd::default(),
        }),
    ))
}
