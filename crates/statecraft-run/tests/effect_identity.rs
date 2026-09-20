//! Spec 003 section 3.3.1: effect identity, one-to-one closing, and the cases
//! the legacy pairing of section 3.6 cannot decide.
//!
//! Every test here reads the record and nothing else. The support is reachable
//! from no verb: `fold_effects` is called here and in no other place.

use serde::Deserialize;
use serde_json::json;
use statecraft_run::record::{Chain, EffectId, Entry, Identity, Kind};
use statecraft_run::recovery::{EffectFold, FoldDefect, fold_effects, unmatched_intents};

/// A valid identity, or a panic: a test that meant to write one and wrote
/// nothing would assert against the wrong contract.
fn id(s: &str) -> Identity {
    Identity::Valid(EffectId::new(s).expect("a test identity is never empty"))
}

fn entry(kind: Kind, run: &str, attempt: u32, subject: &str, effect_id: Identity) -> Entry {
    Entry {
        kind,
        run_id: run.to_string(),
        attempt,
        subject: subject.to_string(),
        idempotency_key: None,
        detail: serde_json::Value::Null,
        effect_id,
    }
}

/// Append records to a fresh chain and hand it back, reopened from disk, so
/// every assertion is about what was written rather than what was held.
fn chain_of(
    home: &tempfile::TempDir,
    target: &tempfile::TempDir,
    entries: &[Entry],
) -> statecraft_run::record::Chain {
    let (mut chain, _) = Chain::open(home.path(), target.path()).expect("open");
    for (i, e) in entries.iter().enumerate() {
        chain
            .append(&format!("r{i}"), "2026-09-19T00:00:00Z", e)
            .expect("append");
    }
    let (chain, _) = Chain::open(home.path(), target.path()).expect("reopen");
    chain
}

fn homes() -> (tempfile::TempDir, tempfile::TempDir) {
    (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
}

fn keys(fold: &EffectFold) -> Vec<String> {
    fold.closed().iter().map(|k| k.to_string()).collect()
}

// ---------------------------------------------------------------------------
// 1. The decisive case: a partially closed pair.
// ---------------------------------------------------------------------------

/// Two effects bracketed in one attempt under one subject, one of them closed.
///
/// The legacy pairing cannot see this. `unmatched_intents` collects the
/// `(run_id, attempt, subject)` of **every** outcome into a set and keeps an
/// intent only when its own triple is absent from that set. Both intents carry
/// the same triple and the single outcome contributes it, so the membership
/// test succeeds for both and the fold reports zero unmatched intents: the
/// intent for `e2`, whose effect was never closed, reads as closed. Under
/// section 3.6 recovery then never reconciles it, nothing blocks, and a retry
/// proceeds as though the effect had finished.
#[test]
fn a_partially_closed_pair_keeps_the_open_effect_and_its_identity() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Intent, "run-1", 1, "publish", id("e2")),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
        ],
    );

    let fold = fold_effects(&chain);
    assert_eq!(fold.open().len(), 1, "one effect was never closed");
    assert_eq!(fold.open()[0].key.run_id, "run-1");
    assert_eq!(fold.open()[0].key.effect_id.as_str(), "e2");
    assert_eq!(fold.open()[0].intent.subject, "publish");
    assert_eq!(keys(&fold), vec!["(run-1, e1)".to_string()]);
    assert!(fold.ambiguous().is_empty());
    assert!(fold.is_clean(), "nothing here is a defect");
}

/// The same three records with no identities, folded by the legacy pairing.
///
/// Recorded, not desired: this is the behavior clause 7 preserves for records
/// written before section 3.3.1, and the finding it leaves standing.
#[test]
fn the_legacy_pairing_reports_the_same_shape_as_fully_closed() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", Identity::Absent),
            entry(Kind::Intent, "run-1", 1, "publish", Identity::Absent),
            entry(Kind::Outcome, "run-1", 1, "publish", Identity::Absent),
        ],
    );
    assert!(
        unmatched_intents(&chain).is_empty(),
        "one outcome closes every intent sharing the triple"
    );
    assert_eq!(
        fold_effects(&chain),
        EffectFold::default(),
        "and no identity was carried"
    );
}

// ---------------------------------------------------------------------------
// 2 to 6, 8 to 11: the identity rules.
// ---------------------------------------------------------------------------

#[test]
fn a_fully_closed_pair_pairs_rather_than_merely_splits() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Intent, "run-1", 1, "publish", id("e2")),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e2")),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
        ],
    );
    let fold = fold_effects(&chain);
    assert!(fold.open().is_empty());
    assert_eq!(
        keys(&fold),
        vec!["(run-1, e1)".to_string(), "(run-1, e2)".to_string()]
    );
    assert!(fold.is_clean());
}

#[test]
fn a_closed_identity_is_not_closed_again() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
        ],
    );
    let fold = fold_effects(&chain);
    assert_eq!(fold.defects().len(), 1);
    match &fold.defects()[0] {
        FoldDefect::DoubleClose { key, first, second } => {
            assert_eq!(key.effect_id.as_str(), "e1");
            assert_eq!(key.run_id, "run-1");
            assert_eq!((*first, *second), (1, 2));
        }
        other => panic!("expected a double close, got {other:?}"),
    }
    assert_eq!(
        keys(&fold),
        vec!["(run-1, e1)".to_string()],
        "the first outcome stands"
    );
    assert!(fold.open().is_empty());
}

#[test]
fn an_outcome_with_no_intent_closes_nothing_and_is_reported() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[entry(Kind::Outcome, "run-1", 1, "publish", id("ghost"))],
    );
    let fold = fold_effects(&chain);
    assert!(fold.closed().is_empty(), "it closes nothing");
    assert!(fold.open().is_empty());
    match &fold.defects()[0] {
        FoldDefect::OrphanOutcome { key, record_index } => {
            assert_eq!(key.effect_id.as_str(), "ghost");
            assert_eq!(*record_index, 0);
        }
        other => panic!("expected an orphan outcome, got {other:?}"),
    }
}

#[test]
fn an_empty_identity_is_invalid_and_is_not_an_absent_one() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[entry(
            Kind::Intent,
            "run-1",
            1,
            "publish",
            Identity::Invalid(json!("")),
        )],
    );
    let fold = fold_effects(&chain);
    assert!(fold.open().is_empty());
    assert!(fold.closed().is_empty());
    match &fold.defects()[0] {
        FoldDefect::InvalidIdentity { run_id, raw, .. } => {
            assert_eq!(run_id, "run-1");
            assert_eq!(*raw, json!(""));
        }
        other => panic!("expected an invalid identity, got {other:?}"),
    }
    assert!(
        unmatched_intents(&chain).is_empty(),
        "an invalid identity is present, so the legacy pairing does not claim it"
    );
}

#[test]
fn a_non_string_identity_is_invalid_and_keeps_its_value() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[entry(
            Kind::Intent,
            "run-1",
            1,
            "publish",
            Identity::Invalid(json!(7)),
        )],
    );
    let fold = fold_effects(&chain);
    match &fold.defects()[0] {
        FoldDefect::InvalidIdentity { raw, .. } => assert_eq!(*raw, json!(7)),
        other => panic!("expected an invalid identity, got {other:?}"),
    }
    assert!(unmatched_intents(&chain).is_empty());
}

/// The test an `Option<EffectId>` fails: serde maps a present `null` and a
/// missing key to the same `None`, and clause 3 forbids reading them as one.
#[test]
fn an_explicit_null_is_present_and_a_missing_key_is_not() {
    let (home, target) = homes();
    let with_null = entry(
        Kind::Intent,
        "run-1",
        1,
        "publish",
        Identity::Invalid(json!(null)),
    );
    let with_none = entry(Kind::Intent, "run-2", 1, "publish", Identity::Absent);

    let null_json = serde_json::to_value(&with_null).unwrap();
    assert_eq!(
        null_json.get("effectId"),
        Some(&json!(null)),
        "the key is written"
    );
    let none_json = serde_json::to_value(&with_none).unwrap();
    assert!(none_json.get("effectId").is_none(), "no key at all");

    assert_eq!(
        serde_json::from_value::<Entry>(null_json)
            .unwrap()
            .effect_id,
        Identity::Invalid(json!(null)),
        "a present null decodes as invalid, never as absent"
    );
    assert_eq!(
        serde_json::from_value::<Entry>(none_json)
            .unwrap()
            .effect_id,
        Identity::Absent
    );

    let chain = chain_of(&home, &target, &[with_null, with_none]);
    let fold = fold_effects(&chain);
    assert_eq!(fold.defects().len(), 1, "the null record, and only it");
    match &fold.defects()[0] {
        FoldDefect::InvalidIdentity { run_id, raw, .. } => {
            assert_eq!(run_id, "run-1");
            assert_eq!(*raw, json!(null));
        }
        other => panic!("expected an invalid identity, got {other:?}"),
    }
    assert!(fold.open().is_empty());

    let legacy = unmatched_intents(&chain);
    assert_eq!(
        legacy.len(),
        1,
        "only the key-absent record is the legacy pairing's"
    );
    assert_eq!(legacy[0].run_id, "run-2");
}

#[test]
fn two_intents_of_one_run_with_one_identity_are_both_kept_and_ambiguous() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Intent, "run-1", 2, "publish", id("e1")),
        ],
    );
    let fold = fold_effects(&chain);
    match &fold.defects()[0] {
        FoldDefect::DuplicateIntent {
            key,
            first,
            second,
            second_follows_closure,
        } => {
            assert_eq!(key.effect_id.as_str(), "e1");
            assert_eq!((*first, *second), (0, 1));
            assert!(!second_follows_closure);
        }
        other => panic!("expected a duplicate intent, got {other:?}"),
    }
    assert!(fold.open().is_empty(), "not reported as one open effect");
    assert_eq!(fold.ambiguous().len(), 1);
    assert!(fold.closed().is_empty());
}

#[test]
fn closure_does_not_release_an_identity_for_reuse() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
            entry(Kind::Intent, "run-1", 2, "publish", id("e1")),
        ],
    );
    let fold = fold_effects(&chain);
    assert_eq!(fold.defects().len(), 1);
    match &fold.defects()[0] {
        FoldDefect::DuplicateIntent {
            second_follows_closure,
            first,
            second,
            ..
        } => {
            assert!(
                second_follows_closure,
                "the report says which shape it found"
            );
            assert_eq!((*first, *second), (0, 2));
        }
        other => panic!("expected a duplicate intent, got {other:?}"),
    }
    assert_eq!(
        fold.ambiguous().len(),
        1,
        "ambiguous from the second intent onward"
    );
    assert!(fold.closed().is_empty());
}

#[test]
fn an_outcome_for_an_ambiguous_identity_closes_nothing() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
        ],
    );
    let fold = fold_effects(&chain);
    assert_eq!(fold.defects().len(), 2, "the duplicate and the close");
    assert!(
        fold.defects().iter().any(|d| matches!(
            d,
            FoldDefect::AmbiguousClose {
                record_index: 2,
                ..
            }
        )),
        "got {:?}",
        fold.defects()
    );
    assert!(fold.closed().is_empty());
    assert_eq!(fold.ambiguous().len(), 1);
}

#[test]
fn one_identity_string_in_two_runs_is_two_effects() {
    for closed_run in ["run-a", "run-b"] {
        let (home, target) = homes();
        let open_run = if closed_run == "run-a" {
            "run-b"
        } else {
            "run-a"
        };
        let chain = chain_of(
            &home,
            &target,
            &[
                entry(Kind::Intent, "run-a", 1, "publish", id("e1")),
                entry(Kind::Intent, "run-b", 1, "publish", id("e1")),
                entry(Kind::Outcome, closed_run, 1, "publish", id("e1")),
            ],
        );
        let fold = fold_effects(&chain);
        assert!(
            fold.is_clean(),
            "uniqueness is within a run: {:?}",
            fold.defects()
        );
        assert_eq!(fold.open().len(), 1);
        assert_eq!(fold.open()[0].key.run_id, open_run);
        assert_eq!(fold.open()[0].key.effect_id.as_str(), "e1");
        assert_eq!(fold.closed().len(), 1);
        assert_eq!(fold.closed()[0].run_id, closed_run);
    }
}

// ---------------------------------------------------------------------------
// 12 to 15: compatibility, separation, and the backlog.
// ---------------------------------------------------------------------------

#[test]
fn a_record_without_an_identity_is_written_exactly_as_before() {
    let e = entry(
        Kind::Intent,
        "run-1",
        1,
        "prepare-workspace",
        Identity::Absent,
    );
    assert_eq!(
        serde_json::to_string(&e).unwrap(),
        r#"{"kind":"intent","run_id":"run-1","attempt":1,"subject":"prepare-workspace","detail":null}"#,
        "no effectId key, and every other key as it was"
    );

    let written_before: Entry = serde_json::from_value(json!({
        "kind": "outcome",
        "run_id": "run-1",
        "attempt": 1,
        "subject": "attempt",
        "detail": {"outcome": "completed"}
    }))
    .expect("a record written before the field decodes");
    assert_eq!(written_before.effect_id, Identity::Absent);
}

/// Separation under the hardest shape the legacy key allows: one run, one
/// attempt, one subject, and all three identity states present at once.
///
/// The legacy pairing keys on `(run_id, attempt, subject)`, which every record
/// here shares, so if either rule reached the other's records the outcomes
/// below would close the wrong intent.
#[test]
fn an_identified_or_invalid_outcome_cannot_close_a_legacy_intent() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", Identity::Absent),
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(
                Kind::Intent,
                "run-1",
                1,
                "publish",
                Identity::Invalid(json!(null)),
            ),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
            entry(
                Kind::Outcome,
                "run-1",
                1,
                "publish",
                Identity::Invalid(json!(false)),
            ),
        ],
    );

    let legacy = unmatched_intents(&chain);
    assert_eq!(
        legacy.len(),
        1,
        "the key-absent intent is still open: neither the identified outcome \
         nor the invalid one is the legacy pairing's record"
    );
    assert_eq!(legacy[0].subject, "publish");

    let fold = fold_effects(&chain);
    assert_eq!(
        keys(&fold),
        vec!["(run-1, e1)".to_string()],
        "the identified outcome closes its own effect and nothing else"
    );
    assert!(fold.open().is_empty());
    assert_eq!(
        fold.defects().len(),
        2,
        "both invalid records are reported and folded by neither rule: {:?}",
        fold.defects()
    );
    assert!(
        fold.defects()
            .iter()
            .all(|d| matches!(d, FoldDefect::InvalidIdentity { .. })),
        "got {:?}",
        fold.defects()
    );
}

/// The mirror: an outcome carrying no identity closes no identified effect,
/// even when its run, attempt and subject are the identified intent's.
#[test]
fn an_identity_free_outcome_cannot_close_an_identified_intent() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Outcome, "run-1", 1, "publish", Identity::Absent),
        ],
    );

    let fold = fold_effects(&chain);
    assert_eq!(fold.open().len(), 1, "the identified effect is still open");
    assert_eq!(fold.open()[0].key.effect_id.as_str(), "e1");
    assert!(fold.closed().is_empty());
    assert!(fold.is_clean(), "and an absent key is not a defect");

    assert!(
        unmatched_intents(&chain).is_empty(),
        "the legacy pairing has an outcome and no intent of its own, and it \
         never reaches the identified one"
    );
}

/// Correlation is `(run_id, effectId)` and nothing else (clause 2): an outcome
/// closes its effect across a different attempt and a different subject.
#[test]
fn the_identity_correlates_across_a_different_attempt_and_subject() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Intent, "run-1", 1, "prepare", id("e1")),
            entry(Kind::Outcome, "run-1", 7, "something-else", id("e1")),
        ],
    );

    let fold = fold_effects(&chain);
    assert!(
        fold.open().is_empty(),
        "attempt and subject are not consulted, so the effect is closed"
    );
    assert_eq!(keys(&fold), vec!["(run-1, e1)".to_string()]);
    assert!(fold.is_clean());
}

/// The permanently unmatched `prepare-workspace` intents are a separate
/// finding. Clause 7 preserves them, and this pins them so a later change
/// cannot repair the backlog by accident.
#[test]
fn the_legacy_unmatched_backlog_is_preserved() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(
                Kind::Intent,
                "run-1",
                1,
                statecraft_run::session::INTENT_SUBJECT,
                Identity::Absent,
            ),
            entry(
                Kind::Outcome,
                "run-1",
                1,
                statecraft_run::session::OUTCOME_SUBJECT,
                Identity::Absent,
            ),
        ],
    );
    let legacy = unmatched_intents(&chain);
    assert_eq!(
        legacy.len(),
        1,
        "the subjects differ, so the intent is never closed"
    );
    assert_eq!(legacy[0].subject, statecraft_run::session::INTENT_SUBJECT);
    assert_eq!(
        fold_effects(&chain),
        EffectFold::default(),
        "and S1 does not repair it"
    );
}

/// A build that does not know the key reads the record otherwise unchanged.
#[test]
fn an_older_reader_ignores_the_key() {
    #[derive(Debug, Deserialize)]
    struct OlderEntry {
        kind: Kind,
        run_id: String,
        attempt: u32,
        subject: String,
    }

    let payload =
        serde_json::to_value(entry(Kind::Intent, "run-1", 3, "publish", id("e1"))).unwrap();
    let older: OlderEntry = serde_json::from_value(payload).expect("unknown keys are ignored");
    assert_eq!(older.kind, Kind::Intent);
    assert_eq!(older.run_id, "run-1");
    assert_eq!(older.attempt, 3);
    assert_eq!(older.subject, "publish");
}

// ---------------------------------------------------------------------------
// Chain order (section 3.3.1 clause 5, second paragraph).
// ---------------------------------------------------------------------------

/// An outcome recorded before its own intent closes nothing, and the intent
/// that follows it is open rather than retroactively matched.
#[test]
fn an_outcome_before_its_intent_is_an_orphan_and_the_intent_stays_open() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
        ],
    );

    let fold = fold_effects(&chain);
    assert_eq!(fold.defects().len(), 1, "got {:?}", fold.defects());
    match &fold.defects()[0] {
        FoldDefect::OrphanOutcome { key, record_index } => {
            assert_eq!(key.run_id, "run-1");
            assert_eq!(key.effect_id.as_str(), "e1");
            assert_eq!(*record_index, 0, "the outcome's own chain position");
        }
        other => panic!("expected an orphan outcome, got {other:?}"),
    }

    assert_eq!(fold.open().len(), 1, "the later intent is open");
    assert_eq!(fold.open()[0].key.run_id, "run-1");
    assert_eq!(fold.open()[0].key.effect_id.as_str(), "e1");
    assert!(
        fold.closed().is_empty(),
        "a later intent does not retroactively match an earlier outcome"
    );
    assert!(fold.ambiguous().is_empty());
}

/// A subsequent valid outcome closes that intent, and the earlier orphan
/// defect is still reported: closing it later does not erase the malformed
/// sequence that preceded it.
#[test]
fn a_subsequent_outcome_closes_the_intent_and_the_orphan_defect_remains() {
    let (home, target) = homes();
    let chain = chain_of(
        &home,
        &target,
        &[
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
            entry(Kind::Intent, "run-1", 1, "publish", id("e1")),
            entry(Kind::Outcome, "run-1", 1, "publish", id("e1")),
        ],
    );

    let fold = fold_effects(&chain);
    assert_eq!(
        fold.defects().len(),
        1,
        "the orphan is retained and the valid close is not a double close: {:?}",
        fold.defects()
    );
    match &fold.defects()[0] {
        FoldDefect::OrphanOutcome { key, record_index } => {
            assert_eq!(key.run_id, "run-1");
            assert_eq!(key.effect_id.as_str(), "e1");
            assert_eq!(*record_index, 0, "still the first record");
        }
        other => panic!("expected the original orphan outcome, got {other:?}"),
    }

    assert_eq!(
        keys(&fold),
        vec!["(run-1, e1)".to_string()],
        "the third record closes the intent the second one opened"
    );
    assert!(fold.open().is_empty());
    assert!(fold.ambiguous().is_empty());
}
