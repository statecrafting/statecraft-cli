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
        }
    }

    /// Which spec owns the behavior behind it.
    pub fn owning_spec(self) -> &'static str {
        // Every verb in the tree today is spec 002's. That is not a coincidence
        // to be tidied away: 003 to 005 own behavior that has no operator verb
        // bound yet, and adding one is the change that binds it.
        "002-environment-lifecycle"
    }

    /// Every verb, in the order the help text lists them.
    pub fn all() -> [Verb; 9] {
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
        ]
    }

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

    match Verb::parse(args) {
        Some((verb, consumed)) => {
            let tail = &args[consumed..];
            Ok(Invocation {
                verb,
                rest: tail.iter().filter(|a| *a != "--json").cloned().collect(),
                json: tail.iter().any(|a| a == "--json"),
            })
        }
        None => Err(UsageError {
            given: args.join(" "),
            available,
        }),
    }
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
