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
        // Spec 002 section 3.24's settings intent. Until spec 006 binds the
        // flags that choose it, `home apply` asks for what it always did: the
        // plan for the modification, and no write.
        Verb::HomeApply => Operation::HomeApply {
            settings: statecraft_home::settings::Intent::Withheld,
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
        _ => return None,
    })
}

/// What each managed-environment verb needs after its verb.
pub fn usage(verb: crate::commands::Verb) -> &'static str {
    use crate::commands::Verb;
    match verb {
        Verb::HomeShow | Verb::HomePlan | Verb::HomeApply => "",
        Verb::ProjectEnroll => " <path> <team>",
        Verb::ConfigShow => " <path> [key=value ...]",
        Verb::ApprovalGrant => " <path> <subject> <operator> <reason...>",
        Verb::ApprovalShow => " <path> <subject>",
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
