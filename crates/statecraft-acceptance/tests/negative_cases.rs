//! Spec 005 section 3.10, the observable negative cases, one test per row.
//!
//! Named after the row each covers, so a row that stops being covered shows up
//! as a deleted test rather than as a quietly weakened assertion.

use statecraft_acceptance::absence::{Absence, Recorded, Statement};
use statecraft_acceptance::authority::{
    CorpusVerdict, Declared, DeltaReport, ENVIRONMENT_MANIFEST, NoDeltaReport, StaticDeltaReport,
    evaluate,
};
use statecraft_acceptance::delta::SpecSpineDeltaReport;
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
        spec_spine_version: "spec-spine 0.20.0".into(),
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

// Row 4: this product does not read spec-spine's delta report.
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
    assert!(v.note.contains("0.20.0"), "names the installed version");
    assert!(
        v.note.contains("spec 088"),
        "names the report it does not read"
    );
    assert!(
        !v.may_accept_on_own_suite,
        "refusing without the report is available; classifying without it is not"
    );
}

// Row 4, the half that was measurably unprotected: the recorded reason
// attributes the absence to THIS product, never to spec-spine. The earlier
// wording said the installed spec-spine carried no such report, which the
// `=0.20.0` pin falsified, and no test held it, so a false sentence sat in the
// record until someone read it. Spec 005 section 3.3 now requires the
// attribution, and this is what holds it.
#[test]
fn the_not_recorded_reason_blames_this_product_and_not_the_installed_spec_spine() {
    let v = evaluate(
        &["crates/x/src/lib.rs".to_string()],
        &declared(),
        &no_report(),
    );

    assert!(
        v.note.contains("this product does not read"),
        "the absence is this product's gap: {}",
        v.note
    );
    for false_claim in [
        "is in no release",
        "carries no change-classification report",
        "no release carries",
    ] {
        assert!(
            !v.note.contains(false_claim),
            "the reason must not claim anything about what a release carries, \
             because the pin can move under it: found {false_claim:?} in {}",
            v.note
        );
    }
}

// Row 4b: the report is read, and it is read from bytes spec-spine actually
// wrote. The fixture is the `--json` envelope the pinned 0.20.0 binary emitted
// for the two correction commits on `main`, captured verbatim. A reader tested
// only against JSON this repository invented would pass while disagreeing with
// the tool it claims to read.
const REAL_REPORT: &[u8] = include_bytes!("../testdata/delta/spec-spine-0.20.0-corrections.json");

fn real_report() -> SpecSpineDeltaReport {
    SpecSpineDeltaReport::from_envelope_json(REAL_REPORT).expect("the pinned binary's own output")
}

fn paths_of(r: &SpecSpineDeltaReport) -> Vec<String> {
    r.report().changes.iter().map(|c| c.path.clone()).collect()
}

fn report_from(json: &str) -> SpecSpineDeltaReport {
    SpecSpineDeltaReport::from_envelope_json(json.as_bytes()).expect("a well formed envelope")
}

fn envelope(changes: &str, counts: &str, required: bool) -> String {
    format!(
        r#"{{"verb":"delta","exitCode":0,"ok":true,"schemaVersion":"0.4.0","report":{{
           "schemaVersion":"0.1.0","tool":{{"name":"spec-spine","version":"0.20.0"}},
           "classifiedUnder":"base","base":"{b}","mergeBase":"{b}","head":"{h}",
           "changes":[{changes}],"counts":{{{counts}}},
           "priorPolicy":{{"required":{required},"classes":[]}}}}}}"#,
        b = "b".repeat(40),
        h = "h".repeat(40),
    )
}

#[test]
fn the_delta_report_reads_the_bytes_the_pinned_binary_writes() {
    let r = real_report();

    assert_eq!(r.version(), "spec-spine 0.20.0");
    assert_eq!(r.report().classified_under, "base");
    assert!(r.classes_used().contains(&"policy".to_string()));
    assert!(r.classes_used().contains(&"requirement".to_string()));
    assert!(
        r.report().prior_policy.required,
        "spec-spine's own summary, recorded verbatim"
    );
}

// Row 4c: the report answers, and a `policy` class on a **hashed input** is
// detail rather than a membership answer. The real report's one `policy` path
// is `docs/decisions/00-founding-decisions.md`, which is classed that way
// because the base hashes it, not because `001` section 3.5 makes it a member.
// Reading the class as membership would let the authority set be widened by
// editing a configuration list, and would put `README.md` in it.
#[test]
fn a_hashed_input_classed_policy_in_a_real_report_is_not_a_corpus_member() {
    let r = real_report();
    let v = evaluate(&paths_of(&r), &declared(), &r);

    assert!(
        r.report()
            .changes
            .iter()
            .any(|c| c.path.starts_with("docs/") && c.classes.contains(&"policy".to_string())),
        "the fixture is the one with a hashed input in it"
    );
    assert_eq!(v.corpus_members, CorpusVerdict::Members(vec![]));
    assert!(
        v.corpus_classes.contains(&"policy".to_string()),
        "the class is kept as detail for the reviewer"
    );
    assert_eq!(v.prior_policy_required, Recorded::Present(true));
}

// Row 4c, the other half: the base's own configuration **is** the corpus-side
// policy member, and a candidate that edits it is an authority change.
#[test]
fn a_change_to_the_bases_own_configuration_is_an_authority_change() {
    let r = report_from(&envelope(
        r#"{"path":"spec-spine.toml","change":"modified","classes":["policy"]}"#,
        r#""policy":1"#,
        true,
    ));
    let v = evaluate(&paths_of(&r), &declared(), &r);

    assert_eq!(
        v.corpus_members,
        CorpusVerdict::Members(vec!["policy".into()])
    );
    assert!(v.authority_change);
    assert!(!v.may_accept_on_own_suite);
    assert!(v.note.contains("policy"));
}

// Row 4d: **the conclusion the integration newly permits.** With a report in
// hand that names no corpus-side member, and no repository artifact and no
// environment manifest in the diff, acceptance may rest on the candidate's own
// suite. Before the report was read this was unreachable: every candidate was
// refused for want of an answer.
#[test]
fn a_report_naming_no_corpus_member_lets_acceptance_rest_on_the_candidates_own_suite() {
    let r = report_from(&envelope(
        r#"{"path":"crates/x/src/lib.rs","change":"modified","classes":["implementation"]},
           {"path":"specs/00x-y/spec.md","change":"modified","classes":["requirement"]}"#,
        r#""implementation":1,"requirement":1"#,
        true,
    ));
    let v = evaluate(&paths_of(&r), &declared(), &r);

    assert_eq!(v.corpus_members, CorpusVerdict::Members(vec![]));
    assert!(!v.authority_change);
    assert!(
        v.may_accept_on_own_suite,
        "with an answer in hand and no member touched, the suite settles it"
    );
    assert!(
        v.prior_policy_required == Recorded::Present(true),
        "spec-spine's `required` is recorded even where this product concludes no member \
         was touched: the two answer different questions (spec 088 section 3.5)"
    );
}

// Row 4e: a spec's `verify:cli` plan is an acceptance instruction, so changing
// it is an authority change even though the same file's body edit is not.
#[test]
fn a_changed_verification_plan_is_an_authority_change() {
    let r = report_from(&envelope(
        r#"{"path":"specs/00x-y/spec.md","change":"modified","classes":["requirement","verification"]}"#,
        r#""requirement":1,"verification":1"#,
        true,
    ));
    let v = evaluate(&paths_of(&r), &declared(), &r);

    assert_eq!(
        v.corpus_members,
        CorpusVerdict::Members(vec!["acceptance-instructions".into()])
    );
    assert!(v.authority_change);
    assert!(!v.may_accept_on_own_suite);
}

// Row 4f: a class token this build does not know could be a member. The answer
// is an absence naming the token, never a member list computed as though the
// token were not there.
#[test]
fn a_class_this_build_cannot_place_makes_the_verdict_not_recorded() {
    for class in ["unknown", "quorum"] {
        let r = report_from(&envelope(
            &format!(r#"{{"path":"odd.md","change":"modified","classes":["{class}"]}}"#),
            &format!(r#""{class}":1"#),
            true,
        ));
        let v = evaluate(&paths_of(&r), &declared(), &r);

        assert_eq!(
            v.corpus_members,
            CorpusVerdict::Absent(Absence::NotRecorded),
            "for class {class}"
        );
        assert!(!v.may_accept_on_own_suite, "for class {class}");
        assert!(v.note.contains(class), "the reason names it: {}", v.note);
    }
}

// Row 4g: a report about some other change is not an answer about this one.
#[test]
fn a_report_that_does_not_cover_the_candidates_paths_is_not_read_as_an_answer() {
    let r = report_from(&envelope(
        r#"{"path":"a.rs","change":"modified","classes":["implementation"]}"#,
        r#""implementation":1"#,
        false,
    ));
    let v = evaluate(
        &["a.rs".to_string(), "crates/x/src/b.rs".to_string()],
        &declared(),
        &r,
    );

    assert_eq!(
        v.corpus_members,
        CorpusVerdict::Absent(Absence::NotRecorded)
    );
    assert!(!v.may_accept_on_own_suite);
    assert!(v.note.contains("crates/x/src/b.rs"));
}

// Row 4, the half that stays protected after the integration: every reason the
// product can record names what it asked for and what came back, and none of
// them claims anything about what a spec-spine release carries. The pin can
// move under a record that made such a claim, which is how the last one became
// false.
#[test]
fn no_recorded_absence_reason_claims_anything_about_what_a_release_carries() {
    let unplaceable = report_from(&envelope(
        r#"{"path":"odd.md","change":"modified","classes":["quorum"]}"#,
        r#""quorum":1"#,
        true,
    ));
    let uncovered = report_from(&envelope(
        r#"{"path":"a.rs","change":"modified","classes":["implementation"]}"#,
        r#""implementation":1"#,
        false,
    ));

    let notes = [
        evaluate(&["x.rs".to_string()], &declared(), &no_report()).note,
        evaluate(&paths_of(&unplaceable), &declared(), &unplaceable).note,
        evaluate(
            &["a.rs".to_string(), "b.rs".to_string()],
            &declared(),
            &uncovered,
        )
        .note,
    ];

    for note in notes {
        assert!(note.contains("0.20.0"), "names the version: {note}");
        for false_claim in [
            "is in no release",
            "carries no change-classification report",
            "no release carries",
        ] {
            assert!(
                !note.contains(false_claim),
                "found {false_claim:?} in {note}"
            );
        }
    }
}

// Row 4h: an envelope the reader will not accept is refused rather than read
// partially. Each case names what came back.
#[test]
fn an_envelope_this_build_does_not_read_is_refused_and_says_why() {
    let wrong_schema =
        envelope("", "", false).replace(r#""schemaVersion":"0.1.0""#, r#""schemaVersion":"0.2.0""#);
    let wrong_side = envelope("", "", false)
        .replace(r#""classifiedUnder":"base""#, r#""classifiedUnder":"head""#);
    let wrong_verb = envelope("", "", false).replace(r#""verb":"delta""#, r#""verb":"couple""#);

    for (bytes, expected) in [
        (wrong_schema, "0.2.0"),
        (wrong_side, "head"),
        (wrong_verb, "couple"),
    ] {
        let err = SpecSpineDeltaReport::from_envelope_json(bytes.as_bytes())
            .expect_err("this build does not read it");
        assert!(
            err.to_string().contains(expected),
            "the refusal names what came back: {err}"
        );
    }
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
            ..
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
            ..
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
        digest: "whatever the old rule produced".into(),
        bytes: 3,
        construction: Construction::CanonicalRecordSha256 {
            canonicalization_version: "1".into(),
        },
        ..Reference::over_file_bytes("record", "1", b"abc")
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
