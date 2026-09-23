//! The consumer's readiness read against one exact producer build, named
//! explicitly rather than found on `PATH`.
//!
//! Spec 003 section 3.1.2. **Ignored by default**, because it needs a producer
//! binary this repository does not pin and a check that did not run is never a
//! pass. Run it deliberately, naming the binary and the revision it was built
//! from:
//!
//! ```sh
//! STATECRAFT_PRODUCER_BIN=/abs/path/spec-spine \
//! STATECRAFT_PRODUCER_REV=<commit it was built from> \
//! cargo test -p statecraft-run --test producer_candidate -- --ignored --nocapture
//! ```
//!
//! It copies this repository's committed tree into a scratch directory with
//! `git archive`, pins that copy to the named producer's version, compiles and
//! indexes the copy's ledger **with that producer**, and reads it through
//! [`SpecSpineCli`] with the binary set to the named path. Every read comes
//! from one producer's own state; nothing reads a ledger another version
//! wrote. Against the pinned producer the copy is not repinned.

use statecraft_run::contract;
use statecraft_run::report::{ReportSource, SpecSpineCli, StatusSource};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn run(program: &Path, args: &[&str], dir: &Path) -> std::process::Output {
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("{} {args:?}: {e}", program.display()))
}

/// The named binary and revision, and a scratch copy of this repository's
/// committed tree whose ledger that binary compiled where its version is not
/// the pinned one.
fn named_producer() -> (PathBuf, String, String, tempfile::TempDir, PathBuf) {
    let binary =
        PathBuf::from(std::env::var("STATECRAFT_PRODUCER_BIN").expect(
            "STATECRAFT_PRODUCER_BIN must name the producer binary; nothing is found on PATH",
        ));
    let revision = std::env::var("STATECRAFT_PRODUCER_REV")
        .expect("STATECRAFT_PRODUCER_REV must name the commit the binary was built from");
    assert!(binary.is_absolute(), "name the binary by absolute path");
    let scratch = tempfile::tempdir().unwrap();
    let corpus = scratch.path().join("corpus");
    std::fs::create_dir_all(&corpus).unwrap();
    let archive = Command::new("sh")
        .arg("-c")
        .arg("git -C \"$1\" archive HEAD | tar -x -C \"$2\"")
        .arg("archive")
        .arg(repo())
        .arg(&corpus)
        .status()
        .unwrap();
    assert!(archive.success(), "git archive");
    let version = String::from_utf8_lossy(&run(&binary, &["--version"], &corpus).stdout)
        .split_whitespace()
        .next_back()
        .unwrap_or_default()
        .to_string();
    let config = corpus.join("spec-spine.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    let pinned = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("required_version = \"="))
        .map(|v| v.trim_end_matches('"').to_string())
        .expect("the committed pin");
    if pinned != version {
        std::fs::write(
            &config,
            text.replace(
                &format!("required_version = \"={pinned}\""),
                &format!("required_version = \"={version}\""),
            ),
        )
        .unwrap();
        for verb in ["compile", "index"] {
            let out = run(&binary, &[verb], &corpus);
            assert!(
                out.status.success(),
                "{verb} with the named producer: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
    (binary, revision, version, scratch, corpus)
}

fn sha256(path: &Path) -> String {
    let out = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}

#[test]
#[ignore = "needs STATECRAFT_PRODUCER_BIN and STATECRAFT_PRODUCER_REV naming an exact producer build"]
fn the_named_producer_build_reads_as_one_state_through_the_consumer() {
    let (binary, revision, version, _scratch, corpus) = named_producer();
    let report = SpecSpineCli {
        binary: binary.display().to_string(),
    }
    .corpus_report(&corpus)
    .unwrap_or_else(|e| panic!("the consumer refused the named producer's own state: {e}"));
    println!(
        "producer {version} at {revision}, binary sha256 {}: {} ready row(s), status from {}",
        sha256(&binary),
        report.ready.len(),
        report.status_source.describe()
    );
    assert_eq!(report.spec_spine_version, version);
    let carries = report.ready.iter().any(|r| r.status.is_some());
    assert_eq!(
        report.status_source,
        if carries {
            StatusSource::ListAgreeingWithPlan
        } else {
            StatusSource::ListOnly
        }
    );
}

/// Spec 003 section 3.1.3 through the real producer: a producer that resolves
/// closures binds one, and resolving the same request again answers the same
/// digest (the purity spec 005 section 3.18 compares on); a producer that
/// does not is `unsupported`, naming its version.
#[test]
#[ignore = "needs STATECRAFT_PRODUCER_BIN and STATECRAFT_PRODUCER_REV naming an exact producer build"]
fn the_named_producer_build_binds_a_contract_or_says_it_cannot() {
    let (binary, revision, version, _scratch, corpus) = named_producer();
    let cli = SpecSpineCli {
        binary: binary.display().to_string(),
    };
    let report = cli.corpus_report(&corpus).unwrap();
    let spec = report
        .ready
        .first()
        .map_or("000-bootstrap".to_string(), |r| r.id.clone());
    let first = contract::bind(&cli, &corpus, &spec, report.lifecycle_of(&spec));
    println!(
        "producer {version} at {revision}: contract for {spec}: {}",
        first.describe()
    );
    match first.state {
        contract::State::Bound => {
            let again = contract::bind(&cli, &corpus, &spec, report.lifecycle_of(&spec));
            assert_eq!(
                first.digest, again.digest,
                "the same request, the same ledger"
            );
            assert!(!first.members.is_empty());
        }
        contract::State::Unsupported => {
            let detail = first.detail.clone().unwrap_or_default();
            assert!(
                detail.contains(&format!("spec-spine {version} was asked")),
                "{detail}"
            );
        }
        other => panic!(
            "the named producer answered {}: {}",
            other.word(),
            first.describe()
        ),
    }
}

/// spec-spine 103's portable verifier fixtures, replayed through the named
/// build's own `verify-attestation --recompute`, each case against its
/// recorded exit. Evidence for adopting that build: the verifier this
/// repository's governance would then run classifies each recorded tampering
/// as its producer recorded. Statecraft has no verifier of its own for these
/// attestations, and this builds none.
///
/// The fixtures carry the tool version that produced them, so they are
/// evidence about that build only: name the fixture directory from the same
/// revision as the binary, `STATECRAFT_PRODUCER_FIXTURES=<checkout>/crates/
/// spec-spine-core/fixtures/verifier`.
#[test]
#[ignore = "needs STATECRAFT_PRODUCER_BIN and STATECRAFT_PRODUCER_FIXTURES from one producer revision"]
fn the_named_producer_build_reproduces_its_portable_verifier_fixtures() {
    let binary =
        PathBuf::from(std::env::var("STATECRAFT_PRODUCER_BIN").expect("STATECRAFT_PRODUCER_BIN"));
    let fixtures = PathBuf::from(std::env::var("STATECRAFT_PRODUCER_FIXTURES").expect(
        "STATECRAFT_PRODUCER_FIXTURES must name fixtures/verifier from the binary's revision",
    ));
    let index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("index.json")).unwrap()).unwrap();
    let cases = index["cases"]
        .as_array()
        .expect("index.json lists its cases");
    assert!(!cases.is_empty());
    let scratch = tempfile::tempdir().unwrap();
    let mut mismatches = Vec::new();
    for case in cases {
        let id = case.as_str().unwrap();
        let dir = fixtures.join(id);
        let spec: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("case.json")).unwrap()).unwrap();
        let expected = spec["expect"]["exit"].as_i64().unwrap();
        // A copy, so nothing the verifier does touches the fixture.
        let repo = scratch.path().join(id);
        std::fs::create_dir_all(&repo).unwrap();
        if spec["needsCorpus"].as_bool().unwrap_or(false) {
            let copied = Command::new("cp")
                .arg("-R")
                .arg(dir.join("corpus/."))
                .arg(&repo)
                .status()
                .unwrap();
            assert!(copied.success());
        }
        let out = Command::new(&binary)
            .args(["verify-attestation", "--recompute", "--json", "--repo"])
            .arg(&repo)
            .arg("--attestation")
            .arg(dir.join("payload.json"))
            .output()
            .unwrap();
        let got = out.status.code().map_or(-1, i64::from);
        println!("{id}: expected exit {expected}, got {got}");
        if got != expected {
            mismatches.push(format!("{id}: expected {expected}, got {got}"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} case(s) did not reproduce: {mismatches:?}",
        mismatches.len(),
        cases.len()
    );
}
