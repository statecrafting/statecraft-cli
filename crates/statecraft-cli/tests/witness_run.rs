//! Spec 013 through the real `run` path and the real confinement, with a
//! fake provider: `--witness` and `--require-witness` (section 7,
//! 2026-10-17), the producer at its one home path, and the posture each
//! attempt records (section 3.7).
//!
//! The producer is verified by digest against the adopted ledger, so only
//! the released executable can start a sidecar. The last test runs it when
//! `STATECRAFT_TEST_WIRE_WITNESS` names that file, and says it skipped
//! otherwise.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

#[path = "support/provider_token.rs"]
mod provider_token;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn executable(path: &Path, script: &str) {
    statecraft_adapter::fixture::install_script(path, script, 0o755).unwrap();
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

struct Fixture {
    target: tempfile::TempDir,
    home: tempfile::TempDir,
    bin: tempfile::TempDir,
    operator: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let f = Fixture {
        target: tempfile::tempdir().unwrap(),
        home: tempfile::tempdir().unwrap(),
        bin: tempfile::tempdir().unwrap(),
        operator: tempfile::tempdir().unwrap(),
    };
    git(f.target.path(), &["init", "--quiet"]);
    git(f.target.path(), &["config", "user.name", "fixture"]);
    git(
        f.target.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    std::fs::write(f.target.path().join("source.txt"), "base").unwrap();
    git(f.target.path(), &["add", "source.txt"]);
    git(
        f.target.path(),
        &["commit", "--quiet", "-m", "fixture base"],
    );
    executable(
        &f.bin.path().join("spec-spine"),
        r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check|'check --help') exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"replay","title":"recorded stream"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"replay","status":"approved","implementation":"pending"}]}' ;;
  'verify '*' --plan --json') printf '{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"%s"},"schemaVersion":"0.6.0","verb":"verify"}' $2 ;;
  *) exit 3 ;;
esac
"#,
    );
    let stream = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl");
    std::fs::copy(stream, f.bin.path().join("native.jsonl")).unwrap();
    // The child reports the proxy and trust anchor it was given, and whether
    // it can read the certificate, then replays a recorded stream. It makes
    // no network request.
    executable(
        &f.bin.path().join("claude"),
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267'; exit 0; fi
/bin/cat > /dev/null
printf '%s' "$HTTPS_PROXY" > child-proxy
printf '%s' "$NODE_EXTRA_CA_CERTS" > child-ca
if [ -n "$NODE_EXTRA_CA_CERTS" ] && /bin/cat "$NODE_EXTRA_CA_CERTS" > /dev/null 2>&1; then echo yes > child-read-ca; fi
/bin/cat "$(dirname "$0")/native.jsonl"
"#,
    );
    provider_token::with_token(f.home.path());
    f
}

impl Fixture {
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home.path())
            .env("HOME", self.operator.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.bin.path().display()),
            )
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    fn arm(&self) {
        let root = self.target.path().to_str().unwrap();
        let registered = self.run(&["project", "register", root]);
        assert!(registered.status.code().unwrap() <= 1, "{registered:?}");
        let armed = self.run(&["project", "arm", root]);
        assert_eq!(armed.status.code(), Some(0), "{armed:?}");
    }

    fn root(&self) -> &str {
        self.target.path().to_str().unwrap()
    }

    /// The last attempt outcome's detail, read through the chain's own API.
    fn last_outcome(&self) -> serde_json::Value {
        let (chain, _) =
            statecraft_run::record::Chain::open(self.home.path(), self.target.path()).unwrap();
        chain
            .entries()
            .into_iter()
            .rev()
            .find(|e| e.kind == statecraft_run::Kind::Outcome && e.subject == "attempt")
            .expect("an attempt outcome")
            .detail
    }

    fn shown_posture(&self) -> String {
        let show = self.run(&["run", "show", self.root(), "replay", "--json"]);
        let v: serde_json::Value = json_naming::from_output(&show.stdout).unwrap();
        let text = v.to_string();
        let witness = v["report"]["witness"]
            .as_array()
            .unwrap_or_else(|| panic!("{text}"));
        witness.last().unwrap()["posture"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn workspace(output: &Output) -> PathBuf {
        let answer: serde_json::Value = json_naming::from_output(&output.stdout).unwrap();
        PathBuf::from(answer["report"]["workspaceRetained"].as_str().unwrap())
    }
}

#[test]
fn an_unflagged_run_records_witness_not_requested_and_gives_the_child_no_proxy() {
    let f = fixture();
    f.arm();
    let output = f.run(&["run", f.root(), "replay", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let workspace = Fixture::workspace(&output);
    assert_eq!(
        std::fs::read_to_string(workspace.join("child-proxy")).unwrap(),
        ""
    );
    let detail = f.last_outcome();
    assert_eq!(detail["witnessPosture"], "witness-not-requested");
    assert_eq!(detail["witnessRequired"], false);
    assert!(detail.get("witness").is_none());
    assert_eq!(f.shown_posture(), "witness-not-requested");
}

#[test]
fn a_requested_witness_without_its_producer_runs_unwitnessed_as_unavailable() {
    let f = fixture();
    f.arm();
    let output = f.run(&["run", f.root(), "replay", "--witness", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let workspace = Fixture::workspace(&output);
    assert_eq!(
        std::fs::read_to_string(workspace.join("child-proxy")).unwrap(),
        ""
    );
    let detail = f.last_outcome();
    assert_eq!(detail["witnessPosture"], "witness-unavailable");
    assert_eq!(detail["witnessRequired"], false);
    assert!(
        detail["witness"]["producer"]["unavailable"].is_string(),
        "{detail}"
    );
    // No endpoint was reserved for a producer that is not there.
    assert!(
        detail["posture"]["confinement"]
            .get("witnessEndpoint")
            .is_none(),
        "{detail}"
    );
    assert_eq!(f.shown_posture(), "witness-unavailable");
}

#[test]
fn a_foreign_file_at_the_producer_path_is_unavailable_and_required_is_recorded() {
    let f = fixture();
    f.arm();
    if let Some(target) = statecraft_adapter::witness::ledger::current_target() {
        let path = statecraft_home::witness::producer_path(f.home.path(), target);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        executable(&path, "#!/bin/sh\nexit 0\n");
    }
    let output = f.run(&["run", f.root(), "replay", "--require-witness", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let detail = f.last_outcome();
    assert_eq!(detail["witnessPosture"], "witness-unavailable");
    assert_eq!(detail["witnessRequired"], true);
}

#[test]
fn an_unknown_flag_or_a_second_operand_is_usage() {
    let f = fixture();
    f.arm();
    for args in [
        &["run", f.root(), "replay", "--witnessed"][..],
        &["run", f.root(), "replay", "other"][..],
    ] {
        let output = f.run(args);
        assert_eq!(output.status.code(), Some(3), "{args:?} {output:?}");
    }
}

#[test]
fn the_released_producer_witnesses_a_run_end_to_end() {
    let Ok(released) = std::env::var("STATECRAFT_TEST_WIRE_WITNESS") else {
        eprintln!("skipped: STATECRAFT_TEST_WIRE_WITNESS names no released wire-witness");
        return;
    };
    let f = fixture();
    f.arm();
    let target = statecraft_adapter::witness::ledger::current_target().unwrap();
    let path = statecraft_home::witness::producer_path(f.home.path(), target);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::copy(&released, &path).unwrap();
    let output = f.run(&["run", f.root(), "replay", "--witness", "--json"]);
    let detail = f.last_outcome();
    assert_eq!(output.status.code(), Some(0), "{detail}");
    let workspace = Fixture::workspace(&output);
    let proxy = std::fs::read_to_string(workspace.join("child-proxy")).unwrap();
    assert!(proxy.starts_with("http://127.0.0.1:"), "{proxy} {detail}");
    let ca = std::fs::read_to_string(workspace.join("child-ca")).unwrap();
    assert!(ca.ends_with("witness/capture/ca.pem"), "{ca}");
    assert_eq!(
        std::fs::read_to_string(workspace.join("child-read-ca"))
            .ok()
            .as_deref(),
        Some("yes\n"),
        "the child could not read its trust anchor"
    );
    // The child made no request, so the capture is absent and the posture
    // says so: testimony of nothing is not `witnessed`.
    assert_eq!(detail["witnessPosture"], "witness-incomplete", "{detail}");
    let endpoint = detail["posture"]["confinement"]["witnessEndpoint"]
        .as_str()
        .unwrap();
    assert_eq!(proxy, format!("http://{endpoint}"));
    let (chain, _) = statecraft_run::record::Chain::open(f.home.path(), f.target.path()).unwrap();
    let sidecar: Vec<_> = chain
        .entries()
        .into_iter()
        .filter(|e| e.subject == "witness-sidecar")
        .collect();
    assert_eq!(sidecar.len(), 2, "one intent and one closing outcome");
    assert_eq!(
        sidecar[1].detail["termination"], "finished",
        "{:?}",
        sidecar[1].detail
    );
    assert_eq!(sidecar[1].detail["posture"], "witness-incomplete");
}
