//! The authority-set rule, and the classification this product must not build.
//!
//! Spec 005 section 3.3. If the candidate's diff touches the authority set, the
//! acceptance records an **authority change** and does not accept on the
//! strength of the candidate's own suite. This holds **even when the
//! candidate's suite passes, and especially then**.
//!
//! # The split that matters
//!
//! Classifying a change under the base's rules is spec-spine's job (its spec
//! 088), and this product does not read its report. Declaring which paths are
//! members **here** is this product's job, and is done by path.
//!
//! Reading the second as an answer to the first would leave the
//! repository-artifact members unchecked while the verdict still read as
//! complete. So: corpus-side members read `not-recorded` until this product
//! reads the delta report, repository artifacts are declared by path
//! here, and the environment manifest is computed here because spec-spine cannot
//! know about it at all.
//!
//! **Refusing without the report is available; classifying without it is not.**

use crate::absence::Absence;
use serde::{Deserialize, Serialize};

/// A member of the authority set that is a repository artifact.
///
/// Membership is declared here, by path. This says which paths matter in this
/// repository; it never says how the base would classify a change to them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredMember {
    /// A path prefix, repository-relative.
    pub path_prefix: String,
    /// Which member this is, in this product's vocabulary.
    pub member: String,
}

/// The repository-artifact members, declared by path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declared {
    /// One entry per declared prefix.
    pub members: Vec<DeclaredMember>,
}

impl Declared {
    /// Nothing declared.
    pub fn none() -> Self {
        Self::default()
    }

    /// Declare a prefix as a member.
    #[must_use]
    pub fn declaring(mut self, path_prefix: &str, member: &str) -> Self {
        self.members.push(DeclaredMember {
            path_prefix: path_prefix.to_string(),
            member: member.to_string(),
        });
        self
    }

    /// Which declared members a changed path touches.
    pub fn touched_by(&self, path: &str) -> Vec<&DeclaredMember> {
        self.members
            .iter()
            .filter(|m| path.starts_with(&m.path_prefix))
            .collect()
    }
}

/// Whether the installed spec-spine can classify corpus-side changes.
///
/// A trait so the day a release carries spec 088's delta report, the
/// implementation changes here and nowhere else.
pub trait DeltaReport {
    /// The corpus-side authority members a diff touched, if this spec-spine can
    /// say. `None` means it cannot, which is a recorded fact and not a zero.
    fn corpus_members_touched(&self, changed_paths: &[String]) -> Option<Vec<String>>;
    /// The spec-spine version, so a `not-recorded` verdict can name it.
    fn version(&self) -> String;
}

/// The delta report this product does not read.
///
/// Correct today and deliberately empty-handed. Spec-spine 088 landed in
/// `v0.19.0` and the `=0.20.0` pin carries it, so the report exists and nothing
/// here asks for it. Integrating it is its own change (spec 005 section 5,
/// 2026-09-17); until then the answer is a named absence and never a guess.
#[derive(Debug, Clone)]
pub struct NoDeltaReport {
    /// The pinned version, named in the verdict.
    pub spec_spine_version: String,
}

impl DeltaReport for NoDeltaReport {
    fn corpus_members_touched(&self, _changed_paths: &[String]) -> Option<Vec<String>> {
        None
    }
    fn version(&self) -> String {
        self.spec_spine_version.clone()
    }
}

/// A delta report backed by an explicit answer, for tests and for the day a
/// release carries one.
#[derive(Debug, Clone)]
pub struct StaticDeltaReport {
    /// What it reports.
    pub members: Vec<String>,
    /// The version.
    pub version: String,
}

impl DeltaReport for StaticDeltaReport {
    fn corpus_members_touched(&self, _changed_paths: &[String]) -> Option<Vec<String>> {
        Some(self.members.clone())
    }
    fn version(&self) -> String {
        self.version.clone()
    }
}

/// What the authority-set evaluation concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// Repository-artifact members touched, computed here by path.
    pub repository_members_touched: Vec<String>,
    /// The environment manifest, computed here because nothing else can.
    pub environment_manifest_touched: bool,
    /// Corpus-side members, or the absence that stands in for them.
    pub corpus_members: CorpusVerdict,
    /// Whether this is an authority change.
    pub authority_change: bool,
    /// Whether acceptance may rest on the candidate's own suite.
    ///
    /// Always false when this is an authority change, and also false when the
    /// corpus-side answer is `not-recorded`: refusing without the report is
    /// available, and it is what happens.
    pub may_accept_on_own_suite: bool,
    /// What a reader needs to know, in words.
    pub note: String,
}

/// The corpus-side half of the verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum CorpusVerdict {
    /// The delta report answered.
    Members(Vec<String>),
    /// It could not, and this names the absence.
    Absent(Absence),
}

/// Where the environment manifest lives in a target.
pub const ENVIRONMENT_MANIFEST: &str = ".statecraft/environment.json";

/// Evaluate the authority-set rule over a candidate's changed paths.
pub fn evaluate(changed_paths: &[String], declared: &Declared, delta: &dyn DeltaReport) -> Verdict {
    let mut repository_members_touched: Vec<String> = changed_paths
        .iter()
        .flat_map(|p| declared.touched_by(p))
        .map(|m| m.member.clone())
        .collect();
    repository_members_touched.sort();
    repository_members_touched.dedup();

    let environment_manifest_touched = changed_paths.iter().any(|p| p == ENVIRONMENT_MANIFEST);

    let corpus = match delta.corpus_members_touched(changed_paths) {
        Some(members) => CorpusVerdict::Members(members),
        None => CorpusVerdict::Absent(Absence::NotRecorded),
    };

    let corpus_touched = match &corpus {
        CorpusVerdict::Members(m) => !m.is_empty(),
        CorpusVerdict::Absent(_) => false,
    };
    let corpus_unknown = matches!(corpus, CorpusVerdict::Absent(_));

    let authority_change =
        !repository_members_touched.is_empty() || environment_manifest_touched || corpus_touched;

    let note = if corpus_unknown {
        // Spec 005 section 3.3: the absence is attributed to THIS product, not to
        // spec-spine. The older wording said the installed spec-spine carried no
        // such report, which the `=0.20.0` pin falsified; this wording is true
        // whichever version is installed, so an old record and a new one make
        // the same claim about the same thing.
        format!(
            "corpus-side authority classification is not-recorded: this product does not read \
             spec-spine's change-classification report (its spec 088), so acceptance is refused \
             on the candidate's own suite rather than classified locally; the installed \
             spec-spine is {}",
            delta.version()
        )
    } else if authority_change {
        "the candidate's diff touches the authority set; a human decision recorded outside \
         the candidate is required, and the candidate's own suite does not settle it"
            .to_string()
    } else {
        "no authority-set member was touched".to_string()
    };

    Verdict {
        repository_members_touched,
        environment_manifest_touched,
        corpus_members: corpus,
        authority_change,
        may_accept_on_own_suite: !authority_change && !corpus_unknown,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared() -> Declared {
        Declared::none()
            .declaring("Makefile", "check-suite")
            .declaring(".github/workflows/", "check-suite")
            .declaring("scripts/", "verifier")
            .declaring(".claude/hooks/", "hooks")
    }

    fn no_report() -> NoDeltaReport {
        NoDeltaReport {
            spec_spine_version: "spec-spine 0.18.0".into(),
        }
    }

    #[test]
    fn with_no_delta_report_the_corpus_verdict_is_not_recorded_and_names_the_version() {
        let v = evaluate(&["src/lib.rs".to_string()], &declared(), &no_report());
        assert_eq!(
            v.corpus_members,
            CorpusVerdict::Absent(Absence::NotRecorded)
        );
        assert!(v.note.contains("0.18.0"));
        assert!(v.note.contains("spec 088"));
    }

    #[test]
    fn without_the_report_acceptance_never_rests_on_the_candidates_own_suite() {
        let v = evaluate(&["src/lib.rs".to_string()], &declared(), &no_report());
        assert!(
            !v.may_accept_on_own_suite,
            "refusing without the report is available; classifying without it is not"
        );
    }

    #[test]
    fn a_repository_artifact_member_is_detected_here_by_path() {
        let v = evaluate(&["Makefile".to_string()], &declared(), &no_report());
        assert_eq!(v.repository_members_touched, ["check-suite"]);
        assert!(v.authority_change);
    }

    #[test]
    fn the_environment_manifest_is_computed_here_because_nothing_else_can() {
        let v = evaluate(
            &[ENVIRONMENT_MANIFEST.to_string()],
            &declared(),
            &no_report(),
        );
        assert!(v.environment_manifest_touched);
        assert!(v.authority_change);
    }

    #[test]
    fn with_a_report_and_nothing_touched_acceptance_may_rest_on_the_suite() {
        let delta = StaticDeltaReport {
            members: vec![],
            version: "spec-spine 0.99.0".into(),
        };
        let v = evaluate(&["README.md".to_string()], &declared(), &delta);
        assert!(!v.authority_change);
        assert!(v.may_accept_on_own_suite);
    }

    #[test]
    fn a_corpus_member_reported_by_the_delta_report_is_an_authority_change() {
        let delta = StaticDeltaReport {
            members: vec!["policy".into()],
            version: "spec-spine 0.99.0".into(),
        };
        let v = evaluate(&["spec-spine.toml".to_string()], &declared(), &delta);
        assert!(v.authority_change);
        assert!(!v.may_accept_on_own_suite);
    }
}
