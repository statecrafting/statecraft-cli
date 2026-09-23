//! The contract an attempt is bound to: one context closure, resolved by the
//! producer, written into the attempt's intent.
//!
//! Spec 003 section 3.1.3, recorded on 2026-09-22. spec-spine's specs 106 and
//! 107 resolve qualified obligation references and context closures; the
//! resolution is the producer's, and it is pure. This module asks for one
//! closure per attempt (the unit of work's spec and every obligation the spec
//! declares), records the answer verbatim, and names every other answer. It
//! computes no digest, reads no ledger file, and decides nothing about whether
//! a bound contract still holds: spec 005 section 3.18 does that, from a new
//! resolution of the same request.

use crate::report::{SpecLifecycle, SpecSpineCli};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// The command every resolution runs, recorded beside what it answered.
pub const COMMAND: &str = "registry closure --request - --json";

/// A closure request, in the producer's own shape (spec-spine 107).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Whole specs, by id.
    pub specs: Vec<String>,
    /// Qualified obligation references, `<spec-id>#<obligation-id>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub obligations: Vec<String>,
}

/// Rule 1: the unit of work's spec, and every obligation `registry list` says
/// it declares, withdrawn ones included. Nothing is added or dropped.
pub fn request_for(spec_id: &str, lifecycle: Option<&SpecLifecycle>) -> Request {
    Request {
        specs: vec![spec_id.to_string()],
        obligations: lifecycle
            .map(|l| {
                l.obligations
                    .iter()
                    .map(|o| format!("{spec_id}#{}", o.id))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// What the producer answered for one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "answer")]
pub enum Resolution {
    /// The closure resolved.
    Resolved {
        /// The producer's order-independent digest.
        digest: String,
        /// Every member, verbatim.
        members: Vec<serde_json::Value>,
    },
    /// The producer does not answer `registry closure --help` with success.
    Unsupported {
        /// What it answered.
        detail: String,
    },
    /// The producer refused a member (its exit 1).
    Unresolved {
        /// Its message, naming the member.
        detail: String,
    },
    /// The producer refused a stale ledger (its exit 2).
    Stale {
        /// Its message.
        detail: String,
    },
    /// The answer was not the JSON this build reads, or the producer could
    /// not be run.
    Unreadable {
        /// What went wrong.
        detail: String,
    },
}

/// Where closures come from. A trait so tests need no producer.
pub trait ContractSource {
    /// The producer's version, for the record.
    fn producer_version(&self, target: &Path) -> String;
    /// Resolve one request in `target`.
    fn resolve(&self, target: &Path, request: &Request) -> Resolution;
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no detail")
        .to_string()
}

impl ContractSource for SpecSpineCli {
    fn producer_version(&self, target: &Path) -> String {
        Command::new(&self.binary)
            .arg("--version")
            .current_dir(target)
            .output()
            .map(|o| crate::report::version_token(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    }

    fn resolve(&self, target: &Path, request: &Request) -> Resolution {
        // Support is asked, never inferred from an exit code: the pinned
        // 0.20.0 answers an unknown subcommand with its usage code, and 2 is
        // also the producer's code for a stale ledger.
        let probe = Command::new(&self.binary)
            .args(["registry", "closure", "--help"])
            .current_dir(target)
            .output();
        match probe {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                return Resolution::Unsupported {
                    detail: format!(
                        "`registry closure --help` exited {}: {}",
                        o.status
                            .code()
                            .map_or("by a signal".to_string(), |c| c.to_string()),
                        first_line(&o.stderr)
                    ),
                };
            }
            Err(e) => {
                return Resolution::Unreadable {
                    detail: format!("could not run {}: {e}", self.binary),
                };
            }
        }
        let body = match serde_json::to_vec(request) {
            Ok(b) => b,
            Err(e) => {
                return Resolution::Unreadable {
                    detail: e.to_string(),
                };
            }
        };
        let child = Command::new(&self.binary)
            .args(["registry", "closure", "--request", "-", "--json"])
            .current_dir(target)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                return Resolution::Unreadable {
                    detail: format!("could not run {}: {e}", self.binary),
                };
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&body);
        }
        let out = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                return Resolution::Unreadable {
                    detail: e.to_string(),
                };
            }
        };
        interpret(out.status.code(), &out.stdout, &out.stderr)
    }
}

/// Read one `registry closure` answer by its exit code and bytes.
pub fn interpret(code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Resolution {
    match code {
        Some(0) => {
            #[derive(Deserialize)]
            struct Answer {
                digest: String,
                members: Vec<serde_json::Value>,
            }
            match serde_json::from_slice::<Answer>(stdout) {
                Ok(a) if !a.digest.is_empty() => Resolution::Resolved {
                    digest: a.digest,
                    members: a.members,
                },
                Ok(_) => Resolution::Unreadable {
                    detail: "the closure answered an empty digest".to_string(),
                },
                Err(e) => Resolution::Unreadable {
                    detail: format!("the closure answer is not readable: {e}"),
                },
            }
        }
        Some(1) => Resolution::Unresolved {
            detail: first_line(stderr),
        },
        Some(2) => Resolution::Stale {
            detail: first_line(stderr),
        },
        other => Resolution::Unreadable {
            detail: format!(
                "`{COMMAND}` exited {}: {}",
                other.map_or("by a signal".to_string(), |c| c.to_string()),
                first_line(stderr)
            ),
        },
    }
}

/// `detail.contract` in an attempt's intent (rules 2 and 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    /// Which of rule 3's states.
    pub state: State,
    /// What was asked, where anything was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<Request>,
    /// The digest, where the closure resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// Every member, verbatim, where the closure resolved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<serde_json::Value>,
    /// The producer version that answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
    /// The command that answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// What was answered, where the state is not `bound`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Rule 3's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// The closure resolved.
    Bound,
    /// The producer does not resolve closures.
    Unsupported,
    /// The producer refused a member.
    Unresolved,
    /// The producer refused a stale ledger.
    Stale,
    /// The answer could not be read.
    Unreadable,
    /// Spec 002 section 3.33's trial: authorized against no spec.
    NotAUnitOfWork,
}

impl State {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            State::Bound => "bound",
            State::Unsupported => "unsupported",
            State::Unresolved => "unresolved",
            State::Stale => "stale",
            State::Unreadable => "unreadable",
            State::NotAUnitOfWork => "not-a-unit-of-work",
        }
    }
}

impl Binding {
    /// The binding a resolution makes.
    pub fn of(request: Request, resolution: Resolution, producer: &str) -> Self {
        let base = |state: State, detail: Option<String>| Binding {
            state,
            request: Some(request.clone()),
            digest: None,
            members: Vec::new(),
            producer: Some(producer.to_string()),
            command: Some(COMMAND.to_string()),
            detail,
        };
        match resolution {
            Resolution::Resolved { digest, members } => Binding {
                digest: Some(digest),
                members,
                ..base(State::Bound, None)
            },
            Resolution::Unsupported { detail } => base(
                State::Unsupported,
                Some(format!(
                    "spec-spine {producer} was asked for a closure and did not answer one: {detail}"
                )),
            ),
            Resolution::Unresolved { detail } => base(State::Unresolved, Some(detail)),
            Resolution::Stale { detail } => base(State::Stale, Some(detail)),
            Resolution::Unreadable { detail } => base(State::Unreadable, Some(detail)),
        }
    }

    /// The trial's binding: no spec, so no request.
    pub fn not_a_unit_of_work() -> Self {
        Binding {
            state: State::NotAUnitOfWork,
            request: None,
            digest: None,
            members: Vec::new(),
            producer: None,
            command: None,
            detail: Some(
                "spec 002 section 3.33's trial is authorized against no spec, so no contract \
                 was asked for"
                    .to_string(),
            ),
        }
    }

    /// Read a binding back from an intent's `detail`. `None` where the
    /// intent carries none, which is every intent written before section
    /// 3.1.3.
    pub fn from_detail(detail: &serde_json::Value) -> Option<Result<Self, String>> {
        detail
            .get("contract")
            .map(|v| serde_json::from_value(v.clone()).map_err(|e| e.to_string()))
    }

    /// One line an operator reads.
    pub fn describe(&self) -> String {
        match (self.state, &self.digest) {
            (State::Bound, Some(digest)) => format!(
                "bound {digest}: {} member(s), from spec-spine {}",
                self.members.len(),
                self.producer.as_deref().unwrap_or("unknown")
            ),
            (state, _) => format!(
                "{}: {}",
                state.word(),
                self.detail.as_deref().unwrap_or("no detail")
            ),
        }
    }
}

/// Bind one unit of work's contract (rules 1 to 3).
pub fn bind(
    source: &dyn ContractSource,
    target: &Path,
    spec_id: &str,
    lifecycle: Option<&SpecLifecycle>,
) -> Binding {
    let request = request_for(spec_id, lifecycle);
    let producer = source.producer_version(target);
    let resolution = source.resolve(target, &request);
    Binding::of(request, resolution, &producer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::DeclaredObligation;

    fn recorded(report: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("testdata/producer/released-0.23.0")
                .join(report),
        )
        .unwrap()
    }

    // The published 0.23.0's own answers, recorded; before these, every
    // closure this crate read was written by hand in the shape measured.
    #[test]
    fn a_recorded_closure_answer_from_the_published_producer_resolves() {
        match interpret(Some(0), &recorded("closure-002.json"), b"") {
            Resolution::Resolved { digest, members } => {
                assert_eq!(digest.len(), 64, "{digest}");
                assert_eq!(members.len(), 1, "{members:?}");
                assert_eq!(members[0]["kind"], "spec");
                assert_eq!(members[0]["spec"], "002-environment-lifecycle");
                assert!(members[0]["contentHash"].as_str().is_some());
            }
            other => panic!("expected a resolved closure, got {other:?}"),
        }
    }

    #[test]
    fn a_recorded_refusal_of_a_missing_member_is_unresolved_and_names_it() {
        match interpret(
            Some(1),
            &recorded("closure-missing.stdout"),
            &recorded("closure-missing.stderr"),
        ) {
            Resolution::Unresolved { detail } => assert!(detail.contains("999-absent"), "{detail}"),
            other => panic!("expected an unresolved member, got {other:?}"),
        }
    }

    fn lifecycle() -> SpecLifecycle {
        SpecLifecycle {
            id: "107-x".into(),
            status: "approved".into(),
            implementation: Some("complete".into()),
            obligations: vec![
                DeclaredObligation {
                    id: "R-1".into(),
                    withdrawn: false,
                },
                DeclaredObligation {
                    id: "R-2".into(),
                    withdrawn: true,
                },
            ],
        }
    }

    #[test]
    fn the_request_is_the_spec_and_every_declared_obligation_withdrawn_ones_included() {
        let r = request_for("107-x", Some(&lifecycle()));
        assert_eq!(r.specs, ["107-x"]);
        assert_eq!(r.obligations, ["107-x#R-1", "107-x#R-2"]);
        let bare = request_for("002-y", None);
        assert!(bare.obligations.is_empty());
        // The producer's shape: no `obligations` key when there are none.
        assert_eq!(
            serde_json::to_string(&bare).unwrap(),
            r#"{"specs":["002-y"]}"#
        );
    }

    #[test]
    fn each_exit_code_is_its_own_answer() {
        let ok = br#"{"digest":"d","members":[{"kind":"spec","spec":"a","contentHash":"h"}],"schemaVersion":"0.6.0"}"#;
        assert!(matches!(
            interpret(Some(0), ok, b""),
            Resolution::Resolved { .. }
        ));
        assert!(matches!(
            interpret(Some(1), b"", b"spec-spine: not found: spec '099'"),
            Resolution::Unresolved { .. }
        ));
        assert!(matches!(
            interpret(Some(2), b"", b"stale"),
            Resolution::Stale { .. }
        ));
        assert!(matches!(
            interpret(Some(0), b"{", b""),
            Resolution::Unreadable { .. }
        ));
        assert!(matches!(
            interpret(Some(3), b"", b"usage"),
            Resolution::Unreadable { .. }
        ));
        assert!(matches!(
            interpret(None, b"", b""),
            Resolution::Unreadable { .. }
        ));
    }

    #[test]
    fn a_binding_round_trips_through_an_intent_detail_and_says_what_it_is() {
        let b = Binding::of(
            request_for("107-x", Some(&lifecycle())),
            Resolution::Resolved {
                digest: "abc".into(),
                members: vec![serde_json::json!({"kind":"spec","spec":"107-x","contentHash":"h"})],
            },
            "0.22.0",
        );
        let detail = serde_json::json!({"baseCommit": "x", "contract": b});
        assert_eq!(Binding::from_detail(&detail).unwrap().unwrap(), b);
        assert!(
            b.describe()
                .starts_with("bound abc: 1 member(s), from spec-spine 0.22.0")
        );
        assert!(Binding::from_detail(&serde_json::json!({"baseCommit": "x"})).is_none());
        let unsupported = Binding::of(
            request_for("107-x", None),
            Resolution::Unsupported {
                detail: "exited 3".into(),
            },
            "0.20.0",
        );
        // It names the producer that answered and asserts nothing about a release.
        let line = unsupported.describe();
        assert!(line.contains("spec-spine 0.20.0 was asked"), "{line}");
        assert!(!line.contains("release"), "{line}");
        assert_eq!(
            Binding::not_a_unit_of_work().state.word(),
            "not-a-unit-of-work"
        );
    }
}
