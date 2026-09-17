//! The command tree, and the rule that a command is only a binding.
//!
//! Spec 006 sections 3.1 and 3.2. A command parses arguments, calls exactly one
//! library operation, renders the value it returns, and maps it to an exit code.
//! **It contains no rule the owning spec did not state.**
//!
//! Verbs owned by specs 003 to 005 join this tree as each is bound, by an
//! `extends` edge from spec 006 naming that spec. A verb is added by the change
//! that binds it, never ahead of it: a command that prints "not implemented" is
//! a worse answer than a command that does not exist, because only one of them
//! is discoverable as absent.

use serde::{Deserialize, Serialize};

/// A verb this binary has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verb {
    /// `project register <path>`
    ProjectRegister,
    /// `project list`
    ProjectList,
    /// `project arm <path>`
    ProjectArm,
    /// `project disarm <path>`
    ProjectDisarm,
    /// `env plan`
    EnvPlan,
    /// `env apply`
    EnvApply,
    /// `env upgrade`
    EnvUpgrade,
    /// `env remove`
    EnvRemove,
    /// `doctor`
    Doctor,
    /// `work list <path>`
    WorkList,
    /// `work show <path> <id>`
    WorkShow,
    /// `run <path> <id>`
    Run,
    /// `run list <path>`
    RunList,
    /// `run show <path> <run>`
    RunShow,
    /// `accept <path> <run>`
    Accept,
    /// `--help`, optionally with a group or a verb as its topic.
    ///
    /// Not part of the command tree: [`Verb::all`] lists the operations, and a
    /// help request is not one. It is a verb here only so one dispatch handles
    /// every invocation.
    Help,
}

impl Verb {
    /// How the verb is written on a command line.
    pub fn spelling(self) -> &'static str {
        match self {
            Verb::ProjectRegister => "project register",
            Verb::ProjectList => "project list",
            Verb::ProjectArm => "project arm",
            Verb::ProjectDisarm => "project disarm",
            Verb::EnvPlan => "env plan",
            Verb::EnvApply => "env apply",
            Verb::EnvUpgrade => "env upgrade",
            Verb::EnvRemove => "env remove",
            Verb::Doctor => "doctor",
            Verb::WorkList => "work list",
            Verb::WorkShow => "work show",
            Verb::Run => "run",
            Verb::RunList => "run list",
            Verb::RunShow => "run show",
            Verb::Accept => "accept",
            Verb::Help => "--help",
        }
    }

    /// Which spec owns the behavior behind it.
    ///
    /// Spec 009's edge added the verbs 003, 004 and 005 name, so this is no
    /// longer one answer. Spec 006 section 3.1's table is where the mapping
    /// lives; this is that table, in code.
    pub fn owning_spec(self) -> &'static str {
        match self {
            Verb::ProjectRegister
            | Verb::ProjectList
            | Verb::ProjectArm
            | Verb::ProjectDisarm
            | Verb::EnvPlan
            | Verb::EnvApply
            | Verb::EnvUpgrade
            | Verb::EnvRemove
            | Verb::Doctor => "002-environment-lifecycle",
            Verb::WorkList | Verb::WorkShow | Verb::RunList => "003-work-and-run-semantics",
            // `run` is 003's semantics through 004's adapter, and 009 section
            // 3.1 names both. The record and the outcome are 003's, so that is
            // the owner; the adapter is how the attempt happens.
            Verb::Run => "003-work-and-run-semantics",
            Verb::RunShow | Verb::Accept => "005-acceptance-and-evidence",
            Verb::Help => "006-command-surface",
        }
    }

    /// Every operation, in the order the help text lists them.
    ///
    /// [`Verb::Help`] is deliberately absent: it is not an operation, and a
    /// usage error listing it would offer help as a thing to do.
    pub fn all() -> [Verb; 15] {
        [
            Verb::ProjectRegister,
            Verb::ProjectList,
            Verb::ProjectArm,
            Verb::ProjectDisarm,
            Verb::EnvPlan,
            Verb::EnvApply,
            Verb::EnvUpgrade,
            Verb::EnvRemove,
            Verb::Doctor,
            Verb::WorkList,
            Verb::WorkShow,
            Verb::Run,
            Verb::RunList,
            Verb::RunShow,
            Verb::Accept,
        ]
    }

    /// The groups a help topic may name.
    pub const GROUPS: [&'static str; 5] = ["project", "env", "work", "run", "accept"];

    /// Parse a verb from the leading arguments, returning how many it consumed.
    pub fn parse(args: &[String]) -> Option<(Verb, usize)> {
        let first = args.first()?.as_str();
        let second = args.get(1).map(String::as_str);
        match (first, second) {
            ("project", Some("register")) => Some((Verb::ProjectRegister, 2)),
            ("project", Some("list")) => Some((Verb::ProjectList, 2)),
            ("project", Some("arm")) => Some((Verb::ProjectArm, 2)),
            ("project", Some("disarm")) => Some((Verb::ProjectDisarm, 2)),
            ("env", Some("plan")) => Some((Verb::EnvPlan, 2)),
            ("env", Some("apply")) => Some((Verb::EnvApply, 2)),
            ("env", Some("upgrade")) => Some((Verb::EnvUpgrade, 2)),
            ("env", Some("remove")) => Some((Verb::EnvRemove, 2)),
            ("doctor", _) => Some((Verb::Doctor, 1)),
            ("work", Some("list")) => Some((Verb::WorkList, 2)),
            ("work", Some("show")) => Some((Verb::WorkShow, 2)),
            // `list` and `show` are reserved after `run`, so a run id may not
            // be spelled either of them. Stated here rather than discovered:
            // the alternative is an id that silently becomes a subcommand.
            ("run", Some("list")) => Some((Verb::RunList, 2)),
            ("run", Some("show")) => Some((Verb::RunShow, 2)),
            ("run", _) => Some((Verb::Run, 1)),
            ("accept", _) => Some((Verb::Accept, 1)),
            _ => None,
        }
    }
}

/// What the caller asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The verb.
    pub verb: Verb,
    /// Everything after it.
    pub rest: Vec<String>,
    /// Whether `--json` was given.
    pub json: bool,
    /// Whether this is a help request rather than an operation.
    pub help: bool,
}

/// Why the arguments do not name an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError {
    /// What the caller wrote.
    pub given: String,
    /// The verbs that exist.
    pub available: Vec<String>,
}

impl UsageError {
    /// A message that names the verb and lists the ones that exist.
    pub fn describe(&self) -> String {
        let mut out = if self.given.is_empty() {
            "no verb given\n".to_string()
        } else {
            format!("unknown verb `{}`\n", self.given)
        };
        out.push_str("available verbs:\n");
        for v in &self.available {
            out.push_str(&format!("  {v}\n"));
        }
        out
    }
}

/// Parse a command line.
///
/// `--json` is accepted anywhere after the verb, because an operator adding it
/// to a command they already typed should not have to move it.
pub fn parse(args: &[String]) -> Result<Invocation, UsageError> {
    let available: Vec<String> = Verb::all()
        .iter()
        .map(|v| v.spelling().to_string())
        .collect();

    // A help request is answered before a verb is resolved, because its topic
    // may be a GROUP (`work`, `run`) that is not a verb. `work --help` has to
    // work: it is the only thing that tells an operator which `work` verbs
    // exist, and a usage error would be the wrong answer to a right question.
    if args.iter().any(|a| a == "--help" || a == "-h") {
        let topic: Vec<String> = args
            .iter()
            .filter(|a| !a.starts_with('-'))
            .cloned()
            .collect();
        return Ok(Invocation {
            verb: Verb::Help,
            rest: topic,
            json: args.iter().any(|a| a == "--json"),
            help: true,
        });
    }

    match Verb::parse(args) {
        Some((verb, consumed)) => {
            let tail = &args[consumed..];
            Ok(Invocation {
                verb,
                rest: tail.iter().filter(|a| *a != "--json").cloned().collect(),
                json: tail.iter().any(|a| a == "--json"),
                help: false,
            })
        }
        None => Err(UsageError {
            given: args.join(" "),
            available,
        }),
    }
}

/// The help text for a topic: a group, a verb, or everything.
///
/// A rendering of [`Verb::all`] rather than a second list, so a verb that joins
/// the tree appears here without anyone remembering to add it.
pub fn help_text(topic: &[String]) -> String {
    let prefix = topic.first().map(String::as_str).unwrap_or("");
    let matching: Vec<Verb> = Verb::all()
        .into_iter()
        .filter(|v| prefix.is_empty() || v.spelling().split(' ').next() == Some(prefix))
        .collect();

    let mut out = String::new();
    if matching.is_empty() {
        out.push_str(&format!("no verbs match `{prefix}`\n"));
        out.push_str("groups:\n");
        for g in Verb::GROUPS {
            out.push_str(&format!("  {g}\n"));
        }
        return out;
    }
    out.push_str("verbs:\n");
    for v in matching {
        out.push_str(&format!("  {:<18} {}\n", v.spelling(), v.owning_spec()));
    }
    out.push_str("\nEvery verb takes a registered target path, and every verb accepts --json.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn every_verb_parses_from_its_own_spelling() {
        for v in Verb::all() {
            let parsed = parse(&argv(v.spelling()))
                .unwrap_or_else(|_| panic!("{} did not parse", v.spelling()));
            assert_eq!(parsed.verb, v);
        }
    }

    #[test]
    fn an_unknown_verb_is_a_usage_error_that_names_it_and_lists_the_others() {
        let e = parse(&argv("env publish")).unwrap_err();
        let text = e.describe();
        assert!(text.contains("env publish"));
        assert!(text.contains("env apply"));
        assert!(text.contains("doctor"));
    }

    #[test]
    fn no_verb_at_all_is_a_usage_error_too() {
        let e = parse(&[]).unwrap_err();
        assert!(e.describe().contains("no verb given"));
    }

    #[test]
    fn json_is_accepted_after_the_verb_and_removed_from_the_rest() {
        let i = parse(&argv("project register /tmp/x --json")).unwrap();
        assert!(i.json);
        assert_eq!(i.rest, ["/tmp/x"]);
    }

    #[test]
    fn without_json_the_default_is_the_human_rendering() {
        assert!(!parse(&argv("doctor")).unwrap().json);
    }

    #[test]
    fn there_is_no_verb_that_publishes() {
        for v in Verb::all() {
            assert!(!v.spelling().contains("publish"));
            assert!(!v.spelling().contains("release"));
        }
    }
}
