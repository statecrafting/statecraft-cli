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
use statecraft_environment::claimant::{ForeignClaims, UnobservedShadows};
use statecraft_environment::manifest::Manifest;
use statecraft_environment::probe::CommandProbe;
use statecraft_environment::registry::Registry;
use statecraft_environment::time::SystemClock;
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
