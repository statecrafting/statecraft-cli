//! Admission is policy (spec 004 B-10 to B-12): a pure function over verdicts
//! and decisions, returning the CLI's `Admission` with the reasons named, and
//! the claim of the `statecraft/policy-eval/v1` attestation that records it.

use std::collections::BTreeSet;

use crate::attestation::{AttestationId, Principal};
use crate::dimensions::{
    Admission, AdmissionPolicy, Integrity, IssuerTrust, RefusalCode, Signature,
    SignatureRequirement, SubjectBinding,
};
use crate::hash::Hash;
use crate::value::Value;
use crate::verdict::{Evidence, EvidenceVerdict};

/// A decision the policy counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The attestation.
    pub id: AttestationId,
    /// Approval or change request.
    pub approves: bool,
    /// Who.
    pub reviewer: Principal,
}

/// The inputs.
#[derive(Debug, Clone)]
pub struct AdmissionInput<'a> {
    /// Every artifact verdict for the revision, with its attestation id.
    pub verdicts: &'a [(AttestationId, EvidenceVerdict)],
    /// Every decision recorded for the revision.
    pub decisions: &'a [Decision],
    /// Who submitted.
    pub submitter: &'a Principal,
}

/// What a policy requires of a signature, from the CLI booleans plus the extension.
fn signature_requirement(p: &AdmissionPolicy) -> SignatureRequirement {
    if p.require_issuer_trust {
        SignatureRequirement::Trusted
    } else if p.require_signature {
        SignatureRequirement::UnsignedOk
    } else {
        SignatureRequirement::Any
    }
}

/// Evaluate. Deterministic; reads nothing but its arguments.
pub fn evaluate(policy: &AdmissionPolicy, input: &AdmissionInput<'_>) -> Admission {
    let mut reasons = Vec::new();

    for required in &policy.require_artifacts {
        let present = input.verdicts.iter().any(|(_, v)| matches!(&v.evidence, Evidence::Artifact { reference } if reference.evidence_type == *required));
        if !present {
            reasons.push(RefusalCode::MissingRequiredArtifact {
                evidence_type: required.clone(),
            });
        }
    }

    for (_, v) in input.verdicts {
        if policy.require_integrity {
            match v.integrity.status {
                Integrity::Pass => {}
                Integrity::Fail => reasons.push(RefusalCode::RequiredDimensionNotPassed {
                    dimension: "integrity".into(),
                    value: "fail".into(),
                }),
                Integrity::Unknown => reasons.push(RefusalCode::DimensionUnknown {
                    dimension: "integrity".into(),
                }),
            }
        }
        match signature_requirement(policy) {
            SignatureRequirement::Any => {}
            SignatureRequirement::UnsignedOk => match v.signature.status {
                Signature::Pass | Signature::Unsigned => {}
                Signature::Fail => reasons.push(RefusalCode::RequiredDimensionNotPassed {
                    dimension: "signature".into(),
                    value: "fail".into(),
                }),
                Signature::Unknown => reasons.push(RefusalCode::DimensionUnknown {
                    dimension: "signature".into(),
                }),
            },
            SignatureRequirement::Trusted => {
                if v.signature.status != Signature::Pass {
                    let value = match v.signature.status {
                        Signature::Unsigned => "unsigned",
                        Signature::Fail => "fail",
                        _ => "unknown",
                    };
                    reasons.push(RefusalCode::RequiredDimensionNotPassed {
                        dimension: "signature".into(),
                        value: value.into(),
                    });
                }
                match v.issuer_trust.status {
                    IssuerTrust::Pass => {}
                    IssuerTrust::Fail => reasons.push(RefusalCode::RequiredDimensionNotPassed {
                        dimension: "issuerTrust".into(),
                        value: "fail".into(),
                    }),
                    IssuerTrust::Unknown => reasons.push(RefusalCode::IssuerTrustRequired),
                }
            }
        }
        if policy.require_subject_binding {
            match v.subject_binding.status {
                SubjectBinding::Pass | SubjectBinding::NotApplicable => {}
                SubjectBinding::Fail => reasons.push(RefusalCode::SubjectMismatch),
                SubjectBinding::Unknown => reasons.push(RefusalCode::DimensionUnknown {
                    dimension: "subjectBinding".into(),
                }),
            }
        }
    }

    let mut approvers: Vec<&Principal> = input
        .decisions
        .iter()
        .filter(|d| d.approves)
        .map(|d| &d.reviewer)
        .collect();
    if policy.approver_may_not_be_submitter && approvers.contains(&input.submitter) {
        reasons.push(RefusalCode::ApproverIsSubmitter);
        approvers.retain(|p| *p != input.submitter);
    }
    let count = if policy.approvers_distinct {
        approvers.iter().collect::<BTreeSet<_>>().len() as u32
    } else {
        approvers.len() as u32
    };
    if count < policy.min_approvals {
        reasons.push(RefusalCode::ApprovalsInsufficient {
            have: count,
            need: policy.min_approvals,
        });
    }

    if reasons.is_empty() {
        Admission::Admit
    } else {
        Admission::refuse(reasons)
    }
}

/// The claim of the `statecraft/policy-eval/v1` attestation recording an evaluation.
pub fn policy_eval_claim(
    policy_digest: Hash,
    inputs: &[AttestationId],
    admission: &Admission,
) -> Value {
    let (decision, reasons): (&str, Vec<Value>) = match admission {
        Admission::Admit => ("admit", Vec::new()),
        Admission::Refuse { reasons, reason } => {
            let list: Vec<&RefusalCode> = if reasons.is_empty() {
                vec![reason]
            } else {
                reasons.iter().collect()
            };
            (
                "refuse",
                list.iter()
                    .map(|r| {
                        Value::from_json(&serde_json::to_value(r).expect("reason serializes"))
                            .expect("portable")
                    })
                    .collect(),
            )
        }
    };
    Value::map()
        .with("policy", Value::text(policy_digest.to_hex()))
        .unwrap()
        .with(
            "inputs",
            Value::Array(inputs.iter().map(|i| Value::text(i.0.to_hex())).collect()),
        )
        .unwrap()
        .with("decision", Value::text(decision))
        .unwrap()
        .with("reasons", Value::Array(reasons))
        .unwrap()
}
