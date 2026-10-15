//! The three closed projections (spec 027 section 3.3).
//!
//! A projection is a pure function from an observation's reports to a value
//! with a canonical encoding. The set is closed: `corpus-state`,
//! `work-eligibility` and `contract-identity`. A refused observation has only
//! the first (section 3.2).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The canonical digest of a value: SHA-256 of its JSON with every object's
/// keys in sorted order, which is how `serde_json::Value` holds them.
pub fn canonical_digest<T: Serialize>(value: &T) -> String {
    let value = serde_json::to_value(value).expect("projection values serialize");
    let bytes = serde_json::to_vec(&value).expect("a JSON value serializes");
    statecraft_environment::digest::digest_bytes(&bytes)
}

/// `corpus-state`: whether spec-spine answered for the observed commit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum CorpusState {
    /// spec-spine answered.
    Answered,
    /// spec-spine declined to answer, with its own exit class.
    Refused {
        /// spec-spine's class, as it reported it.
        class: String,
    },
}

impl CorpusState {
    /// Whether this is `answered`.
    pub fn answered(&self) -> bool {
        matches!(self, CorpusState::Answered)
    }

    /// The value as section 3.3 writes it: `answered` or `refused(<class>)`.
    pub fn word(&self) -> String {
        match self {
            CorpusState::Answered => "answered".to_string(),
            CorpusState::Refused { class } => format!("refused({class})"),
        }
    }
}

/// One `work-eligibility` row: spec 003's join, for one spec the plan report
/// names as ready.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "eligibility", rename_all = "kebab-case")]
pub enum Eligibility {
    /// Eligible to run.
    Eligible,
    /// Excluded, with spec 003's reason.
    Excluded {
        /// spec 003's reason.
        reason: String,
    },
}

/// One `contract-identity` row, for an `eligible` spec: the context-closure
/// digest spec 003 section 3.1.3 binds an attempt to, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "contract", rename_all = "kebab-case")]
pub enum Contract {
    /// The closure digest.
    Digest {
        /// The digest, as spec 003's binding records it.
        digest: String,
    },
    /// No closure, with spec-spine's reason.
    Unavailable {
        /// spec-spine's reason.
        reason: String,
    },
}

impl Contract {
    /// The digest, when there is one.
    pub fn digest(&self) -> Option<&str> {
        match self {
            Contract::Digest { digest } => Some(digest),
            Contract::Unavailable { .. } => None,
        }
    }

    /// Whether moving from `self` to `other` is a `contract-moved`
    /// transition: two different digests, or a digest on one side only. Two
    /// `unavailable` values are not a move, whatever their reasons, because
    /// no contract was bound under either (section 3.3).
    pub fn moved_to(&self, other: &Contract) -> bool {
        match (self.digest(), other.digest()) {
            (Some(a), Some(b)) => a != b,
            (None, None) => false,
            _ => true,
        }
    }
}

/// An observation's projections. A refused observation carries only
/// `corpus-state`; its two maps are empty and never compared (section 3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Projections {
    /// `corpus-state`.
    pub corpus_state: CorpusState,
    /// `work-eligibility`, keyed by spec id.
    #[serde(default, with = "rows")]
    pub work_eligibility: BTreeMap<String, Eligibility>,
    /// `contract-identity`, keyed by spec id, for every `eligible` row.
    #[serde(default, with = "rows")]
    pub contract_identity: BTreeMap<String, Contract>,
}

/// A map keyed by spec id, written as rows in id order, each naming its
/// spec: a spec id is data, not a field name, so it is never a JSON key.
mod rows {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    #[derive(Serialize, Deserialize)]
    struct Row<T> {
        spec: String,
        #[serde(flatten)]
        value: T,
    }

    pub fn serialize<T: Serialize + Clone, S: Serializer>(
        map: &BTreeMap<String, T>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        map.iter()
            .map(|(spec, value)| Row {
                spec: spec.clone(),
                value: value.clone(),
            })
            .collect::<Vec<_>>()
            .serialize(s)
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<String, T>, D::Error> {
        Ok(Vec::<Row<T>>::deserialize(d)?
            .into_iter()
            .map(|r| (r.spec, r.value))
            .collect())
    }
}

/// Each projection's canonical digest; a refused observation has only one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionDigests {
    /// `corpus-state`.
    pub corpus_state: String,
    /// `work-eligibility`, for an answered observation.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub work_eligibility: Option<String>,
    /// `contract-identity`, for an answered observation.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub contract_identity: Option<String>,
}

impl Projections {
    /// An answered observation's projections.
    ///
    /// `contract` is consulted only for `eligible` rows, and an `eligible`
    /// row it has no answer for is a programming error the caller avoids by
    /// binding every eligible row; it is recorded as `unavailable` rather than
    /// dropped, so a missing answer is never read as no contract at all.
    pub fn answered(
        work_eligibility: BTreeMap<String, Eligibility>,
        mut contract: impl FnMut(&str) -> Contract,
    ) -> Self {
        let contract_identity = work_eligibility
            .iter()
            .filter(|(_, e)| matches!(e, Eligibility::Eligible))
            .map(|(id, _)| (id.clone(), contract(id)))
            .collect();
        Projections {
            corpus_state: CorpusState::Answered,
            work_eligibility,
            contract_identity,
        }
    }

    /// A refused observation's one projection.
    pub fn refused(class: &str) -> Self {
        Projections {
            corpus_state: CorpusState::Refused {
                class: class.to_string(),
            },
            work_eligibility: BTreeMap::new(),
            contract_identity: BTreeMap::new(),
        }
    }

    /// Each projection's canonical digest.
    pub fn digests(&self) -> ProjectionDigests {
        let answered = self.corpus_state.answered();
        ProjectionDigests {
            corpus_state: canonical_digest(&self.corpus_state),
            work_eligibility: answered.then(|| canonical_digest(&self.work_eligibility)),
            contract_identity: answered.then(|| canonical_digest(&self.contract_identity)),
        }
    }

    /// The specs `eligible` here, in id order.
    pub fn eligible(&self) -> impl Iterator<Item = &str> {
        self.work_eligibility
            .iter()
            .filter(|(_, e)| matches!(e, Eligibility::Eligible))
            .map(|(id, _)| id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(d: &str) -> Contract {
        Contract::Digest {
            digest: d.to_string(),
        }
    }
    fn unavailable(r: &str) -> Contract {
        Contract::Unavailable {
            reason: r.to_string(),
        }
    }

    #[test]
    fn two_unavailable_contracts_are_not_a_move_and_every_other_difference_is() {
        assert!(!unavailable("a").moved_to(&unavailable("b")));
        assert!(!digest("x").moved_to(&digest("x")));
        assert!(digest("x").moved_to(&digest("y")));
        assert!(digest("x").moved_to(&unavailable("a")));
        assert!(unavailable("a").moved_to(&digest("x")));
    }

    #[test]
    fn a_refused_observation_has_one_projection_and_one_digest() {
        let p = Projections::refused("stale");
        assert_eq!(p.corpus_state.word(), "refused(stale)");
        let d = p.digests();
        assert!(d.work_eligibility.is_none() && d.contract_identity.is_none());
    }

    #[test]
    fn contracts_are_bound_for_eligible_rows_only_and_digests_are_canonical() {
        let rows = BTreeMap::from([
            ("002-b".to_string(), Eligibility::Eligible),
            (
                "001-a".to_string(),
                Eligibility::Excluded {
                    reason: "draft".to_string(),
                },
            ),
        ]);
        let mut asked = Vec::new();
        let p = Projections::answered(rows.clone(), |id| {
            asked.push(id.to_string());
            digest("c")
        });
        assert_eq!(asked, ["002-b"]);
        assert_eq!(p.eligible().collect::<Vec<_>>(), ["002-b"]);
        let q = Projections::answered(rows, |_| digest("c"));
        assert_eq!(p.digests(), q.digests());
        assert_eq!(
            canonical_digest(&serde_json::json!({"b": 1, "a": 2})),
            canonical_digest(&serde_json::json!({"a": 2, "b": 1}))
        );
    }
}
