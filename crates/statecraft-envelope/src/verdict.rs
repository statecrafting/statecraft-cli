//! The verdict (spec 003 B-17 to B-20; hqgit 064 B-2, B-3, B-8 adopted).
//!
//! Four dimensions, always present, each with its reason on every status but
//! `pass`; coverage of the stages that ran; the stop reason; the verifier and
//! the root set. No decision.

use serde::{Deserialize, Serialize};

use crate::attestation::{Attestation, ClaimVerdict, PredicateRegistry};
use crate::dimensions::{Dimension, Dimensions, Integrity, IssuerTrust, Signature, SubjectBinding};
use crate::fact::Resolved;
use crate::hash::Hash;
use crate::reference::{GitSubject, Reference};
use crate::roots::{AnchorOrigin, KeyValidity, RootSet};
use crate::value::Value;

/// The stages, in the order they run. Frozen.
pub const STAGES: [&str; 4] = ["structure", "signature", "issuer", "claim"];

/// What happened at a stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "outcome")]
pub enum StageOutcome {
    /// Ran and passed.
    Passed,
    /// Ran and failed.
    Failed {
        /// Which failure.
        failure: String,
    },
    /// Could not run for lack of an anchor; distinct from a refusal.
    NotAnchored {
        /// What was missing.
        gap: String,
    },
    /// Did not run because an earlier stage stopped the pipeline.
    NotEvaluated,
}

/// The verifier's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierId {
    /// The crate.
    pub name: String,
    /// Its version.
    pub version: String,
}

impl VerifierId {
    /// This crate.
    pub fn this() -> Self {
        VerifierId {
            name: crate::VERIFIER_NAME.into(),
            version: crate::VERIFIER_VERSION.into(),
        }
    }
}

/// What the verdict is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Evidence {
    /// A foreign artifact, by its reference.
    Artifact {
        /// The reference.
        reference: Box<Reference>,
    },
    /// A native attestation, by id.
    Attestation {
        /// The id.
        id: Hash,
        /// Its predicate.
        predicate: String,
    },
}

/// The verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceVerdict {
    /// Version of this shape.
    pub verdict_version: u32,
    /// What was judged.
    pub evidence: Evidence,
    /// Who judged.
    pub verifier: VerifierId,
    /// The root set digest and origin.
    pub verified_under: VerifiedUnder,
    /// Integrity.
    pub integrity: Dimension<Integrity>,
    /// Signature.
    pub signature: Dimension<Signature>,
    /// Issuer trust.
    pub issuer_trust: Dimension<IssuerTrust>,
    /// Subject binding.
    pub subject_binding: Dimension<SubjectBinding>,
    /// Stage coverage, in `STAGES` order.
    pub coverage: Vec<(String, StageOutcome)>,
    /// Where the pipeline stopped, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<String>,
}

/// The root set a verdict was produced under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedUnder {
    /// Its digest.
    pub root_set: Hash,
    /// Its origin.
    pub origin: AnchorOrigin,
}

impl EvidenceVerdict {
    /// The bare statuses, the CLI's summary shape.
    pub fn dimensions(&self) -> Dimensions {
        Dimensions {
            integrity: self.integrity.status,
            signature: self.signature.status,
            issuer_trust: self.issuer_trust.status,
            subject_binding: self.subject_binding.status,
        }
    }

    /// The claim value for a `statecraft/evidence-verdict/v1` attestation.
    pub fn to_claim(&self) -> Value {
        Value::from_json(&serde_json::to_value(self).expect("verdict serializes"))
            .expect("verdict values are portable")
    }

    /// Refuse a document that folds the four into one word (spec 003 B-17).
    pub fn validate_shape(j: &serde_json::Value) -> Result<(), crate::Error> {
        let o = j
            .as_object()
            .ok_or_else(|| crate::Error::Validation("verdict is not an object".into()))?;
        for k in ["ok", "valid", "verified", "trusted"] {
            if o.contains_key(k) {
                return Err(crate::Error::Validation(format!(
                    "verdict carries folded field {k:?}"
                )));
            }
        }
        for k in ["integrity", "signature", "issuer_trust", "subject_binding"] {
            if !o.contains_key(k) {
                return Err(crate::Error::Validation(format!(
                    "verdict lacks dimension {k:?}"
                )));
            }
        }
        Ok(())
    }
}

/// The subject question for a foreign artifact: the registered revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectQuestion {
    /// The registered subject.
    pub subject: GitSubject,
}

/// Judge a foreign artifact (spec 003 B-19): integrity under the named
/// construction, `unsigned`, issuer trust `unknown`, subject binding computed
/// by the platform from the reference's subject against the question.
pub fn verify_artifact(
    reference: &Reference,
    bytes: Option<&[u8]>,
    question: Option<&SubjectQuestion>,
    roots: &RootSet,
) -> EvidenceVerdict {
    let integrity = match bytes {
        None => Dimension::because(Integrity::Unknown, "bytes unavailable"),
        Some(_) if !reference.checkable_here() => Dimension::because(
            Integrity::Unknown,
            format!(
                "construction {} not supported by this verifier",
                reference.construction.name()
            ),
        ),
        Some(b) => {
            if reference.matches(b) {
                Dimension::pass(Integrity::Pass)
            } else {
                Dimension::because(
                    Integrity::Fail,
                    "bytes do not recompute under the named construction",
                )
            }
        }
    };
    let signature = Dimension::because(
        Signature::Unsigned,
        "the artifact format carries no signature",
    );
    let issuer_trust = Dimension::because(IssuerTrust::Unknown, "unsigned");
    let subject_binding = match (question, &reference.subject) {
        (None, _) => Dimension::because(SubjectBinding::Unknown, "no subject question"),
        (Some(_), None) => {
            Dimension::because(SubjectBinding::Unknown, "the artifact names no subject")
        }
        (Some(q), Some(s)) => bind_subject(&q.subject, s),
    };
    EvidenceVerdict {
        verdict_version: 1,
        evidence: Evidence::Artifact {
            reference: Box::new(reference.clone()),
        },
        verifier: VerifierId::this(),
        verified_under: VerifiedUnder {
            root_set: roots.digest(),
            origin: roots.origin,
        },
        integrity,
        signature,
        issuer_trust,
        subject_binding,
        coverage: vec![
            ("structure".into(), StageOutcome::Passed),
            ("signature".into(), StageOutcome::NotEvaluated),
            ("issuer".into(), StageOutcome::NotEvaluated),
            ("claim".into(), StageOutcome::NotEvaluated),
        ],
        stop: None,
    }
}

fn bind_subject(question: &GitSubject, claimed: &GitSubject) -> Dimension<SubjectBinding> {
    if question.repository != claimed.repository {
        return Dimension::because(SubjectBinding::Fail, "repository-mismatch");
    }
    if question.commit.oid != claimed.commit.oid {
        return Dimension::because(SubjectBinding::Fail, "commit-mismatch");
    }
    if question.commit.format != claimed.commit.format {
        return Dimension::because(SubjectBinding::Fail, "commit-format-mismatch");
    }
    match (&question.tree, &claimed.tree) {
        (Some(qt), Some(ct)) => {
            if qt.oid != ct.oid || qt.format != ct.format {
                return Dimension::because(SubjectBinding::Fail, "tree-mismatch");
            }
        }
        (Some(_), None) => {
            return Dimension::because(SubjectBinding::Unknown, "the artifact names no tree");
        }
        (None, _) => {}
    }
    Dimension::pass(SubjectBinding::Pass)
}

/// Judge a native attestation (hqgit 064 B-2, B-8; spec 003 B-18).
///
/// `claim` is what the object store answered for the claim commitment;
/// `question` is the subject asked about; `roots` supplies keys and
/// eligibility; `ledger_keys` is a lookup of keys known only from the DAG
/// under judgement, which can check a signature and never establish trust.
pub fn verify_attestation(
    att: &Attestation,
    claim: &Resolved,
    question: Option<&Hash>,
    roots: &RootSet,
    ledger_keys: &dyn Fn(&crate::hash::KeyId) -> Option<crate::sign::PublicKey>,
    registry: &PredicateRegistry,
) -> EvidenceVerdict {
    let mut coverage: Vec<(String, StageOutcome)> = STAGES
        .iter()
        .map(|s| (s.to_string(), StageOutcome::NotEvaluated))
        .collect();
    let mut stop: Option<String> = None;
    let evidence = Evidence::Attestation {
        id: att.id().0,
        predicate: att.body.predicate.0.clone(),
    };
    let under = VerifiedUnder {
        root_set: roots.digest(),
        origin: roots.origin,
    };

    // Stage 1: structure. The decoded type already enforces shape; the
    // predicate grammar was checked at construction.
    coverage[0].1 = StageOutcome::Passed;

    // Stage 2: signature.
    let signature: Dimension<Signature>;
    let mut self_anchored = false;
    let key = roots.public_key(&att.issuer_key).or_else(|| {
        self_anchored = true;
        ledger_keys(&att.issuer_key)
    });
    match key {
        None => {
            coverage[1].1 = StageOutcome::NotAnchored { gap: "key".into() };
            signature = Dimension::because(Signature::Unknown, "key-not-anchored");
            stop = Some("signature".into());
        }
        Some(k) => match att.verify_signature(&k) {
            Ok(()) => {
                coverage[1].1 = StageOutcome::Passed;
                signature = Dimension::pass(Signature::Pass);
            }
            Err(_) => {
                coverage[1].1 = StageOutcome::Failed {
                    failure: "BadSignature".into(),
                };
                signature = Dimension::because(Signature::Fail, "signature does not verify");
                stop = Some("signature".into());
            }
        },
    }

    // Stage 3: issuer.
    let issuer_trust: Dimension<IssuerTrust> = if stop.is_none() {
        match roots.key_valid_at(&att.issuer_key, att.body.at) {
            KeyValidity::Valid => {
                coverage[2].1 = StageOutcome::Passed;
                match roots.origin {
                    AnchorOrigin::Pinned => Dimension::pass(IssuerTrust::Pass),
                    AnchorOrigin::EvidenceLedger => {
                        Dimension::because(IssuerTrust::Unknown, "self-anchored")
                    }
                }
            }
            KeyValidity::Refused(why) => {
                coverage[2].1 = StageOutcome::Failed {
                    failure: format!("KeyNotValidAt:{why}"),
                };
                stop = Some("issuer".into());
                Dimension::because(IssuerTrust::Fail, why)
            }
            KeyValidity::Unknown => {
                coverage[2].1 = StageOutcome::NotAnchored { gap: "key".into() };
                stop = Some("issuer".into());
                Dimension::because(
                    IssuerTrust::Unknown,
                    if self_anchored {
                        "self-anchored"
                    } else {
                        "key-not-in-root-set"
                    },
                )
            }
        }
    } else {
        Dimension::because(
            IssuerTrust::Unknown,
            format!("not evaluated after {}", stop.as_deref().unwrap_or("")),
        )
    };

    // Stage 4: claim.
    let integrity: Dimension<Integrity> = if stop.is_none() {
        match claim {
            Resolved::Erased(_) => {
                coverage[3].1 = StageOutcome::Failed {
                    failure: "Erased".into(),
                };
                stop = Some("claim".into());
                Dimension::because(Integrity::Unknown, "erased")
            }
            Resolved::Missing => {
                coverage[3].1 = StageOutcome::Failed {
                    failure: "Unavailable".into(),
                };
                stop = Some("claim".into());
                Dimension::because(Integrity::Unknown, "claim unavailable")
            }
            Resolved::Present(bytes) => match crate::cbor::decode(bytes) {
                Err(e) => {
                    coverage[3].1 = StageOutcome::Failed {
                        failure: "BadClaim".into(),
                    };
                    stop = Some("claim".into());
                    Dimension::because(Integrity::Fail, format!("claim does not decode: {e}"))
                }
                Ok(v) => match registry.verify_claim(&att.body.predicate, &v) {
                    ClaimVerdict::Valid => {
                        coverage[3].1 = StageOutcome::Passed;
                        Dimension::pass(Integrity::Pass)
                    }
                    ClaimVerdict::Invalid(why) => {
                        coverage[3].1 = StageOutcome::Failed {
                            failure: "BadClaim".into(),
                        };
                        stop = Some("claim".into());
                        Dimension::because(Integrity::Fail, why)
                    }
                    ClaimVerdict::Unregistered => {
                        coverage[3].1 = StageOutcome::Passed;
                        Dimension::because(
                            Integrity::Unknown,
                            "unregistered predicate; claim not checked",
                        )
                    }
                },
            },
        }
    } else {
        Dimension::because(
            Integrity::Unknown,
            format!("not evaluated after {}", stop.as_deref().unwrap_or("")),
        )
    };

    // Subject binding is not a stage and never moves coverage.
    let subject_binding = match question {
        None => Dimension::because(SubjectBinding::Unknown, "no subject question"),
        Some(h) => {
            if att.body.subject == *h {
                Dimension::pass(SubjectBinding::Pass)
            } else {
                Dimension::because(SubjectBinding::Fail, "subject-mismatch")
            }
        }
    };

    EvidenceVerdict {
        verdict_version: 1,
        evidence,
        verifier: VerifierId::this(),
        verified_under: under,
        integrity,
        signature,
        issuer_trust,
        subject_binding,
        coverage,
        stop,
    }
}
