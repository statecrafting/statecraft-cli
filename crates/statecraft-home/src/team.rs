//! Solo and team: local approvals, the coordination-authority seam, and
//! eligibility.
//!
//! Spec 002 section 3.18. The complete local capability set requires no
//! platform: no account, no login, no hosted connection, no paid plan and no
//! platform-issued token. That is constitution XIII made operational, and it is
//! why every type here has a local answer.
//!
//! Enrollment is explicit and project-scoped, recorded in the project's own
//! committed declaration and nowhere else. For an enrolled project the platform
//! is the coordination authority for shared approvals, eligibility and policy.
//! When it cannot be reached:
//!
//! - the project does **not** become solo-owned;
//! - work that needs no remote authority proceeds under the recorded policy;
//! - a required shared approval is pending or refused, never granted, and **a
//!   local approval never satisfies one**.
//!
//! The hosted platform is not implemented here. [`Unreachable`] is the shipped
//! implementation of the seam, and it reports `unavailable` rather than
//! inventing a verdict. Nothing in this crate presents a fixture as a live
//! integration.

use crate::authority::{TeamAnswer, TeamPolicy};
use crate::home::{HOME_VERSION, HomeError, Layout};
use serde::{Deserialize, Serialize};
use statecraft_environment::manifest::{Enrollment, Project};
use std::path::Path;

/// What the coordination authority says about one subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum SharedApproval {
    /// Granted, by somebody, at some time.
    Granted {
        /// Who granted it.
        by: String,
        /// When, RFC 3339 UTC.
        at: String,
    },
    /// Refused, with the reason.
    Refused {
        /// Why.
        reason: String,
    },
    /// Asked for and not yet decided.
    Pending {
        /// What it is waiting on.
        reason: String,
    },
    /// The authority could not be reached.
    Unavailable {
        /// Why.
        reason: String,
    },
}

/// The coordination authority a team project defers to.
///
/// A trait, and deliberately the only place a platform appears in this crate.
/// Implementing it is a hosted product's job; this repository implements the
/// local boundary and the honest unavailable state.
pub trait CoordinationAuthority {
    /// The team's shared constraints.
    fn team_policy(&self, team: &str) -> TeamAnswer;
    /// Whether a subject carries a shared approval.
    fn shared_approval(&self, team: &str, subject: &str) -> SharedApproval;
    /// What this authority is, for a report to name.
    fn describe(&self) -> String;
}

/// The shipped implementation: nothing is reachable.
///
/// Not a stub and not a placeholder. It is the correct answer for a build that
/// implements no platform client, and it is the answer every offline test needs
/// to be able to rely on.
#[derive(Debug, Clone)]
pub struct Unreachable {
    /// Why nothing is reachable, in words a report can carry.
    pub reason: String,
}

impl Default for Unreachable {
    fn default() -> Self {
        Self {
            reason: "this build implements no platform client, so no coordination authority \
                     can be reached"
                .to_string(),
        }
    }
}

impl CoordinationAuthority for Unreachable {
    fn team_policy(&self, team: &str) -> TeamAnswer {
        TeamAnswer::Unavailable {
            team: team.to_string(),
            reason: self.reason.clone(),
        }
    }
    fn shared_approval(&self, _team: &str, _subject: &str) -> SharedApproval {
        SharedApproval::Unavailable {
            reason: self.reason.clone(),
        }
    }
    fn describe(&self) -> String {
        format!("unreachable ({})", self.reason)
    }
}

/// An authority backed by explicit answers, for tests.
///
/// Answers a test wrote are evidence about this product's behavior and about
/// nothing else. A verdict from here is never reported as a live integration.
#[derive(Debug, Clone, Default)]
pub struct Stated {
    /// The policy to answer with, if any.
    pub policy: Option<TeamPolicy>,
    /// Subject to approval.
    pub approvals: Vec<(String, SharedApproval)>,
}

impl CoordinationAuthority for Stated {
    fn team_policy(&self, team: &str) -> TeamAnswer {
        match &self.policy {
            Some(policy) => TeamAnswer::Policy {
                team: team.to_string(),
                policy: policy.clone(),
            },
            None => TeamAnswer::Unavailable {
                team: team.to_string(),
                reason: "no policy stated".to_string(),
            },
        }
    }
    fn shared_approval(&self, _team: &str, subject: &str) -> SharedApproval {
        self.approvals
            .iter()
            .find(|(s, _)| s == subject)
            .map(|(_, a)| a.clone())
            .unwrap_or(SharedApproval::Pending {
                reason: format!("no decision recorded for {subject}"),
            })
    }
    fn describe(&self) -> String {
        "stated (a test fixture, never a live integration)".to_string()
    }
}

/// The team answer for a project, from whichever authority it defers to.
pub fn team_answer(project: &Project, authority: &dyn CoordinationAuthority) -> TeamAnswer {
    match project.enrollment.team() {
        None => TeamAnswer::NotEnrolled,
        Some(team) => authority.team_policy(team),
    }
}

/// One operator's approval, recorded locally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApproval {
    /// What was approved: a spec id, a run id, or any subject a caller names.
    pub subject: String,
    /// Who approved it.
    pub operator: String,
    /// Why, recorded so an eligibility can surface it.
    pub reason: String,
    /// When, RFC 3339 UTC.
    pub recorded_at: String,
}

/// The local approvals for one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApprovals {
    /// Schema version.
    pub version: u32,
    /// Which project these are for, absolute, for a human reading the file.
    pub project: String,
    /// One per subject, ordered.
    #[serde(default)]
    pub approvals: Vec<LocalApproval>,
}

impl LocalApprovals {
    /// An empty set for a project.
    pub fn new(project: &Path) -> Self {
        Self {
            version: HOME_VERSION,
            project: project.display().to_string(),
            approvals: Vec::new(),
        }
    }

    /// The approval for a subject, if there is one.
    pub fn get(&self, subject: &str) -> Option<&LocalApproval> {
        self.approvals.iter().find(|a| a.subject == subject)
    }

    /// Record an approval, replacing any earlier one for the same subject.
    pub fn grant(&mut self, approval: LocalApproval) {
        match self
            .approvals
            .binary_search_by(|a| a.subject.cmp(&approval.subject))
        {
            Ok(i) => self.approvals[i] = approval,
            Err(i) => self.approvals.insert(i, approval),
        }
    }

    /// Where this project's approvals live under a home.
    ///
    /// Named by a digest of the absolute path, because a path is not a
    /// filename, and carrying the path inside the file keeps it readable.
    pub fn path(layout: &Layout, project: &Path) -> std::path::PathBuf {
        let digest =
            statecraft_environment::digest::digest_bytes(project.display().to_string().as_bytes());
        layout
            .approvals_dir()
            .join(format!("{}.json", &digest[..16]))
    }

    /// Read a project's approvals, or an empty set.
    pub fn read(layout: &Layout, project: &Path) -> Result<Self, HomeError> {
        let path = Self::path(layout, project);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::new(project)),
            Err(source) => return Err(HomeError::Io { path, source }),
        };
        let value: Self =
            serde_json::from_slice(&bytes).map_err(|source| HomeError::Malformed {
                path: path.clone(),
                source,
            })?;
        if value.version != HOME_VERSION {
            return Err(HomeError::UnknownVersion {
                path,
                found: value.version,
            });
        }
        Ok(value)
    }

    /// Write a project's approvals.
    pub fn write(&self, layout: &Layout, project: &Path) -> Result<(), HomeError> {
        let path = Self::path(layout, project);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| HomeError::Io {
                path: path.clone(),
                source,
            })?;
        }
        let mut json =
            serde_json::to_string_pretty(self).map_err(|source| HomeError::Malformed {
                path: path.clone(),
                source,
            })?;
        json.push('\n');
        std::fs::write(&path, json).map_err(|source| HomeError::Io { path, source })
    }
}

/// Where an eligibility's authority came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum AuthoritySource {
    /// One operator, on this machine.
    LocalOperator {
        /// Who.
        operator: String,
    },
    /// A team, through its coordination authority.
    Team {
        /// Which team.
        team: String,
        /// Who granted it there.
        by: String,
    },
}

/// Whether a subject may proceed, and on whose authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Eligibility {
    /// It may, and this is who said so.
    Eligible {
        /// The authority.
        authority: AuthoritySource,
    },
    /// Not yet. Nothing is refused; nothing is granted either.
    Pending {
        /// What it is waiting on.
        reason: String,
    },
    /// No.
    Refused {
        /// Why.
        reason: String,
    },
}

impl Eligibility {
    /// A one-word rendering.
    pub fn word(&self) -> &'static str {
        match self {
            Eligibility::Eligible { .. } => "eligible",
            Eligibility::Pending { .. } => "pending",
            Eligibility::Refused { .. } => "refused",
        }
    }

    /// True only for `eligible`.
    pub fn eligible(&self) -> bool {
        matches!(self, Eligibility::Eligible { .. })
    }

    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Eligibility::Eligible {
                authority: AuthoritySource::LocalOperator { operator },
            } => format!("eligible on the local operator's authority ({operator})"),
            Eligibility::Eligible {
                authority: AuthoritySource::Team { team, by },
            } => format!("eligible on team {team}'s authority (granted by {by})"),
            Eligibility::Pending { reason } => format!("pending: {reason}"),
            Eligibility::Refused { reason } => format!("refused: {reason}"),
        }
    }
}

/// Whether a subject needs a shared approval in this project.
pub fn requires_shared_approval(project: &Project, subject: &str) -> bool {
    matches!(project.enrollment, Enrollment::Team { .. })
        && project
            .shared_approval_required
            .iter()
            .any(|s| s == subject)
}

/// The eligibility of one subject.
///
/// The one rule worth stating twice: where a shared approval is required, the
/// local record is **not consulted at all**. An offline team project does not
/// fall back to the operator's own say-so, which is what would make enrollment
/// decorative.
pub fn eligibility(
    project: &Project,
    subject: &str,
    local: Option<&LocalApproval>,
    authority: &dyn CoordinationAuthority,
) -> Eligibility {
    if requires_shared_approval(project, subject) {
        let team = project.enrollment.team().expect("checked above");
        return match authority.shared_approval(team, subject) {
            SharedApproval::Granted { by, at: _ } => Eligibility::Eligible {
                authority: AuthoritySource::Team {
                    team: team.to_string(),
                    by,
                },
            },
            SharedApproval::Refused { reason } => Eligibility::Refused {
                reason: format!("team {team} refused {subject}: {reason}"),
            },
            SharedApproval::Pending { reason } => Eligibility::Pending {
                reason: format!("team {team} has not decided {subject}: {reason}"),
            },
            SharedApproval::Unavailable { reason } => Eligibility::Pending {
                reason: format!(
                    "{subject} requires team {team}'s shared approval and the coordination \
                     authority is unavailable ({reason}); a local approval does not satisfy one"
                ),
            },
        };
    }

    match local {
        Some(approval) => Eligibility::Eligible {
            authority: AuthoritySource::LocalOperator {
                operator: approval.operator.clone(),
            },
        },
        None => Eligibility::Pending {
            reason: format!("no local approval is recorded for {subject}"),
        },
    }
}

/// Why a local grant will not be recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantRefused {
    /// Why.
    pub reason: String,
}

/// Check that a local grant is allowed before recording it.
///
/// A local approval for a subject that requires a shared one is refused at the
/// point of recording, not silently filed and then ignored. A record that has
/// no effect is worse than no record: somebody will read it as consent.
pub fn may_grant_locally(project: &Project, subject: &str) -> Result<(), GrantRefused> {
    if requires_shared_approval(project, subject) {
        let team = project.enrollment.team().unwrap_or("the team");
        return Err(GrantRefused {
            reason: format!(
                "{subject} requires team {team}'s shared approval; a local approval cannot \
                 satisfy one, and recording it would read as consent it is not"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solo_project() -> Project {
        Project::default()
    }

    fn team_project(subjects: &[&str]) -> Project {
        Project {
            enrollment: Enrollment::Team {
                team: "acme".into(),
            },
            shared_approval_required: subjects.iter().map(|s| (*s).to_string()).collect(),
            ..Project::default()
        }
    }

    fn approval(subject: &str) -> LocalApproval {
        LocalApproval {
            subject: subject.into(),
            operator: "bart".into(),
            reason: "reviewed".into(),
            recorded_at: "1970-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn a_solo_project_is_eligible_on_the_local_operators_authority() {
        let a = approval("003-x");
        let e = eligibility(&solo_project(), "003-x", Some(&a), &Unreachable::default());
        assert!(e.eligible());
        assert_eq!(
            e,
            Eligibility::Eligible {
                authority: AuthoritySource::LocalOperator {
                    operator: "bart".into()
                }
            }
        );
    }

    #[test]
    fn a_solo_project_with_no_approval_is_pending_not_refused() {
        let e = eligibility(&solo_project(), "003-x", None, &Unreachable::default());
        assert_eq!(e.word(), "pending");
    }

    #[test]
    fn a_solo_project_never_needs_the_platform() {
        // The whole local path, against an authority that reaches nothing.
        let a = approval("003-x");
        assert!(
            eligibility(&solo_project(), "003-x", Some(&a), &Unreachable::default()).eligible()
        );
        assert_eq!(
            team_answer(&solo_project(), &Unreachable::default()),
            TeamAnswer::NotEnrolled
        );
    }

    #[test]
    fn an_offline_team_project_cannot_be_satisfied_by_a_local_approval() {
        let project = team_project(&["003-x"]);
        let local = approval("003-x");
        let e = eligibility(&project, "003-x", Some(&local), &Unreachable::default());
        assert_eq!(e.word(), "pending");
        assert!(
            e.describe()
                .contains("a local approval does not satisfy one")
        );
        // And the project is still a team project.
        assert_eq!(
            project.enrollment,
            Enrollment::Team {
                team: "acme".into()
            }
        );
    }

    #[test]
    fn recording_a_local_approval_for_a_shared_subject_is_refused_outright() {
        let project = team_project(&["003-x"]);
        let refused = may_grant_locally(&project, "003-x").unwrap_err();
        assert!(refused.reason.contains("cannot satisfy"));
        // A subject the team does not reserve is still local.
        assert!(may_grant_locally(&project, "004-y").is_ok());
    }

    #[test]
    fn work_that_needs_no_remote_authority_proceeds_while_the_platform_is_down() {
        let project = team_project(&["003-x"]);
        let local = approval("004-y");
        let e = eligibility(&project, "004-y", Some(&local), &Unreachable::default());
        assert!(e.eligible());
    }

    #[test]
    fn a_granted_shared_approval_names_the_team_as_the_authority() {
        let project = team_project(&["003-x"]);
        let stated = Stated {
            policy: None,
            approvals: vec![(
                "003-x".to_string(),
                SharedApproval::Granted {
                    by: "reviewer".into(),
                    at: "1970-01-01T00:00:00Z".into(),
                },
            )],
        };
        match eligibility(&project, "003-x", None, &stated) {
            Eligibility::Eligible {
                authority: AuthoritySource::Team { team, by },
            } => {
                assert_eq!(team, "acme");
                assert_eq!(by, "reviewer");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_refused_shared_approval_is_refused_and_not_pending() {
        let project = team_project(&["003-x"]);
        let stated = Stated {
            policy: None,
            approvals: vec![(
                "003-x".to_string(),
                SharedApproval::Refused {
                    reason: "not this quarter".into(),
                },
            )],
        };
        assert_eq!(
            eligibility(&project, "003-x", None, &stated).word(),
            "refused"
        );
    }

    #[test]
    fn the_shipped_authority_is_unavailable_and_says_so() {
        let u = Unreachable::default();
        assert!(matches!(
            u.shared_approval("acme", "x"),
            SharedApproval::Unavailable { .. }
        ));
        assert!(matches!(
            u.team_policy("acme"),
            TeamAnswer::Unavailable { .. }
        ));
        assert!(u.describe().contains("unreachable"));
    }

    #[test]
    fn approvals_round_trip_per_project_and_do_not_leak_between_projects() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path());
        let one = Path::new("/projects/one");
        let two = Path::new("/projects/two");

        let mut a = LocalApprovals::new(one);
        a.grant(approval("003-x"));
        a.write(&layout, one).unwrap();

        assert_eq!(LocalApprovals::read(&layout, one).unwrap(), a);
        assert!(
            LocalApprovals::read(&layout, two)
                .unwrap()
                .approvals
                .is_empty()
        );
        assert_ne!(
            LocalApprovals::path(&layout, one),
            LocalApprovals::path(&layout, two)
        );
    }

    #[test]
    fn granting_twice_replaces_rather_than_duplicates() {
        let mut a = LocalApprovals::new(Path::new("/p"));
        a.grant(approval("003-x"));
        let mut second = approval("003-x");
        second.reason = "re-reviewed".into();
        a.grant(second);
        assert_eq!(a.approvals.len(), 1);
        assert_eq!(a.get("003-x").unwrap().reason, "re-reviewed");
    }
}
