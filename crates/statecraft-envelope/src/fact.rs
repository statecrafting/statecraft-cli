//! The fact envelope, the registry seam, tombstones and payload resolution
//! (hqgit 019, 020; spec 003 B-7, B-8).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::cbor::{cid_of, decode};
use crate::entry::EntryHash;
use crate::hash::{Cid, Hash};
use crate::value::Value;

const KNOWN_KEYS: [&str; 3] = ["body", "kind", "v"];

/// `{ kind, v, body, extra }`: what every entry payload decodes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactEnvelope {
    /// The fact kind, e.g. `revision.registered`.
    pub kind: String,
    /// The kind's version.
    pub v: u32,
    /// The body, a map.
    pub body: BTreeMap<String, Value>,
    /// Unknown top-level fields, preserved and hashed.
    pub extra: BTreeMap<String, Value>,
}

impl FactEnvelope {
    /// Build.
    pub fn new(kind: &str, v: u32, body: BTreeMap<String, Value>) -> Self {
        FactEnvelope {
            kind: kind.to_string(),
            v,
            body,
            extra: BTreeMap::new(),
        }
    }

    /// The canonical value.
    pub fn to_value(&self) -> Value {
        let mut m = self.extra.clone();
        m.insert("kind".into(), Value::Text(self.kind.clone()));
        m.insert("v".into(), Value::Int(self.v as i64));
        m.insert("body".into(), Value::Map(self.body.clone()));
        Value::Map(m)
    }

    /// The canonical bytes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        crate::cbor::encode(&self.to_value())
    }

    /// The object identifier the payload link names.
    pub fn cid(&self) -> Cid {
        cid_of(&self.to_value())
    }

    /// Decode from canonical bytes; unknown kinds decode fine.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let v = decode(bytes)?;
        let m = v
            .as_map()
            .ok_or_else(|| Error::Validation("fact is not a map".into()))?;
        let mut extra = m.clone();
        let kind = match extra.remove("kind") {
            Some(Value::Text(t)) => t,
            _ => return Err(Error::Validation("missing kind".into())),
        };
        let v = match extra.remove("v") {
            Some(Value::Int(i)) if i >= 0 && i <= u32::MAX as i64 => i as u32,
            _ => return Err(Error::Validation("missing or invalid v".into())),
        };
        let body = match extra.remove("body") {
            Some(Value::Map(b)) => b,
            _ => return Err(Error::Validation("missing body map".into())),
        };
        for k in extra.keys() {
            if KNOWN_KEYS.contains(&k.as_str()) {
                return Err(Error::Validation("extra collides with a known key".into()));
            }
        }
        Ok(FactEnvelope {
            kind,
            v,
            body,
            extra,
        })
    }
}

/// A validator for a registered fact kind.
pub trait FactValidator {
    /// Refuse a body that does not fit the kind.
    fn validate(&self, v: u32, body: &BTreeMap<String, Value>) -> Result<(), Error>;
}

/// The registry seam: known kinds validate, unknown kinds stay opaque.
#[derive(Default)]
pub struct FactRegistry {
    validators: BTreeMap<String, Box<dyn FactValidator>>,
}

/// The outcome of checking a fact against the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactVerdict {
    /// Registered and valid.
    Valid,
    /// Registered and invalid, with the reason.
    Invalid(String),
    /// Not registered; preserved and opaque, never rejected.
    Unregistered,
}

impl FactRegistry {
    /// Register a validator for a kind.
    pub fn register(&mut self, kind: &str, v: Box<dyn FactValidator>) {
        self.validators.insert(kind.to_string(), v);
    }

    /// Check a fact.
    pub fn check(&self, f: &FactEnvelope) -> FactVerdict {
        match self.validators.get(&f.kind) {
            None => FactVerdict::Unregistered,
            Some(val) => match val.validate(f.v, &f.body) {
                Ok(()) => FactVerdict::Valid,
                Err(e) => FactVerdict::Invalid(e.to_string()),
            },
        }
    }

    /// The first-slice vocabulary (P-05), each with a required-keys validator.
    pub fn first_slice() -> Self {
        let mut r = FactRegistry::default();
        let kinds: &[(&str, &[&str])] = &[
            (
                "repository.registered",
                &[
                    "tenant",
                    "origin",
                    "object_format",
                    "default_branch",
                    "policy",
                    "registered_by",
                ],
            ),
            (
                "repository.policy-changed",
                &["repository", "from", "to", "changed_by"],
            ),
            (
                "revision.registered",
                &["repository", "commit", "registered_by"],
            ),
            (
                "attestation.issued",
                &["attestation", "subject", "predicate", "issuer"],
            ),
            ("ledger.tombstone", &["target", "reason", "scope"]),
            ("identity.created", &["kind", "initial_key", "display"]),
            (
                "identity.key_rotated",
                &[
                    "identity",
                    "prev",
                    "next",
                    "next_pub",
                    "effective",
                    "cosign",
                ],
            ),
            (
                "identity.key_revoked",
                &["identity", "key", "reason", "effective"],
            ),
            ("trust.root-set-changed", &["from", "to", "changed_by"]),
        ];
        for (k, req) in kinds {
            r.register(
                k,
                Box::new(RequiredKeys {
                    keys: req.iter().map(|s| s.to_string()).collect(),
                    v: 1,
                }),
            );
        }
        r
    }
}

/// A validator that requires a version and a set of keys.
pub struct RequiredKeys {
    /// The keys that must be present.
    pub keys: Vec<String>,
    /// The only accepted version.
    pub v: u32,
}

impl FactValidator for RequiredKeys {
    fn validate(&self, v: u32, body: &BTreeMap<String, Value>) -> Result<(), Error> {
        if v != self.v {
            return Err(Error::Validation(format!("version {v} is not {}", self.v)));
        }
        for k in &self.keys {
            if !body.contains_key(k) {
                return Err(Error::Validation(format!("missing key {k:?}")));
            }
        }
        Ok(())
    }
}

/// Why an object was erased (hqgit 020 B-5, plus `Retention` per A-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EraseReason {
    /// A tenant's erasure request.
    Erasure,
    /// An operator's moderation action.
    Moderation,
    /// A legal requirement.
    Legal,
    /// Retention expiry.
    Retention,
}

impl EraseReason {
    /// The wire word.
    pub fn as_str(self) -> &'static str {
        match self {
            EraseReason::Erasure => "erasure",
            EraseReason::Moderation => "moderation",
            EraseReason::Legal => "legal",
            EraseReason::Retention => "retention",
        }
    }
}

/// What a tombstone covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TombstoneScope {
    /// The object, for every commitment in the scope that names it.
    Object,
    /// The payload of one entry.
    Payload(EntryHash),
}

/// A tombstone: the fact that an object was erased.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tombstone {
    /// The erased commitment.
    pub target: Cid,
    /// Why.
    pub reason: EraseReason,
    /// What it covers.
    pub scope: TombstoneScope,
}

impl Tombstone {
    /// The `ledger.tombstone` fact.
    pub fn to_fact(&self) -> FactEnvelope {
        let mut body = BTreeMap::new();
        body.insert("target".into(), Value::Link(self.target));
        body.insert("reason".into(), Value::Text(self.reason.as_str().into()));
        body.insert(
            "scope".into(),
            match self.scope {
                TombstoneScope::Object => Value::Text("object".into()),
                TombstoneScope::Payload(h) => Value::map()
                    .with("payload", Value::Bytes(h.0.0.to_vec()))
                    .unwrap(),
            },
        );
        FactEnvelope::new("ledger.tombstone", 1, body)
    }

    /// From a `ledger.tombstone` fact.
    pub fn from_fact(f: &FactEnvelope) -> Result<Self, Error> {
        if f.kind != "ledger.tombstone" || f.v != 1 {
            return Err(Error::Validation("not a ledger.tombstone v1".into()));
        }
        let target = match f.body.get("target") {
            Some(Value::Link(c)) => *c,
            _ => return Err(Error::Validation("tombstone target is not a link".into())),
        };
        let reason = match f.body.get("reason").and_then(Value::as_text) {
            Some("erasure") => EraseReason::Erasure,
            Some("moderation") => EraseReason::Moderation,
            Some("legal") => EraseReason::Legal,
            Some("retention") => EraseReason::Retention,
            _ => return Err(Error::Validation("unknown erase reason".into())),
        };
        let scope = match f.body.get("scope") {
            Some(Value::Text(t)) if t == "object" => TombstoneScope::Object,
            Some(Value::Map(m)) => match m.get("payload") {
                Some(Value::Bytes(b)) => TombstoneScope::Payload(EntryHash(Hash::from_bytes(b)?)),
                _ => {
                    return Err(Error::Validation(
                        "tombstone scope payload is not bytes".into(),
                    ));
                }
            },
            _ => return Err(Error::Validation("unknown tombstone scope".into())),
        };
        Ok(Tombstone {
            target,
            reason,
            scope,
        })
    }
}

/// The tombstones a scope holds, folded from its facts.
#[derive(Debug, Default, Clone)]
pub struct TombstoneSet {
    objects: BTreeMap<Cid, Tombstone>,
    payloads: BTreeMap<EntryHash, Tombstone>,
}

impl TombstoneSet {
    /// Record one.
    pub fn add(&mut self, t: Tombstone) {
        match t.scope {
            TombstoneScope::Object => {
                self.objects.insert(t.target, t);
            }
            TombstoneScope::Payload(h) => {
                self.payloads.insert(h, t);
            }
        }
    }

    /// The tombstone covering an object referenced from an entry, if any.
    pub fn covering(&self, cid: &Cid, entry: Option<EntryHash>) -> Option<&Tombstone> {
        if let Some(t) = self.objects.get(cid) {
            return Some(t);
        }
        entry
            .and_then(|h| self.payloads.get(&h))
            .filter(|t| t.target == *cid)
    }

    /// Every erased object identifier.
    pub fn erased_objects(&self) -> BTreeSet<Cid> {
        self.objects
            .keys()
            .copied()
            .chain(self.payloads.values().map(|t| t.target))
            .collect()
    }
}

/// The three honest answers to a payload lookup (hqgit 020 B-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// The bytes, hash-verified.
    Present(Vec<u8>),
    /// Erased, with the tombstone.
    Erased(Tombstone),
    /// Not erased and not held here.
    Missing,
}

/// Resolve a commitment through a lookup, the tombstones and the hash check.
/// A caller MUST handle all three answers; nothing maps one to another.
pub fn resolve_payload(
    cid: &Cid,
    entry: Option<EntryHash>,
    tombstones: &TombstoneSet,
    lookup: &dyn Fn(&Cid) -> Option<Vec<u8>>,
) -> Result<Resolved, Error> {
    if let Some(t) = tombstones.covering(cid, entry) {
        return Ok(Resolved::Erased(t.clone()));
    }
    match lookup(cid) {
        None => Ok(Resolved::Missing),
        Some(bytes) => {
            if Hash::of(&bytes) != cid.hash {
                return Err(Error::Crypto(format!(
                    "object {} does not hash to its identifier",
                    cid.to_string_key()
                )));
            }
            Ok(Resolved::Present(bytes))
        }
    }
}
