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
//! 071), and this product **reads** its report rather than answering the
//! question itself: [`crate::delta`] is the reader. Declaring which paths are
//! members **here** is this product's job, and is done by path.
//!
//! Reading the second as an answer to the first would leave the
//! repository-artifact members unchecked while the verdict still read as
//! complete. So: corpus-side members come from the delta report, repository
//! artifacts are declared by path here, and the environment manifest is
//! computed here because spec-spine cannot know about it at all.
//!
//! # When there is no answer
//!
//! A corpus-side answer can still be absent: no report was supplied, the report
//! is about a different change, or it carries a class this build will not
//! place. The verdict then reads `not-recorded` and **acceptance is still
//! refused** on the candidate's own suite.
//!
//! **Refusing without the report is available; classifying without it is not.**

use crate::absence::{Absence, Recorded};
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

/// What a delta report had to say about the corpus-side members.
///
/// Two cases, and the second carries **why** in words, because the reason is
/// what reaches the record. Spec 005 section 3.3 constrains that reason: it
/// states what this product asked for and what came back, and never asserts
/// anything about what a spec-spine release carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusAnswer {
    /// The report answered.
    Answered {
        /// The authority-set members its classes witness, in this product's
        /// vocabulary.
        members: Vec<String>,
        /// Every class token the report used, verbatim. Useful detail, and not
        /// a membership answer (spec 005 section 3.3).
        classes: Vec<String>,
        /// spec-spine's own `priorPolicy.required`, recorded verbatim.
        prior_policy_required: bool,
    },
    /// There is no answer, and this is why.
    Unavailable {
        /// The reason, as it reaches the record.
        reason: String,
    },
}

/// A source of spec-spine's change classification for corpus-side members.
///
/// A trait so the way the report is obtained stays outside this module. The
/// implementation that reads a real report is [`crate::delta`].
pub trait DeltaReport {
    /// What the report says about the corpus-side members of this diff.
    fn corpus_answer(&self, changed_paths: &[String]) -> CorpusAnswer;
    /// The spec-spine version, so a verdict can name what answered.
    fn version(&self) -> String;
}

/// No report was supplied for this candidate.
///
/// Correct whenever the caller has none, and deliberately empty-handed. The
/// absence is attributed to this product: spec-spine 071 is released and the
/// `=0.20.0` pin carries it, so the report exists and this instance is the case
/// where nothing asked for it.
#[derive(Debug, Clone)]
pub struct NoDeltaReport {
    /// The installed version, named in the verdict.
    pub spec_spine_version: String,
}

impl DeltaReport for NoDeltaReport {
    fn corpus_answer(&self, _changed_paths: &[String]) -> CorpusAnswer {
        CorpusAnswer::Unavailable {
            reason: "this product does not read spec-spine's change-classification report \
                     (its spec 071) for this candidate, because none was supplied"
                .to_string(),
        }
    }
    fn version(&self) -> String {
        self.spec_spine_version.clone()
    }
}

/// A delta report backed by an explicit answer, for tests.
#[derive(Debug, Clone)]
pub struct StaticDeltaReport {
    /// What it reports.
    pub members: Vec<String>,
    /// The version.
    pub version: String,
}

impl DeltaReport for StaticDeltaReport {
    fn corpus_answer(&self, _changed_paths: &[String]) -> CorpusAnswer {
        CorpusAnswer::Answered {
            members: self.members.clone(),
            classes: vec![],
            prior_policy_required: !self.members.is_empty(),
        }
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
    /// Every class the delta report used, verbatim, or empty when there is no
    /// report. Detail for a reviewer; never read as a membership answer.
    pub corpus_classes: Vec<String>,
    /// spec-spine's own `priorPolicy.required`, recorded as it answered it.
    ///
    /// Spec 071 section 3.5: `false` means only that no structural class above
    /// `implementation` changed. It does not mean the change is safe, correct
    /// or approved, and nothing here reads it as an acceptance.
    pub prior_policy_required: Recorded<bool>,
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

    let answer = delta.corpus_answer(changed_paths);

    let (corpus, corpus_classes, prior_policy_required, absence_reason) = match answer {
        CorpusAnswer::Answered {
            members,
            classes,
            prior_policy_required,
        } => (
            CorpusVerdict::Members(members),
            classes,
            Recorded::Present(prior_policy_required),
            None,
        ),
        CorpusAnswer::Unavailable { reason } => (
            CorpusVerdict::Absent(Absence::NotRecorded),
            Vec::new(),
            Recorded::Absent(Absence::NotRecorded),
            Some(reason),
        ),
    };

    let corpus_members: Vec<String> = match &corpus {
        CorpusVerdict::Members(m) => m.clone(),
        CorpusVerdict::Absent(_) => Vec::new(),
    };
    let corpus_touched = !corpus_members.is_empty();
    let corpus_unknown = matches!(corpus, CorpusVerdict::Absent(_));

    let authority_change =
        !repository_members_touched.is_empty() || environment_manifest_touched || corpus_touched;

    let note = match absence_reason {
        // Spec 005 section 3.3: the recorded reason says what this product
        // asked for and what came back. It never claims that no release carries
        // the report, because the pin can move under a record that said so.
        Some(reason) => format!(
            "corpus-side authority classification is not-recorded: {reason}, so acceptance is \
             refused on the candidate's own suite rather than classified locally; the installed \
             spec-spine is {}",
            delta.version()
        ),
        None if authority_change => {
            let mut touched: Vec<String> = repository_members_touched
                .iter()
                .cloned()
                .chain(corpus_members.iter().cloned())
                .collect();
            if environment_manifest_touched {
                touched.push("environment-manifest".to_string());
            }
            touched.sort();
            touched.dedup();
            format!(
                "the candidate's diff touches the authority set; a human decision recorded \
                 outside the candidate is required, and the candidate's own suite does not \
                 settle it. Members touched: {}",
                touched.join(", ")
            )
        }
        None => "no authority-set member was touched".to_string(),
    };

    Verdict {
        repository_members_touched,
        environment_manifest_touched,
        corpus_members: corpus,
        corpus_classes,
        prior_policy_required,
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
        assert!(v.note.contains("spec 071"));
        assert_eq!(
            v.prior_policy_required,
            Recorded::Absent(Absence::NotRecorded)
        );
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
        assert_eq!(v.prior_policy_required, Recorded::Present(false));
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
        assert!(v.note.contains("policy"), "the note names what was touched");
    }
}
