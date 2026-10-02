//! Spec 029 V-1 and section 3.4: `check` is read from its envelope.
//!
//! The envelopes below were recorded from spec-spine 0.28.0 on 2026-09-30:
//! a fresh tree, one stale shard, and a configuration refusal. The others vary
//! one member of a recorded envelope. Each is classified by its outcome and
//! members, whatever its words say.

use statecraft_environment::probe::{
    CheckAnswer, StaleReading, read_check_envelope, run_check, takes_json,
};

const FRESH: &str = r#"{
  "exitCode": 0,
  "outcome": "ok",
  "report": {
    "index": {"diagnostics": {"byCode": {}, "errors": 0, "warnings": 0}, "fresh": true, "unwitnessed": {"allowed": 0, "total": 0}},
    "registry": {"fresh": true, "validationPassed": true, "warnings": 0}
  },
  "schemaVersion": "1.1.0",
  "summary": "check: ok",
  "tool": "spec-spine",
  "verb": "check"
}"#;

const STALE: &str = r#"{
  "exitCode": 1,
  "outcome": "finding",
  "report": {
    "index": {
      "actual": "1 stale shard(s):\n  modified by-spec/029-one-resolved-judge.json",
      "diagnostics": {"byCode": {}, "errors": 0, "warnings": 0},
      "expected": "32 shard(s) matching the corpus",
      "fresh": false,
      "unwitnessed": {"allowed": 0, "total": 0}
    },
    "registry": {
      "actual": "1 stale shard(s):\n  modified 029-one-resolved-judge.json",
      "expected": "24 shard(s) matching the corpus",
      "fresh": false,
      "validationPassed": true,
      "warnings": 0
    }
  },
  "schemaVersion": "1.1.0",
  "summary": "check: finding",
  "tool": "spec-spine",
  "verb": "check"
}"#;

const REFUSED: &str = r#"{
  "error": {"kind": "config", "message": "config error: TOML parse error at line 133, column 1\n    |\n133 | garbage = 1\n"},
  "exitCode": 2,
  "outcome": "refused",
  "schemaVersion": "1.1.0",
  "summary": "config error: TOML parse error at line 133, column 1",
  "tool": "spec-spine",
  "verb": "check"
}"#;

fn read(code: i32, envelope: &str) -> CheckAnswer {
    read_check_envelope(Some(code), &format!("exit {code}"), envelope.as_bytes())
}

#[test]
fn ok_is_fresh() {
    assert_eq!(read(0, FRESH), CheckAnswer::Fresh);
}

#[test]
fn a_finding_with_a_half_not_fresh_is_stale_whatever_its_words() {
    match read(1, STALE) {
        CheckAnswer::Stale { reading, detail } => {
            assert_eq!(reading, StaleReading::Stale);
            assert!(detail.contains("registry: 1 stale shard(s):"), "{detail}");
            assert!(detail.contains("index: 1 stale shard(s):"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
    // The same members under words the wording reader would never call
    // stale: the envelope decides.
    let reworded = STALE.replace("stale shard(s)", "shard(s) behind");
    assert!(matches!(
        read(1, &reworded),
        CheckAnswer::Stale {
            reading: StaleReading::Stale,
            ..
        }
    ));
}

#[test]
fn a_finding_that_does_not_validate_outranks_staleness() {
    let invalid = STALE.replace(
        r#""validationPassed": true"#,
        r#""validationPassed": false"#,
    );
    assert!(matches!(
        read(1, &invalid),
        CheckAnswer::DoesNotValidate { .. }
    ));
}

#[test]
fn a_fresh_finding_with_an_unresolved_unit_error_is_an_unresolved_claim() {
    let unresolved = FRESH
        .replace(r#""outcome": "ok""#, r#""outcome": "finding""#)
        .replace(r#""exitCode": 0"#, r#""exitCode": 1"#)
        .replace(
            r#""byCode": {}, "errors": 0"#,
            r#""byCode": {"W-001": 1}, "errors": 1"#,
        );
    assert!(matches!(
        read(1, &unresolved),
        CheckAnswer::Stale {
            reading: StaleReading::UnresolvedClaim,
            ..
        }
    ));
}

#[test]
fn refused_usage_and_failed_are_reads_not_performed() {
    match read(2, REFUSED) {
        CheckAnswer::NotPerformed { status, detail } => {
            assert_eq!(status, "exit 2");
            assert_eq!(
                detail,
                "config error: TOML parse error at line 133, column 1"
            );
        }
        other => panic!("{other:?}"),
    }
    for (code, outcome) in [(3, "usage"), (4, "failed")] {
        let envelope = REFUSED
            .replace(r#""exitCode": 2"#, &format!(r#""exitCode": {code}"#))
            .replace(
                r#""outcome": "refused""#,
                &format!(r#""outcome": "{outcome}""#),
            );
        assert!(
            matches!(read(code, &envelope), CheckAnswer::NotPerformed { .. }),
            "{outcome}"
        );
    }
}

#[test]
fn an_envelope_that_cannot_be_trusted_establishes_nothing() {
    // Not JSON at all.
    assert!(matches!(
        read(0, "check: fresh"),
        CheckAnswer::NotPerformed { .. }
    ));
    // Another verb's envelope.
    let other = FRESH.replace(r#""verb": "check""#, r#""verb": "lint""#);
    assert!(matches!(read(0, &other), CheckAnswer::NotPerformed { .. }));
    // A declared exit code the process did not end with.
    assert!(matches!(read(1, FRESH), CheckAnswer::NotPerformed { .. }));
    assert!(matches!(read(0, STALE), CheckAnswer::NotPerformed { .. }));
}

#[test]
fn json_support_is_read_from_the_verbs_own_help() {
    let recorded = "Usage: spec-spine check [OPTIONS]\n\nOptions:\n      --fail-on-unresolved\n      --repo <DIR>\n      --json\n          Emit the verdict as a JSON envelope on stdout (spec 034)\n  -h, --help\n";
    assert!(takes_json(recorded));
    assert!(!takes_json(
        "Usage: spec-spine check [OPTIONS]\n      --repo <DIR>\n  -h, --help\n"
    ));
    // A mention in prose is not the flag.
    assert!(!takes_json("Emits JSON with --jsonl someday\n"));
}

#[cfg(unix)]
#[test]
fn a_stderr_notice_does_not_claim_json_support() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("spec-spine.toml"), "").unwrap();
    let bin = dir.path().join("spec-spine");
    let script = "#!/bin/sh\ncase \"$*\" in\n  'check --help') \
echo 'Usage: spec-spine check'; echo 'notice: --json changes soon' >&2 ;;\n  check) \
echo 'check: fresh'; exit 0 ;;\n  'check --json') echo 'wrong reader' >&2; exit 4 ;;\nesac\n";
    fixture::with_script(&bin, script, || {
        assert_eq!(run_check(bin.to_str().unwrap(), &repo), CheckAnswer::Fresh);
    });
}

#[cfg(unix)]
#[test]
fn an_engine_whose_check_takes_json_is_read_from_its_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("spec-spine.toml"), "").unwrap();
    let bin = dir.path().join("spec-spine");
    let stale = STALE.replace('\n', " ");
    let script = format!(
        "#!/bin/sh\ncase \"$*\" in\n  'check --help') echo '      --json' ;;\n  \
         'check --json') printf '%s\n' '{stale}'; exit 1 ;;\n  \
         check) echo 'this engine was read by its words' >&2; exit 1 ;;\nesac\n"
    );
    fixture::with_script(&bin, &script, || {
        match run_check(bin.to_str().unwrap(), &repo) {
            CheckAnswer::Stale { reading, .. } => assert_eq!(reading, StaleReading::Stale),
            other => panic!("{other:?}"),
        }
    });
}

/// Keep fixture installation and child execution in one serialized lifetime.
/// A concurrent spawn can inherit another thread's writable script descriptor
/// until exec closes it, even after the writer itself has closed and renamed
/// the file. Staging alone therefore does not prevent Linux `ETXTBSY`.
#[cfg(unix)]
mod fixture {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::Mutex;

    static SCRIPT_LIFETIME: Mutex<()> = Mutex::new(());

    pub fn with_script<T>(at: &Path, script: &str, run: impl FnOnce() -> T) -> T {
        let _lifetime = SCRIPT_LIFETIME.lock().unwrap();
        let staged = at.with_extension("staged");
        std::fs::write(&staged, script).unwrap();
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::rename(&staged, at).unwrap();
        run()
    }
}
