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
use statecraft_run::policy::NoDeclarationFiled;
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
            // Spec 003 section 5 (2026-09-23): a re-registration keeps the
            // stored spelling, except where the stored spelling has no records
            // and the typed one has, which is the one case where re-storing it
            // points the registration back at its only history. Held under the
            // stored root's lock, so no run appends while the key moves.
            // The lock is taken first and held until the registry is written,
            // and the condition is decided again under it.
            let records = home.join("records");
            let mut held = None;
            let respelled = match registry.get(&path).map(|r| r.root.clone()) {
                Some(stored)
                    if statecraft_run::repository::respell_allowed(&records, &stored, &path) =>
                {
                    held = Some(match statecraft_run::lock::try_acquire(&home, &stored) {
                        Ok(held) => held,
                        Err(e @ statecraft_run::lock::LockError::Busy { .. }) => {
                            return emit(&slice::lock_busy_answer(&e.to_string()), format);
                        }
                        Err(e) => return fail(&e.to_string(), format),
                    });
                    if statecraft_run::repository::respell_allowed(&records, &stored, &path) {
                        if let Err(e) = registry.respell(&path) {
                            return fail(&e.to_string(), format);
                        }
                        Some(stored)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let probe = CommandProbe::default();
            match bind::project_register(&mut registry, &path, &probe) {
                Ok(mut answer) => {
                    if let Err(e) = registry.write(&home) {
                        return fail(&e.to_string(), format);
                    }
                    drop(held);
                    if let Some(stored) = respelled {
                        answer.summary.push_str(&format!(
                            "\nre-stored from {} as {}: the records filed under this spelling \
                             are read again, and the previous spelling had none\n",
                            stored.display(),
                            path.display()
                        ));
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
            // Spec 002 section 3.4: a drifted managed file is replaced only
            // when the operator names it, so `--replace` is parsed strictly and
            // only where a verb can carry it.
            let consenting = matches!(invocation.verb, Verb::EnvApply | Verb::EnvUpgrade);
            // `doctor --remote [--head <sha>]` (spec 002 section 5, 2026-09-24,
            // the setup-profile entry): a read-only verification against the
            // host, asked for explicitly and never performed otherwise.
            let (remote, rest) = if invocation.verb == Verb::Doctor {
                match bind::remote_arguments(&invocation.rest[1..]) {
                    Ok(split) => split,
                    Err(detail) => {
                        eprintln!("usage: {detail}");
                        return Exit::Usage.code();
                    }
                }
            } else {
                (None, invocation.rest[1..].to_vec())
            };
            let named = match bind::replace_arguments(consenting, &rest) {
                Ok(named) => named,
                Err(detail) => {
                    eprintln!("usage: {detail}");
                    return Exit::Usage.code();
                }
            };
            if !named.is_empty() && matches!(invocation.verb, Verb::EnvRemove | Verb::Doctor) {
                eprintln!(
                    "usage: {} does not take --replace; name a path to `env plan`, then consent to it with `env apply`",
                    invocation.verb.spelling()
                );
                return Exit::Usage.code();
            }
            environment_verb(invocation.verb, &root, &home, &named, remote, format)
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
            let root = match registered(&registry, &absolute(path)) {
                Ok(root) => root,
                Err(answer) => return emit(&answer, format),
            };
            let registration = registry.get(&root).expect("resolved above");
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
        // `session payload` joins them for the same reason (spec 006 section
        // 3.11.1): the payload is a property of this build, and requiring a
        // project in order to print it would be requiring a target in order to
        // read a constant.
        Verb::HomeShow | Verb::HomePlan | Verb::HomeApply | Verb::SessionPayload => {
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
        | Verb::ApprovalShow
        | Verb::HarnessShow
        | Verb::HarnessUpgrade
        | Verb::StartupRecord
        | Verb::StartupCapture
        | Verb::StartupQualify => {
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
        // Spec 006 section 3.11.4. The trial drives a session, so it has
        // `run`'s preconditions: a registered and armed target.
        Verb::StartupTrial => {
            let Some(path) = invocation.rest.first() else {
                eprintln!(
                    "usage: startup trial{}",
                    statecraft_cli::manage::usage(invocation.verb)
                );
                return Exit::Usage.code();
            };
            let root = match registered(&registry, &absolute(path)) {
                Ok(root) => root,
                Err(answer) => return emit(&answer, format),
            };
            let registration = registry.get(&root).expect("resolved above");
            if !registration.armed {
                return emit(&bind::unarmed_answer(&root), format);
            }
            trial_verb(&root, &home, &invocation.rest[1..], format)
        }
        // Spec 006 section 3.11.3. A read: the run record says which attempts
        // exist and how they ended, and the library judges their startup
        // records. Registration is not required, as for the other `startup`
        // verbs; a manifest is, and the library refuses without one.
        Verb::StartupShow => {
            let (Some(path), Some(run_id)) = (invocation.rest.first(), invocation.rest.get(1))
            else {
                eprintln!(
                    "usage: startup show{}",
                    statecraft_cli::manage::usage(invocation.verb)
                );
                return Exit::Usage.code();
            };
            let attempt = match invocation.rest.get(2..).unwrap_or_default() {
                [] => None,
                [flag, n] if flag == "--attempt" => match n.parse::<u32>() {
                    Ok(n) if n > 0 => Some(n),
                    _ => {
                        eprintln!("usage: startup show <path> <run-id> [--attempt <n>]");
                        return Exit::Usage.code();
                    }
                },
                _ => {
                    eprintln!("usage: startup show <path> <run-id> [--attempt <n>]");
                    return Exit::Usage.code();
                }
            };
            // Not a registration precondition, as above; but where the path
            // is registered, its records are filed under the stored root.
            let root = match stored_or_typed(&registry, &absolute(path)) {
                Ok(root) => root,
                Err(answer) => return emit(&answer, format),
            };
            let facts = match Chain::open(&home, &root) {
                Ok((chain, _)) => statecraft_run::session::runs(&chain)
                    .into_iter()
                    .find(|r| r.id == *run_id)
                    .map(|r| {
                        r.attempts
                            .iter()
                            .map(|a| statecraft_home::launch::AttemptFact {
                                number: a.number,
                                outcome: a.outcome.map(|o| o.word().to_string()),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                Err(e) => return fail(&e.to_string(), format),
            };
            emit(
                &statecraft_cli::manage::execute(
                    &home,
                    statecraft_home::service::Operation::StartupShow {
                        root,
                        run_id: run_id.clone(),
                        attempt,
                        facts,
                    },
                ),
                format,
            )
        }
        // Spec 006 section 3.11.5: the readiness override of 003 section
        // 3.1.4. A registered target, as for the work verbs; the journal is
        // the product's own state, so arming is not required to change it.
        Verb::OverrideGrant | Verb::OverrideRevoke | Verb::OverrideShow => {
            let usage = || {
                eprintln!(
                    "usage: {} <path>{}",
                    invocation.verb.spelling(),
                    if invocation.verb == Verb::OverrideShow {
                        ""
                    } else {
                        " <spec-id> <operator> <reason...>"
                    }
                );
                Exit::Usage.code()
            };
            let Some(path) = invocation.rest.first() else {
                return usage();
            };
            let root = match registered(&registry, &absolute(path)) {
                Ok(root) => root,
                Err(answer) => return emit(&answer, format),
            };
            if invocation.verb == Verb::OverrideShow {
                if invocation.rest.len() != 1 {
                    return usage();
                }
                return match statecraft_run::overrides::read(&home, &root) {
                    Ok(journal) => emit(
                        &slice::override_show_answer(&root.display().to_string(), &journal),
                        format,
                    ),
                    Err(e) => fail(&e.to_string(), format),
                };
            }
            let (Some(spec_id), Some(operator)) = (invocation.rest.get(1), invocation.rest.get(2))
            else {
                return usage();
            };
            let reason = invocation.rest.get(3..).unwrap_or_default().join(" ");
            let at = statecraft_environment::time::rfc3339_utc(
                statecraft_environment::time::Clock::now_unix(&SystemClock),
            );
            let request = statecraft_run::overrides::Request {
                home: &home,
                target: &root,
                spec_id,
                operator,
                reason: &reason,
                at: &at,
            };
            if invocation.verb == Verb::OverrideRevoke {
                return emit(
                    &slice::override_change_answer(
                        "revoked",
                        statecraft_run::overrides::revoke(&request),
                    ),
                    format,
                );
            }
            // Rule 1: the spec id must be one the corpus report names. The
            // report is spec-spine's, read the way `work list` reads it.
            let known = match SpecSpineCli::default().corpus_report(&root) {
                Ok(report) => report.lifecycle_of(spec_id).is_some(),
                Err(e) => return emit(&slice::report_error_answer(&e), format),
            };
            emit(
                &slice::override_change_answer(
                    "granted",
                    statecraft_run::overrides::grant(&request, known),
                ),
                format,
            )
        }
        // Spec 006 section 3.11.6: the reconciliation of 003 section 3.6.1.
        Verb::RunReconcile => reconcile_verb(&invocation.rest, &registry, &home, format),
        // Spec 006 section 3.11.7: the transfer verbs of 002 section 3.35. A
        // registered target, as for the environment verbs whose manifest they
        // change; usage is 3, everything else is the library's answer.
        Verb::TransferPlan | Verb::TransferApply | Verb::TransferRevert => {
            let request =
                match statecraft_cli::transfer::parse(invocation.verb, &invocation.rest, |p| {
                    absolute(p)
                }) {
                    Ok(r) => r,
                    Err(why) => {
                        eprintln!("{why}");
                        return Exit::Usage.code();
                    }
                };
            if registry.get(request.root()).is_none() {
                return emit(&bind::unregistered_answer(request.root()), format);
            }
            emit(&statecraft_cli::transfer::execute(&request, &home), format)
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
    let (work, report) = if needs_report {
        match SpecSpineCli::default().corpus_report(root) {
            Ok(report) => {
                let (policy, disagreement) =
                    statecraft_run::policy::resolve(root, &NoDeclarationFiled, None);
                // Spec 003 section 3.1.4 rule 3: a journal that does not read
                // or verify is a failure, never an empty set of overrides.
                let overrides = match statecraft_run::overrides::read(home, root) {
                    Ok(journal) => journal.overrides(),
                    Err(e) => return fail(&e.to_string(), format),
                };
                (
                    Some(statecraft_run::work::select(
                        &report,
                        &policy,
                        &overrides,
                        disagreement,
                    )),
                    Some(report),
                )
            }
            Err(e) => return emit(&slice::report_error_answer(&e), format),
        }
    } else {
        (None, None)
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
            run_verb(
                root,
                home,
                id,
                work.expect("read above"),
                report.as_ref().expect("read above"),
                format,
            )
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
                    let Some(run) = statecraft_run::session::runs(&chain)
                        .into_iter()
                        .find(|r| r.id == *run_id)
                    else {
                        return emit(&slice::no_such_run_answer(run_id), format);
                    };
                    emit(
                        &slice::run_show_with_admissions(
                            statecraft_acceptance::suite::fold(run_id, &chain.entries()),
                            &run,
                            slice::ReconciliationView::of_run(&chain.positioned_entries(), run_id),
                        ),
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

/// `run reconcile <path> <run-id> <attempt> <finding> <launch-state> <operator> <reason...>`
///
/// Spec 006 section 3.11.6 and spec 003 section 3.6.1. The binding reads the
/// launch records through spec 002's `launch::inspect`, which only reads, and
/// passes what it observed to the run crate, which decides and records.
fn reconcile_verb(
    rest: &[String],
    registry: &statecraft_environment::registry::Registry,
    home: &std::path::Path,
    format: Format,
) -> i32 {
    use statecraft_run::reconcile::{EvidenceFile, LaunchState, Observation, Request};
    use statecraft_run::recovery::Verdict;
    let usage = || {
        eprintln!(
            "usage: run reconcile <path> <run-id> <attempt> <confirmed|absent|unknown> \
             <launch-state> <operator> <reason...> [--evidence <file>]..."
        );
        Exit::Usage.code()
    };
    let mut positional: Vec<&String> = Vec::new();
    let mut evidence_paths: Vec<&String> = Vec::new();
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        if arg == "--evidence" {
            match args.next() {
                Some(p) => evidence_paths.push(p),
                None => return usage(),
            }
        } else {
            positional.push(arg);
        }
    }
    if positional.len() < 7 {
        return usage();
    }
    // Filed under the stored root, like every other verb that keys a record,
    // so another spelling of a registered path reads the same chain.
    let root = match stored_or_typed(registry, &absolute(positional[0])) {
        Ok(root) => root,
        Err(answer) => return emit(&answer, format),
    };
    let run_id = positional[1].as_str();
    let Ok(attempt) = positional[2].parse::<u32>() else {
        return usage();
    };
    let finding = match positional[3].as_str() {
        "confirmed" => Verdict::Confirmed,
        "absent" => Verdict::Absent,
        "unknown" => Verdict::Unknown,
        _ => return usage(),
    };
    let inspected = positional[4].as_str();
    if LaunchState::from_word(inspected).is_none() {
        return usage();
    }
    let operator = positional[5].as_str();
    let reason = positional[6..]
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if registry.get(&root).is_none() {
        return emit(&bind::unregistered_answer(&root), format);
    }
    let mut evidence = Vec::new();
    for p in evidence_paths {
        // Hashed as it is read, never held whole: its content is not copied and
        // not read for a decision (spec 003 section 3.6.1 rule 2).
        match std::fs::File::open(p).and_then(statecraft_environment::digest::digest_reader) {
            Ok((sha256, bytes)) => evidence.push(EvidenceFile {
                path: p.clone(),
                bytes,
                sha256,
            }),
            Err(e) => {
                return emit(
                    &slice::reconcile_refused_answer(&format!(
                        "the evidence file {p} could not be read: {e}; nothing was written"
                    )),
                    format,
                );
            }
        }
    }

    // Rule 1: a run still supervising holds the lock.
    let _held = match statecraft_run::lock::try_acquire(home, &root) {
        Ok(held) => held,
        Err(e @ statecraft_run::lock::LockError::Busy { .. }) => {
            return emit(&slice::lock_busy_answer(&e.to_string()), format);
        }
        Err(e) => return fail(&e.to_string(), format),
    };
    let (mut chain, _) = match Chain::open(home, &root) {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string(), format),
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
                return emit(
                    &slice::reconcile_refused_answer(&format!("{why}; nothing was written")),
                    format,
                );
            }
            Err(e) => return fail(&format!("{e}; nothing was written"), format),
        };
    let gate_released_tool_call = gate
        .as_ref()
        .is_some_and(|g| g.iter().any(|w| w == "admitted"));
    let observation = match statecraft_environment::manifest::Manifest::read(&root) {
        Err(e) => return fail(&e.to_string(), format),
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
                    return emit(
                        &slice::reconcile_refused_answer(&format!("{why}; nothing was written")),
                        format,
                    );
                }
                Err(e) => return fail(&e.to_string(), format),
            }
        }
    };
    let at = statecraft_environment::time::rfc3339_utc(
        statecraft_environment::time::Clock::now_unix(&SystemClock),
    );
    let request = Request {
        run_id,
        attempt,
        finding,
        inspected,
        operator,
        reason: &reason,
        evidence,
        at: &at,
    };
    emit(
        &slice::reconcile_answer(statecraft_run::reconcile::reconcile(
            &mut chain,
            &request,
            observation,
        )),
        format,
    )
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
    report: &statecraft_run::report::CorpusReport,
    format: Format,
) -> i32 {
    // A unit of work the policy did not admit is never run, and the reason is
    // the answer (spec 003 section 3.1.1).
    let eligibility = work.eligibility_of(spec_id);
    let statecraft_run::work::Eligibility::Eligible(item) = &eligibility else {
        return emit(&slice::work_show_answer(eligibility), format);
    };
    // Spec 003 section 3.1.4 rule 5: how it was admitted goes into the intent.
    let admission = item.admission(&work.policy);
    // Spec 004 section 3.17 rule 4: the command coverage at planning, from the
    // working tree as the operator invoked `run`, before anything is appended.
    // The requirement is read from the suite this spec declares and the
    // allowance from the adapter and the committed declaration; a suite that
    // cannot be read, a malformed declaration and a missing program each
    // refuse here, with nothing appended and no process created.
    let planned = match statecraft_cli::coverage::at_planning(root, spec_id) {
        Ok(planned) => planned,
        Err(why) => {
            return emit(
                &statecraft_cli::coverage::planning_refused_answer(&why, None),
                format,
            );
        }
    };
    if let Some(why) = planned.refusal() {
        return emit(
            &statecraft_cli::coverage::planning_refused_answer(&why, Some(&planned)),
            format,
        );
    }
    // Spec 003 section 3.1.3: the contract is resolved by the producer and
    // written with the intent, before any effect. It is evidence, not a gate.
    let contract = statecraft_run::contract::bind(
        &SpecSpineCli::default(),
        root,
        spec_id,
        report.lifecycle_of(spec_id),
    );
    launch_attempt(
        root,
        home,
        spec_id,
        AttemptPlan::work_order(spec_id, admission, planned),
        contract,
        None,
        format,
    )
}

/// What an attempt asks of its session. `run` asks for its unit of work; the
/// managed-startup trial asks for the sentinel read (spec 002 section 3.33
/// rule 30), and nothing else differs.
struct AttemptPlan {
    prompt: Vec<u8>,
    max_turns: Option<u32>,
    deadline_seconds: u64,
    /// How the unit of work was admitted, written into the intent (spec 003
    /// section 3.1.4 rule 5). The trial is not a unit of work and has none.
    admission: Option<statecraft_run::work::Admission>,
    /// The planning reading of the command coverage (spec 004 section 3.17
    /// rule 4). The trial has no spec, so none, and its coverage is
    /// `not-applicable`.
    planned: Option<statecraft_adapter::coverage::Coverage>,
}

impl AttemptPlan {
    fn work_order(
        spec_id: &str,
        admission: statecraft_run::work::Admission,
        planned: statecraft_adapter::coverage::Coverage,
    ) -> Self {
        Self {
            prompt: format!("Implement {spec_id} in this workspace.").into_bytes(),
            max_turns: None,
            deadline_seconds: 900,
            admission: Some(admission),
            planned: Some(planned),
        }
    }

    fn trial(deadline_seconds: u64) -> Self {
        Self {
            prompt: statecraft_home::trial::prompt().into_bytes(),
            max_turns: Some(statecraft_home::trial::MAX_TURNS),
            deadline_seconds,
            admission: None,
            planned: None,
        }
    }
}

/// What a trial carries through its attempt, filled in as the launch goes.
struct TrialRun {
    origin: statecraft_home::trial::Origin,
    deadline_seconds: u64,
    sentinel: Option<statecraft_home::trial::Sentinel>,
    probed_version: Option<String>,
    settings_written: Option<String>,
    entries: Vec<statecraft_home::trial::Entry>,
    decision: Option<statecraft_home::trial::DecisionPoint>,
    process: statecraft_home::trial::ProcessEnd,
}

/// One attempt through the adapter: begin, prepare, supervise, conclude. The
/// run path, shared by `run` and by `startup trial`.
fn launch_attempt(
    root: &std::path::Path,
    home: &std::path::Path,
    run_id: &str,
    plan: AttemptPlan,
    contract: statecraft_run::contract::Binding,
    mut trial: Option<TrialRun>,
    format: Format,
) -> i32 {
    // Spec 002 section 3.25: managed execution refuses when the committed
    // requirement's content cannot be established. Judged by the library and
    // before the attempt is appended, so a refusal leaves no attempt behind.
    let layout = statecraft_home::home::Layout::new(home);
    let project_manifest = match statecraft_environment::manifest::Manifest::read(root) {
        Ok(m) => m,
        Err(e) => return fail(&e.to_string(), format),
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
        return emit(&bind::harness_refused_answer(root, &reason), format);
    }

    // Spec 003 section 3.1.4 rule 7: the repository lock, held from before
    // the intent until the outcome is durable, released by the operating
    // system when this process ends however it ends.
    let _held = match statecraft_run::lock::try_acquire(home, root) {
        Ok(held) => held,
        Err(e @ statecraft_run::lock::LockError::Busy { .. }) => {
            return emit(&slice::lock_busy_answer(&e.to_string()), format);
        }
        Err(e) => return fail(&e.to_string(), format),
    };
    let (mut chain, _) = match Chain::open(home, root) {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string(), format),
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
    let planning = plan
        .planned
        .as_ref()
        .map(statecraft_cli::coverage::planning_record);
    let session = match statecraft_run::session::begin_with(
        &mut chain,
        root,
        &run_id,
        "HEAD",
        &SystemClock,
        &statecraft_run::session::IntentDetail {
            contract: Some(&contract),
            admission: plan.admission.as_ref(),
            posture_coverage: planning.as_ref(),
        },
    ) {
        Ok(s) => s,
        Err(e) => {
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
                return emit(
                    &slice::live_attempt_answer(
                        &e,
                        &root.display().to_string(),
                        startup,
                        reconciliation,
                    ),
                    format,
                );
            }
            return emit(&slice::session_error_answer(&e), format);
        }
    };

    // Spec 002 section 3.33 rule 31: the trial's sentinel is in the attempt's
    // own workspace before anything is prepared or launched.
    if let Some(t) = trial.as_mut() {
        match statecraft_home::trial::place_sentinel(&session.workspace.path) {
            Ok(sentinel) => t.sentinel = Some(sentinel),
            Err(e) => {
                let mut accounting = statecraft_run::refusal::Accounting::default();
                accounting.observe(statecraft_run::refusal::RefusalEvent {
                    guard: "trial-sentinel".to_string(),
                    detail: format!("the trial's sentinel could not be placed: {e}"),
                });
                return conclude_and_emit(
                    &mut chain,
                    &places,
                    &session,
                    statecraft_run::attempt::Outcome::Refused,
                    &accounting,
                    serde_json::json!({ "trialRefusal": e.to_string() }),
                    unlaunched(project_manifest.is_some(), None),
                    &contract,
                    trial,
                    format,
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
        Some(planned) => statecraft_cli::coverage::at_base(root, &base_commit, planned),
        None => statecraft_cli::coverage::not_applicable(root, &base_commit),
    };

    // Preflight refuses before any process is created, naming the token (spec
    // 004 section 3.3). The manifest is read here and only here.
    let manifest_adapter = statecraft_adapter_claude_code::manifest();
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
        let mut accounting = statecraft_run::refusal::Accounting::default();
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: statecraft_adapter::coverage::GUARD.to_string(),
            detail: why.clone(),
        });
        return conclude_and_emit(
            &mut chain,
            &places,
            &session,
            statecraft_run::attempt::Outcome::Refused,
            &accounting,
            serde_json::json!({ "postureCoverageRefusal": why, "posture": posture }),
            unlaunched(project_manifest.is_some(), None),
            &contract,
            trial,
            format,
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
            let mut accounting = statecraft_run::refusal::Accounting::default();
            accounting.observe(statecraft_run::refusal::RefusalEvent {
                guard: "adapter-preflight".to_string(),
                detail: refusal.to_string(),
            });
            return conclude_and_emit(
                &mut chain,
                &places,
                &session,
                statecraft_run::attempt::Outcome::Refused,
                &accounting,
                serde_json::json!({ "preflightRefusal": refusal.to_string(), "posture": posture }),
                unlaunched(project_manifest.is_some(), None),
                &contract,
                trial,
                format,
            );
        }
    };

    let (probe, probed_version) = adapters::probe_reporting_version(home);
    if let Some(t) = trial.as_mut() {
        t.probed_version = probed_version;
    }
    posture.qualification = probe.qualification();
    let Some(program) = probe.resolved_executable() else {
        let mut accounting = statecraft_run::refusal::Accounting::default();
        accounting.observe(statecraft_run::refusal::RefusalEvent {
            guard: "constructed-environment".to_string(),
            detail: "the provider executable does not resolve on the child's PATH".to_string(),
        });
        return conclude_and_emit(
            &mut chain,
            &places,
            &session,
            statecraft_run::attempt::Outcome::Refused,
            &accounting,
            serde_json::json!({ "preflightRefusal": "provider executable unresolvable", "posture": posture }),
            unlaunched(project_manifest.is_some(), None),
            &contract,
            trial,
            format,
        );
    };

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
                    let mut accounting = statecraft_run::refusal::Accounting::default();
                    accounting.observe(statecraft_run::refusal::RefusalEvent {
                        guard: statecraft_home::spec_spine::SELECTION_GUARD.to_string(),
                        detail: detail.clone(),
                    });
                    return conclude_and_emit(
                        &mut chain,
                        &places,
                        &session,
                        statecraft_run::attempt::Outcome::Refused,
                        &accounting,
                        serde_json::json!({ "preflightRefusal": detail, "posture": posture }),
                        unlaunched(true, None),
                        &contract,
                        trial,
                        format,
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
    let prepared = match &project_manifest {
        None => None,
        Some(manifest) => {
            let now = statecraft_environment::time::rfc3339_utc(
                statecraft_environment::time::Clock::now_unix(&SystemClock),
            );
            match statecraft_home::launch::prepare(&statecraft_home::launch::Preparation {
                root,
                places: &places,
                workspace: &session.workspace.path,
                base_commit: &session.workspace.base_commit,
                attempt: statecraft_home::launch::AttemptIdentity {
                    run_id: run_id.clone(),
                    attempt: session.attempt,
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
                    let mut accounting = statecraft_run::refusal::Accounting::default();
                    accounting.observe(statecraft_run::refusal::RefusalEvent {
                        guard: statecraft_home::launch::STARTUP_RECORD_GUARD.to_string(),
                        detail: why.to_string(),
                    });
                    return conclude_and_emit(
                        &mut chain,
                        &places,
                        &session,
                        statecraft_run::attempt::Outcome::Refused,
                        &accounting,
                        serde_json::json!({ "startupRefusal": why.to_string(), "posture": posture }),
                        unlaunched(true, Some(why.to_string())),
                        &contract,
                        trial,
                        format,
                    );
                }
            }
        }
    };
    let environment = match &prepared {
        Some(p) => {
            let binding = match &supervised_spec_spine {
                Some(program) => {
                    statecraft_home::spec_spine::managed_binding(&p.intent.environment(), program)
                }
                None => p.intent.environment(),
            };
            adapters::child_environment_with(&binding, covered.as_ref().ok())
        }
        None => environment,
    };

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
        Err(e) => return fail(&e, format),
    };
    let request = statecraft_adapter::protocol::Request {
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
            &SystemClock,
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
    let exchange = statecraft_home::launch::AttemptIdentity {
        run_id: run_id.clone(),
        attempt: session.attempt,
    }
    .exchange_dir(&places);
    let sentinel = trial.as_ref().and_then(|t| t.sentinel.clone());
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
                &mut kept,
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
                w,
            );
            if supervised.is_ok() {
                // A stream that ended before its decision point is decided at
                // its end.
                w.decide_at_end();
            }
            supervised
        }
        (None, _) => statecraft_adapter_claude_code::execution::supervise(
            &invocation,
            &request,
            &environment,
            &negotiation.granted,
        ),
    };
    let watched = watch.map(|w| w.watched()).unwrap_or_default();
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
            let startup = finalize_startup(
                &places,
                &layout,
                project_manifest.as_ref(),
                prepared.as_ref(),
                &statecraft_home::launch::Launch::Failed {
                    reason: e.to_string(),
                },
                &watched,
            );
            return conclude_and_emit(
                &mut chain,
                &places,
                &session,
                statecraft_run::attempt::Outcome::Interrupted,
                &accounting,
                serde_json::json!({ "supervisorError": e.to_string(), "posture": posture, "startup": startup.detail }),
                startup.run,
                &contract,
                trial,
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

    // Spec 002 section 3.31: the record is written from what the launch
    // produced, and a startup decision that refused governed work refuses the
    // attempt under its guard (section 3.32 rule 26).
    let version = provider_version(&execution);
    let startup = finalize_startup(
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

    conclude_and_emit(
        &mut chain,
        &places,
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
            "specId": run_id,
            "execution": execution.evidence(),
            "payload": {
                "digest": statecraft_home::startup::payload_identity(),
                "argument": statecraft_home::session::SETTINGS_ARGUMENT,
            },
            "harnessStanding": standing.word(),
            "startup": startup.detail,
        }),
        startup.run,
        &contract,
        trial,
        format,
    )
}

/// `startup trial <path> (--provider-session | --synthetic) [--deadline <s>]`
///
/// Spec 006 section 3.11.4 and spec 002 section 3.33. Every refusal here comes
/// before an attempt is appended, so a refused trial spends nothing.
fn trial_verb(
    root: &std::path::Path,
    home: &std::path::Path,
    rest: &[String],
    format: Format,
) -> i32 {
    use statecraft_home::trial;
    let usage = || {
        eprintln!(
            "usage: startup trial{}",
            statecraft_cli::manage::usage(Verb::StartupTrial)
        );
        Exit::Usage.code()
    };
    let mut stated = Vec::new();
    let mut deadline = trial::DEFAULT_DEADLINE_SECONDS;
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--provider-session" => stated.push(trial::Origin::ProviderSession),
            "--synthetic" => stated.push(trial::Origin::Synthetic),
            "--deadline" => match args.next().map(|n| n.parse::<u64>()) {
                Some(Ok(n)) => deadline = n,
                _ => return usage(),
            },
            _ => return usage(),
        }
    }
    let origin = match stated.as_slice() {
        [one] => *one,
        [] => {
            return emit(
                &slice::trial_refused_answer(
                    "the trial runs a provider session or a local fake, and the operator must                      say which: pass --provider-session or --synthetic",
                ),
                format,
            );
        }
        _ => {
            return emit(
                &slice::trial_refused_answer(
                    "--provider-session and --synthetic contradict each other; pass one",
                ),
                format,
            );
        }
    };
    if let Some(why) = trial::refuses_deadline(deadline) {
        return emit(&slice::trial_refused_answer(&why), format);
    }
    let manifest = match Manifest::read(root) {
        Ok(Some(m)) => m,
        Ok(None) => {
            return emit(
                &slice::trial_refused_answer(&format!(
                    "{} holds no manifest, so it is not a managed project",
                    root.display()
                )),
                format,
            );
        }
        Err(e) => return fail(&e.to_string(), format),
    };
    let layout = statecraft_home::home::Layout::new(home);
    let standing = statecraft_home::required::evaluate(&layout, &manifest, None);
    if matches!(standing, statecraft_home::required::Standing::Unrequired) {
        return emit(
            &slice::trial_refused_answer(
                "the project commits no harness requirement, so a run is not gated and a trial                  could not answer its question; commit one with `harness upgrade` first",
            ),
            format,
        );
    }
    if let Some(reason) = standing.refuses_a_run() {
        return emit(&bind::harness_refused_answer(root, &reason), format);
    }
    match Chain::open(home, root) {
        Ok((chain, _)) => {
            if let Some(run) = statecraft_run::session::runs(&chain)
                .into_iter()
                .find(|r| r.id == trial::RUN_ID && !r.attempts.is_empty())
            {
                return emit(
                    &slice::trial_refused_answer(&format!(
                        "this project's trial is spent: run {} holds {} attempt(s). A trial \
                         happens once per project, and an uncertain one is never replayed; \
                         inspect it with `startup show {} {}`",
                        trial::RUN_ID,
                        run.attempts.len(),
                        root.display(),
                        trial::RUN_ID
                    )),
                    format,
                );
            }
        }
        Err(e) => return fail(&e.to_string(), format),
    }
    if root.join(trial::SENTINEL).exists() {
        return emit(
            &slice::trial_refused_answer(&format!(
                "{} already exists in the project, so the sentinel's nonce could not be the \
                 only copy",
                trial::SENTINEL
            )),
            format,
        );
    }
    launch_attempt(
        root,
        home,
        trial::RUN_ID,
        AttemptPlan::trial(deadline),
        statecraft_run::contract::Binding::not_a_unit_of_work(),
        Some(TrialRun {
            origin,
            deadline_seconds: deadline,
            sentinel: None,
            probed_version: None,
            settings_written: None,
            entries: Vec::new(),
            decision: None,
            process: statecraft_home::trial::ProcessEnd::default(),
        }),
        format,
    )
}

/// Write the trial's record from what the attempt produced, then read every
/// record back and judge it again (spec 002 section 3.33 rule 34).
fn trial_conclusion(
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
fn unlaunched(managed: bool, error: Option<String>) -> statecraft_home::launch::RunStartup {
    statecraft_home::launch::RunStartup {
        managed,
        error,
        ..statecraft_home::launch::RunStartup::unmanaged()
    }
}

fn manifest_adapter_name(manifest: &statecraft_adapter::manifest::Manifest) -> String {
    manifest.adapter.clone()
}

/// The provider version the stream's init event reported.
fn provider_version(
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
struct Finalized {
    run: statecraft_home::launch::RunStartup,
    detail: serde_json::Value,
    refusal: Option<(&'static str, String)>,
}

/// Write the attempt's record (spec 002 section 3.31), or say why it was not
/// stored. The judgement is the library's; this places its answer.
fn finalize_startup(
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
    let now = statecraft_environment::time::rfc3339_utc(
        statecraft_environment::time::Clock::now_unix(&SystemClock),
    );
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

// The three calls spec 006 section 3.11 leaves to the caller share this one
// conclusion, and each argument is a different fact it records.
#[allow(clippy::too_many_arguments)]
fn conclude_and_emit(
    chain: &mut Chain,
    places: &statecraft_home::launch::Places,
    session: &statecraft_run::session::Session,
    termination: impl Into<statecraft_run::session::Termination>,
    accounting: &statecraft_run::refusal::Accounting,
    detail: serde_json::Value,
    mut startup: statecraft_home::launch::RunStartup,
    contract: &statecraft_run::contract::Binding,
    trial: Option<TrialRun>,
    format: Format,
) -> i32 {
    match statecraft_run::session::conclude_observed(
        chain,
        &places.target,
        session,
        termination.into(),
        accounting,
        detail,
        &SystemClock,
    ) {
        Ok(concluded) => {
            if let Some(t) = trial {
                return emit(&trial_conclusion(places, &concluded, t), format);
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
            emit(&answer, format)
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

    // Spec 005 section 3.18: the contract the attempt was bound to, compared
    // with the producer's resolution now, before anything else is observed.
    let contract = statecraft_acceptance::contract::check(
        &chain.entries(),
        run_id,
        attempt.number,
        root,
        &SpecSpineCli::default(),
    );
    if contract.word == statecraft_acceptance::contract::Word::Stale {
        return emit(&slice::contract_stale_answer(&contract), format);
    }
    if contract.word.moved() {
        return emit(
            &slice::accept_answer_with(
                statecraft_acceptance::outcome::Acceptance::None {
                    reason: statecraft_acceptance::judged::NoAcceptance::ContractMoved {
                        comparison: Box::new(contract.clone()),
                    },
                },
                Some(contract),
            ),
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
    emit(
        &slice::accept_answer_with(acceptance, Some(contract)),
        format,
    )
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
    named: &[statecraft_environment::replace::Consent],
    remote: Option<bind::RemoteAsk>,
    format: Format,
) -> i32 {
    let declarations = adapters::declarations();
    let probe = adapters::probe(home);
    // No second installer is left to claim a path (spec 002 sections 3.21 and
    // 3.22), so no package identity exists to name. An occupied path is still
    // named with its owner: the library classes a file no manifest records as
    // the user's (section 3.2) and says so in every `foreign` finding.
    let foreign = ForeignClaims::none();
    let clock = SystemClock;

    // `env remove` reads the manifest itself and refuses when there is none, so
    // it is the one verb that does not need one read here.
    if verb == Verb::EnvRemove {
        // Spec 002 section 3.13 rule 4: the root bridge is taken back by its
        // record, and a bridge line with no record is reported, not removed.
        let sites = [statecraft_environment::apply::BridgeSite {
            path: statecraft_home::project::ROOT_INSTRUCTIONS.to_string(),
            kind: statecraft_environment::manifest::ModificationKind::ImportBridge,
            line: statecraft_home::bridge::IMPORT_LINE.to_string(),
        }];
        return match statecraft_environment::apply::remove_with(root, &clock, &sites) {
            Ok(removal) => emit(&bind::removal_answer(removal), format),
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
        Verb::EnvPlan => match statecraft_environment::plan::plan_naming(
            root,
            manifest.as_ref(),
            &declarations,
            &probe,
            &foreign,
            &named.iter().map(|c| c.path.clone()).collect::<Vec<_>>(),
        ) {
            Ok(plan) => emit(&bind::plan_answer(plan), format),
            Err(e) => emit(&bind::plan_error_answer(root, &e), format),
        },
        // `env upgrade` is `env apply` against the current declarations, which
        // is spec 002 section 3.6's own position: the two verbs are separated
        // for the operator, not for the machine, and giving them different code
        // paths is how their conflict rules would drift.
        Verb::EnvApply | Verb::EnvUpgrade => {
            // Read again, planned and written by one library operation, so the
            // manifest it writes is the one it planned against.
            let _ = manifest;
            match statecraft_environment::apply::apply_consented_current(
                root,
                &declarations,
                &probe,
                &foreign,
                &clock,
                || adapters::pins(root),
                named,
            ) {
                Ok(consented) => emit(&bind::consented_answer(consented), format),
                Err(e) => emit(&bind::apply_error_answer(&e), format),
            }
        }
        Verb::Doctor => {
            let manifest = manifest.unwrap_or_else(|| Manifest::new(adapters::pins(root)));
            match statecraft_environment::doctor::doctor(
                root,
                &manifest,
                &declarations,
                &probe,
                &foreign,
                &UnobservedShadows,
                &adapters::observed(),
            ) {
                Ok(mut report) => {
                    // Spec 002 section 3.25 names `doctor` as one of the four
                    // things that stay possible while managed execution is
                    // refused. The comparison needs a product home, which the
                    // crate that owns `doctor` cannot see, so it is made here
                    // and reported beside everything else the diagnostic found.
                    let standing = statecraft_home::required::evaluate(
                        &statecraft_home::home::Layout::new(home),
                        &manifest,
                        None,
                    );
                    if let Some(finding) = statecraft_home::required::doctor_finding(&standing) {
                        report.findings.push(finding);
                    }
                    match remote {
                        None => emit(&bind::doctor_answer(report), format),
                        Some(ask) => {
                            let results = statecraft_home::setup::remote_results(
                                root,
                                &manifest,
                                &statecraft_home::setup::GhHost::default(),
                                ask.head.as_deref(),
                            );
                            emit(&bind::doctor_remote_answer(report, results), format)
                        }
                    }
                }
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

/// The registration a typed path names, as the root it was stored under.
///
/// Spec 003 section 3.1.4 rule 7 and section 5 (2026-09-23): the register
/// compares paths component-wise, so `<root>/` and `<root>/.` name the same
/// registration, and every record this product keeps per repository is keyed
/// by the stored root rather than by what was typed. Passing the typed path on
/// would give one repository a second lock, run record and journal.
fn registered(registry: &Registry, typed: &std::path::Path) -> Result<PathBuf, Answer<String>> {
    use statecraft_run::repository::{Unresolved, resolve};
    match resolve(registry, typed) {
        Ok(registration) => Ok(registration.root.clone()),
        Err(Unresolved::NotRegistered) => Err(bind::unregistered_answer(typed)),
        Err(Unresolved::SameDirectory(roots)) => Err(bind::same_directory_answer(typed, &roots)),
    }
}

/// As [`registered`], for a verb that does not require registration: an
/// unregistered path is used as typed, and a registered one as stored.
fn stored_or_typed(
    registry: &Registry,
    typed: &std::path::Path,
) -> Result<PathBuf, Answer<String>> {
    use statecraft_run::repository::{Unresolved, resolve};
    match resolve(registry, typed) {
        Ok(registration) => Ok(registration.root.clone()),
        Err(Unresolved::NotRegistered) => Ok(typed.to_path_buf()),
        Err(Unresolved::SameDirectory(roots)) => Err(bind::same_directory_answer(typed, &roots)),
    }
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
