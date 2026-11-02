//! Spec 052 section 3.4 through the binary: `provider compare` against the
//! committed goldens, with synthetic captures made from them, and the same
//! comparison with networking denied, no credential reachable and no
//! provider on `PATH` (R-3, V-1).

#[path = "support/json_naming.rs"]
mod json_naming;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const NEXT: &str = "2.1.308";
const SCENARIOS: [&str; 5] = [
    "api-error",
    "denied",
    "max-turns",
    "success",
    "tool-removed",
];

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_statecraft-cli"))
}

fn adapter_crate() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../statecraft-adapter-claude-code")
}

fn corpus() -> PathBuf {
    adapter_crate().join("testdata/stream")
}

/// Every golden relabelled to `NEXT`, with `edit` applied to every line of
/// the `denied` stream.
fn captures(edit: impl Fn(&mut Value)) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut entries = Vec::new();
    for scenario in SCENARIOS {
        let text = fs::read_to_string(corpus().join(format!("{scenario}.jsonl"))).unwrap();
        let mut out = String::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let mut v: Value = serde_json::from_str(line).unwrap();
            if v.get("claude_code_version").is_some() {
                v["claude_code_version"] = json!(NEXT);
            }
            if scenario == "denied" {
                edit(&mut v);
            }
            out.push_str(&format!("{v}\n"));
        }
        let file = format!("{scenario}.jsonl");
        fs::write(dir.path().join(&file), out).unwrap();
        entries.push(json!({
            "scenario": scenario,
            "providerVersion": NEXT,
            "platform": "darwin",
            "invocation": "synthetic, from the committed golden",
            "captured": "2026-11-01",
            "file": file,
        }));
    }
    fs::write(
        dir.path().join("captures.json"),
        json!({ "captures": entries }).to_string(),
    )
    .unwrap();
    dir
}

fn drop_denials(v: &mut Value) {
    if v.get("permission_denials").is_some() {
        v["permission_denials"] = json!([]);
    }
}

fn args(dir: &Path) -> Vec<String> {
    vec![
        "provider".into(),
        "compare".into(),
        dir.display().to_string(),
        "--corpus".into(),
        corpus().display().to_string(),
        "--json".into(),
    ]
}

fn report(out: &Output) -> Value {
    let v = json_naming::from_output(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    assert_eq!(v["verb"], "provider.compare", "{v}");
    v
}

#[test]
fn a_version_that_means_what_the_goldens_mean_is_qualifiable() {
    let dir = captures(|_| {});
    let out = Command::new(binary())
        .args(args(dir.path()))
        .output()
        .unwrap();
    let v = report(&out);
    assert_eq!(out.status.code(), Some(0), "{v}");
    assert_eq!(v["report"]["qualifiable"], true);
    assert_eq!(v["report"]["providerVersion"], NEXT);
    assert_eq!(v["report"]["rows"].as_array().unwrap().len(), 5);
}

#[test]
fn an_unreviewed_divergence_is_a_finding_naming_the_scenario() {
    let dir = captures(drop_denials);
    let out = Command::new(binary())
        .args(args(dir.path()))
        .output()
        .unwrap();
    let v = report(&out);
    assert_eq!(out.status.code(), Some(1), "{v}");
    assert_eq!(v["report"]["blocking"], json!(["denied"]));
    assert!(
        v["summary"]
            .as_str()
            .unwrap()
            .contains("denied: diverges-unreviewed")
    );

    let human = Command::new(binary())
        .args(&args(dir.path())[..5])
        .output()
        .unwrap();
    assert_eq!(human.status.code(), Some(1));
    let text = String::from_utf8_lossy(&human.stdout);
    assert!(text.contains("is not qualifiable: denied"), "{text}");
}

#[test]
fn a_missing_corpus_option_is_a_usage_error() {
    let out = Command::new(binary())
        .args(["provider", "compare", "/nonexistent", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(report(&out)["error"]["kind"], "usage");
}

/// The mechanism that runs a child with networking denied on this platform,
/// as a program and its leading arguments, when one starts here.
fn network_denied() -> Option<Vec<String>> {
    let candidates: Vec<Vec<&str>> = if cfg!(target_os = "macos") {
        vec![vec![
            "/usr/bin/sandbox-exec",
            "-p",
            "(version 1)(allow default)(deny network*)",
        ]]
    } else {
        // A user namespace first; a CI runner that restricts unprivileged
        // namespaces gets a fresh network namespace through sudo. Absolute
        // paths, because the child's own `PATH` is emptied.
        vec![
            vec!["/usr/bin/unshare", "--net", "--map-root-user", "--"],
            vec!["/usr/bin/sudo", "-n", "/usr/bin/unshare", "--net", "--"],
        ]
    };
    candidates.into_iter().find_map(|prefix| {
        let started = Command::new(prefix[0])
            .args(&prefix[1..])
            .arg("/bin/sh")
            .args(["-c", "exit 0"])
            .output()
            .is_ok_and(|o| o.status.success());
        started.then(|| prefix.iter().map(|s| s.to_string()).collect())
    })
}

#[test]
fn the_comparison_runs_with_networking_denied_and_nothing_to_authenticate_with() {
    let Some(prefix) = network_denied() else {
        // Not a pass: spec 005 section 3.2 rule 3 makes a check that did not
        // run unknown, so the suite fails rather than skipping.
        panic!("no mechanism to deny networking started on this runner");
    };
    let home = tempfile::tempdir().unwrap();
    for (edit, code) in [(None, 0), (Some(drop_denials as fn(&mut Value)), 1)] {
        let dir = captures(|v| {
            if let Some(edit) = edit {
                edit(v);
            }
        });
        let out = Command::new(&prefix[0])
            .args(&prefix[1..])
            .arg(binary())
            .args(args(dir.path()))
            .env_clear()
            .env("PATH", "")
            .env("HOME", home.path())
            .env("STATECRAFT_HOME", home.path().join("statecraft"))
            .output()
            .unwrap();
        let v = report(&out);
        assert_eq!(out.status.code(), Some(code), "{v}");
    }
    // Nothing was written under the home the child was given.
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
}

/// R-3 from the source side: the comparison and its binding name no
/// process, socket, environment or credential API.
#[test]
fn the_comparison_names_no_process_socket_or_credential() {
    for file in [
        adapter_crate().join("src/golden.rs"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/golden.rs"),
    ] {
        let text = fs::read_to_string(&file).unwrap();
        for forbidden in [
            "std::net",
            "std::process",
            "Command",
            "TcpStream",
            "UdpSocket",
            "env::var",
            "CREDENTIAL_PATH",
            "spawn(",
            "statecraft_home",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} names `{forbidden}`",
                file.display()
            );
        }
    }
}
