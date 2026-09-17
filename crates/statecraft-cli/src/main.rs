//! The `statecraft-cli` executable.
//!
//! Spec 006. This file is deliberately thin: it parses, dispatches to one
//! binding, prints one rendering, and exits with the code that binding chose.
//! Every decision worth testing lives in the library beside it, which is why
//! `tests/` can cover the command surface without spawning a process for the
//! cases that do not need one.

use statecraft_cli::bind;
use statecraft_cli::commands::{Verb, parse};
use statecraft_cli::exit::Exit;
use statecraft_cli::product_home;
use statecraft_cli::render::{Answer, Format};
use statecraft_environment::probe::CommandProbe;
use statecraft_environment::registry::Registry;
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
        // The environment verbs need an adapter set to plan against, and
        // choosing one is not this spec's to make: no spec ratifies a provider
        // adapter yet. Refusing is the honest answer, and it is a refusal
        // because it is a precondition that was not met, not a failure.
        Verb::EnvPlan | Verb::EnvApply | Verb::EnvUpgrade | Verb::EnvRemove => {
            let answer = Answer::new(
                format!(
                    "{} needs a configured adapter set, and no spec ratifies an adapter yet; \
                     spec 002's operations exist and nothing here may choose their inputs",
                    invocation.verb.spelling()
                ),
                Exit::Refused,
                format!(
                    "refused: {} needs a configured adapter set, and none is ratified yet",
                    invocation.verb.spelling()
                ),
            );
            emit(&answer, format)
        }
        Verb::Doctor => {
            let answer = Answer::new(
                "doctor needs a target and a configured adapter set; neither is selected \
                 by this invocation"
                    .to_string(),
                Exit::Refused,
                "refused: doctor needs a target and a configured adapter set",
            );
            emit(&answer, format)
        }
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
