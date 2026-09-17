//! The DAG entry (hqgit 017 B-1 to B-6, adopted; spec 003 B-5, B-6).
//!
//! `Entry { parents, issuer, hlc, payload, sig, extra }`; canonical bytes are
//! the CBOR map with `extra` flattened; the signing preimage is those bytes
//! with `sig` absent under the entry domain; the hash covers the signature.
//! In the first slice a scope is linear: exactly one parent, the head
//! (amendment A-02), and the format keeps `parents` a vector.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::cbor::{decode, encode};
use crate::hash::{Cid, Hash, KeyId};
use crate::hlc::Hlc;
use crate::sign::{DOMAIN_ENTRY, PublicKey, Signature, Signer};
use crate::value::Value;

/// The hash of an entry's canonical bytes, signature included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntryHash(pub Hash);

const KNOWN_KEYS: [&str; 5] = ["hlc", "issuer", "parents", "payload", "sig"];

/// An entry before it is signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedEntry {
    /// Sorted, deduplicated parent hashes; empty only for genesis.
    pub parents: Vec<EntryHash>,
    /// The signing key's id.
    pub issuer: KeyId,
    /// The issuer's clock.
    pub hlc: Hlc,
    /// The payload commitment: a fact envelope object.
    pub payload: Cid,
    /// Unknown fields, preserved and hashed.
    pub extra: BTreeMap<String, Value>,
}

/// A signed entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The unsigned fields.
    pub body: UnsignedEntry,
    /// The signature over the unsigned canonical bytes.
    pub sig: Signature,
}

impl UnsignedEntry {
    /// Build with the parent rule enforced.
    pub fn new(
        parents: Vec<EntryHash>,
        issuer: KeyId,
        hlc: Hlc,
        payload: Cid,
        extra: BTreeMap<String, Value>,
    ) -> Result<Self, Error> {
        let mut sorted = parents.clone();
        sorted.sort();
        sorted.dedup();
        if sorted != parents {
            return Err(Error::Validation(
                "parents must be sorted ascending and free of duplicates".into(),
            ));
        }
        for k in extra.keys() {
            if KNOWN_KEYS.contains(&k.as_str()) {
                return Err(Error::Validation(format!(
                    "extra key {k:?} collides with a known key"
                )));
            }
        }
        Ok(UnsignedEntry {
            parents,
            issuer,
            hlc,
            payload,
            extra,
        })
    }

    fn to_value(&self, sig: Option<&Signature>) -> Value {
        let mut m = self.extra.clone();
        m.insert("hlc".into(), self.hlc.to_value());
        m.insert("issuer".into(), Value::Bytes(self.issuer.0.0.to_vec()));
        m.insert(
            "parents".into(),
            Value::Array(
                self.parents
                    .iter()
                    .map(|p| Value::Bytes(p.0.0.to_vec()))
                    .collect(),
            ),
        );
        m.insert("payload".into(), Value::Link(self.payload));
        if let Some(s) = sig {
            m.insert("sig".into(), Value::Bytes(s.0.to_vec()));
        }
        Value::Map(m)
    }

    /// The signing preimage body: canonical bytes with `sig` absent.
    pub fn unsigned_bytes(&self) -> Vec<u8> {
        encode(&self.to_value(None))
    }

    /// The only way to produce a signed entry.
    pub fn sign(self, signer: &Signer) -> Result<Entry, Error> {
        if signer.public().id() != self.issuer {
            return Err(Error::Validation(
                "signer does not hold the issuer key".into(),
            ));
        }
        let sig = signer.sign(DOMAIN_ENTRY, &self.unsigned_bytes());
        Ok(Entry { body: self, sig })
    }
}

impl Entry {
    /// Canonical bytes, signature included.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        encode(&self.body.to_value(Some(&self.sig)))
    }

    /// Recomputed from bytes, never cached (hqgit 017 B-4).
    pub fn hash(&self) -> EntryHash {
        EntryHash(Hash::of(&self.canonical_bytes()))
    }

    /// The `Raw` object identifier the entry travels under: its hash (hqgit 091 B-2).
    pub fn object_cid(&self) -> Cid {
        Cid {
            codec: crate::hash::Codec::Raw,
            hash: self.hash().0,
        }
    }

    /// Verify the signature against a public key.
    pub fn verify_signature(&self, key: &PublicKey) -> Result<(), Error> {
        if key.id() != self.body.issuer {
            return Err(Error::Crypto("key is not the entry's issuer".into()));
        }
        key.verify(DOMAIN_ENTRY, &self.body.unsigned_bytes(), &self.sig)
    }

    /// Decode from canonical bytes, refusing a violation of the shape.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let v = decode(bytes)?;
        let m = v
            .as_map()
            .ok_or_else(|| Error::Validation("entry is not a map".into()))?;
        let mut extra = m.clone();
        let hlc = Hlc::from_value(
            extra
                .remove("hlc")
                .ok_or_else(|| Error::Validation("missing hlc".into()))?
                .as_ref(),
        )?;
        let issuer = match extra.remove("issuer") {
            Some(Value::Bytes(b)) => KeyId(Hash::from_bytes(&b)?),
            _ => return Err(Error::Validation("missing issuer".into())),
        };
        let parents = match extra.remove("parents") {
            Some(Value::Array(a)) => a
                .iter()
                .map(|x| match x {
                    Value::Bytes(b) => Ok(EntryHash(Hash::from_bytes(b)?)),
                    _ => Err(Error::Validation("parent is not bytes".into())),
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(Error::Validation("missing parents".into())),
        };
        let payload = match extra.remove("payload") {
            Some(Value::Link(c)) => c,
            _ => return Err(Error::Validation("missing payload link".into())),
        };
        let sig = match extra.remove("sig") {
            Some(Value::Bytes(b)) => Signature::from_bytes(&b)?,
            _ => return Err(Error::Validation("missing sig".into())),
        };
        let body = UnsignedEntry::new(parents, issuer, hlc, payload, extra)?;
        let e = Entry { body, sig };
        if e.canonical_bytes() != bytes {
            return Err(Error::Validation(
                "bytes are not the canonical encoding of the decoded entry".into(),
            ));
        }
        Ok(e)
    }
}

trait AsRefValue {
    fn as_ref(&self) -> &Value;
}
impl AsRefValue for Value {
    fn as_ref(&self) -> &Value {
        self
    }
}

/// A linear scope in memory: genesis, head, append checks (spec 003 B-6, B-9's seam).
#[derive(Debug, Clone)]
pub struct LinearScope {
    entries: Vec<Entry>,
}

impl LinearScope {
    /// The only constructor: a genesis entry with no parents.
    pub fn new(genesis: Entry) -> Result<Self, Error> {
        if !genesis.body.parents.is_empty() {
            return Err(Error::Validation("genesis must have no parents".into()));
        }
        Ok(LinearScope {
            entries: vec![genesis],
        })
    }

    /// The current head.
    pub fn head(&self) -> EntryHash {
        self.entries.last().expect("a scope has a genesis").hash()
    }

    /// The head entry's clock.
    pub fn head_hlc(&self) -> Hlc {
        self.entries.last().expect("a scope has a genesis").body.hlc
    }

    /// Append: exactly one parent, the head; a strictly greater clock; a
    /// duplicate hash is a no-op `Ok`.
    pub fn append(&mut self, entry: Entry) -> Result<EntryHash, Error> {
        let h = entry.hash();
        if self.entries.iter().any(|e| e.hash() == h) {
            return Ok(h);
        }
        let head = self.head();
        if entry.body.parents.len() != 1 || entry.body.parents[0] != head {
            return Err(Error::Validation(format!(
                "entry must name the head {} as its only parent",
                head.0.to_hex()
            )));
        }
        if entry.body.hlc <= self.head_hlc() {
            return Err(Error::Validation(
                "entry clock must be strictly greater than the head's".into(),
            ));
        }
        self.entries.push(entry);
        Ok(h)
    }

    /// Every entry in order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Whether the scope holds an entry.
    pub fn contains(&self, h: EntryHash) -> bool {
        self.entries.iter().any(|e| e.hash() == h)
    }

    /// Walk every entry: recomputed hash chain, parent rule, clock rule and
    /// signatures through the resolver. Lists every failure; never stops at
    /// the first (hqgit 017 B-7).
    pub fn verify(
        &self,
        resolve: &dyn Fn(&KeyId) -> Option<PublicKey>,
    ) -> Vec<(EntryHash, String)> {
        let mut failures = Vec::new();
        let mut prev: Option<&Entry> = None;
        for e in &self.entries {
            let h = e.hash();
            match prev {
                None => {
                    if !e.body.parents.is_empty() {
                        failures.push((h, "genesis has parents".into()));
                    }
                }
                Some(p) => {
                    if e.body.parents.len() != 1 || e.body.parents[0] != p.hash() {
                        failures.push((h, "parent is not the previous entry".into()));
                    }
                    if e.body.hlc <= p.body.hlc {
                        failures.push((h, "clock not greater than parent".into()));
                    }
                }
            }
            match resolve(&e.body.issuer) {
                None => failures.push((h, "issuer key unknown".into())),
                Some(k) => {
                    if e.verify_signature(&k).is_err() {
                        failures.push((h, "signature does not verify".into()));
                    }
                }
            }
            prev = Some(e);
        }
        failures
    }
}
