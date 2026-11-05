//! Spec 054: the AI pre-screen is an advisory attestation shown beside
//! approvals and never counted as one.
//!
//! I-1: no pre-screen is counted as an approval or a change request, and no
//! verdict admits; at most it refuses, under the opt-in. R-1: the verdict is
//! the structured word or nothing. R-2: the opt-in refuses a `findings`
//! pre-screen and equally a missing, unreadable or other-revision one, and a
//! policy without it evaluates, serializes and digests exactly as before.
//! V-1's differential half is `admission_differential.rs`.

use statecraft_envelope::admission::{
    AdmissionInput, Decision, Prescreen, PrescreenVerdict, ai_prescreen_claim, evaluate,
    policy_eval_claim,
};
use statecraft_envelope::attestation::{
    AI_PRESCREEN, AttestationId, ClaimVerdict, PredicateRegistry, PredicateType, Principal,
    PrincipalKind,
};
use statecraft_envelope::cbor::encode;
use statecraft_envelope::dimensions::{Admission, AdmissionPolicy, RefusalCode};
use statecraft_envelope::hash::Hash;
use statecraft_envelope::value::Value;

fn sam() -> Principal {
    Principal::new(PrincipalKind::Human, "sam")
}

fn revision() -> Hash {
    Hash::of(b"the revision")
}

fn id() -> AttestationId {
    AttestationId(Hash::of(b"the pre-screen"))
}

fn claim(verdict: PrescreenVerdict, findings: u64) -> Value {
    ai_prescreen_claim(
        "claude-test-model",
        "sha256:prompt",
        "sha256:guidelines",
        verdict,
        findings,
    )
}

fn read(verdict: PrescreenVerdict, findings: u64) -> Prescreen {
    Prescreen::read(revision(), id(), revision(), &claim(verdict, findings))
}

fn no_findings() -> Prescreen {
    read(PrescreenVerdict::NoFindings, 0)
}

fn findings() -> Prescreen {
    read(PrescreenVerdict::Findings, 2)
}

fn opting() -> AdmissionPolicy {
    AdmissionPolicy {
        refuse_on_prescreen_findings: true,
        ..AdmissionPolicy::permissive()
    }
}

fn admit(policy: &AdmissionPolicy, decisions: &[Decision], prescreen: Prescreen) -> Admission {
    evaluate(
        policy,
        &AdmissionInput {
            verdicts: &[],
            decisions,
            submitter: &sam(),
            prescreen,
        },
    )
}

fn refused(prescreen: &str) -> RefusalCode {
    RefusalCode::PrescreenRefused {
        prescreen: prescreen.into(),
    }
}

/// Section 3.1: the predicate is registered with the five fields and refuses
/// a claim that carries a confidence or poses as a decision (3.4).
#[test]
fn the_predicate_is_registered_and_its_claim_carries_no_confidence_or_decision() {
    let registry = PredicateRegistry::first_slice();
    let predicate = PredicateType::new(AI_PRESCREEN).unwrap();
    assert_eq!(AI_PRESCREEN, "statecraft/ai-prescreen/v1");
    assert!(registry.knows(&predicate));
    let good = claim(PrescreenVerdict::NoFindings, 0);
    assert_eq!(
        registry.verify_claim(&predicate, &good),
        ClaimVerdict::Valid
    );
    let m = good.as_map().unwrap();
    assert_eq!(
        m.keys().collect::<Vec<_>>(),
        ["findings", "guidelines", "model", "prompt", "verdict"]
    );
    for key in ["model", "prompt", "guidelines", "verdict", "findings"] {
        let mut missing = m.clone();
        missing.remove(key);
        assert!(
            matches!(
                registry.verify_claim(&predicate, &Value::Map(missing)),
                ClaimVerdict::Invalid(_)
            ),
            "{key} is required"
        );
    }
    for (key, value) in [
        ("confidence", Value::text("0.97")),
        ("decision", Value::text("admit")),
        ("approves", Value::Bool(true)),
    ] {
        let with = good.clone().with(key, value).unwrap();
        assert!(
            matches!(
                registry.verify_claim(&predicate, &with),
                ClaimVerdict::Invalid(_)
            ),
            "{key} is forbidden"
        );
    }
}

/// R-1 and 3.4: the verdict is the structured field's exact word. A text
/// that contains a verdict word, a count that disagrees with the word, a
/// missing field or a non-map claim is unreadable, never a verdict.
#[test]
fn the_verdict_is_the_structured_word_or_the_record_is_unreadable() {
    let Prescreen::Read {
        model,
        verdict,
        findings,
        ..
    } = findings()
    else {
        panic!("a well-formed claim reads");
    };
    assert_eq!(model, "claude-test-model");
    assert_eq!(verdict, PrescreenVerdict::Findings);
    assert_eq!(findings, 2);
    assert_eq!(no_findings().state(), "no-findings");
    assert_eq!(self::findings().state(), "findings");

    let good = claim(PrescreenVerdict::NoFindings, 0);
    let with = |key: &str, value: Value| {
        let mut m = good.as_map().unwrap().clone();
        m.insert(key.into(), value);
        Prescreen::read(revision(), id(), revision(), &Value::Map(m))
    };
    let unreadable = |p: Prescreen, why: &str| match p {
        Prescreen::Unreadable { reason, .. } => assert!(reason.contains(why), "{reason}"),
        other => panic!("{other:?} should be unreadable"),
    };
    for word in [
        "APPROVE",
        "no-findings APPROVE",
        "Findings",
        "no_findings",
        "findings ",
    ] {
        unreadable(with("verdict", Value::text(word)), "neither findings");
    }
    unreadable(with("verdict", Value::text("")), "non-empty text");
    unreadable(with("findings", Value::int(3).unwrap()), "counts findings");
    unreadable(
        with("verdict", Value::text("findings")),
        "findings verdict with no findings",
    );
    unreadable(with("findings", Value::text("2")), "not a count");
    unreadable(with("findings", Value::int(-1).unwrap()), "not a count");
    unreadable(with("model", Value::text("")), "non-empty text");
    unreadable(
        Prescreen::read(revision(), id(), revision(), &Value::text("ok")),
        "not a map",
    );
    let mut without = good.as_map().unwrap().clone();
    without.remove("guidelines");
    unreadable(
        Prescreen::read(revision(), id(), revision(), &Value::Map(without)),
        "no \"guidelines\"",
    );
    // The subject decides before the claim is read.
    let other = Hash::of(b"another revision");
    assert_eq!(
        Prescreen::read(revision(), id(), other, &good),
        Prescreen::OtherRevision {
            id: id(),
            subject: other
        }
    );
    assert_eq!(Prescreen::Missing.state(), "missing");
    assert_eq!(Prescreen::Missing.id(), None);
}

/// I-1 and V-1: a `no-findings` pre-screen with no approval refuses for the
/// missing approval under every policy, opting or not, and a pre-screen
/// never satisfies an approval requirement.
#[test]
fn a_no_findings_prescreen_never_stands_in_for_an_approval() {
    let one_approval = AdmissionPolicy {
        min_approvals: 1,
        ..AdmissionPolicy::permissive()
    };
    let opting_one = AdmissionPolicy {
        min_approvals: 1,
        ..opting()
    };
    for policy in [&one_approval, &opting_one] {
        let a = admit(policy, &[], no_findings());
        assert_eq!(
            a.reasons(),
            vec![&RefusalCode::ApprovalsInsufficient { have: 0, need: 1 }],
            "{policy:?}"
        );
    }
    // A change request is not cancelled by a pre-screen either: the count is
    // of decisions, and the pre-screen is not one.
    let request = [Decision {
        id: AttestationId(Hash::of(b"cr")),
        approves: false,
        reviewer: Principal::new(PrincipalKind::Human, "dana"),
    }];
    assert_eq!(
        admit(&one_approval, &request, no_findings()).reasons(),
        vec![&RefusalCode::ApprovalsInsufficient { have: 0, need: 1 }]
    );
    // With the approval, a no-findings pre-screen is satisfied under the
    // opt-in and admits nothing by itself: the approval did.
    let approval = [Decision {
        id: AttestationId(Hash::of(b"ok")),
        approves: true,
        reviewer: Principal::new(PrincipalKind::Human, "dana"),
    }];
    assert_eq!(
        admit(&opting_one, &approval, no_findings()),
        Admission::Admit
    );
}

/// R-2 and V-1: under the opt-in, `findings` refuses with the closed reason,
/// and so do a missing, an unreadable and an other-revision pre-screen; each
/// is the only reason, and the reason names the state.
#[test]
fn the_opt_in_refuses_findings_and_equally_every_absent_form() {
    let cases = [
        (findings(), "findings"),
        (Prescreen::Missing, "missing"),
        (
            Prescreen::Unreadable {
                id: id(),
                reason: "the claim is not a map".into(),
            },
            "unreadable",
        ),
        (
            Prescreen::OtherRevision {
                id: id(),
                subject: Hash::of(b"another revision"),
            },
            "other-revision",
        ),
    ];
    for (prescreen, state) in cases {
        let a = admit(&opting(), &[], prescreen.clone());
        assert_eq!(a.reasons(), vec![&refused(state)], "{prescreen:?}");
        assert!(!a.is_admit());
    }
    assert_eq!(admit(&opting(), &[], no_findings()), Admission::Admit);
    // The reason is reported last, after the approval checks.
    let strict = AdmissionPolicy {
        min_approvals: 1,
        ..opting()
    };
    assert_eq!(
        admit(&strict, &[], findings()).reasons(),
        vec![
            &RefusalCode::ApprovalsInsufficient { have: 0, need: 1 },
            &refused("findings")
        ]
    );
}

/// R-2 and V-1: a policy without the option registers no check, so every
/// pre-screen state gives the same admission, the same serialized bytes and
/// the same policy-eval claim bytes as no pre-screen at all.
#[test]
fn a_default_policy_changes_nothing_whatever_the_prescreen_says() {
    let policies = [
        AdmissionPolicy::permissive(),
        AdmissionPolicy::strict(),
        AdmissionPolicy {
            min_approvals: 1,
            approver_may_not_be_submitter: true,
            ..AdmissionPolicy::permissive()
        },
    ];
    let states = [
        findings(),
        no_findings(),
        Prescreen::Unreadable {
            id: id(),
            reason: "x".into(),
        },
        Prescreen::OtherRevision {
            id: id(),
            subject: Hash::of(b"y"),
        },
    ];
    for policy in &policies {
        let baseline = admit(policy, &[], Prescreen::Missing);
        let claim = policy_eval_claim(policy.digest(), &[], &baseline);
        for state in &states {
            let a = admit(policy, &[], state.clone());
            assert_eq!(a, baseline, "{policy:?} {state:?}");
            assert_eq!(
                serde_json::to_vec(&a).unwrap(),
                serde_json::to_vec(&baseline).unwrap()
            );
            assert_eq!(
                encode(&policy_eval_claim(policy.digest(), &[], &a)),
                encode(&claim)
            );
        }
    }
}

/// Section 3.3: the option is a defaulted field skipped when unset, so a
/// policy without it serializes and digests exactly as before it existed,
/// and one with it reads back and is a different policy.
#[test]
fn the_option_is_absent_from_a_policy_that_does_not_set_it() {
    let before = AdmissionPolicy::permissive();
    let json = serde_json::to_string(&before).unwrap();
    assert!(!json.contains("prescreen"), "{json}");
    let read: AdmissionPolicy = serde_json::from_str(
        r#"{"require_integrity":false,"require_signature":false,"require_issuer_trust":false}"#,
    )
    .unwrap();
    assert_eq!(read, before);
    assert_eq!(read.digest(), before.digest());
    let with: AdmissionPolicy = serde_json::from_str(
        r#"{"require_integrity":false,"require_signature":false,"require_issuer_trust":false,"refuse_on_prescreen_findings":true}"#,
    )
    .unwrap();
    assert_eq!(with, opting());
    assert_ne!(with.digest(), before.digest());
    assert!(
        serde_json::to_string(&with)
            .unwrap()
            .contains(r#""refuse_on_prescreen_findings":true"#)
    );
}

/// The new closed reason round-trips through its wire form and a reader that
/// predates it keeps it verbatim as unknown rather than dropping it.
#[test]
fn the_refusal_code_serializes_as_one_tagged_object() {
    let code = refused("missing");
    let json = serde_json::to_value(&code).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"prescreen-refused": {"prescreen": "missing"}})
    );
    let back: RefusalCode = serde_json::from_value(json).unwrap();
    assert_eq!(back, code);
    let malformed: RefusalCode =
        serde_json::from_value(serde_json::json!({"prescreen-refused": {}})).unwrap();
    assert!(matches!(malformed, RefusalCode::Unknown(_)));
    let a = Admission::refuse(vec![refused("findings")]);
    let text = serde_json::to_string(&a).unwrap();
    assert!(text.contains(r#""reason":{"prescreen-refused":{"prescreen":"findings"}}"#));
    let back: Admission = serde_json::from_str(&text).unwrap();
    assert_eq!(back, a);
}

/// The claim builder writes the five fields of section 3.1 as portable
/// values, and the record carries no other key.
#[test]
fn the_claim_builder_writes_exactly_the_record() {
    let c = claim(PrescreenVerdict::Findings, 3);
    assert_eq!(
        c.to_json(),
        serde_json::json!({
            "model": "claude-test-model",
            "prompt": "sha256:prompt",
            "guidelines": "sha256:guidelines",
            "verdict": "findings",
            "findings": 3,
        })
    );
    assert_eq!(
        claim(PrescreenVerdict::NoFindings, 0).to_json()["verdict"],
        "no-findings"
    );
}
