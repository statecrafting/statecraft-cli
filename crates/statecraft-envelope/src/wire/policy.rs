//! The attempt's wire-evidence policy (spec 014 section 3.5).
//!
//! Witness evidence is optional by default: a policy with no `requirement`
//! requires nothing, and a policy recorded before this member existed decodes
//! as exactly that. A requirement binds the artifact type, a minimum
//! completeness, the evidence dimensions that must pass, and the absence
//! states it permits. **It has no member that could select a provider, trust
//! root, credential, retention expansion or live action**, and an unknown
//! member is refused rather than ignored, so a policy cannot carry one.
//!
//! `secretDetector` is the exact identity section 3.4's screen is bound to.
//! A policy without one admits nothing: the screen fails closed.

use serde::{Deserialize, Serialize};

use super::manifest::Completeness;
use crate::hash::sha256_hex;

/// An evidence dimension a requirement can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DimensionName {
    /// `integrity`.
    Integrity,
    /// `signature`.
    Signature,
    /// `issuerTrust`.
    IssuerTrust,
    /// `subjectBinding`.
    SubjectBinding,
}

/// An absence state a requirement may permit in place of the artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermittedAbsence {
    /// No artifact was recorded for the attempt.
    NotRecorded,
    /// The artifact could not be read.
    Unavailable,
    /// The artifact's state could not be decided.
    Unknown,
}

/// What a policy requires of one attempt's wire evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Requirement {
    /// The artifact type required; only `statecraft/wire-exchange/v1` exists.
    pub artifact_type: String,
    /// The lowest completeness admitted.
    pub minimum_completeness: Completeness,
    /// The dimensions that must be `pass`.
    #[serde(default)]
    pub required_dimensions: Vec<DimensionName>,
    /// The absence states accepted in place of an admitted artifact.
    #[serde(default)]
    pub permitted_absence: Vec<PermittedAbsence>,
}

/// One attempt's wire-evidence policy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WirePolicy {
    /// The exact secret-detector identity the screen is bound to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_detector: Option<String>,
    /// The requirement, or none: the evidence is optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<Requirement>,
}

impl WirePolicy {
    /// The policy's identity: `sha256:` over its JSON bytes as this type
    /// writes them. Field order is fixed by the type, and the two lists are
    /// sorted and deduplicated first, so one policy has one identity.
    pub fn identity(&self) -> String {
        let mut canonical = self.clone();
        if let Some(r) = canonical.requirement.as_mut() {
            r.required_dimensions.sort();
            r.required_dimensions.dedup();
            r.permitted_absence.sort();
            r.permitted_absence.dedup();
        }
        let bytes = serde_json::to_vec(&canonical).expect("a policy serializes");
        format!("sha256:{}", sha256_hex(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_historical_policy_is_the_empty_requirement() {
        let p: WirePolicy = serde_json::from_str("{}").unwrap();
        assert_eq!(p, WirePolicy::default());
        assert!(p.requirement.is_none());
    }

    #[test]
    fn a_policy_cannot_carry_a_member_that_selects_a_provider() {
        let r = serde_json::from_str::<WirePolicy>(r#"{"provider":"x"}"#);
        assert!(r.is_err());
    }

    #[test]
    fn list_order_does_not_change_the_identity() {
        let with = |dims: Vec<DimensionName>| WirePolicy {
            secret_detector: Some("d".into()),
            requirement: Some(Requirement {
                artifact_type: super::super::EVIDENCE_TYPE.into(),
                minimum_completeness: Completeness::Complete,
                required_dimensions: dims,
                permitted_absence: Vec::new(),
            }),
        };
        let a = with(vec![DimensionName::Signature, DimensionName::Integrity]);
        let b = with(vec![DimensionName::Integrity, DimensionName::Signature]);
        assert_eq!(a.identity(), b.identity());
        assert_ne!(a.identity(), WirePolicy::default().identity());
    }
}
