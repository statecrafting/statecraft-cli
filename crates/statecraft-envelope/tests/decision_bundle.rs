//! Spec 038 section 3.6: the offline decision-bundle reader, against bundles
//! assembled here from this crate's own constructors and a pinned V2 root
//! set. Every seed is a conspicuously public, test-only value.

use std::collections::BTreeMap;

use statecraft_envelope::admission::policy_eval_claim;
use statecraft_envelope::attestation::{
    Attestation, AttestationId, PredicateType, Principal, PrincipalKind, UnsignedAttestation,
};
use statecraft_envelope::bundle::{
    APPROVAL, AdmissionCheck, DecisionBundle, EVIDENCE_VERDICT, POLICY_EVAL, policy_digest, verify,
};
use statecraft_envelope::cbor::{cid_of, encode};
use statecraft_envelope::dimensions::{Admission, AdmissionPolicy};
use statecraft_envelope::entry::{Entry, EntryHash, UnsignedEntry};
use statecraft_envelope::fact::{EraseReason, FactEnvelope, Tombstone, TombstoneScope};
use statecraft_envelope::hash::{Cid, Hash};
use statecraft_envelope::hlc::Hlc;
use statecraft_envelope::issuer::{
    Authorization, Enrolment, IssuerIdentity, IssuerPayload, OwnerRoot, RootSetV2, VerifiedRootSet,
    identity_created_fact,
};
use statecraft_envelope::roots::RootStatus;
use statecraft_envelope::sign::Signer;
use statecraft_envelope::value::Value;

fn signer(label: &str) -> Signer {
    Signer::from_seed(blake3::hash(label.as_bytes()).as_bytes())
}

const T0: u64 = 1_790_000_000_000;
const SCOPE: &str = "platform";

fn at(ms: u64) -> Hlc {
    Hlc::new(T0 + ms, 0)
}

/// A pinned set enrolling `issuer` in `scope`, authorized by `owner`.
fn pinned(owner: &Signer, issuer: &Signer, scope: &str) -> VerifiedRootSet {
    let fact = identity_created_fact(&issuer.public(), at(0));
    let identity = IssuerIdentity::decode(&fact.canonical_bytes()).unwrap();
    let payload = IssuerPayload::Enrolment(Enrolment {
        issuer_id: identity.id,
        public_key: issuer.public(),
        scope: scope.into(),
        not_before: Some(at(0)),
        not_after: None,
    });
    let signature = owner.sign(payload.kind().domain(), &payload.canonical_bytes());
    RootSetV2 {
        root_set_version: 1,
        owner_roots: vec![OwnerRoot {
            public_key: owner.public(),
            status: RootStatus::Active,
            not_before: None,
            not_after: None,
            since: None,
        }],
        enrolments: vec![Authorization {
            root_key_id: owner.public().id(),
            payload,
            signature,
        }],
        rotations: vec![],
        revocations: vec![],
    }
    .verify()
    .unwrap()
}

/// A scope written by the platform issuer: the bundle and its parts.
struct Scope {
    issuer: Signer,
    entries: Vec<Entry>,
    objects: BTreeMap<String, Vec<u8>>,
    tombstones: Vec<Vec<u8>>,
    revision: Hash,
    attestations: Vec<Hash>,
}

impl Scope {
    fn new(issuer: Signer) -> Scope {
        let mut body = BTreeMap::new();
        body.insert("repository".into(), Value::text("11".repeat(32)));
        body.insert("commit".into(), Value::text("c0ffee"));
        body.insert("tree".into(), Value::Null);
        body.insert("base".into(), Value::Null);
        body.insert("registeredBy".into(), Value::text("sam"));
        let fact = FactEnvelope::new("revision.registered", 1, body);
        let revision = Hash::of(&fact.canonical_bytes());
        let mut s = Scope {
            issuer,
            entries: Vec::new(),
            objects: BTreeMap::new(),
            tombstones: Vec::new(),
            revision,
            attestations: Vec::new(),
        };
        s.append(&fact);
        s
    }

    fn put(&mut self, cid: Cid, bytes: Vec<u8>) {
        self.objects.insert(cid.to_string_key(), bytes);
    }

    fn append(&mut self, fact: &FactEnvelope) {
        let bytes = fact.canonical_bytes();
        self.put(fact.cid(), bytes);
        // The first entry's parent lies outside the slice: a fixed stand-in.
        let parent = match self.entries.last() {
            Some(e) => e.hash(),
            None => EntryHash(Hash::of(b"outside the slice")),
        };
        let n = self.entries.len() as u64;
        let entry = UnsignedEntry::new(
            vec![parent],
            self.issuer.public().id(),
            at(10 + n),
            fact.cid(),
            BTreeMap::new(),
        )
        .unwrap()
        .sign(&self.issuer)
        .unwrap();
        self.entries.push(entry);
    }

    /// Issue an attestation about the revision and record it in the scope.
    fn attest(&mut self, predicate: &str, claim: Value) -> Hash {
        let claim_bytes = encode(&claim);
        let claim_cid = cid_of(&claim);
        self.put(claim_cid, claim_bytes);
        let n = self.entries.len() as u64;
        let a: Attestation = UnsignedAttestation {
            subject: self.revision,
            predicate: PredicateType::new(predicate).unwrap(),
            issuer: Principal::new(PrincipalKind::Service, "platform"),
            claim: claim_cid,
            at: at(10 + n),
            extra: BTreeMap::new(),
        }
        .issue(&self.issuer)
        .unwrap();
        self.put(a.object_cid(), a.canonical_bytes());
        let mut body = BTreeMap::new();
        body.insert("attestation".into(), Value::Link(a.object_cid()));
        body.insert("subject".into(), Value::Bytes(self.revision.0.to_vec()));
        body.insert("predicate".into(), Value::text(predicate));
        body.insert(
            "issuer".into(),
            Principal::new(PrincipalKind::Service, "platform").to_value(),
        );
        self.append(&FactEnvelope::new("attestation.issued", 1, body));
        self.attestations.push(a.id().0);
        a.id().0
    }

    fn bundle(
        &self,
        root_set: Hash,
        admission: Option<Hash>,
        decisions: Vec<Hash>,
    ) -> DecisionBundle {
        DecisionBundle {
            revision: self.revision,
            entries: self.entries.iter().map(Entry::canonical_bytes).collect(),
            objects: self.objects.clone(),
            tombstones: self.tombstones.clone(),
            root_set,
            admission,
            decisions,
        }
    }
}

fn verdict_claim() -> Value {
    statecraft_envelope::verdict::verify_artifact(
        &statecraft_envelope::reference::Reference::over_file_bytes("receipt", "1", b"bytes"),
        Some(b"bytes"),
        None,
        &statecraft_envelope::roots::RootSet::empty(),
    )
    .to_claim()
}

fn approval_claim(revision: &Hash, reviewer: &str) -> Value {
    Value::map()
        .with(
            "reviewer",
            Principal::new(PrincipalKind::Human, reviewer).to_value(),
        )
        .unwrap()
        .with("revision", Value::text(revision.to_hex()))
        .unwrap()
        .with("basis", Value::Array(vec![]))
        .unwrap()
        .with("issuedBy", Value::text("platform"))
        .unwrap()
}

/// A complete scope: a verdict, an approval by `rae`, and the admission the
/// platform recorded under `policy`.
fn decided(policy: &AdmissionPolicy) -> (Scope, VerifiedRootSet, Hash, Hash) {
    let owner = signer("statecraft TEST-ONLY bundle owner root: public, never production");
    let issuer = signer("statecraft TEST-ONLY bundle platform issuer: public, never production");
    let set = pinned(&owner, &issuer, SCOPE);
    let mut s = Scope::new(issuer);
    let verdict = s.attest(EVIDENCE_VERDICT, verdict_claim());
    let revision = s.revision;
    let approval = s.attest(APPROVAL, approval_claim(&revision, "rae"));
    let admission = statecraft_envelope::admission::evaluate(
        policy,
        &statecraft_envelope::admission::AdmissionInput {
            verdicts: &[(
                AttestationId(verdict),
                serde_json::from_value(verdict_claim().to_json()).unwrap(),
            )],
            decisions: &[statecraft_envelope::admission::Decision {
                id: AttestationId(approval),
                approves: true,
                reviewer: Principal::new(PrincipalKind::Human, "rae"),
            }],
            submitter: &Principal::new(PrincipalKind::Human, "sam"),
        },
    );
    let eval = s.attest(
        POLICY_EVAL,
        policy_eval_claim(
            policy_digest(policy),
            &[AttestationId(verdict), AttestationId(approval)],
            &admission,
        ),
    );
    (s, set, approval, eval)
}

#[test]
fn a_faithful_bundle_verifies_and_its_admission_is_reproduced() {
    let policy = AdmissionPolicy::permissive();
    let (s, set, approval, eval) = decided(&policy);
    let b = s.bundle(set.digest, Some(eval), vec![approval]);
    // Through the JSON the cell sends, with its extra word.
    let mut json = serde_json::json!({
        "revision": b.revision, "entries": b.entries, "objects": b.objects,
        "tombstones": b.tombstones, "root_set": b.root_set, "admission": b.admission,
        "decisions": b.decisions, "admissionWord": "recorded",
    });
    let parsed = DecisionBundle::parse(&serde_json::to_vec(&json).unwrap()).unwrap();
    assert_eq!(parsed, b);
    let v = verify(&parsed, &set, SCOPE, Some(&policy));
    assert_eq!(v.divergence, None);
    assert!(v.root_set_matches());
    assert_eq!(
        v.admission,
        AdmissionCheck::Reproduced {
            decision: "admit".into()
        }
    );
    assert_eq!(v.decisions.len(), 1);
    assert_eq!(v.decisions[0].reviewer.id, "rae");
    assert!(v.passes());
    json["root_set"] = serde_json::json!("not hex");
    assert!(DecisionBundle::parse(&serde_json::to_vec(&json).unwrap()).is_err());
}

#[test]
fn a_bundle_under_another_root_set_is_a_finding_naming_both() {
    let policy = AdmissionPolicy::permissive();
    let (s, set, approval, eval) = decided(&policy);
    let other = Hash::of(b"the cell's own root set");
    let v = verify(
        &s.bundle(other, Some(eval), vec![approval]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert!(!v.root_set_matches());
    assert_eq!((v.pinned_root_set, v.bundle_root_set), (set.digest, other));
    assert!(!v.passes());
}

#[test]
fn a_tampered_entry_object_or_signature_is_the_first_divergence() {
    let policy = AdmissionPolicy::permissive();
    let (s, set, approval, eval) = decided(&policy);
    let good = s.bundle(set.digest, Some(eval), vec![approval]);

    // The last entry: only its signature can catch it.
    let mut b = good.clone();
    let last = b.entries.len() - 1;
    let n = b.entries[last].len();
    b.entries[last][n - 1] ^= 1;
    let v = verify(&b, &set, SCOPE, Some(&policy));
    assert!(
        v.divergence
            .as_deref()
            .unwrap()
            .starts_with(&format!("entry {last}")),
        "{v:?}"
    );
    assert!(!v.passes());

    // An object whose bytes no longer hash to its identifier.
    let mut b = good.clone();
    let key = b.objects.keys().next().unwrap().clone();
    b.objects.get_mut(&key).unwrap().push(0);
    let v = verify(&b, &set, SCOPE, Some(&policy));
    assert!(v.divergence.unwrap().contains("do not hash"));

    // A dropped entry breaks the parent link of the next.
    let mut b = good.clone();
    b.entries.remove(1);
    let v = verify(&b, &set, SCOPE, Some(&policy));
    assert!(v.divergence.unwrap().contains("parent"));

    // Another revision id.
    let mut b = good.clone();
    b.revision = Hash::of(b"another revision");
    let v = verify(&b, &set, SCOPE, Some(&policy));
    assert!(v.divergence.unwrap().contains("revision"));

    // A key the pinned set does not know, and one enrolled for another scope.
    let stranger = signer("statecraft TEST-ONLY stranger: public, never production");
    let unpinned = pinned(&stranger, &stranger, SCOPE);
    let v = verify(&good, &unpinned, SCOPE, Some(&policy));
    assert!(
        v.divergence
            .unwrap()
            .contains("unknown to the pinned root set")
    );
    let v = verify(&good, &set, "repository", Some(&policy));
    assert!(v.divergence.unwrap().contains("scope-mismatch"));
}

#[test]
fn a_tombstoned_object_is_told_apart_from_a_missing_one() {
    let policy = AdmissionPolicy::permissive();
    let (mut s, set, approval, _) = decided(&policy);
    // Erase the approval's claim and record the tombstone.
    let att_key = s
        .objects
        .keys()
        .find(|k| Attestation::decode(&s.objects[*k]).is_ok_and(|a| a.id().0 == approval));
    let a = Attestation::decode(&s.objects[att_key.unwrap()]).unwrap();
    let claim = a.body.claim;
    s.objects.remove(&claim.to_string_key());
    let missing = s.bundle(set.digest, None, vec![approval]);
    let v = verify(&missing, &set, SCOPE, Some(&policy));
    assert!(v.divergence.unwrap().contains("no tombstone covers it"));
    let tombstone = Tombstone {
        target: claim,
        reason: EraseReason::Erasure,
        scope: TombstoneScope::Object,
    }
    .to_fact();
    // Carried but recorded by no entry: not trusted.
    s.tombstones.push(tombstone.canonical_bytes());
    let v = verify(
        &s.bundle(set.digest, None, vec![approval]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert!(v.divergence.unwrap().contains("not recorded by any entry"));
    s.append(&tombstone);
    let v = verify(
        &s.bundle(set.digest, None, vec![approval]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert!(v.divergence.unwrap().contains("tombstoned"));
    assert_eq!(v.tombstoned, vec![claim.to_string_key()]);
}

#[test]
fn none_is_not_success_and_an_unreproducible_admission_says_why() {
    let policy = AdmissionPolicy::permissive();
    let (s, set, approval, eval) = decided(&policy);
    let v = verify(
        &s.bundle(set.digest, None, vec![]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert_eq!(v.divergence, None);
    assert_eq!(v.admission, AdmissionCheck::None);
    assert!(!v.passes());
    // No pinned policy: verified, not reproduced.
    let b = s.bundle(set.digest, Some(eval), vec![approval]);
    let v = verify(&b, &set, SCOPE, None);
    assert!(matches!(v.admission, AdmissionCheck::NotReproduced(ref w) if w.contains("no pinned")));
    assert!(!v.passes());
    // Another pinned policy: the digests differ, and both are named.
    let v = verify(&b, &set, SCOPE, Some(&AdmissionPolicy::strict()));
    assert!(
        matches!(v.admission, AdmissionCheck::NotReproduced(ref w) if w.contains("not the pinned"))
    );
}

#[test]
fn a_refused_admission_is_reproduced_as_a_refusal_and_does_not_pass() {
    let policy = AdmissionPolicy::strict();
    let (s, set, approval, eval) = decided(&policy);
    let v = verify(
        &s.bundle(set.digest, Some(eval), vec![approval]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert_eq!(v.divergence, None);
    assert_eq!(
        v.admission,
        AdmissionCheck::Reproduced {
            decision: "refuse".into()
        }
    );
    assert!(!v.passes());
}

#[test]
fn an_admission_the_pinned_policy_contradicts_diverges() {
    let policy = AdmissionPolicy::strict();
    let owner = signer("statecraft TEST-ONLY bundle owner root: public, never production");
    let issuer = signer("statecraft TEST-ONLY bundle platform issuer: public, never production");
    let set = pinned(&owner, &issuer, SCOPE);
    let mut s = Scope::new(issuer);
    let verdict = s.attest(EVIDENCE_VERDICT, verdict_claim());
    // The cell claims an admit that the strict policy refuses.
    let eval = s.attest(
        POLICY_EVAL,
        policy_eval_claim(
            policy_digest(&policy),
            &[AttestationId(verdict)],
            &Admission::Admit,
        ),
    );
    let v = verify(
        &s.bundle(set.digest, Some(eval), vec![]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert_eq!(
        v.admission,
        AdmissionCheck::Diverged {
            recorded: "admit".into(),
            reproduced: "refuse".into()
        }
    );
    assert!(!v.passes());
}

#[test]
fn an_attestation_about_another_revision_is_not_this_revisions_decision() {
    let policy = AdmissionPolicy::permissive();
    let owner = signer("statecraft TEST-ONLY bundle owner root: public, never production");
    let issuer = signer("statecraft TEST-ONLY bundle platform issuer: public, never production");
    let set = pinned(&owner, &issuer, SCOPE);
    let mut s = Scope::new(issuer);
    let real = s.revision;
    // Issued, validly signed, about another revision.
    s.revision = Hash::of(b"another revision");
    let eval = s.attest(
        POLICY_EVAL,
        policy_eval_claim(policy_digest(&policy), &[], &Admission::Admit),
    );
    let approval = s.attest(APPROVAL, approval_claim(&real, "rae"));
    s.revision = real;
    let v = verify(
        &s.bundle(set.digest, Some(eval), vec![]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert_eq!(
        v.divergence.as_deref(),
        Some("the admission is about another revision")
    );
    let v = verify(
        &s.bundle(set.digest, None, vec![approval]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert!(v.divergence.unwrap().contains("about another revision"));
}

#[test]
fn an_erased_revision_registration_is_a_divergence() {
    let policy = AdmissionPolicy::permissive();
    let (mut s, set, _, _) = decided(&policy);
    let first = Entry::decode(&s.entries[0].canonical_bytes())
        .unwrap()
        .body
        .payload;
    s.objects.remove(&first.to_string_key());
    let tombstone = Tombstone {
        target: first,
        reason: EraseReason::Erasure,
        scope: TombstoneScope::Object,
    }
    .to_fact();
    s.tombstones.push(tombstone.canonical_bytes());
    s.append(&tombstone);
    let v = verify(
        &s.bundle(set.digest, None, vec![]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert!(
        v.divergence
            .unwrap()
            .contains("revision id cannot be checked")
    );
}

#[test]
fn a_decision_named_twice_is_a_divergence() {
    let policy = AdmissionPolicy::permissive();
    let (s, set, approval, eval) = decided(&policy);
    let v = verify(
        &s.bundle(set.digest, Some(eval), vec![approval, approval]),
        &set,
        SCOPE,
        Some(&policy),
    );
    assert!(v.divergence.unwrap().contains("named twice"));
}
