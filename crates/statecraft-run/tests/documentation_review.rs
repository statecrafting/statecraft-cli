//! Spec 021's run-record half: review and correction records are closed,
//! digest-sealed and append-only; each type is read back in its record order.
//! An unreadable record stays visible. The `run show` projection lists reviews
//! before corrections without judging them (sections 3.4, 3.8 and 3.10).
//! Every chain read is reopened from disk.

use std::path::Path;
use std::process::Command;

use serde_json::json;

use statecraft_environment::time::FixedClock;
use statecraft_run::context::custody_digest;
use statecraft_run::documentation::DocumentError;
use statecraft_run::documentation::review::{
    self, ArtifactDisposition, ArtifactIdentity, ArtifactReview, Authority, ClaimDisposition,
    ClaimReview, CoreTuple, Correction, Listed, Read, Recommendation, RequestedCorrection, Review,
    Reviewer,
};
use statecraft_run::record::{Chain, Entry, Identity, Kind};
use statecraft_run::session::{self, IntentDetail};

const RUN: &str = "run-021";

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        &["init", "--quiet", "--initial-branch=main"][..],
        &["config", "user.email", "test@example.invalid"],
        &["config", "user.name", "test"],
        &["config", "commit.gpgsign", "false"],
    ] {
        git(dir.path(), args);
    }
    std::fs::write(dir.path().join("README.md"), b"x").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "one"]);
    dir
}

fn d(tag: &str) -> String {
    custody_digest(tag.as_bytes())
}

fn subject() -> CoreTuple {
    CoreTuple {
        repository: "example/app".into(),
        trusted_base: "0".repeat(40),
        run_id: RUN.into(),
        attempt: 1,
        candidate: "1".repeat(40),
        context_manifest_digest: d("manifest"),
        task_digest: d("task"),
        result_custody_digest: d("result"),
        result_schema_digest: d("schema"),
        artifacts: vec![ArtifactIdentity {
            logical_path: "docs/parser.md".into(),
            digest: d("artifact"),
        }],
        evidence: vec!["evidence-1".into()],
        policy_digest: d("policy"),
    }
}

fn review(recommendation: Recommendation) -> Review {
    Review {
        schema: review::REVIEW_SCHEMA.into(),
        subject: subject(),
        reviewer: Reviewer {
            identity: "owner@example.invalid".into(),
            authority: Authority::RepositoryOwner,
            authority_source: "policy reviewers[0]".into(),
        },
        reviewed_at: "2026-10-22T00:00:00Z".into(),
        claims: vec![ClaimReview {
            claim_id: "c1".into(),
            disposition: ClaimDisposition::InsufficientGrade,
            reasons: vec!["claims tested with implementation evidence only".into()],
        }],
        artifacts: vec![ArtifactReview {
            logical_path: "docs/parser.md".into(),
            disposition: ArtifactDisposition::NeedsCorrection,
            reasons: vec![],
        }],
        corrections: vec![RequestedCorrection {
            target: "c1".into(),
            request: "state the grade the evidence supports".into(),
        }],
        recommendation,
        digest_construction: String::new(),
        review_digest: String::new(),
    }
    .seal()
}

fn correction() -> Correction {
    Correction {
        schema: review::CORRECTION_SCHEMA.into(),
        subject: subject(),
        supersedes: d("artifact"),
        author: "owner@example.invalid".into(),
        reason: "the grade the evidence supports".into(),
        changed_claims: vec!["c1".into()],
        changed_artifacts: vec![ArtifactIdentity {
            logical_path: "docs/parser.md".into(),
            digest: d("artifact-2"),
        }],
        corrected_result: None,
        digest_construction: String::new(),
        correction_digest: String::new(),
    }
    .seal()
}

fn chain_with_attempt() -> (tempfile::TempDir, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    session::begin_with(
        &mut chain,
        target.path(),
        RUN,
        "main",
        &FixedClock(1_790_000_000),
        &IntentDetail::default(),
    )
    .unwrap();
    (home, target)
}

// Section 3.4: a review is closed and sealed; it names the tuple, not itself.
#[test]
fn a_review_decodes_from_its_own_bytes_and_its_digest_covers_every_member() {
    let r = review(Recommendation::Revise);
    let bytes = serde_json::to_vec(&r).unwrap();
    assert_eq!(Review::decode(&bytes).unwrap(), r);
    let mut v = serde_json::to_value(&r).unwrap();
    v["recommendation"] = json!("accept");
    assert!(matches!(
        Review::decode(&serde_json::to_vec(&v).unwrap()),
        Err(DocumentError::Digest { .. })
    ));
    let mut v = serde_json::to_value(&r).unwrap();
    v["approved"] = json!(true);
    assert!(matches!(
        Review::decode(&serde_json::to_vec(&v).unwrap()),
        Err(DocumentError::Malformed(_))
    ));
    let mut v = serde_json::to_value(&r).unwrap();
    v["reviewer"]["authority"] = json!("provider");
    assert!(matches!(
        Review::decode(&serde_json::to_vec(&v).unwrap()),
        Err(DocumentError::Malformed(_))
    ));
}

// Section 3.1: repeated evidence identities cannot form an unambiguous subject.
#[test]
fn reviews_and_corrections_refuse_non_adjacent_duplicate_evidence() {
    let evidence = vec![
        "evidence-1".into(),
        "evidence-2".into(),
        "evidence-1".into(),
    ];
    let mut r = review(Recommendation::Accept);
    r.subject.evidence = evidence.clone();
    let bytes = serde_json::to_vec(&r.seal()).unwrap();
    assert!(matches!(
        Review::decode(&bytes),
        Err(DocumentError::Duplicate(_))
    ));

    let mut c = correction();
    c.subject.evidence = evidence;
    let bytes = serde_json::to_vec(&c.seal()).unwrap();
    assert!(matches!(
        Correction::decode(&bytes),
        Err(DocumentError::Duplicate(_))
    ));
}

#[test]
fn a_review_with_a_repeated_claim_or_an_escaping_artifact_refuses() {
    let mut r = review(Recommendation::Accept);
    r.claims.push(r.claims[0].clone());
    assert!(matches!(
        r.seal().validate(),
        Err(DocumentError::Duplicate(_))
    ));
    let mut r = review(Recommendation::Accept);
    r.artifacts[0].logical_path = "../outside.md".into();
    assert!(matches!(
        r.seal().validate(),
        Err(DocumentError::Invalid(_))
    ));
}

// Section 3.8: a correction names what it supersedes and changes something.
#[test]
fn a_correction_must_change_something_and_name_what_it_supersedes() {
    let c = correction();
    assert_eq!(
        Correction::decode(&serde_json::to_vec(&c).unwrap()).unwrap(),
        c
    );
    let mut empty = correction();
    empty.changed_claims.clear();
    empty.changed_artifacts.clear();
    assert!(matches!(
        empty.seal().validate(),
        Err(DocumentError::Invalid(_))
    ));
    // A claim or an artifact is changed once.
    let mut twice = correction();
    twice.changed_claims.push("c1".into());
    assert!(matches!(
        twice.seal().validate(),
        Err(DocumentError::Duplicate(_))
    ));
    let mut twice = correction();
    let again = twice.changed_artifacts[0].clone();
    twice.changed_artifacts.push(again);
    assert!(matches!(
        twice.seal().validate(),
        Err(DocumentError::Duplicate(_))
    ));
}

// Sections 3.4 and 3.10: each type retains its append order, including unreadable
// records. The identity projection groups reviews before corrections; it does
// not interleave the two types chronologically.
#[test]
fn each_record_type_keeps_append_order_and_identities_list_reviews_first() {
    let (home, target) = chain_with_attempt();
    let first = review(Recommendation::Revise);
    let second = review(Recommendation::Accept);
    {
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        review::record_review(&mut chain, &first, "2026-10-22T00:00:01Z").unwrap();
        review::record_correction(&mut chain, &correction(), "2026-10-22T00:00:02Z").unwrap();
        review::record_review(&mut chain, &second, "2026-10-22T00:00:03Z").unwrap();
        chain
            .append(
                &format!("{RUN}/1/{}/x", review::REVIEW_SUBJECT),
                "2026-10-22T00:00:04Z",
                &Entry {
                    kind: Kind::Evidence,
                    run_id: RUN.into(),
                    attempt: 1,
                    subject: review::REVIEW_SUBJECT.into(),
                    idempotency_key: None,
                    detail: json!({ "record": { "schema": "free-form approval" } }),
                    effect_id: Identity::Absent,
                },
            )
            .unwrap();
    }
    let (chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let reviews = review::recorded_reviews(&chain, RUN, 1);
    assert_eq!(reviews.len(), 3);
    assert_eq!(reviews[0], Read::Valid(first.clone()));
    assert_eq!(reviews[1], Read::Valid(second.clone()));
    assert!(matches!(reviews[2], Read::Unreadable(_)));
    assert_eq!(review::recorded_corrections(&chain, RUN, 1).len(), 1);
    assert!(review::recorded_reviews(&chain, RUN, 2).is_empty());

    let listed = review::identities(&chain, RUN, 1);
    assert_eq!(
        listed[0],
        Listed::Review {
            digest: first.review_digest.clone(),
            reviewer: "owner@example.invalid".into(),
            recommendation: Recommendation::Revise,
        }
    );
    assert!(matches!(listed[2], Listed::Unreadable { .. }));
    assert_eq!(
        listed[3],
        Listed::Correction {
            digest: correction().correction_digest,
            supersedes: d("artifact"),
        }
    );
}
