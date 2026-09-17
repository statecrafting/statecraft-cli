//! The one evidence primitive (hqgit 027 as amended; spec 003 B-10 to B-12).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::cbor::{decode, encode};
use crate::hash::{Cid, Hash, KeyId};
use crate::hlc::Hlc;
use crate::sign::{DOMAIN_ATTESTATION, PublicKey, Signature, Signer};
use crate::value::Value;

/// The kinds of principal (hqgit 010 B-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrincipalKind {
    /// A person.
    Human,
    /// An agent.
    Agent,
    /// A service, such as the platform issuer.
    Service,
    /// An organization.
    Org,
}

impl PrincipalKind {
    /// The wire word.
    pub fn as_str(self) -> &'static str {
        match self {
            PrincipalKind::Human => "human",
            PrincipalKind::Agent => "agent",
            PrincipalKind::Service => "service",
            PrincipalKind::Org => "org",
        }
    }
    fn parse(s: &str) -> Result<Self, Error> {
        Ok(match s {
            "human" => PrincipalKind::Human,
            "agent" => PrincipalKind::Agent,
            "service" => PrincipalKind::Service,
            "org" => PrincipalKind::Org,
            _ => return Err(Error::Validation(format!("unknown principal kind {s:?}"))),
        })
    }
}

/// A principal: kind plus an identifier text (an identity id or a principal record id).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Principal {
    /// The kind.
    pub kind: PrincipalKind,
    /// The identifier.
    pub id: String,
}

impl Principal {
    /// Build.
    pub fn new(kind: PrincipalKind, id: &str) -> Self {
        Principal {
            kind,
            id: id.to_string(),
        }
    }
    /// The canonical value.
    pub fn to_value(&self) -> Value {
        Value::map()
            .with("kind", Value::text(self.kind.as_str()))
            .unwrap()
            .with("id", Value::text(&self.id))
            .unwrap()
    }
    /// From the canonical value.
    pub fn from_value(v: &Value) -> Result<Self, Error> {
        let m = v
            .as_map()
            .ok_or_else(|| Error::Validation("principal is not a map".into()))?;
        let kind = PrincipalKind::parse(
            m.get("kind")
                .and_then(Value::as_text)
                .ok_or_else(|| Error::Validation("principal.kind missing".into()))?,
        )?;
        let id = m
            .get("id")
            .and_then(Value::as_text)
            .ok_or_else(|| Error::Validation("principal.id missing".into()))?
            .to_string();
        Ok(Principal { kind, id })
    }
}

/// A predicate type, matching `^[a-z0-9-]+(/[a-z0-9-]+)*/v[0-9]+$`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PredicateType(pub String);

impl PredicateType {
    /// Validate and build.
    pub fn new(s: &str) -> Result<Self, Error> {
        let ok = (|| {
            let (path, ver) = s.rsplit_once('/')?;
            if !ver.starts_with('v')
                || ver.len() < 2
                || !ver[1..].bytes().all(|b| b.is_ascii_digit())
            {
                return None;
            }
            for seg in path.split('/') {
                if seg.is_empty()
                    || !seg
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                {
                    return None;
                }
            }
            Some(())
        })()
        .is_some();
        if !ok {
            return Err(Error::Validation(format!(
                "predicate {s:?} does not match the grammar"
            )));
        }
        Ok(PredicateType(s.to_string()))
    }
}

/// The first-slice predicates (spec 003 B-12).
pub const ARTIFACT: &str = "statecraft/artifact/v1";
/// The platform's verdict over an artifact.
pub const EVIDENCE_VERDICT: &str = "statecraft/evidence-verdict/v1";
/// The replayable admission result.
pub const POLICY_EVAL: &str = "statecraft/policy-eval/v1";
/// Approval, and nothing else.
pub const APPROVAL: &str = "statecraft/approval/v1";
/// A request for changes, its own predicate.
pub const CHANGE_REQUEST: &str = "statecraft/change-request/v1";

/// The hash of an attestation's canonical bytes, signature included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AttestationId(pub Hash);

const KNOWN_KEYS: [&str; 7] = [
    "at",
    "claim",
    "issuer",
    "issuer_key",
    "predicate",
    "sig",
    "subject",
];

/// An attestation before signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedAttestation {
    /// What it is about: a native hash (a revision id, an attestation id).
    pub subject: Hash,
    /// The predicate.
    pub predicate: PredicateType,
    /// Who issued it.
    pub issuer: Principal,
    /// The claim object's identifier (a `DagCbor` object).
    pub claim: Cid,
    /// The issuer's clock.
    pub at: Hlc,
    /// Unknown fields, preserved and hashed.
    pub extra: BTreeMap<String, Value>,
}

/// A signed attestation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    /// The unsigned fields.
    pub body: UnsignedAttestation,
    /// The signing key's id, set from the signer.
    pub issuer_key: KeyId,
    /// The signature.
    pub sig: Signature,
}

impl UnsignedAttestation {
    fn to_value(&self, issuer_key: &KeyId, sig: Option<&Signature>) -> Value {
        let mut m = self.extra.clone();
        m.insert("at".into(), self.at.to_value());
        m.insert("claim".into(), Value::Link(self.claim));
        m.insert("issuer".into(), self.issuer.to_value());
        m.insert("issuer_key".into(), Value::Bytes(issuer_key.0.0.to_vec()));
        m.insert("predicate".into(), Value::Text(self.predicate.0.clone()));
        m.insert("subject".into(), Value::Bytes(self.subject.0.to_vec()));
        if let Some(s) = sig {
            m.insert("sig".into(), Value::Bytes(s.0.to_vec()));
        }
        Value::Map(m)
    }

    /// The only constructor of a signed attestation (hqgit 027 B-3).
    pub fn issue(self, signer: &Signer) -> Result<Attestation, Error> {
        for k in self.extra.keys() {
            if KNOWN_KEYS.contains(&k.as_str()) {
                return Err(Error::Validation(format!(
                    "extra key {k:?} collides with a known key"
                )));
            }
        }
        let issuer_key = signer.public().id();
        let preimage = encode(&self.to_value(&issuer_key, None));
        let sig = signer.sign(DOMAIN_ATTESTATION, &preimage);
        Ok(Attestation {
            body: self,
            issuer_key,
            sig,
        })
    }
}

impl Attestation {
    /// The signing preimage body.
    pub fn unsigned_bytes(&self) -> Vec<u8> {
        encode(&self.body.to_value(&self.issuer_key, None))
    }
    /// Canonical bytes, signature included.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        encode(&self.body.to_value(&self.issuer_key, Some(&self.sig)))
    }
    /// The id: hash of the canonical bytes with `sig`.
    pub fn id(&self) -> AttestationId {
        AttestationId(Hash::of(&self.canonical_bytes()))
    }
    /// The `DagCbor` object identifier it is stored under; its hash equals the id.
    pub fn object_cid(&self) -> Cid {
        Cid {
            codec: crate::hash::Codec::DagCbor,
            hash: self.id().0,
        }
    }
    /// Verify the signature against a public key.
    pub fn verify_signature(&self, key: &PublicKey) -> Result<(), Error> {
        if key.id() != self.issuer_key {
            return Err(Error::Crypto(
                "key is not the attestation's issuer key".into(),
            ));
        }
        key.verify(DOMAIN_ATTESTATION, &self.unsigned_bytes(), &self.sig)
    }
    /// Decode from canonical bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let v = decode(bytes)?;
        let m = v
            .as_map()
            .ok_or_else(|| Error::Validation("attestation is not a map".into()))?;
        let mut extra = m.clone();
        let at = Hlc::from_value(
            &extra
                .remove("at")
                .ok_or_else(|| Error::Validation("missing at".into()))?,
        )?;
        let claim = match extra.remove("claim") {
            Some(Value::Link(c)) => c,
            _ => return Err(Error::Validation("missing claim link".into())),
        };
        let issuer = Principal::from_value(
            &extra
                .remove("issuer")
                .ok_or_else(|| Error::Validation("missing issuer".into()))?,
        )?;
        let issuer_key = match extra.remove("issuer_key") {
            Some(Value::Bytes(b)) => KeyId(Hash::from_bytes(&b)?),
            _ => return Err(Error::Validation("missing issuer_key".into())),
        };
        let predicate = PredicateType::new(
            extra
                .remove("predicate")
                .as_ref()
                .and_then(Value::as_text)
                .ok_or_else(|| Error::Validation("missing predicate".into()))?,
        )?;
        let subject = match extra.remove("subject") {
            Some(Value::Bytes(b)) => Hash::from_bytes(&b)?,
            _ => return Err(Error::Validation("missing subject".into())),
        };
        let sig = match extra.remove("sig") {
            Some(Value::Bytes(b)) => Signature::from_bytes(&b)?,
            _ => return Err(Error::Validation("missing sig".into())),
        };
        let a = Attestation {
            body: UnsignedAttestation {
                subject,
                predicate,
                issuer,
                claim,
                at,
                extra,
            },
            issuer_key,
            sig,
        };
        if a.canonical_bytes() != bytes {
            return Err(Error::Validation(
                "bytes are not the canonical encoding of the decoded attestation".into(),
            ));
        }
        Ok(a)
    }
}

/// A claim validator for a registered predicate.
pub trait ClaimValidator {
    /// Refuse a claim that does not fit the predicate.
    fn validate(&self, claim: &Value) -> Result<(), Error>;
}

/// What a claim check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimVerdict {
    /// Registered and valid.
    Valid,
    /// Registered and invalid.
    Invalid(String),
    /// Not registered: preserved, signature-verified, never rejected.
    Unregistered,
}

/// The predicate registry.
#[derive(Default)]
pub struct PredicateRegistry {
    validators: BTreeMap<String, Box<dyn ClaimValidator>>,
}

impl PredicateRegistry {
    /// Register.
    pub fn register(&mut self, predicate: &str, v: Box<dyn ClaimValidator>) {
        self.validators.insert(predicate.to_string(), v);
    }
    /// Whether a predicate is registered.
    pub fn knows(&self, p: &PredicateType) -> bool {
        self.validators.contains_key(&p.0)
    }
    /// Check a claim.
    pub fn verify_claim(&self, p: &PredicateType, claim: &Value) -> ClaimVerdict {
        match self.validators.get(&p.0) {
            None => ClaimVerdict::Unregistered,
            Some(v) => match v.validate(claim) {
                Ok(()) => ClaimVerdict::Valid,
                Err(e) => ClaimVerdict::Invalid(e.to_string()),
            },
        }
    }
    /// The first-slice predicates with their validators.
    pub fn first_slice() -> Self {
        let mut r = PredicateRegistry::default();
        r.register(
            ARTIFACT,
            Box::new(Keys {
                required: &["reference", "submitted_by", "producer", "producer_claims"],
                forbidden: &[],
            }),
        );
        r.register(
            EVIDENCE_VERDICT,
            Box::new(Keys {
                required: &[
                    "evidence",
                    "verifier",
                    "verified_under",
                    "integrity",
                    "signature",
                    "issuer_trust",
                    "subject_binding",
                    "coverage",
                ],
                forbidden: &["ok", "valid", "verified", "trusted"],
            }),
        );
        r.register(
            POLICY_EVAL,
            Box::new(Keys {
                required: &["policy", "inputs", "decision", "reasons"],
                forbidden: &[],
            }),
        );
        r.register(
            APPROVAL,
            Box::new(Keys {
                required: &["revision", "reviewer", "basis"],
                forbidden: &["decision", "verdict"],
            }),
        );
        r.register(
            CHANGE_REQUEST,
            Box::new(Keys {
                required: &["revision", "reviewer", "basis"],
                forbidden: &["decision", "verdict"],
            }),
        );
        r
    }
}

/// Required and forbidden keys on a map claim.
pub struct Keys {
    /// Must be present.
    pub required: &'static [&'static str],
    /// Must be absent: an approval carrying a `verdict` is not an approval.
    pub forbidden: &'static [&'static str],
}

impl ClaimValidator for Keys {
    fn validate(&self, claim: &Value) -> Result<(), Error> {
        let m = claim
            .as_map()
            .ok_or_else(|| Error::Validation("claim is not a map".into()))?;
        for k in self.required {
            if !m.contains_key(*k) {
                return Err(Error::Validation(format!("claim missing {k:?}")));
            }
        }
        for k in self.forbidden {
            if m.contains_key(*k) {
                return Err(Error::Validation(format!(
                    "claim carries forbidden key {k:?}"
                )));
            }
        }
        Ok(())
    }
}
