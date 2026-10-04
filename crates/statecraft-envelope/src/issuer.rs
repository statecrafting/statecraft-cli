//! Offline issuer authorization (spec 035, amending spec 005).
//!
//! An offline owner root authorizes three kinds of record about the platform
//! issuer: its enrollment, a rotation of its online key, and a revocation of
//! one of its keys. Each kind has exactly one signing domain, fixed by the
//! kind and never chosen by an input string or a payload, so a signature over
//! one kind can never be presented as a signature over another, even over
//! identical bytes.
//!
//! Everything here is a pure function of its arguments. Nothing reads a file,
//! a clock or the environment, and nothing generates entropy: the offline
//! command surface (spec 036) owns custody, and the platform (its spec 004)
//! owns adoption. The seed never appears in a type of this module.
//!
//! The pieces, in the order the spec states them:
//!
//! - strict lowercase-hex parsing of public keys, key ids and signatures;
//! - the enrollment payload and the two fact payloads, each decoded strictly
//!   from canonical bytes;
//! - the detached signature record and the owner root's public record;
//! - [`RootSetV2`], its digest, its full history verification and the issuer
//!   eligibility it answers at an explicit time and scope;
//! - the operator pin that refuses a rollback or a same-version replacement;
//! - the boundary reader that refuses to hand a V2 document to a V1 reader.

use std::collections::BTreeMap;

use crate::Error;
use crate::cbor;
use crate::fact::FactEnvelope;
use crate::hash::{Hash, KeyId};
use crate::hlc::Hlc;
use crate::roots::{RootSet, RootStatus};
use crate::sign::{
    DOMAIN_ISSUER_ENROLMENT, DOMAIN_ISSUER_REVOCATION, DOMAIN_ISSUER_ROTATION, PublicKey,
    SignDomain, Signature, Signer,
};
use crate::value::{PORTABLE_MAX, Value};

/// The only scope an owner root or an enrolled issuer has (spec 035 section 3.2).
pub const SCOPE_PLATFORM: &str = "platform";
/// The detached signature record's and the owner public record's schema.
pub const RECORD_SCHEMA_VERSION: i64 = 1;
/// The versioned root set's wire schema.
pub const ROOT_SET_SCHEMA_VERSION: i64 = 2;

const FACT_IDENTITY_CREATED: &str = "identity.created";
const FACT_KEY_ROTATED: &str = "identity.key_rotated";
const FACT_KEY_REVOKED: &str = "identity.key_revoked";

// ---------------------------------------------------------------------------
// Strict scalars
// ---------------------------------------------------------------------------

fn invalid(msg: impl Into<String>) -> Error {
    Error::Validation(msg.into())
}

/// Decode exactly `len` bytes from lowercase hex. Uppercase, odd or wrong
/// lengths and non-hex characters are refused. The message never echoes the
/// input, so a caller may use this on material it must not print.
pub fn lower_hex(s: &str, len: usize, what: &str) -> Result<Vec<u8>, Error> {
    if s.len() != len * 2 {
        return Err(invalid(format!(
            "{what} is not {} lowercase hex characters",
            len * 2
        )));
    }
    if !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(invalid(format!("{what} is not lowercase hex")));
    }
    hex::decode(s).map_err(|_| invalid(format!("{what} is not lowercase hex")))
}

/// A public key from 64 lowercase hex characters, refusing a byte string that
/// is not a valid Ed25519 point and a weak (small-order) key.
pub fn strict_public_key(s: &str) -> Result<PublicKey, Error> {
    let v = lower_hex(s, 32, "public key")?;
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    check_public_key(&PublicKey(a))?;
    Ok(PublicKey(a))
}

fn check_public_key(pk: &PublicKey) -> Result<(), Error> {
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk.0)
        .map_err(|_| invalid("public key is not a valid Ed25519 point"))?;
    if vk.is_weak() {
        return Err(invalid("public key is weak (small order)"));
    }
    Ok(())
}

/// A signature from 128 lowercase hex characters.
pub fn strict_signature(s: &str) -> Result<Signature, Error> {
    Signature::from_bytes(&lower_hex(s, 64, "signature")?)
}

/// A BLAKE3-256 hash or key id from 64 lowercase hex characters.
pub fn strict_hash(s: &str, what: &str) -> Result<Hash, Error> {
    Hash::from_bytes(&lower_hex(s, 32, what)?)
}

/// An HLC inside the range spec 035 section 3.2 admits: physical in
/// `0..=2^53-1` (the canonical decoder's portable range, which bounds the
/// proposal's `i64::MAX`), logical in `0..=u32::MAX`.
fn check_hlc(h: Hlc, what: &str) -> Result<(), Error> {
    if h.physical > PORTABLE_MAX as u64 {
        return Err(invalid(format!(
            "{what}.physical exceeds the portable range"
        )));
    }
    Ok(())
}

/// Exactly the keys named, no others; the required ones present.
fn exact_keys(
    m: &BTreeMap<String, Value>,
    required: &[&str],
    optional: &[&str],
    what: &str,
) -> Result<(), Error> {
    for k in required {
        if !m.contains_key(*k) {
            return Err(invalid(format!("{what} is missing {k:?}")));
        }
    }
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(invalid(format!("{what} has an unknown member {k:?}")));
        }
    }
    Ok(())
}

fn text<'a>(m: &'a BTreeMap<String, Value>, k: &str, what: &str) -> Result<&'a str, Error> {
    match m.get(k) {
        Some(Value::Text(t)) => Ok(t),
        _ => Err(invalid(format!("{what}.{k} is not text"))),
    }
}

/// An HLC from its canonical value form, with no member beyond the two.
fn strict_hlc(v: &Value, what: &str) -> Result<Hlc, Error> {
    let m = v
        .as_map()
        .ok_or_else(|| invalid(format!("{what} is not a map")))?;
    exact_keys(m, &["physical", "logical"], &[], what)?;
    let h = Hlc::from_value(v).map_err(|e| invalid(format!("{what}: {e}")))?;
    check_hlc(h, what)?;
    Ok(h)
}

/// The canonical bytes must re-encode to themselves.
fn decode_canonical(bytes: &[u8], what: &str) -> Result<Value, Error> {
    let v = cbor::decode(bytes)?;
    if cbor::encode(&v) != bytes {
        return Err(Error::Decode(format!("{what} is not canonical")));
    }
    Ok(v)
}

// ---------------------------------------------------------------------------
// Record kinds
// ---------------------------------------------------------------------------

/// The three kinds an owner root authorizes. The kind alone fixes the domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IssuerRecordKind {
    /// An issuer enrollment.
    Enrolment,
    /// An issuer key rotation.
    Rotation,
    /// An issuer key revocation.
    Revocation,
}

impl IssuerRecordKind {
    /// The fixed signing domain.
    pub fn domain(self) -> SignDomain {
        match self {
            IssuerRecordKind::Enrolment => DOMAIN_ISSUER_ENROLMENT,
            IssuerRecordKind::Rotation => DOMAIN_ISSUER_ROTATION,
            IssuerRecordKind::Revocation => DOMAIN_ISSUER_REVOCATION,
        }
    }

    /// The wire word.
    pub fn word(self) -> &'static str {
        match self {
            IssuerRecordKind::Enrolment => "enrolment",
            IssuerRecordKind::Rotation => "rotation",
            IssuerRecordKind::Revocation => "revocation",
        }
    }

    /// From the wire word; nothing else is accepted.
    pub fn parse(s: &str) -> Result<Self, Error> {
        Ok(match s {
            "enrolment" => IssuerRecordKind::Enrolment,
            "rotation" => IssuerRecordKind::Rotation,
            "revocation" => IssuerRecordKind::Revocation,
            _ => return Err(invalid("kind is not enrolment, rotation or revocation")),
        })
    }
}

// ---------------------------------------------------------------------------
// The issuer identity fact
// ---------------------------------------------------------------------------

/// The `identity.created` v1 fact for a Service issuer, byte for byte the
/// platform's construction (its spec 004 B-6).
pub fn identity_created_fact(public_key: &PublicKey, at: Hlc) -> FactEnvelope {
    let mut body: BTreeMap<String, Value> = BTreeMap::new();
    body.insert("publicKey".into(), Value::text(hex::encode(public_key.0)));
    body.insert("keyId".into(), Value::text(public_key.id().0.to_hex()));
    body.insert("kind".into(), Value::text("service"));
    body.insert("at".into(), at.to_value());
    FactEnvelope::new(FACT_IDENTITY_CREATED, 1, body)
}

/// A strictly decoded issuer identity: its exact original bytes and the
/// identity they establish, which is their envelope hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerIdentity {
    /// The identity: BLAKE3 of the exact original fact bytes.
    pub id: Hash,
    /// The issuer's online public key.
    pub public_key: PublicKey,
    /// The original timestamp, preserved.
    pub at: Hlc,
    /// The exact original bytes.
    pub bytes: Vec<u8>,
}

impl IssuerIdentity {
    /// Decode canonical `identity.created` v1 bytes, kind Service, whose key id
    /// is derived from its public key. Anything else is refused.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        decode_canonical(bytes, "identity")?;
        let f = FactEnvelope::decode(bytes)?;
        if f.kind != FACT_IDENTITY_CREATED || f.v != 1 || !f.extra.is_empty() {
            return Err(invalid("identity is not an identity.created v1 fact"));
        }
        exact_keys(
            &f.body,
            &["publicKey", "keyId", "kind", "at"],
            &[],
            "identity",
        )?;
        if text(&f.body, "kind", "identity")? != "service" {
            return Err(invalid("identity kind is not service"));
        }
        let public_key = strict_public_key(text(&f.body, "publicKey", "identity")?)?;
        let key_id = strict_hash(text(&f.body, "keyId", "identity")?, "identity.keyId")?;
        if key_id != public_key.id().0 {
            return Err(invalid("identity keyId is not derived from its public key"));
        }
        let at = strict_hlc(&f.body["at"], "identity.at")?;
        Ok(IssuerIdentity {
            id: Hash::of(bytes),
            public_key,
            at,
            bytes: bytes.to_vec(),
        })
    }
}

// ---------------------------------------------------------------------------
// The enrollment payload
// ---------------------------------------------------------------------------

/// The enrollment an owner root signs: the platform's existing canonical map,
/// snake_case keys, absent windows omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enrolment {
    /// BLAKE3 of the issuer's `identity.created` bytes.
    pub issuer_id: Hash,
    /// The online key enrolled.
    pub public_key: PublicKey,
    /// Always `platform`.
    pub scope: String,
    /// Inclusive start.
    pub not_before: Option<Hlc>,
    /// Inclusive end.
    pub not_after: Option<Hlc>,
}

impl Enrolment {
    /// The canonical value.
    pub fn to_value(&self) -> Value {
        let mut m = BTreeMap::new();
        m.insert("issuer_id".into(), Value::text(self.issuer_id.to_hex()));
        m.insert(
            "public_key".into(),
            Value::text(hex::encode(self.public_key.0)),
        );
        m.insert("scope".into(), Value::text(&self.scope));
        if let Some(nb) = self.not_before {
            m.insert("not_before".into(), nb.to_value());
        }
        if let Some(na) = self.not_after {
            m.insert("not_after".into(), na.to_value());
        }
        Value::Map(m)
    }

    /// The exact bytes the owner signs.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        cbor::encode(&self.to_value())
    }

    /// From the value form, strictly.
    pub fn from_value(v: &Value) -> Result<Self, Error> {
        let m = v
            .as_map()
            .ok_or_else(|| invalid("enrolment is not a map"))?;
        exact_keys(
            m,
            &["issuer_id", "public_key", "scope"],
            &["not_before", "not_after"],
            "enrolment",
        )?;
        let e = Enrolment {
            issuer_id: strict_hash(text(m, "issuer_id", "enrolment")?, "enrolment.issuer_id")?,
            public_key: strict_public_key(text(m, "public_key", "enrolment")?)?,
            scope: text(m, "scope", "enrolment")?.to_string(),
            not_before: m
                .get("not_before")
                .map(|v| strict_hlc(v, "enrolment.not_before"))
                .transpose()?,
            not_after: m
                .get("not_after")
                .map(|v| strict_hlc(v, "enrolment.not_after"))
                .transpose()?,
        };
        e.validate()?;
        Ok(e)
    }

    /// Decode the exact canonical bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_value(&decode_canonical(bytes, "enrolment")?)
    }

    /// The scope and window rules.
    pub fn validate(&self) -> Result<(), Error> {
        if self.scope != SCOPE_PLATFORM {
            return Err(invalid("enrolment scope is not platform"));
        }
        check_public_key(&self.public_key)?;
        if let (Some(nb), Some(na)) = (self.not_before, self.not_after)
            && nb > na
        {
            return Err(invalid("enrolment window is inverted"));
        }
        for (h, w) in [
            (self.not_before, "not_before"),
            (self.not_after, "not_after"),
        ] {
            if let Some(h) = h {
                check_hlc(h, w)?;
            }
        }
        Ok(())
    }

    /// Bind the enrollment to its identity fact: same identity, same key.
    pub fn check_identity(&self, identity: &IssuerIdentity) -> Result<(), Error> {
        if self.issuer_id != identity.id {
            return Err(invalid(
                "enrolment issuer_id is not the identity fact's hash",
            ));
        }
        if self.public_key != identity.public_key {
            return Err(invalid("enrolment public_key is not the identity's key"));
        }
        Ok(())
    }

    /// When the owner's authorization is judged: `not_before`, or the
    /// identity fact's `at` when the enrollment omits it.
    pub fn authorization_time(&self, identity: Option<&IssuerIdentity>) -> Result<Hlc, Error> {
        match (self.not_before, identity) {
            (Some(nb), _) => Ok(nb),
            (None, Some(i)) => Ok(i.at),
            (None, None) => Err(invalid(
                "enrolment omits not_before and no identity fact supplies its time",
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// The rotation and revocation facts
// ---------------------------------------------------------------------------

/// An `identity.key_rotated` v1 fact: the platform's body
/// `{identity, from, to, toKeyId, effective}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotation {
    /// The issuer identity.
    pub identity: Hash,
    /// The predecessor key id.
    pub from: KeyId,
    /// The successor public key.
    pub to: PublicKey,
    /// From when the successor signs.
    pub effective: Hlc,
}

impl Rotation {
    /// The fact envelope, byte for byte the platform's construction.
    pub fn to_fact(&self) -> FactEnvelope {
        let mut body: BTreeMap<String, Value> = BTreeMap::new();
        body.insert("identity".into(), Value::text(self.identity.to_hex()));
        body.insert("from".into(), Value::text(self.from.0.to_hex()));
        body.insert("to".into(), Value::text(hex::encode(self.to.0)));
        body.insert("toKeyId".into(), Value::text(self.to.id().0.to_hex()));
        body.insert("effective".into(), self.effective.to_value());
        FactEnvelope::new(FACT_KEY_ROTATED, 1, body)
    }

    /// The exact bytes the owner signs: the complete fact envelope.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.to_fact().canonical_bytes()
    }

    /// From the fact's value form, strictly.
    pub fn from_value(v: &Value) -> Result<Self, Error> {
        let body = fact_body(v, FACT_KEY_ROTATED)?;
        exact_keys(
            body,
            &["identity", "from", "to", "toKeyId", "effective"],
            &[],
            "rotation",
        )?;
        let to = strict_public_key(text(body, "to", "rotation")?)?;
        let to_key_id = strict_hash(text(body, "toKeyId", "rotation")?, "rotation.toKeyId")?;
        if to_key_id != to.id().0 {
            return Err(invalid("rotation toKeyId is not derived from its key"));
        }
        let from = KeyId(strict_hash(
            text(body, "from", "rotation")?,
            "rotation.from",
        )?);
        if from == to.id() {
            return Err(invalid("rotation successor is its predecessor"));
        }
        Ok(Rotation {
            identity: strict_hash(text(body, "identity", "rotation")?, "rotation.identity")?,
            from,
            to,
            effective: strict_hlc(&body["effective"], "rotation.effective")?,
        })
    }

    /// Decode the exact canonical fact bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_value(&decode_canonical(bytes, "rotation")?)
    }
}

/// An `identity.key_revoked` v1 fact: the platform's body
/// `{identity, key, since, reason}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revocation {
    /// The issuer identity.
    pub identity: Hash,
    /// The revoked key id.
    pub key: KeyId,
    /// The start of the compromise window, inclusive.
    pub since: Hlc,
    /// Why; nonempty.
    pub reason: String,
}

impl Revocation {
    /// The fact envelope, byte for byte the platform's construction.
    pub fn to_fact(&self) -> FactEnvelope {
        let mut body: BTreeMap<String, Value> = BTreeMap::new();
        body.insert("identity".into(), Value::text(self.identity.to_hex()));
        body.insert("key".into(), Value::text(self.key.0.to_hex()));
        body.insert("since".into(), self.since.to_value());
        body.insert("reason".into(), Value::text(&self.reason));
        FactEnvelope::new(FACT_KEY_REVOKED, 1, body)
    }

    /// The exact bytes the owner signs: the complete fact envelope.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.to_fact().canonical_bytes()
    }

    /// From the fact's value form, strictly.
    pub fn from_value(v: &Value) -> Result<Self, Error> {
        let body = fact_body(v, FACT_KEY_REVOKED)?;
        exact_keys(
            body,
            &["identity", "key", "since", "reason"],
            &[],
            "revocation",
        )?;
        let reason = text(body, "reason", "revocation")?.to_string();
        if reason.is_empty() {
            return Err(invalid("revocation reason is empty"));
        }
        Ok(Revocation {
            identity: strict_hash(text(body, "identity", "revocation")?, "revocation.identity")?,
            key: KeyId(strict_hash(
                text(body, "key", "revocation")?,
                "revocation.key",
            )?),
            since: strict_hlc(&body["since"], "revocation.since")?,
            reason,
        })
    }

    /// Decode the exact canonical fact bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_value(&decode_canonical(bytes, "revocation")?)
    }
}

fn fact_body<'a>(v: &'a Value, kind: &str) -> Result<&'a BTreeMap<String, Value>, Error> {
    let m = v.as_map().ok_or_else(|| invalid("fact is not a map"))?;
    exact_keys(m, &["kind", "v", "body"], &[], "fact")?;
    if m.get("kind").and_then(Value::as_text) != Some(kind) {
        return Err(invalid(format!("fact is not {kind}")));
    }
    if m.get("v") != Some(&Value::Int(1)) {
        return Err(invalid(format!("fact {kind} is not v1")));
    }
    m.get("body")
        .and_then(Value::as_map)
        .ok_or_else(|| invalid("fact body is not a map"))
}

/// A decoded payload of any of the three kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssuerPayload {
    /// An enrollment.
    Enrolment(Enrolment),
    /// A rotation.
    Rotation(Rotation),
    /// A revocation.
    Revocation(Revocation),
}

impl IssuerPayload {
    /// Decode exact canonical bytes as the given kind; the kind is never read
    /// from the bytes.
    pub fn decode(kind: IssuerRecordKind, bytes: &[u8]) -> Result<Self, Error> {
        Ok(match kind {
            IssuerRecordKind::Enrolment => IssuerPayload::Enrolment(Enrolment::decode(bytes)?),
            IssuerRecordKind::Rotation => IssuerPayload::Rotation(Rotation::decode(bytes)?),
            IssuerRecordKind::Revocation => IssuerPayload::Revocation(Revocation::decode(bytes)?),
        })
    }

    /// From the value form (the root set's JSON payload, converted).
    pub fn from_value(kind: IssuerRecordKind, v: &Value) -> Result<Self, Error> {
        Ok(match kind {
            IssuerRecordKind::Enrolment => IssuerPayload::Enrolment(Enrolment::from_value(v)?),
            IssuerRecordKind::Rotation => IssuerPayload::Rotation(Rotation::from_value(v)?),
            IssuerRecordKind::Revocation => IssuerPayload::Revocation(Revocation::from_value(v)?),
        })
    }

    /// Its kind.
    pub fn kind(&self) -> IssuerRecordKind {
        match self {
            IssuerPayload::Enrolment(_) => IssuerRecordKind::Enrolment,
            IssuerPayload::Rotation(_) => IssuerRecordKind::Rotation,
            IssuerPayload::Revocation(_) => IssuerRecordKind::Revocation,
        }
    }

    /// Its canonical value.
    pub fn to_value(&self) -> Value {
        match self {
            IssuerPayload::Enrolment(e) => e.to_value(),
            IssuerPayload::Rotation(r) => r.to_fact().to_value(),
            IssuerPayload::Revocation(r) => r.to_fact().to_value(),
        }
    }

    /// Its canonical bytes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        cbor::encode(&self.to_value())
    }

    /// The identity the payload names.
    pub fn identity(&self) -> Hash {
        match self {
            IssuerPayload::Enrolment(e) => e.issuer_id,
            IssuerPayload::Rotation(r) => r.identity,
            IssuerPayload::Revocation(r) => r.identity,
        }
    }

    /// The key the payload is about: the enrolled key, the successor, or the
    /// revoked key.
    pub fn subject_key(&self) -> KeyId {
        match self {
            IssuerPayload::Enrolment(e) => e.public_key.id(),
            IssuerPayload::Rotation(r) => r.to.id(),
            IssuerPayload::Revocation(r) => r.key,
        }
    }

    /// Bind the payload to the identity fact it names.
    pub fn check_identity(&self, identity: &IssuerIdentity) -> Result<(), Error> {
        match self {
            IssuerPayload::Enrolment(e) => e.check_identity(identity),
            _ if self.identity() != identity.id => {
                Err(invalid("payload identity is not the identity fact's hash"))
            }
            _ => Ok(()),
        }
    }

    /// The decoded public fields, for inspection and reports.
    pub fn to_json(&self) -> serde_json::Value {
        self.to_value().to_json()
    }
}

// ---------------------------------------------------------------------------
// Strict JSON
// ---------------------------------------------------------------------------

type JsonMap = serde_json::Map<String, serde_json::Value>;

/// Parse JSON bytes strictly: no duplicate member at any depth, no
/// non-integer or non-portable number, no byte-order mark.
pub fn strict_json(bytes: &[u8], what: &str) -> Result<serde_json::Value, Error> {
    crate::portable::scan(bytes)
        .map_err(|e| invalid(format!("{what} is ambiguous JSON: {e:?}")))?;
    serde_json::from_slice(bytes).map_err(|e| invalid(format!("{what} is not JSON: {e}")))
}

fn jobj<'a>(
    v: &'a serde_json::Value,
    required: &[&str],
    optional: &[&str],
    what: &str,
) -> Result<&'a JsonMap, Error> {
    let m = v
        .as_object()
        .ok_or_else(|| invalid(format!("{what} is not an object")))?;
    for k in required {
        if !m.contains_key(*k) {
            return Err(invalid(format!("{what} is missing {k:?}")));
        }
    }
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(invalid(format!("{what} has an unknown member {k:?}")));
        }
    }
    Ok(m)
}

fn jstr<'a>(m: &'a JsonMap, k: &str, what: &str) -> Result<&'a str, Error> {
    m.get(k)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid(format!("{what}.{k} is not a string")))
}

fn jint(m: &JsonMap, k: &str, what: &str) -> Result<i64, Error> {
    m.get(k)
        .and_then(serde_json::Value::as_i64)
        .filter(|i| *i >= 0)
        .ok_or_else(|| invalid(format!("{what}.{k} is not a nonnegative integer")))
}

fn jhlc(v: &serde_json::Value, what: &str) -> Result<Hlc, Error> {
    strict_hlc(&Value::from_json(v)?, what)
}

fn to_line(v: &serde_json::Value) -> Vec<u8> {
    let mut out = serde_json::to_vec_pretty(v).expect("json serializes");
    out.push(b'\n');
    out
}

// ---------------------------------------------------------------------------
// The owner root's public record
// ---------------------------------------------------------------------------

/// The generator's public-only record: exactly
/// `{schema_version: 1, public_key, key_id, scope: "platform"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerRootPublic {
    /// The owner's public key.
    pub public_key: PublicKey,
}

impl OwnerRootPublic {
    /// The record for a signer's public key.
    pub fn of(signer: &Signer) -> Self {
        OwnerRootPublic {
            public_key: signer.public(),
        }
    }

    /// The key id: BLAKE3 of the raw public-key bytes.
    pub fn key_id(&self) -> KeyId {
        self.public_key.id()
    }

    /// The JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": RECORD_SCHEMA_VERSION,
            "public_key": hex::encode(self.public_key.0),
            "key_id": self.key_id().0.to_hex(),
            "scope": SCOPE_PLATFORM,
        })
    }

    /// The bytes written: pretty JSON and one newline.
    pub fn to_bytes(&self) -> Vec<u8> {
        to_line(&self.to_json())
    }

    /// Parse strictly; the key id must be derived from the public key.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let v = strict_json(bytes, "owner public record")?;
        let m = jobj(
            &v,
            &["schema_version", "public_key", "key_id", "scope"],
            &[],
            "owner public record",
        )?;
        if jint(m, "schema_version", "owner public record")? != RECORD_SCHEMA_VERSION {
            return Err(invalid("owner public record schema_version is not 1"));
        }
        if jstr(m, "scope", "owner public record")? != SCOPE_PLATFORM {
            return Err(invalid("owner public record scope is not platform"));
        }
        let public_key = strict_public_key(jstr(m, "public_key", "owner public record")?)?;
        let key_id = strict_hash(jstr(m, "key_id", "owner public record")?, "key_id")?;
        if key_id != public_key.id().0 {
            return Err(invalid(
                "owner public record key_id is not derived from its key",
            ));
        }
        Ok(OwnerRootPublic { public_key })
    }
}

// ---------------------------------------------------------------------------
// The detached signature record
// ---------------------------------------------------------------------------

/// The detached signature: exactly `{schema_version: 1, kind, root_key_id,
/// payload_digest, signature}`. `root_key_id` and `payload_digest` are checked
/// selectors; they grant no authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetachedSignature {
    /// Which kind, and therefore which domain.
    pub kind: IssuerRecordKind,
    /// The owner root that signed.
    pub root_key_id: KeyId,
    /// BLAKE3 of the exact canonical payload bytes.
    pub payload_digest: Hash,
    /// The signature over the family preimage of the payload.
    pub signature: Signature,
}

impl DetachedSignature {
    /// Sign a payload as the given kind. The caller has validated the payload
    /// and obtained the owner's confirmation; this only signs.
    pub fn sign(signer: &Signer, kind: IssuerRecordKind, payload: &[u8]) -> Self {
        DetachedSignature {
            kind,
            root_key_id: signer.public().id(),
            payload_digest: Hash::of(payload),
            signature: signer.sign(kind.domain(), payload),
        }
    }

    /// The JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": RECORD_SCHEMA_VERSION,
            "kind": self.kind.word(),
            "root_key_id": self.root_key_id.0.to_hex(),
            "payload_digest": self.payload_digest.to_hex(),
            "signature": hex::encode(self.signature.0),
        })
    }

    /// The bytes written: pretty JSON and one newline.
    pub fn to_bytes(&self) -> Vec<u8> {
        to_line(&self.to_json())
    }

    /// Parse strictly.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let v = strict_json(bytes, "signature record")?;
        let m = jobj(
            &v,
            &[
                "schema_version",
                "kind",
                "root_key_id",
                "payload_digest",
                "signature",
            ],
            &[],
            "signature record",
        )?;
        if jint(m, "schema_version", "signature record")? != RECORD_SCHEMA_VERSION {
            return Err(invalid("signature record schema_version is not 1"));
        }
        Ok(DetachedSignature {
            kind: IssuerRecordKind::parse(jstr(m, "kind", "signature record")?)?,
            root_key_id: KeyId(strict_hash(
                jstr(m, "root_key_id", "signature record")?,
                "root_key_id",
            )?),
            payload_digest: strict_hash(
                jstr(m, "payload_digest", "signature record")?,
                "payload_digest",
            )?,
            signature: strict_signature(jstr(m, "signature", "signature record")?)?,
        })
    }

    /// Verify against an independently pinned owner key, as the expected kind,
    /// over the exact payload bytes. Selectors are checked first; the
    /// signature covers the full payload through the fixed domain.
    pub fn verify(
        &self,
        root: &PublicKey,
        kind: IssuerRecordKind,
        payload: &[u8],
    ) -> Result<(), Error> {
        if self.kind != kind {
            return Err(Error::Crypto(format!(
                "signature is for {}, not {}",
                self.kind.word(),
                kind.word()
            )));
        }
        if self.root_key_id != root.id() {
            return Err(Error::Crypto("signature names a different root".into()));
        }
        if self.payload_digest != Hash::of(payload) {
            return Err(Error::Crypto("payload digest does not match".into()));
        }
        check_public_key(root).map_err(|e| Error::Crypto(e.to_string()))?;
        root.verify_strict(kind.domain(), payload, &self.signature)
    }
}

/// The detached check `issuer-verify` performs without a history: strict
/// decoding of the payload and the identity, their binding, and the owner's
/// signature under the kind's fixed domain. No eligibility, no rollback.
pub fn verify_detached(
    root: &OwnerRootPublic,
    kind: IssuerRecordKind,
    payload: &[u8],
    identity: &[u8],
    signature: &DetachedSignature,
) -> Result<IssuerPayload, Error> {
    let decoded = IssuerPayload::decode(kind, payload)?;
    let identity = IssuerIdentity::decode(identity)?;
    decoded.check_identity(&identity)?;
    if let IssuerPayload::Enrolment(e) = &decoded {
        e.authorization_time(Some(&identity))?;
    }
    signature.verify(&root.public_key, kind, payload)?;
    Ok(decoded)
}

// ---------------------------------------------------------------------------
// RootSetV2
// ---------------------------------------------------------------------------

/// One owner root in a V2 set: an enrollment authority, never a direct signer
/// of entries or attestations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerRoot {
    /// Its public key.
    pub public_key: PublicKey,
    /// Active, revoked or excluded.
    pub status: RootStatus,
    /// Inclusive start.
    pub not_before: Option<Hlc>,
    /// Inclusive end.
    pub not_after: Option<Hlc>,
    /// For a revoked root, the start of its compromise window.
    pub since: Option<Hlc>,
}

impl OwnerRoot {
    /// Its key id.
    pub fn key_id(&self) -> KeyId {
        self.public_key.id()
    }

    fn to_value(&self) -> Value {
        let mut m = BTreeMap::new();
        m.insert("key_id".into(), Value::text(self.key_id().0.to_hex()));
        m.insert(
            "public_key".into(),
            Value::text(hex::encode(self.public_key.0)),
        );
        m.insert("scope".into(), Value::text(SCOPE_PLATFORM));
        m.insert(
            "status".into(),
            Value::text(match self.status {
                RootStatus::Active => "active",
                RootStatus::Revoked => "revoked",
                RootStatus::Excluded => "excluded",
            }),
        );
        for (k, h) in [
            ("not_before", self.not_before),
            ("not_after", self.not_after),
            ("since", self.since),
        ] {
            if let Some(h) = h {
                m.insert(k.into(), h.to_value());
            }
        }
        Value::Map(m)
    }

    /// Whether this root may authorize an event at `at`.
    pub fn authorizes_at(&self, at: Hlc) -> Result<(), &'static str> {
        match self.status {
            RootStatus::Excluded => return Err("root-excluded"),
            RootStatus::Revoked if at >= self.since.unwrap_or_default() => {
                return Err("root-revoked");
            }
            _ => {}
        }
        if let Some(nb) = self.not_before
            && at < nb
        {
            return Err("root-not-yet-effective");
        }
        if let Some(na) = self.not_after
            && at > na
        {
            return Err("root-expired");
        }
        Ok(())
    }
}

/// One detached root authorization stored in the set: `{root_key_id,
/// payload, signature}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorization {
    /// The authorizing root.
    pub root_key_id: KeyId,
    /// The decoded payload.
    pub payload: IssuerPayload,
    /// The root's signature over the payload's canonical bytes.
    pub signature: Signature,
}

impl Authorization {
    fn to_value(&self) -> Value {
        let mut m = BTreeMap::new();
        m.insert(
            "root_key_id".into(),
            Value::text(self.root_key_id.0.to_hex()),
        );
        m.insert("payload".into(), self.payload.to_value());
        m.insert(
            "signature".into(),
            Value::text(hex::encode(self.signature.0)),
        );
        Value::Map(m)
    }

    /// As a detached signature record over the canonical payload.
    pub fn detached(&self) -> DetachedSignature {
        DetachedSignature {
            kind: self.payload.kind(),
            root_key_id: self.root_key_id,
            payload_digest: Hash::of(&self.payload.canonical_bytes()),
            signature: self.signature,
        }
    }
}

/// The versioned, pinned root set. Its members are exactly `schema_version`
/// (2), `root_set_version`, `origin` (`pinned`), `owner_roots`, `enrolments`,
/// `rotations` and `revocations`. It is operator configuration, not a
/// root-signed document; `signed_by` does not exist here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSetV2 {
    /// The monotonic operator revision.
    pub root_set_version: u64,
    /// The owner roots.
    pub owner_roots: Vec<OwnerRoot>,
    /// Enrollment authorizations.
    pub enrolments: Vec<Authorization>,
    /// Rotation authorizations.
    pub rotations: Vec<Authorization>,
    /// Revocation authorizations.
    pub revocations: Vec<Authorization>,
}

impl RootSetV2 {
    /// Parse strictly. Unknown or duplicate members, an unsupported schema, an
    /// origin other than `pinned` and a null optional field are refused.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let v = strict_json(bytes, "root set")?;
        Self::from_json(&v)
    }

    fn from_json(v: &serde_json::Value) -> Result<Self, Error> {
        let m = jobj(
            v,
            &[
                "schema_version",
                "root_set_version",
                "origin",
                "owner_roots",
                "enrolments",
                "rotations",
                "revocations",
            ],
            &[],
            "root set",
        )?;
        if jint(m, "schema_version", "root set")? != ROOT_SET_SCHEMA_VERSION {
            return Err(invalid("root set schema_version is not 2"));
        }
        if jstr(m, "origin", "root set")? != "pinned" {
            return Err(invalid("root set origin is not pinned"));
        }
        let root_set_version = jint(m, "root_set_version", "root set")? as u64;
        let arr = |k: &str| -> Result<&Vec<serde_json::Value>, Error> {
            m[k].as_array()
                .ok_or_else(|| invalid(format!("root set {k} is not an array")))
        };
        let mut owner_roots = Vec::new();
        for r in arr("owner_roots")? {
            owner_roots.push(parse_owner_root(r)?);
        }
        let auths = |k: &str, kind: IssuerRecordKind| -> Result<Vec<Authorization>, Error> {
            arr(k)?
                .iter()
                .map(|a| parse_authorization(a, kind))
                .collect()
        };
        Ok(RootSetV2 {
            root_set_version,
            owner_roots,
            enrolments: auths("enrolments", IssuerRecordKind::Enrolment)?,
            rotations: auths("rotations", IssuerRecordKind::Rotation)?,
            revocations: auths("revocations", IssuerRecordKind::Revocation)?,
        })
    }

    /// The canonical value of the complete object, root authorizations included.
    pub fn to_value(&self) -> Value {
        let mut m = BTreeMap::new();
        m.insert("schema_version".into(), Value::Int(ROOT_SET_SCHEMA_VERSION));
        m.insert(
            "root_set_version".into(),
            Value::Int(self.root_set_version as i64),
        );
        m.insert("origin".into(), Value::text("pinned"));
        m.insert(
            "owner_roots".into(),
            Value::Array(self.owner_roots.iter().map(OwnerRoot::to_value).collect()),
        );
        for (k, list) in [
            ("enrolments", &self.enrolments),
            ("rotations", &self.rotations),
            ("revocations", &self.revocations),
        ] {
            m.insert(
                k.into(),
                Value::Array(list.iter().map(Authorization::to_value).collect()),
            );
        }
        Value::Map(m)
    }

    /// The JSON rendering, written with one trailing newline.
    pub fn to_bytes(&self) -> Vec<u8> {
        to_line(&self.to_value().to_json())
    }

    /// BLAKE3 of the canonical portable-value encoding of the complete object.
    pub fn digest(&self) -> Hash {
        Hash::of(&cbor::encode(&self.to_value()))
    }

    /// Verify the complete configuration: every root, every authorization's
    /// cryptography and authorization time, and the full issuer history. Any
    /// defect refuses the whole set; no partial trust is granted.
    pub fn verify(&self) -> Result<VerifiedRootSet, Error> {
        if self.root_set_version > PORTABLE_MAX as u64 {
            return Err(invalid("root_set_version exceeds the portable range"));
        }
        let mut roots: BTreeMap<KeyId, OwnerRoot> = BTreeMap::new();
        for r in &self.owner_roots {
            if roots.insert(r.key_id(), r.clone()).is_some() {
                return Err(invalid("duplicate owner root key id"));
            }
        }
        let mut v = VerifiedRootSet {
            root_set_version: self.root_set_version,
            digest: self.digest(),
            roots,
            issuers: BTreeMap::new(),
        };
        for a in &self.enrolments {
            let IssuerPayload::Enrolment(e) = &a.payload else {
                return Err(invalid("enrolments holds a payload of another kind"));
            };
            let at = e.authorization_time(None)?;
            v.authorize(a, at)?;
            v.admit_enrolment(e)?;
        }
        let mut rotations: Vec<&Rotation> = Vec::new();
        for a in &self.rotations {
            let IssuerPayload::Rotation(r) = &a.payload else {
                return Err(invalid("rotations holds a payload of another kind"));
            };
            v.authorize(a, r.effective)?;
            rotations.push(r);
        }
        // History is applied in effective order, so a set's array order is
        // not part of what it means.
        rotations.sort_by(|a, b| {
            (a.identity, a.effective, a.from).cmp(&(b.identity, b.effective, b.from))
        });
        for r in rotations {
            v.apply_rotation(r)?;
        }
        for a in &self.revocations {
            let IssuerPayload::Revocation(r) = &a.payload else {
                return Err(invalid("revocations holds a payload of another kind"));
            };
            v.authorize(a, r.since)?;
            v.apply_revocation(r)?;
        }
        Ok(v)
    }
}

fn parse_owner_root(v: &serde_json::Value) -> Result<OwnerRoot, Error> {
    let what = "owner root";
    let m = jobj(
        v,
        &["key_id", "public_key", "scope", "status"],
        &["not_before", "not_after", "since"],
        what,
    )?;
    if jstr(m, "scope", what)? != SCOPE_PLATFORM {
        return Err(invalid("owner root scope is not platform"));
    }
    let public_key = strict_public_key(jstr(m, "public_key", what)?)?;
    let key_id = strict_hash(jstr(m, "key_id", what)?, "owner root key_id")?;
    if key_id != public_key.id().0 {
        return Err(invalid("owner root key_id is not derived from its key"));
    }
    let status = match jstr(m, "status", what)? {
        "active" => RootStatus::Active,
        "revoked" => RootStatus::Revoked,
        "excluded" => RootStatus::Excluded,
        _ => {
            return Err(invalid(
                "owner root status is not active, revoked or excluded",
            ));
        }
    };
    let opt = |k: &str| -> Result<Option<Hlc>, Error> {
        m.get(k)
            .map(|h| jhlc(h, &format!("owner root {k}")))
            .transpose()
    };
    let r = OwnerRoot {
        public_key,
        status,
        not_before: opt("not_before")?,
        not_after: opt("not_after")?,
        since: opt("since")?,
    };
    if let (Some(nb), Some(na)) = (r.not_before, r.not_after)
        && nb > na
    {
        return Err(invalid("owner root window is inverted"));
    }
    match (r.status, r.since) {
        (RootStatus::Revoked, None) => Err(invalid("revoked owner root has no since")),
        (RootStatus::Active | RootStatus::Excluded, Some(_)) => {
            Err(invalid("only a revoked owner root carries since"))
        }
        _ => Ok(r),
    }
}

fn parse_authorization(
    v: &serde_json::Value,
    kind: IssuerRecordKind,
) -> Result<Authorization, Error> {
    let what = "authorization";
    let m = jobj(v, &["root_key_id", "payload", "signature"], &[], what)?;
    Ok(Authorization {
        root_key_id: KeyId(strict_hash(jstr(m, "root_key_id", what)?, "root_key_id")?),
        payload: IssuerPayload::from_value(kind, &Value::from_json(&m["payload"])?)?,
        signature: strict_signature(jstr(m, "signature", what)?)?,
    })
}

// ---------------------------------------------------------------------------
// Verified history and eligibility
// ---------------------------------------------------------------------------

/// One key's span in an issuer's history: eligible from `from` (inclusive)
/// until `until` (exclusive), before windows and revocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySpan {
    /// The key.
    pub key: PublicKey,
    /// Eligible from, inclusive.
    pub from: Hlc,
    /// Rotated out at, exclusive.
    pub until: Option<Hlc>,
}

/// One enrolled issuer's verified history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerHistory {
    /// The enrollment.
    pub enrolment: Enrolment,
    /// Its keys, in order.
    pub keys: Vec<KeySpan>,
    /// Revoked keys and the start of each compromise window.
    pub revoked: BTreeMap<KeyId, Hlc>,
}

impl IssuerHistory {
    fn head(&self) -> &KeySpan {
        self.keys.last().expect("an enrolled issuer has a key")
    }
}

/// A root set whose every authorization and history step verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRootSet {
    /// The operator revision.
    pub root_set_version: u64,
    /// The digest that names it in every verdict.
    pub digest: Hash,
    /// Owner roots by key id.
    pub roots: BTreeMap<KeyId, OwnerRoot>,
    /// Issuers by identity.
    pub issuers: BTreeMap<Hash, IssuerHistory>,
}

/// What the verified set says about an issuer key at a time and scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssuerEligibility {
    /// Eligible, for this identity.
    Eligible {
        /// The issuer identity.
        identity: Hash,
    },
    /// Positively refused, with why.
    Refused(&'static str),
    /// No pinned authorization names the key.
    Unknown,
}

impl IssuerEligibility {
    /// The dimension word.
    pub fn word(&self) -> &'static str {
        match self {
            IssuerEligibility::Eligible { .. } => "pass",
            IssuerEligibility::Refused(_) => "fail",
            IssuerEligibility::Unknown => "unknown",
        }
    }
}

impl VerifiedRootSet {
    fn all_keys(&self) -> impl Iterator<Item = (Hash, &KeySpan)> {
        self.issuers
            .iter()
            .flat_map(|(id, h)| h.keys.iter().map(move |k| (*id, k)))
    }

    /// Check one root authorization: the root is pinned, may authorize at the
    /// event's time, and signed the canonical payload under the kind's domain.
    pub fn check_root(&self, root_key_id: &KeyId, at: Hlc) -> Result<&OwnerRoot, Error> {
        let root = self
            .roots
            .get(root_key_id)
            .ok_or_else(|| invalid("authorization names an owner root not in the set"))?;
        root.authorizes_at(at)
            .map_err(|why| invalid(format!("owner root cannot authorize: {why}")))?;
        Ok(root)
    }

    fn authorize(&self, a: &Authorization, at: Hlc) -> Result<(), Error> {
        let root = self.check_root(&a.root_key_id, at)?;
        root.public_key.verify_strict(
            a.payload.kind().domain(),
            &a.payload.canonical_bytes(),
            &a.signature,
        )
    }

    fn admit_enrolment(&mut self, e: &Enrolment) -> Result<(), Error> {
        if self.issuers.contains_key(&e.issuer_id) {
            return Err(invalid("conflicting enrolments of one issuer"));
        }
        if self.all_keys().any(|(_, k)| k.key == e.public_key) {
            return Err(invalid("a key is enrolled more than once"));
        }
        let from = e.authorization_time(None)?;
        self.issuers.insert(
            e.issuer_id,
            IssuerHistory {
                enrolment: e.clone(),
                keys: vec![KeySpan {
                    key: e.public_key,
                    from,
                    until: None,
                }],
                revoked: BTreeMap::new(),
            },
        );
        Ok(())
    }

    /// Whether a rotation would extend the history: a pinned, authorizing
    /// root, the current head key as predecessor, a fresh successor and an
    /// effective time strictly after the head became effective.
    pub fn check_rotation(&self, root_key_id: &KeyId, r: &Rotation) -> Result<(), Error> {
        self.check_root(root_key_id, r.effective)?;
        self.rotation_step(r)
    }

    fn rotation_step(&self, r: &Rotation) -> Result<(), Error> {
        let h = self
            .issuers
            .get(&r.identity)
            .ok_or_else(|| invalid("rotation names an issuer with no enrolment"))?;
        let head = h.head();
        if head.key.id() != r.from {
            return Err(invalid(
                "rotation predecessor is not the issuer's current key (fork, duplicate or wrong predecessor)",
            ));
        }
        if r.effective <= head.from {
            return Err(invalid(
                "rotation effective time is not after its predecessor's",
            ));
        }
        if self.all_keys().any(|(_, k)| k.key == r.to) {
            return Err(invalid(
                "rotation successor is already a key (cycle or cross-identity)",
            ));
        }
        Ok(())
    }

    fn apply_rotation(&mut self, r: &Rotation) -> Result<(), Error> {
        self.rotation_step(r)?;
        let h = self.issuers.get_mut(&r.identity).expect("checked");
        h.keys.last_mut().expect("nonempty").until = Some(r.effective);
        h.keys.push(KeySpan {
            key: r.to,
            from: r.effective,
            until: None,
        });
        Ok(())
    }

    /// Whether a revocation would extend the history: a pinned, authorizing
    /// root, a key of that issuer's approved history, not already revoked.
    pub fn check_revocation(&self, root_key_id: &KeyId, r: &Revocation) -> Result<(), Error> {
        self.check_root(root_key_id, r.since)?;
        self.revocation_step(r)
    }

    fn revocation_step(&self, r: &Revocation) -> Result<(), Error> {
        let h = self
            .issuers
            .get(&r.identity)
            .ok_or_else(|| invalid("revocation names an issuer with no enrolment"))?;
        if !h.keys.iter().any(|k| k.key.id() == r.key) {
            return Err(invalid(
                "revocation names a key outside the issuer's history",
            ));
        }
        if h.revoked.contains_key(&r.key) {
            return Err(invalid("conflicting revocations of one key"));
        }
        Ok(())
    }

    fn apply_revocation(&mut self, r: &Revocation) -> Result<(), Error> {
        self.revocation_step(r)?;
        self.issuers
            .get_mut(&r.identity)
            .expect("checked")
            .revoked
            .insert(r.key, r.since);
        Ok(())
    }

    /// Issuer eligibility of a key at an explicit evidence time and scope.
    /// No clock is read and no current time is substituted.
    pub fn eligibility(&self, key: &KeyId, at: Hlc, scope: &str) -> IssuerEligibility {
        let Some((identity, span)) = self.all_keys().find(|(_, k)| k.key.id() == *key) else {
            return IssuerEligibility::Unknown;
        };
        let h = &self.issuers[&identity];
        if scope != h.enrolment.scope {
            return IssuerEligibility::Refused("scope-mismatch");
        }
        if let Some(since) = h.revoked.get(key)
            && at >= *since
        {
            return IssuerEligibility::Refused("key-revoked");
        }
        if let Some(nb) = h.enrolment.not_before
            && at < nb
        {
            return IssuerEligibility::Refused("enrolment-not-yet-effective");
        }
        if let Some(na) = h.enrolment.not_after
            && at > na
        {
            return IssuerEligibility::Refused("enrolment-expired");
        }
        if at < span.from {
            return IssuerEligibility::Refused("key-not-yet-effective");
        }
        if let Some(until) = span.until
            && at >= until
        {
            return IssuerEligibility::Refused("key-rotated-out");
        }
        IssuerEligibility::Eligible { identity }
    }

    /// Whether an independently pinned owner public record is one of the
    /// set's roots, with the same key.
    pub fn holds_root(&self, root: &OwnerRootPublic) -> bool {
        self.roots
            .get(&root.key_id())
            .is_some_and(|r| r.public_key == root.public_key)
    }
}

// ---------------------------------------------------------------------------
// The operator pin
// ---------------------------------------------------------------------------

/// The operator's pin of an exact root-set revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootSetPin {
    /// The pinned revision.
    pub root_set_version: u64,
    /// The pinned digest.
    pub digest: Hash,
}

/// What admitting a pinned set changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinTransition {
    /// The same revision and digest as before.
    Unchanged,
    /// A strictly newer revision; the old and new digests are what
    /// `trust.root-set-changed` records.
    Advanced {
        /// The previous pin, if there was one.
        from: Option<RootSetPin>,
        /// The new pin.
        to: RootSetPin,
    },
}

/// Admit a verified set under the operator's expected pin, against the pin
/// currently in force. A mismatch with the expected pin, a lower revision and
/// a same revision with a different digest are all refused. This, not any
/// number of valid signatures, is what refuses a rollback or an omitted
/// revocation.
pub fn admit_pinned(
    current: Option<&RootSetPin>,
    expected: &RootSetPin,
    candidate: &VerifiedRootSet,
) -> Result<PinTransition, Error> {
    if candidate.root_set_version != expected.root_set_version
        || candidate.digest != expected.digest
    {
        return Err(invalid("root set does not match the operator pin"));
    }
    match current {
        Some(c) if candidate.root_set_version < c.root_set_version => {
            Err(invalid("root set version rollback"))
        }
        Some(c) if candidate.root_set_version == c.root_set_version => {
            if candidate.digest == c.digest {
                Ok(PinTransition::Unchanged)
            } else {
                Err(invalid("same root set version with a different digest"))
            }
        }
        _ => Ok(PinTransition::Advanced {
            from: current.copied(),
            to: *expected,
        }),
    }
}

// ---------------------------------------------------------------------------
// The boundary reader
// ---------------------------------------------------------------------------

/// A root-set document read at the strict boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootSetDocument {
    /// A legacy V1 set: unsigned, verification-only. It can never establish a
    /// trusted V2 issuer, and its `signed_by` is not proof of anything.
    LegacyV1(RootSet),
    /// A V2 set.
    V2(RootSetV2),
}

impl RootSetDocument {
    /// Read a root-set document. Duplicate or unknown members are refused in
    /// both shapes; a `schema_version` other than 2 is refused; a document
    /// without one is read as V1, strictly.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let v = strict_json(bytes, "root set")?;
        let m = v
            .as_object()
            .ok_or_else(|| invalid("root set is not an object"))?;
        if m.contains_key("schema_version") {
            return Ok(RootSetDocument::V2(RootSetV2::from_json(&v)?));
        }
        jobj(
            &v,
            &["root_set_version", "roots", "signed_by", "origin"],
            &[],
            "legacy root set",
        )?;
        for r in m["roots"]
            .as_array()
            .ok_or_else(|| invalid("legacy root set roots is not an array"))?
        {
            jobj(
                r,
                &["key_id", "public_key", "scope", "status"],
                &["not_before", "not_after", "since"],
                "legacy root",
            )?;
        }
        let set: RootSet =
            serde_json::from_value(v).map_err(|e| invalid(format!("legacy root set: {e}")))?;
        Ok(RootSetDocument::LegacyV1(set))
    }

    /// The V2 set, if this is one. A legacy set answers `None`: it cannot
    /// establish a hosted trusted issuer.
    pub fn v2(&self) -> Option<&RootSetV2> {
        match self {
            RootSetDocument::V2(s) => Some(s),
            RootSetDocument::LegacyV1(_) => None,
        }
    }
}
