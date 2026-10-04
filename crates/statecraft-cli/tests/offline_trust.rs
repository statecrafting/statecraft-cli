//! Spec 036: the offline trust verbs, through the built binary.
//!
//! Every seed is the public, labelled TEST-ONLY seed frozen with spec 035's
//! vectors in `crates/statecraft-envelope/testdata/vectors/issuer/`. No test
//! signs real material, and `root-generate` writes only into a temporary
//! directory that is deleted afterwards.

#[path = "support/json_naming.rs"]
mod json_naming;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use statecraft_envelope::issuer::{
    Authorization, Enrolment, IssuerPayload, OwnerRoot, OwnerRootPublic, RootSetV2,
};
use statecraft_envelope::roots::RootStatus;
use statecraft_envelope::sign::Signer;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_statecraft-cli"))
}

fn vectors() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../statecraft-envelope/testdata/vectors/issuer")
}

fn vector(name: &str) -> Vec<u8> {
    std::fs::read(vectors().join(name)).unwrap()
}

fn seed_text() -> String {
    String::from_utf8(vector("test-only-owner-root.seed")).unwrap()
}

fn test_signer() -> Signer {
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&hex::decode(seed_text().trim_end()).unwrap());
    Signer::from_seed(&seed)
}

fn owner_key_id() -> String {
    test_signer().public().id().0.to_hex()
}

/// A scratch directory with no home, so a verb that read one would fail.
struct Scratch {
    dir: tempfile::TempDir,
}

impl Scratch {
    fn new() -> Self {
        Scratch {
            dir: tempfile::tempdir().unwrap(),
        }
    }
    fn p(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
    fn s(&self, name: &str) -> String {
        self.p(name).display().to_string()
    }
    fn put(&self, name: &str, bytes: &[u8], mode: u32) -> String {
        std::fs::write(self.p(name), bytes).unwrap();
        std::fs::set_permissions(self.p(name), std::fs::Permissions::from_mode(mode)).unwrap();
        self.s(name)
    }
    fn vector(&self, name: &str) -> String {
        self.put(name, &vector(name), 0o644)
    }
    fn seed(&self) -> String {
        self.put("owner.seed", seed_text().as_bytes(), 0o600)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(binary())
            .args(args)
            .current_dir(self.dir.path())
            .env("STATECRAFT_HOME", self.p("no-home"))
            .env("HOME", self.p("no-home"))
            .output()
            .expect("the binary runs")
    }
    fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn json(o: &Output) -> Value {
    json_naming::from_output(&o.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stdout)))
}

fn leaks(o: &Output, secret: &str) -> bool {
    let all = [o.stdout.as_slice(), o.stderr.as_slice()].concat();
    String::from_utf8_lossy(&all).contains(secret)
}

fn payload_digest(bytes: &[u8]) -> String {
    statecraft_envelope::hash::Hash::of(bytes).to_hex()
}

/// A pinned history holding only the vector enrollment, for signing a
/// rotation or revocation against.
fn enrolment_only_history() -> Vec<u8> {
    let owner = test_signer();
    let e = Enrolment::decode(&vector("enrolment.cbor")).unwrap();
    let payload = IssuerPayload::Enrolment(e);
    let signature = owner.sign(payload.kind().domain(), &payload.canonical_bytes());
    RootSetV2 {
        root_set_version: 6,
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
    .to_bytes()
}

fn sign_args<'a>(s: &'a [String; 5], kind: &'a str, root: &'a str) -> Vec<&'a str> {
    vec![
        "trust",
        "issuer-sign",
        "--kind",
        kind,
        "--seed-file",
        &s[0],
        "--root-key-id",
        root,
        "--request",
        &s[1],
        "--identity",
        &s[2],
        "--out",
        &s[3],
    ]
}

// ---------------------------------------------------------------------------
// Golden vectors across the shared verifier and the offline command
// ---------------------------------------------------------------------------

#[test]
fn issuer_sign_reproduces_the_frozen_signature_bytes_for_every_kind() {
    for kind in ["enrolment", "rotation", "revocation"] {
        let sc = Scratch::new();
        let request = format!("{kind}.cbor");
        let paths = [
            sc.seed(),
            sc.vector(&request),
            sc.vector("identity.created.cbor"),
            sc.s("signature.json"),
            String::new(),
        ];
        let root = owner_key_id();
        let digest = payload_digest(&vector(&request));
        let mut args = sign_args(&paths, kind, &root);
        let history;
        if kind != "enrolment" {
            history = sc.put("history.json", &enrolment_only_history(), 0o644);
            args.extend(["--history-root-set", &history]);
        }
        args.extend(["--confirm", &digest, "--json"]);
        let o = sc.run(&args);
        assert_eq!(
            code(&o),
            0,
            "{kind}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
        let v = json(&o);
        assert_eq!(v["verb"], "trust.issuer-sign");
        assert_eq!(v["report"]["payloadDigest"], digest);
        assert_eq!(
            std::fs::read(sc.p("signature.json")).unwrap(),
            vector(&format!("{kind}.signature.json")),
            "{kind}: the command's bytes differ from the frozen vector"
        );
        assert!(!leaks(&o, seed_text().trim_end()));
    }
}

#[test]
fn issuer_verify_accepts_the_frozen_vectors_and_refuses_a_mismatch() {
    let sc = Scratch::new();
    let root = sc.vector("owner-root.public.json");
    let identity = sc.vector("identity.created.cbor");
    for kind in ["enrolment", "rotation", "revocation"] {
        let request = sc.vector(&format!("{kind}.cbor"));
        let sig = sc.vector(&format!("{kind}.signature.json"));
        let o = sc.run(&[
            "trust",
            "issuer-verify",
            "--kind",
            kind,
            "--root-public",
            &root,
            "--request",
            &request,
            "--identity",
            &identity,
            "--signature",
            &sig,
            "--json",
        ]);
        assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
        assert_eq!(json(&o)["report"]["verdict"], "verified");
    }
    // A signature of another kind over other bytes: the payload is read as
    // the kind asked for, and an enrollment is not a rotation fact.
    let o = sc.run(&[
        "trust",
        "issuer-verify",
        "--kind",
        "rotation",
        "--root-public",
        &root,
        "--request",
        &sc.s("enrolment.cbor"),
        "--identity",
        &identity,
        "--signature",
        &sc.s("enrolment.signature.json"),
        "--json",
    ]);
    assert_eq!(code(&o), 2);
    // The right payload with a signature record of another kind.
    let o = sc.run(&[
        "trust",
        "issuer-verify",
        "--kind",
        "rotation",
        "--root-public",
        &root,
        "--request",
        &sc.s("rotation.cbor"),
        "--identity",
        &identity,
        "--signature",
        &sc.s("revocation.signature.json"),
        "--json",
    ]);
    assert_eq!(code(&o), 1);
    assert_eq!(json(&o)["report"]["verdict"], "not-verified");
    // A tampered signature.
    let mut tampered: Value = serde_json::from_slice(&vector("enrolment.signature.json")).unwrap();
    let s = tampered["signature"].as_str().unwrap().to_string();
    let flipped = format!("{}{}", if &s[..1] == "0" { "1" } else { "0" }, &s[1..]);
    tampered["signature"] = Value::from(flipped);
    let bad = sc.put("bad.json", &serde_json::to_vec(&tampered).unwrap(), 0o644);
    let o = sc.run(&[
        "trust",
        "issuer-verify",
        "--kind",
        "enrolment",
        "--root-public",
        &root,
        "--request",
        &sc.s("enrolment.cbor"),
        "--identity",
        &identity,
        "--signature",
        &bad,
        "--json",
    ]);
    assert_eq!(code(&o), 1);
    // A different pinned root: the key comes from the pin, never the signature.
    let other = Signer::from_seed(&[9u8; 32]);
    let other_root = sc.put("other.json", &OwnerRootPublic::of(&other).to_bytes(), 0o644);
    let o = sc.run(&[
        "trust",
        "issuer-verify",
        "--kind",
        "enrolment",
        "--root-public",
        &other_root,
        "--request",
        &sc.s("enrolment.cbor"),
        "--identity",
        &identity,
        "--signature",
        &sc.s("enrolment.signature.json"),
        "--json",
    ]);
    assert_eq!(code(&o), 1);
}

#[test]
fn full_eligibility_answers_at_an_explicit_time_under_the_operator_pin() {
    let sc = Scratch::new();
    let root = sc.vector("owner-root.public.json");
    let identity = sc.vector("identity.created.cbor");
    let request = sc.vector("enrolment.cbor");
    let sig = sc.vector("enrolment.signature.json");
    let history = sc.vector("root-set-v2.json");
    let summary: Value = serde_json::from_slice(&vector("vectors.json")).unwrap();
    let digest = summary["root_set_digest"].as_str().unwrap().to_string();
    let run = |at: &str, pin_version: &str, pin_digest: &str| {
        sc.run(&[
            "trust",
            "issuer-verify",
            "--kind",
            "enrolment",
            "--root-public",
            &root,
            "--request",
            &request,
            "--identity",
            &identity,
            "--signature",
            &sig,
            "--history-root-set",
            &history,
            "--pin-version",
            pin_version,
            "--pin-digest",
            pin_digest,
            "--at-physical",
            at,
            "--at-logical",
            "0",
            "--scope",
            "platform",
            "--json",
        ])
    };
    // The vector set revokes the enrolled key at T0 + 500 and rotates at T0 + 1000.
    let o = run("1790000000499", "7", &digest);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(json(&o)["report"]["eligibility"]["issuerTrust"], "pass");
    let o = run("1790000000500", "7", &digest);
    assert_eq!(code(&o), 1);
    let v = json(&o);
    assert_eq!(v["report"]["eligibility"]["issuerTrust"], "fail");
    assert_eq!(v["report"]["eligibility"]["reason"], "key-revoked");
    // The operator pin refuses another version or digest.
    assert_eq!(code(&run("1790000000499", "6", &digest)), 2);
    assert_eq!(code(&run("1790000000499", "7", &"0".repeat(64))), 2);
    // The eligibility options come together or not at all.
    let o = sc.run(&[
        "trust",
        "issuer-verify",
        "--kind",
        "enrolment",
        "--root-public",
        &root,
        "--request",
        &request,
        "--identity",
        &identity,
        "--signature",
        &sig,
        "--history-root-set",
        &history,
        "--json",
    ]);
    assert_eq!(code(&o), 3);
}

// ---------------------------------------------------------------------------
// root-generate
// ---------------------------------------------------------------------------

#[test]
fn root_generate_refuses_without_its_explicit_confirmation() {
    let sc = Scratch::new();
    let o = sc.run(&[
        "trust",
        "root-generate",
        "--out",
        &sc.s("root.seed"),
        "--public-out",
        &sc.s("root.public.json"),
        "--json",
    ]);
    assert_eq!(code(&o), 2);
    assert_eq!(json(&o)["error"]["kind"], "refused");
    assert!(
        sc.names().is_empty(),
        "nothing is written: {:?}",
        sc.names()
    );
}

#[test]
fn root_generate_writes_the_seed_only_to_out_with_owner_only_permissions() {
    let sc = Scratch::new();
    let o = sc.run(&[
        "trust",
        "root-generate",
        "--out",
        &sc.s("root.seed"),
        "--public-out",
        &sc.s("root.public.json"),
        "--confirm-new-root",
        "--json",
    ]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(sc.names(), vec!["root.public.json", "root.seed"]);
    let seed = std::fs::read(sc.p("root.seed")).unwrap();
    assert_eq!(seed.len(), 65);
    assert_eq!(seed[64], b'\n');
    assert!(
        seed[..64]
            .iter()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    );
    let mode = std::fs::metadata(sc.p("root.seed"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let seed_hex = std::str::from_utf8(&seed[..64]).unwrap();
    assert!(!leaks(&o, seed_hex), "the seed reached stdout or stderr");
    let public = OwnerRootPublic::parse(&std::fs::read(sc.p("root.public.json")).unwrap()).unwrap();
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&hex::decode(seed_hex).unwrap());
    assert_eq!(public.public_key, Signer::from_seed(&raw).public());
    assert!(
        !String::from_utf8_lossy(&std::fs::read(sc.p("root.public.json")).unwrap())
            .contains(seed_hex)
    );
    let v = json(&o);
    assert_eq!(v["report"]["keyId"], public.key_id().0.to_hex());

    // A second run never overwrites custody files.
    let before = std::fs::read(sc.p("root.seed")).unwrap();
    let o = sc.run(&[
        "trust",
        "root-generate",
        "--out",
        &sc.s("root.seed"),
        "--public-out",
        &sc.s("other.json"),
        "--confirm-new-root",
    ]);
    assert_eq!(code(&o), 2);
    assert_eq!(std::fs::read(sc.p("root.seed")).unwrap(), before);
    assert!(!sc.p("other.json").exists());
}

#[test]
fn root_generate_refuses_links_missing_directories_and_one_path_twice() {
    let sc = Scratch::new();
    std::os::unix::fs::symlink(sc.p("elsewhere"), sc.p("link.seed")).unwrap();
    let o = sc.run(&[
        "trust",
        "root-generate",
        "--out",
        &sc.s("link.seed"),
        "--public-out",
        &sc.s("p.json"),
        "--confirm-new-root",
    ]);
    assert_eq!(code(&o), 2);
    assert!(!sc.p("elsewhere").exists());
    let o = sc.run(&[
        "trust",
        "root-generate",
        "--out",
        &sc.s("absent/root.seed"),
        "--public-out",
        &sc.s("p.json"),
        "--confirm-new-root",
    ]);
    assert_eq!(code(&o), 2);
    let o = sc.run(&[
        "trust",
        "root-generate",
        "--out",
        &sc.s("same"),
        "--public-out",
        &sc.s("same"),
        "--confirm-new-root",
    ]);
    assert_eq!(code(&o), 3);
    let o = sc.run(&[
        "trust",
        "root-generate",
        "--out",
        &sc.s("x"),
        "--confirm-new-root",
    ]);
    assert_eq!(code(&o), 3, "missing --public-out");
    let o = sc.run(&[
        "trust",
        "root-generate",
        "--out",
        &sc.s("x"),
        "--out",
        &sc.s("y"),
        "--public-out",
        &sc.s("z"),
        "--confirm-new-root",
    ]);
    assert_eq!(code(&o), 3, "repeated option");
    assert_eq!(sc.names(), vec!["link.seed"]);
}

// ---------------------------------------------------------------------------
// issuer-sign refusals
// ---------------------------------------------------------------------------

#[test]
fn issuer_sign_shows_the_public_fields_and_refuses_without_confirmation() {
    let sc = Scratch::new();
    let paths = [
        sc.seed(),
        sc.vector("enrolment.cbor"),
        sc.vector("identity.created.cbor"),
        sc.s("signature.json"),
        String::new(),
    ];
    let root = owner_key_id();
    let digest = payload_digest(&vector("enrolment.cbor"));
    let mut args = sign_args(&paths, "enrolment", &root);
    args.push("--json");
    let o = sc.run(&args);
    assert_eq!(code(&o), 2);
    let v = json(&o);
    let details = &v["error"]["details"];
    assert_eq!(details["payloadDigest"], digest);
    assert_eq!(details["rootKeyId"], root);
    assert_eq!(details["fields"]["scope"], "platform");
    assert!(
        details["fields"]["notAfter"].is_null(),
        "the current renderer is unbounded"
    );
    assert!(!sc.p("signature.json").exists());
    assert!(!leaks(&o, seed_text().trim_end()));
    // A confirmation naming another digest is refused too.
    let mut args = sign_args(&paths, "enrolment", &root);
    let wrong = "0".repeat(64);
    args.extend(["--confirm", &wrong]);
    assert_eq!(code(&sc.run(&args)), 2);
    assert!(!sc.p("signature.json").exists());
}

#[test]
fn issuer_sign_refuses_unsafe_or_noncanonical_seed_files_without_leaking_them() {
    let digest = payload_digest(&vector("enrolment.cbor"));
    let root = owner_key_id();
    let seed = seed_text();
    let cases: Vec<(&str, Vec<u8>, u32)> = vec![
        ("group-readable", seed.as_bytes().to_vec(), 0o640),
        ("world-readable", seed.as_bytes().to_vec(), 0o644),
        ("uppercase", seed.to_uppercase().into_bytes(), 0o600),
        ("no-newline", seed.trim_end().as_bytes().to_vec(), 0o600),
        (
            "crlf",
            format!("{}\r\n", seed.trim_end()).into_bytes(),
            0o600,
        ),
        ("trailing", format!("{seed}\n").into_bytes(), 0o600),
    ];
    for (name, bytes, mode) in cases {
        let sc = Scratch::new();
        let seed_path = sc.put("owner.seed", &bytes, mode);
        let paths = [
            seed_path,
            sc.vector("enrolment.cbor"),
            sc.vector("identity.created.cbor"),
            sc.s("signature.json"),
            String::new(),
        ];
        let mut args = sign_args(&paths, "enrolment", &root);
        args.extend(["--confirm", &digest, "--json"]);
        let o = sc.run(&args);
        assert_eq!(
            code(&o),
            2,
            "{name}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
        assert!(!sc.p("signature.json").exists(), "{name}");
        assert!(!leaks(&o, seed.trim_end()), "{name} leaked the seed");
        assert!(
            !leaks(&o, &seed.trim_end().to_uppercase()),
            "{name} leaked the seed"
        );
    }
    // A symbolic link to a valid seed file.
    let sc = Scratch::new();
    let real = sc.seed();
    std::os::unix::fs::symlink(&real, sc.p("link.seed")).unwrap();
    let paths = [
        sc.s("link.seed"),
        sc.vector("enrolment.cbor"),
        sc.vector("identity.created.cbor"),
        sc.s("signature.json"),
        String::new(),
    ];
    let mut args = sign_args(&paths, "enrolment", &root);
    args.extend(["--confirm", &digest]);
    assert_eq!(code(&sc.run(&args)), 2);
    assert!(!sc.p("signature.json").exists());
}

#[test]
fn issuer_sign_refuses_existing_outputs_wrong_roots_and_unbound_requests() {
    let digest = payload_digest(&vector("enrolment.cbor"));
    let root = owner_key_id();
    // An existing output is never overwritten.
    let sc = Scratch::new();
    let existing = sc.put("signature.json", b"keep\n", 0o644);
    let paths = [
        sc.seed(),
        sc.vector("enrolment.cbor"),
        sc.vector("identity.created.cbor"),
        existing,
        String::new(),
    ];
    let mut args = sign_args(&paths, "enrolment", &root);
    args.extend(["--confirm", &digest]);
    assert_eq!(code(&sc.run(&args)), 2);
    assert_eq!(std::fs::read(sc.p("signature.json")).unwrap(), b"keep\n");

    // A seed that is not the named root.
    let sc = Scratch::new();
    let paths = [
        sc.seed(),
        sc.vector("enrolment.cbor"),
        sc.vector("identity.created.cbor"),
        sc.s("signature.json"),
        String::new(),
    ];
    let other = "1".repeat(64);
    let mut args = sign_args(&paths, "enrolment", &other);
    args.extend(["--confirm", &digest]);
    assert_eq!(code(&sc.run(&args)), 2);
    assert!(!sc.p("signature.json").exists());

    // A request bound to another identity fact.
    let sc = Scratch::new();
    let other_identity = statecraft_envelope::issuer::identity_created_fact(
        &test_signer().public(),
        statecraft_envelope::hlc::Hlc::new(1, 0),
    )
    .canonical_bytes();
    let paths = [
        sc.seed(),
        sc.vector("enrolment.cbor"),
        sc.put("identity.cbor", &other_identity, 0o644),
        sc.s("signature.json"),
        String::new(),
    ];
    let mut args = sign_args(&paths, "enrolment", &root);
    args.extend(["--confirm", &digest]);
    assert_eq!(code(&sc.run(&args)), 2);

    // Kind and history option rules are usage errors.
    let sc = Scratch::new();
    let paths = [
        sc.seed(),
        sc.vector("rotation.cbor"),
        sc.vector("identity.created.cbor"),
        sc.s("signature.json"),
        String::new(),
    ];
    assert_eq!(code(&sc.run(&sign_args(&paths, "rotation", &root))), 3);
    let history = sc.put("history.json", &enrolment_only_history(), 0o644);
    let mut args = sign_args(&paths, "enrolment", &root);
    args.extend(["--history-root-set", &history]);
    assert_eq!(code(&sc.run(&args)), 3);
    assert_eq!(code(&sc.run(&sign_args(&paths, "attestation", &root))), 3);

    // A rotation the pinned history already holds is refused before signing.
    let full = sc.vector("root-set-v2.json");
    let rdigest = payload_digest(&vector("rotation.cbor"));
    let mut args = sign_args(&paths, "rotation", &root);
    args.extend(["--history-root-set", &full, "--confirm", &rdigest]);
    assert_eq!(code(&sc.run(&args)), 2);
    assert!(!sc.p("signature.json").exists());
}

#[test]
fn the_trust_verbs_read_no_home_and_write_nothing_else() {
    let sc = Scratch::new();
    let paths = [
        sc.seed(),
        sc.vector("enrolment.cbor"),
        sc.vector("identity.created.cbor"),
        sc.s("signature.json"),
        String::new(),
    ];
    let root = owner_key_id();
    let digest = payload_digest(&vector("enrolment.cbor"));
    let mut args = sign_args(&paths, "enrolment", &root);
    args.extend(["--confirm", &digest]);
    let o = sc.run(&args);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(
        sc.names(),
        vec![
            "enrolment.cbor",
            "identity.created.cbor",
            "owner.seed",
            "signature.json"
        ]
    );
    assert!(!Path::new(&sc.p("no-home")).exists());
}
