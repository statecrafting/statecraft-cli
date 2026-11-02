//! Spec 052's V-1, against the committed corpus and synthetic captures made
//! from it. Nothing here spawns a provider: every capture is a recorded
//! golden, relabelled or edited line by line.

use serde_json::{Value, json};
use statecraft_adapter::manifest::SUITE_VERSION;
use statecraft_adapter_claude_code::golden::{
    CAPTURE_MANIFEST, CaptureSet, Corpus, GoldenError, Status, compare,
};
use statecraft_adapter_claude_code::record_compared;
use std::fs;
use std::path::{Path, PathBuf};

const NEXT: &str = "2.1.308";
const SCENARIOS: [&str; 5] = [
    "api-error",
    "denied",
    "max-turns",
    "success",
    "tool-removed",
];

fn stream_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/stream")
}

fn golden_text(scenario: &str) -> String {
    fs::read_to_string(stream_dir().join(format!("{scenario}.jsonl"))).unwrap()
}

/// Rewrite every line of a stream through `edit`.
fn rewrite(text: &str, edit: impl Fn(&mut Value)) -> String {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            edit(&mut v);
            format!("{v}\n")
        })
        .collect()
}

/// The same stream, as if recorded from `version`.
fn relabel(text: &str, version: &str) -> String {
    rewrite(text, |v| {
        if v.get("claude_code_version").is_some() {
            v["claude_code_version"] = json!(version);
        }
    })
}

/// A copy of the committed corpus, to edit without touching it.
fn corpus_copy() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for entry in fs::read_dir(stream_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            fs::copy(&path, dir.path().join(path.file_name().unwrap())).unwrap();
        }
    }
    dir
}

/// A capture set at `version` holding each `(scenario, stream)`.
fn capture_set(version: &str, streams: &[(&str, String)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let captures: Vec<Value> = streams
        .iter()
        .map(|(scenario, text)| {
            let file = format!("{scenario}.jsonl");
            fs::write(dir.path().join(&file), text).unwrap();
            json!({
                "scenario": scenario,
                "providerVersion": version,
                "platform": "darwin",
                "invocation": "synthetic, from the committed golden",
                "captured": "2026-11-01",
                "file": file,
            })
        })
        .collect();
    fs::write(
        dir.path().join(CAPTURE_MANIFEST),
        json!({ "captures": captures }).to_string(),
    )
    .unwrap();
    dir
}

/// Every golden, relabelled to `NEXT`, with `scenario` replaced by `text`.
fn next_version_with(scenario: &str, text: String) -> tempfile::TempDir {
    let streams: Vec<(&str, String)> = SCENARIOS
        .iter()
        .map(|s| {
            if *s == scenario {
                (*s, text.clone())
            } else {
                (*s, relabel(&golden_text(s), NEXT))
            }
        })
        .collect();
    capture_set(NEXT, &streams)
}

fn denied_without_its_denial() -> String {
    relabel(
        &rewrite(&golden_text("denied"), |v| {
            if v.get("permission_denials").is_some() {
                v["permission_denials"] = json!([]);
            }
        }),
        NEXT,
    )
}

fn write_ledger(dir: &Path, entries: Value) {
    fs::write(
        dir.join("ledger.json"),
        json!({ "entries": entries }).to_string(),
    )
    .unwrap();
}

fn status(comparison: &statecraft_adapter_claude_code::Comparison, scenario: &str) -> Status {
    comparison
        .rows
        .iter()
        .find(|r| r.scenario == scenario)
        .unwrap()
        .status
}

#[test]
fn the_committed_corpus_is_the_five_recordings_at_2_1_267() {
    let corpus = Corpus::from_dir(&stream_dir()).unwrap();
    let scenarios: Vec<String> = corpus.scenarios().into_iter().collect();
    assert_eq!(scenarios, SCENARIOS);
    for g in &corpus.goldens {
        assert_eq!(g.capture.provider_version, "2.1.267");
        assert_eq!(g.capture.platform, "darwin");
        assert_eq!(g.capture.captured, "2026-09-17");
        assert!(!g.capture.redactions.is_empty());
    }
    assert!(corpus.quarantine.is_empty());
    assert!(corpus.ledger.is_empty());
}

#[test]
fn a_new_version_that_means_the_same_matches_and_is_qualifiable() {
    let corpus = Corpus::from_dir(&stream_dir()).unwrap();
    // Ids, costs, turn counts and timestamps differ every run and are ignored.
    let success = rewrite(&relabel(&golden_text("success"), NEXT), |v| {
        for (field, value) in [
            ("session_id", json!("22222222-2222-2222-2222-222222222222")),
            ("uuid", json!("33333333-3333-3333-3333-333333333333")),
            ("total_cost_usd", json!(0.5)),
            ("num_turns", json!(4)),
            ("duration_ms", json!(9)),
        ] {
            if v.get(field).is_some() {
                v[field] = value;
            }
        }
    });
    let set = CaptureSet::from_dir(next_version_with("success", success).path()).unwrap();
    let comparison = compare(&corpus, &set).unwrap();
    assert!(comparison.rows.iter().all(|r| r.status == Status::Matches));
    assert!(comparison.qualifiable);
    let record = record_compared(&comparison, SUITE_VERSION, "2026-11-01T00:00:00Z").unwrap();
    assert_eq!(record.provider_version, NEXT);
}

#[test]
fn a_mapped_field_divergence_refuses_the_versions_qualification() {
    let corpus = Corpus::from_dir(&stream_dir()).unwrap();
    let set = CaptureSet::from_dir(next_version_with("denied", denied_without_its_denial()).path())
        .unwrap();
    let comparison = compare(&corpus, &set).unwrap();
    assert_eq!(status(&comparison, "denied"), Status::DivergesUnreviewed);
    let row = comparison
        .rows
        .iter()
        .find(|r| r.scenario == "denied")
        .unwrap();
    assert_eq!(row.differing_fields, ["deniedTools", "outcome"]);
    assert!(!comparison.qualifiable);
    assert_eq!(comparison.blocking, ["denied"]);
    let refused = record_compared(&comparison, SUITE_VERSION, "2026-11-01T00:00:00Z").unwrap_err();
    assert_eq!(refused.scenarios, ["denied"]);
    assert!(refused.to_string().contains("denied"), "{refused}");
}

#[test]
fn a_reviewed_ledger_entry_lets_the_same_capture_pass() {
    let corpus_dir = corpus_copy();
    let captures = next_version_with("denied", denied_without_its_denial());
    let entry = |version: &str, fields: Value| {
        json!({
            "scenario": "denied",
            "providerVersion": version,
            "fields": fields,
            "reason": "synthetic: the comparison's own acceptance",
            "reviewer": "acceptance",
            "date": "2026-11-01",
        })
    };

    // An entry for another version, or one naming only part of the
    // difference, covers nothing.
    write_ledger(
        corpus_dir.path(),
        json!([
            entry("2.1.299", json!(["deniedTools", "outcome"])),
            entry(NEXT, json!(["deniedTools"])),
        ]),
    );
    let corpus = Corpus::from_dir(corpus_dir.path()).unwrap();
    let set = CaptureSet::from_dir(captures.path()).unwrap();
    let comparison = compare(&corpus, &set).unwrap();
    assert_eq!(status(&comparison, "denied"), Status::DivergesUnreviewed);

    write_ledger(
        corpus_dir.path(),
        json!([entry(NEXT, json!(["deniedTools", "outcome"]))]),
    );
    let corpus = Corpus::from_dir(corpus_dir.path()).unwrap();
    let comparison = compare(&corpus, &set).unwrap();
    assert_eq!(status(&comparison, "denied"), Status::DivergesReviewed);
    assert_eq!(
        comparison
            .rows
            .iter()
            .find(|r| r.scenario == "denied")
            .unwrap()
            .ledger_entry,
        Some(0)
    );
    assert!(comparison.qualifiable);
    assert!(record_compared(&comparison, SUITE_VERSION, "2026-11-01T00:00:00Z").is_ok());
}

#[test]
fn a_ledger_entry_must_be_reviewed_name_mapped_fields_and_be_appended() {
    let corpus_dir = corpus_copy();
    let entry = |reviewer: &str, field: &str, date: &str| {
        json!({
            "scenario": "denied",
            "providerVersion": NEXT,
            "fields": [field],
            "reason": "synthetic",
            "reviewer": reviewer,
            "date": date,
        })
    };
    for entries in [
        json!([entry("", "outcome", "2026-11-01")]),
        json!([entry("acceptance", "total_cost_usd", "2026-11-01")]),
        json!([
            entry("acceptance", "outcome", "2026-11-02"),
            entry("acceptance", "outcome", "2026-11-01"),
        ]),
    ] {
        write_ledger(corpus_dir.path(), entries.clone());
        assert!(
            matches!(
                Corpus::from_dir(corpus_dir.path()),
                Err(GoldenError::Invalid(_))
            ),
            "{entries}"
        );
    }
}

#[test]
fn a_quarantined_golden_is_kept_skipped_and_reported() {
    let corpus_dir = corpus_copy();
    let root = corpus_dir.path();
    fs::create_dir(root.join("quarantine")).unwrap();
    fs::rename(
        root.join("tool-removed.jsonl"),
        root.join("quarantine/tool-removed.jsonl"),
    )
    .unwrap();
    let mut manifest: Value =
        serde_json::from_str(&fs::read_to_string(root.join("corpus.json")).unwrap()).unwrap();
    manifest["captures"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c["scenario"] != "tool-removed");
    fs::write(root.join("corpus.json"), manifest.to_string()).unwrap();
    let quarantine = |reason: &str, file: &str, lifted: Value| {
        json!({ "entries": [{
            "scenario": "tool-removed",
            "providerVersion": "2.1.267",
            "platform": "darwin",
            "file": file,
            "reason": reason,
            "evidence": "synthetic: the comparison's own acceptance",
            "quarantined": "2026-11-01",
            "liftedBy": lifted,
        }]})
        .to_string()
    };

    fs::write(
        root.join("quarantine.json"),
        quarantine(
            "a wrong recording",
            "quarantine/tool-removed.jsonl",
            Value::Null,
        ),
    )
    .unwrap();
    let corpus = Corpus::from_dir(root).unwrap();
    let streams: Vec<(&str, String)> = SCENARIOS
        .iter()
        .filter(|s| **s != "tool-removed")
        .map(|s| (*s, relabel(&golden_text(s), NEXT)))
        .collect();
    let set = CaptureSet::from_dir(capture_set(NEXT, &streams).path()).unwrap();
    let comparison = compare(&corpus, &set).unwrap();
    let row = comparison
        .rows
        .iter()
        .find(|r| r.scenario == "tool-removed")
        .unwrap();
    assert_eq!(row.status, Status::Quarantined);
    assert_eq!(row.quarantine_reason.as_deref(), Some("a wrong recording"));
    assert!(
        comparison.qualifiable,
        "a quarantined scenario does not block"
    );

    // R-2: no reason, no kept recording, or a lift with no fresh golden is
    // refused rather than read as an exclusion.
    for (text, why) in [
        (
            quarantine("", "quarantine/tool-removed.jsonl", Value::Null),
            "no reason",
        ),
        (
            quarantine("wrong", "quarantine/gone.jsonl", Value::Null),
            "deleted",
        ),
        (
            quarantine("wrong", "tool-removed.jsonl", Value::Null),
            "not kept under quarantine/",
        ),
        (
            quarantine(
                "wrong",
                "quarantine/tool-removed.jsonl",
                json!("success.jsonl"),
            ),
            "lifted by another scenario's golden",
        ),
    ] {
        fs::write(root.join("quarantine.json"), text).unwrap();
        assert!(Corpus::from_dir(root).is_err(), "{why}");
    }
}

#[test]
fn a_missing_scenario_blocks_qualification() {
    let corpus = Corpus::from_dir(&stream_dir()).unwrap();
    let streams: Vec<(&str, String)> = SCENARIOS
        .iter()
        .filter(|s| **s != "max-turns")
        .map(|s| (*s, relabel(&golden_text(s), NEXT)))
        .collect();
    let set = CaptureSet::from_dir(capture_set(NEXT, &streams).path()).unwrap();
    let comparison = compare(&corpus, &set).unwrap();
    assert_eq!(status(&comparison, "max-turns"), Status::Missing);
    assert_eq!(comparison.blocking, ["max-turns"]);
}

#[test]
fn a_capture_whose_version_or_scenario_does_not_match_is_refused() {
    let corpus = Corpus::from_dir(&stream_dir()).unwrap();

    // The entry says 2.1.308; the stream says 2.1.267.
    let mislabelled = capture_set(NEXT, &[("success", golden_text("success"))]);
    assert!(matches!(
        CaptureSet::from_dir(mislabelled.path()),
        Err(GoldenError::Refused(_))
    ));

    // A scenario no golden names.
    let unknown = capture_set(
        NEXT,
        &[("unheard-of", relabel(&golden_text("success"), NEXT))],
    );
    let set = CaptureSet::from_dir(unknown.path()).unwrap();
    assert!(matches!(
        compare(&corpus, &set),
        Err(GoldenError::Refused(_))
    ));

    // A golden whose stream does not say what its entry says.
    let corpus_dir = corpus_copy();
    fs::write(
        corpus_dir.path().join("success.jsonl"),
        relabel(&golden_text("success"), NEXT),
    )
    .unwrap();
    assert!(matches!(
        Corpus::from_dir(corpus_dir.path()),
        Err(GoldenError::Refused(_))
    ));
}
