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
    /// `home show`
    HomeShow,
    /// `home plan`
    HomePlan,
    /// `home apply`
    HomeApply,
    /// `init plan <path>`
    InitPlan,
    /// `init apply <path>`
    InitApply,
    /// `migrate plan <path>`
    MigratePlan,
    /// `migrate apply <path>`
    MigrateApply,
    /// `project enroll <path> <team>`
    ProjectEnroll,
    /// `project unenroll <path>`
    ProjectUnenroll,
    /// `config show <path>`
    ConfigShow,
    /// `approval grant <path> <subject> <operator> <reason...>`
    ApprovalGrant,
    /// `approval show <path> <subject>`
    ApprovalShow,
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
            Verb::HomeShow => "home show",
            Verb::HomePlan => "home plan",
            Verb::HomeApply => "home apply",
            Verb::InitPlan => "init plan",
            Verb::InitApply => "init apply",
            Verb::MigratePlan => "migrate plan",
            Verb::MigrateApply => "migrate apply",
            Verb::ProjectEnroll => "project enroll",
            Verb::ProjectUnenroll => "project unenroll",
            Verb::ConfigShow => "config show",
            Verb::ApprovalGrant => "approval grant",
            Verb::ApprovalShow => "approval show",
            Verb::Help => "--help",
        }
    }

    /// Which spec owns the behavior behind it.
    ///
    /// Spec 006's edges added the verbs 003, 004 and 005 name, so this is no
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
            // `run` is 003's semantics through 004's adapter, and 006 section
            // 3.1 names both. The record and the outcome are 003's, so that is
            // the owner; the adapter is how the attempt happens.
            Verb::Run => "003-work-and-run-semantics",
            Verb::RunShow | Verb::Accept => "005-acceptance-and-evidence",
            // Spec 010's verbs. The behavior behind each is in that spec's
            // crate, and the binding reaches it through one `extends` edge on
            // the crate 006 owns, which is how 006 section 3.1 admits a verb.
            Verb::HomeShow
            | Verb::HomePlan
            | Verb::HomeApply
            | Verb::InitPlan
            | Verb::InitApply
            | Verb::MigratePlan
            | Verb::MigrateApply
            | Verb::ProjectEnroll
            | Verb::ProjectUnenroll
            | Verb::ConfigShow
            | Verb::ApprovalGrant
            | Verb::ApprovalShow => "010-managed-environment-and-initialization",
            Verb::Help => "006-command-surface",
        }
    }

    /// Every operation, in the order the help text lists them.
    ///
    /// [`Verb::Help`] is deliberately absent: it is not an operation, and a
    /// usage error listing it would offer help as a thing to do.
    pub fn all() -> [Verb; 27] {
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
            Verb::HomeShow,
            Verb::HomePlan,
            Verb::HomeApply,
            Verb::InitPlan,
            Verb::InitApply,
            Verb::MigratePlan,
            Verb::MigrateApply,
            Verb::ProjectEnroll,
            Verb::ProjectUnenroll,
            Verb::ConfigShow,
            Verb::ApprovalGrant,
            Verb::ApprovalShow,
        ]
    }

    /// The groups a help topic may name.
    pub const GROUPS: [&'static str; 9] = [
        "project", "env", "work", "run", "accept", "home", "init", "migrate", "config",
    ];

    /// Parse a verb from the leading arguments, returning how many it consumed.
    pub fn parse(args: &[String]) -> Option<(Verb, usize)> {
        let first = args.first()?.as_str();
        let second = args.get(1).map(String::as_str);
        match (first, second) {
            ("project", Some("register")) => Some((Verb::ProjectRegister, 2)),
            ("project", Some("list")) => Some((Verb::ProjectList, 2)),
            ("project", Some("arm")) => Some((Verb::ProjectArm, 2)),
            ("project", Some("enroll")) => Some((Verb::ProjectEnroll, 2)),
            ("project", Some("unenroll")) => Some((Verb::ProjectUnenroll, 2)),
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
            ("home", Some("show")) => Some((Verb::HomeShow, 2)),
            ("home", Some("plan")) => Some((Verb::HomePlan, 2)),
            ("home", Some("apply")) => Some((Verb::HomeApply, 2)),
            ("init", Some("plan")) => Some((Verb::InitPlan, 2)),
            ("init", Some("apply")) => Some((Verb::InitApply, 2)),
            ("migrate", Some("plan")) => Some((Verb::MigratePlan, 2)),
            ("migrate", Some("apply")) => Some((Verb::MigrateApply, 2)),
            ("config", Some("show")) => Some((Verb::ConfigShow, 2)),
            ("approval", Some("grant")) => Some((Verb::ApprovalGrant, 2)),
            ("approval", Some("show")) => Some((Verb::ApprovalShow, 2)),
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
    out.push_str(
        "\nMost verbs take a target path, and every verb accepts --json.\n\
         The `home` verbs read and write the product's own home and take no path.\n\
         `init plan` and `home plan` write nothing; the matching `apply` performs it.\n\
         Initialization stops after registering and qualifying: arming and running\n\
         are separate explicit acts.\n",
    );
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
    fn the_new_groups_have_a_preview_and_a_performance_spelled_the_same_way() {
        // `env plan` / `env apply` is the vocabulary this binary already has,
        // so `home` and `init` and `migrate` use it rather than inventing a
        // second spelling for the same distinction.
        for group in ["home", "init", "migrate"] {
            let plan = parse(&argv(&format!("{group} plan"))).unwrap();
            let apply = parse(&argv(&format!("{group} apply"))).unwrap();
            assert_ne!(plan.verb, apply.verb);
            assert!(plan.verb.spelling().ends_with("plan"));
            assert!(apply.verb.spelling().ends_with("apply"));
        }
    }

    #[test]
    fn every_group_names_at_least_one_verb_in_the_help() {
        for group in Verb::GROUPS {
            let text = help_text(&[group.to_string()]);
            assert!(
                !text.starts_with("no verbs match"),
                "the help has nothing for the group `{group}`"
            );
        }
    }

    #[test]
    fn a_reason_with_spaces_survives_parsing() {
        let i = parse(&argv("approval grant /p 003-x bart reviewed the diff")).unwrap();
        assert_eq!(i.verb, Verb::ApprovalGrant);
        assert_eq!(i.rest, ["/p", "003-x", "bart", "reviewed", "the", "diff"]);
    }

    #[test]
    fn there_is_no_verb_that_publishes() {
        for v in Verb::all() {
            assert!(!v.spelling().contains("publish"));
            assert!(!v.spelling().contains("release"));
        }
    }
}
