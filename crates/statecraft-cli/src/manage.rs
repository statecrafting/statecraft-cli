//! The bindings for the managed-environment verbs.
//!
//! Spec 006 section 3.2, unchanged: a command parses what it was given, calls
//! **exactly one** library operation, and maps the value it returns onto an
//! exit code. Every one of these calls `statecraft_home::service::execute`,
//! which is spec 002 section 3.20's typed operation boundary, and none of them
//! contains a rule spec 002 did not state.
//!
//! The severity of an outcome is decided by that boundary and mapped here. A
//! binding that decided for itself whether a partial initialization is a
//! finding or a failure would be carrying one of spec 002's rules where nobody
//! looks for it.

use crate::exit::Exit;
use crate::render::Answer;
use statecraft_environment::probe::CommandProbe;
use statecraft_environment::time::SystemClock;
use statecraft_home::authority::{GitRevision, RunChoices};
use statecraft_home::flow::SpecSpineCommand;
use statecraft_home::home::Layout;
use statecraft_home::producer::Library;
use statecraft_home::service::{self, Operation, Severity};
use statecraft_home::settings::Intent;
use statecraft_home::team::Unreachable;
use std::path::Path;

/// The default revision the trusted declaration is read at.
///
/// `HEAD` rather than the working tree, because spec 002 section 3.16 reads the
/// trusted configuration at the base revision and the candidate does not get
/// to choose the policy that judges it.
pub const DEFAULT_BASE_REVISION: &str = "HEAD";

/// Perform one operation against one product home.
///
/// The concrete implementations are chosen here and nowhere else: the real
/// spec-spine library as the governance producer, the real `spec-spine` binary
/// as the corpus tool, git as the revision reader, and the coordination
/// authority that reaches nothing, because this build implements no platform
/// client and saying so is the honest answer.
pub fn execute(home: &Path, operation: Operation) -> Answer<service::Answer> {
    let layout = Layout::new(home);
    let producer = Library;
    let corpus = SpecSpineCommand::default();
    let probe = CommandProbe::default();
    let authority = Unreachable::default();
    let revisions = GitRevision;
    let clock = SystemClock;
    let ports = service::Ports {
        home: &layout,
        producer: &producer,
        corpus: &corpus,
        target_probe: &probe,
        authority: &authority,
        revisions: &revisions,
        clock: &clock,
        native_root: statecraft_home::delivery::native_parent(),
        product_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    wrap(service::execute(&ports, operation))
}

fn answer_exit(answer: &service::Answer) -> Exit {
    match answer.severity() {
        Severity::Ok => Exit::Ok,
        Severity::Finding => Exit::Finding,
        Severity::Refused => Exit::Refused,
        Severity::Failed => Exit::Failed,
    }
}

/// The explicit run choices in an invocation, as `key=value` arguments.
///
/// An option that would alter behavior is an argument, visible in the
/// invocation that produced a run record, rather than ambient state (spec 006
/// section 3.6). An argument that is not a `key=value` pair is ignored here and
/// reported by the caller, never guessed at.
pub fn choices(args: &[String]) -> (RunChoices, Vec<String>) {
    let mut choices = RunChoices::none();
    let mut unparsed = Vec::new();
    for arg in args {
        match arg.split_once('=') {
            Some((key, value)) if !key.is_empty() => {
                choices = choices.choosing(key, value);
            }
            _ => unparsed.push(arg.clone()),
        }
    }
    (choices, unparsed)
}

/// Build the answer's `Answer<T>` wrapper from a service answer.
///
/// Separate from [`execute`] so a test can hold the mapping without a product
/// home, a producer or a process.
pub fn wrap(answer: service::Answer) -> Answer<service::Answer> {
    let summary = answer.render();
    let exit = answer_exit(&answer);
    Answer::new(answer, exit, summary)
}

/// The settings intent an invocation of `home apply` carries.
///
/// Spec 002 section 3.24: the modification is **refused by default**, so the
/// absence of a flag is [`Intent::Withheld`] and not a shorthand for consent.
/// The token is the one the plan printed, repeated back, which is what makes
/// consent specific to the modification the operator actually read. It covers
/// both halves of that modification: the exact content, and the file it would
/// go into as it stood when the plan was computed. Either one changing
/// produces a different token, so a plan nobody reviewed is presented rather
/// than performed.
///
/// `None` means the flags do not name an operation, which the caller renders as
/// a usage error rather than guessing at.
pub fn settings_intent(args: &[String]) -> Option<Intent> {
    let mut intent = Intent::Withheld;
    let mut seen = 0usize;
    let mut i = 0usize;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--remove-settings" {
            intent = Intent::Remove;
            seen += 1;
        } else if let Some(token) = arg.strip_prefix("--consent-settings=") {
            intent = Intent::Consented {
                token: token.to_string(),
            };
            seen += 1;
        } else if arg == "--consent-settings" {
            let token = args.get(i + 1)?;
            if token.starts_with("--") {
                return None;
            }
            intent = Intent::Consented {
                token: token.clone(),
            };
            seen += 1;
            i += 1;
        } else if arg.starts_with("--") {
            // An unrecognised flag on a verb that writes is a usage error, not
            // something to ignore: ignoring it is how a mistyped consent flag
            // reads as a silent refusal.
            return None;
        }
        i += 1;
    }
    // Two intents in one invocation name no single operation. Refusing is the
    // answer; picking the last one would be this binding deciding a policy.
    if seen > 1 { None } else { Some(intent) }
}

/// Which operation a verb names, given what followed it.
pub fn operation(
    verb: crate::commands::Verb,
    rest: &[String],
    root: Option<&Path>,
) -> Option<Operation> {
    use crate::commands::Verb;
    let root = || root.map(Path::to_path_buf);
    Some(match verb {
        Verb::HomeShow => Operation::HomeShow,
        Verb::HomePlan => Operation::HomePlan,
        Verb::HomeApply => Operation::HomeApply {
            settings: settings_intent(rest)?,
        },
        Verb::InitPlan => Operation::InitPlan { root: root()? },
        Verb::InitApply => Operation::InitApply { root: root()? },
        Verb::MigratePlan => Operation::MigratePlan { root: root()? },
        Verb::MigrateApply => Operation::MigrateApply { root: root()? },
        Verb::ProjectEnroll => Operation::Enroll {
            root: root()?,
            team: rest.first()?.clone(),
        },
        Verb::ProjectUnenroll => Operation::Unenroll { root: root()? },
        Verb::ConfigShow => {
            let (choices, _) = choices(rest);
            Operation::ConfigShow {
                root: root()?,
                base_revision: DEFAULT_BASE_REVISION.to_string(),
                choices,
            }
        }
        Verb::ApprovalGrant => Operation::ApprovalGrant {
            root: root()?,
            subject: rest.first()?.clone(),
            operator: rest.get(1)?.clone(),
            reason: rest.get(2..).map(|r| r.join(" ")).unwrap_or_default(),
        },
        Verb::ApprovalShow => Operation::ApprovalShow {
            root: root()?,
            subject: rest.first()?.clone(),
        },
        // Spec 006 section 3.11.1. Each is one operation on the same boundary,
        // and the parse is the whole binding: nothing here decides what a
        // standing means, what an upgrade would do, or whether evidence
        // qualifies a session.
        Verb::HarnessShow => Operation::HarnessShow { root: root()? },
        Verb::HarnessUpgrade => Operation::HarnessUpgrade { root: root()? },
        Verb::SessionPayload => Operation::SessionPayload,
        Verb::StartupRecord => Operation::StartupRecord {
            root: root()?,
            session_id: rest.first()?.clone(),
        },
        Verb::StartupCapture => startup_capture(root()?, rest)?,
        Verb::StartupQualify => Operation::StartupQualify {
            root: root()?,
            session_id: rest.first()?.clone(),
            submission: std::path::PathBuf::from(rest.get(1)?),
        },
        _ => return None,
    })
}

/// `startup capture`'s arguments, and nothing it could use to describe an
/// invocation: the arguments, prompt, commands and settings are constructed by
/// the operation (spec 006 section 3.11.2). The environment is this process's
/// own, passed through exactly, because the provider is the operator's.
fn startup_capture(root: std::path::PathBuf, rest: &[String]) -> Option<Operation> {
    let control = statecraft_home::admission::Control::from_word(rest.first()?)?;
    let directory = std::path::PathBuf::from(rest.get(1)?);
    let mut program = "claude".to_string();
    let mut deadline_seconds = statecraft_home::capture::DEFAULT_DEADLINE_SECONDS;
    let mut synthetic = false;
    let mut i = 2;
    while i < rest.len() {
        match rest[i].as_str() {
            "--program" => {
                program = rest.get(i + 1)?.clone();
                i += 1;
            }
            "--deadline" => {
                deadline_seconds = rest.get(i + 1)?.parse().ok().filter(|s| *s > 0)?;
                i += 1;
            }
            "--synthetic" => synthetic = true,
            _ => return None,
        }
        i += 1;
    }
    Some(Operation::StartupCapture {
        root,
        control,
        directory,
        program,
        deadline_seconds,
        synthetic,
        // A variable that is not UTF-8 cannot be carried in the map the
        // supervisor takes, and is left out rather than panicking.
        environment: std::env::vars_os()
            .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
            .collect(),
    })
}

/// What each managed-environment verb needs after its verb.
pub fn usage(verb: crate::commands::Verb) -> &'static str {
    use crate::commands::Verb;
    match verb {
        Verb::HomeShow | Verb::HomePlan => "",
        Verb::HomeApply => " [--consent-settings <token> | --remove-settings]",
        Verb::ProjectEnroll => " <path> <team>",
        Verb::ConfigShow => " <path> [key=value ...]",
        Verb::ApprovalGrant => " <path> <subject> <operator> <reason...>",
        Verb::ApprovalShow => " <path> <subject>",
        Verb::SessionPayload => "",
        Verb::StartupRecord => " <path> <session-id>",
        Verb::StartupCapture => {
            " <path> <refusal|allowed-command|without-payload> <capture-dir> \
             [--program <executable>] [--deadline <seconds>] [--synthetic]"
        }
        Verb::StartupQualify => " <path> <session-id> <capture-dir>",
        _ => " <path>",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Verb;
    use statecraft_home::flow;

    #[test]
    fn a_complete_initialization_exits_zero_and_a_partial_one_is_a_finding() {
        let report = |outcome| {
            service::Answer::Init(Box::new(flow::Report {
                mode: flow::Mode::Apply,
                root: "/p".into(),
                steps: vec![],
                writes: vec![],
                withheld: vec![],
                adopted: vec![],
                conformance: None,
                bridge: None,
                delivery: vec![],
                qualification: None,
                outcome,
            }))
        };
        assert_eq!(wrap(report(flow::Outcome::Complete)).exit, Exit::Ok);
        assert_eq!(wrap(report(flow::Outcome::Partial)).exit, Exit::Finding);
        assert_eq!(wrap(report(flow::Outcome::Refused)).exit, Exit::Refused);
    }

    #[test]
    fn a_refusal_and_a_failure_are_different_exits() {
        assert_eq!(
            wrap(service::Answer::Refused { reason: "x".into() }).exit,
            Exit::Refused
        );
        assert_eq!(
            wrap(service::Answer::Failed { reason: "x".into() }).exit,
            Exit::Failed
        );
    }

    #[test]
    fn key_value_arguments_become_run_choices_and_anything_else_is_kept_back() {
        let args: Vec<String> = ["model=approved-a", "loose", "k="]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (choices, unparsed) = choices(&args);
        assert_eq!(
            choices.0.get("model").map(String::as_str),
            Some("approved-a")
        );
        assert_eq!(choices.0.get("k").map(String::as_str), Some(""));
        assert_eq!(unparsed, ["loose"]);
    }

    #[test]
    fn a_verb_missing_its_argument_names_no_operation() {
        assert!(operation(Verb::ApprovalShow, &[], Some(Path::new("/p"))).is_none());
        assert!(operation(Verb::InitApply, &[], None).is_none());
        assert!(operation(Verb::HomeShow, &[], None).is_some());
    }

    #[test]
    fn home_apply_withholds_the_settings_modification_unless_it_is_consented_to() {
        let argv = |s: &str| -> Vec<String> { s.split_whitespace().map(str::to_string).collect() };
        // No flag is a refusal, not a shorthand for consent.
        assert_eq!(settings_intent(&[]), Some(Intent::Withheld));
        assert_eq!(
            settings_intent(&argv("--consent-settings abc123")),
            Some(Intent::Consented {
                token: "abc123".into()
            })
        );
        assert_eq!(
            settings_intent(&argv("--consent-settings=abc123")),
            Some(Intent::Consented {
                token: "abc123".into()
            })
        );
        assert_eq!(
            settings_intent(&argv("--remove-settings")),
            Some(Intent::Remove)
        );
        // A consent flag with no token, a mistyped flag, and two intents at
        // once each name no operation, so each is a usage error rather than a
        // silent withholding.
        assert_eq!(settings_intent(&argv("--consent-settings")), None);
        assert_eq!(
            settings_intent(&argv("--consent-settings --remove-settings")),
            None
        );
        assert_eq!(settings_intent(&argv("--consent-setting abc")), None);
        assert_eq!(
            settings_intent(&argv("--remove-settings --consent-settings=x")),
            None
        );
    }

    #[test]
    fn a_mistyped_consent_flag_names_no_operation_at_all() {
        let rest = vec!["--consent-settings".to_string()];
        assert!(operation(Verb::HomeApply, &rest, None).is_none());
        assert!(operation(Verb::HomeApply, &[], None).is_some());
    }

    #[test]
    fn a_reason_with_spaces_is_one_reason() {
        let rest: Vec<String> = ["003-x", "bart", "reviewed", "the", "diff"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        match operation(Verb::ApprovalGrant, &rest, Some(Path::new("/p"))).unwrap() {
            Operation::ApprovalGrant { reason, .. } => assert_eq!(reason, "reviewed the diff"),
            other => panic!("{other:?}"),
        }
    }
}
