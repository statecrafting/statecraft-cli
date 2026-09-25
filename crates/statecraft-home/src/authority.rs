//! Configuration authority: four layers, with provenance, and no merge.
//!
//! Spec 002 section 3.16. There is one policy and approval model, and it is not
//! a last-writer-wins merge. Each key is answered with the value, the layer
//! that supplied it, and every layer that constrained it. Three rules make the
//! answer honest:
//!
//! - A run choice outside a constraint is **refused**, naming the constraint
//!   and its origin. It is never silently clamped to the nearest allowed value.
//! - A required policy or a required piece of evidence that is missing resolves
//!   to **unknown**, and unknown is not success.
//! - **A candidate cannot choose or weaken the policy that judges it.** The
//!   trusted configuration is read at the trusted base revision (spec 001
//!   section 3.5.1), and a candidate diff that touches the declaration is
//!   reported as an authority change (spec 001 section 3.5.2).

use crate::home::Personal;
use serde::{Deserialize, Serialize};
use statecraft_environment::manifest::{Enrollment, Manifest, Project};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The reserved requirement value meaning "team policy supplies this".
///
/// A declaration that defers a key is what lets an unreachable platform be
/// `unknown` for exactly the keys that need it, rather than for every key or
/// for none.
pub const DEFER_TO_TEAM: &str = "team";

/// Which layer an answer came from, or was constrained by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    /// The operator's personal defaults. The weakest layer.
    PersonalDefault,
    /// The project's own setting.
    ProjectOverride,
    /// The project's requirement. Binds every other layer.
    ProjectRequirement,
    /// Team policy, when enrolled. Nothing local overrides it.
    TeamPolicy,
    /// An explicit choice in the invocation.
    RunChoice,
}

impl Layer {
    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            Layer::PersonalDefault => "personal-default",
            Layer::ProjectOverride => "project-override",
            Layer::ProjectRequirement => "project-requirement",
            Layer::TeamPolicy => "team-policy",
            Layer::RunChoice => "run-choice",
        }
    }
}

/// The shared constraints a team imposes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPolicy {
    /// Key to the values that key may take. An absent key is unconstrained.
    #[serde(default)]
    pub constraints: BTreeMap<String, Vec<String>>,
}

impl TeamPolicy {
    /// The allowed values for a key, if the team constrains it.
    pub fn allowed(&self, key: &str) -> Option<&[String]> {
        self.constraints.get(key).map(Vec::as_slice)
    }
}

/// What the coordination authority had to say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum TeamAnswer {
    /// The project is solo. There is no team to ask.
    NotEnrolled,
    /// The team answered.
    Policy {
        /// The team.
        team: String,
        /// Its constraints.
        policy: TeamPolicy,
    },
    /// The project is enrolled and the authority could not be reached.
    ///
    /// **Not a downgrade to solo.** The project stays enrolled and the keys it
    /// defers stay unknown.
    Unavailable {
        /// The team.
        team: String,
        /// Why.
        reason: String,
    },
}

impl TeamAnswer {
    /// The team, when there is one.
    pub fn team(&self) -> Option<&str> {
        match self {
            TeamAnswer::NotEnrolled => None,
            TeamAnswer::Policy { team, .. } | TeamAnswer::Unavailable { team, .. } => Some(team),
        }
    }
}

/// Explicit choices in one invocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunChoices(pub BTreeMap<String, String>);

impl RunChoices {
    /// No choices.
    pub fn none() -> Self {
        Self::default()
    }

    /// One choice.
    #[must_use]
    pub fn choosing(mut self, key: &str, value: &str) -> Self {
        self.0.insert(key.to_string(), value.to_string());
        self
    }
}

/// One key's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Answer {
    /// A value, and where it came from.
    Resolved {
        /// The value.
        value: String,
        /// The layer that supplied it.
        supplied_by: Layer,
        /// Every layer that constrained it, in layer order.
        constrained_by: Vec<Layer>,
    },
    /// A value was offered and is not allowed here.
    Refused {
        /// What was offered.
        attempted: String,
        /// The layer whose constraint refused it.
        constrained_by: Layer,
        /// Why.
        reason: String,
    },
    /// Nothing answers for this key, and that is not a default.
    Unknown {
        /// Why.
        reason: String,
    },
}

impl Answer {
    /// A one-word rendering.
    pub fn word(&self) -> &'static str {
        match self {
            Answer::Resolved { .. } => "resolved",
            Answer::Refused { .. } => "refused",
            Answer::Unknown { .. } => "unknown",
        }
    }

    /// The value, when there is one. `None` for refused and unknown, which is
    /// what keeps "unknown is not success" a type-level property rather than a
    /// convention.
    pub fn value(&self) -> Option<&str> {
        match self {
            Answer::Resolved { value, .. } => Some(value.as_str()),
            _ => None,
        }
    }

    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Answer::Resolved {
                value,
                supplied_by,
                constrained_by,
            } => {
                let mut out = format!("{value} ({})", supplied_by.word());
                if !constrained_by.is_empty() {
                    out.push_str(&format!(
                        ", constrained by {}",
                        constrained_by
                            .iter()
                            .map(|l| l.word())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                out
            }
            Answer::Refused {
                attempted,
                constrained_by,
                reason,
            } => format!(
                "refused `{attempted}`: {reason} ({})",
                constrained_by.word()
            ),
            Answer::Unknown { reason } => format!("unknown: {reason}"),
        }
    }
}

/// The resolved configuration for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    /// One answer per key, ordered.
    pub keys: BTreeMap<String, Answer>,
    /// Which declaration answered, and from where.
    pub source: Source,
    /// The authority change, when the candidate's declaration differs from the
    /// trusted one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority_change: Option<AuthorityChange>,
}

impl Resolution {
    /// Whether anything is refused or unknown.
    pub fn has_findings(&self) -> bool {
        self.keys
            .values()
            .any(|a| !matches!(a, Answer::Resolved { .. }))
            || self.authority_change.is_some()
    }

    /// A human-readable rendering.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("configuration from {}\n", self.source.describe()));
        if let Some(change) = &self.authority_change {
            out.push_str(&format!("authority-change: {}\n", change.reason));
        }
        for (key, answer) in &self.keys {
            out.push_str(&format!("{key}: {}\n", answer.describe()));
        }
        out
    }
}

/// Which copy of the declaration was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Source {
    /// The trusted base revision, which is what a run reads.
    TrustedBase {
        /// The revision.
        revision: String,
    },
    /// The working tree, because no trusted baseline exists yet.
    ///
    /// Correct only outside a run: a first initialization has no base to read
    /// from. A run resolving from here is an authority change in itself, and
    /// that is what [`Resolution::authority_change`] says.
    WorkingTree,
}

impl Source {
    /// A one-line rendering.
    pub fn describe(&self) -> String {
        match self {
            Source::TrustedBase { revision } => format!("the trusted base revision {revision}"),
            Source::WorkingTree => "the working tree, with no trusted baseline".to_string(),
        }
    }
}

/// A candidate diff that touches the authority set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityChange {
    /// Why this is one.
    pub reason: String,
    /// The keys whose answers differ between the trusted copy and the
    /// candidate's, in key order.
    pub keys: Vec<String>,
}

/// How a file is read at a revision.
///
/// A trait because the real implementation shells out to `git`, which is the
/// same boundary spec 002's probe draws, and because a test must be able to
/// state a base without building a history to hold it.
pub trait RevisionReader {
    /// A file's text at a revision, or `None` when it is not there.
    fn read_at(&self, root: &Path, revision: &str, repo_relative: &str) -> Option<String>;
}

/// The reader that asks git.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitRevision;

impl RevisionReader for GitRevision {
    fn read_at(&self, root: &Path, revision: &str, repo_relative: &str) -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["show", &format!("{revision}:{repo_relative}")])
            .current_dir(root)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout).ok()
    }
}

/// A reader backed by an explicit table.
#[derive(Debug, Clone, Default)]
pub struct StaticRevision(pub BTreeMap<String, String>);

impl RevisionReader for StaticRevision {
    fn read_at(&self, _root: &Path, revision: &str, repo_relative: &str) -> Option<String> {
        self.0.get(&format!("{revision}:{repo_relative}")).cloned()
    }
}

/// The declaration a run is judged by, and whether the candidate differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trusted {
    /// The project block in force.
    pub project: Project,
    /// Where it came from.
    pub source: Source,
    /// The authority change, when there is one.
    pub authority_change: Option<AuthorityChange>,
}

/// Read the trusted declaration for a run.
///
/// The base revision wins, always. A candidate that changed the declaration is
/// reported and does not take effect, which is the whole of section 3.6's third
/// rule: the thing being judged does not get to choose the rules.
pub fn trusted(
    root: &Path,
    base_revision: &str,
    reader: &dyn RevisionReader,
    candidate: &Project,
) -> Trusted {
    let at_base = reader
        .read_at(root, base_revision, crate::project::DECLARATION)
        .and_then(|text| serde_json::from_str::<Manifest>(&text).ok())
        .map(|m| m.project);

    match at_base {
        Some(base) => {
            let authority_change = (base != *candidate).then(|| AuthorityChange {
                reason: format!(
                    "the candidate changes {}, which is a member of the authority set; \
                     the base revision's declaration is what applies",
                    crate::project::DECLARATION
                ),
                keys: differing_keys(&base, candidate),
            });
            Trusted {
                project: base,
                source: Source::TrustedBase {
                    revision: base_revision.to_string(),
                },
                authority_change,
            }
        }
        None => Trusted {
            project: candidate.clone(),
            source: Source::WorkingTree,
            authority_change: Some(AuthorityChange {
                reason: format!(
                    "no {} exists at {base_revision}, so there is no trusted baseline to \
                     judge this candidate against",
                    crate::project::DECLARATION
                ),
                keys: Vec::new(),
            }),
        },
    }
}

/// Which keys two project blocks disagree about.
fn differing_keys(base: &Project, candidate: &Project) -> Vec<String> {
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for (map_base, map_candidate) in [
        (&base.requirements, &candidate.requirements),
        (&base.overrides, &candidate.overrides),
    ] {
        for key in map_base.keys().chain(map_candidate.keys()) {
            if map_base.get(key) != map_candidate.get(key) {
                keys.insert(key.clone());
            }
        }
    }
    if base.enrollment != candidate.enrollment {
        keys.insert("enrollment".to_string());
    }
    if base.shared_approval_required != candidate.shared_approval_required {
        keys.insert("shared-approval-required".to_string());
    }
    keys.into_iter().collect()
}

/// Resolve every key, with provenance.
pub fn resolve(
    personal: &Personal,
    trusted: &Trusted,
    team: &TeamAnswer,
    run: &RunChoices,
) -> Resolution {
    let project = &trusted.project;
    let mut keys: BTreeSet<String> = BTreeSet::new();
    keys.extend(personal.defaults.keys().cloned());
    keys.extend(project.requirements.keys().cloned());
    keys.extend(project.overrides.keys().cloned());
    keys.extend(run.0.keys().cloned());
    if let TeamAnswer::Policy { policy, .. } = team {
        keys.extend(policy.constraints.keys().cloned());
    }

    let mut answers = BTreeMap::new();
    for key in keys {
        answers.insert(key.clone(), answer_for(&key, personal, project, team, run));
    }

    Resolution {
        keys: answers,
        source: trusted.source.clone(),
        authority_change: trusted.authority_change.clone(),
    }
}

fn answer_for(
    key: &str,
    personal: &Personal,
    project: &Project,
    team: &TeamAnswer,
    run: &RunChoices,
) -> Answer {
    let requirement = project.requirements.get(key);
    let chosen = run.0.get(key);
    let mut constrained_by = Vec::new();

    // A requirement binds every other layer, including the project's own
    // override and the operator's own choice. That is what makes it a
    // requirement rather than a strong default.
    if let Some(required) = requirement {
        if required == DEFER_TO_TEAM {
            return deferred_to_team(key, team, project, run);
        }
        constrained_by.push(Layer::ProjectRequirement);
        if let Some(offered) = chosen
            && offered != required
        {
            return Answer::Refused {
                attempted: offered.clone(),
                constrained_by: Layer::ProjectRequirement,
                reason: format!("the project requires `{required}` for {key}"),
            };
        }
        if let Some(over) = project.overrides.get(key)
            && over != required
        {
            return Answer::Refused {
                attempted: over.clone(),
                constrained_by: Layer::ProjectRequirement,
                reason: format!("the declaration sets {key} to `{over}` and requires `{required}`"),
            };
        }
        return within_team(
            key,
            required.clone(),
            Layer::ProjectRequirement,
            constrained_by,
            team,
        );
    }

    // No requirement: precedence is run choice, then the project's own setting,
    // then the operator's default. Never a merge of the three.
    let (value, layer) = if let Some(v) = chosen {
        (v.clone(), Layer::RunChoice)
    } else if let Some(v) = project.overrides.get(key) {
        (v.clone(), Layer::ProjectOverride)
    } else if let Some(v) = personal.defaults.get(key) {
        (v.clone(), Layer::PersonalDefault)
    } else {
        return match team {
            TeamAnswer::Unavailable { team, reason } => Answer::Unknown {
                reason: format!(
                    "no layer supplies {key}, and team {team}'s policy could not be read: {reason}"
                ),
            },
            _ => Answer::Unknown {
                reason: format!("no layer supplies {key}"),
            },
        };
    };

    within_team(key, value, layer, constrained_by, team)
}

/// Apply the team's constraint to a value some local layer supplied.
fn within_team(
    key: &str,
    value: String,
    supplied_by: Layer,
    mut constrained_by: Vec<Layer>,
    team: &TeamAnswer,
) -> Answer {
    match team {
        TeamAnswer::NotEnrolled => Answer::Resolved {
            value,
            supplied_by,
            constrained_by,
        },
        TeamAnswer::Unavailable { team, reason } => {
            // The key is not deferred, so local authority answers it. The
            // project stays enrolled and the unavailability is still recorded
            // where it matters, which is the deferred keys and the approvals.
            let _ = (team, reason);
            Answer::Resolved {
                value,
                supplied_by,
                constrained_by,
            }
        }
        TeamAnswer::Policy { policy, .. } => match policy.allowed(key) {
            None => Answer::Resolved {
                value,
                supplied_by,
                constrained_by,
            },
            Some(allowed) if allowed.contains(&value) => {
                constrained_by.push(Layer::TeamPolicy);
                constrained_by.sort();
                constrained_by.dedup();
                Answer::Resolved {
                    value,
                    supplied_by,
                    constrained_by,
                }
            }
            Some(allowed) => Answer::Refused {
                attempted: value,
                constrained_by: Layer::TeamPolicy,
                reason: format!("team policy allows {key} in [{}]", allowed.join(", ")),
            },
        },
    }
}

/// A key the declaration defers to team policy.
fn deferred_to_team(key: &str, team: &TeamAnswer, project: &Project, run: &RunChoices) -> Answer {
    match team {
        TeamAnswer::NotEnrolled => Answer::Unknown {
            reason: format!(
                "the declaration defers {key} to team policy and this project is {}",
                match project.enrollment {
                    Enrollment::Solo => "not enrolled",
                    Enrollment::Team { .. } => "enrolled, but no team answer was supplied",
                }
            ),
        },
        TeamAnswer::Unavailable { team, reason } => Answer::Unknown {
            reason: format!(
                "the declaration defers {key} to team {team}, and the coordination authority \
                 could not be reached: {reason}"
            ),
        },
        TeamAnswer::Policy { policy, .. } => match policy.allowed(key) {
            None => Answer::Unknown {
                reason: format!("the declaration defers {key} to team policy, which is silent"),
            },
            Some([only]) => Answer::Resolved {
                value: only.clone(),
                supplied_by: Layer::TeamPolicy,
                constrained_by: vec![Layer::ProjectRequirement, Layer::TeamPolicy],
            },
            Some(allowed) => match run.0.get(key) {
                Some(offered) if allowed.iter().any(|a| a == offered) => Answer::Resolved {
                    value: offered.clone(),
                    supplied_by: Layer::RunChoice,
                    constrained_by: vec![Layer::ProjectRequirement, Layer::TeamPolicy],
                },
                Some(offered) => Answer::Refused {
                    attempted: offered.clone(),
                    constrained_by: Layer::TeamPolicy,
                    reason: format!("team policy allows {key} in [{}]", allowed.join(", ")),
                },
                None => Answer::Unknown {
                    reason: format!(
                        "team policy allows {key} in [{}] and nothing chose one",
                        allowed.join(", ")
                    ),
                },
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn personal(pairs: &[(&str, &str)]) -> Personal {
        let mut p = Personal::default();
        for (k, v) in pairs {
            p.defaults.insert((*k).into(), (*v).into());
        }
        p
    }

    fn project(requirements: &[(&str, &str)], overrides: &[(&str, &str)]) -> Project {
        let mut p = Project::default();
        for (k, v) in requirements {
            p.requirements.insert((*k).into(), (*v).into());
        }
        for (k, v) in overrides {
            p.overrides.insert((*k).into(), (*v).into());
        }
        p
    }

    fn solo(project: Project) -> Trusted {
        Trusted {
            project,
            source: Source::TrustedBase {
                revision: "base".into(),
            },
            authority_change: None,
        }
    }

    fn policy(team: &str, pairs: &[(&str, &[&str])]) -> TeamAnswer {
        let mut p = TeamPolicy::default();
        for (k, vs) in pairs {
            p.constraints
                .insert((*k).into(), vs.iter().map(|s| (*s).to_string()).collect());
        }
        TeamAnswer::Policy {
            team: team.into(),
            policy: p,
        }
    }

    #[test]
    fn a_personal_default_supplies_an_unconstrained_key() {
        let r = resolve(
            &personal(&[("editor", "vi")]),
            &solo(Project::default()),
            &TeamAnswer::NotEnrolled,
            &RunChoices::none(),
        );
        assert_eq!(
            r.keys["editor"],
            Answer::Resolved {
                value: "vi".into(),
                supplied_by: Layer::PersonalDefault,
                constrained_by: vec![],
            }
        );
    }

    #[test]
    fn a_project_override_beats_a_personal_default_and_says_so() {
        let r = resolve(
            &personal(&[("editor", "vi")]),
            &solo(project(&[], &[("editor", "ed")])),
            &TeamAnswer::NotEnrolled,
            &RunChoices::none(),
        );
        match &r.keys["editor"] {
            Answer::Resolved {
                value, supplied_by, ..
            } => {
                assert_eq!(value, "ed");
                assert_eq!(*supplied_by, Layer::ProjectOverride);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_run_choice_beats_both_and_is_recorded_as_the_run_choice() {
        let r = resolve(
            &personal(&[("editor", "vi")]),
            &solo(project(&[], &[("editor", "ed")])),
            &TeamAnswer::NotEnrolled,
            &RunChoices::none().choosing("editor", "acme"),
        );
        match &r.keys["editor"] {
            Answer::Resolved {
                value, supplied_by, ..
            } => {
                assert_eq!(value, "acme");
                assert_eq!(*supplied_by, Layer::RunChoice);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_run_choice_against_a_project_requirement_is_refused_not_clamped() {
        let r = resolve(
            &Personal::default(),
            &solo(project(&[("spec-spine", "=0.20.0")], &[])),
            &TeamAnswer::NotEnrolled,
            &RunChoices::none().choosing("spec-spine", "=0.19.0"),
        );
        match &r.keys["spec-spine"] {
            Answer::Refused {
                attempted,
                constrained_by,
                ..
            } => {
                assert_eq!(attempted, "=0.19.0");
                assert_eq!(*constrained_by, Layer::ProjectRequirement);
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(r.keys["spec-spine"].value().is_none());
    }

    #[test]
    fn a_value_outside_a_team_constraint_is_refused() {
        let r = resolve(
            &Personal::default(),
            &solo(project(&[], &[("model", "cheap")])),
            &policy("acme", &[("model", &["approved-a", "approved-b"])]),
            &RunChoices::none(),
        );
        match &r.keys["model"] {
            Answer::Refused {
                constrained_by,
                reason,
                ..
            } => {
                assert_eq!(*constrained_by, Layer::TeamPolicy);
                assert!(reason.contains("approved-a"));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_value_inside_a_team_constraint_records_the_constraint_too() {
        let r = resolve(
            &Personal::default(),
            &solo(project(&[], &[("model", "approved-a")])),
            &policy("acme", &[("model", &["approved-a", "approved-b"])]),
            &RunChoices::none(),
        );
        match &r.keys["model"] {
            Answer::Resolved {
                supplied_by,
                constrained_by,
                ..
            } => {
                assert_eq!(*supplied_by, Layer::ProjectOverride);
                assert_eq!(constrained_by, &vec![Layer::TeamPolicy]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_deferred_key_with_an_unreachable_team_is_unknown_and_never_defaulted() {
        let mut p = project(&[("model", DEFER_TO_TEAM)], &[]);
        p.enrollment = Enrollment::Team {
            team: "acme".into(),
        };
        let r = resolve(
            &personal(&[("model", "whatever-i-like")]),
            &solo(p),
            &TeamAnswer::Unavailable {
                team: "acme".into(),
                reason: "no route to the coordination authority".into(),
            },
            &RunChoices::none().choosing("model", "whatever-i-like"),
        );
        match &r.keys["model"] {
            Answer::Unknown { reason } => {
                assert!(reason.contains("could not be reached"));
            }
            other => panic!("expected unknown, got {other:?}"),
        }
        assert!(r.keys["model"].value().is_none(), "unknown is not success");
    }

    #[test]
    fn a_deferred_key_resolves_from_the_team_when_the_team_answers() {
        let r = resolve(
            &Personal::default(),
            &solo(project(&[("model", DEFER_TO_TEAM)], &[])),
            &policy("acme", &[("model", &["approved-a"])]),
            &RunChoices::none(),
        );
        match &r.keys["model"] {
            Answer::Resolved {
                value, supplied_by, ..
            } => {
                assert_eq!(value, "approved-a");
                assert_eq!(*supplied_by, Layer::TeamPolicy);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_key_nothing_supplies_is_unknown_rather_than_empty() {
        let r = resolve(
            &Personal::default(),
            &solo(Project::default()),
            &policy("acme", &[("model", &["a", "b"])]),
            &RunChoices::none(),
        );
        assert!(matches!(r.keys["model"], Answer::Unknown { .. }));
        assert!(r.has_findings());
    }

    #[test]
    fn the_base_revisions_declaration_wins_over_the_candidates() {
        let mut base = Manifest::new(statecraft_environment::manifest::Pins {
            product: "0".into(),
            spec_spine: "0".into(),
            adapters: Default::default(),
            producer: None,
        });
        base.project = project(&[("model", "approved-a")], &[]);
        let reader = StaticRevision(
            [(
                format!("base:{}", crate::project::DECLARATION),
                serde_json::to_string(&base).unwrap(),
            )]
            .into_iter()
            .collect(),
        );
        // The candidate tries to weaken its own requirement.
        let candidate = project(&[("model", "anything-i-like")], &[]);
        let t = trusted(Path::new("/x"), "base", &reader, &candidate);
        assert_eq!(t.project, base.project);
        let change = t.authority_change.clone().expect("reported");
        assert_eq!(change.keys, ["model"]);

        let r = resolve(
            &Personal::default(),
            &t,
            &TeamAnswer::NotEnrolled,
            &RunChoices::none(),
        );
        assert_eq!(r.keys["model"].value(), Some("approved-a"));
        assert!(r.authority_change.is_some());
        assert!(r.has_findings());
    }

    #[test]
    fn no_baseline_is_reported_rather_than_silently_trusting_the_working_tree() {
        let t = trusted(
            Path::new("/x"),
            "base",
            &StaticRevision::default(),
            &Project::default(),
        );
        assert_eq!(t.source, Source::WorkingTree);
        assert!(t.authority_change.is_some());
    }

    #[test]
    fn an_identical_candidate_is_not_an_authority_change() {
        let mut base = Manifest::new(statecraft_environment::manifest::Pins {
            product: "0".into(),
            spec_spine: "0".into(),
            adapters: Default::default(),
            producer: None,
        });
        base.project = project(&[("model", "a")], &[]);
        let reader = StaticRevision(
            [(
                format!("base:{}", crate::project::DECLARATION),
                serde_json::to_string(&base).unwrap(),
            )]
            .into_iter()
            .collect(),
        );
        let t = trusted(Path::new("/x"), "base", &reader, &base.project);
        assert!(t.authority_change.is_none());
    }
}
