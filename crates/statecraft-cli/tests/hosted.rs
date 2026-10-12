//! Spec 038 through the binary: `hosted login`, `logout`, `submit` and
//! `decision` against a scripted cell on a loopback port, with a committed
//! `.statecraft/hosted.json` in a fixture project. Section 3.7's negative
//! cases that need the binary are here; the crate's own suites hold the rest.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

#[path = "../../statecraft-hosted/tests/support/cell.rs"]
mod cell;

use cell::{Answer, Cell, discovery, json as answer, jwt};
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

const REPO: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const REV: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const REFRESH: &str = "refresh-secret-0038";
const DEVICE: &str = "device-secret-0038";

struct Project {
    root: tempfile::TempDir,
    home: tempfile::TempDir,
}

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

impl Project {
    fn new(origin: &str) -> Project {
        let p = Project {
            root: tempfile::tempdir().unwrap(),
            home: tempfile::tempdir().unwrap(),
        };
        let root = p.root.path();
        git(root, &["init", "--quiet"]);
        git(root, &["config", "user.name", "fixture"]);
        git(root, &["config", "user.email", "fixture@example.com"]);
        std::fs::write(root.join("a.txt"), "a").unwrap();
        git(root, &["add", "a.txt"]);
        git(root, &["commit", "--quiet", "-m", "base"]);
        std::fs::create_dir_all(root.join(".statecraft")).unwrap();
        std::fs::write(
            root.join(".statecraft/hosted.json"),
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": 1,
                "cell": origin,
                "repository": REPO,
                "clientId": "statecraft-cli",
                "scopes": ["statecraft:submit", "statecraft:read"],
            }))
            .unwrap(),
        )
        .unwrap();
        p
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home.path())
            .env("HOME", self.home.path())
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap()
    }

    fn path(&self) -> &str {
        self.root.path().to_str().unwrap()
    }

    fn head(&self) -> String {
        git(self.root.path(), &["rev-parse", "HEAD"])
    }

    /// A saved `accept --json` answer carrying a receipt for HEAD.
    fn accept_answer(&self) -> std::path::PathBuf {
        let receipt = statecraft_acceptance::receipt::Receipt {
            repository: self.path().into(),
            base: self.head(),
            candidate: self.head(),
            suite: vec![],
            confinement: None,
            policy_digest: "policy".into(),
            product_version: "0.0.0".into(),
            spec_spine_version: "0.29.0".into(),
            adapter_version: "0.0.0".into(),
            harness_revision: statecraft_acceptance::absence::Recorded::Present("h-1".into()),
            attempt: "replay/1".into(),
            authority_paths_touched: vec![],
            judge: None,
        };
        let file = self.home.path().join("accept.json");
        std::fs::write(
            &file,
            serde_json::to_vec(&json!({
                "verb": "accept",
                "exitCode": 0,
                "report": {"acceptance": "accepted", "receipt": receipt},
            }))
            .unwrap(),
        )
        .unwrap();
        file
    }
}

fn token_response(origin: &str) -> Value {
    json!({
        "access_token": jwt("RS256", json!([origin])),
        "token_type": "Bearer",
        "expires_in": 3600,
        "refresh_token": REFRESH,
    })
}

/// A cell that discovers, grants at once and answers the API from `api`.
fn cell(api: impl Fn(&str, &cell::Request, usize) -> Answer + Send + Sync + 'static) -> Cell {
    Cell::start(Box::new(move |origin, req, n| {
        if let Some(a) = discovery(origin, &req.path) {
            return a;
        }
        match req.path.as_str() {
            "/auth/v1/oidc/device" => answer(
                200,
                json!({
                    "device_code": DEVICE,
                    "user_code": "ABCD-0038",
                    "verification_uri": format!("{origin}/auth/v1/device"),
                    "expires_in": 60,
                    "interval": 1,
                }),
            ),
            "/auth/v1/oidc/token" => answer(200, token_response(origin)),
            _ => api(origin, req, n),
        }
    }))
}

fn secrets_absent(p: &Project, out: &Output) {
    let access =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    for secret in [REFRESH, DEVICE, "c2lnbmF0dXJl"] {
        assert!(!access.contains(secret), "{secret} in the output");
    }
    // Nothing in the repository carries a token.
    let mut stack = vec![p.root.path().to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = e.path();
            if path.ends_with(".git") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).to_string();
                for secret in [REFRESH, DEVICE, "c2lnbmF0dXJl"] {
                    assert!(!text.contains(secret), "{} carries a token", path.display());
                }
            }
        }
    }
}

#[test]
fn login_stores_the_credential_owner_only_and_prints_no_token() {
    let c = cell(|_, _, _| answer(404, json!({})));
    let p = Project::new(&c.origin);
    let out = p.run(&["hosted", "login", p.path(), "--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let v = json_naming::from_output(&out.stdout).unwrap();
    assert_eq!(v["report"]["cell"], c.origin);
    assert!(String::from_utf8_lossy(&out.stderr).contains("ABCD-0038"));
    let store = statecraft_hosted::credential::Store::new(p.home.path());
    let origin = statecraft_hosted::Origin::parse(&c.origin).unwrap();
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(store.path(&origin))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    secrets_absent(&p, &out);
}

#[test]
fn without_a_declaration_or_a_credential_the_verbs_refuse() {
    let c = cell(|_, _, _| answer(500, json!({})));
    let p = Project::new(&c.origin);
    std::fs::remove_file(p.root.path().join(".statecraft/hosted.json")).unwrap();
    let out = p.run(&["hosted", "login", p.path()]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let p = Project::new(&c.origin);
    let receipt = p.accept_answer();
    let out = p.run(&[
        "hosted",
        "submit",
        p.path(),
        "--receipt",
        receipt.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).contains(&format!("log in to {}", c.origin)));
    assert_eq!(
        c.count("POST", &format!("/api/repositories/{REPO}/revisions")),
        0
    );
}

#[test]
fn submit_plans_without_a_request_and_refuses_without_a_receipt() {
    let c = cell(|_, _, _| answer(500, json!({})));
    let p = Project::new(&c.origin);
    let receipt = p.accept_answer();
    let out = p.run(&[
        "hosted",
        "submit",
        p.path(),
        "--receipt",
        receipt.to_str().unwrap(),
        "--plan",
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let v = json_naming::from_output(&out.stdout).unwrap();
    assert_eq!(v["report"]["plan"]["register"]["commit"], p.head());
    assert_eq!(v["report"]["plan"]["repository"], REPO);
    assert!(c.requests().is_empty(), "the plan sent a request");
    // An accept answer with no receipt.
    let failed = p.home.path().join("failed.json");
    std::fs::write(
        &failed,
        br#"{"verb":"accept","report":{"acceptance":"failed"}}"#,
    )
    .unwrap();
    let out = p.run(&[
        "hosted",
        "submit",
        p.path(),
        "--receipt",
        failed.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(c.requests().is_empty());
}

#[test]
fn submit_registers_submits_and_records_runtime_state_only() {
    let c = cell(|_, req, n| {
        if req.path == format!("/api/repositories/{REPO}/revisions") {
            answer(
                if n == 0 { 201 } else { 200 },
                json!({"revision": REV, "repository": REPO}),
            )
        } else if req.path == "/api/evidence/submissions" {
            answer(
                200,
                json!({"attestations": ["x"], "verdicts": [{"integrity": "pass"}]}),
            )
        } else {
            answer(404, json!({}))
        }
    });
    let p = Project::new(&c.origin);
    assert_eq!(p.run(&["hosted", "login", p.path()]).status.code(), Some(0));
    let receipt = p.accept_answer();
    let first = p.run(&[
        "hosted",
        "submit",
        p.path(),
        "--receipt",
        receipt.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(first.status.code(), Some(0), "{first:?}");
    let again = p.run(&[
        "hosted",
        "submit",
        p.path(),
        "--receipt",
        receipt.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    let a = json_naming::from_output(&first.stdout).unwrap();
    let b = json_naming::from_output(&again.stdout).unwrap();
    assert_eq!(a["report"]["submission"]["registered"], "new");
    assert_eq!(b["report"]["submission"]["registered"], "existing");
    assert_eq!(
        a["report"]["submission"]["idempotencyKey"],
        b["report"]["submission"]["idempotencyKey"]
    );
    let registered = c
        .requests()
        .into_iter()
        .find(|r| r.path.ends_with("/revisions"))
        .unwrap();
    let body: Value = serde_json::from_slice(&registered.body).unwrap();
    assert_eq!(body["commit"]["oid"], p.head());
    assert_eq!(body["commit"]["format"], "sha1");
    let log = std::fs::read_to_string(
        p.root
            .path()
            .join(".statecraft/state/hosted/submissions.jsonl"),
    )
    .unwrap();
    assert_eq!(log.lines().count(), 2);
    secrets_absent(&p, &first);
}

fn pinned_root_set(dir: &Path) -> (std::path::PathBuf, String) {
    use statecraft_envelope::issuer::{
        Authorization, Enrolment, IssuerIdentity, IssuerPayload, OwnerRoot, RootSetV2,
        identity_created_fact,
    };
    use statecraft_envelope::sign::Signer;
    let owner = Signer::from_seed(blake3_seed(
        "statecraft TEST-ONLY hosted owner: public, never production",
    ));
    let issuer = Signer::from_seed(blake3_seed(
        "statecraft TEST-ONLY hosted issuer: public, never production",
    ));
    let at = statecraft_envelope::hlc::Hlc::new(1_790_000_000_000, 0);
    let identity =
        IssuerIdentity::decode(&identity_created_fact(&issuer.public(), at).canonical_bytes())
            .unwrap();
    let payload = IssuerPayload::Enrolment(Enrolment {
        issuer_id: identity.id,
        public_key: issuer.public(),
        scope: "platform".into(),
        not_before: Some(at),
        not_after: None,
    });
    let signature = owner.sign(payload.kind().domain(), &payload.canonical_bytes());
    let set = RootSetV2 {
        root_set_version: 1,
        owner_roots: vec![OwnerRoot {
            public_key: owner.public(),
            status: statecraft_envelope::roots::RootStatus::Active,
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
    };
    let digest = set.verify().unwrap().digest.to_hex();
    let file = dir.join("pinned-v2.json");
    std::fs::write(&file, set.to_bytes()).unwrap();
    (file, digest)
}

fn blake3_seed(label: &str) -> &'static [u8; 32] {
    use statecraft_envelope::hash::Hash;
    Box::leak(Box::new(Hash::of(label.as_bytes()).0))
}

#[test]
fn decision_reports_not_registered_none_and_a_root_set_mismatch() {
    let none_bundle = |root_set: &str| {
        json!({
            "revision": REV, "entries": [], "objects": {}, "tombstones": [],
            "root_set": root_set, "admission": null, "decisions": [], "admissionWord": "none",
        })
    };
    let c = cell(move |_, req, _| {
        if req.path.ends_with(&format!("{REV}/decision")) {
            answer(200, none_bundle("33".repeat(32).as_str()))
        } else {
            answer(404, json!({}))
        }
    });
    let p = Project::new(&c.origin);
    assert_eq!(p.run(&["hosted", "login", p.path()]).status.code(), Some(0));
    let (set, digest) = pinned_root_set(p.home.path());
    let decide = |revision: &str| {
        p.run(&[
            "hosted",
            "decision",
            p.path(),
            "--revision",
            revision,
            "--root-set",
            set.to_str().unwrap(),
            "--pin-version",
            "1",
            "--pin-digest",
            &digest,
            "--json",
        ])
    };
    let unregistered = decide(&"44".repeat(32));
    assert_eq!(unregistered.status.code(), Some(2), "{unregistered:?}");
    assert!(String::from_utf8_lossy(&unregistered.stdout).contains("is not registered"));
    let out = decide(REV);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let v = json_naming::from_output(&out.stdout).unwrap();
    assert_eq!(v["report"]["rootSet"]["matches"], false);
    assert_eq!(v["report"]["rootSet"]["pinned"], digest);
    assert_eq!(v["report"]["rootSet"]["bundle"], "33".repeat(32));
    assert!(v["report"]["divergence"].is_string());
    // A wrong pin refuses before any request.
    let before = c.requests().len();
    let wrong = p.run(&[
        "hosted",
        "decision",
        p.path(),
        "--revision",
        REV,
        "--root-set",
        set.to_str().unwrap(),
        "--pin-version",
        "2",
        "--pin-digest",
        &digest,
    ]);
    assert_eq!(wrong.status.code(), Some(2), "{wrong:?}");
    assert_eq!(c.requests().len(), before);
    secrets_absent(&p, &out);
}

#[test]
fn logout_revokes_then_deletes_and_there_is_no_approval_verb() {
    let c = cell(|_, req, _| match req.path.as_str() {
        "/session/token/revoke" => answer(200, json!({})),
        _ => answer(404, json!({})),
    });
    let p = Project::new(&c.origin);
    assert_eq!(p.run(&["hosted", "login", p.path()]).status.code(), Some(0));
    let out = p.run(&["hosted", "logout", p.path(), "--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let v = json_naming::from_output(&out.stdout).unwrap();
    assert_eq!(v["report"]["revocation"], "confirmed");
    assert_eq!(c.count("POST", "/session/token/revoke"), 1);
    let store = statecraft_hosted::credential::Store::new(p.home.path());
    assert!(
        !store
            .path(&statecraft_hosted::Origin::parse(&c.origin).unwrap())
            .exists()
    );
    for verb in ["approve", "merge", "dispatch"] {
        let out = p.run(&["hosted", verb, p.path()]);
        assert_eq!(out.status.code(), Some(3), "{verb}");
    }
}
