//! Spec 005 section 3.10, the observable negative cases, one test per row.
//!
//! Named after the row each covers, so a row that stops being covered shows up
//! as a deleted test rather than as a quietly weakened assertion.

use statecraft_acceptance::absence::{Absence, Recorded, Statement};
use statecraft_acceptance::authority::{
    CorpusVerdict, Declared, ENVIRONMENT_MANIFEST, NoDeltaReport, StaticDeltaReport, evaluate,
};
use statecraft_acceptance::dimensions::{
    Admission, AdmissionPolicy, Dimensions, Integrity, IssuerTrust, RefusalCode, Signature,
    SubjectBinding, admit,
};
use statecraft_acceptance::evidence::{Construction, Reference, integrity};
use statecraft_acceptance::independence::{Check, SuiteResult};
use statecraft_acceptance::judged::{
    Base, Candidate, Judged, NoAcceptance, NotAttemptedReason, Policy, eligibility,
};
use statecraft_acceptance::outcome::{Acceptance, freshness_field};
use statecraft_acceptance::receipt::{MintContext, NoReceipt, freshness, mint};
use statecraft_acceptance::trust::{Anchor, RootSet, issuer_trust};
use statecraft_run::attempt::Outcome;

fn judged(head_stable: bool, clean: bool) -> Judged {
    Judged {
        candidate: Candidate {
            sha: "c".repeat(40),
            work_tree_clean: clean,
            head_stable,
            dirty_paths: if clean {
                vec![]
            } else {
                vec!["src/leftover.rs".into()]
            },
        },
        base: Base {
            sha: "b".repeat(40),
        },
        policy: Policy {
            digest: "d".repeat(64),
        },
    }
}

fn context() -> MintContext {
    MintContext {
        repository: "statecraft-cli".into(),
        product_version: "0.0.0".into(),
        spec_spine_version: "spec-spine 0.18.0".into(),
        adapter_version: "1.0.0".into(),
        attempt: "run-1/1".into(),
        authority_paths_touched: vec![],
    }
}

fn declared() -> Declared {
    Declared::none()
        .declaring("Makefile", "check-suite")
        .declaring(".github/workflows/", "check-suite")
        .declaring("scripts/", "verifier")
        .declaring(".claude/hooks/", "hooks")
}

fn no_report() -> NoDeltaReport {
    NoDeltaReport {
        spec_spine_version: "spec-spine 0.18.0".into(),
    }
}

// Row 1: the agent reports success; the suite fails.
#[test]
fn an_agent_claiming_success_over_a_failing_suite_yields_no_receipt_and_the_claim_is_retained() {
    let suite = SuiteResult::new(
        vec![Check::ran("make gate", 1, None)],
        Some("I finished the work and everything passes".into()),
    );
    assert!(!suite.passed());
    assert!(suite.agent_claim.is_some(), "the claim is retained");

    match mint(&judged(true, true), &suite, &context(), None) {
        Err(NoReceipt::SuiteDidNotPass { .. }) => {}
        other => panic!("expected no receipt, got {other:?}"),
    }

    // And it is kept in a field named for a claim, never merged with what was
    // observed.
    let claim = Statement::Narrative {
        text: suite.agent_claim.clone().unwrap(),
    };
    assert!(!claim.is_observed());
}

// Row 2: the agent reports success; the suite never ran.
#[test]
fn an_agent_claiming_success_over_a_suite_that_never_ran_is_no_acceptance_with_the_unrun_count() {
    let suite = SuiteResult::new(
        vec![
            Check::did_not_run("make gate", "the runner never started"),
            Check::did_not_run("make code", "the runner never started"),
        ],
        Some("all green".into()),
    );
    assert!(!suite.ran());
    assert!(!suite.passed(), "not a pass");
    assert_eq!(suite.unrun_count(), 2);

    let none = NoAcceptance::SuiteDidNotRun {
        unrun_checks: suite.unrun_count(),
    };
    let acceptance = Acceptance::None { reason: none };
    assert_eq!(acceptance.word(), "no-acceptance", "not a pass, not a fail");
}

// Row 3: the attempt outcome is refused, failed, interrupted or cancelled.
#[test]
fn every_ineligible_attempt_outcome_is_not_attempted_with_its_reason_named() {
    let expected = [
        (Outcome::Failed, "attempt-failed"),
        (Outcome::Refused, "attempt-refused"),
        (Outcome::Interrupted, "attempt-interrupted"),
        (Outcome::Cancelled, "attempt-cancelled"),
    ];
    for (outcome, word) in expected {
        let reason = eligibility(outcome).unwrap_err();
        assert_eq!(reason.word(), word);
        let a = Acceptance::NotAttempted {
            reason,
            refusal_count: None,
        };
        assert_eq!(a.word(), "not-attempted");
        assert!(a.receipt().is_none(), "no receipt");
    }
    assert!(eligibility(Outcome::Completed).is_ok());
}

#[test]
fn a_refused_attempt_carries_the_refusal_count_beside_the_reason() {
    let a = Acceptance::NotAttempted {
        reason: NotAttemptedReason::AttemptRefused,
        refusal_count: Some(2),
    };
    let json = serde_json::to_string(&a).unwrap();
    assert!(json.contains("attempt-refused") && json.contains('2'));
}

// Row 4: the installed spec-spine carries no delta report.
#[test]
fn with_no_delta_report_the_verdict_is_not_recorded_and_acceptance_is_still_refused() {
    let v = evaluate(
        &["crates/x/src/lib.rs".to_string()],
        &declared(),
        &no_report(),
    );

    assert_eq!(
        v.corpus_members,
        CorpusVerdict::Absent(Absence::NotRecorded)
    );
    assert!(v.note.contains("0.18.0"), "names its own pinned version");
    assert!(v.note.contains("spec 088"), "names the missing capability");
    assert!(
        !v.may_accept_on_own_suite,
        "refusing without the report is available; classifying without it is not"
    );
}

// Row 5: no harness package exists.
#[test]
fn the_receipts_harness_revision_reads_not_recorded_and_is_never_omitted() {
    let suite = SuiteResult::new(vec![Check::ran("make gate", 0, None)], None);
    let r = mint(&judged(true, true), &suite, &context(), None).unwrap();

    assert_eq!(r.harness_revision, Recorded::Absent(Absence::NotRecorded));
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("harness_revision"), "present, not omitted");
    assert!(json.contains("not-recorded"));
}

// Row 6: HEAD moved during the suite.
#[test]
fn a_head_that_moved_during_the_suite_mints_no_receipt() {
    let suite = SuiteResult::new(vec![Check::ran("make gate", 0, None)], None);
    assert_eq!(
        mint(&judged(false, true), &suite, &context(), None),
        Err(NoReceipt::HeadMoved)
    );
}

// Row 7: the work tree was dirty at the end of the suite.
#[test]
fn a_dirty_work_tree_mints_no_receipt_and_names_the_dirty_paths() {
    let suite = SuiteResult::new(vec![Check::ran("make gate", 0, None)], None);
    match mint(&judged(true, false), &suite, &context(), None) {
        Err(NoReceipt::WorkTreeDirty { dirty_paths }) => {
            assert_eq!(dirty_paths, ["src/leftover.rs"]);
        }
        other => panic!("expected a named dirty tree, got {other:?}"),
    }
}

// Row 8: the candidate touches the authority set and its suite passes.
#[test]
fn an_authority_change_blocks_acceptance_even_when_the_candidates_suite_passes() {
    let delta = StaticDeltaReport {
        members: vec![],
        version: "spec-spine 0.99.0".into(),
    };
    let v = evaluate(&["Makefile".to_string()], &declared(), &delta);
    assert!(v.authority_change);
    assert!(!v.may_accept_on_own_suite);

    let passing = SuiteResult::new(vec![Check::ran("make gate", 0, None)], None);
    assert!(passing.passed(), "the suite does pass");
    match mint(&judged(true, true), &passing, &context(), Some(v.note)) {
        Err(NoReceipt::AuthorityChange { note }) => {
            assert!(note.contains("human decision"));
        }
        other => panic!("expected an authority change, got {other:?}"),
    }
}

#[test]
fn the_environment_manifest_is_an_authority_member_computed_here() {
    let v = evaluate(
        &[ENVIRONMENT_MANIFEST.to_string()],
        &declared(),
        &no_report(),
    );
    assert!(v.environment_manifest_touched);
    assert!(v.authority_change);
}

// Row 9: the policy digest cannot be computed at the base.
#[test]
fn a_policy_digest_that_cannot_be_computed_is_no_acceptance_with_the_reason_recorded() {
    let n = NoAcceptance::PolicyDigestUncomputable {
        detail: "the base revision is not readable".into(),
    };
    let a = Acceptance::None { reason: n };
    assert_eq!(a.word(), "no-acceptance");
    let json = serde_json::to_string(&a).unwrap();
    assert!(json.contains("policy-digest-uncomputable"));
    assert!(json.contains("not readable"), "the reason is recorded");
}

// Row 10: a tool exits zero but its structured report says failed.
#[test]
fn a_structured_report_saying_failed_beats_a_zero_exit_code() {
    let c = Check::ran("spec-spine verify 001", 0, Some(false));
    assert!(!c.passed(), "the report wins");
    assert_eq!(c.exit_code, Some(0), "and the exit code is still recorded");
}

// Row 11: a tool emits no structured report where one is expected.
#[test]
fn a_missing_expected_report_is_unknown_and_the_omission_is_named() {
    let c = Check::missing_report("spec-spine verify 001", 0);
    assert!(
        c.unknown(),
        "unknown for what the report would have carried"
    );
    assert!(!c.passed());
    let suite = SuiteResult::new(vec![c], None);
    assert_eq!(suite.unrun_count(), 1);
}

// Row 12: evidence with a self-anchored chain under a policy requiring trust.
#[test]
fn a_self_anchored_chain_may_pass_signature_but_never_establishes_issuer_trust() {
    let roots = RootSet::empty("deployment").trusting("release-key");
    let trust = issuer_trust(Signature::Pass, &Anchor::SelfCarried, &roots);
    assert_eq!(
        trust,
        IssuerTrust::Unknown,
        "whatever its internal consistency"
    );

    let d = Dimensions {
        integrity: Integrity::Pass,
        signature: Signature::Pass,
        issuer_trust: trust,
        subject_binding: SubjectBinding::NotApplicable,
    };
    match admit(&d, &AdmissionPolicy::strict()) {
        Admission::Refuse {
            reason: RefusalCode::IncompleteEvidence { missing },
        } => assert!(missing.contains(&"issuerTrust".to_string())),
        other => panic!("expected admission refused with a reason, got {other:?}"),
    }
}

// Row 13: the same intact unsigned evidence under two policies.
#[test]
fn the_same_intact_unsigned_evidence_is_admitted_by_one_policy_and_refused_by_another() {
    let d = Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::NotApplicable);

    assert_eq!(admit(&d, &AdmissionPolicy::permissive()), Admission::Admit);
    match admit(&d, &AdmissionPolicy::strict()) {
        Admission::Refuse {
            reason: RefusalCode::RequiredDimensionNotPassed { dimension, value },
        } => {
            assert_eq!(dimension, "signature");
            assert_eq!(value, "unsigned");
        }
        other => panic!("expected a named refusal, got {other:?}"),
    }
    // Neither result implies the evidence is trusted: the dimension is unchanged
    // under both, and only the admission differs.
    assert_eq!(d.signature, Signature::Unsigned);
    assert_eq!(d.issuer_trust, IssuerTrust::Unknown);
}

// Row 14: a byte-level mutation of preserved evidence.
#[test]
fn a_byte_level_mutation_fails_integrity_and_is_never_repaired() {
    let original = b"the preserved evidence";
    let reference = Reference::over_file_bytes("receipt", "1", original);

    assert_eq!(integrity(&reference, Some(original)), Integrity::Pass);
    assert_eq!(
        integrity(&reference, Some(b"the preserved evidencf")),
        Integrity::Fail
    );
    // The reference did not move to accommodate the mutation.
    assert_eq!(
        reference.digest,
        Reference::over_file_bytes("receipt", "1", original).digest
    );
}

// Row 15: a record written under an older hash construction.
#[test]
fn a_record_under_an_older_construction_stays_verifiable_under_that_construction() {
    let older = Reference {
        evidence_type: "record".into(),
        schema_version: "1".into(),
        digest: "whatever the old rule produced".into(),
        bytes: 3,
        construction: Construction::CanonicalRecordSha256 {
            canonicalization_version: "1".into(),
        },
        embedded: None,
    };

    // This build cannot check that construction, and says so rather than
    // rewriting the record to one it can.
    assert!(!older.checkable_here());
    assert_eq!(integrity(&older, Some(b"abc")), Integrity::Unknown);
    assert_eq!(older.construction.name(), "canonical-record-sha256/1");

    // A newer construction is a different reference, not a replacement.
    let newer = Construction::CanonicalRecordSha256 {
        canonicalization_version: "2".into(),
    };
    assert_ne!(older.construction.name(), newer.name());
}

// Row 16: a receipt for a head the branch has moved past.
#[test]
fn a_receipt_for_a_head_the_branch_moved_past_is_reported_as_stale() {
    let suite = SuiteResult::new(vec![Check::ran("make gate", 0, None)], None);
    let r = mint(&judged(true, true), &suite, &context(), None).unwrap();

    assert_eq!(freshness(&r, &r.candidate), None, "current");
    assert_eq!(freshness(&r, &"f".repeat(40)), Some(Absence::Stale));

    assert_eq!(
        freshness_field(Some(&r), Some(&"f".repeat(40))),
        Recorded::Absent(Absence::Stale),
        "a reported state, not an error and not a pass"
    );
}

// Row 17: a field a future contract will add, absent today.
#[test]
fn a_field_a_future_contract_will_add_reads_not_recorded_never_none_and_never_omitted() {
    let future: Recorded<String> = Recorded::Absent(Absence::NotRecorded);
    assert_eq!(serde_json::to_string(&future).unwrap(), "\"not-recorded\"");
    assert_ne!(future.absence(), Some(Absence::None));

    // And the one such field that exists today behaves that way.
    let suite = SuiteResult::new(vec![Check::ran("make gate", 0, None)], None);
    let r = mint(&judged(true, true), &suite, &context(), None).unwrap();
    assert_eq!(r.harness_revision.absence(), Some(Absence::NotRecorded));
}

#[test]
fn no_name_for_absence_can_be_read_as_a_pass() {
    for a in Absence::all() {
        assert!(!matches!(a.word(), "pass" | "ok" | "accepted" | ""));
    }
}
