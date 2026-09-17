//! Golden vectors for the native shapes (the platform's spec 003 B-3; hqgit
//! 012's design).
//!
//! **Provisional, and the transfer into this repository did not change that.**
//! They freeze at the platform's first signed entry, under the platform's own
//! constitution VIII; no such entry exists. Spec 007 section 3.6 says what that
//! means here: until the freeze, a vector change is an ordinary change, and
//! nothing in this repository may cite these as a frozen interoperability
//! contract.
//!
//! `STATECRAFT_MINT_VECTORS=1 cargo test -p statecraft-envelope --test vectors`
//! writes them; without the variable the walk re-derives every field from
//! the recorded inputs and asserts byte identity.

use std::collections::BTreeMap;

use statecraft_envelope::attestation::{
    APPROVAL, PredicateType, Principal, PrincipalKind, UnsignedAttestation,
};
use statecraft_envelope::cbor::{cid_of, encode};
use statecraft_envelope::entry::{EntryHash, UnsignedEntry};
use statecraft_envelope::fact::{EraseReason, FactEnvelope, Tombstone, TombstoneScope};
use statecraft_envelope::hash::{Cid, Codec, Hash};
use statecraft_envelope::hlc::Hlc;
use statecraft_envelope::sign::Signer;
use statecraft_envelope::value::Value;

fn dir() -> String {
    format!("{}/testdata/vectors", env!("CARGO_MANIFEST_DIR"))
}

fn derive() -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    let seed = [42u8; 32];
    let signer = Signer::from_seed(&seed);
    let key_id = signer.public().id();

    // 1. A fact envelope: revision.registered.
    let mut body = BTreeMap::new();
    body.insert(
        "repository".into(),
        Value::Bytes(Hash::of(b"repo").0.to_vec()),
    );
    body.insert(
        "commit".into(),
        Value::map()
            .with("format", Value::text("sha1"))
            .unwrap()
            .with(
                "oid",
                Value::text("89abcdef0123456789abcdef0123456789abcdef"),
            )
            .unwrap(),
    );
    body.insert(
        "registered_by".into(),
        Principal::new(PrincipalKind::Human, "dana").to_value(),
    );
    let fact = FactEnvelope::new("revision.registered", 1, body);
    out.insert("fact-revision-registered".into(), serde_json::json!({
        "inputs": { "kind": fact.kind, "v": fact.v, "body": Value::Map(fact.body.clone()).to_json() },
        "canonical_bytes": hex::encode(fact.canonical_bytes()),
        "cid": fact.cid().to_string_key(),
    }));

    // 2. A genesis entry and a second entry.
    let genesis_payload = FactEnvelope::new("repository.registered", 1, {
        let mut b = BTreeMap::new();
        b.insert(
            "origin".into(),
            Value::text("https://github.com/acme/widget"),
        );
        b
    });
    let genesis = UnsignedEntry::new(
        vec![],
        key_id,
        Hlc::new(1_000, 0),
        genesis_payload.cid(),
        BTreeMap::new(),
    )
    .unwrap()
    .sign(&signer)
    .unwrap();
    let mut extra = BTreeMap::new();
    extra.insert("note".into(), Value::text("a field a newer writer added"));
    let second = UnsignedEntry::new(
        vec![genesis.hash()],
        key_id,
        Hlc::new(1_000, 1),
        fact.cid(),
        extra,
    )
    .unwrap()
    .sign(&signer)
    .unwrap();
    out.insert("entry-genesis".into(), serde_json::json!({
        "seed": hex::encode(seed),
        "inputs": { "parents": [], "hlc": { "physical": 1000, "logical": 0 }, "payload": genesis_payload.cid().to_string_key() },
        "unsigned_bytes": hex::encode(genesis.body.unsigned_bytes()),
        "signature": hex::encode(genesis.sig.0),
        "canonical_bytes": hex::encode(genesis.canonical_bytes()),
        "hash": genesis.hash().0.to_hex(),
    }));
    out.insert("entry-with-extra".into(), serde_json::json!({
        "seed": hex::encode(seed),
        "inputs": { "parents": [genesis.hash().0.to_hex()], "hlc": { "physical": 1000, "logical": 1 }, "payload": fact.cid().to_string_key(), "extra": { "note": "a field a newer writer added" } },
        "unsigned_bytes": hex::encode(second.body.unsigned_bytes()),
        "signature": hex::encode(second.sig.0),
        "canonical_bytes": hex::encode(second.canonical_bytes()),
        "hash": second.hash().0.to_hex(),
    }));

    // 3. An approval attestation.
    let claim = Value::map()
        .with("revision", Value::text(Hash::of(b"rev").to_hex()))
        .unwrap()
        .with(
            "reviewer",
            Principal::new(PrincipalKind::Human, "dana").to_value(),
        )
        .unwrap()
        .with(
            "basis",
            Value::Array(vec![Value::text(Hash::of(b"verdict").to_hex())]),
        )
        .unwrap();
    let att = UnsignedAttestation {
        subject: Hash::of(b"rev"),
        predicate: PredicateType::new(APPROVAL).unwrap(),
        issuer: Principal::new(PrincipalKind::Service, "platform-issuer"),
        claim: cid_of(&claim),
        at: Hlc::new(2_000, 0),
        extra: BTreeMap::new(),
    }
    .issue(&signer)
    .unwrap();
    out.insert("attestation-approval".into(), serde_json::json!({
        "seed": hex::encode(seed),
        "inputs": { "subject": Hash::of(b"rev").to_hex(), "predicate": APPROVAL, "issuer": { "kind": "service", "id": "platform-issuer" }, "claim": claim.to_json(), "at": { "physical": 2000, "logical": 0 } },
        "claim_canonical_bytes": hex::encode(encode(&claim)),
        "unsigned_bytes": hex::encode(att.unsigned_bytes()),
        "signature": hex::encode(att.sig.0),
        "canonical_bytes": hex::encode(att.canonical_bytes()),
        "id": att.id().0.to_hex(),
    }));

    // 4. A tombstone.
    let t = Tombstone {
        target: Cid {
            codec: Codec::Raw,
            hash: Hash::of(b"artifact"),
        },
        reason: EraseReason::Retention,
        scope: TombstoneScope::Payload(EntryHash(second.hash().0)),
    };
    out.insert("fact-tombstone".into(), serde_json::json!({
        "inputs": { "target": t.target.to_string_key(), "reason": "retention", "scope": { "payload": second.hash().0.to_hex() } },
        "canonical_bytes": hex::encode(t.to_fact().canonical_bytes()),
        "cid": t.to_fact().cid().to_string_key(),
    }));
    out
}

#[test]
fn walk() {
    let derived = derive();
    if std::env::var("STATECRAFT_MINT_VECTORS").is_ok() {
        std::fs::create_dir_all(dir()).unwrap();
        for (name, v) in &derived {
            std::fs::write(
                format!("{}/{name}.json", dir()),
                serde_json::to_string_pretty(v).unwrap() + "\n",
            )
            .unwrap();
        }
        return;
    }
    let mut walked = 0;
    for (name, v) in &derived {
        let path = format!("{}/{name}.json", dir());
        let recorded: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing vector {path}")),
        )
        .unwrap();
        assert_eq!(
            &recorded, v,
            "vector {name} diverged from its re-derivation"
        );
        walked += 1;
    }
    assert_eq!(walked, 5, "every vector walked");
}

#[test]
fn every_vector_decodes_to_the_value_that_produced_it() {
    for (name, v) in derive() {
        if let Some(b) = v.get("canonical_bytes").and_then(|x| x.as_str()) {
            let bytes = hex::decode(b).unwrap();
            let decoded =
                statecraft_envelope::cbor::decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                encode(&decoded),
                bytes,
                "{name}: encode of decode is the identity"
            );
        }
    }
}
