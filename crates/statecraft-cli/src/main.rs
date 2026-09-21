//! The `statecraft-cli` executable.
//!
//! Spec 006. This file is deliberately thin: it parses, dispatches to one
//! binding, prints one rendering, and exits with the code that binding chose.
//! Every decision worth testing lives in the library beside it, which is why
//! `tests/` can cover the command surface without spawning a process for the
//! cases that do not need one.

use statecraft_cli::adapters;
use statecraft_cli::bind;
use statecraft_cli::commands::{Verb, parse};
use statecraft_cli::exit::Exit;
use statecraft_cli::product_home;
use statecraft_cli::render::{Answer, Format};
use statecraft_cli::slice;
use statecraft_environment::claimant::{ForeignClaims, UnobservedShadows};
use statecraft_environment::manifest::Manifest;
use statecraft_environment::probe::CommandProbe;
use statecraft_environment::registry::Registry;
use statecraft_environment::time::SystemClock;
use statecraft_run::policy::{NoDeclarationFiled, Overrides};
use statecraft_run::record::Chain;
use statecraft_run::report::{ReportSource, SpecSpineCli};
use std::path::PathBuf;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args);
    std::process::ExitCode::from(code as u8)
}

fn run(args: &[String]) -> i32 {
    let invocation = match parse(args) {
        Ok(i) => i,
        Err(e) => {
            // Usage errors go to stderr: a caller piping `--json` into a parser
            // should not have to filter a help text out of its input.
            eprint!("{}", e.describe());
            return Exit::Usage.code();
        }
    };
    let format = if invocation.json {
        Format::Json
    } else {
        Format::Human
    };

    let home = product_home();
    let mut registry = match Registry::read(&home) {
        Ok(r) => r,
        Err(e) => return fail(&e.to_string(), format),
    };

    match invocation.verb {
        Verb::ProjectRegister => {
            let Some(path) = invocation.rest.first() else {
                eprintln!("usage: project register <path>");
                return Exit::Usage.code();
            };
            let path = absolute(path);
            let probe = CommandProbe::default();
            match bind::project_register(&mut registry, &path, &probe) {
                Ok(answer) => {
                    if let Err(e) = registry.write(&home) {
                        return fail(&e.to_string(), format);
                    }
                    emit(&answer, format)
                }
                Err(answer) => emit(&answer, format),
            }
        }
        Verb::ProjectList => emit(&bind::project_list(&registry), format),
        Verb::ProjectArm | Verb::ProjectDisarm => {
            let Some(path) = invocation.rest.first() else {
                eprintln!("usage: {} <path>", invocation.verb.spelling());
                return Exit::Usage.code();
            };
            let armed = invocation.verb == Verb::ProjectArm;
            match bind::project_set_armed(&mut registry, &absolute(path), armed) {
                Ok(answer) => {
                    if let Err(e) = registry.write(&home) {
                        return fail(&e.to_string(), format);
                    }
                    emit(&answer, format)
                }
                Err(answer) => emit(&answer, format),
            }
        }
        // The environment verbs have a configured adapter set as of spec 008,
        // which ratified the first provider adapter. Until then they refused
        // for want of one, and the refusal was correct rather than a stub: the
        // reason it stated is what spec 008 falsified, so the binding changed
        // in the same change.
        Verb::EnvPlan | Verb::EnvApply | Verb::EnvUpgrade | Verb::EnvRemove | Verb::Doctor => {
            let Some(path) = invocation.rest.first() else {
                eprintln!("usage: {} <path>", invocation.verb.spelling());
                return Exit::Usage.code();
            };
            let root = absolute(path);
            // Every environment verb needs a registered target. A precondition,
            // so a refusal (2) naming the path, which is spec 006 section 3.7.
            if registry.get(&root).is_none() {
                return emit(&bind::unregistered_answer(&root), format);
            }
            environment_verb(invocation.verb, &root, &home, format)
        }
        // Spec 006's edges: the verbs 003, 004 and 005 name, bound here for the
        // first time. Same precondition as the environment verbs, for the same
        // reason: this product works in registered targets.
        Verb::WorkList
        | Verb::WorkShow
        | Verb::Run
        | Verb::RunList
        | Verb::RunShow
        | Verb::Accept => {
            let Some(path) = invocation.rest.first() else {
                eprintln!(
                    "usage: {} <path>{}",
                    invocation.verb.spelling(),
                    argument_hint(invocation.verb)
                );
                return Exit::Usage.code();
            };
            let root = absolute(path);
            let Some(registration) = registry.get(&root) else {
                return emit(&bind::unregistered_answer(&root), format);
            };
            // Registration makes a target visible; arming is what consents to
            // it being driven (spec 002 section 3.1). `run` is the one verb
            // here that drives, so it is the one verb this gates: discovery
            // and inspection read, and reading an unarmed target was always
            // the point of registering one.
            //
            // Placed here rather than inside `run_verb` because the consent is
            // a property of the registration, which is read here and only
            // here, and because refusing before the corpus report is what
            // keeps the refusal ahead of every effect: no workspace, no
            // appended attempt, no spawned provider.
            if invocation.verb == Verb::Run && !registration.armed {
                return emit(&bind::unarmed_answer(&root), format);
            }
            slice_verb(invocation.verb, &root, &home, &invocation.rest[1..], format)
        }
        // Spec 010's verbs. Three of them take no path at all: the `home`
        // verbs are about the product's own home and not about a target, and
        // requiring one would be requiring a project in order to look at the
        // environment that exists before any project does.
        Verb::HomeShow | Verb::HomePlan | Verb::HomeApply => {
            manage_verb(invocation.verb, &invocation.rest, None, &home, format)
        }
        // The rest take a path, and deliberately NOT a registered one:
        // `init apply` is what makes a project registrable, so a registration
        // precondition here would be the bootstrap cycle spec 002 section 3.17
        // avoids by ordering.
        Verb::InitPlan
        | Verb::InitApply
        | Verb::MigratePlan
        | Verb::MigrateApply
        | Verb::ProjectEnroll
        | Verb::ProjectUnenroll
        | Verb::ConfigShow
        | Verb::ApprovalGrant
        | Verb::ApprovalShow => {
            let Some(path) = invocation.rest.first() else {
                eprintln!(
                    "usage: {}{}",
                    invocation.verb.spelling(),
                    statecraft_cli::manage::usage(invocation.verb)
                );
                return Exit::Usage.code();
            };
            let root = absolute(path);
            manage_verb(
                invocation.verb,
                &invocation.rest[1..],
                Some(&root),
                &home,
                format,
            )
        }
        // A help request is not an operation, so it consults nothing and
        // changes nothing. Exit 0: the question was asked and answered.
        Verb::Help => {
            print!("{}", statecraft_cli::commands::help_text(&invocation.rest));
            Exit::Ok.code()
        }
    }
}

/// One spec 010 verb, against one product home.
///
/// The whole binding: name the operation, call the one boundary, render what
/// it returned. The operation's severity is decided there (spec 010 section
/// 3.10), so nothing here decides what a partial initialization means.
fn manage_verb(
    verb: Verb,
    rest: &[String],
    root: Option<&std::path::Path>,
    home: &std::path::Path,
    format: Format,
) -> i32 {
    let Some(operation) = statecraft_cli::manage::operation(verb, rest, root) else {
        eprintln!(
            "usage: {}{}",
            verb.spelling(),
            statecraft_cli::manage::usage(verb)
        );
        return Exit::Usage.code();
    };
    emit(&statecraft_cli::manage::execute(home, operation), format)
}

/// What each work, run and accept verb needs after the target path.
fn argument_hint(verb: Verb) -> &'static str {
    match verb {
        Verb::WorkShow => " <spec-id>",
        Verb::Run => " <spec-id>",
        Verb::RunShow | Verb::Accept => " <run-id>",
        _ => "",
    }
}

/// One `work`, `run` or `accept` verb against one registered target.
///
/// Each arm calls entry points the owning crates expose and maps what they
/// return. Spec 006 section 3.11: nothing here derives an answer an owning crate
/// could have returned.
fn slice_verb(
    verb: Verb,
    root: &std::path::Path,
    home: &std::path::Path,
    rest: &[String],
    format: Format,
) -> i32 {
    // Discovery is the join spec 003 section 3.1.1 prescribes, performed by the
    // crate that owns it. Both halves come from spec-spine's structured output.
    let needs_report = matches!(verb, Verb::WorkList | Verb::WorkShow | Verb::Run);
    let work = if needs_report {
        match SpecSpineCli::default().corpus_report(root) {
            Ok(report) => {
                let (policy, disagreement) =
                    statecraft_run::policy::resolve(root, &NoDeclarationFiled, None);
                Some(statecraft_run::work::select(
                    &report,
                    &policy,
                    &Overrides::none(),
                    disagreement,
                ))
            }
            Err(e) => return emit(&slice::report_error_answer(&e), format),
        }
    } else {
        None
    };

    match verb {
        Verb::WorkList => emit(&slice::work_list_answer(work.expect("read above")), format),
        Verb::WorkShow => {
            let Some(id) = rest.first() else {
                eprintln!("usage: work show <path> <spec-id>");
                return Exit::Usage.code();
            };
            emit(
                &slice::work_show_answer(work.expect("read above").eligibility_of(id)),
                format,
            )
        }
        Verb::Run => {
            let Some(id) = rest.first() else {
                eprintln!("usage: run <path> <spec-id>");
                return Exit::Usage.code();
            };
            run_verb(root, home, id, work.expect("read above"), format)
        }
        Verb::RunList => match Chain::open(home, root) {
            Ok((chain, _)) => emit(
                &slice::run_list_answer(statecraft_run::session::runs(&chain)),
                format,
            ),
            Err(e) => fail(&e.to_string(), format),
        },
        Verb::RunShow => {
            let Some(run_id) = rest.first() else {
                eprintln!("usage: run show <path> <run-id>");
                return Exit::Usage.code();
            };
            match Chain::open(home, root) {
                Ok((chain, _)) => {
                    if !statecraft_run::session::runs(&chain)
                        .iter()
                        .any(|r| r.id == *run_id)
                    {
                        return emit(&slice::no_such_run_answer(run_id), format);
                    }
                    emit(
                        &slice::run_show_answer(statecraft_acceptance::suite::fold(
                            run_id,
                            &chain.entries(),
                        )),
                        format,
                    )
                }
                Err(e) => fail(&e.to_string(), format),
            }
        }
        Verb::Accept => {
            let Some(run_id) = rest.first() else {
                eprintln!("usage: accept <path> <run-id>");
                return Exit::Usage.code();
            };
            accept_verb(root, home, run_id, format)
        }
        _ => Exit::Usage.code(),
    }
}

/// `run <path> <spec-id>`
///
/// The three calls spec 006 section 3.11 leaves to the caller, in order: begin,
/// supervise, conclude. The supervision is the adapter's; the other two are
/// `statecraft_run::session`'s.
fn run_verb(
    root: &std::path::Path,
    home: &std::path::Path,
    spec_id: &str,
    work: statecraft_run::work::WorkList,
    format: Format,
) -> i32 {
    // A unit of work the policy did not admit is never run, and the reason is
    // the answer (spec 003 section 3.1.1).
    let eligibility = work.eligibility_of(spec_id);
    if !eligibility.schedulable() {
        return emit(&slice::work_show_answer(eligibility), format);
    }

    let (mut chain, _) = match Chain::open(home, root) {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string(), format),
    };

    // The run id is the spec id: spec 006 section 3.1 says the operator names a
    // unit of work, and spec 003 section 3.4 makes a retry an appended attempt
    // of the same run, so a fresh id per invocation would turn every retry into
    // a new run.
    let run_id = spec_id.to_string();
    let session =
        match statecraft_run::session::begin(&mut chain, root, &run_id, "HEAD", &SystemClock) {
            Ok(s) => s,
            Err(e) => return emit(&slice::session_error_answer(&e), format),
        };

    // Preflight refuses before any process is created, naming the token (spec
    // 004 section 3.3). The manifest is read here and only here.
    let manifest = statecraft_adapter_claude_code::manifest();
    let environment = adapters::child_environment();
    let requested = statecraft_adapter::capability::Requested::none()
        .requiring(statecraft_adapter::capability::Capability::StructuredRefusals)
        .preferring(statecraft_adapter::capability::Capability::TurnLimit)
        .preferring(statecraft_adapter::capability::Capability::CostReport);

    let mut posture = statecraft_adapter::posture::Posture::new(
        &manifest,
        statecraft_adapter::manifest::Qualification::Unqualified,
        &requested,
        &statecraft_adapter::capability::negotiate(&requested, &manifest.supports),
        &[],
        &environment,
    );
    let negotiation = match statecraft_adapter::supervisor::preflight(
        &manifest,
        &requested,
        &environment,
    ) {
        Ok(n) => n,
        Err(refusal) => {
            // No process was created, so the attempt is concluded as
            // refused rather than left live: an intent with no outcome
            // would send the next run to reconciliation for something that
            // never started.
            let mut accounting = statecraft_run::refusal::Accounting::default();
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: "adapter-preflight".to_string(),
                detail: refusal.to_string(),
            });
            return conclude_and_emit(
                &mut chain,
                root,
                &session,
                statecraft_run::attempt::Outcome::Refused,
                &accounting,
                serde_json::json!({ "preflightRefusal": refusal.to_string(), "posture": posture }),
                format,
            );
        }
    };

    let probe = adapters::probe(home);
    posture.qualification = probe.qualification();
    let Some(program) = probe.resolved_executable() else {
        let mut accounting = statecraft_run::refusal::Accounting::default();
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: "constructed-environment".to_string(),
            detail: "the provider executable does not resolve on the child's PATH".to_string(),
        });
        return conclude_and_emit(
            &mut chain,
            root,
            &session,
            statecraft_run::attempt::Outcome::Refused,
            &accounting,
            serde_json::json!({ "preflightRefusal": "provider executable unresolvable", "posture": posture }),
            format,
        );
    };

    let invocation =
        statecraft_adapter_claude_code::Invocation::new(&program.display().to_string(), &[], None);
    let request = statecraft_adapter::protocol::Request {
        workspace: session.workspace.path.clone(),
        base_commit: session.workspace.base_commit.clone(),
        prompt: format!("Implement {spec_id} in this workspace.").into_bytes(),
        capabilities: requested.clone(),
        deadline_seconds: 900,
        attempt: statecraft_adapter::protocol::AttemptIdentity {
            run_id: run_id.clone(),
            number: session.attempt,
        },
    };

    let execution = match statecraft_adapter_claude_code::execution::supervise(
        &invocation,
        &request,
        &environment,
        &negotiation.granted,
    ) {
        Ok(s) => s,
        Err(e) => {
            let mut accounting = statecraft_run::refusal::Accounting::default();
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: "supervisor".to_string(),
                detail: e.to_string(),
            });
            return conclude_and_emit(
                &mut chain,
                root,
                &session,
                statecraft_run::attempt::Outcome::Interrupted,
                &accounting,
                serde_json::json!({ "supervisorError": e.to_string(), "posture": posture }),
                format,
            );
        }
    };

    let supervised = &execution.supervised;
    let posture = posture.with_execution(supervised);
    let mut accounting = statecraft_run::refusal::Accounting::default();
    for refusal in statecraft_adapter::protocol::refusals(&supervised.events) {
        accounting.observe(refusal);
    }

    conclude_and_emit(
        &mut chain,
        root,
        &session,
        execution.termination(),
        &accounting,
        serde_json::json!({
            "requested": negotiation
                .granted
                .iter()
                .map(|c| c.token())
                .collect::<Vec<_>>(),
            "applied": posture.applied.iter().map(|c| c.token()).collect::<Vec<_>>(),
            "posture": posture,
            "degraded": negotiation.degraded.iter().map(|c| c.token()).collect::<Vec<_>>(),
            "specId": spec_id,
            "execution": execution.evidence(),
        }),
        format,
    )
}

fn conclude_and_emit(
    chain: &mut Chain,
    root: &std::path::Path,
    session: &statecraft_run::session::Session,
    termination: impl Into<statecraft_run::session::Termination>,
    accounting: &statecraft_run::refusal::Accounting,
    detail: serde_json::Value,
    format: Format,
) -> i32 {
    match statecraft_run::session::conclude_observed(
        chain,
        root,
        session,
        termination.into(),
        accounting,
        detail,
        &SystemClock,
    ) {
        Ok(concluded) => {
            let account = statecraft_acceptance::suite::fold(&session.run_id, &chain.entries());
            emit(
                &slice::run_answer_with_posture(concluded, account.posture),
                format,
            )
        }
        Err(e) => emit(&slice::session_error_answer(&e), format),
    }
}

/// `accept <path> <run-id>`
fn accept_verb(
    root: &std::path::Path,
    home: &std::path::Path,
    run_id: &str,
    format: Format,
) -> i32 {
    let (chain, _) = match Chain::open(home, root) {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string(), format),
    };
    let Some(run) = statecraft_run::session::runs(&chain)
        .into_iter()
        .find(|r| r.id == run_id)
    else {
        return emit(&slice::no_such_run_answer(run_id), format);
    };
    let Some(attempt) = run.attempts.last().cloned() else {
        return emit(&slice::no_such_run_answer(run_id), format);
    };

    // Exactly one of spec 003's five outcomes is eligible, and the other four
    // are recorded reasons rather than blanks (spec 005 section 3.1.1).
    let Some(outcome) = attempt.outcome else {
        return emit(
            &slice::accept_answer(statecraft_acceptance::outcome::Acceptance::None {
                reason: statecraft_acceptance::judged::NoAcceptance::CandidateUnidentified {
                    detail: format!(
                        "run {run_id} attempt {} is live, so nothing identifies stable candidate bytes",
                        attempt.number
                    ),
                },
            }),
            format,
        );
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
        return emit(
            &slice::accept_answer(statecraft_acceptance::outcome::Acceptance::NotAttempted {
                reason,
                refusal_count,
            }),
            format,
        );
    }

    let workspace = statecraft_run::workspace::workspace_path(root, run_id);
    let context = statecraft_cli::accept::Context {
        repository: root.display().to_string(),
        spec_spine_version: adapters::observed()
            .spec_spine
            .unwrap_or_else(|| "not-recorded".to_string()),
        adapter_version: statecraft_adapter_claude_code::manifest().version,
        attempt: format!("{run_id}/{}", attempt.number),
        spec_id: run_id.to_string(),
    };
    let (acceptance, _suite, _freshness) = statecraft_cli::accept::judge(
        root,
        &workspace,
        &attempt.base_commit,
        // The attempt concluded `completed`, which spec 003 section 3.8 makes
        // impossible when the base moved: a moved base is `interrupted`.
        true,
        &context,
    );
    emit(&slice::accept_answer(acceptance), format)
}

/// One environment verb against one registered target.
///
/// Each arm calls exactly one library operation and maps what it returns, which
/// is spec 006 section 3.2. The adapter set and the probe come from
/// [`statecraft_cli::adapters`]; nothing here decides what an adapter declares.
fn environment_verb(
    verb: Verb,
    root: &std::path::Path,
    home: &std::path::Path,
    format: Format,
) -> i32 {
    let declarations = adapters::declarations();
    let probe = adapters::probe(home);
    let foreign = ForeignClaims::none();
    let clock = SystemClock;

    // `env remove` reads the manifest itself and refuses when there is none, so
    // it is the one verb that does not need one read here.
    if verb == Verb::EnvRemove {
        return match statecraft_environment::apply::remove(root, &clock) {
            Ok(outcome) => emit(&bind::outcome_answer(outcome), format),
            Err(e) => emit(&bind::apply_error_answer(&e), format),
        };
    }

    let manifest = match Manifest::read(root) {
        Ok(m) => m,
        // An unreadable manifest is a failure (4), never a refusal: spec 006
        // section 3.3 names this case explicitly.
        Err(e) => return fail(&e.to_string(), format),
    };

    match verb {
        Verb::EnvPlan => match statecraft_environment::plan::plan(
            root,
            manifest.as_ref(),
            &declarations,
            &probe,
            &foreign,
        ) {
            Ok(plan) => emit(&bind::plan_answer(plan), format),
            Err(e) => emit(&bind::plan_error_answer(root, &e), format),
        },
        // `env upgrade` is `env apply` against the current declarations, which
        // is spec 002 section 3.6's own position: the two verbs are separated
        // for the operator, not for the machine, and giving them different code
        // paths is how their conflict rules would drift.
        Verb::EnvApply | Verb::EnvUpgrade => {
            let mut manifest = manifest.unwrap_or_else(|| Manifest::new(adapters::pins()));
            match statecraft_environment::apply::apply(
                root,
                &mut manifest,
                &declarations,
                &probe,
                &foreign,
                &clock,
            ) {
                Ok(outcome) => {
                    if !outcome.refused() {
                        if let Err(e) = manifest.write(root) {
                            return fail(&e.to_string(), format);
                        }
                    }
                    emit(&bind::outcome_answer(outcome), format)
                }
                Err(e) => emit(&bind::apply_error_answer(&e), format),
            }
        }
        Verb::Doctor => {
            let manifest = manifest.unwrap_or_else(|| Manifest::new(adapters::pins()));
            match statecraft_environment::doctor::doctor(
                root,
                &manifest,
                &declarations,
                &probe,
                &foreign,
                &UnobservedShadows,
                &adapters::observed(),
            ) {
                Ok(report) => emit(&bind::doctor_answer(report), format),
                Err(e) => emit(&bind::plan_error_answer(root, &e), format),
            }
        }
        // Handled above, and by the four `project` verbs in `run`.
        _ => Exit::Usage.code(),
    }
}

fn emit<T: serde::Serialize>(answer: &Answer<T>, format: Format) -> i32 {
    print!("{}", answer.render(format));
    answer.exit.code()
}

fn fail(detail: &str, format: Format) -> i32 {
    let answer = Answer::new(detail.to_string(), Exit::Failed, detail);
    print!("{}", answer.render(format));
    Exit::Failed.code()
}

/// Make a path absolute without touching the filesystem's opinion of it.
///
/// Spec 002 refuses a relative path outright, so this exists only to turn what
/// an operator typed into what the register stores. It does not canonicalize:
/// resolving symlinks would record a path the operator did not name.
fn absolute(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        return p;
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(p),
        Err(_) => p,
    }
}
