//! Managed-session permission delivery.
//!
//! Spec 002 section 3.27, settled by the owner on 2026-09-21. It separates two
//! things this build had joined: registering an adapter **globally**, and
//! delivering the deny floor to a **managed session**.
//!
//! # Why they are not one delivery
//!
//! A hook script can be gated to a Statecraft project because it *runs*: it
//! tests for `.statecraft/environment.json` and exits, which is what
//! [`crate::harness::GATE`] states and what every shipped script does. A deny
//! entry has no such moment. The harness evaluates it before anything of this
//! product's runs, so it carries no gate and acquires none from the scripts
//! registered beside it. Written into a user's global settings it would refuse
//! those commands in every repository that user opens, managed or not, which
//! is what section 3.14 rule 3 forbids.
//!
//! So [`crate::settings`] carries hook registrations into the global file and
//! nothing else, and the floor is delivered here, scoped to one session.
//!
//! # Documentation is not evidence
//!
//! Claude Code documents `--settings <file-or-json>`, which is the right shape
//! for this. Section 3.27 does not let that be the end of it: the **installed
//! version** and the **effective behavior** are both verified before this
//! product relies on the mechanism, and an unverified mechanism is an
//! **unavailable** one.
//!
//! Those are two different measurements and only one of them can be made
//! without a provider session. [`probe_version`] reads the installed binary
//! and establishes that the argument exists. Whether a refusal passed that way
//! is actually **enforced**, in this version, for this command, is a claim
//! about a running session, and nothing short of a running session establishes
//! it. Until one has, [`Qualification::NotQualified`] is the answer, and
//! section 3.27's last paragraph fixes what may not be done about that: the
//! floor is not lowered so delivery can succeed, and no repository-local
//! generic harness copy is written to make the limitation invisible.

use serde::Serialize;
use std::path::Path;
use std::process::Command;

/// The harness whose session settings this module knows how to build.
pub const SUPPORTED_HARNESS: &str = "claude-code";

/// The argument that carries per-session settings.
pub const SETTINGS_ARGUMENT: &str = "--settings";

/// The canonical settings a managed session runs under.
///
/// The deny floor of section 3.23, and nothing else. No allow entry, no `ask`
/// setting, no `defaultMode`, no model selection: section 3.28 names those as
/// the user's, and a session payload that carried them would be deciding them
/// on the user's behalf every time a session started.
pub fn payload() -> serde_json::Value {
    serde_json::json!({
        "permissions": {
            "deny": crate::settings::DENY_FLOOR,
        }
    })
}

/// The payload as the exact bytes the argument would receive.
pub fn payload_json() -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(&payload()).expect("a Value serializes")
    )
}

/// Whether the mechanism has been established well enough to rely on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Qualification {
    /// The mechanism was measured end to end and the floor is enforced.
    ///
    /// Reachable only from a live-session observation. Nothing in this module
    /// produces it from a file read, and that is the point.
    ///
    /// It carries the evidence for the same reason
    /// [`crate::startup::Observation::Observed`] does: section 3.29 rule 5
    /// makes every route to an admitted observation run the same admission,
    /// and a variant that carried only two strings would be a route that
    /// carried none of it. Constructing this by hand is possible and is not a
    /// shortcut: whatever is put in `evidence` is what the admission judges.
    Qualified {
        /// The harness version the observation was made against.
        version: String,
        /// What was observed, in one line.
        observed: String,
        /// What it was admitted from.
        evidence: Box<crate::admission::Evidence>,
    },
    /// The mechanism is not established, so this session does not qualify.
    ///
    /// Not a failure and not a refusal: a session still runs, it simply may
    /// not be described as carrying the managed-execution claim.
    NotQualified {
        /// The harness version, where one could be read.
        version: Option<String>,
        /// What is established, and what is not.
        reason: String,
    },
}

impl Qualification {
    /// A one-line rendering.
    pub fn describe(&self) -> String {
        match self {
            Qualification::Qualified {
                version, observed, ..
            } => {
                format!("qualified against {version}: {observed}")
            }
            Qualification::NotQualified { version, reason } => format!(
                "not qualified for the managed-execution claim ({}): {reason}",
                version.as_deref().unwrap_or("version unread")
            ),
        }
    }

    /// True only for a measured, live observation.
    ///
    /// The shape alone is not enough: a `Qualified` built by hand carries
    /// evidence, and that evidence is re-judged here and must not be synthetic
    /// (spec 002 sections 3.29 rule 5 and 3.30).
    pub fn qualified(&self) -> bool {
        match self {
            Qualification::Qualified { evidence, .. } => {
                crate::admission::admit(evidence).is_ok() && !evidence.synthetic()
            }
            Qualification::NotQualified { .. } => false,
        }
    }
}

/// What reading the installed harness binary established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionProbe {
    /// The version the binary reports, when it could be read.
    pub version: Option<String>,
    /// Whether the binary's own help names [`SETTINGS_ARGUMENT`].
    pub argument_present: bool,
}

/// Read the installed harness binary: its version, and whether it carries the
/// argument this delivery needs.
///
/// A read. It launches the binary with `--version` and `--help`, which write
/// nothing and start no session.
pub fn probe_version(binary: &Path) -> VersionProbe {
    let run = |arg: &str| -> Option<String> {
        let out = Command::new(binary).arg(arg).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).to_string())
    };
    VersionProbe {
        version: run("--version")
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        argument_present: run("--help")
            .map(|h| h.contains(SETTINGS_ARGUMENT))
            .unwrap_or(false),
    }
}

/// The qualification a probe alone supports, which is never [`Qualification::Qualified`].
///
/// Section 3.27: the installed version **and** the effective behavior are
/// verified. This establishes the first. The second is a statement about a
/// running session, in the sense of spec 002 section 3.26's third evidence
/// class, and a file read cannot produce one however carefully it is
/// performed. A configured deny entry is not an enforced one (section 3.28).
pub fn qualification_from(probe: &VersionProbe) -> Qualification {
    let reason = match (&probe.version, probe.argument_present) {
        (None, _) => "the harness binary did not answer, so neither its version nor its \
                      arguments are established"
            .to_string(),
        (Some(_), false) => format!(
            "this version's help does not name `{SETTINGS_ARGUMENT}`, so there is no verified \
             mechanism to deliver the floor to a session with"
        ),
        (Some(_), true) => format!(
            "`{SETTINGS_ARGUMENT}` is present in this version, which establishes the mechanism \
             exists and not that a refusal passed through it is enforced; enforcement is an \
             observation of a running session and no read of a settings file substitutes for \
             one"
        ),
    };
    Qualification::NotQualified {
        version: probe.version.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_payload_carries_the_floor_and_no_permission() {
        let value = payload();
        let deny = value
            .pointer("/permissions/deny")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(deny.len(), crate::settings::DENY_FLOOR.len());
        // Section 3.28: the keys this product does not touch. A session
        // payload that set them would decide them for the user on every start.
        for key in ["allow", "ask", "defaultMode", "additionalDirectories"] {
            assert!(
                value.pointer(&format!("/permissions/{key}")).is_none(),
                "the session payload carries permissions.{key}"
            );
        }
        assert!(value.get("model").is_none());
        assert_eq!(value.as_object().unwrap().len(), 1);
    }

    #[test]
    fn a_probe_alone_never_qualifies_a_session() {
        // Every shape a probe can take, including the most favourable one.
        for probe in [
            VersionProbe {
                version: None,
                argument_present: false,
            },
            VersionProbe {
                version: Some("2.1.267".into()),
                argument_present: false,
            },
            VersionProbe {
                version: Some("2.1.267".into()),
                argument_present: true,
            },
        ] {
            let q = qualification_from(&probe);
            assert!(
                !q.qualified(),
                "a file and flag read qualified a session: {q:?}"
            );
            assert!(matches!(q, Qualification::NotQualified { .. }));
        }
    }

    #[test]
    fn the_favourable_probe_says_what_it_does_and_does_not_establish() {
        let q = qualification_from(&VersionProbe {
            version: Some("2.1.267".into()),
            argument_present: true,
        });
        let text = q.describe();
        assert!(text.contains("not qualified"), "{text}");
        assert!(
            text.contains("enforced") || text.contains("enforcement"),
            "the reason does not name what is missing: {text}"
        );
    }

    #[test]
    fn a_missing_binary_is_not_qualified_rather_than_a_panic() {
        let probe = probe_version(Path::new("/nonexistent/claude-binary"));
        assert_eq!(probe.version, None);
        assert!(!probe.argument_present);
        assert!(!qualification_from(&probe).qualified());
    }
}
