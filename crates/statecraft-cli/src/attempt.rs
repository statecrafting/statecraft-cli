//! The attempt helpers `run` and `startup trial` share (spec 040 slice 2):
//! what an attempt asks of its session, what a trial carries through it,
//! the startup record finalized after it, and the conclusion recorded and
//! answered. Moved from the binary unchanged in behavior; each returns its
//! answer or value and prints nothing.

use crate::application::Response;
use crate::render::Answer;
use crate::slice;
use statecraft_environment::time::Clock;
use statecraft_run::record::Chain;

/// What an attempt asks of its session. `run` asks for its unit of work; the
/// managed-startup trial asks for the sentinel read (spec 002 section 3.33
/// rule 30), and nothing else differs.
pub struct AttemptPlan {
    /// The prompt the session is given.
    pub prompt: Vec<u8>,
    /// The turn cap, if any.
    pub max_turns: Option<u32>,
    /// The session deadline, in seconds.
    pub deadline_seconds: u64,
    /// How the unit of work was admitted, written into the intent (spec 003
    /// section 3.1.4 rule 5). The trial is not a unit of work and has none.
    pub admission: Option<statecraft_run::work::Admission>,
    /// The planning reading of the command coverage (spec 004 section 3.17
    /// rule 4). The trial has no spec, so none, and its coverage is
    /// `not-applicable`.
    pub planned: Option<statecraft_adapter::coverage::Coverage>,
    /// The repository lock `run` took before it read the override journal
    /// (spec 003 section 3.1.5 rule 8). The trial takes its own.
    pub held: Option<statecraft_run::lock::Held>,
    /// The revision the workspace is prepared at: `HEAD`, or for an admitted
    /// spec 027 proposal the commit its fresh observation read.
    pub base: String,
    /// The spec 027 proposal whose admission this attempt is, written into
    /// the intent.
    pub proposal: Option<String>,
}

impl AttemptPlan {
    /// What `run` asks: its unit of work.
    pub fn work_order(
        spec_id: &str,
        admission: statecraft_run::work::Admission,
        planned: statecraft_adapter::coverage::Coverage,
        held: statecraft_run::lock::Held,
    ) -> Self {
        Self {
            prompt: format!("Implement {spec_id} in this workspace.").into_bytes(),
            max_turns: None,
            deadline_seconds: 900,
            admission: Some(admission),
            planned: Some(planned),
            held: Some(held),
            base: "HEAD".to_string(),
            proposal: None,
        }
    }

    /// What the managed-startup trial asks: the sentinel read.
    pub fn trial(deadline_seconds: u64) -> Self {
        Self {
            prompt: statecraft_home::trial::prompt().into_bytes(),
            max_turns: Some(statecraft_home::trial::MAX_TURNS),
            deadline_seconds,
            admission: None,
            planned: None,
            held: None,
            base: "HEAD".to_string(),
            proposal: None,
        }
    }
}

/// What a trial carries through its attempt, filled in as the launch goes.
pub struct TrialRun {
    /// Live or synthetic.
    pub origin: statecraft_home::trial::Origin,
    /// The session deadline, in seconds.
    pub deadline_seconds: u64,
    /// The sentinel the session is asked to read.
    pub sentinel: Option<statecraft_home::trial::Sentinel>,
    /// The provider version the probe read.
    pub probed_version: Option<String>,
    /// The settings file the launch wrote.
    pub settings_written: Option<String>,
    /// The stream entries the trial judges.
    pub entries: Vec<statecraft_home::trial::Entry>,
    /// The admission decision point, if reached.
    pub decision: Option<statecraft_home::trial::DecisionPoint>,
    /// How the process ended.
    pub process: statecraft_home::trial::ProcessEnd,
}

/// Judge and record a concluded trial, and answer it (spec 002 section 3.33).
pub fn trial_conclusion(
    places: &statecraft_home::launch::Places,
    concluded: &statecraft_run::session::Concluded,
    t: TrialRun,
) -> Answer<slice::TrialView> {
    use statecraft_home::trial;
    let attempt = statecraft_home::launch::AttemptIdentity {
        run_id: concluded.run_id.clone(),
        attempt: concluded.attempt,
    };
    let fact = statecraft_home::launch::AttemptFact {
        number: concluded.attempt,
        outcome: Some(concluded.outcome.word().to_string()),
    };
    let inspect = || {
        statecraft_home::launch::inspect(
            places,
            &attempt.run_id,
            Some(attempt.attempt),
            std::slice::from_ref(&fact),
        )
    };
    let mut not_stored = None;
    match (&t.sentinel, inspect()) {
        (None, _) => {}
        (Some(sentinel), Ok(before)) => {
            let facts = trial::Facts {
                version: trial::TRIAL_VERSION,
                origin: t.origin,
                attempt: attempt.clone(),
                deadline_seconds: t.deadline_seconds,
                max_turns: trial::MAX_TURNS,
                prompt: trial::prompt(),
                sentinel: sentinel.clone(),
                settings_written: t.settings_written,
                probed_version: t.probed_version,
                entries: t.entries,
                decision: t.decision,
                process: t.process,
            };
            match attempt.locate(places) {
                Ok(paths) => {
                    let gate = trial::read_gate(&paths);
                    let judgement = trial::judge(
                        &facts,
                        before.admission.as_deref(),
                        before
                            .intent
                            .as_ref()
                            .and_then(|i| i.required_harness.as_deref()),
                        &gate,
                        before.verdict,
                    );
                    if let Err(e) =
                        trial::write(&paths, &trial::TrialRecord::new(facts, gate, judgement))
                    {
                        not_stored = Some(format!("the trial's record could not be written: {e}"));
                    }
                }
                Err(e) => {
                    not_stored = Some(format!("the trial's record was not written: {e}"));
                }
            }
        }
        (Some(_), Err(e)) => {
            not_stored = Some(format!(
                "the attempt's records could not be read to judge the trial: {e}"
            ));
        }
    }
    slice::trial_answer(concluded, inspect(), t.sentinel.is_some(), not_stored)
}

/// A run attempt that never reached its launch: no record to write.
pub fn unlaunched(managed: bool, error: Option<String>) -> statecraft_home::launch::RunStartup {
    statecraft_home::launch::RunStartup {
        managed,
        error,
        ..statecraft_home::launch::RunStartup::unmanaged()
    }
}

/// The adapter name a manifest declares.
pub fn manifest_adapter_name(manifest: &statecraft_adapter::manifest::Manifest) -> String {
    manifest.adapter.clone()
}

/// The provider version the stream's init event reported.
pub fn provider_version(
    execution: &statecraft_adapter_claude_code::execution::Execution,
) -> Option<String> {
    execution
        .supervised
        .events
        .iter()
        .find_map(|event| match event {
            statecraft_adapter::protocol::Event::Init {
                provider_version, ..
            } => Some(provider_version.clone()),
            _ => None,
        })
}

/// What finalizing an attempt's startup evidence produced, for the record and
/// for the answer.
pub struct Finalized {
    /// The run's startup.
    pub run: statecraft_home::launch::RunStartup,
    /// The detail recorded with the outcome.
    pub detail: serde_json::Value,
    /// The guard and reason, when the record refuses the attempt.
    pub refusal: Option<(&'static str, String)>,
}

/// Write the attempt's record (spec 002 section 3.31), or say why it was not
/// stored. The judgement is the library's; this places its answer.
pub fn finalize_startup(
    clock: &dyn Clock,
    places: &statecraft_home::launch::Places,
    layout: &statecraft_home::home::Layout,
    manifest: Option<&statecraft_environment::manifest::Manifest>,
    prepared: Option<&statecraft_home::launch::Prepared>,
    launch: &statecraft_home::launch::Launch<'_>,
    watched: &statecraft_home::launch::Watched,
) -> Finalized {
    let (Some(manifest), Some(prepared)) = (manifest, prepared) else {
        return Finalized {
            run: statecraft_home::launch::RunStartup::unmanaged(),
            detail: serde_json::json!({ "managed": false }),
            refusal: None,
        };
    };
    let launched = watched.spawned.is_some();
    let admission = watched.admission.as_ref().map(|a| {
        format!(
            "{} at {}: {}",
            a.decision.describe(),
            a.decided_at,
            a.reason
        )
    });
    let now = statecraft_environment::time::rfc3339_utc(clock.now_unix());
    match statecraft_home::launch::finalize(
        places, layout, manifest, prepared, launch, watched, &now,
    ) {
        Ok((record, path)) => {
            let harness = record.launch.as_ref().map(|l| l.harness.describe());
            let refusal = statecraft_home::launch::refuses_the_attempt(&record);
            Finalized {
                detail: serde_json::json!({
                    "managed": true,
                    "intent": prepared.path.display().to_string(),
                    "intentDigest": prepared.digest,
                    "record": path.display().to_string(),
                    "observed": harness,
                    "standing": record.standing.word(),
                    "admission": watched.admission,
                    "spawnConfirmed": watched.spawned.is_some() && watched.confirmation_error.is_none(),
                }),
                run: statecraft_home::launch::RunStartup {
                    managed: true,
                    launched,
                    intent: Some(prepared.path.display().to_string()),
                    record: Some(path.display().to_string()),
                    verdict: None,
                    observed: harness,
                    admission,
                    error: None,
                },
                refusal,
            }
        }
        Err(e) => Finalized {
            detail: serde_json::json!({
                "managed": true,
                "intent": prepared.path.display().to_string(),
                "intentDigest": prepared.digest,
                "record": serde_json::Value::Null,
                "notStored": e.to_string(),
            }),
            run: statecraft_home::launch::RunStartup {
                managed: true,
                launched,
                intent: Some(prepared.path.display().to_string()),
                record: None,
                verdict: None,
                observed: None,
                admission,
                error: Some(format!("the startup record could not be written: {e}")),
            },
            refusal: None,
        },
    }
}

/// Conclude an attempt and answer it: the three calls spec 006 section 3.11
/// leaves to the caller share this one conclusion, and each argument is a
/// different fact it records.
#[allow(clippy::too_many_arguments)]
pub fn conclude(
    clock: &dyn Clock,
    chain: &mut Chain,
    places: &statecraft_home::launch::Places,
    session: &statecraft_run::session::Session,
    boundary: Option<&statecraft_home::confinement::Admission>,
    termination: impl Into<statecraft_run::session::Termination>,
    accounting: &statecraft_run::refusal::Accounting,
    mut detail: serde_json::Value,
    mut startup: statecraft_home::launch::RunStartup,
    contract: &statecraft_run::contract::Binding,
    trial: Option<TrialRun>,
) -> Response {
    let mut accounting = accounting.clone();
    detail["privateReferenceCleanup"] = match boundary.map_or_else(
        || Ok(Vec::new()),
        |boundary| statecraft_home::confinement::remove_extra_refs(boundary, &session.run_id),
    ) {
        Ok(removed) => serde_json::json!({ "removed": removed }),
        Err(error) => {
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: "private-reference-cleanup".into(),
                detail: error.clone(),
            });
            serde_json::json!({ "refused": error })
        }
    };
    match statecraft_run::session::conclude_observed(
        chain,
        &places.target,
        session,
        termination.into(),
        &accounting,
        detail,
        clock,
    ) {
        Ok(concluded) => {
            if let Some(t) = trial {
                return trial_conclusion(places, &concluded, t).erase();
            }
            let account = statecraft_acceptance::suite::fold(&session.run_id, &chain.entries());
            // The verdict is read back from what was written, beside the
            // outcome just recorded, so the answer is the persisted judgement
            // and not the one this process happened to hold (002 section 3.31
            // rule 21).
            if startup.managed {
                let fact = statecraft_home::launch::AttemptFact {
                    number: concluded.attempt,
                    outcome: Some(concluded.outcome.word().to_string()),
                };
                match statecraft_home::launch::inspect(
                    places,
                    &concluded.run_id,
                    Some(concluded.attempt),
                    &[fact],
                ) {
                    Ok(shown) => startup.verdict = Some(shown.verdict),
                    Err(e) if startup.error.is_none() => {
                        startup.error =
                            Some(format!("the startup records could not be read back: {e}"));
                    }
                    Err(_) => {}
                }
            }
            let mut answer = slice::run_answer_with_posture(concluded, account.posture, startup);
            // Spec 003 section 3.1.3: what this attempt was bound to, as its
            // intent records it.
            answer.value.contract = Some(contract.clone());
            answer
                .summary
                .push_str(&format!("contract  {}\n", contract.describe()));
            answer.erase()
        }
        Err(e) => slice::session_error_answer(&e).erase(),
    }
}
