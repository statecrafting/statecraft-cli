//! The root set, supplied outside the evidence (spec 004 B-4, B-5).

use serde::{Deserialize, Serialize};

use crate::hash::{Hash, KeyId};
use crate::hlc::Hlc;
use crate::sign::PublicKey;

/// A root's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RootStatus {
    /// Eligible inside its window.
    Active,
    /// Positively revoked; signatures at or after `since` fail.
    Revoked,
    /// Positively excluded; every signature fails.
    Excluded,
}

/// One root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Root {
    /// The key id (hash of the public key).
    pub key_id: KeyId,
    /// The public key, lowercase hex.
    pub public_key: String,
    /// What it may sign.
    pub scope: String,
    /// Eligible from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_before: Option<Hlc>,
    /// Eligible until.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_after: Option<Hlc>,
    /// Status.
    pub status: RootStatus,
    /// For a revoked key, the start of the compromise window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<Hlc>,
}

/// Where the anchors came from (hqgit 064 B-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnchorOrigin {
    /// Obtained independently of the evidence being judged.
    Pinned,
    /// Folded from the ledger that carries the evidence: self-anchored.
    EvidenceLedger,
}

/// The root set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootSet {
    /// Its version.
    pub root_set_version: u32,
    /// The roots.
    pub roots: Vec<Root>,
    /// Who signed the set, out of band.
    pub signed_by: String,
    /// Where it came from.
    pub origin: AnchorOrigin,
}

/// What the set says about a key at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValidity {
    /// Eligible.
    Valid,
    /// Positively refused, with why.
    Refused(&'static str),
    /// Not in the set.
    Unknown,
}

impl RootSet {
    /// An empty pinned set.
    pub fn empty() -> Self {
        RootSet {
            root_set_version: 0,
            roots: Vec::new(),
            signed_by: String::new(),
            origin: AnchorOrigin::Pinned,
        }
    }

    /// The digest that names this set in every verdict.
    pub fn digest(&self) -> Hash {
        let j = serde_json::to_value(self).expect("root set serializes");
        let v = crate::value::Value::from_json(&j).expect("root set values are portable");
        Hash::of(&crate::cbor::encode(&v))
    }

    /// The public key for an id, if the set holds it.
    pub fn public_key(&self, id: &KeyId) -> Option<PublicKey> {
        self.roots
            .iter()
            .find(|r| r.key_id == *id)
            .and_then(|r| PublicKey::from_hex(&r.public_key).ok())
    }

    /// Eligibility of a key at a time.
    pub fn key_valid_at(&self, id: &KeyId, at: Hlc) -> KeyValidity {
        let Some(r) = self.roots.iter().find(|r| r.key_id == *id) else {
            return KeyValidity::Unknown;
        };
        match r.status {
            RootStatus::Excluded => KeyValidity::Refused("key-excluded"),
            RootStatus::Revoked => {
                let since = r.since.unwrap_or(Hlc::default());
                if at >= since {
                    KeyValidity::Refused("key-revoked")
                } else {
                    Self::window(r, at)
                }
            }
            RootStatus::Active => Self::window(r, at),
        }
    }

    fn window(r: &Root, at: Hlc) -> KeyValidity {
        if let Some(nb) = r.not_before
            && at < nb
        {
            return KeyValidity::Refused("key-not-yet-effective");
        }
        if let Some(na) = r.not_after
            && at > na
        {
            return KeyValidity::Refused("key-expired");
        }
        KeyValidity::Valid
    }
}
