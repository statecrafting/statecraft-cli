//! The admission evaluator as it stood **before** spec 037 moved it onto
//! action-gate's closed evaluation mode, kept as the oracle the differential
//! test compares against.
//!
//! The body of [`evaluate`] is `src/admission.rs` at `8ba0a8e`, copied
//! character for character; only the `use` lines changed, from `crate::` to
//! `statecraft_envelope::`, and the shared input types are imported rather than
//! redeclared. **It is never edited to make a test pass.** A disagreement
//! between this function and `statecraft_envelope::admission::evaluate` is a
//! behaviour change to be decided under spec 005, not a transcription to fix.

#![allow(dead_code)]

use std::collections::BTreeSet;

use statecraft_envelope::admission::AdmissionInput;
use statecraft_envelope::attestation::Principal;
use statecraft_envelope::dimensions::{
    Admission, AdmissionPolicy, Integrity, IssuerTrust, RefusalCode, Signature,
    SignatureRequirement, SubjectBinding,
};
use statecraft_envelope::verdict::Evidence;

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
