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

/// Spec 019 section 3.2: the context packets could not be assembled, or the
/// manifest they compose may not execute. Refused before the intent, so no
/// attempt was appended and nothing was launched.
pub fn context_refused(detail: &str) -> Response {
    Answer::new(
        serde_json::json!({ "outcome": "refused", "reason": "context-ineligible", "detail": detail }),
        Exit::Refused,
        format!("statecraft: context refused: {detail}"),
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
            Ok(held) => std::sync::Arc::new(held),
            Err(e @ statecraft_run::lock::LockError::Busy { .. }) => {
                return slice::lock_busy_answer(&e.to_string()).erase();
            }
            Err(e) => return failed(&e.to_string()),
        },
    };
    // Spec 050 section 3.3: a chain that cannot be read or verified is a halt
    // state that cannot be read, and it refuses rather than admitting; the
    // halt state is then folded from the chain just opened, under the lock,
    // before anything about the attempt is prepared. `begin_checked` folds it
    // again before the intent, so no launch path skips it.
    // A chain filed under another spelling of the repository's path keeps
    // its own refusal and guidance (spec 003); it is not a record that failed
    // to read.
    let (mut chain, _) = match Chain::open(home, root) {
        Ok(c) => c,
        Err(e @ statecraft_run::record::RecordError::FiledElsewhere { .. }) => {
            return failed(&e.to_string());
        }
        Err(e) => return slice::halt_refused_answer(&statecraft_run::halt::Refusal::from(e)),
    };
    // Spec 050 section 5, 2026-11-01: a request queued while another run
    // held the lock, and left behind when that supervisor ended without
    // applying it, is appended first, so a queued halt refuses this launch.
    if let Err(e) = statecraft_run::halt::apply_pending(&mut chain, home, root) {
        return match e {
            statecraft_run::halt::HaltError::Unreadable(r) => slice::halt_refused_answer(&r),
            other => failed(&other.to_string()),
        };
    }
    if let Err(refusal) = statecraft_run::halt::admit(&chain) {
        return slice::halt_refused_answer(&refusal);
    }
    // Spec 002 section 3.37: the attempt's launch records and its exchange
    // directory, in the product home and keyed as the chain just opened.
    let places = statecraft_home::launch::Places::of(home, root);
    // Spec 019 sections 3.1 and 3.2: the manifest is composed for the attempt
    // this intent will append, from packets assembled and retained now, and
    // refused here when it may not execute, before anything is recorded.
    let context = match &plan.context {
        None => None,
        Some(file) => {
            let number = statecraft_run::session::runs(&chain)
                .iter()
                .find(|r| r.id == run_id)
                .map_or(1, |r| r.attempts.len() as u32 + 1);
            match crate::context::prepare(root, home, run_id, number, file) {
                Ok(prepared) => Some(prepared),
                Err(why) => return context_refused(&why),
            }
        }
    };

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
    // Spec 047 sections 3.2 and 3.3: what this launch needs is read from the
    // selected adapter's declaration. A witness only requested of an adapter
    // that cannot carry one is not started and the degradation is recorded;
    // a required field it does not declare refuses below, before spawn.
    let declared = adapters::capabilities();
    let fitted = statecraft_adapter::capabilities::fit(
        &declared,
        &statecraft_adapter::capabilities::Needs::launch(plan.witness, project_manifest.is_some()),
    );
    let mut witness =
        crate::witness::Witness::resolve(home, fitted.witness_requirement(plan.witness))
            .degraded(&fitted.degraded);
    let reserves_witness = witness.reserves();
    let per_run_config_dir = declared.per_run_config_dir;
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
                    provider: per_run_config_dir,
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
            // Spec 019 section 3.6: the manifest and its policy, once, with
            // the intent, before the adapter process exists.
            context_manifest: context.as_ref().map(|c| &c.manifest),
            context_policy: context.as_ref().map(|c| &c.policy),
            documentation_task: None,
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
            if let statecraft_run::session::SessionError::Halted(refusal) = &e {
                return slice::halt_refused_answer(refusal);
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

    // Spec 047 section 3.3: a required capability field the adapter does not
    // declare refuses before spawn, naming the field, concluded the way a
    // missing required token is above.
    if let Some(undeclared) = fitted.refusal(&manifest.adapter) {
        let mut accounting = statecraft_run::refusal::Accounting::without_process();
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: statecraft_adapter::capabilities::REASON.to_string(),
            detail: undeclared.to_string(),
        });
        return crate::attempt::conclude(
            clock,
            &mut chain,
            &places,
            &session,
            boundary_admission.as_ref(),
            statecraft_run::attempt::Outcome::Refused,
            &accounting,
            witness.marked(serde_json::json!({
                "capabilityRefusal": undeclared.to_string(),
                "undeclared": undeclared.fields,
                "posture": posture,
            })),
            unlaunched(project_manifest.is_some(), None),
            &contract,
            trial,
        );
    }

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
    // Spec 019 section 3.5: the delivered pages, read back from custody, ride
    // beside the prompt.
    let bundle: Result<Option<_>, statecraft_adapter::context::BundleError> = match &context {
        None => Ok(None),
        Some(c) => match statecraft_adapter::context::Bundle::assemble(&c.manifest, |custody| {
            statecraft_run::context::read_retained(home, custody)
                .ok()
                .flatten()
        }) {
            Ok(b) => Ok(b),
            Err(e) => Err(e),
        }
        .map(Some),
    };
    // Section 3.6: the request must carry exactly the manifest the intent
    // recorded. A refusal comes after the intent and before any process, so
    // the attempt is concluded `refused` under the guard `context-bundle`.
    let bundle = bundle.and_then(|b| {
        let recorded = statecraft_run::context::recorded_manifest(&chain, &run_id, session.attempt)
            .map_err(|why| {
                statecraft_adapter::context::BundleError::Unexpected(format!(
                    "the recorded manifest: {why}"
                ))
            })?;
        statecraft_adapter::context::check(b.as_ref(), recorded.as_ref())?;
        Ok(b)
    });
    let bundle = match bundle {
        Ok(b) => b,
        Err(why) => {
            let why = why.to_string();
            let mut accounting = statecraft_run::refusal::Accounting::without_process();
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: "context-bundle".to_string(),
                detail: why.clone(),
            });
            return crate::attempt::conclude(
                clock,
                &mut chain,
                &places,
                &session,
                Some(&boundary_admission),
                statecraft_run::attempt::Outcome::Refused,
                &accounting,
                witness.marked(serde_json::json!({ "contextRefusal": why })),
                unlaunched(project_manifest.is_some(), None),
                &contract,
                trial,
            );
        }
    };
    // Spec 020 section 3.4: the request carries exactly the task the intent
    // recorded, or none when it recorded none, and every capability the task
    // requires is one the selected adapter declares. This path holds no pinned
    // skill's bytes, so a task that pins one refuses here rather than spawn
    // without it. A refusal is concluded like the context bundle's, under the
    // guard `documentation-bundle`.
    let documentation =
        statecraft_run::documentation::recorded_task(&chain, &run_id, session.attempt)
            .map_err(|why| {
                statecraft_adapter::documentation::BundleError::Task(format!(
                    "the recorded task: {why}"
                ))
            })
            .and_then(|recorded| {
                let b = recorded
                    .as_ref()
                    .map(|t| statecraft_adapter::documentation::Bundle::assemble(t, None))
                    .transpose()?;
                statecraft_adapter::documentation::check(
                    b.as_ref(),
                    recorded.as_ref(),
                    &manifest.supports,
                )?;
                Ok(b)
            });
    let documentation = match documentation {
        Ok(b) => b,
        Err(why) => {
            let why = why.to_string();
            let mut accounting = statecraft_run::refusal::Accounting::without_process();
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: "documentation-bundle".to_string(),
                detail: why.clone(),
            });
            return crate::attempt::conclude(
                clock,
                &mut chain,
                &places,
                &session,
                Some(&boundary_admission),
                statecraft_run::attempt::Outcome::Refused,
                &accounting,
                witness.marked(serde_json::json!({ "documentationRefusal": why })),
                unlaunched(project_manifest.is_some(), None),
                &contract,
                trial,
            );
        }
    };
    let request = statecraft_adapter::protocol::Request {
        context: bundle,
        documentation,
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
    // Spec 050 section 5, 2026-11-01: while the attempt runs, its supervisor
    // polls for halt requests queued while it holds the lock, appends them,
    // and stops the attempt a halt asks to stop. It borrows the chain for the
    // supervision only.
    let mut poller = HaltPoller::new(&mut chain, home, root, &exchange, &run_id, session.attempt);
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
                &mut witness.custody(&mut Polled {
                    inner: &mut kept,
                    poller: &mut poller,
                }),
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
                &mut witness.custody(&mut Polled {
                    inner: &mut *w,
                    poller: &mut poller,
                }),
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
            &mut witness.custody(&mut Polled {
                inner: &mut statecraft_adapter::supervisor::Unwatched,
                poller: &mut poller,
            }),
        ),
    };
    let halted = poller.finish();
    let watched = watch.map(|w| w.watched()).unwrap_or_default();
    // ... and again once the process has ended, before anything is appended.
    // The chain appends from the head it read at opening, so a change found
    // here ends the attempt's writes: it stays live (rule 2).
    let record_change = before
        .and_then(|before| {
            // The supervisor's own halt appends are compared with the bytes
            // it left, as its own launch records are (section 5, 2026-11-01).
            let mut wrote = watched.wrote.clone();
            wrote.extend(halted.wrote.clone());
            statecraft_run::tamper::Snapshot::take(home, root)
                .map(|after| after.changes_since(&before, &wrote))
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

    // Spec 019 sections 3.5 and 3.6: the adapter's delivery testimony is
    // appended beside the intended plan and never copied from it. A record
    // that cannot be written is a refusal, never a silent `not-reported`.
    if let Some(c) = &context {
        for observation in statecraft_adapter::context::observation(
            &supervised.events,
            supervised.stream_error.as_ref(),
        ) {
            if let Err(why) = statecraft_run::context::record_delivery(
                &mut chain,
                &c.manifest,
                &observation,
                &now(),
            ) {
                accounting.observe(statecraft_run::refusal::RefusalEvent {
                    guard: "context-delivery".to_string(),
                    detail: format!("the delivery observation could not be written: {why}"),
                });
            }
        }
    }
    if let Some(why) = witness.write_outcome(&mut chain, &now()) {
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: crate::witness::GUARD.to_string(),
            detail: format!("the witness outcome could not be written: {why}"),
        });
    }
    // Spec 050 section 3.4 and spec 003 section 3.4: an attempt a halt
    // stopped was cancelled by an operator, not interrupted; the provider's
    // own claim is kept beside it unchanged.
    let mut termination = execution.termination();
    if halted.stop.is_some() {
        termination.observed = statecraft_run::attempt::Outcome::Cancelled;
    }
    let mut detail = serde_json::json!({
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
    });
    if let Some(evidence) = halted.evidence() {
        detail["halt"] = evidence;
    }
    crate::attempt::conclude(
        clock,
        &mut chain,
        &places,
        &session,
        Some(&boundary_admission),
        termination,
        &accounting,
        witness.marked(detail),
        startup.run,
        &contract,
        trial,
    )
}

/// How often a supervisor reads the halt requests queued for it (spec 050
/// section 5, 2026-11-01): a quarter second, well under the time a person
/// takes to look at `run list` after `halt`, and a directory listing that
/// rarely holds anything, so the cost on a supervision loop that turns every
/// 10 ms is one listing in twenty-five turns.
pub const HALT_POLL: std::time::Duration = std::time::Duration::from_millis(250);

/// The halted decision the admission gate reads in place of the startup
/// decision once `tool-effects` is halted (spec 050 section 3.2). Anything but
/// `admitted` refuses, so the gate script is unchanged.
pub const HALTED_DECISION: &str = "halted";

/// What the supervisor's halt poll did, once the supervision has ended.
#[derive(Debug, Default)]
struct HaltPolled {
    /// The stop a halt asked of this attempt, which this supervisor acted on.
    stop: Option<statecraft_run::halt::Stop>,
    /// Whether `tool-effects` was halted, and what writing the halted
    /// decision into the exchange directory came to.
    tool_effects: Option<Result<(), String>>,
    /// Why polling ended without a halt: a request that does not read, a
    /// record that could not be written, or a record changed underneath.
    failure: Option<String>,
    /// The record's bytes as the supervisor's own appends left them, for the
    /// tamper comparison.
    wrote: Vec<(std::path::PathBuf, Vec<u8>)>,
}

impl HaltPolled {
    /// The outcome record's `halt` detail, where the poll did anything.
    fn evidence(&self) -> Option<serde_json::Value> {
        if self.stop.is_none() && self.failure.is_none() && self.wrote.is_empty() {
            return None;
        }
        Some(serde_json::json!({
            "stop": self.stop,
            "toolEffectsDecision": self.tool_effects.as_ref().map(|r| match r {
                Ok(()) => serde_json::json!("written"),
                Err(why) => serde_json::json!({ "failed": why }),
            }),
            "failure": self.failure,
        }))
    }
}

/// The supervisor's halt poll (spec 050 section 5, 2026-11-01). Holds the
/// attempt's chain for the length of the supervision; the repository lock is
/// held by the caller throughout.
struct HaltPoller<'c> {
    chain: &'c mut Chain,
    home: &'c std::path::Path,
    root: &'c std::path::Path,
    exchange: &'c std::path::Path,
    run_id: &'c str,
    attempt: u32,
    last: Option<std::time::Instant>,
    /// The record's bytes as this supervisor last saw or left them. A record
    /// that differs is not appended to: the tamper comparison reports it.
    expected: Option<Vec<u8>>,
    polled: HaltPolled,
    done: bool,
}

impl<'c> HaltPoller<'c> {
    fn new(
        chain: &'c mut Chain,
        home: &'c std::path::Path,
        root: &'c std::path::Path,
        exchange: &'c std::path::Path,
        run_id: &'c str,
        attempt: u32,
    ) -> Self {
        let expected = std::fs::read(statecraft_run::record::chain_path(home, root)).ok();
        Self {
            chain,
            home,
            root,
            exchange,
            run_id,
            attempt,
            last: None,
            expected,
            polled: HaltPolled::default(),
            done: false,
        }
    }

    fn poll(&mut self) -> statecraft_adapter::supervisor::Control {
        use statecraft_adapter::supervisor::Control;
        use statecraft_run::halt;
        if self.done {
            return Control::Continue;
        }
        let now = std::time::Instant::now();
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < HALT_POLL)
        {
            return Control::Continue;
        }
        self.last = Some(now);
        match halt::pending(self.home, self.root) {
            Ok(queued) if queued.is_empty() => return Control::Continue,
            Ok(_) => {}
            // A request that does not read may be a halt: the attempt is not
            // let run past it (section 3.3's fail-closed rule), and it is
            // stopped as interrupted, since no halt is recorded.
            Err(refusal) => return self.fail(refusal.to_string()),
        }
        let chain_path = statecraft_run::record::chain_path(self.home, self.root);
        if std::fs::read(&chain_path).ok() != self.expected {
            // Not appended to: the record changed under this supervisor, and
            // the comparison after the process ends reports it. The request
            // stays queued for the next writer.
            self.done = true;
            self.polled.failure = Some(
                "the run record changed during the attempt, so the queued halt request was not \
                 appended and stays queued"
                    .to_string(),
            );
            return Control::Continue;
        }
        let applied = halt::apply_pending(self.chain, self.home, self.root);
        if let Ok(bytes) = std::fs::read(&chain_path) {
            if Some(&bytes) != self.expected.as_ref() {
                self.polled.wrote = vec![(chain_path.clone(), bytes.clone())];
            }
            self.expected = Some(bytes);
        }
        match applied {
            Ok(_) => {}
            // The entry is durable and only its file was left behind. The
            // next writer removes it without appending it again, so the halt
            // stands and is acted on here; the leftover file is reported.
            Err(e @ statecraft_run::halt::HaltError::Queue { .. }) => {
                self.polled.failure = Some(e.to_string());
            }
            Err(e) => return self.fail(e.to_string()),
        }
        let state = match halt::fold(self.chain) {
            Ok(state) => state,
            Err(refusal) => return self.fail(refusal.to_string()),
        };
        let Some(stop) = state
            .stop_of(self.run_id, self.attempt)
            .filter(|s| s.confirmed_at.is_none())
            .cloned()
        else {
            // A dispatch halt, or a resume: recorded, and nothing to stop.
            return Control::Continue;
        };
        self.done = true;
        if let Some(h) = state.of(halt::Scope::ToolEffects) {
            self.polled.tool_effects = Some(write_halted_decision(self.exchange, h));
        }
        let why = format!(
            "halted by {} at {}: {}; the attempt is cancelled",
            stop.operator, stop.requested_at, stop.reason
        );
        self.polled.stop = Some(stop);
        Control::Stop(why)
    }

    fn fail(&mut self, why: String) -> statecraft_adapter::supervisor::Control {
        self.done = true;
        self.polled.failure = Some(why.clone());
        statecraft_adapter::supervisor::Control::Stop(why)
    }

    fn finish(self) -> HaltPolled {
        self.polled
    }
}

/// Replace the exchange copy of the decision with the halted one (spec 050
/// section 3.2; spec 004 section 3.18 rule 5: the child reads the exchange
/// directory and creates nothing in it). Written beside and renamed over, so
/// the gate reads the old decision or the new one, never a torn one. Only a
/// gated attempt whose decision is already published has a copy to replace:
/// before it, the gate holds every call anyway, and the launcher's own copy
/// is written once and refuses a file it did not write.
fn write_halted_decision(
    exchange: &std::path::Path,
    halted: &statecraft_run::halt::Halted,
) -> Result<(), String> {
    use std::io::Write;
    let copy = exchange.join(statecraft_home::launch::files::ADMISSION);
    if !exchange
        .join(statecraft_home::launch::files::GATE)
        .is_file()
    {
        return Err("no admission gate reads this attempt's exchange directory".to_string());
    }
    if !copy.is_file() {
        return Err(
            "no decision was published yet, so the gate already holds every tool call".to_string(),
        );
    }
    let line = format!(
        "{}\n",
        serde_json::json!({
            "decision": HALTED_DECISION,
            "scope": halted.scope,
            "operator": halted.operator,
            "reason": halted.reason,
            "at": halted.at,
        })
    );
    let partial = exchange.join(format!(".statecraft-halted-{}", std::process::id()));
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        file.write_all(line.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&partial, &copy)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(e.to_string());
    }
    // Once renamed, the gate reads the halted decision on its next call. The
    // directory sync only keeps the rename across a host crash, which no live
    // attempt survives, so its failure does not undo what is already in force.
    let _ = std::fs::File::open(exchange).and_then(|d| d.sync_all());
    Ok(())
}

/// The attempt's watch with the halt poll beside it: every call is forwarded
/// unchanged, and the poll runs after the inner watch's own.
struct Polled<'a, 'c> {
    inner: &'a mut dyn statecraft_adapter::supervisor::Watch<(
        usize,
        statecraft_adapter_claude_code::stream::ProviderEvent,
    )>,
    poller: &'a mut HaltPoller<'c>,
}

impl
    statecraft_adapter::supervisor::Watch<(
        usize,
        statecraft_adapter_claude_code::stream::ProviderEvent,
    )> for Polled<'_, '_>
{
    fn spawned(&mut self, pid: u32) -> Result<(), String> {
        self.inner.spawned(pid)
    }

    fn event(
        &mut self,
        event: &(usize, statecraft_adapter_claude_code::stream::ProviderEvent),
    ) -> statecraft_adapter::supervisor::Control {
        self.inner.event(event)
    }

    fn poll(&mut self) -> statecraft_adapter::supervisor::Control {
        match self.inner.poll() {
            statecraft_adapter::supervisor::Control::Continue => self.poller.poll(),
            stop => stop,
        }
    }
}
