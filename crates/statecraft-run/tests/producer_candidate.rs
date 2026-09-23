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

#[test]
#[ignore = "needs STATECRAFT_PRODUCER_BIN and STATECRAFT_PRODUCER_REV naming an exact producer build"]
fn the_named_producer_build_reads_as_one_state_through_the_consumer() {
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

    let report = SpecSpineCli {
        binary: binary.display().to_string(),
    }
    .corpus_report(&corpus)
    .unwrap_or_else(|e| panic!("the consumer refused the named producer's own state: {e}"));
    let digest = {
        let out = Command::new("shasum")
            .args(["-a", "256"])
            .arg(&binary)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
    };
    println!(
        "producer {version} at {revision}, binary sha256 {digest}: {} ready row(s), status from {}",
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
