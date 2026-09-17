//! The four dimensions, admission, the refusal codes and the policy.
//!
//! statecraft-cli spec 005 section 3.5 fixed these value sets and this
//! product's spec 007 moved them here without changing one of them: the four
//! sets are still closed, `unsigned` still belongs only to `signature`, and
//! `not-applicable` still belongs only to `subjectBinding`. What is added is
//! refusal-code members and policy fields, each defaulted and skipped at its
//! default, so a value written by statecraft-cli reads and re-writes
//! unchanged. `statecraft-acceptance::dimensions` re-exports them.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `integrity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Integrity {
    /// Intact under the named construction.
    Pass,
    /// Not intact.
    Fail,
    /// The check did not run or could not.
    Unknown,
}

/// `signature`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Signature {
    /// Verified.
    Pass,
    /// Present and did not verify.
    Fail,
    /// No signature. Only this dimension has this value.
    Unsigned,
    /// The check did not run.
    Unknown,
}

/// `issuerTrust`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IssuerTrust {
    /// Eligible under the supplied roots.
    Pass,
    /// Positively revoked or excluded.
    Fail,
    /// Not decided: absent root, unsigned, self-anchored, or not run.
    Unknown,
}

/// `subjectBinding`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubjectBinding {
    /// Names exactly the subject asked about.
    Pass,
    /// Names another.
    Fail,
    /// The question or the evidence lacks what is needed.
    Unknown,
    /// This record type has no subject. Only this dimension has this value.
    NotApplicable,
}

/// All four, bare statuses: the CLI's summary shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    /// Integrity.
    pub integrity: Integrity,
    /// Signature.
    pub signature: Signature,
    /// Issuer trust.
    pub issuer_trust: IssuerTrust,
    /// Subject binding.
    pub subject_binding: SubjectBinding,
}

impl Dimensions {
    /// Every check unperformed.
    ///
    /// An unperformed check is `unknown` in its own dimension, and **does not
    /// lower or raise another**.
    pub fn all_unknown() -> Self {
        Dimensions {
            integrity: Integrity::Unknown,
            signature: Signature::Unknown,
            issuer_trust: IssuerTrust::Unknown,
            subject_binding: SubjectBinding::Unknown,
        }
    }

    /// What statecraft-cli reports today: nothing is signed.
    ///
    /// statecraft-cli spec 005 section 3.5 says so in as many words: every
    /// `signature` is `unsigned` and every `issuerTrust` is `unknown`. **That
    /// is the honest report, not a gap in the implementation**, which is why
    /// it is a named constructor and not a TODO.
    pub fn unsigned_today(integrity: Integrity, subject_binding: SubjectBinding) -> Self {
        Dimensions {
            integrity,
            signature: Signature::Unsigned,
            issuer_trust: IssuerTrust::Unknown,
            subject_binding,
        }
    }
}

/// A status with the reason required on every value but `pass` (spec 003 B-17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimension<S> {
    /// The status.
    pub status: S,
    /// Why, for every status but `pass`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl<S> Dimension<S> {
    /// A pass.
    pub fn pass(status: S) -> Self {
        Dimension {
            status,
            reason: None,
        }
    }
    /// A non-pass with its reason.
    pub fn because(status: S, reason: impl Into<String>) -> Self {
        Dimension {
            status,
            reason: Some(reason.into()),
        }
    }
}

/// Why admission was refused. The CLI's two members plus the platform's,
/// each externally tagged in kebab-case; an unknown member is preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusalCode {
    /// A dimension the policy requires is not `pass`.
    RequiredDimensionNotPassed {
        /// Which dimension.
        dimension: String,
        /// What it read.
        value: String,
    },
    /// Evidence the policy requires was not there at all.
    IncompleteEvidence {
        /// What was missing.
        missing: Vec<String>,
    },
    /// The evidence names another subject.
    SubjectMismatch,
    /// The policy requires a trusted issuer and none was established.
    IssuerTrustRequired,
    /// Fewer approvals than the policy requires.
    ApprovalsInsufficient {
        /// Counted.
        have: u32,
        /// Required.
        need: u32,
    },
    /// The approver is the submitter and the policy forbids it.
    ApproverIsSubmitter,
    /// A required artifact type is absent.
    MissingRequiredArtifact {
        /// Which.
        evidence_type: String,
    },
    /// A dimension the policy requires as `pass` is `unknown`.
    DimensionUnknown {
        /// Which.
        dimension: String,
    },
    /// The input could not be bound unambiguously.
    AmbiguousJson,
    /// No policy is pinned.
    NoPolicy,
    /// A code this build does not know, preserved verbatim.
    Unknown(serde_json::Value),
}

impl Serialize for RefusalCode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde_json::json;
        let v = match self {
            RefusalCode::RequiredDimensionNotPassed { dimension, value } => {
                json!({ "required-dimension-not-passed": { "dimension": dimension, "value": value } })
            }
            RefusalCode::IncompleteEvidence { missing } => {
                json!({ "incomplete-evidence": { "missing": missing } })
            }
            RefusalCode::SubjectMismatch => json!("subject-mismatch"),
            RefusalCode::IssuerTrustRequired => json!("issuer-trust-required"),
            RefusalCode::ApprovalsInsufficient { have, need } => {
                json!({ "approvals-insufficient": { "have": have, "need": need } })
            }
            RefusalCode::ApproverIsSubmitter => json!("approver-is-submitter"),
            RefusalCode::MissingRequiredArtifact { evidence_type } => {
                json!({ "missing-required-artifact": { "evidence_type": evidence_type } })
            }
            RefusalCode::DimensionUnknown { dimension } => {
                json!({ "dimension-unknown": { "dimension": dimension } })
            }
            RefusalCode::AmbiguousJson => json!("ambiguous-json"),
            RefusalCode::NoPolicy => json!("no-policy"),
            RefusalCode::Unknown(v) => v.clone(),
        };
        v.serialize(s)
    }
}

impl<'de> Deserialize<'de> for RefusalCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        let s =
            |x: &serde_json::Value, k: &str| x.get(k).and_then(|y| y.as_str()).map(str::to_string);
        Ok(match &v {
            serde_json::Value::String(t) => match t.as_str() {
                "subject-mismatch" => RefusalCode::SubjectMismatch,
                "issuer-trust-required" => RefusalCode::IssuerTrustRequired,
                "approver-is-submitter" => RefusalCode::ApproverIsSubmitter,
                "ambiguous-json" => RefusalCode::AmbiguousJson,
                "no-policy" => RefusalCode::NoPolicy,
                _ => RefusalCode::Unknown(v),
            },
            serde_json::Value::Object(o) if o.len() == 1 => {
                let (k, inner) = o.iter().next().unwrap();
                match k.as_str() {
                    "required-dimension-not-passed" => {
                        match (s(inner, "dimension"), s(inner, "value")) {
                            (Some(dimension), Some(value)) => {
                                RefusalCode::RequiredDimensionNotPassed { dimension, value }
                            }
                            _ => RefusalCode::Unknown(v),
                        }
                    }
                    "incomplete-evidence" => {
                        match inner.get("missing").and_then(|m| m.as_array()) {
                            Some(a) => RefusalCode::IncompleteEvidence {
                                missing: a
                                    .iter()
                                    .filter_map(|x| x.as_str().map(str::to_string))
                                    .collect(),
                            },
                            None => RefusalCode::Unknown(v),
                        }
                    }
                    "approvals-insufficient" => match (
                        inner.get("have").and_then(|x| x.as_u64()),
                        inner.get("need").and_then(|x| x.as_u64()),
                    ) {
                        (Some(h), Some(n)) => RefusalCode::ApprovalsInsufficient {
                            have: h as u32,
                            need: n as u32,
                        },
                        _ => RefusalCode::Unknown(v),
                    },
                    "missing-required-artifact" => match s(inner, "evidence_type") {
                        Some(evidence_type) => {
                            RefusalCode::MissingRequiredArtifact { evidence_type }
                        }
                        None => RefusalCode::Unknown(v),
                    },
                    "dimension-unknown" => match s(inner, "dimension") {
                        Some(dimension) => RefusalCode::DimensionUnknown { dimension },
                        None => RefusalCode::Unknown(v),
                    },
                    _ => RefusalCode::Unknown(v),
                }
            }
            _ => RefusalCode::Unknown(v),
        })
    }
}

/// Admitted, or refused with a reason. The CLI's `admission` tag is kept;
/// `reasons` is added and omitted when empty, `reason` is always its first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "admission")]
pub enum Admission {
    /// Admitted.
    Admit,
    /// Refused.
    Refuse {
        /// The first reason.
        reason: RefusalCode,
        /// Every reason (added).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        reasons: Vec<RefusalCode>,
    },
}

impl Admission {
    /// A refusal from a non-empty list of reasons.
    pub fn refuse(reasons: Vec<RefusalCode>) -> Self {
        let reason = reasons.first().cloned().expect("a refusal has a reason");
        Admission::Refuse { reason, reasons }
    }
    /// Whether admitted.
    pub fn is_admit(&self) -> bool {
        matches!(self, Admission::Admit)
    }

    /// Every reason a refusal carries, in order.
    ///
    /// `reasons` is omitted when empty, and a refusal written by a producer
    /// that only ever names one (statecraft-cli's `admit`) omits it. An empty
    /// `reasons` therefore means "the one in `reason`", never "no reason", and
    /// this is the accessor that keeps a reader from having to know that.
    pub fn reasons(&self) -> Vec<&RefusalCode> {
        match self {
            Admission::Admit => Vec::new(),
            Admission::Refuse { reason, reasons } => {
                if reasons.is_empty() {
                    vec![reason]
                } else {
                    reasons.iter().collect()
                }
            }
        }
    }
}

/// What a policy requires of a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SignatureRequirement {
    /// Any value.
    #[default]
    Any,
    /// A signature, if present, must verify; unsigned is acceptable.
    UnsignedOk,
    /// A verified signature by a trusted issuer.
    Trusted,
}

/// The admission policy: the CLI's three booleans, extended additively with
/// spec 004 B-10's fields, every addition defaulted so a CLI value reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AdmissionPolicy {
    /// Integrity must pass.
    pub require_integrity: bool,
    /// A signature must verify.
    pub require_signature: bool,
    /// The issuer must be trusted.
    pub require_issuer_trust: bool,
    /// The policy version (added).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    /// Artifact types that must be present (added).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub require_artifacts: Vec<String>,
    /// Subject binding must pass (added).
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_subject_binding: bool,
    /// Approvals required (added).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min_approvals: u32,
    /// Approvers must be distinct principals (added).
    #[serde(default, skip_serializing_if = "is_false")]
    pub approvers_distinct: bool,
    /// The submitter may not approve (added).
    #[serde(default, skip_serializing_if = "is_false")]
    pub approver_may_not_be_submitter: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}
fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl AdmissionPolicy {
    /// Requires nothing.
    pub fn permissive() -> Self {
        Self::default()
    }
    /// Intact bytes, a verified signature, a trusted issuer.
    pub fn strict() -> Self {
        AdmissionPolicy {
            require_integrity: true,
            require_signature: true,
            require_issuer_trust: true,
            ..Self::default()
        }
    }
    /// The digest that pins this policy: BLAKE3 over its canonical value.
    pub fn digest(&self) -> crate::hash::Hash {
        let j = serde_json::to_value(self).expect("policy serializes");
        let v = crate::value::Value::from_json(&j).expect("policy values are portable");
        crate::hash::Hash::of(&crate::cbor::encode(&v))
    }
}
