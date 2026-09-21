//! spec-spine's change-classification report (its spec 071), read as a typed
//! value.
//!
//! Spec 005 section 3.3. Classifying a change under the base's rules is
//! spec-spine's job. This module is the reader for that answer and **nothing
//! else**: it parses the report, checks that the report is about the candidate
//! it is being asked about, and maps spec-spine's structural classes onto the
//! authority-set members `001` section 3.5 enumerates.
//!
//! # What this module does not do
//!
//! **It does not run spec-spine.** The report arrives as bytes the caller
//! obtained, because `001` section 3.5 rule 1 requires every authority-set
//! member to be read at the **trusted base revision**, and which binary
//! produced the report is part of that: a candidate that chose its own
//! classifier would be classifying itself. Section 3.3 fixes the invocation
//! contract the caller must satisfy; this crate still has no function with an
//! external effect.
//!
//! **It builds no classifier.** Every class token below is spec-spine's, read
//! verbatim. The mapping in [`reading_of`] says which member of *this*
//! product's authority set a class witnesses; it never says how the base would
//! classify anything.
//!
//! # Conservatism
//!
//! Four things make the corpus-side answer an absence rather than a member
//! list, and each is a case where answering would mean guessing:
//!
//! 1. a report whose schema this build does not read;
//! 2. a class token this build does not know, which could be a member;
//! 3. spec-spine's own `unknown` class, which is a path it could not place;
//! 4. a report that does not cover every path the caller is judging.
//!
//! An absence refuses acceptance on the candidate's own suite (`authority.rs`),
//! so every one of these is safe in the direction that matters.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::authority::{CorpusAnswer, DeltaReport};

/// The `DELTA_SCHEMA_VERSION` line this build reads.
///
/// Spec 071 section 3.6 starts the report's schema at `0.1.0` on its own axis.
/// On a `0.x` line the MINOR is the breaking position, so this build reads
/// `0.1.z` and refuses anything else rather than deserializing a contract it
/// was not written against.
pub const READS_DELTA_SCHEMA: &str = "0.1";

/// The verb whose envelope this is.
pub const DELTA_VERB: &str = "delta";

/// What spec-spine 071 section 3.2 requires the report to have classified
/// under, and the only value this reader accepts.
pub const CLASSIFIED_UNDER_BASE: &str = "base";

/// The eleven class tokens spec 071 section 3.3 fixes.
///
/// A token outside this list is not an error in the report; it is a contract
/// this build has not read. [`reading_of`] returns [`ClassReading::Unplaceable`]
/// for it, which makes the corpus-side answer an absence.
pub const KNOWN_CLASSES: [&str; 11] = [
    "implementation",
    "requirement",
    "verification",
    "authority",
    "lifecycle",
    "constitutional",
    "policy",
    "derived",
    "bypassed",
    "unowned",
    "unknown",
];

/// The member name a corpus-side class witnesses, in this product's vocabulary.
///
/// Both names are `001` section 3.5's own: "its policy, including the lifecycle
/// policy `003` section 3.1.1 reads from it" and "the acceptance instructions
/// the product reads". Nothing here adds a member to that set.
pub const MEMBER_POLICY: &str = "policy";

/// The base's own configuration file, which is the corpus-side `policy` member.
///
/// Every other path spec-spine classes as `policy` is there because the base
/// hashes it (`[index] extra_hashed_inputs`), and whether such a path is a
/// member **here** is declared by path, not read off the class: spec 005
/// section 3.3 case 2.
pub const SPEC_SPINE_CONFIG: &str = "spec-spine.toml";

/// The member name for an acceptance instruction that lives inside a `spec.md`.
///
/// `005` section 3.3 case 2 declares the acceptance instructions that live
/// *outside* a `spec.md` by path. The ones inside one are a spec's `verify:cli`
/// plan, which is exactly what spec-spine's `verification` class reports.
pub const MEMBER_ACCEPTANCE_INSTRUCTIONS: &str = "acceptance-instructions";

/// What one of spec-spine's classes means for this product's authority set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassReading {
    /// The class witnesses a member of `001` section 3.5's set.
    Member(&'static str),
    /// The class is real, understood, and names no member here.
    NotAMember,
    /// The class cannot be placed, so no answer is available.
    Unplaceable,
}

/// Whether a class token is one this build can place at all.
///
/// Placement is a property of the token; *which* member it witnesses can still
/// depend on the path. A token this returns `false` for makes the corpus-side
/// answer an absence.
pub fn class_is_placeable(class: &str) -> bool {
    // spec-spine's own `unknown` is a path it could not place (071 `D-4`), and
    // anything outside the eleven is a contract this build has not read.
    class != "unknown" && KNOWN_CLASSES.contains(&class)
}

/// How this product reads one of spec-spine's classes on one path.
///
/// Spec 005 section 3.3: the class names are spec-spine's and are **not** this
/// product's member names, so the mapping is written down rather than assumed.
/// It takes the path because one class is not path-blind, and reading it as
/// though it were would answer a membership question with a structural class,
/// which is the mistake section 3.3 case 2 exists to prevent.
pub fn reading_of(class: &str, path: &str) -> ClassReading {
    match class {
        // `policy` covers two different things. The base's own configuration is
        // the corpus-side policy member. Every other `policy` path is there
        // because the base hashes it, and those are repository artifacts whose
        // membership is declared here by path (case 2): `README.md` and
        // `AGENTS.md` are hashed inputs and are not members of `001` section
        // 3.5's set, so reading the class as membership would widen the
        // authority set by editing a configuration list.
        "policy" => {
            if path == SPEC_SPINE_CONFIG {
                ClassReading::Member(MEMBER_POLICY)
            } else {
                ClassReading::NotAMember
            }
        }
        // The corpus's own governance, all of it path-independent: the tier-2
        // document that governs every spec, the lifecycle policy `003` section
        // 3.1.1 reads, and the ownership and dependency edges that decide which
        // spec governs a unit.
        "constitutional" | "lifecycle" | "authority" => ClassReading::Member(MEMBER_POLICY),
        // The acceptance instructions that live inside a `spec.md`.
        "verification" => ClassReading::Member(MEMBER_ACCEPTANCE_INSTRUCTIONS),
        // Real classes that name no member here. `requirement` is deliberate:
        // a spec body edit is the ordinary shape of work in this corpus, and
        // reading it as an authority change would refuse every candidate, which
        // is not a stricter rule but an inoperative one. What a spec *requires*
        // is judged by review; what *judges* the candidate is the member set
        // above.
        "implementation" | "requirement" | "derived" | "bypassed" | "unowned" => {
            ClassReading::NotAMember
        }
        // Unplaceable: spec-spine's own `unknown`, or a token from a contract
        // this build has not read.
        _ => ClassReading::Unplaceable,
    }
}

/// The tool that produced a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
}

/// One changed path and every class that applies to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// The path, repository-relative.
    pub path: String,
    /// `added`, `modified` or `removed`, as spec-spine reports it.
    pub change: String,
    /// Every class that applies, verbatim.
    pub classes: Vec<String>,
}

/// spec-spine's own summary of what a consumer must judge under the base's
/// policy, recorded verbatim and never recomputed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorPolicy {
    /// Whether any structural class above `implementation` changed.
    ///
    /// Spec 071 section 3.5 is explicit that `false` means only that: it does
    /// not mean the change is safe, correct or approved.
    pub required: bool,
    /// Which classes, verbatim.
    #[serde(default)]
    pub classes: Vec<String>,
}

/// The report itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// `DELTA_SCHEMA_VERSION`, on its own axis.
    pub schema_version: String,
    /// What produced it.
    pub tool: Tool,
    /// Which side's rules classified. Spec 071 section 3.2: the base's.
    pub classified_under: String,
    /// The base ref's commit.
    pub base: String,
    /// The merge base of base and head.
    pub merge_base: String,
    /// The head commit.
    pub head: String,
    /// Every changed path, sorted by path.
    pub changes: Vec<Change>,
    /// Counts per class. Read as a map so a class this build does not know is
    /// visible rather than dropped by the deserializer.
    #[serde(default)]
    pub counts: BTreeMap<String, u64>,
    /// spec-spine's summary.
    pub prior_policy: PriorPolicy,
}

/// The `--json` envelope spec 034 wraps a verb's answer in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// The verb that answered.
    pub verb: String,
    /// Its exit code.
    pub exit_code: i32,
    /// Whether the verb considers its own answer well formed.
    pub ok: bool,
    /// The report.
    pub report: Report,
}

/// Why a report could not be read.
///
/// Every variant is a true statement about what this product asked for and what
/// came back. None of them claims anything about what a spec-spine release
/// carries: spec 005 section 3.3 forbids that, because the pin can move under a
/// record that made such a claim.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// The bytes are not the envelope this reader expects.
    #[error("the delta report did not parse as spec-spine's `--json` envelope: {detail}")]
    NotAnEnvelope {
        /// What the deserializer said.
        detail: String,
    },
    /// The envelope answered a different verb.
    #[error("the envelope is for the verb `{found}`, not `{DELTA_VERB}`")]
    WrongVerb {
        /// The verb the envelope named.
        found: String,
    },
    /// The verb did not produce a usable answer.
    #[error("spec-spine {version} produced no usable delta report: exit {exit_code}, ok={ok}")]
    VerbDidNotAnswer {
        /// The exit code it reported.
        exit_code: i32,
        /// What it said about its own answer.
        ok: bool,
        /// The version that answered.
        version: String,
    },
    /// The report's schema is not the one this build reads.
    #[error(
        "spec-spine {version} reported delta schema {found}; this build reads {READS_DELTA_SCHEMA}.z \
         and does not deserialize a contract it was not written against"
    )]
    SchemaNotRead {
        /// The schema the report declared.
        found: String,
        /// The version that produced it.
        version: String,
    },
    /// The report classified under something other than the base.
    #[error(
        "the report classified under `{found}`, not `{CLASSIFIED_UNDER_BASE}`; a candidate \
         classified under its own rules classifies itself"
    )]
    NotClassifiedUnderBase {
        /// What it said it classified under.
        found: String,
    },
}

/// A delta report this product read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSpineDeltaReport {
    report: Report,
}

impl SpecSpineDeltaReport {
    /// Read a report from the bytes of `spec-spine delta --json`.
    ///
    /// The caller obtained these bytes; see the module documentation for why
    /// this crate does not obtain them itself.
    pub fn from_envelope_json(bytes: &[u8]) -> Result<Self, ReadError> {
        let envelope: Envelope =
            serde_json::from_slice(bytes).map_err(|e| ReadError::NotAnEnvelope {
                detail: e.to_string(),
            })?;
        Self::from_envelope(envelope)
    }

    /// Read a report from an already deserialized envelope.
    pub fn from_envelope(envelope: Envelope) -> Result<Self, ReadError> {
        if envelope.verb != DELTA_VERB {
            return Err(ReadError::WrongVerb {
                found: envelope.verb,
            });
        }
        let version = envelope.report.tool.version.clone();
        if !envelope.ok || envelope.exit_code != 0 {
            return Err(ReadError::VerbDidNotAnswer {
                exit_code: envelope.exit_code,
                ok: envelope.ok,
                version,
            });
        }
        let report = envelope.report;
        if !schema_is_read(&report.schema_version) {
            return Err(ReadError::SchemaNotRead {
                found: report.schema_version,
                version,
            });
        }
        if report.classified_under != CLASSIFIED_UNDER_BASE {
            return Err(ReadError::NotClassifiedUnderBase {
                found: report.classified_under,
            });
        }
        Ok(Self { report })
    }

    /// The report, for a caller that records it beside the verdict.
    pub fn report(&self) -> &Report {
        &self.report
    }

    /// Every class token the report used anywhere, sorted and deduplicated.
    ///
    /// Read from the changes and from `counts`, because a class this build does
    /// not know must be visible from either place.
    pub fn classes_used(&self) -> Vec<String> {
        let mut classes: Vec<String> = self
            .report
            .changes
            .iter()
            .flat_map(|c| c.classes.iter().cloned())
            .chain(
                self.report
                    .counts
                    .iter()
                    .filter(|(_, n)| **n > 0)
                    .map(|(k, _)| k.clone()),
            )
            .collect();
        classes.sort();
        classes.dedup();
        classes
    }

    /// A class token in the report that this build cannot place, if there is
    /// one. Both spec-spine's own `unknown` and a token from a later contract.
    fn unplaceable_class(&self) -> Option<String> {
        self.classes_used()
            .into_iter()
            .find(|c| !class_is_placeable(c))
    }

    /// A path the caller is judging that this report does not cover, if there
    /// is one.
    ///
    /// A report about a different diff is not an answer about this candidate,
    /// and reading it as one is the shape of mistake this whole section exists
    /// to prevent.
    fn uncovered_path(&self, changed_paths: &[String]) -> Option<String> {
        changed_paths
            .iter()
            .find(|p| !self.report.changes.iter().any(|c| &&c.path == p))
            .cloned()
    }
}

/// Whether a report's schema version is one this build reads.
fn schema_is_read(schema_version: &str) -> bool {
    schema_version == READS_DELTA_SCHEMA
        || schema_version
            .strip_prefix(READS_DELTA_SCHEMA)
            .is_some_and(|rest| rest.starts_with('.'))
}

impl DeltaReport for SpecSpineDeltaReport {
    fn corpus_answer(&self, changed_paths: &[String]) -> CorpusAnswer {
        if let Some(class) = self.unplaceable_class() {
            return CorpusAnswer::Unavailable {
                reason: format!(
                    "this product read spec-spine's change-classification report (its spec 071) \
                     and will not place the class `{class}`, so the corpus-side answer is an \
                     absence rather than a guess"
                ),
            };
        }
        if let Some(path) = self.uncovered_path(changed_paths) {
            return CorpusAnswer::Unavailable {
                reason: format!(
                    "the change-classification report this product read does not cover `{path}`, \
                     so it is a report about a different change and is not read as an answer \
                     about this one"
                ),
            };
        }

        let mut members: Vec<String> = Vec::new();
        for change in &self.report.changes {
            for class in &change.classes {
                if let ClassReading::Member(name) = reading_of(class, &change.path) {
                    members.push(name.to_string());
                }
            }
        }
        members.sort();
        members.dedup();

        CorpusAnswer::Answered {
            members,
            classes: self.classes_used(),
            prior_policy_required: self.report.prior_policy.required,
        }
    }

    fn version(&self) -> String {
        format!("{} {}", self.report.tool.name, self.report.tool.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_with(changes: Vec<Change>, counts: BTreeMap<String, u64>) -> Envelope {
        Envelope {
            verb: DELTA_VERB.into(),
            exit_code: 0,
            ok: true,
            report: Report {
                schema_version: "0.1.0".into(),
                tool: Tool {
                    name: "spec-spine".into(),
                    version: "0.20.0".into(),
                },
                classified_under: CLASSIFIED_UNDER_BASE.into(),
                base: "b".repeat(40),
                merge_base: "b".repeat(40),
                head: "h".repeat(40),
                changes,
                counts,
                prior_policy: PriorPolicy {
                    required: false,
                    classes: vec![],
                },
            },
        }
    }

    fn change(path: &str, classes: &[&str]) -> Change {
        Change {
            path: path.into(),
            change: "modified".into(),
            classes: classes.iter().map(|c| (*c).to_string()).collect(),
        }
    }

    #[test]
    fn every_class_spec_071_fixes_has_a_reading() {
        for class in KNOWN_CLASSES {
            let reading = reading_of(class, SPEC_SPINE_CONFIG);
            if class == "unknown" {
                assert_eq!(reading, ClassReading::Unplaceable);
                assert!(!class_is_placeable(class));
            } else {
                assert_ne!(
                    reading,
                    ClassReading::Unplaceable,
                    "{class} is one of spec 071's eleven and must be placed"
                );
                assert!(class_is_placeable(class));
            }
        }
    }

    #[test]
    fn a_class_token_this_build_does_not_know_is_unplaceable() {
        assert_eq!(reading_of("quorum", "any"), ClassReading::Unplaceable);
        assert!(!class_is_placeable("quorum"));
    }

    #[test]
    fn the_policy_class_is_a_member_only_on_the_bases_own_configuration() {
        assert_eq!(
            reading_of("policy", SPEC_SPINE_CONFIG),
            ClassReading::Member(MEMBER_POLICY)
        );
        // Hashed inputs. `001` section 3.5 does not make these members, and a
        // class is not a membership answer (005 section 3.3 case 2): whether
        // one of them is a member here is declared by path.
        for hashed_input in [
            "README.md",
            "AGENTS.md",
            "docs/decisions/00-x.md",
            "Makefile",
        ] {
            assert_eq!(
                reading_of("policy", hashed_input),
                ClassReading::NotAMember,
                "for {hashed_input}"
            );
        }
    }

    #[test]
    fn an_implementation_only_change_touches_no_corpus_member() {
        let e = report_with(
            vec![change("crates/x/src/lib.rs", &["implementation"])],
            BTreeMap::from([("implementation".into(), 1)]),
        );
        let r = SpecSpineDeltaReport::from_envelope(e).unwrap();
        match r.corpus_answer(&["crates/x/src/lib.rs".into()]) {
            CorpusAnswer::Answered { members, .. } => assert!(members.is_empty()),
            other => panic!("expected an answer, got {other:?}"),
        }
    }

    #[test]
    fn a_spec_body_edit_alone_is_not_an_authority_member() {
        let e = report_with(
            vec![change("specs/005-x/spec.md", &["requirement"])],
            BTreeMap::from([("requirement".into(), 1)]),
        );
        let r = SpecSpineDeltaReport::from_envelope(e).unwrap();
        match r.corpus_answer(&["specs/005-x/spec.md".into()]) {
            CorpusAnswer::Answered { members, .. } => assert!(members.is_empty()),
            other => panic!("expected an answer, got {other:?}"),
        }
    }

    #[test]
    fn a_verification_plan_change_is_the_acceptance_instructions_member() {
        let e = report_with(
            vec![change(
                "specs/005-x/spec.md",
                &["requirement", "verification"],
            )],
            BTreeMap::from([("requirement".into(), 1), ("verification".into(), 1)]),
        );
        let r = SpecSpineDeltaReport::from_envelope(e).unwrap();
        match r.corpus_answer(&["specs/005-x/spec.md".into()]) {
            CorpusAnswer::Answered {
                members, classes, ..
            } => {
                assert_eq!(members, [MEMBER_ACCEPTANCE_INSTRUCTIONS]);
                assert!(classes.contains(&"verification".to_string()));
            }
            other => panic!("expected an answer, got {other:?}"),
        }
    }

    #[test]
    fn the_corpus_side_governance_classes_all_name_the_policy_member() {
        for (class, path) in [
            ("policy", SPEC_SPINE_CONFIG),
            ("constitutional", "standards/spec/constitution.md"),
            ("lifecycle", "specs/00x-y/spec.md"),
            ("authority", "specs/00x-y/spec.md"),
        ] {
            let e = report_with(
                vec![change(path, &[class])],
                BTreeMap::from([(class.to_string(), 1)]),
            );
            let r = SpecSpineDeltaReport::from_envelope(e).unwrap();
            match r.corpus_answer(&[path.into()]) {
                CorpusAnswer::Answered { members, .. } => {
                    assert_eq!(members, [MEMBER_POLICY], "for class {class}");
                }
                other => panic!("expected an answer for {class}, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_hashed_input_classed_policy_is_not_read_as_a_corpus_member() {
        let e = report_with(
            vec![change("README.md", &["policy"])],
            BTreeMap::from([("policy".into(), 1)]),
        );
        let r = SpecSpineDeltaReport::from_envelope(e).unwrap();
        match r.corpus_answer(&["README.md".into()]) {
            CorpusAnswer::Answered {
                members, classes, ..
            } => {
                assert!(
                    members.is_empty(),
                    "membership here is declared by path, never read off the class"
                );
                assert!(
                    classes.contains(&"policy".to_string()),
                    "the class is kept as detail"
                );
            }
            other => panic!("expected an answer, got {other:?}"),
        }
    }

    #[test]
    fn spec_spines_own_unknown_class_makes_the_answer_an_absence() {
        let e = report_with(
            vec![change("weird.md", &["unknown"])],
            BTreeMap::from([("unknown".into(), 1)]),
        );
        let r = SpecSpineDeltaReport::from_envelope(e).unwrap();
        assert!(matches!(
            r.corpus_answer(&["weird.md".into()]),
            CorpusAnswer::Unavailable { .. }
        ));
    }

    #[test]
    fn a_report_that_does_not_cover_a_judged_path_is_not_an_answer_about_it() {
        let e = report_with(
            vec![change("a.rs", &["implementation"])],
            BTreeMap::from([("implementation".into(), 1)]),
        );
        let r = SpecSpineDeltaReport::from_envelope(e).unwrap();
        match r.corpus_answer(&["a.rs".into(), "b.rs".into()]) {
            CorpusAnswer::Unavailable { reason } => assert!(reason.contains("b.rs")),
            other => panic!("expected an absence, got {other:?}"),
        }
    }

    #[test]
    fn a_schema_this_build_does_not_read_is_refused() {
        let mut e = report_with(vec![], BTreeMap::new());
        e.report.schema_version = "0.2.0".into();
        assert!(matches!(
            SpecSpineDeltaReport::from_envelope(e),
            Err(ReadError::SchemaNotRead { .. })
        ));
    }

    #[test]
    fn a_report_not_classified_under_the_base_is_refused() {
        let mut e = report_with(vec![], BTreeMap::new());
        e.report.classified_under = "head".into();
        assert!(matches!(
            SpecSpineDeltaReport::from_envelope(e),
            Err(ReadError::NotClassifiedUnderBase { .. })
        ));
    }

    #[test]
    fn a_verb_that_did_not_answer_is_refused_and_names_its_exit_code() {
        let mut e = report_with(vec![], BTreeMap::new());
        e.ok = false;
        e.exit_code = 3;
        match SpecSpineDeltaReport::from_envelope(e) {
            Err(ReadError::VerbDidNotAnswer { exit_code, .. }) => assert_eq!(exit_code, 3),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_version_reported_is_the_binary_that_classified() {
        let r = SpecSpineDeltaReport::from_envelope(report_with(vec![], BTreeMap::new())).unwrap();
        assert_eq!(r.version(), "spec-spine 0.20.0");
    }

    #[test]
    fn no_reason_this_module_produces_claims_anything_about_a_release() {
        let unplaceable = SpecSpineDeltaReport::from_envelope(report_with(
            vec![change("weird.md", &["unknown"])],
            BTreeMap::from([("unknown".into(), 1)]),
        ))
        .unwrap();
        let uncovered = SpecSpineDeltaReport::from_envelope(report_with(
            vec![change("a.rs", &["implementation"])],
            BTreeMap::from([("implementation".into(), 1)]),
        ))
        .unwrap();

        let reasons: Vec<String> = vec![
            match unplaceable.corpus_answer(&["weird.md".into()]) {
                CorpusAnswer::Unavailable { reason } => reason,
                other => panic!("{other:?}"),
            },
            match uncovered.corpus_answer(&["a.rs".into(), "b.rs".into()]) {
                CorpusAnswer::Unavailable { reason } => reason,
                other => panic!("{other:?}"),
            },
        ];

        for reason in reasons {
            for false_claim in [
                "is in no release",
                "carries no change-classification report",
                "no release carries",
            ] {
                assert!(
                    !reason.contains(false_claim),
                    "a reason must not claim anything about what a release carries: \
                     found {false_claim:?} in {reason}"
                );
            }
        }
    }
}
