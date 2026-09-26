//! Two renderings of one value.
//!
//! Spec 006 section 3.4. Every command supports `--json`. Human output and JSON
//! output are two renderings of the **same** returned value, produced from it by
//! this crate, never two code paths that compute their own answers.
//!
//! JSON output is a contract: adding a field is compatible, removing or
//! retyping one is a change to the spec that owns it. Human output is not a
//! contract and may be reshaped freely, which is exactly why a caller is given
//! `--json` instead.
//!
//! Spec 007 fixes the JSON rendering as the family envelope spec-spine emits
//! from 0.26.0 (its spec 132 section 3.4): `schemaVersion`, `tool`, `verb`,
//! `outcome`, `exitCode`, `summary`, then exactly one of `report` (exit 0 or 1)
//! and `error` (exit 2, 3 or 4), with keys sorted and a trailing newline.

use crate::commands::Verb;
use crate::exit::Exit;
use serde::Serialize;
use serde_json::{Map, Value};

/// The envelope's own version, on its own axis (spec 007 section 3.1).
///
/// A field added is MINOR; a field removed or retyped, or an `error.kind`
/// outside [`ErrorKind::all`], is MAJOR.
pub const ENVELOPE_SCHEMA_VERSION: &str = "1.0.0";

/// The `tool` member: the executable's recorded name (spec 006 section 3.5),
/// stated once here and asserted equal to the built binary's by the tests.
pub const TOOL: &str = "statecraft-cli";

/// Which rendering the caller asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// For a person.
    Human,
    /// For a caller. The contract. Carries the verb the envelope names.
    Json(Verb),
}

/// The closed set of `error.kind` tokens (spec 007 section 3.2): the family's,
/// spelled as spec-spine spells them, and never extended locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Authored content that does not validate.
    Validation,
    /// A derived artifact that does not match its sources.
    Stale,
    /// A named thing that does not exist.
    NotFound,
    /// Code and its owning specification disagree.
    Drift,
    /// A precondition was not met and nothing was done.
    Refused,
    /// Configuration that cannot be used.
    Config,
    /// A read or write that did not complete.
    Io,
    /// A document that does not parse as the shape it must have.
    Schema,
    /// Arguments that name no operation.
    Usage,
    /// A defect in this product.
    Internal,
}

impl ErrorKind {
    /// Every kind. A test asserts the set stays the family's ten.
    pub fn all() -> [ErrorKind; 10] {
        [
            ErrorKind::Validation,
            ErrorKind::Stale,
            ErrorKind::NotFound,
            ErrorKind::Drift,
            ErrorKind::Refused,
            ErrorKind::Config,
            ErrorKind::Io,
            ErrorKind::Schema,
            ErrorKind::Usage,
            ErrorKind::Internal,
        ]
    }

    /// The token this kind is reported as.
    pub fn word(self) -> &'static str {
        match self {
            ErrorKind::Validation => "validation",
            ErrorKind::Stale => "stale",
            ErrorKind::NotFound => "not-found",
            ErrorKind::Drift => "drift",
            ErrorKind::Refused => "refused",
            ErrorKind::Config => "config",
            ErrorKind::Io => "io",
            ErrorKind::Schema => "schema",
            ErrorKind::Usage => "usage",
            ErrorKind::Internal => "internal",
        }
    }
}

/// A command's answer: one value, plus how it should end the process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer<T: Serialize> {
    /// What the command found.
    pub value: T,
    /// The exit this implies.
    pub exit: Exit,
    /// What a person should read.
    ///
    /// Derived from `value` by the command, and carried in the JSON too, so the
    /// two renderings cannot disagree about what happened: there is one value
    /// and this is a view of it.
    pub summary: String,
    /// The kind a refusal or failure names, where its site knows better than
    /// the exit's default (see [`Answer::error_kind`]).
    pub kind: Option<ErrorKind>,
}

impl<T: Serialize> Answer<T> {
    /// An answer.
    pub fn new(value: T, exit: Exit, summary: impl Into<String>) -> Self {
        Self {
            value,
            exit,
            summary: summary.into(),
            kind: None,
        }
    }

    /// The same answer, naming the kind its site knows the refusal or failure
    /// to be. Ignored for exits 0 and 1, which carry a report, not an error.
    pub fn with_kind(mut self, kind: ErrorKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// The kind the envelope's `error` names: `None` for 0 and 1; otherwise the
    /// site's own, or the exit's default (`refused` for 2, `usage` for 3, `io`
    /// for 4, because every failure this product constructs is a read or write
    /// of its own store or the target that did not complete).
    pub fn error_kind(&self) -> Option<ErrorKind> {
        match self.exit {
            Exit::Ok | Exit::Finding => None,
            Exit::Usage => Some(ErrorKind::Usage),
            Exit::Refused => Some(self.kind.unwrap_or(ErrorKind::Refused)),
            Exit::Failed => Some(self.kind.unwrap_or(ErrorKind::Io)),
        }
    }

    /// The family envelope for this answer, as `verb` (spec 007 section 3.1).
    pub fn envelope(&self, verb: &str) -> Value {
        let value = serde_json::to_value(&self.value).unwrap_or_else(|e| {
            // A value this crate built and cannot serialize is a defect in this
            // crate, and saying so is better than a panic in an operator's
            // terminal.
            Value::String(format!("could not serialize the answer: {e}"))
        });
        let body = match self.error_kind() {
            None => Body::Report(value),
            Some(kind) => Body::Error {
                kind,
                details: value,
            },
        };
        envelope(verb, self.exit, &self.summary, body)
    }

    /// Render for the chosen format.
    pub fn render(&self, format: Format) -> String {
        match format {
            Format::Human => {
                if self.summary.ends_with('\n') {
                    self.summary.clone()
                } else {
                    format!("{}\n", self.summary)
                }
            }
            Format::Json(verb) => canonical(&self.envelope(&verb.dotted())),
        }
    }
}

/// What follows the header.
enum Body {
    Report(Value),
    Error { kind: ErrorKind, details: Value },
}

fn envelope(verb: &str, exit: Exit, summary: &str, body: Body) -> Value {
    let summary = summary.trim_end_matches('\n');
    let mut m = Map::new();
    m.insert("schemaVersion".into(), ENVELOPE_SCHEMA_VERSION.into());
    m.insert("tool".into(), TOOL.into());
    m.insert("verb".into(), verb.into());
    m.insert("outcome".into(), exit.word().into());
    m.insert("exitCode".into(), exit.code().into());
    m.insert("summary".into(), summary.into());
    match body {
        Body::Report(value) => {
            m.insert("report".into(), value);
        }
        Body::Error { kind, details } => {
            let mut e = Map::new();
            e.insert("kind".into(), kind.word().into());
            e.insert("message".into(), summary.into());
            // `details` carries what the refusal already carried, and is
            // omitted when it would only repeat the message.
            let repeats = match &details {
                Value::Null => true,
                Value::String(s) => s.trim_end_matches('\n') == summary,
                _ => false,
            };
            if !repeats {
                e.insert("details".into(), details);
            }
            m.insert("error".into(), Value::Object(e));
        }
    }
    Value::Object(m)
}

/// Canonical JSON: sorted keys (serde_json's map is ordered), two-space
/// pretty-print and one trailing newline, as spec-spine writes its envelope.
fn canonical(value: &Value) -> String {
    let mut s = serde_json::to_string_pretty(value)
        .unwrap_or_else(|e| format!("{{\"error\":\"could not serialize the answer: {e}\"}}"));
    s.push('\n');
    s
}

/// A usage error under `--json` (spec 007 section 3.3): the envelope on stdout,
/// `error.kind` `usage`, exit 3. `verb` is the operation's dotted name, or the
/// words as typed, dotted, when they name none.
pub fn usage_envelope(verb: &str, message: &str) -> String {
    canonical(&envelope(
        verb,
        Exit::Usage,
        message,
        Body::Error {
            kind: ErrorKind::Usage,
            details: Value::Null,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, PartialEq, Eq, Debug, Clone)]
    struct Found {
        verdict: String,
        reasons: Vec<String>,
    }

    fn answer() -> Answer<Found> {
        Answer::new(
            Found {
                verdict: "ungoverned".into(),
                reasons: vec!["no spec-spine corpus is present".into()],
            },
            Exit::Finding,
            "ungoverned: no spec-spine corpus is present",
        )
    }

    #[test]
    fn both_renderings_come_from_one_value() {
        let a = answer();
        let human = a.render(Format::Human);
        let json = a.render(Format::Json(Verb::Doctor));
        // The same fact appears in both, because there is one value behind them.
        assert!(human.contains("no spec-spine corpus is present"));
        assert!(json.contains("no spec-spine corpus is present"));
    }

    fn parsed(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn the_json_rendering_is_the_family_envelope() {
        let json = answer().render(Format::Json(Verb::Doctor));
        let v = parsed(&json);
        assert_eq!(v["schemaVersion"], ENVELOPE_SCHEMA_VERSION);
        assert_eq!(v["tool"], TOOL);
        assert_eq!(v["verb"], "doctor");
        assert_eq!(v["outcome"], "finding");
        assert_eq!(v["exitCode"], 1);
        assert_eq!(v["summary"], "ungoverned: no spec-spine corpus is present");
        assert_eq!(v["report"]["verdict"], "ungoverned");
        assert!(v.get("error").is_none());
        // Spec 007 section 3.1: the envelope replaces `value` and `exit`.
        assert!(v.get("value").is_none() && v.get("exit").is_none());
    }

    #[test]
    fn keys_are_sorted_and_the_rendering_ends_with_one_newline() {
        let json = answer().render(Format::Json(Verb::EnvApply));
        let keys: Vec<&str> = json
            .lines()
            .filter(|l| l.starts_with("  \""))
            .map(|l| l.trim_start().split('"').nth(1).unwrap())
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
        assert!(json.ends_with("}\n"));
        assert_eq!(parsed(&json)["verb"], "env.apply");
    }

    #[test]
    fn a_refusal_carries_an_error_and_no_report() {
        let a = Answer::new(answer().value, Exit::Refused, "not armed");
        let v = parsed(&a.render(Format::Json(Verb::Run)));
        assert!(v.get("report").is_none());
        assert_eq!(v["outcome"], "refused");
        assert_eq!(v["exitCode"], 2);
        assert_eq!(v["error"]["kind"], "refused");
        assert_eq!(v["error"]["message"], "not armed");
        assert_eq!(v["error"]["details"]["verdict"], "ungoverned");
    }

    #[test]
    fn each_error_exit_has_a_default_kind_and_a_site_may_name_its_own() {
        let refused = Answer::new(String::new(), Exit::Refused, "x");
        assert_eq!(refused.error_kind(), Some(ErrorKind::Refused));
        let failed = Answer::new(String::new(), Exit::Failed, "x");
        assert_eq!(failed.error_kind(), Some(ErrorKind::Io));
        let usage = Answer::new(String::new(), Exit::Usage, "x").with_kind(ErrorKind::Io);
        assert_eq!(usage.error_kind(), Some(ErrorKind::Usage));
        let found = Answer::new(String::new(), Exit::Refused, "x").with_kind(ErrorKind::NotFound);
        assert_eq!(found.error_kind(), Some(ErrorKind::NotFound));
        // A report never carries a kind, whatever a site named.
        let ok = Answer::new(String::new(), Exit::Ok, "x").with_kind(ErrorKind::Io);
        assert_eq!(ok.error_kind(), None);
    }

    #[test]
    fn details_that_only_repeat_the_message_are_omitted() {
        let a = Answer::new("gone\n".to_string(), Exit::Failed, "gone\n").with_kind(ErrorKind::Io);
        let v = parsed(&a.render(Format::Json(Verb::ProjectList)));
        assert_eq!(
            v["error"],
            serde_json::json!({"kind": "io", "message": "gone"})
        );
        assert_eq!(v["summary"], "gone");
    }

    #[test]
    fn the_kind_set_is_the_familys_ten() {
        let words: Vec<&str> = ErrorKind::all().iter().map(|k| k.word()).collect();
        assert_eq!(
            words,
            [
                "validation",
                "stale",
                "not-found",
                "drift",
                "refused",
                "config",
                "io",
                "schema",
                "usage",
                "internal"
            ]
        );
    }

    #[test]
    fn a_usage_envelope_names_the_verb_and_the_usage_kind() {
        let v = parsed(&usage_envelope("run.list", "usage: run list <path>"));
        assert_eq!(v["exitCode"], 3);
        assert_eq!(v["outcome"], "usage");
        assert_eq!(v["verb"], "run.list");
        assert_eq!(v["error"]["kind"], "usage");
        assert!(v.get("report").is_none());
    }

    #[test]
    fn the_human_rendering_ends_with_exactly_one_newline() {
        let out = answer().render(Format::Human);
        assert!(out.ends_with('\n'));
        assert!(!out.ends_with("\n\n"));
    }
}
