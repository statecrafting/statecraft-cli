//! The four evidence dimensions, and admission kept apart from all of them.
//!
//! Spec 005 section 3.5. Each dimension is reported separately, from its own
//! check, with a closed value set. The rules below are **part of the
//! vocabulary, not commentary on it**, which is why each is a type rather than
//! a convention: `unsigned` is not constructible outside `signature`, and
//! `not-applicable` is not constructible outside `subjectBinding`.
//!
//! # Where these types live now
//!
//! The four enums, [`Dimensions`], [`Admission`], [`RefusalCode`] and
//! [`AdmissionPolicy`] are defined in `statecraft-envelope` and re-exported
//! here (spec 007). They are the bytes two products exchange, and one owner is
//! the only way the two agree by construction rather than by inspection. The
//! serialized forms are unchanged: the four value sets are still closed and
//! still refuse a fifth member, and the envelope's additions are members and
//! optional fields that a statecraft-cli value never carries.
//!
//! What stays here is this product's **judgment** over those types: [`admit`]
//! decides admission from the four summary statuses under a policy, and
//! [`issuer_trust_may_pass`] is section 3.5's gate. The envelope's own
//! evaluator answers a different question over a different input (per-artifact
//! verdicts with reasons and approvals), and neither is the other's fallback.

pub use statecraft_envelope::dimensions::{
    Admission, AdmissionPolicy, Dimension, Dimensions, Integrity, IssuerTrust, RefusalCode,
    Signature, SubjectBinding,
};

/// Decide admission from the dimensions and a policy.
///
/// Incomplete required evidence refuses admission and **names what was
/// missing**, rather than refusing with a bare verdict the operator has to
/// reverse-engineer.
///
/// A refusal from here names exactly one reason and leaves `reasons` empty,
/// which is what keeps the serialized form identical to the one this product
/// wrote before the envelope existed. [`Admission::reasons`] reads it back as
/// the one-element list it is.
pub fn admit(d: &Dimensions, policy: &AdmissionPolicy) -> Admission {
    let mut missing = Vec::new();
    let refuse = |reason: RefusalCode| Admission::Refuse {
        reason,
        reasons: Vec::new(),
    };

    if policy.require_integrity && d.integrity != Integrity::Pass {
        if d.integrity == Integrity::Unknown {
            missing.push("integrity".to_string());
        } else {
            return refuse(RefusalCode::RequiredDimensionNotPassed {
                dimension: "integrity".into(),
                value: "fail".into(),
            });
        }
    }
    if policy.require_signature && d.signature != Signature::Pass {
        let value = match d.signature {
            Signature::Unsigned => "unsigned",
            Signature::Fail => "fail",
            _ => {
                missing.push("signature".to_string());
                ""
            }
        };
        if !value.is_empty() {
            return refuse(RefusalCode::RequiredDimensionNotPassed {
                dimension: "signature".into(),
                value: value.into(),
            });
        }
    }
    if policy.require_issuer_trust && d.issuer_trust != IssuerTrust::Pass {
        if d.issuer_trust == IssuerTrust::Unknown {
            missing.push("issuerTrust".to_string());
        } else {
            return refuse(RefusalCode::RequiredDimensionNotPassed {
                dimension: "issuerTrust".into(),
                value: "fail".into(),
            });
        }
    }

    if missing.is_empty() {
        Admission::Admit
    } else {
        refuse(RefusalCode::IncompleteEvidence { missing })
    }
}

/// Whether `issuerTrust` may pass, given the signature.
///
/// Section 3.5: `issuerTrust` can pass **only after a passing `signature`
/// against an eligible root**. A passing signature alone never establishes
/// issuer trust, so this is the gate and not the answer.
pub fn issuer_trust_may_pass(signature: Signature) -> bool {
    signature == Signature::Pass
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unperformed_check_is_unknown_in_its_own_dimension_only() {
        let d = Dimensions::all_unknown();
        assert_eq!(d.integrity, Integrity::Unknown);
        assert_eq!(d.signature, Signature::Unknown);
        assert_eq!(d.issuer_trust, IssuerTrust::Unknown);
        assert_eq!(d.subject_binding, SubjectBinding::Unknown);
    }

    #[test]
    fn what_this_product_reports_today_is_unsigned_and_unknown_issuer_trust() {
        let d = Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::NotApplicable);
        assert_eq!(d.signature, Signature::Unsigned);
        assert_eq!(d.issuer_trust, IssuerTrust::Unknown);
    }

    #[test]
    fn issuer_trust_cannot_pass_without_a_passing_signature() {
        assert!(!issuer_trust_may_pass(Signature::Unsigned));
        assert!(!issuer_trust_may_pass(Signature::Unknown));
        assert!(!issuer_trust_may_pass(Signature::Fail));
        assert!(issuer_trust_may_pass(Signature::Pass));
    }

    #[test]
    fn the_same_intact_unsigned_evidence_is_admitted_or_refused_by_the_policy() {
        let d = Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::NotApplicable);
        assert_eq!(admit(&d, &AdmissionPolicy::permissive()), Admission::Admit);
        match admit(&d, &AdmissionPolicy::strict()) {
            Admission::Refuse {
                reason: RefusalCode::RequiredDimensionNotPassed { dimension, value },
                ..
            } => {
                assert_eq!(dimension, "signature");
                assert_eq!(value, "unsigned");
            }
            other => panic!("expected a named refusal, got {other:?}"),
        }
    }

    #[test]
    fn incomplete_evidence_refuses_and_names_what_was_missing() {
        let d = Dimensions::all_unknown();
        match admit(&d, &AdmissionPolicy::strict()) {
            Admission::Refuse {
                reason: RefusalCode::IncompleteEvidence { missing },
                ..
            } => {
                assert!(missing.contains(&"integrity".to_string()));
                assert!(missing.contains(&"signature".to_string()));
                assert!(missing.contains(&"issuerTrust".to_string()));
            }
            other => panic!("expected incomplete evidence, got {other:?}"),
        }
    }

    #[test]
    fn a_byte_level_mutation_fails_integrity_and_refuses_under_any_policy_requiring_it() {
        let d = Dimensions {
            integrity: Integrity::Fail,
            ..Dimensions::all_unknown()
        };
        assert!(matches!(
            admit(
                &d,
                &AdmissionPolicy {
                    require_integrity: true,
                    ..AdmissionPolicy::permissive()
                }
            ),
            Admission::Refuse { .. }
        ));
    }

    #[test]
    fn a_refusal_from_this_product_names_one_reason_and_writes_no_reasons_array() {
        let a = admit(
            &Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::NotApplicable),
            &AdmissionPolicy::strict(),
        );
        let json = serde_json::to_string(&a).unwrap();
        assert!(!json.contains("reasons"), "{json}");
        assert_eq!(a.reasons().len(), 1, "and it reads back as the one reason");
    }
}
