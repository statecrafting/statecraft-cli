//! The four evidence dimensions, and admission kept apart from all of them.
//!
//! Spec 005 section 3.5. Each dimension is reported separately, from its own
//! check, with a closed value set. The rules below are **part of the
//! vocabulary, not commentary on it**, which is why each is a type rather than
//! a convention: `unsigned` is not constructible outside `signature`, and
//! `not-applicable` is not constructible outside `subjectBinding`.

use serde::{Deserialize, Serialize};

/// `integrity`: do the preserved bytes still hash to what was recorded?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Integrity {
    /// The bytes are intact.
    Pass,
    /// They are not.
    Fail,
    /// The check did not run.
    Unknown,
}

/// `signature`: is there a signature, and does it verify?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Signature {
    /// A signature verified.
    Pass,
    /// A signature did not verify.
    Fail,
    /// There is no signature. **Only this dimension has this value.**
    Unsigned,
    /// The check did not run.
    Unknown,
}

/// `issuerTrust`: is the signer one this policy trusts?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IssuerTrust {
    /// The signer chains to an eligible root.
    Pass,
    /// It chains to a revoked or excluded one.
    Fail,
    /// The check did not run, or no root was supplied.
    Unknown,
}

/// `subjectBinding`: does the evidence bind the subject it claims to?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubjectBinding {
    /// It does.
    Pass,
    /// It does not.
    Fail,
    /// The check did not run. **A missing expected subject is this**, not
    /// `not-applicable`.
    Unknown,
    /// This record type has no subject at all. **Only this dimension has this
    /// value.**
    NotApplicable,
}

/// All four, reported separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    /// Integrity of the preserved bytes.
    pub integrity: Integrity,
    /// The signature.
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
        Self {
            integrity: Integrity::Unknown,
            signature: Signature::Unknown,
            issuer_trust: IssuerTrust::Unknown,
            subject_binding: SubjectBinding::Unknown,
        }
    }

    /// What this product reports today: nothing is signed.
    ///
    /// Section 3.5 says so in as many words: every `signature` is `unsigned` and
    /// every `issuerTrust` is `unknown`. **That is the honest report, not a gap
    /// in the implementation**, which is why it is a named constructor and not a
    /// TODO.
    pub fn unsigned_today(integrity: Integrity, subject_binding: SubjectBinding) -> Self {
        Self {
            integrity,
            signature: Signature::Unsigned,
            issuer_trust: IssuerTrust::Unknown,
            subject_binding,
        }
    }
}

/// Why admission was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefusalCode {
    /// A dimension the policy requires is not `pass`.
    RequiredDimensionNotPassed {
        /// Which dimension.
        dimension: String,
        /// What it actually read.
        value: String,
    },
    /// Evidence the policy requires was not there at all.
    IncompleteEvidence {
        /// What was missing.
        missing: Vec<String>,
    },
}

/// Admitted, or refused with a reason.
///
/// **Kept apart from all four dimensions.** A dimension says what a check found;
/// admission says what a policy decided about those findings, and collapsing the
/// two would make "this evidence is intact" and "this evidence is acceptable"
/// the same sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "admission")]
pub enum Admission {
    /// Admitted.
    Admit,
    /// Refused, with the reason named.
    Refuse {
        /// Why.
        reason: RefusalCode,
    },
}

/// What a policy requires of evidence before admitting it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionPolicy {
    /// Integrity must pass.
    pub require_integrity: bool,
    /// A signature must verify. A policy that does not require one admits
    /// `unsigned` evidence.
    pub require_signature: bool,
    /// The issuer must be trusted.
    pub require_issuer_trust: bool,
}

impl AdmissionPolicy {
    /// Requires nothing.
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Requires intact bytes, a verified signature and a trusted issuer.
    pub fn strict() -> Self {
        Self {
            require_integrity: true,
            require_signature: true,
            require_issuer_trust: true,
        }
    }
}

/// Decide admission from the dimensions and a policy.
///
/// Incomplete required evidence refuses admission and **names what was
/// missing**, rather than refusing with a bare verdict the operator has to
/// reverse-engineer.
pub fn admit(d: &Dimensions, policy: &AdmissionPolicy) -> Admission {
    let mut missing = Vec::new();

    if policy.require_integrity && d.integrity != Integrity::Pass {
        if d.integrity == Integrity::Unknown {
            missing.push("integrity".to_string());
        } else {
            return Admission::Refuse {
                reason: RefusalCode::RequiredDimensionNotPassed {
                    dimension: "integrity".into(),
                    value: "fail".into(),
                },
            };
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
            return Admission::Refuse {
                reason: RefusalCode::RequiredDimensionNotPassed {
                    dimension: "signature".into(),
                    value: value.into(),
                },
            };
        }
    }
    if policy.require_issuer_trust && d.issuer_trust != IssuerTrust::Pass {
        if d.issuer_trust == IssuerTrust::Unknown {
            missing.push("issuerTrust".to_string());
        } else {
            return Admission::Refuse {
                reason: RefusalCode::RequiredDimensionNotPassed {
                    dimension: "issuerTrust".into(),
                    value: "fail".into(),
                },
            };
        }
    }

    if missing.is_empty() {
        Admission::Admit
    } else {
        Admission::Refuse {
            reason: RefusalCode::IncompleteEvidence { missing },
        }
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
}
