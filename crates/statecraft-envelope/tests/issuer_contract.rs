//! Spec 035: the offline issuer authorization contract.
//!
//! Every seed here is a conspicuously public, deterministic test-only value
//! derived from an ASCII label that says so. None is, or may become,
//! production ceremony material.
//!
//! The golden vectors under `testdata/vectors/issuer/` are committed bytes.
//! `STATECRAFT_MINT_VECTORS=1 cargo test -p statecraft-envelope --test
//! issuer_contract` writes them; without the variable every byte is derived
//! again from the seeds and times below and compared with the committed copy.

use std::collections::BTreeMap;
use std::path::PathBuf;

use statecraft_envelope::cbor;
use statecraft_envelope::hash::{Hash, KeyId};
use statecraft_envelope::hlc::Hlc;
use statecraft_envelope::issuer::{
    Authorization, DetachedSignature, Enrolment, IssuerEligibility, IssuerIdentity, IssuerPayload,
    IssuerRecordKind, OwnerRoot, OwnerRootPublic, PinTransition, Revocation, RootSetDocument,
    RootSetPin, RootSetV2, Rotation, admit_pinned, identity_created_fact, verify_detached,
};
use statecraft_envelope::roots::RootStatus;
use statecraft_envelope::sign::{
    DOMAIN_ATTESTATION, DOMAIN_ENTRY, DOMAIN_ISSUER_ENROLMENT, DOMAIN_ISSUER_REVOCATION,
    DOMAIN_ISSUER_ROTATION, PublicKey, Signature, Signer,
};
use statecraft_envelope::value::Value;

// ---------------------------------------------------------------------------
// Test-only material
// ---------------------------------------------------------------------------

const OWNER_LABEL: &str = "statecraft TEST-ONLY owner root seed: public, never production";
const OTHER_OWNER_LABEL: &str =
    "statecraft TEST-ONLY second owner root seed: public, never production";
const ONLINE_LABEL: &str = "statecraft TEST-ONLY online issuer seed: public, never production";
const SUCCESSOR_LABEL: &str =
    "statecraft TEST-ONLY successor issuer seed: public, never production";
const SECOND_ISSUER_LABEL: &str =
    "statecraft TEST-ONLY second issuer seed: public, never production";

fn seed(label: &str) -> [u8; 32] {
    *blake3::hash(label.as_bytes()).as_bytes()
}

fn signer(label: &str) -> Signer {
    Signer::from_seed(&seed(label))
}

const T0: u64 = 1_790_000_000_000;

fn at(ms_after: u64, logical: u32) -> Hlc {
    Hlc::new(T0 + ms_after, logical)
}

struct World {
    owner: Signer,
    online: Signer,
    successor: Signer,
    identity: IssuerIdentity,
    enrolment: Enrolment,
    rotation: Rotation,
    revocation: Revocation,
}

fn world() -> World {
    let owner = signer(OWNER_LABEL);
    let online = signer(ONLINE_LABEL);
    let successor = signer(SUCCESSOR_LABEL);
    let fact = identity_created_fact(&online.public(), at(0, 0));
    let identity = IssuerIdentity::decode(&fact.canonical_bytes()).unwrap();
    let enrolment = Enrolment {
        issuer_id: identity.id,
        public_key: online.public(),
        scope: "platform".into(),
        not_before: Some(at(0, 0)),
        not_after: None,
    };
    let rotation = Rotation {
        identity: identity.id,
        from: online.public().id(),
        to: successor.public(),
        effective: at(1_000, 0),
    };
    let revocation = Revocation {
        identity: identity.id,
        key: online.public().id(),
        since: at(500, 0),
        reason: "compromise".into(),
    };
    World {
        owner,
        online,
        successor,
        identity,
        enrolment,
        rotation,
        revocation,
    }
}

fn auth(owner: &Signer, payload: IssuerPayload) -> Authorization {
    let sig = owner.sign(payload.kind().domain(), &payload.canonical_bytes());
    Authorization {
        root_key_id: owner.public().id(),
        payload,
        signature: sig,
    }
}

fn active_root(s: &Signer) -> OwnerRoot {
    OwnerRoot {
        public_key: s.public(),
        status: RootStatus::Active,
        not_before: None,
        not_after: None,
        since: None,
    }
}

fn set(w: &World, rotations: bool, revocations: bool) -> RootSetV2 {
    RootSetV2 {
        root_set_version: 7,
        owner_roots: vec![active_root(&w.owner)],
        enrolments: vec![auth(
            &w.owner,
            IssuerPayload::Enrolment(w.enrolment.clone()),
        )],
        rotations: if rotations {
            vec![auth(&w.owner, IssuerPayload::Rotation(w.rotation.clone()))]
        } else {
            vec![]
        },
        revocations: if revocations {
            vec![auth(
                &w.owner,
                IssuerPayload::Revocation(w.revocation.clone()),
            )]
        } else {
            vec![]
        },
    }
}

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/vectors/issuer")
}

// ---------------------------------------------------------------------------
// Golden vectors
// ---------------------------------------------------------------------------

fn derive_vectors() -> BTreeMap<&'static str, Vec<u8>> {
    let w = world();
    let mut out: BTreeMap<&'static str, Vec<u8>> = BTreeMap::new();
    let seed_line = format!("{}\n", hex::encode(seed(OWNER_LABEL)));
    out.insert("test-only-owner-root.seed", seed_line.into_bytes());
    out.insert(
        "owner-root.public.json",
        OwnerRootPublic::of(&w.owner).to_bytes(),
    );
    out.insert("identity.created.cbor", w.identity.bytes.clone());
    let e = w.enrolment.canonical_bytes();
    out.insert(
        "enrolment.signature.json",
        DetachedSignature::sign(&w.owner, IssuerRecordKind::Enrolment, &e).to_bytes(),
    );
    out.insert("enrolment.cbor", e);
    let r = w.rotation.canonical_bytes();
    out.insert(
        "rotation.signature.json",
        DetachedSignature::sign(&w.owner, IssuerRecordKind::Rotation, &r).to_bytes(),
    );
    out.insert("rotation.cbor", r);
    let v = w.revocation.canonical_bytes();
    out.insert(
        "revocation.signature.json",
        DetachedSignature::sign(&w.owner, IssuerRecordKind::Revocation, &v).to_bytes(),
    );
    out.insert("revocation.cbor", v);
    let full = set(&w, true, true);
    out.insert("root-set-v2.json", full.to_bytes());
    let summary = serde_json::json!({
        "note": "TEST-ONLY vectors from public labelled seeds; never production material",
        "owner_seed_label": OWNER_LABEL,
        "online_seed_label": ONLINE_LABEL,
        "successor_seed_label": SUCCESSOR_LABEL,
        "owner_public_key": hex::encode(w.owner.public().0),
        "owner_key_id": w.owner.public().id().0.to_hex(),
        "online_public_key": hex::encode(w.online.public().0),
        "online_key_id": w.online.public().id().0.to_hex(),
        "successor_public_key": hex::encode(w.successor.public().0),
        "issuer_id": w.identity.id.to_hex(),
        "enrolment_digest": Hash::of(&w.enrolment.canonical_bytes()).to_hex(),
        "rotation_digest": Hash::of(&w.rotation.canonical_bytes()).to_hex(),
        "revocation_digest": Hash::of(&w.revocation.canonical_bytes()).to_hex(),
        "root_set_version": full.root_set_version,
        "root_set_digest": full.digest().to_hex(),
    });
    let mut s = serde_json::to_vec_pretty(&summary).unwrap();
    s.push(b'\n');
    out.insert("vectors.json", s);
    out
}

#[test]
fn golden_vectors_match_the_committed_bytes() {
    let vectors = derive_vectors();
    if std::env::var("STATECRAFT_MINT_VECTORS").as_deref() == Ok("1") {
        std::fs::create_dir_all(dir()).unwrap();
        for (name, bytes) in &vectors {
            std::fs::write(dir().join(name), bytes).unwrap();
        }
        return;
    }
    for (name, bytes) in &vectors {
        let committed = std::fs::read(dir().join(name))
            .unwrap_or_else(|e| panic!("{name}: committed vector missing: {e}"));
        assert_eq!(&committed, bytes, "{name} drifted from its frozen bytes");
    }
}

#[test]
fn the_committed_vectors_verify_through_the_public_readers() {
    let read = |n: &str| std::fs::read(dir().join(n)).unwrap();
    let root = OwnerRootPublic::parse(&read("owner-root.public.json")).unwrap();
    // The key id is BLAKE3 of the raw public-key bytes.
    assert_eq!(root.key_id().0, Hash::of(&root.public_key.0));
    for (kind, payload, sig) in [
        (
            IssuerRecordKind::Enrolment,
            "enrolment.cbor",
            "enrolment.signature.json",
        ),
        (
            IssuerRecordKind::Rotation,
            "rotation.cbor",
            "rotation.signature.json",
        ),
        (
            IssuerRecordKind::Revocation,
            "revocation.cbor",
            "revocation.signature.json",
        ),
    ] {
        let s = DetachedSignature::parse(&read(sig)).unwrap();
        verify_detached(
            &root,
            kind,
            &read(payload),
            &read("identity.created.cbor"),
            &s,
        )
        .unwrap_or_else(|e| panic!("{payload}: {e}"));
    }
    let set = RootSetV2::parse(&read("root-set-v2.json")).unwrap();
    let verified = set.verify().unwrap();
    assert!(verified.holds_root(&root));
    let summary: serde_json::Value = serde_json::from_slice(&read("vectors.json")).unwrap();
    assert_eq!(summary["root_set_digest"], verified.digest.to_hex());
}

// ---------------------------------------------------------------------------
// Enrollment
// ---------------------------------------------------------------------------

#[test]
fn an_enrolment_round_trips_and_preserves_the_identity_timestamp() {
    let w = world();
    let bytes = w.enrolment.canonical_bytes();
    assert_eq!(Enrolment::decode(&bytes).unwrap(), w.enrolment);
    // The identity is the hash of the exact original fact bytes, and its
    // timestamp is the one it was created with.
    assert_eq!(w.identity.id, Hash::of(&w.identity.bytes));
    assert_eq!(w.identity.at, at(0, 0));
    let root = OwnerRootPublic::of(&w.owner);
    let sig = DetachedSignature::sign(&w.owner, IssuerRecordKind::Enrolment, &bytes);
    let decoded = verify_detached(
        &root,
        IssuerRecordKind::Enrolment,
        &bytes,
        &w.identity.bytes,
        &sig,
    )
    .unwrap();
    assert_eq!(decoded, IssuerPayload::Enrolment(w.enrolment));
}

#[test]
fn absent_windows_are_omitted_never_null() {
    let w = world();
    let mut e = w.enrolment.clone();
    e.not_before = None;
    let v = cbor::decode(&e.canonical_bytes()).unwrap();
    let m = v.as_map().unwrap();
    assert!(!m.contains_key("not_before") && !m.contains_key("not_after"));
    // An explicit null is refused.
    let mut with_null = m.clone();
    with_null.insert("not_after".into(), Value::Null);
    assert!(Enrolment::decode(&cbor::encode(&Value::Map(with_null))).is_err());
    // Without not_before, the identity fact's at is the authorization time.
    assert_eq!(
        e.authorization_time(Some(&w.identity)).unwrap(),
        w.identity.at
    );
    assert!(e.authorization_time(None).is_err());
}

#[test]
fn a_modified_enrolment_wrong_root_or_wrong_domain_refuses() {
    let w = world();
    let root = OwnerRootPublic::of(&w.owner);
    let good = w.enrolment.canonical_bytes();
    let sig = DetachedSignature::sign(&w.owner, IssuerRecordKind::Enrolment, &good);
    let ok = |bytes: &[u8], identity: &[u8], sig: &DetachedSignature, root: &OwnerRootPublic| {
        verify_detached(root, IssuerRecordKind::Enrolment, bytes, identity, sig).is_ok()
    };
    assert!(ok(&good, &w.identity.bytes, &sig, &root));

    let mut modified = Vec::new();
    let mut e = w.enrolment.clone();
    e.issuer_id = Hash::of(b"another identity");
    modified.push(e);
    let mut e = w.enrolment.clone();
    e.public_key = w.successor.public();
    modified.push(e);
    let mut e = w.enrolment.clone();
    e.not_before = Some(at(1, 0));
    modified.push(e);
    let mut e = w.enrolment.clone();
    e.not_after = Some(at(9_000, 0));
    modified.push(e);
    for e in modified {
        assert!(
            !ok(&e.canonical_bytes(), &w.identity.bytes, &sig, &root),
            "{e:?}"
        );
        // Even re-signed, a mismatched identity or key refuses on binding.
        let resigned =
            DetachedSignature::sign(&w.owner, IssuerRecordKind::Enrolment, &e.canonical_bytes());
        let binds = e.check_identity(&w.identity).is_ok();
        assert_eq!(
            ok(&e.canonical_bytes(), &w.identity.bytes, &resigned, &root),
            binds
        );
    }
    // A scope other than platform does not decode.
    let mut v = cbor::decode(&good).unwrap().as_map().unwrap().clone();
    v.insert("scope".into(), Value::text("tenant"));
    assert!(Enrolment::decode(&cbor::encode(&Value::Map(v))).is_err());
    // A different identity fact.
    let other = identity_created_fact(&w.online.public(), at(1, 0)).canonical_bytes();
    assert!(!ok(&good, &other, &sig, &root));
    // A different root.
    let wrong_root = OwnerRootPublic::of(&signer(OTHER_OWNER_LABEL));
    assert!(!ok(&good, &w.identity.bytes, &sig, &wrong_root));
    let by_other = DetachedSignature::sign(
        &signer(OTHER_OWNER_LABEL),
        IssuerRecordKind::Enrolment,
        &good,
    );
    assert!(!ok(&good, &w.identity.bytes, &by_other, &root));
}

#[test]
fn identical_bytes_under_another_domain_never_verify() {
    let w = world();
    let bytes = w.enrolment.canonical_bytes();
    let pk = w.owner.public();
    let domains = [
        DOMAIN_ISSUER_ENROLMENT,
        DOMAIN_ISSUER_ROTATION,
        DOMAIN_ISSUER_REVOCATION,
        DOMAIN_ENTRY,
        DOMAIN_ATTESTATION,
    ];
    for signed in domains {
        let sig = w.owner.sign(signed, &bytes);
        for checked in domains {
            assert_eq!(
                pk.verify_strict(checked, &bytes, &sig).is_ok(),
                signed == checked,
                "{signed:?} checked as {checked:?}"
            );
        }
    }
    // The detached record's kind is a selector, not a choice of domain: a
    // rotation-domain signature relabelled as an enrolment still fails.
    let mut relabelled = DetachedSignature::sign(&w.owner, IssuerRecordKind::Rotation, &bytes);
    relabelled.kind = IssuerRecordKind::Enrolment;
    assert!(
        relabelled
            .verify(&pk, IssuerRecordKind::Enrolment, &bytes)
            .is_err()
    );
    // And a record of one kind is refused when another is expected.
    let sig = DetachedSignature::sign(&w.owner, IssuerRecordKind::Enrolment, &bytes);
    assert!(
        sig.verify(&pk, IssuerRecordKind::Revocation, &bytes)
            .is_err()
    );
}

// ---------------------------------------------------------------------------
// Malformed records
// ---------------------------------------------------------------------------

#[test]
fn malformed_payloads_refuse_before_trust_evaluation() {
    let w = world();
    let good = w.enrolment.canonical_bytes();
    // Trailing bytes.
    let mut trailing = good.clone();
    trailing.push(0);
    assert!(Enrolment::decode(&trailing).is_err());
    // Noncanonical CBOR: a one-byte integer written in two bytes.
    let mut noncanonical = good.clone();
    let pos = noncanonical.iter().position(|b| *b == 0xa4).unwrap();
    assert_eq!(pos, 0, "the enrolment map has four members");
    noncanonical = [vec![0xb8, 0x04], noncanonical[1..].to_vec()].concat();
    assert!(Enrolment::decode(&noncanonical).is_err());
    // An unknown member.
    let mut m = cbor::decode(&good).unwrap().as_map().unwrap().clone();
    m.insert("extra".into(), Value::Int(1));
    assert!(Enrolment::decode(&cbor::encode(&Value::Map(m))).is_err());
    // A duplicate member: the canonical decoder refuses it.
    let dup: Vec<u8> = vec![0xa2, 0x61, b'a', 0x01, 0x61, b'a', 0x02];
    assert!(cbor::decode(&dup).is_err());
    // An unknown member inside an HLC.
    let mut m = cbor::decode(&good).unwrap().as_map().unwrap().clone();
    m.insert(
        "not_before".into(),
        at(0, 0).to_value().with("extra", Value::Int(0)).unwrap(),
    );
    assert!(Enrolment::decode(&cbor::encode(&Value::Map(m))).is_err());
    // An inverted window.
    let mut e = w.enrolment.clone();
    e.not_after = Some(at(0, 0));
    e.not_before = Some(at(1, 0));
    assert!(Enrolment::decode(&e.canonical_bytes()).is_err());
    // A JSON-text payload, or CBOR of a JSON string, is not the payload.
    let json = serde_json::to_vec(&w.enrolment.to_value().to_json()).unwrap();
    assert!(Enrolment::decode(&json).is_err());
    let as_string = cbor::encode(&Value::text(String::from_utf8(json).unwrap()));
    assert!(Enrolment::decode(&as_string).is_err());
    // A fact with an unsupported member or version.
    let mut f = w.rotation.to_fact();
    f.extra.insert("cosign".into(), Value::Null);
    assert!(Rotation::decode(&f.canonical_bytes()).is_err());
    let mut f = w.revocation.to_fact();
    f.v = 2;
    assert!(Revocation::decode(&f.canonical_bytes()).is_err());
    let mut f = w.rotation.to_fact();
    f.body
        .insert("toKeyId".into(), Value::text(Hash::of(b"x").to_hex()));
    assert!(
        Rotation::decode(&f.canonical_bytes()).is_err(),
        "key id mismatch"
    );
    let mut f = w.revocation.to_fact();
    f.body.insert("reason".into(), Value::text(""));
    assert!(Revocation::decode(&f.canonical_bytes()).is_err());
}

#[test]
fn malformed_keys_and_signatures_are_refused() {
    use statecraft_envelope::issuer::{strict_public_key, strict_signature};
    let w = world();
    let good = hex::encode(w.owner.public().0);
    assert!(strict_public_key(&good).is_ok());
    assert!(
        strict_public_key(&good.to_uppercase()).is_err(),
        "uppercase"
    );
    assert!(strict_public_key(&good[..62]).is_err(), "short");
    assert!(
        strict_public_key(&format!("{}zz", &good[..62])).is_err(),
        "non-hex"
    );
    // The identity point is small order: weak.
    let mut identity_point = [0u8; 32];
    identity_point[0] = 1;
    assert!(
        strict_public_key(&hex::encode(identity_point)).is_err(),
        "weak"
    );
    // A detached record carrying a weak key's signature is refused by the
    // verifier itself, not only by the parser.
    let weak = PublicKey(identity_point);
    let s = DetachedSignature::sign(&w.owner, IssuerRecordKind::Enrolment, b"x");
    assert!(
        weak.verify_strict(DOMAIN_ISSUER_ENROLMENT, b"x", &s.signature)
            .is_err()
    );

    let sig = w.owner.sign(DOMAIN_ISSUER_ENROLMENT, b"payload");
    let sig_hex = hex::encode(sig.0);
    assert!(strict_signature(&sig_hex).is_ok());
    assert!(strict_signature(&sig_hex.to_uppercase()).is_err());
    assert!(strict_signature(&sig_hex[..126]).is_err());
    // A noncanonical S (S + L) is refused by strict verification.
    const L: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10,
    ];
    let mut bumped = sig.0;
    let mut carry = 0u16;
    for i in 0..32 {
        let v = bumped[32 + i] as u16 + L[i] as u16 + carry;
        bumped[32 + i] = v as u8;
        carry = v >> 8;
    }
    assert!(
        w.owner
            .public()
            .verify_strict(DOMAIN_ISSUER_ENROLMENT, b"payload", &Signature(bumped))
            .is_err()
    );
    // A flipped bit.
    let mut flipped = sig.0;
    flipped[5] ^= 1;
    assert!(
        w.owner
            .public()
            .verify_strict(DOMAIN_ISSUER_ENROLMENT, b"payload", &Signature(flipped))
            .is_err()
    );
}

#[test]
fn records_refuse_unknown_and_duplicate_members() {
    let w = world();
    let rec = DetachedSignature::sign(&w.owner, IssuerRecordKind::Enrolment, b"p").to_bytes();
    assert!(DetachedSignature::parse(&rec).is_ok());
    let text = String::from_utf8(rec).unwrap();
    let extra = text.replacen('{', "{\n  \"note\": \"x\",", 1);
    assert!(DetachedSignature::parse(extra.as_bytes()).is_err());
    let dup = text.replacen('{', "{\n  \"kind\": \"enrolment\",", 1);
    assert!(DetachedSignature::parse(dup.as_bytes()).is_err());
    let v2 = text.replace("\"schema_version\": 1", "\"schema_version\": 2");
    assert!(DetachedSignature::parse(v2.as_bytes()).is_err());
    let kind = text.replace("\"enrolment\"", "\"attestation\"");
    assert!(DetachedSignature::parse(kind.as_bytes()).is_err());

    let public = String::from_utf8(OwnerRootPublic::of(&w.owner).to_bytes()).unwrap();
    assert!(OwnerRootPublic::parse(public.as_bytes()).is_ok());
    let wrong_id = public.replace(
        &w.owner.public().id().0.to_hex(),
        &Hash::of(b"other").to_hex(),
    );
    assert!(OwnerRootPublic::parse(wrong_id.as_bytes()).is_err());
    let scope = public.replace("\"platform\"", "\"user\"");
    assert!(OwnerRootPublic::parse(scope.as_bytes()).is_err());
}

// ---------------------------------------------------------------------------
// Root set, history and eligibility
// ---------------------------------------------------------------------------

#[test]
fn the_root_set_round_trips_strictly() {
    let w = world();
    let s = set(&w, true, true);
    let bytes = s.to_bytes();
    assert_eq!(RootSetV2::parse(&bytes).unwrap(), s);
    let text = String::from_utf8(bytes).unwrap();
    for bad in [
        text.replacen('{', "{\n  \"signed_by\": \"owner\",", 1),
        text.replacen('{', "{\n  \"origin\": \"pinned\",", 1),
        text.replace("\"origin\": \"pinned\"", "\"origin\": \"evidence-ledger\""),
        text.replace("\"schema_version\": 2", "\"schema_version\": 3"),
    ] {
        assert!(RootSetV2::parse(bad.as_bytes()).is_err(), "{bad}");
    }
}

#[test]
fn a_key_known_only_from_the_dag_is_unknown() {
    let w = world();
    let v = set(&w, false, false).verify().unwrap();
    let dag_only = signer(SECOND_ISSUER_LABEL).public().id();
    assert_eq!(
        v.eligibility(&dag_only, at(10, 0), "platform"),
        IssuerEligibility::Unknown
    );
    // An empty pinned set vouches for nobody.
    let empty = RootSetV2 {
        root_set_version: 1,
        owner_roots: vec![active_root(&w.owner)],
        enrolments: vec![],
        rotations: vec![],
        revocations: vec![],
    }
    .verify()
    .unwrap();
    assert_eq!(
        empty.eligibility(&w.online.public().id(), at(10, 0), "platform"),
        IssuerEligibility::Unknown
    );
    // An enrolment signed by a root the set does not pin refuses the set.
    let mut s = set(&w, false, false);
    s.owner_roots = vec![active_root(&signer(OTHER_OWNER_LABEL))];
    assert!(s.verify().is_err());
}

#[test]
fn signed_by_metadata_or_a_legacy_key_row_establishes_no_v2_enrolment() {
    let w = world();
    let legacy = serde_json::json!({
        "root_set_version": 3,
        "roots": [{
            "key_id": w.online.public().id().0.to_hex(),
            "public_key": hex::encode(w.online.public().0),
            "scope": "platform",
            "status": "active"
        }],
        "signed_by": "owner",
        "origin": "pinned"
    });
    let bytes = serde_json::to_vec(&legacy).unwrap();
    let doc = RootSetDocument::parse(&bytes).unwrap();
    assert!(matches!(doc, RootSetDocument::LegacyV1(_)));
    assert!(doc.v2().is_none());
    assert!(RootSetV2::parse(&bytes).is_err());
    // An unknown member in a legacy document is refused at the boundary.
    let mut extra = legacy.clone();
    extra["enrolments"] = serde_json::json!([]);
    assert!(RootSetDocument::parse(&serde_json::to_vec(&extra).unwrap()).is_err());
    // A V2 document goes to the V2 reader.
    let v2 = set(&w, false, false).to_bytes();
    assert!(RootSetDocument::parse(&v2).unwrap().v2().is_some());
}

#[test]
fn enrolment_window_boundaries_are_inclusive() {
    let w = world();
    let mut s = set(&w, false, false);
    let mut e = w.enrolment.clone();
    e.not_before = Some(at(100, 0));
    e.not_after = Some(at(200, 0));
    s.enrolments = vec![auth(&w.owner, IssuerPayload::Enrolment(e))];
    let v = s.verify().unwrap();
    let k = w.online.public().id();
    let el = |h| v.eligibility(&k, h, "platform");
    assert_eq!(
        el(at(99, u32::MAX)),
        IssuerEligibility::Refused("enrolment-not-yet-effective")
    );
    assert!(matches!(el(at(100, 0)), IssuerEligibility::Eligible { .. }));
    assert!(matches!(el(at(200, 0)), IssuerEligibility::Eligible { .. }));
    assert_eq!(
        el(at(200, 1)),
        IssuerEligibility::Refused("enrolment-expired")
    );
    assert_eq!(
        v.eligibility(&k, at(150, 0), "tenant"),
        IssuerEligibility::Refused("scope-mismatch")
    );
    // A V2 enrolment must carry not_before: no identity fact supplies its time.
    let mut s = set(&w, false, false);
    let mut e = w.enrolment.clone();
    e.not_before = None;
    s.enrolments = vec![auth(&w.owner, IssuerPayload::Enrolment(e))];
    assert!(s.verify().is_err());
}

#[test]
fn rotation_boundaries_keep_historical_signatures() {
    let w = world();
    let v = set(&w, true, false).verify().unwrap();
    let old = w.online.public().id();
    let new = w.successor.public().id();
    let eff = w.rotation.effective;
    let before = Hlc::new(eff.physical - 1, 0);
    assert!(matches!(
        v.eligibility(&old, before, "platform"),
        IssuerEligibility::Eligible { .. }
    ));
    assert_eq!(
        v.eligibility(&old, eff, "platform"),
        IssuerEligibility::Refused("key-rotated-out")
    );
    assert_eq!(
        v.eligibility(&old, at(5_000, 0), "platform"),
        IssuerEligibility::Refused("key-rotated-out")
    );
    assert_eq!(
        v.eligibility(&new, before, "platform"),
        IssuerEligibility::Refused("key-not-yet-effective")
    );
    let identity = w.identity.id;
    assert_eq!(
        v.eligibility(&new, eff, "platform"),
        IssuerEligibility::Eligible { identity }
    );
    assert_eq!(
        v.eligibility(&new, at(5_000, 0), "platform"),
        IssuerEligibility::Eligible { identity }
    );
}

#[test]
fn bad_rotation_histories_refuse_the_whole_set() {
    let w = world();
    let third = signer(SECOND_ISSUER_LABEL);
    let with_rotations = |rs: Vec<Rotation>| {
        let mut s = set(&w, false, false);
        s.rotations = rs
            .into_iter()
            .map(|r| auth(&w.owner, IssuerPayload::Rotation(r)))
            .collect();
        s.verify()
    };
    assert!(with_rotations(vec![w.rotation.clone()]).is_ok());
    // Wrong predecessor.
    let mut r = w.rotation.clone();
    r.from = third.public().id();
    assert!(with_rotations(vec![r]).is_err());
    // Identity without an enrolment (cross-identity).
    let mut r = w.rotation.clone();
    r.identity = Hash::of(b"another identity");
    assert!(with_rotations(vec![r]).is_err());
    // Duplicate and fork: two rotations from the same predecessor.
    assert!(with_rotations(vec![w.rotation.clone(), w.rotation.clone()]).is_err());
    let mut fork = w.rotation.clone();
    fork.to = third.public();
    fork.effective = at(2_000, 0);
    assert!(with_rotations(vec![w.rotation.clone(), fork]).is_err());
    // A cycle back to the enrolled key.
    let back = Rotation {
        identity: w.identity.id,
        from: w.successor.public().id(),
        to: w.online.public(),
        effective: at(2_000, 0),
    };
    assert!(with_rotations(vec![w.rotation.clone(), back]).is_err());
    // Effective not after the predecessor became effective.
    let mut r = w.rotation.clone();
    r.effective = at(0, 0);
    assert!(with_rotations(vec![r]).is_err());
    // A successor already enrolled for another identity.
    let mut s = set(&w, true, false);
    let other_fact = identity_created_fact(&w.successor.public(), at(0, 0));
    let other = Enrolment {
        issuer_id: Hash::of(&other_fact.canonical_bytes()),
        public_key: w.successor.public(),
        scope: "platform".into(),
        not_before: Some(at(0, 0)),
        not_after: None,
    };
    s.enrolments
        .push(auth(&w.owner, IssuerPayload::Enrolment(other)));
    assert!(s.verify().is_err());
    // A valid chain of two rotations in either array order.
    let second = Rotation {
        identity: w.identity.id,
        from: w.successor.public().id(),
        to: third.public(),
        effective: at(2_000, 0),
    };
    assert!(with_rotations(vec![second.clone(), w.rotation.clone()]).is_ok());
}

#[test]
fn revocation_takes_priority_from_since_and_nothing_resurrects_it() {
    let w = world();
    let v = set(&w, false, true).verify().unwrap();
    let k = w.online.public().id();
    let since = w.revocation.since;
    let before = Hlc::new(since.physical - 1, 0);
    assert!(matches!(
        v.eligibility(&k, before, "platform"),
        IssuerEligibility::Eligible { .. }
    ));
    assert_eq!(
        v.eligibility(&k, since, "platform"),
        IssuerEligibility::Refused("key-revoked")
    );
    assert_eq!(
        v.eligibility(&k, at(9_000, 0), "platform"),
        IssuerEligibility::Refused("key-revoked")
    );
    // A later enrolment of the same key, even as a new identity, refuses the set.
    let mut s = set(&w, false, true);
    let again = identity_created_fact(&w.online.public(), at(9_000, 0));
    let e = Enrolment {
        issuer_id: Hash::of(&again.canonical_bytes()),
        public_key: w.online.public(),
        scope: "platform".into(),
        not_before: Some(at(9_000, 0)),
        not_after: None,
    };
    s.enrolments
        .push(auth(&w.owner, IssuerPayload::Enrolment(e)));
    assert!(s.verify().is_err());
    // Conflicting revocations of one key.
    let mut s = set(&w, false, true);
    let mut r = w.revocation.clone();
    r.since = at(700, 0);
    s.revocations
        .push(auth(&w.owner, IssuerPayload::Revocation(r)));
    assert!(s.verify().is_err());
    // A revocation of a key outside the issuer's history.
    let mut s = set(&w, false, false);
    let mut r = w.revocation.clone();
    r.key = w.successor.public().id();
    s.revocations = vec![auth(&w.owner, IssuerPayload::Revocation(r))];
    assert!(s.verify().is_err());
    // Revoking the rotated-in key after rotation.
    let mut s = set(&w, true, false);
    let r = Revocation {
        identity: w.identity.id,
        key: w.successor.public().id(),
        since: at(3_000, 0),
        reason: "compromise".into(),
    };
    s.revocations = vec![auth(&w.owner, IssuerPayload::Revocation(r))];
    let v = s.verify().unwrap();
    let new = w.successor.public().id();
    assert!(matches!(
        v.eligibility(&new, at(2_999, 0), "platform"),
        IssuerEligibility::Eligible { .. }
    ));
    assert_eq!(
        v.eligibility(&new, at(3_000, 0), "platform"),
        IssuerEligibility::Refused("key-revoked")
    );
}

#[test]
fn a_root_authorizes_only_inside_its_window_and_before_its_compromise() {
    let w = world();
    let attempt = |root: OwnerRoot| {
        let mut s = set(&w, true, false);
        s.owner_roots = vec![root];
        s.verify()
    };
    let base = active_root(&w.owner);
    assert!(attempt(base.clone()).is_ok());
    // Outside its window at the enrolment's not_before.
    let mut r = base.clone();
    r.not_before = Some(at(1, 0));
    assert!(attempt(r).is_err());
    // Expired before the rotation's effective time.
    let mut r = base.clone();
    r.not_after = Some(at(999, 0));
    assert!(attempt(r).is_err());
    let mut r = base.clone();
    r.not_after = Some(at(1_000, 0));
    assert!(attempt(r).is_ok(), "inclusive");
    // Revoked: authorizes before since, never at or after.
    let mut r = base.clone();
    r.status = RootStatus::Revoked;
    r.since = Some(at(1_000, 0));
    assert!(attempt(r).is_err(), "rotation at since");
    let mut r = base.clone();
    r.status = RootStatus::Revoked;
    r.since = Some(at(1_000, 1));
    assert!(attempt(r).is_ok(), "both events precede since");
    // Excluded: always refuses.
    let mut r = base.clone();
    r.status = RootStatus::Excluded;
    assert!(attempt(r).is_err());
    // An invalid signature anywhere refuses the whole set.
    let mut s = set(&w, true, true);
    s.revocations[0].signature.0[0] ^= 1;
    assert!(s.verify().is_err());
    // Duplicate owner roots refuse.
    let mut s = set(&w, false, false);
    s.owner_roots.push(active_root(&w.owner));
    assert!(s.verify().is_err());
}

// ---------------------------------------------------------------------------
// Operator pin and digest
// ---------------------------------------------------------------------------

#[test]
fn the_operator_pin_refuses_rollback_replacement_and_omission() {
    let w = world();
    let full = set(&w, true, true).verify().unwrap();
    let pin = RootSetPin {
        root_set_version: full.root_set_version,
        digest: full.digest,
    };
    assert_eq!(
        admit_pinned(None, &pin, &full).unwrap(),
        PinTransition::Advanced {
            from: None,
            to: pin
        }
    );
    assert_eq!(
        admit_pinned(Some(&pin), &pin, &full).unwrap(),
        PinTransition::Unchanged
    );

    // An omitted revocation: every signature is valid, the set verifies, and
    // only the operator pin refuses it.
    let mut omitted = set(&w, true, false);
    let v = omitted.verify().unwrap();
    assert!(
        admit_pinned(Some(&pin), &pin, &v).is_err(),
        "digest is not the pin"
    );
    let same_version = RootSetPin {
        root_set_version: 7,
        digest: v.digest,
    };
    assert!(
        admit_pinned(Some(&pin), &same_version, &v).is_err(),
        "same-version replacement"
    );
    omitted.root_set_version = 6;
    let v = omitted.verify().unwrap();
    let older = RootSetPin {
        root_set_version: 6,
        digest: v.digest,
    };
    assert!(admit_pinned(Some(&pin), &older, &v).is_err(), "rollback");
    // A newer revision under its own pin advances, recording both digests.
    omitted.root_set_version = 8;
    let v = omitted.verify().unwrap();
    let newer = RootSetPin {
        root_set_version: 8,
        digest: v.digest,
    };
    assert_eq!(
        admit_pinned(Some(&pin), &newer, &v).unwrap(),
        PinTransition::Advanced {
            from: Some(pin),
            to: newer
        }
    );
}

#[test]
fn the_digest_names_every_signature_and_authorization() {
    let w = world();
    let base = set(&w, true, true);
    let d = base.digest();
    let mut sig = base.clone();
    sig.rotations[0].signature.0[63] ^= 1;
    assert_ne!(sig.digest(), d);
    let mut root = base.clone();
    root.rotations[0].root_key_id = KeyId(Hash::of(b"x"));
    assert_ne!(root.digest(), d);
    let mut dropped = base.clone();
    dropped.revocations.clear();
    assert_ne!(dropped.digest(), d);
    let mut version = base.clone();
    version.root_set_version += 1;
    assert_ne!(version.digest(), d);
    // The digest is BLAKE3 of the canonical encoding of the complete object.
    assert_eq!(d, Hash::of(&cbor::encode(&base.to_value())));
    // And reading the JSON back yields the same digest.
    assert_eq!(RootSetV2::parse(&base.to_bytes()).unwrap().digest(), d);
}

#[test]
fn candidates_are_checked_against_the_pinned_history_before_signing() {
    let w = world();
    let v = set(&w, false, false).verify().unwrap();
    let owner = w.owner.public().id();
    assert!(v.check_rotation(&owner, &w.rotation).is_ok());
    assert!(v.check_revocation(&owner, &w.revocation).is_ok());
    let other = signer(OTHER_OWNER_LABEL).public().id();
    assert!(v.check_rotation(&other, &w.rotation).is_err());
    let mut r = w.rotation.clone();
    r.from = w.successor.public().id();
    assert!(v.check_rotation(&owner, &r).is_err());
    let v = set(&w, true, true).verify().unwrap();
    assert!(
        v.check_rotation(&owner, &w.rotation).is_err(),
        "already applied"
    );
    assert!(
        v.check_revocation(&owner, &w.revocation).is_err(),
        "already revoked"
    );
}
