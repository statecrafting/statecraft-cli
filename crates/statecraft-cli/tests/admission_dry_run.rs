//! Spec 049 through the binary: `admission dry-run --bundle <file> --policy
//! <file>` against bundles assembled here from the envelope crate's own
//! constructors, with the policy's digest, relation and check trace read back
//! from the family envelope. Every seed is a conspicuously public, test-only
//! value.
//!
//! The decision and codes are compared with `admission::evaluate` on the same
//! inputs (I-1 at the verb), the trace's `overruled` marks are checked against
//! the decision (R-2), the relation words against what the bundle records
//! (R-1), and a run against a read-only home and target leaves both
//! byte-identical (I-2, V-1).

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

use serde_json::{Value, json};
use statecraft_envelope::admission::{AdmissionInput, Decision, evaluate, policy_eval_claim};
use statecraft_envelope::attestation::{
    Attestation, AttestationId, PredicateType, Principal, PrincipalKind, UnsignedAttestation,
};
use statecraft_envelope::bundle::{APPROVAL, EVIDENCE_VERDICT, POLICY_EVAL, policy_digest};
use statecraft_envelope::cbor::{cid_of, encode};
use statecraft_envelope::dimensions::{Admission, AdmissionPolicy};
use statecraft_envelope::entry::{Entry, EntryHash, UnsignedEntry};
use statecraft_envelope::fact::FactEnvelope;
use statecraft_envelope::hash::{Cid, Hash};
use statecraft_envelope::hlc::Hlc;
use statecraft_envelope::sign::Signer;
use statecraft_envelope::value::Value as Portable;
use statecraft_envelope::verdict::EvidenceVerdict;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

const T0: u64 = 1_790_000_000_000;

fn at(ms: u64) -> Hlc {
    Hlc::new(T0 + ms, 0)
}

fn issuer() -> Signer {
    Signer::from_seed(
        &Hash::of(b"statecraft TEST-ONLY dry-run platform issuer: public, never production").0,
    )
}

/// A scope written by the platform issuer: the bundle's parts.
struct Scope {
    issuer: Signer,
    entries: Vec<Entry>,
    objects: BTreeMap<String, Vec<u8>>,
    revision: Hash,
}

impl Scope {
    fn new() -> Scope {
        let mut body = BTreeMap::new();
        body.insert("repository".into(), Portable::text("11".repeat(32)));
        body.insert("commit".into(), Portable::text("c0ffee"));
        body.insert("tree".into(), Portable::Null);
        body.insert("base".into(), Portable::Null);
        body.insert("registeredBy".into(), Portable::text("sam"));
        let fact = FactEnvelope::new("revision.registered", 1, body);
        let mut s = Scope {
            issuer: issuer(),
            entries: Vec::new(),
            objects: BTreeMap::new(),
            revision: Hash::of(&fact.canonical_bytes()),
        };
        s.append(&fact);
        s
    }

    fn put(&mut self, cid: Cid, bytes: Vec<u8>) {
        self.objects.insert(cid.to_string_key(), bytes);
    }

    fn append(&mut self, fact: &FactEnvelope) {
        self.put(fact.cid(), fact.canonical_bytes());
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

    /// Issue an attestation about the revision and record it.
    fn attest(&mut self, predicate: &str, claim: Portable) -> Hash {
        let claim_cid = cid_of(&claim);
        self.put(claim_cid, encode(&claim));
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
        body.insert("attestation".into(), Portable::Link(a.object_cid()));
        body.insert("subject".into(), Portable::Bytes(self.revision.0.to_vec()));
        body.insert("predicate".into(), Portable::text(predicate));
        body.insert(
            "issuer".into(),
            Principal::new(PrincipalKind::Service, "platform").to_value(),
        );
        self.append(&FactEnvelope::new("attestation.issued", 1, body));
        a.id().0
    }

    /// The bundle as the cell would send it.
    fn bundle_json(&self, admission: Option<Hash>, decisions: Vec<Hash>) -> Value {
        let entries: Vec<Vec<u8>> = self.entries.iter().map(Entry::canonical_bytes).collect();
        json!({
            "revision": self.revision,
            "entries": entries,
            "objects": self.objects,
            "tombstones": Vec::<Vec<u8>>::new(),
            "root_set": Hash::of(b"root set"),
            "admission": admission,
            "decisions": decisions,
            "admissionWord": "recorded",
        })
    }
}

fn verdict_claim() -> Portable {
    statecraft_envelope::verdict::verify_artifact(
        &statecraft_envelope::reference::Reference::over_file_bytes("receipt", "1", b"bytes"),
        Some(b"bytes"),
        None,
        &statecraft_envelope::roots::RootSet::empty(),
    )
    .to_claim()
}

fn approval_claim(revision: &Hash, reviewer: &str) -> Portable {
    Portable::map()
        .with(
            "reviewer",
            Principal::new(PrincipalKind::Human, reviewer).to_value(),
        )
        .unwrap()
        .with("revision", Portable::text(revision.to_hex()))
        .unwrap()
        .with("basis", Portable::Array(vec![]))
        .unwrap()
        .with("issuedBy", Portable::text("platform"))
        .unwrap()
}

/// A scope with one verdict and one approval by `rae`; when `recorded` names
/// a policy, the admission the platform evaluated under it is recorded too.
/// Returns the bundle JSON and the inputs `evaluate` sees for it.
fn fixture(
    recorded: Option<&AdmissionPolicy>,
) -> (Value, Vec<(AttestationId, EvidenceVerdict)>, Vec<Decision>) {
    let mut s = Scope::new();
    let verdict = s.attest(EVIDENCE_VERDICT, verdict_claim());
    let revision = s.revision;
    let approval = s.attest(APPROVAL, approval_claim(&revision, "rae"));
    let verdicts = vec![(
        AttestationId(verdict),
        serde_json::from_value(verdict_claim().to_json()).unwrap(),
    )];
    let decisions = vec![Decision {
        id: AttestationId(approval),
        approves: true,
        reviewer: Principal::new(PrincipalKind::Human, "rae"),
    }];
    let admission = recorded.map(|policy| {
        let a = evaluate(
            policy,
            &AdmissionInput {
                verdicts: &verdicts,
                decisions: &decisions,
                submitter: &Principal::new(PrincipalKind::Human, "sam"),
            },
        );
        s.attest(
            POLICY_EVAL,
            policy_eval_claim(
                policy_digest(policy),
                &[AttestationId(verdict), AttestationId(approval)],
                &a,
            ),
        )
    });
    (
        s.bundle_json(admission, vec![approval]),
        verdicts,
        decisions,
    )
}

fn expected(
    policy: &AdmissionPolicy,
    verdicts: &[(AttestationId, EvidenceVerdict)],
    decisions: &[Decision],
) -> Admission {
    evaluate(
        policy,
        &AdmissionInput {
            verdicts,
            decisions,
            submitter: &Principal::new(PrincipalKind::Human, "sam"),
        },
    )
}

fn write(dir: &Path, name: &str, v: &Value) -> String {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_vec(v).unwrap()).unwrap();
    path.to_str().unwrap().to_string()
}

fn run(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("STATECRAFT_HOME", home)
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap()
}

fn dry_run(home: &Path, cwd: &Path, bundle: &str, policy: &str) -> (Output, Value) {
    let out = run(
        home,
        cwd,
        &[
            "admission",
            "dry-run",
            "--bundle",
            bundle,
            "--policy",
            policy,
            "--json",
        ],
    );
    let v = json_naming::from_output(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "not an envelope ({e}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    assert_eq!(v["verb"], "admission.dry-run");
    (out, v)
}

/// The deny codes in the trace, in order, as JSON.
fn denied(report: &Value) -> Vec<Value> {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["result"] == "deny")
        .map(|c| c["code"].clone())
        .collect()
}

fn results(report: &Value) -> Vec<&str> {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["result"].as_str().unwrap())
        .collect()
}

#[test]
fn the_recorded_policy_admits_and_every_check_allows() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let policy = AdmissionPolicy::permissive();
    let (bundle, verdicts, decisions) = fixture(Some(&policy));
    let b = write(target.path(), "bundle.json", &bundle);
    let p = write(
        target.path(),
        "policy.json",
        &serde_json::to_value(&policy).unwrap(),
    );
    let (out, v) = dry_run(home.path(), target.path(), &b, &p);
    assert_eq!(out.status.code(), Some(0), "{v}");
    let r = &v["report"];
    assert_eq!(r["decision"], "admit");
    assert_eq!(r["reasons"], json!([]));
    assert!(expected(&policy, &verdicts, &decisions).is_admit());
    // R-1: the digest, and that it is the recorded admission's policy.
    assert_eq!(r["policy"]["digest"], policy_digest(&policy).to_hex());
    assert_eq!(r["policy"]["relation"], "recorded");
    assert_eq!(
        r["policy"]["recordedPolicy"],
        policy_digest(&policy).to_hex()
    );
    assert_eq!(
        r["inputs"],
        json!({"submitter": "sam", "verdicts": 1, "decisions": 1})
    );
    // R-2: registration order, one verdict, nothing overruled on an admit.
    let ids: Vec<&str> = r["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "verdict/0/integrity",
            "verdict/0/signature",
            "verdict/0/issuer-trust",
            "verdict/0/subject-binding",
            "approver-is-submitter",
            "approval-count",
        ]
    );
    assert!(results(r).iter().all(|w| *w == "allow"), "{r}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("nothing was written"),
        "the summary says so"
    );
}

#[test]
fn a_proposed_policy_refuses_with_evaluates_codes_and_marks_allowing_checks_overruled() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let recorded = AdmissionPolicy::permissive();
    let proposed = AdmissionPolicy {
        require_artifacts: vec!["receipt".into(), "sbom".into()],
        min_approvals: 2,
        approver_may_not_be_submitter: true,
        ..AdmissionPolicy::strict()
    };
    let (bundle, verdicts, decisions) = fixture(Some(&recorded));
    let b = write(target.path(), "bundle.json", &bundle);
    let p = write(
        target.path(),
        "policy.json",
        &serde_json::to_value(&proposed).unwrap(),
    );
    let (out, v) = dry_run(home.path(), target.path(), &b, &p);
    // Section 3.2: a refusal is the answer, exit 1 as a finding.
    assert_eq!(out.status.code(), Some(1), "{v}");
    let r = &v["report"];
    assert_eq!(r["decision"], "refuse");
    let want = expected(&proposed, &verdicts, &decisions);
    assert!(!want.is_admit());
    // I-1 at the verb: the same codes in the same order and serialized form.
    assert_eq!(r["reasons"], serde_json::to_value(want.reasons()).unwrap());
    assert_eq!(denied(r), r["reasons"].as_array().unwrap().clone());
    // R-1: a different policy than the recorded admission's.
    assert_eq!(r["policy"]["digest"], policy_digest(&proposed).to_hex());
    assert_eq!(r["policy"]["relation"], "different");
    assert_eq!(
        r["policy"]["recordedPolicy"],
        policy_digest(&recorded).to_hex()
    );
    // R-2: two artifact checks lead; the present artifact allows and is
    // overruled, the absent one denies; no check reads `allow` on a refusal.
    let words = results(r);
    assert_eq!(words.len(), 2 + 4 + 2, "{r}");
    assert_eq!(words[0], "overruled");
    assert_eq!(words[1], "deny");
    assert_eq!(
        r["checks"][1]["code"],
        json!({"missing-required-artifact": {"evidence_type": "sbom"}})
    );
    assert_eq!(words[7], "deny", "two approvals required, one given");
    assert!(
        words.iter().all(|w| *w != "allow" && *w != "undecided"),
        "{r}"
    );
    assert!(words.contains(&"overruled"), "{r}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("refusal:"), "{stdout}");
}

#[test]
fn a_bundle_without_an_admission_says_none_and_still_answers() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (bundle, verdicts, decisions) = fixture(None);
    let policy = AdmissionPolicy {
        min_approvals: 1,
        ..AdmissionPolicy::permissive()
    };
    let b = write(target.path(), "bundle.json", &bundle);
    let p = write(
        target.path(),
        "policy.json",
        &serde_json::to_value(&policy).unwrap(),
    );
    let (out, v) = dry_run(home.path(), target.path(), &b, &p);
    let r = &v["report"];
    assert_eq!(r["policy"]["relation"], "none");
    assert_eq!(r["policy"]["recordedPolicy"], Value::Null);
    let want = expected(&policy, &verdicts, &decisions);
    assert_eq!(out.status.code(), Some(if want.is_admit() { 0 } else { 1 }));
    assert_eq!(r["reasons"], serde_json::to_value(want.reasons()).unwrap());
}

/// Every regular file under `dir`, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let path = e.unwrap().path();
            let rel = path
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                stack.push(path);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn chmod(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn a_dry_run_against_a_read_only_home_and_target_writes_nothing() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let policy = AdmissionPolicy::strict();
    let (bundle, _, _) = fixture(Some(&AdmissionPolicy::permissive()));
    let b = write(target.path(), "bundle.json", &bundle);
    let p = write(
        target.path(),
        "policy.json",
        &serde_json::to_value(&policy).unwrap(),
    );
    // A home with something in it, so a write would have somewhere to land.
    std::fs::write(home.path().join("projects.json"), "[]").unwrap();
    let before_home = snapshot(home.path());
    let before_target = snapshot(target.path());
    chmod(home.path(), 0o555);
    chmod(target.path(), 0o555);
    let (out, v) = dry_run(home.path(), target.path(), &b, &p);
    chmod(home.path(), 0o755);
    chmod(target.path(), 0o755);
    // Section 3.2: the strict policy refuses the unsigned verdict; that is
    // the answer, and the read-only trees did not stop it.
    assert_eq!(out.status.code(), Some(1), "{v}");
    assert_eq!(v["report"]["decision"], "refuse");
    assert_eq!(snapshot(home.path()), before_home, "the home changed");
    assert_eq!(snapshot(target.path()), before_target, "the target changed");
}

#[test]
fn unreadable_inputs_are_refused_with_the_existing_kinds() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (bundle, _, _) = fixture(None);
    let b = write(target.path(), "bundle.json", &bundle);
    let p = write(
        target.path(),
        "policy.json",
        &serde_json::to_value(&AdmissionPolicy::permissive()).unwrap(),
    );
    let missing = target.path().join("absent.json");
    let (out, v) = dry_run(home.path(), target.path(), missing.to_str().unwrap(), &p);
    assert_eq!(out.status.code(), Some(2), "{v}");
    assert_eq!(v["error"]["kind"], "not-found");

    let bad_policy = write(
        target.path(),
        "bad-policy.json",
        &json!({"min_approvals": "two"}),
    );
    let (out, v) = dry_run(home.path(), target.path(), &b, &bad_policy);
    assert_eq!(out.status.code(), Some(2), "{v}");
    assert_eq!(v["error"]["kind"], "schema");

    let bad_bundle = write(
        target.path(),
        "bad-bundle.json",
        &json!({"revision": "not hex"}),
    );
    let (out, v) = dry_run(home.path(), target.path(), &bad_bundle, &p);
    assert_eq!(out.status.code(), Some(2), "{v}");
    assert_eq!(v["error"]["kind"], "schema");

    // A bundle that parses but does not resolve: an object whose bytes do
    // not hash to its identifier.
    let mut tampered = bundle.clone();
    let key = tampered["objects"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    tampered["objects"][&key] = json!([0u8]);
    let t = write(target.path(), "tampered.json", &tampered);
    let (out, v) = dry_run(home.path(), target.path(), &t, &p);
    assert_eq!(out.status.code(), Some(2), "{v}");
    assert_eq!(v["error"]["kind"], "validation");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not resolve"),
        "{v}"
    );
}

#[test]
fn both_options_are_required_and_nothing_else_is_accepted() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    for args in [
        vec!["admission", "dry-run", "--json"],
        vec!["admission", "dry-run", "--bundle", "b.json", "--json"],
        vec![
            "admission",
            "dry-run",
            "--bundle",
            "b.json",
            "--policy",
            "p.json",
            "--root-set",
            "r",
            "--json",
        ],
    ] {
        let out = run(home.path(), target.path(), &args);
        assert_eq!(out.status.code(), Some(3), "{args:?}");
        assert!(out.stderr.is_empty(), "{args:?}");
        let v = json_naming::from_output(&out.stdout).unwrap();
        assert_eq!(v["verb"], "admission.dry-run");
        assert_eq!(v["error"]["kind"], "usage");
    }
    assert!(
        statecraft_cli::commands::help_text(&["admission".to_string()])
            .contains("admission dry-run  049-admission-dry-run"),
        "the help names the verb and its owning spec"
    );
}
