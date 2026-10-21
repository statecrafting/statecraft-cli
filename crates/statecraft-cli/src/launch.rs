//! The shared attempt launch (spec 040 slice 3): begin, prepare, supervise
//! and conclude one attempt through the adapter, for `run` and for `startup
//! trial`. Moved from the binary in order, with its supervision and held
//! resources unchanged; it returns its answer and prints nothing.

#![allow(clippy::too_many_lines)]

use crate::application::{Response, failed};
use crate::attempt::{
    AttemptPlan, TrialRun, finalize_startup, manifest_adapter_name, provider_version, unlaunched,
};
use crate::exit::Exit;
use crate::render::Answer;
use crate::{adapters, bind, slice};
use statecraft_environment::time::Clock;
use statecraft_run::record::Chain;

/// The startup intent could not be recorded before admission, so nothing was
/// launched and no attempt was appended. This is not a boundary failure.
pub fn startup_record_refused(why: &str) -> Response {
    Answer::new(
        serde_json::json!({
            "outcome": "refused",
            "reason": statecraft_home::launch::STARTUP_RECORD_GUARD,
            "detail": why,
            "startup": { "launched": false, "error": why },
        }),
        Exit::Refused,
        format!("statecraft: startup refused: {why}"),
    )
}

/// The confinement could not be established: nothing was launched.
pub fn boundary_refused(detail: &str) -> Response {
    Answer::new(
        serde_json::json!({ "outcome": "refused", "reason": "boundary-unavailable", "detail": detail, "platform": std::env::consts::OS }),
        Exit::Refused,
        format!("statecraft: boundary unavailable: {detail}"),
    )
}

/// One attempt through the adapter: begin, prepare, supervise, conclude. The
/// run path, shared by `run` and by `startup trial`.
pub fn launch_attempt(
    clock: &dyn Clock,
    root: &std::path::Path,
    home: &std::path::Path,
    run_id: &str,
    plan: AttemptPlan,
    contract: statecraft_run::contract::Binding,
    mut trial: Option<TrialRun>,
) -> Response {
    // Spec 002 section 3.25: managed execution refuses when the committed
    // requirement's content cannot be established. Judged by the library and
    // before the attempt is appended, so a refusal leaves no attempt behind.
    let layout = statecraft_home::home::Layout::new(home);
    let project_manifest = match statecraft_environment::manifest::Manifest::read(root) {
        Ok(m) => m,
        Err(e) => return failed(&e.to_string()),
    };
    let standing = match &project_manifest {
        // Nothing has resolved for this session yet, and nothing is claimed
        // to have: a mismatch is decided from the session's own startup
        // acknowledgment, before its tool calls are released (spec 002
        // section 3.32 rule 26).
        Some(manifest) => statecraft_home::required::evaluate(&layout, manifest, None),
        None => statecraft_home::required::Standing::Unrequired,
    };
    if let Some(reason) = standing.refuses_a_run() {
        return bind::harness_refused_answer(root, &reason).erase();
    }

    // Spec 003 section 3.1.4 rule 7: the repository lock, held from before
    // the intent until the outcome is durable, released by the operating
    // system when this process ends however it ends. `run` took it before it
    // read the override journal (3.1.5 rule 8).
    let _held = match plan.held {
        Some(held) => held,
        None => match statecraft_run::lock::try_acquire(home, root) {
            Ok(held) => held,
            Err(e @ statecraft_run::lock::LockError::Busy { .. }) => {
                return slice::lock_busy_answer(&e.to_string()).erase();
            }
            Err(e) => return failed(&e.to_string()),
        },
    };
    let (mut chain, _) = match Chain::open(home, root) {
        Ok(c) => c,
        Err(e) => return failed(&e.to_string()),
    };
    // Spec 002 section 3.37: the attempt's launch records and its exchange
    // directory, in the product home and keyed as the chain just opened.
    let places = statecraft_home::launch::Places::of(home, root);

    // The run id is the spec id: spec 006 section 3.1 says the operator names a
    // unit of work, and spec 003 section 3.4 makes a retry an appended attempt
    // of the same run, so a fresh id per invocation would turn every retry into
    // a new run.
    let run_id = run_id.to_string();
    // Spec 004 section 3.17 rule 4: the intent records the planning verdict
    // and the digests of the plan and the allowance it read.
    let planning = plan.planned.as_ref().map(crate::coverage::planning_record);
    // Spec 029 section 3.3: the intent names the spec-spine this run's reads
    // were judged by, the one resolution every later step reuses.
    let judge = crate::judge::selection(root).record();
    let manifest_adapter = statecraft_adapter_claude_code::manifest();
    let initial_environment = adapters::child_environment();
    let path = initial_environment
        .variables
        .get("PATH")
        .cloned()
        .unwrap_or_default();
    let preliminary_probe =
        statecraft_adapter_claude_code::ConstructedEnvironmentProbe::from_child_environment(
            &initial_environment.variables,
        );
    let Some(program) = preliminary_probe.resolved_executable() else {
        return boundary_refused("provider executable unavailable");
    };
    let probe_executable = match std::env::current_exe() {
        Ok(program) => program,
        Err(e) => return boundary_refused(&e.to_string()),
    };
    let mut startup_prepared = None;
    let mut boundary_admission = None;
    let mut git_variables = std::collections::BTreeMap::new();
    let mut confined_probe = None;
    let mut preparation_owned = None;
    let mut startup_refusal = None;
    // Spec 013: the producer is located before the child's admission, which
    // reserves the endpoint only when there is a producer to bind it.
    let mut witness = crate::witness::Witness::resolve(home, plan.witness);
    let reserves_witness = witness.reserves();
    let mut admission =
        |workspace: &statecraft_run::workspace::Workspace, number: u32| -> Result<(), String> {
            let identity = statecraft_home::launch::AttemptIdentity {
                run_id: run_id.clone(),
                attempt: number,
            };
            let exchange = identity.exchange_dir(&places);
            let records = identity.records_dir(&places);
            if exchange.symlink_metadata().is_ok() || records.symlink_metadata().is_ok() {
                return Err("preparation paths already exist and are not adopted".into());
            }
            preparation_owned = Some((exchange, records));
            startup_prepared = match &project_manifest {
                None => None,
                Some(manifest) => {
                    let now = statecraft_environment::time::rfc3339_utc(clock.now_unix());
                    match statecraft_home::launch::prepare(&statecraft_home::launch::Preparation {
                        root,
                        places: &places,
                        workspace: &workspace.path,
                        base_commit: &workspace.base_commit,
                        attempt: statecraft_home::launch::AttemptIdentity {
                            run_id: run_id.clone(),
                            attempt: number,
                        },
                        recorded_at: &now,
                        layout: &layout,
                        manifest,
                        adapter: statecraft_home::startup::AdapterIdentity {
                            name: manifest_adapter_name(&manifest_adapter),
                            harness: statecraft_home::session::SUPPORTED_HARNESS.to_string(),
                            version: manifest_adapter.version.clone(),
                        },
                        program: &program.display().to_string(),
                    }) {
                        Ok(prepared) => Some(prepared),
                        Err(why) => {
                            // Not a boundary failure: the startup intent could
                            // not be recorded, and the refusal says so.
                            startup_refusal = Some(why.to_string());
                            return Err(why.to_string());
                        }
                    }
                }
            };
            let identity = statecraft_home::launch::AttemptIdentity {
                run_id: run_id.clone(),
                attempt: number,
            };
            let exchange = identity.exchange_dir(&places);
            std::fs::create_dir_all(&exchange).map_err(|e| e.to_string())?;
            let objects = exchange.join("objects");
            std::fs::create_dir(&objects).map_err(|e| e.to_string())?;
            let common = statecraft_run::trusted_git::common(root).map_err(|e| e.to_string())?;
            let refs = common.join("refs/heads/statecraft").join(&run_id);
            let logs = common.join("logs/refs/heads/statecraft").join(&run_id);
            for directory in [&refs, &logs] {
                std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
            }
            let admin = common
                .join("worktrees")
                .join(workspace.path.file_name().ok_or("workspace has no name")?);
            if !admin.is_dir() {
                return Err("exact worktree administrative directory unavailable".into());
            }
            for (name, value) in [
                ("GIT_DIR", admin.clone()),
                ("GIT_COMMON_DIR", common.clone()),
                ("GIT_WORK_TREE", workspace.path.clone()),
                ("GIT_OBJECT_DIRECTORY", objects.clone()),
                ("GIT_ALTERNATE_OBJECT_DIRECTORIES", common.join("objects")),
            ] {
                git_variables.insert(name.into(), value.display().to_string());
            }
            let gate = identity.gate_log_path(&places);
            let gated = gate.is_file();
            let admitted = statecraft_home::confinement::admit(
                statecraft_home::confinement::Roots {
                    home,
                    target: root,
                    workspace: &workspace.path,
                    exchange: &exchange,
                    records: &places.records,
                    gate: gated.then_some(gate.as_path()),
                    git_writes: vec![objects, refs, logs, admin],
                    provider: true,
                    authenticated: true,
                    witness: reserves_witness,
                },
                &probe_executable,
                &path,
            )?;
            admitted
                .boundary
                .resolve(&program, &path)
                .map_err(|e| e.to_string())?;
            let mut variables = initial_environment.variables.clone();
            variables.extend(admitted.variables.clone());
            variables.extend(git_variables.clone());
            confined_probe = Some(adapters::probe_confined(
                home,
                &program,
                &workspace.path,
                &variables,
                &admitted,
            )?);
            admitted.retest(&probe_executable)?;
            boundary_admission = Some(admitted);
            Ok(())
        };
    let session = match statecraft_run::session::begin_checked(
        &mut chain,
        root,
        &run_id,
        &plan.base,
        clock,
        &statecraft_run::session::IntentDetail {
            contract: Some(&contract),
            admission: plan.admission.as_ref(),
            posture_coverage: planning.as_ref(),
            judge: judge.as_ref(),
            proposal: plan.proposal.as_deref(),
            // Spec 019: `run` binds no context packet until its command
            // projection lands; the intent renders `not-recorded`.
            context_manifest: None,
            context_policy: None,
        },
        Some(&mut admission),
    ) {
        Ok(s) => s,
        Err(e) => {
            if let statecraft_run::session::SessionError::Boundary(detail) = &e {
                // A startup intent that could not be recorded is refused as
                // itself, not as an unavailable boundary.
                let refuse = |suffix: String| match &startup_refusal {
                    Some(why) => startup_record_refused(&format!("{why}{suffix}")),
                    None => boundary_refused(&format!("{detail}{suffix}")),
                };
                // The refusal is durable before these supervisor-created,
                // unlaunched paths move. Retain them for diagnosis without
                // making the next attempt adopt a failed preparation.
                if let Some((exchange, records)) = &preparation_owned {
                    let archive = places.records.join("refused-preparations").join(format!(
                        "{}-{}",
                        run_id,
                        chain.entries().len()
                    ));
                    let timestamp = statecraft_environment::time::rfc3339_utc(clock.now_unix());
                    let key = format!("{run_id}/boundary-archive/{}", chain.entries().len());
                    let decision = statecraft_run::Entry {
                        kind: statecraft_run::Kind::Outcome,
                        run_id: run_id.clone(),
                        attempt: 0,
                        subject: "boundary-preparation-archive".into(),
                        effect_id: statecraft_run::Identity::Absent,
                        idempotency_key: None,
                        detail: serde_json::json!({ "refusal": detail, "exchange": exchange, "records": records, "archive": archive }),
                    };
                    if let Err(error) = chain.append(&key, &timestamp, &decision) {
                        return refuse(format!("; archive decision: {error}"));
                    }
                    let archived = (|| -> std::io::Result<()> {
                        std::fs::create_dir_all(&archive)?;
                        for (source, name) in [(exchange, "exchange"), (records, "records")] {
                            if source.symlink_metadata().is_ok() {
                                std::fs::rename(source, archive.join(name))?;
                            }
                        }
                        Ok(())
                    })();
                    if let Err(error) = archived {
                        return refuse(format!("; archive preparation: {error}"));
                    }
                }
                return refuse(String::new());
            }
            // Spec 002 section 3.32 rule 24: a live attempt is never
            // replayed, and the refusal names what its launch records
            // establish.
            if let statecraft_run::session::SessionError::LiveAttempt {
                run_id: live_run,
                attempt,
            } = &e
            {
                let startup = project_manifest.as_ref().and_then(|_| {
                    statecraft_home::launch::inspect(
                        &places,
                        live_run,
                        Some(*attempt),
                        &[statecraft_home::launch::AttemptFact {
                            number: *attempt,
                            outcome: None,
                        }],
                    )
                    .ok()
                });
                // Spec 003 section 3.6.1 rule 4: the refusal names the
                // attempt's `unknown` reconciliation where it has one.
                let reconciliation = statecraft_run::session::runs(&chain)
                    .into_iter()
                    .find(|r| r.id == *live_run)
                    .and_then(|r| {
                        r.attempts
                            .into_iter()
                            .find(|a| a.number == *attempt)
                            .and_then(|a| a.reconciliation)
                    });
                // Spec 003 section 3.5.1 rule 2: a later run names the
                // change found while that attempt's process ran.
                let audit = statecraft_run::tamper::read_audit(home, root)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|f| f.run_id == *live_run && f.attempt == *attempt)
                    .collect();
                return slice::live_attempt_answer(
                    &e,
                    &root.display().to_string(),
                    startup,
                    reconciliation,
                    audit,
                )
                .erase();
            }
            return slice::session_error_answer(&e).erase();
        }
    };

    // Spec 002 section 3.33 rule 31: the trial's sentinel is in the attempt's
    // own workspace before anything is prepared or launched.
    if let Some(t) = trial.as_mut() {
        match statecraft_home::trial::place_sentinel(&session.workspace.path) {
            Ok(sentinel) => t.sentinel = Some(sentinel),
            Err(e) => {
                let mut accounting = statecraft_run::refusal::Accounting::without_process();
                accounting.observe(statecraft_run::refusal::RefusalEvent {
                    guard: "trial-sentinel".to_string(),
                    detail: format!("the trial's sentinel could not be placed: {e}"),
                });
                return crate::attempt::conclude(
                    clock,
                    &mut chain,
                    &places,
                    &session,
                    boundary_admission.as_ref(),
                    statecraft_run::attempt::Outcome::Refused,
                    &accounting,
                    witness.marked(serde_json::json!({ "trialRefusal": e.to_string() })),
                    unlaunched(project_manifest.is_some(), None),
                    &contract,
                    trial,
                );
            }
        }
    }

    // Spec 004 section 3.17 rule 4: the comparison is confirmed at launch,
    // after the base commit is resolved and before the spawn, from that
    // commit and never from the reused workspace. This is the one recorded.
    // The base is the one the intent recorded, never the reused workspace's
    // `HEAD`, which a previous session may have moved (spec 003 section 3.2).
    let base_commit = session.base_commit.clone();
    let covered = match &plan.planned {
        Some(planned) => crate::coverage::at_base(root, &base_commit, planned),
        None => crate::coverage::not_applicable(root, &base_commit),
    };

    // Preflight refuses before any process is created, naming the token (spec
    // 004 section 3.3). The manifest is read here and only here.

    let manifest = &manifest_adapter;
    let environment = adapters::child_environment_with(&[], covered.as_ref().ok());
    let requested = statecraft_adapter::capability::Requested::none()
        .requiring(statecraft_adapter::capability::Capability::StructuredRefusals)
        .preferring(statecraft_adapter::capability::Capability::TurnLimit)
        .preferring(statecraft_adapter::capability::Capability::CostReport);

    let mut posture = statecraft_adapter::posture::Posture::new(
        manifest,
        statecraft_adapter::manifest::Qualification::Unqualified,
        &requested,
        &statecraft_adapter::capability::negotiate(&requested, &manifest.supports),
        &[],
        &environment,
    );
    match &covered {
        Ok(c) => posture = posture.with_coverage(c.clone()),
        // Nothing was compared; the record says why, beside the coverage line.
        Err(why) => {
            posture.coverage = statecraft_adapter::coverage::CoverageRecord::Unread(
                statecraft_adapter::coverage::Unread {
                    unread: why.clone(),
                },
            );
        }
    }
    // A launch refusal comes after the intent, so the attempt is concluded
    // `refused` under the guard `posture-coverage`, the way a preflight
    // refusal after the intent is concluded (spec 004 section 3.17 rule 4).
    let coverage_refusal = match &covered {
        Err(why) => Some(why.clone()),
        Ok(c) => c.refusal(),
    };
    if let Some(why) = coverage_refusal {
        let mut accounting = statecraft_run::refusal::Accounting::without_process();
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: statecraft_adapter::coverage::GUARD.to_string(),
            detail: why.clone(),
        });
        return crate::attempt::conclude(
            clock,
            &mut chain,
            &places,
            &session,
            boundary_admission.as_ref(),
            statecraft_run::attempt::Outcome::Refused,
            &accounting,
            witness
                .marked(serde_json::json!({ "postureCoverageRefusal": why, "posture": posture })),
            unlaunched(project_manifest.is_some(), None),
            &contract,
            trial,
        );
    }
    let negotiation = match statecraft_adapter::supervisor::preflight(
        manifest,
        &requested,
        &environment,
    ) {
        Ok(n) => n,
        Err(refusal) => {
            // No process was created, so the attempt is concluded as
            // refused rather than left live: an intent with no outcome
            // would send the next run to reconciliation for something that
            // never started.
            let mut accounting = statecraft_run::refusal::Accounting::without_process();
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: "adapter-preflight".to_string(),
                detail: refusal.to_string(),
            });
            return crate::attempt::conclude(
                clock,
                &mut chain,
                &places,
                &session,
                boundary_admission.as_ref(),
                statecraft_run::attempt::Outcome::Refused,
                &accounting,
                witness.marked(serde_json::json!({ "preflightRefusal": refusal.to_string(), "posture": posture })),
                unlaunched(project_manifest.is_some(), None),
                &contract,
                trial,
            );
        }
    };

    let mut boundary_admission =
        boundary_admission.expect("successful boundary check admitted the launch");
    let (probe, probed_version) = confined_probe.expect("admission observed the provider version");
    if let Some(t) = trial.as_mut() {
        t.probed_version = probed_version;
    }
    posture.qualification = probe.qualification();

    // Spec 002 section 5, 2026-09-25: the supervisor selects the spec-spine
    // binary a managed session's hooks use, by the convention candidates
    // alone on the child's PATH, and places it in the constructed environment
    // below over any value already there. Neither selection variable is read
    // from the operator's environment, so a shell value cannot reach a managed
    // hook. Candidates that exist with none the project admits refuse the
    // attempt before its intent is written; no candidate at all leaves the
    // hooks their absent-binary behavior.
    let supervised_spec_spine = match &project_manifest {
        None => None,
        Some(_) => {
            let selection = statecraft_home::spec_spine::for_supervisor(
                root,
                std::env::var("PATH").ok().as_deref(),
            );
            match selection.outcome {
                Ok(selected) => Some(selected.program),
                Err(statecraft_home::spec_spine::Unselected::Absent) => None,
                Err(statecraft_home::spec_spine::Unselected::Refused(why)) => {
                    let mut detail = selection.passed_over.join("; ");
                    if !detail.is_empty() {
                        detail.push_str("; ");
                    }
                    detail.push_str(&why);
                    let mut accounting = statecraft_run::refusal::Accounting::without_process();
                    accounting.observe(statecraft_run::refusal::RefusalEvent {
                        guard: statecraft_home::spec_spine::SELECTION_GUARD.to_string(),
                        detail: detail.clone(),
                    });
                    return crate::attempt::conclude(
                        clock,
                        &mut chain,
                        &places,
                        &session,
                        Some(&boundary_admission),
                        statecraft_run::attempt::Outcome::Refused,
                        &accounting,
                        witness.marked(
                            serde_json::json!({ "preflightRefusal": detail, "posture": posture }),
                        ),
                        unlaunched(true, None),
                        &contract,
                        trial,
                    );
                }
            }
        }
    };

    // Spec 002 section 3.31 rules 13 to 15: every preflight has passed, so the
    // intent is written now, before the process exists. If it cannot be,
    // nothing is launched and the attempt is refused under its own guard. A
    // repository holding no manifest is not a managed session and records no
    // startup evidence; its answer says so.

    let prepared = startup_prepared;
    let mut environment = match &prepared {
        Some(p) => {
            let binding = statecraft_home::spec_spine::managed_binding(
                &p.intent.environment(),
                supervised_spec_spine.as_deref(),
            );
            adapters::child_environment_with(&binding, covered.as_ref().ok())
        }
        None => environment,
    };
    environment
        .variables
        .extend(boundary_admission.variables.clone());
    environment.variables.extend(git_variables);
    environment.confinement = Some(std::sync::Arc::clone(&boundary_admission.boundary));
    posture = posture.with_confinement(boundary_admission.boundary.record().clone());

    // Spec 013: every preflight has passed and the attempt's intent is
    // durable, so the sidecar's effect intent is written and the sidecar
    // started now. A sidecar that fails after the endpoint was reserved
    // refuses the child (sections 3.3 and 4).
    let exchange = statecraft_home::launch::AttemptIdentity {
        run_id: run_id.clone(),
        attempt: session.attempt,
    }
    .exchange_dir(&places);
    let started = witness.start(
        &mut chain,
        &statecraft_environment::time::rfc3339_utc(clock.now_unix()),
        &crate::witness::Roots {
            home,
            records: &places.records,
            target: root,
            workspace: &session.workspace.path,
            exchange: &exchange,
        },
        &run_id,
        session.attempt,
        &mut boundary_admission,
        &probe_executable,
    );
    match started {
        Ok(Some(variables)) => environment.variables.extend(variables),
        Ok(None) => {}
        Err(why) => {
            let mut accounting = statecraft_run::refusal::Accounting::without_process();
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: crate::witness::GUARD.to_string(),
                detail: why.clone(),
            });
            let timestamp = statecraft_environment::time::rfc3339_utc(clock.now_unix());
            if let Some(error) = witness.write_outcome(&mut chain, &timestamp) {
                accounting.observe(statecraft_run::refusal::RefusalEvent {
                    guard: crate::witness::GUARD.to_string(),
                    detail: format!("the witness outcome could not be written: {error}"),
                });
            }
            return crate::attempt::conclude(
                clock,
                &mut chain,
                &places,
                &session,
                Some(&boundary_admission),
                statecraft_run::attempt::Outcome::Refused,
                &accounting,
                witness.marked(serde_json::json!({ "witnessRefusal": why, "posture": posture })),
                unlaunched(project_manifest.is_some(), None),
                &contract,
                trial,
            );
        }
    }

    // Spec 002 section 3.27: the floor reaches a managed session through the
    // per-session settings argument. Section 3.32 rule 25: where a revision is
    // selected, the same document registers its startup hook and this
    // attempt's admission gate, so nothing depends on a registration in the
    // operator's home. The bytes are the intent's, exactly. Delivering them
    // claims nothing about enforcement; the posture below still says
    // unqualified.
    let floor: Vec<String> = statecraft_home::settings::DENY_FLOOR
        .iter()
        .map(|r| (*r).to_string())
        .collect();
    let mut invocation = statecraft_adapter_claude_code::Invocation::new(
        &program.display().to_string(),
        &floor,
        plan.max_turns,
    );
    if let Some(hooks) = prepared.as_ref().and_then(|p| p.intent.hooks()) {
        invocation = invocation.with_hooks(hooks);
    }
    let document = prepared.as_ref().map_or_else(
        statecraft_home::session::payload_json,
        statecraft_home::launch::Prepared::settings_document,
    );
    let invocation = match invocation.with_settings_document(document) {
        Ok(invocation) => invocation,
        Err(e) => return failed(&e),
    };
    let request = statecraft_adapter::protocol::Request {
        context: None,
        workspace: session.workspace.path.clone(),
        base_commit: session.workspace.base_commit.clone(),
        prompt: plan.prompt.clone(),
        capabilities: requested.clone(),
        deadline_seconds: plan.deadline_seconds,
        attempt: statecraft_adapter::protocol::AttemptIdentity {
            run_id: run_id.clone(),
            number: session.attempt,
        },
    };

    // Spec 002 section 3.32 rules 22 and 26: a managed launch is watched. The
    // spawn is confirmed on disk before the prompt is delivered, and the
    // startup decision is made at the first event after the startup hooks,
    // stopping the process when it refuses.
    let now = || {
        statecraft_environment::time::rfc3339_utc(statecraft_environment::time::Clock::now_unix(
            clock,
        ))
    };
    let mut watch = match (&prepared, &project_manifest) {
        (Some(p), Some(m)) => Some(statecraft_home::launch::LaunchWatch::new(
            &places, &layout, m, p, &now,
        )),
        _ => None,
    };
    // Spec 002 section 3.37 rule 2: a managed attempt's settings document is
    // written into its exchange directory, which `prepare` created.
    let sentinel = trial.as_ref().and_then(|t| t.sentinel.clone());
    // Spec 003 section 3.5.1 rule 1, second kind: the protected files are
    // digested after the intent is durable and before the spawn, under the
    // repository lock this process holds throughout.
    let before = statecraft_run::tamper::Snapshot::take(home, root);
    let supervised = match (watch.as_mut(), &sentinel) {
        // Spec 002 section 3.33 rule 34: the trial keeps a timeline beside
        // the attempt's own watch, which it forwards to unchanged.
        (Some(w), Some(sentinel)) => {
            let mut kept = statecraft_home::trial::TrialWatch::new(w, sentinel);
            let supervised = statecraft_adapter_claude_code::execution::supervise_with_in(
                &invocation,
                &request,
                &environment,
                &negotiation.granted,
                &exchange,
                &mut witness.custody(&mut kept),
            );
            if supervised.is_ok() {
                kept.decide_at_end();
            }
            if let Some(t) = trial.as_mut() {
                t.entries = std::mem::take(&mut kept.entries);
                t.decision = kept.decision.take();
            }
            supervised
        }
        (Some(w), None) => {
            let supervised = statecraft_adapter_claude_code::execution::supervise_with_in(
                &invocation,
                &request,
                &environment,
                &negotiation.granted,
                &exchange,
                &mut witness.custody(&mut *w),
            );
            if supervised.is_ok() {
                // A stream that ended before its decision point is decided at
                // its end.
                w.decide_at_end();
            }
            supervised
        }
        (None, _) => statecraft_adapter_claude_code::execution::supervise_with_in(
            &invocation,
            &request,
            &environment,
            &negotiation.granted,
            &exchange,
            &mut witness.custody(&mut statecraft_adapter::supervisor::Unwatched),
        ),
    };
    let watched = watch.map(|w| w.watched()).unwrap_or_default();
    // ... and again once the process has ended, before anything is appended.
    // The chain appends from the head it read at opening, so a change found
    // here ends the attempt's writes: it stays live (rule 2).
    let record_change = before
        .and_then(|before| {
            statecraft_run::tamper::Snapshot::take(home, root)
                .map(|after| after.changes_since(&before, &watched.wrote))
        })
        .map_err(|e| format!("the record could not be digested: {e}"));
    if let Ok(changes) = &record_change
        && !changes.is_empty()
    {
        let finding = statecraft_run::tamper::AuditFinding {
            kind: "record-change".to_string(),
            run_id: run_id.clone(),
            attempt: session.attempt,
            found_at: statecraft_environment::time::rfc3339_utc(clock.now_unix()),
            changes: changes.clone(),
        };
        // The attempt stays live, and so does the sidecar's effect.
        witness.abort("record-change");
        let recorded =
            statecraft_run::tamper::append_audit(home, root, &finding).map_err(|e| e.to_string());
        return slice::record_changed_answer(&root.display().to_string(), finding, recorded)
            .erase();
    }
    let protected = {
        let record = boundary_admission.boundary.record();
        let forbidden: Vec<_> = record
            .policy
            .inaccessible
            .iter()
            .chain(&record.policy.readonly)
            .cloned()
            .collect();
        let writable: Vec<_> = record
            .policy
            .writable
            .iter()
            .map(|g| g.path.clone())
            .collect();
        statecraft_run::tamper::Protected::new(home, &forbidden, &writable)
    };
    // Rule 6: a confinement record without its self-test result leaves both
    // kinds unknown.
    let self_tested = !boundary_admission.boundary.record().self_test.is_empty();
    let record_answer = match &record_change {
        Ok(_) if self_tested => statecraft_run::tamper::Answer::NoneObserved,
        _ => statecraft_run::tamper::Answer::Unknown,
    };
    if let Some(t) = trial.as_mut() {
        t.process = match &supervised {
            Ok(e) => statecraft_home::trial::ProcessEnd {
                spawned: watched.spawned.is_some(),
                outcome: Some(e.supervised.outcome.word().to_string()),
                stopped: e.supervised.stopped.clone(),
                stream_error: e.supervised.stream_error.as_ref().map(ToString::to_string),
                surviving: e.supervised.surviving_processes.clone(),
                failed: None,
                timed_out: e.supervised.timed_out,
            },
            Err(e) => statecraft_home::trial::ProcessEnd {
                spawned: watched.spawned.is_some(),
                failed: Some(e.to_string()),
                ..statecraft_home::trial::ProcessEnd::default()
            },
        };
        if let Ok(e) = &supervised {
            t.settings_written = Some(String::from_utf8_lossy(&e.settings_written).into_owned());
        }
    }
    let execution = match supervised {
        Ok(s) => s,
        Err(e) => {
            let mut accounting = statecraft_run::refusal::Accounting::default();
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: "supervisor".to_string(),
                detail: e.to_string(),
            });
            // No stream was mapped; with no process there was nothing to map.
            accounting.tamper_answers = statecraft_run::tamper::Answers {
                write_request: if watched.spawned.is_none() && self_tested {
                    statecraft_run::tamper::Answer::NoneObserved
                } else {
                    statecraft_run::tamper::Answer::Unknown
                },
                record_change: record_answer,
            };
            let startup = finalize_startup(
                clock,
                &places,
                &layout,
                project_manifest.as_ref(),
                prepared.as_ref(),
                &statecraft_home::launch::Launch::Failed {
                    reason: e.to_string(),
                },
                &watched,
            );
            if let Some(error) = witness.write_outcome(&mut chain, &now()) {
                accounting.observe(statecraft_run::refusal::RefusalEvent {
                    guard: crate::witness::GUARD.to_string(),
                    detail: format!("the witness outcome could not be written: {error}"),
                });
            }
            return crate::attempt::conclude(
                clock,
                &mut chain,
                &places,
                &session,
                Some(&boundary_admission),
                statecraft_run::attempt::Outcome::Interrupted,
                &accounting,
                witness.marked(serde_json::json!({ "supervisorError": e.to_string(), "posture": posture, "startup": startup.detail })),
                startup.run,
                &contract,
                trial,
            );
        }
    };

    let transfer = statecraft_home::confinement::import_objects(
        &boundary_admission,
        root,
        &session.workspace.path,
        &run_id,
        &environment.variables,
    );
    // Section 3.3: custody ends after the child and its objects; the object
    // import retests the child's boundary, whose probe connects to the
    // endpoint the sidecar still holds.
    witness.close();
    let supervised = &execution.supervised;
    let posture = posture.with_execution(supervised);
    let mut accounting = statecraft_run::refusal::Accounting::default();
    if let Err(why) = &transfer {
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: "object-transfer".into(),
            detail: why.clone(),
        });
    }
    for refusal in statecraft_adapter::protocol::refusals(&supervised.events) {
        accounting.observe(refusal);
    }
    // Spec 003 section 3.5.1 rule 1, first kind: beside the count, which it
    // does not change.
    let write_answer = match &execution.write_requests {
        Ok(requests) => {
            accounting.tamper_findings = protected.judge(&session.workspace.path, requests);
            statecraft_run::tamper::Answer::of(!accounting.tamper_findings.is_empty())
        }
        Err(_) => statecraft_run::tamper::Answer::Unknown,
    };
    accounting.tamper_answers = statecraft_run::tamper::Answers {
        write_request: if self_tested {
            write_answer
        } else {
            statecraft_run::tamper::Answer::Unknown
        },
        record_change: record_answer,
    };

    // Spec 002 section 3.31: the record is written from what the launch
    // produced, and a startup decision that refused governed work refuses the
    // attempt under its guard (section 3.32 rule 26).
    let version = provider_version(&execution);
    let startup = finalize_startup(
        clock,
        &places,
        &layout,
        project_manifest.as_ref(),
        prepared.as_ref(),
        &statecraft_home::launch::Launch::Spawned {
            hook_responses: &execution.hook_responses,
            session_id: execution.session_id.as_deref(),
            provider_version: version.as_deref(),
            settings_written: &execution.settings_written,
            outcome: supervised.outcome.word(),
            stream_error: supervised.stream_error.as_ref().map(ToString::to_string),
            surviving_processes: supervised.surviving_processes.clone(),
            stopped: supervised.stopped.clone(),
        },
        &watched,
    );
    if let Some((guard, why)) = &startup.refusal {
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: (*guard).to_string(),
            detail: why.clone(),
        });
    }

    if let Some(why) = witness.write_outcome(&mut chain, &now()) {
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: crate::witness::GUARD.to_string(),
            detail: format!("the witness outcome could not be written: {why}"),
        });
    }
    crate::attempt::conclude(
        clock,
        &mut chain,
        &places,
        &session,
        Some(&boundary_admission),
        execution.termination(),
        &accounting,
        witness.marked(serde_json::json!({
            "requested": negotiation
                .granted
                .iter()
                .map(|c| c.token())
                .collect::<Vec<_>>(),
            "objectTransfer": transfer.as_ref().map_err(ToString::to_string),
            "applied": posture.applied.iter().map(|c| c.token()).collect::<Vec<_>>(),
            "posture": posture,
            "degraded": negotiation.degraded.iter().map(|c| c.token()).collect::<Vec<_>>(),
            "specId": run_id,
            "execution": execution.evidence(),
            "payload": {
                "digest": statecraft_home::startup::payload_identity(),
                "argument": statecraft_home::session::SETTINGS_ARGUMENT,
            },
            "harnessStanding": standing.word(),
            "startup": startup.detail,
        })),
        startup.run,
        &contract,
        trial,
    )
}
