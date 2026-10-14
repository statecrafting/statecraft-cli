//! The `statecraft-cli` executable.
//!
//! Spec 006. This file is deliberately thin: it parses, dispatches to one
//! binding, prints one rendering, and exits with the code that binding chose.
//! Every decision worth testing lives in the library beside it, which is why
//! `tests/` can cover the command surface without spawning a process for the
//! cases that do not need one.

use statecraft_cli::adapters;
use statecraft_cli::attempt::{AttemptPlan, TrialRun};
use statecraft_cli::bind;
use statecraft_cli::commands::{Verb, parse, usage_verb};
use statecraft_cli::exit::Exit;
use statecraft_cli::product_home;
use statecraft_cli::render::{Answer, Format, usage_envelope};
use statecraft_cli::slice;
use statecraft_environment::claimant::{ForeignClaims, UnobservedShadows};
use statecraft_environment::manifest::Manifest;
use statecraft_environment::registry::Registry;
use statecraft_environment::time::SystemClock;
use statecraft_run::policy::NoDeclarationFiled;
use statecraft_run::record::Chain;
use statecraft_run::report::ReportSource;
use std::path::PathBuf;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The boundary self-test re-executes this binary inside the confinement.
    // The probe parses one typed JSON argument, only attempts accesses with the
    // caller's own rights, and reports booleans; a malformed argument exits 3.
    if args.len() == 2 && args[0] == "__boundary_probe" {
        return std::process::ExitCode::from(statecraft_adapter::boundary::probe(&args[1]) as u8);
    }
    let code = run(&args);
    std::process::ExitCode::from(code as u8)
}

fn run(args: &[String]) -> i32 {
    let invocation = match parse(args) {
        Ok(i) => i,
        Err(e) => {
            // Usage errors go to stderr: a caller piping `--json` into a parser
            // should not have to filter a help text out of its input. Under
            // `--json` the envelope goes to stdout instead (spec 007 section
            // 3.3), naming the attempted operation without its operands.
            if args.iter().any(|a| a == "--json") {
                let verb = usage_verb(args);
                print!("{}", usage_envelope(&verb, &e.describe()));
            } else {
                eprint!("{}", e.describe());
            }
            return Exit::Usage.code();
        }
    };
    let format = if invocation.json {
        Format::Json(invocation.verb)
    } else {
        Format::Human
    };

    if !matches!(invocation.verb, Verb::InitPlan | Verb::InitApply)
        && invocation
            .rest
            .iter()
            .any(|a| a == "--spine" || a.starts_with("--spine="))
    {
        return usage(
            format,
            "--spine is accepted only by init plan and init apply".into(),
        );
    }
    // Spec 036: the offline trust verbs read no product home and no
    // registered project, so they are dispatched before either is consulted.
    if matches!(
        invocation.verb,
        Verb::TrustRootGenerate | Verb::TrustIssuerSign | Verb::TrustIssuerVerify
    ) {
        return trust_verb(invocation.verb, &invocation.rest, format);
    }
    let home = product_home();
    let mut registry = match Registry::read(&home) {
        Ok(r) => r,
        Err(e) => return fail(&e.to_string(), format),
    };

    match invocation.verb {
        Verb::ProjectRegister => {
            let Some(path) = invocation.rest.first() else {
                return usage(format, "usage: project register <path>".to_string());
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
            let probe =
                statecraft_home::spec_spine::probe_for(&statecraft_cli::judge::selection(&path));
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
                return usage(
                    format,
                    format!("usage: {} <path>", invocation.verb.spelling()),
                );
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
                return usage(
                    format,
                    format!("usage: {} <path>", invocation.verb.spelling()),
                );
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
                        return usage(format, format!("usage: {detail}"));
                    }
                }
            } else {
                (None, invocation.rest[1..].to_vec())
            };
            let named = match bind::replace_arguments(consenting, &rest) {
                Ok(named) => named,
                Err(detail) => {
                    return usage(format, format!("usage: {detail}"));
                }
            };
            if !named.is_empty() && matches!(invocation.verb, Verb::EnvRemove | Verb::Doctor) {
                return usage(
                    format,
                    format!(
                        "usage: {} does not take --replace; name a path to `env plan`, then consent to it with `env apply`",
                        invocation.verb.spelling()
                    ),
                );
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
                return usage(
                    format,
                    format!(
                        "usage: {} <path>{}",
                        invocation.verb.spelling(),
                        argument_hint(invocation.verb)
                    ),
                );
            };
            // Spec 040: `accept` is a callable operation that resolves its
            // own registration; the adapter only parses and renders.
            if invocation.verb == Verb::Accept {
                return accept_verb(
                    &absolute(path),
                    &home,
                    &registry,
                    invocation.rest.get(1).map(String::as_str),
                    format,
                );
            }
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
                return usage(
                    format,
                    format!(
                        "usage: {}{}",
                        invocation.verb.spelling(),
                        statecraft_cli::manage::usage(invocation.verb)
                    ),
                );
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
                return usage(
                    format,
                    format!(
                        "usage: startup trial{}",
                        statecraft_cli::manage::usage(invocation.verb)
                    ),
                );
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
                return usage(
                    format,
                    format!(
                        "usage: startup show{}",
                        statecraft_cli::manage::usage(invocation.verb)
                    ),
                );
            };
            let attempt = match invocation.rest.get(2..).unwrap_or_default() {
                [] => None,
                [flag, n] if flag == "--attempt" => match n.parse::<u32>() {
                    Ok(n) if n > 0 => Some(n),
                    _ => {
                        return usage(
                            format,
                            "usage: startup show <path> <run-id> [--attempt <n>]".to_string(),
                        );
                    }
                },
                _ => {
                    return usage(
                        format,
                        "usage: startup show <path> <run-id> [--attempt <n>]".to_string(),
                    );
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
            let show_usage = || {
                usage(
                    format,
                    format!(
                        "usage: {} <path>{}",
                        invocation.verb.spelling(),
                        if invocation.verb == Verb::OverrideShow {
                            ""
                        } else {
                            " <spec-id> <operator> <reason...>"
                        }
                    ),
                )
            };
            let Some(path) = invocation.rest.first() else {
                return show_usage();
            };
            let root = match registered(&registry, &absolute(path)) {
                Ok(root) => root,
                Err(answer) => return emit(&answer, format),
            };
            if invocation.verb == Verb::OverrideShow {
                if invocation.rest.len() != 1 {
                    return show_usage();
                }
                return match statecraft_run::overrides::read(&home, &root) {
                    Ok(journal) => emit(
                        &slice::override_show_answer(&root.display().to_string(), &journal),
                        format,
                    ),
                    Err(e) => emit(&slice::journal_error_answer(&e), format),
                };
            }
            let (Some(spec_id), Some(operator)) = (invocation.rest.get(1), invocation.rest.get(2))
            else {
                return show_usage();
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
            let known = match statecraft_cli::judge::report_source(&root)
                .and_then(|cli| cli.corpus_report(&root))
            {
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
        // Spec 006 section 3.11.8: the recovery of 003 section 3.1.5.
        Verb::OverrideRecover => recover_verb(&invocation.rest, &registry, &home, format),
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
                        return usage(format, why.to_string());
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
        Verb::TrustRootGenerate | Verb::TrustIssuerSign | Verb::TrustIssuerVerify => {
            trust_verb(invocation.verb, &invocation.rest, format)
        }
        Verb::ProviderTokenSet | Verb::ProviderTokenClear | Verb::ProviderTokenShow => {
            let outcome = match invocation.verb {
                Verb::ProviderTokenSet => {
                    statecraft_cli::provider::token_set(&home, &invocation.rest)
                }
                Verb::ProviderTokenClear => {
                    statecraft_cli::provider::token_clear(&home, &invocation.rest)
                }
                _ => statecraft_cli::provider::token_show(&home, &invocation.rest),
            };
            match outcome {
                Ok(answer) => emit(&answer, format),
                Err(message) => usage(format, message),
            }
        }
        Verb::HostedLogin | Verb::HostedLogout | Verb::HostedSubmit | Verb::HostedDecision => {
            use statecraft_cli::hosted;
            let outcome = match invocation.verb {
                Verb::HostedLogin => hosted::login(&home, &invocation.rest),
                Verb::HostedLogout => hosted::logout(&home, &invocation.rest),
                Verb::HostedSubmit => hosted::submit(&home, &invocation.rest),
                _ => hosted::decision(&home, &invocation.rest),
            };
            match outcome {
                Ok(answer) => emit(&answer, format),
                Err(message) => usage(format, message),
            }
        }
    }
}

/// One spec 036 offline trust verb: parse its explicit options, call its one
/// operation, render the answer. It never reads the product home.
fn trust_verb(verb: Verb, rest: &[String], format: Format) -> i32 {
    use statecraft_cli::trust;
    let outcome = match verb {
        Verb::TrustRootGenerate => trust::root_generate(rest),
        Verb::TrustIssuerSign => trust::issuer_sign(rest),
        _ => trust::issuer_verify(rest),
    };
    match outcome {
        Ok(answer) => emit(&answer, format),
        Err(message) => usage(format, message),
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
    if let Some(message) = statecraft_cli::manage::setup_usage(verb, rest) {
        return usage(format, message);
    }
    let Some(operation) = statecraft_cli::manage::operation(verb, rest, root) else {
        return usage(
            format,
            format!(
                "usage: {}{}",
                verb.spelling(),
                statecraft_cli::manage::usage(verb)
            ),
        );
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
    // Spec 003 section 3.1.5 rule 8: `run` reads the journal and its authority
    // once, under the repository lock, and holds that lock through its attempt,
    // so no grant, revocation or recovery lands between the read and the
    // intent.
    let mut held = None;
    if verb == Verb::Run {
        held = Some(match statecraft_run::lock::try_acquire(home, root) {
            Ok(held) => held,
            Err(e @ statecraft_run::lock::LockError::Busy { .. }) => {
                return emit(&slice::lock_busy_answer(&e.to_string()), format);
            }
            Err(e) => return fail(&e.to_string(), format),
        });
    }
    let (work, report) = if needs_report {
        match statecraft_cli::judge::report_source(root).and_then(|cli| cli.corpus_report(root)) {
            Ok(report) => {
                let (policy, disagreement) =
                    statecraft_run::policy::resolve(root, &NoDeclarationFiled, None);
                // Spec 003 section 3.1.4 rule 3 and 3.1.5 rule 3: a journal
                // that does not read or verify, or disagrees with its state
                // authority, is a failure, never an empty set of overrides; a
                // pending line or a write in progress is a refusal.
                let read = match &held {
                    Some(held) => statecraft_run::overrides::read_held(home, root, held),
                    None => statecraft_run::overrides::read(home, root),
                };
                let overrides = match read {
                    Ok(journal) => journal.overrides(),
                    Err(e) => return emit(&slice::journal_error_answer(&e), format),
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
                return usage(format, "usage: work show <path> <spec-id>".to_string());
            };
            emit(
                &slice::work_show_answer(work.expect("read above").eligibility_of(id)),
                format,
            )
        }
        Verb::Run => {
            let Some(id) = rest.first() else {
                return usage(format, "usage: run <path> <spec-id>".to_string());
            };
            run_verb(
                root,
                home,
                id,
                work.expect("read above"),
                report.as_ref().expect("read above"),
                held.expect("taken above"),
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
                return usage(format, "usage: run show <path> <run-id>".to_string());
            };
            match Chain::open(home, root) {
                Ok((chain, _)) => {
                    let Some(run) = statecraft_run::session::runs(&chain)
                        .into_iter()
                        .find(|r| r.id == *run_id)
                    else {
                        return emit(&slice::no_such_run_answer(run_id), format);
                    };
                    // Spec 003 section 3.5.1: each attempt's evidence, from
                    // its accounting record and the audit file beside it.
                    let tamper = statecraft_run::tamper::read_audit(home, root).and_then(|audit| {
                        slice::AttemptTamperView::of_run(&chain.entries(), run_id, &audit)
                    });
                    let tamper = match tamper {
                        Ok(t) => t,
                        Err(e) => return fail(&e, format),
                    };
                    emit(
                        &slice::run_show_with_admissions(
                            statecraft_acceptance::suite::fold(run_id, &chain.entries()),
                            &run,
                            slice::ReconciliationView::of_run(&chain.positioned_entries(), run_id),
                            tamper,
                        ),
                        format,
                    )
                }
                Err(e) => fail(&e.to_string(), format),
            }
        }
        _ => Exit::Usage.code(),
    }
}

/// `override recover <path> [<choice> <journal-digest> <authority-digest> <operator> <reason...>]`
///
/// Spec 006 section 3.11.8 and spec 003 section 3.1.5 rule 5. Without a
/// choice it reports both files and writes nothing; with one it records that
/// choice against the state the digests name, and never picks one itself.
fn recover_verb(
    rest: &[String],
    registry: &statecraft_environment::registry::Registry,
    home: &std::path::Path,
    format: Format,
) -> i32 {
    use statecraft_cli::application::{Context, RecoverChoice, override_recover};
    use statecraft_run::overrides::Choice;
    let show_usage = || {
        usage(
            format,
            "usage: override recover <path> [<complete-pending|discard-pending|adopt-as-read|\
             adopt-prefix|adopt-empty> <journal-digest|absent> <authority-digest|absent> \
             <operator> <reason...>]"
                .to_string(),
        )
    };
    let Some(path) = rest.first() else {
        return show_usage();
    };
    let choice = match rest.get(1) {
        None => None,
        Some(word) => match Choice::parse(word) {
            Some(c) if rest.len() >= 5 => Some(c),
            _ => return show_usage(),
        },
    };
    let reason = rest.get(5..).unwrap_or_default().join(" ");
    let clock = statecraft_cli::application::process_clock();
    let ctx = Context {
        home,
        registry,
        clock: clock.as_ref(),
    };
    emit(
        &override_recover(
            &ctx,
            &absolute(path),
            choice.map(|choice| RecoverChoice {
                choice,
                journal: &rest[2],
                authority: &rest[3],
                operator: &rest[4],
                reason: &reason,
            }),
        ),
        format,
    )
}

/// `run reconcile <path> <run-id> <attempt> <finding> <launch-state> <operator> <reason...>`
///
/// Spec 006 section 3.11.6 and spec 003 section 3.6.1, through spec 040's
/// callable operation.
fn reconcile_verb(
    rest: &[String],
    registry: &statecraft_environment::registry::Registry,
    home: &std::path::Path,
    format: Format,
) -> i32 {
    use statecraft_cli::application::{Context, ReconcileOperands, run_reconcile};
    let show_usage = || {
        usage(
            format,
            "usage: run reconcile <path> <run-id> <attempt> <confirmed|absent|unknown> \
             <launch-state> <operator> <reason...> [--evidence <file>]..."
                .to_string(),
        )
    };
    let mut positional: Vec<&String> = Vec::new();
    let mut evidence: Vec<String> = Vec::new();
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        if arg == "--evidence" {
            match args.next() {
                Some(p) => evidence.push(p.clone()),
                None => return show_usage(),
            }
        } else {
            positional.push(arg);
        }
    }
    if positional.len() < 7 {
        return show_usage();
    }
    let reason = positional[6..]
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let path = absolute(positional[0]);
    let clock = statecraft_cli::application::process_clock();
    let ctx = Context {
        home,
        registry,
        clock: clock.as_ref(),
    };
    match run_reconcile(
        &ctx,
        &ReconcileOperands {
            path: &path,
            run_id: positional[1],
            attempt: positional[2],
            finding: positional[3],
            inspected: positional[4],
            operator: positional[5],
            reason: &reason,
            evidence: &evidence,
        },
    ) {
        Ok(answer) => emit(&answer, format),
        Err(u) => usage(format, u.0),
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
    held: statecraft_run::lock::Held,
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
    let judge = match statecraft_cli::judge::report_source(root) {
        Ok(judge) => judge,
        Err(e) => return emit(&slice::report_error_answer(&e), format),
    };
    let contract =
        statecraft_run::contract::bind(&judge, root, spec_id, report.lifecycle_of(spec_id));
    launch_attempt(
        root,
        home,
        spec_id,
        AttemptPlan::work_order(spec_id, admission, planned, held),
        contract,
        None,
        format,
    )
}

/// One attempt through the adapter, through spec 040's library launch.
#[allow(clippy::too_many_arguments)]
fn launch_attempt(
    root: &std::path::Path,
    home: &std::path::Path,
    run_id: &str,
    plan: AttemptPlan,
    contract: statecraft_run::contract::Binding,
    trial: Option<TrialRun>,
    format: Format,
) -> i32 {
    emit(
        &statecraft_cli::launch::launch_attempt(
            &SystemClock,
            root,
            home,
            run_id,
            plan,
            contract,
            trial,
        ),
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
        usage(
            format,
            format!(
                "usage: startup trial{}",
                statecraft_cli::manage::usage(Verb::StartupTrial)
            ),
        )
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
/// `accept <path> <run-id>`
fn accept_verb(
    path: &std::path::Path,
    home: &std::path::Path,
    registry: &Registry,
    run_id: Option<&str>,
    format: Format,
) -> i32 {
    use statecraft_cli::application::{Context, accept};
    let clock = statecraft_cli::application::process_clock();
    let ctx = Context {
        home,
        registry,
        clock: clock.as_ref(),
    };
    match accept(&ctx, path, run_id) {
        Ok(answer) => emit(&answer, format),
        Err(u) => usage(format, u.0),
    }
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
    let declarations = adapters::declarations_for(root);
    let probe = match adapters::probe(home, root) {
        Ok(probe) => probe,
        Err(detail) => return boundary_refused(&detail, format),
    };
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
                &adapters::observed(root),
            ) {
                Ok(mut report) => {
                    // Spec 029 section 3.3: the judge this project resolves,
                    // never a separately probed PATH binary.
                    report
                        .notes
                        .extend(statecraft_cli::judge::doctor_notes(root));
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
                    // Spec 026: recorded, journaled and rendered ownership,
                    // compared. The rendering is `init plan`'s preflight and
                    // the declarations `env plan` reads; both write nothing.
                    let mut rendering = statecraft_cli::manage::rendering(home, root);
                    if let statecraft_environment::ownership::RenderingFact::Computed(r) =
                        &mut rendering
                    {
                        r.name_declarations(&declarations);
                    }
                    statecraft_environment::ownership::report(
                        root,
                        &manifest,
                        &rendering,
                        &mut report,
                    );
                    match remote {
                        None => emit(&bind::doctor_answer(report), format),
                        Some(ask) => {
                            // Spec 012: the six results and the per-field
                            // desired-state comparison, read-only.
                            let (results, comparison) = statecraft_home::setup::remote_report(
                                root,
                                &manifest,
                                &statecraft_home::setup::GhHost::default(),
                                ask.head.as_deref(),
                            );
                            emit(
                                &bind::doctor_remote_answer(report, results, comparison),
                                format,
                            )
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

/// A failure no request asked for: a product store (registry, journal, lock,
/// record) that could not be read or written. Its kind is the exit's default,
/// `io` (spec 007 section 3.2).
fn fail(detail: &str, format: Format) -> i32 {
    let answer = Answer::new(detail.to_string(), Exit::Failed, detail);
    print!("{}", answer.render(format));
    Exit::Failed.code()
}

/// A usage error: text on stderr, or under `--json` the envelope on stdout
/// with `error.kind` `usage` (spec 007 section 3.3). Exit 3 either way.
fn usage(format: Format, message: String) -> i32 {
    match format {
        Format::Human => eprintln!("{message}"),
        Format::Json(verb) => print!("{}", usage_envelope(&verb.dotted(), &message)),
    }
    Exit::Usage.code()
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

fn boundary_refused(detail: &str, format: Format) -> i32 {
    emit(
        &Answer::new(
            serde_json::json!({ "outcome": "refused", "reason": "boundary-unavailable", "detail": detail, "platform": std::env::consts::OS }),
            Exit::Refused,
            format!("statecraft: boundary unavailable: {detail}"),
        ),
        format,
    )
}
