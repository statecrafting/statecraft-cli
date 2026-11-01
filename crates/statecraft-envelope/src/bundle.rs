//! The decision bundle reader (statecraft-cli spec 038 section 3.6).
//!
//! Written from the platform's format description (its spec 005 B-4 to B-6),
//! never from its code: the bundle is `{ revision, entries, objects,
//! tombstones, root_set, admission, decisions }` with the entries and objects
//! as the stored canonical bytes, never re-encoded.
//!
//! Pure, like the rest of this crate: no file, clock, entropy or network
//! access. The caller supplies the bytes, the operator's independently pinned
//! [`VerifiedRootSet`] and, when it has one, the pinned admission policy. The
//! bundle's own root set, the cell's answer and anything a repository carries
//! are never trusted in its place.
//!
//! **What is checked, in order.** The root set the bundle was judged under
//! against the pinned one; every entry's decoding, parent link, clock and
//! signature, the signing key resolved from the pinned set and eligible at the
//! entry's time in the given scope; every object's identifier against its
//! bytes; every entry payload and every attestation and claim it links to,
//! resolved, with a tombstoned object told apart from a missing one; the
//! revision id against the hash of its `revision.registered` fact; each
//! attestation's signature; the decisions and the admission named; and the
//! admission re-evaluated under the pinned policy. The first divergence is
//! reported; nothing after it is trusted.
//!
//! One limit is the format's: the first entry's parent lies outside the
//! bundle, so its parent link cannot be checked from the bundle alone.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::Error;
use crate::admission::{AdmissionInput, Decision, evaluate, policy_eval_claim};
use crate::attestation::{Attestation, AttestationId, Principal, PrincipalKind};
use crate::dimensions::{Admission, AdmissionPolicy};
use crate::entry::Entry;
use crate::fact::{FactEnvelope, Tombstone, TombstoneSet};
use crate::hash::{Cid, Codec, Hash};
use crate::issuer::{IssuerEligibility, VerifiedRootSet};
use crate::value::Value;
use crate::verdict::EvidenceVerdict;

/// The verdict predicate.
pub const EVIDENCE_VERDICT: &str = "statecraft/evidence-verdict/v1";
/// The admission predicate.
pub const POLICY_EVAL: &str = "statecraft/policy-eval/v1";
/// An approval.
pub const APPROVAL: &str = "statecraft/approval/v1";
/// A change request.
pub const CHANGE_REQUEST: &str = "statecraft/change-request/v1";

/// The bundle as the cell sends it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DecisionBundle {
    /// The revision id.
    pub revision: Hash,
    /// Entry bytes, in scope order from `revision.registered`.
    pub entries: Vec<Vec<u8>>,
    /// Every non-tombstoned object, by `<codec>:<hex>`.
    pub objects: BTreeMap<String, Vec<u8>>,
    /// The tombstone facts' canonical bytes.
    pub tombstones: Vec<Vec<u8>>,
    /// The root-set digest the cell judged under.
    pub root_set: Hash,
    /// The current admission attestation, or none.
    pub admission: Option<Hash>,
    /// The decision attestations.
    pub decisions: Vec<Hash>,
}

impl DecisionBundle {
    /// Parse the cell's JSON answer. Extra members (such as the cell's
    /// `admissionWord`) are ignored; a missing or mistyped one refuses.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        serde_json::from_slice(bytes)
            .map_err(|e| Error::Decode(format!("the decision bundle is malformed: {e}")))
    }
}

/// A decision named by the bundle, as resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedDecision {
    /// The attestation id.
    pub id: Hash,
    /// Whether it approves.
    pub approves: bool,
    /// The reviewer.
    pub reviewer: Principal,
}

/// What became of the admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionCheck {
    /// The bundle names no admission: the word `none`, never success.
    None,
    /// Re-evaluated under the pinned policy and in agreement.
    Reproduced {
        /// `admit` or `refuse`.
        decision: String,
    },
    /// Re-evaluated and in disagreement with what the cell recorded.
    Diverged {
        /// What the cell recorded.
        recorded: String,
        /// What the pinned evaluation says.
        reproduced: String,
    },
    /// It could not be re-evaluated, and why.
    NotReproduced(String),
}

/// The verification's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    /// The pinned root set's digest.
    pub pinned_root_set: Hash,
    /// The digest the bundle names.
    pub bundle_root_set: Hash,
    /// How many entries were read.
    pub entries: usize,
    /// The first divergence, if any; nothing after it is trusted.
    pub divergence: Option<String>,
    /// Objects the tombstones account for.
    pub tombstoned: Vec<String>,
    /// The decisions named.
    pub decisions: Vec<NamedDecision>,
    /// The admission.
    pub admission: AdmissionCheck,
}

impl Verification {
    /// Whether the bundle was judged under the pinned root set.
    pub fn root_set_matches(&self) -> bool {
        self.pinned_root_set == self.bundle_root_set
    }

    /// Exit 0 only when the bundle verifies and the reproduced admission is a
    /// pass (section 3.6 rule 4).
    pub fn passes(&self) -> bool {
        self.root_set_matches()
            && self.divergence.is_none()
            && matches!(&self.admission, AdmissionCheck::Reproduced { decision } if decision == "admit")
    }
}

struct Reader<'a> {
    bundle: &'a DecisionBundle,
    /// The operator's pinned set. `None` reads the bundle without verifying
    /// any signature, which only the admission dry-run (spec 049) does.
    pinned: Option<&'a VerifiedRootSet>,
    scope: &'a str,
    tombstones: TombstoneSet,
    tombstoned: BTreeSet<String>,
}

impl Reader<'_> {
    /// An object's bytes, checked against its identifier. `Ok(None)` is a
    /// tombstoned object; a missing one that no tombstone covers diverges.
    fn object(&mut self, cid: &Cid, what: &str) -> Result<Option<&[u8]>, String> {
        let key = cid.to_string_key();
        match self.bundle.objects.get(&key) {
            Some(bytes) => {
                if Hash::of(bytes) != cid.hash {
                    return Err(format!(
                        "{what} {key}: the bytes do not hash to the identifier"
                    ));
                }
                Ok(Some(bytes.as_slice()))
            }
            None if self.tombstones.covering(cid, None).is_some() => {
                self.tombstoned.insert(key);
                Ok(None)
            }
            None => Err(format!(
                "{what} {key} is missing and no tombstone covers it"
            )),
        }
    }

    /// The key a signature is checked with, or `None` when no set is pinned
    /// and nothing is checked.
    fn key_for(
        &self,
        key: &crate::hash::KeyId,
        at: crate::hlc::Hlc,
    ) -> Result<Option<crate::sign::PublicKey>, String> {
        let Some(pinned) = self.pinned else {
            return Ok(None);
        };
        match pinned.eligibility(key, at, self.scope) {
            IssuerEligibility::Eligible { .. } => {
                pinned.issuer_public_key(key).map(Some).ok_or_else(|| {
                    "the pinned root set holds no key for an eligible issuer".to_string()
                })
            }
            IssuerEligibility::Refused(why) => Err(format!(
                "the signing key is refused by the pinned root set: {why}"
            )),
            IssuerEligibility::Unknown => {
                Err("the signing key is unknown to the pinned root set".to_string())
            }
        }
    }

    /// An attestation's claim, decoded; `None` when it is tombstoned.
    fn claim_of(&mut self, a: &Attestation, what: &str) -> Result<Option<Value>, String> {
        match self.object(&a.body.claim, what)? {
            Some(bytes) => crate::cbor::decode(bytes)
                .map(Some)
                .map_err(|e| format!("{what}: the claim does not decode: {e}")),
            None => Ok(None),
        }
    }
}

fn text<'v>(m: &'v BTreeMap<String, Value>, k: &str) -> Option<&'v str> {
    m.get(k).and_then(Value::as_text)
}

/// Verify `bundle` against the operator's `pinned` root set, issuer keys
/// eligible in `scope`, and re-evaluate its admission under `policy` when one
/// is given.
pub fn verify(
    bundle: &DecisionBundle,
    pinned: &VerifiedRootSet,
    scope: &str,
    policy: Option<&AdmissionPolicy>,
) -> Verification {
    let mut out = Verification {
        pinned_root_set: pinned.digest,
        bundle_root_set: bundle.root_set,
        entries: bundle.entries.len(),
        divergence: None,
        tombstoned: Vec::new(),
        decisions: Vec::new(),
        admission: AdmissionCheck::NotReproduced("the bundle did not verify".into()),
    };
    let mut reader = Reader {
        bundle,
        pinned: Some(pinned),
        scope,
        tombstones: TombstoneSet::default(),
        tombstoned: BTreeSet::new(),
    };
    match walk(&mut reader, policy) {
        Ok((decisions, admission)) => {
            out.decisions = decisions;
            out.admission = admission;
        }
        Err(why) => out.divergence = Some(why),
    }
    out.tombstoned = reader.tombstoned.into_iter().collect();
    out
}

/// What the admission is evaluated over, as the bundle records it (spec 049
/// section 3.1): the collection [`verify`] makes before it re-evaluates, made
/// without a pinned root set. No signature is verified here, so this is a
/// reading of what the bundle says and not a verification of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionInputs {
    /// `registeredBy` of the revision, when the fact names one.
    pub submitter: Option<String>,
    /// The decisions the bundle names, resolved.
    pub decisions: Vec<NamedDecision>,
    /// Every evidence verdict about the revision, with its attestation id.
    pub verdicts: Vec<(AttestationId, EvidenceVerdict)>,
    /// The admission the bundle names, when it names one.
    pub recorded: Option<RecordedAdmission>,
}

/// What the bundle's recorded admission claim says, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedAdmission {
    /// The policy digest the claim names.
    pub policy: Option<String>,
    /// `admit` or `refuse`.
    pub decision: String,
}

/// Collect `bundle`'s admission inputs without a pinned root set. The first
/// thing that does not resolve is the error; a tombstoned verdict is one.
pub fn admission_inputs(bundle: &DecisionBundle) -> Result<AdmissionInputs, String> {
    let mut r = Reader {
        bundle,
        pinned: None,
        scope: "",
        tombstones: TombstoneSet::default(),
        tombstoned: BTreeSet::new(),
    };
    let resolved = resolve(&mut r)?;
    let recorded = admission_claim(&mut r, &resolved)?.map(|rm| RecordedAdmission {
        policy: text(&rm, "policy").map(str::to_string),
        decision: text(&rm, "decision").unwrap_or("").to_string(),
    });
    let verdicts = match verdicts(&mut r, &resolved)? {
        Verdicts::Collected(v) => v,
        Verdicts::Tombstoned(what) => return Err(format!("{what} is tombstoned")),
    };
    Ok(AdmissionInputs {
        submitter: resolved.submitter,
        decisions: resolved.decisions,
        verdicts,
        recorded,
    })
}

type Walked = (Vec<NamedDecision>, AdmissionCheck);

/// What the entries commit to, once every one of them is read.
struct Resolved {
    attestations: BTreeMap<Hash, Attestation>,
    submitter: Option<String>,
    decisions: Vec<NamedDecision>,
}

/// The objects, the entry chain, the tombstones, the attestations, the
/// submitter and the decisions named, in that order; the first divergence is
/// the error.
fn resolve(r: &mut Reader<'_>) -> Result<Resolved, String> {
    for (key, bytes) in &r.bundle.objects {
        let cid = parse_object_key(key)
            .ok_or_else(|| format!("object {key}: not an object identifier"))?;
        if Hash::of(bytes) != cid.hash {
            return Err(format!(
                "object {key}: the bytes do not hash to the identifier"
            ));
        }
    }
    if r.bundle.entries.is_empty() {
        return Err("the bundle holds no entries; it must begin at revision.registered".into());
    }
    // First every entry's chain and signature, so the payloads they commit to
    // are known before anything is resolved.
    let mut entries: Vec<Entry> = Vec::new();
    let mut previous: Option<Entry> = None;
    for (i, bytes) in r.bundle.entries.iter().enumerate() {
        let entry = Entry::decode(bytes).map_err(|e| format!("entry {i} does not decode: {e}"))?;
        if let Some(p) = &previous {
            if entry.body.parents.len() != 1 || entry.body.parents[0] != p.hash() {
                return Err(format!("entry {i}: its parent is not the previous entry"));
            }
            if entry.body.hlc <= p.body.hlc {
                return Err(format!("entry {i}: its clock is not after its parent's"));
            }
        }
        if let Some(key) = r
            .key_for(&entry.body.issuer, entry.body.hlc)
            .map_err(|e| format!("entry {i}: {e}"))?
        {
            entry
                .verify_signature(&key)
                .map_err(|_| format!("entry {i}: the signature does not recompute"))?;
        }
        previous = Some(entry.clone());
        entries.push(entry);
    }
    // A tombstone is trusted only when a verified entry commits to it: one the
    // bundle merely carries could hide a missing object.
    let committed: BTreeSet<Cid> = entries.iter().map(|e| e.body.payload).collect();
    for (i, bytes) in r.bundle.tombstones.iter().enumerate() {
        let fact = FactEnvelope::decode(bytes)
            .map_err(|e| format!("tombstone {i} does not decode: {e}"))?;
        if !committed.contains(&fact.cid()) {
            return Err(format!(
                "tombstone {i} is not recorded by any entry in the bundle"
            ));
        }
        let t = Tombstone::from_fact(&fact).map_err(|e| format!("tombstone {i}: {e}"))?;
        r.tombstones.add(t);
    }
    let mut attestations: BTreeMap<Hash, Attestation> = BTreeMap::new();
    let mut submitter: Option<String> = None;
    for (i, entry) in entries.iter().enumerate() {
        let Some(payload) = r.object(&entry.body.payload, &format!("entry {i} payload"))? else {
            // The revision is identified by its first fact; erased, it cannot
            // be checked, and nothing after it can be bound to it.
            if i == 0 {
                return Err("entry 0's revision.registered fact is tombstoned; the revision id cannot be checked".into());
            }
            continue;
        };
        let fact = FactEnvelope::decode(payload)
            .map_err(|e| format!("entry {i}: the payload does not decode: {e}"))?;
        if i == 0 {
            if fact.kind != "revision.registered" {
                return Err("entry 0 is not revision.registered".into());
            }
            if Hash::of(payload) != r.bundle.revision {
                return Err(
                    "the revision id is not the hash of its revision.registered fact".into(),
                );
            }
            submitter = text(&fact.body, "registeredBy").map(str::to_string);
        }
        if fact.kind == "attestation.issued" {
            let Some(Value::Link(cid)) = fact.body.get("attestation") else {
                return Err(format!(
                    "entry {i}: attestation.issued names no attestation"
                ));
            };
            let cid = *cid;
            if let Some(abytes) = r.object(&cid, &format!("entry {i} attestation"))? {
                let a = Attestation::decode(abytes)
                    .map_err(|e| format!("entry {i}: the attestation does not decode: {e}"))?;
                if let Some(akey) = r
                    .key_for(&a.issuer_key, a.body.at)
                    .map_err(|e| format!("entry {i} attestation: {e}"))?
                {
                    a.verify_signature(&akey).map_err(|_| {
                        format!("entry {i}: the attestation's signature does not recompute")
                    })?;
                }
                attestations.insert(a.id().0, a);
            }
        }
    }

    // The decisions named.
    let mut decisions = Vec::new();
    let mut named = BTreeSet::new();
    for id in &r.bundle.decisions {
        // A decision is counted once: a repeated id would count it twice.
        if !named.insert(*id) {
            return Err(format!("decision {} is named twice", id.to_hex()));
        }
        let a = attestations
            .get(id)
            .ok_or_else(|| {
                format!(
                    "decision {} is not an attestation in the bundle",
                    id.to_hex()
                )
            })?
            .clone();
        if a.body.subject != r.bundle.revision {
            return Err(format!(
                "decision {} is about another revision",
                id.to_hex()
            ));
        }
        let approves = match a.body.predicate.0.as_str() {
            APPROVAL => true,
            CHANGE_REQUEST => false,
            other => {
                return Err(format!(
                    "decision {} has the predicate {other}",
                    id.to_hex()
                ));
            }
        };
        let what = format!("decision {}", id.to_hex());
        let claim = r
            .claim_of(&a, &what)?
            .ok_or_else(|| format!("{what}: its claim is tombstoned"))?;
        let m = claim
            .as_map()
            .ok_or_else(|| format!("{what}: the claim is not a map"))?;
        if text(m, "revision") != Some(r.bundle.revision.to_hex().as_str()) {
            return Err(format!("{what} names another revision"));
        }
        let reviewer = m
            .get("reviewer")
            .ok_or_else(|| format!("{what} names no reviewer"))
            .and_then(|v| Principal::from_value(v).map_err(|e| format!("{what}: {e}")))?;
        decisions.push(NamedDecision {
            id: *id,
            approves,
            reviewer,
        });
    }
    Ok(Resolved {
        attestations,
        submitter,
        decisions,
    })
}

/// The recorded admission's claim, as a map; `None` when the bundle names no
/// admission. An admission that does not resolve is the error.
fn admission_claim(
    r: &mut Reader<'_>,
    resolved: &Resolved,
) -> Result<Option<BTreeMap<String, Value>>, String> {
    let Some(admission_id) = r.bundle.admission else {
        return Ok(None);
    };
    let a = resolved
        .attestations
        .get(&admission_id)
        .ok_or_else(|| "the admission is not an attestation in the bundle".to_string())?
        .clone();
    if a.body.subject != r.bundle.revision {
        return Err("the admission is about another revision".into());
    }
    if a.body.predicate.0 != POLICY_EVAL {
        return Err(format!(
            "the admission has the predicate {}",
            a.body.predicate.0
        ));
    }
    let recorded = r
        .claim_of(&a, "the admission")?
        .ok_or("the admission's claim is tombstoned")?;
    let rm = recorded
        .as_map()
        .ok_or("the admission's claim is not a map")?;
    Ok(Some(rm.clone()))
}

/// The evidence verdicts about the revision, or the first one that is
/// tombstoned, which is not a divergence.
enum Verdicts {
    Collected(Vec<(AttestationId, EvidenceVerdict)>),
    Tombstoned(String),
}

fn verdicts(r: &mut Reader<'_>, resolved: &Resolved) -> Result<Verdicts, String> {
    let mut verdicts: Vec<(AttestationId, EvidenceVerdict)> = Vec::new();
    for (id, att) in &resolved.attestations {
        if att.body.predicate.0 != EVIDENCE_VERDICT || att.body.subject != r.bundle.revision {
            continue;
        }
        let what = format!("verdict {}", id.to_hex());
        let Some(claim) = r.claim_of(att, &what)? else {
            return Ok(Verdicts::Tombstoned(what));
        };
        let verdict: EvidenceVerdict = serde_json::from_value(claim.to_json())
            .map_err(|e| format!("{what}: the claim is not a verdict: {e}"))?;
        verdicts.push((AttestationId(*id), verdict));
    }
    Ok(Verdicts::Collected(verdicts))
}

fn walk(r: &mut Reader<'_>, policy: Option<&AdmissionPolicy>) -> Result<Walked, String> {
    let resolved = resolve(r)?;
    let Some(rm) = admission_claim(r, &resolved)? else {
        return Ok((resolved.decisions, AdmissionCheck::None));
    };
    let recorded_decision = text(&rm, "decision").unwrap_or("").to_string();
    let Some(policy) = policy else {
        return Ok((
            resolved.decisions,
            AdmissionCheck::NotReproduced("no pinned admission policy was given".into()),
        ));
    };
    let policy_digest = policy_digest(policy);
    if text(&rm, "policy") != Some(policy_digest.to_hex().as_str()) {
        return Ok((
            resolved.decisions,
            AdmissionCheck::NotReproduced(format!(
                "the admission was evaluated under policy {}, not the pinned {}",
                text(&rm, "policy").unwrap_or("(none)"),
                policy_digest.to_hex()
            )),
        ));
    }
    let Some(submitter) = resolved.submitter.clone() else {
        return Ok((
            resolved.decisions,
            AdmissionCheck::NotReproduced("the revision names no submitter".into()),
        ));
    };
    let verdicts = match verdicts(r, &resolved)? {
        Verdicts::Collected(v) => v,
        Verdicts::Tombstoned(what) => {
            return Ok((
                resolved.decisions,
                AdmissionCheck::NotReproduced(format!("{what} is tombstoned")),
            ));
        }
    };
    let decisions = resolved.decisions;
    let counted: Vec<Decision> = decisions
        .iter()
        .map(|d| Decision {
            id: AttestationId(d.id),
            approves: d.approves,
            reviewer: d.reviewer.clone(),
        })
        .collect();
    let submitter = Principal::new(PrincipalKind::Human, &submitter);
    let reproduced = evaluate(
        policy,
        &AdmissionInput {
            verdicts: &verdicts,
            decisions: &counted,
            submitter: &submitter,
        },
    );
    let expected = policy_eval_claim(policy_digest, &[], &reproduced);
    let em = expected.as_map().expect("the claim is a map");
    let reproduced_word = match reproduced {
        Admission::Admit => "admit",
        Admission::Refuse { .. } => "refuse",
    }
    .to_string();
    if rm.get("decision") != em.get("decision") || rm.get("reasons") != em.get("reasons") {
        return Ok((
            decisions,
            AdmissionCheck::Diverged {
                recorded: recorded_decision,
                reproduced: reproduced_word,
            },
        ));
    }
    Ok((
        decisions,
        AdmissionCheck::Reproduced {
            decision: reproduced_word,
        },
    ))
}

/// A policy's digest: the canonical encoding of its portable value, the
/// construction the root set's digest uses (platform spec 004, section 5).
pub fn policy_digest(policy: &AdmissionPolicy) -> Hash {
    let j = serde_json::to_value(policy).expect("a policy serializes");
    let v = Value::from_json(&j).expect("policy values are portable");
    Hash::of(&crate::cbor::encode(&v))
}

/// Whether `key` names a `DagCbor` or `Raw` object identifier.
pub fn parse_object_key(key: &str) -> Option<Cid> {
    let (codec, hex) = key.split_once(':')?;
    let codec = match codec {
        c if c == Codec::Raw.as_str() => Codec::Raw,
        c if c == Codec::DagCbor.as_str() => Codec::DagCbor,
        _ => return None,
    };
    Some(Cid {
        codec,
        hash: Hash::from_hex(hex).ok()?,
    })
}
