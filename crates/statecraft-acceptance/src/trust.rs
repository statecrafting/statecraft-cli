//! Trust roots, supplied independently of the evidence being judged.
//!
//! Spec 005 section 3.6. The rule that gives this module its shape: **a chain
//! anchored on a key the evidence itself carries cannot satisfy a trusted-issuer
//! policy, whatever its internal consistency.** Self-consistency is not trust,
//! and a verifier that accepted it would be checking arithmetic rather than
//! provenance.
//!
//! A verifier **never executes anything the evidence carries**.

use crate::dimensions::{IssuerTrust, Signature};
use serde::{Deserialize, Serialize};

/// A root this deployment trusts, or explicitly does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Root {
    /// How the root is identified.
    pub id: String,
    /// False for a root that is revoked or excluded.
    pub eligible: bool,
}

/// The set of roots, and its own identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootSet {
    /// An identifier for the set as a whole, recorded in the outcome.
    pub id: String,
    /// The roots.
    pub roots: Vec<Root>,
}

impl RootSet {
    /// A named, empty set. Every issuer is then `unknown`.
    pub fn empty(id: &str) -> Self {
        Self {
            id: id.to_string(),
            roots: Vec::new(),
        }
    }

    /// Add an eligible root.
    #[must_use]
    pub fn trusting(mut self, id: &str) -> Self {
        self.roots.push(Root {
            id: id.to_string(),
            eligible: true,
        });
        self
    }

    /// Add a revoked or excluded root.
    #[must_use]
    pub fn revoking(mut self, id: &str) -> Self {
        self.roots.push(Root {
            id: id.to_string(),
            eligible: false,
        });
        self
    }

    /// Look a root up.
    pub fn get(&self, id: &str) -> Option<&Root> {
        self.roots.iter().find(|r| r.id == id)
    }
}

/// What the evidence says it is anchored on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "anchor")]
pub enum Anchor {
    /// A root identified by id, to be looked up in an independently supplied set.
    External {
        /// The root's id.
        root_id: String,
    },
    /// A key the evidence itself carries.
    ///
    /// **Cannot satisfy a trusted-issuer policy.** Internal consistency is not
    /// the question being asked.
    SelfCarried,
}

/// What a verifier did, preserved in the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierRecord {
    /// Which verifier.
    pub verifier: String,
    /// Its version.
    pub version: String,
    /// The root-set identity it judged against.
    pub root_set: String,
    /// What was actually checked.
    pub coverage: Vec<String>,
    /// Why it stopped, if it stopped early.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_early: Option<String>,
}

/// Judge issuer trust.
///
/// Every path through this function that is not an eligible external root
/// refuses to say `pass`, which is the only way the section 3.6 rules can be
/// enforced rather than described.
pub fn issuer_trust(signature: Signature, anchor: &Anchor, roots: &RootSet) -> IssuerTrust {
    // Issuer trust can pass only AFTER a passing signature. A passing signature
    // alone never establishes it, and a non-passing one settles it here.
    if signature != Signature::Pass {
        return IssuerTrust::Unknown;
    }
    match anchor {
        Anchor::SelfCarried => IssuerTrust::Unknown,
        Anchor::External { root_id } => match roots.get(root_id) {
            None => IssuerTrust::Unknown,
            Some(r) if r.eligible => IssuerTrust::Pass,
            Some(_) => IssuerTrust::Fail,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_root_yields_unknown() {
        let roots = RootSet::empty("none");
        assert_eq!(
            issuer_trust(
                Signature::Pass,
                &Anchor::External {
                    root_id: "nobody".into()
                },
                &roots
            ),
            IssuerTrust::Unknown
        );
    }

    #[test]
    fn a_revoked_root_yields_fail() {
        let roots = RootSet::empty("set").revoking("compromised");
        assert_eq!(
            issuer_trust(
                Signature::Pass,
                &Anchor::External {
                    root_id: "compromised".into()
                },
                &roots
            ),
            IssuerTrust::Fail
        );
    }

    #[test]
    fn a_self_carried_anchor_never_satisfies_a_trusted_issuer_policy() {
        let roots = RootSet::empty("set").trusting("anything");
        assert_eq!(
            issuer_trust(Signature::Pass, &Anchor::SelfCarried, &roots),
            IssuerTrust::Unknown,
            "internal consistency is not trust"
        );
    }

    #[test]
    fn a_passing_signature_alone_does_not_establish_issuer_trust() {
        // The signature verifies, and there is no root set to judge it against.
        assert_eq!(
            issuer_trust(
                Signature::Pass,
                &Anchor::External {
                    root_id: "r".into()
                },
                &RootSet::empty("empty")
            ),
            IssuerTrust::Unknown
        );
    }

    #[test]
    fn an_eligible_root_with_a_passing_signature_passes() {
        let roots = RootSet::empty("set").trusting("release-key");
        assert_eq!(
            issuer_trust(
                Signature::Pass,
                &Anchor::External {
                    root_id: "release-key".into()
                },
                &roots
            ),
            IssuerTrust::Pass
        );
    }

    #[test]
    fn without_a_passing_signature_issuer_trust_is_unknown_whatever_the_root_says() {
        let roots = RootSet::empty("set").trusting("release-key");
        for s in [Signature::Unsigned, Signature::Fail, Signature::Unknown] {
            assert_eq!(
                issuer_trust(
                    s,
                    &Anchor::External {
                        root_id: "release-key".into()
                    },
                    &roots
                ),
                IssuerTrust::Unknown
            );
        }
    }
}
