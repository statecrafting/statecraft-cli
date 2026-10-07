//! Spec 037: admission on action-gate's closed mode decides exactly what the
//! hand-written evaluator decided.
//!
//! [`admission_oracle`] is the pre-change evaluator, frozen. Every case below
//! is evaluated by both, and the two must agree on the `Admission` value, on
//! its serialized bytes, and on the `statecraft/policy-eval/v1` claim and its
//! canonical bytes. The cases are the named ones (permissive, empty, every
//! single refusal, multi-refusals), two exhaustive sweeps (every evidence
//! requirement against every single verdict; every approval requirement
//! against every decision sequence of up to three), and a seeded sweep of
//! mixed inputs, which then proves its own coverage: every refusal code
//! appears alone and inside a multi-refusal, and admissions occur.

mod admission_oracle;

use std::collections::BTreeMap;

use statecraft_envelope::admission::{AdmissionInput, Decision, evaluate, policy_eval_claim};
use statecraft_envelope::attestation::{AttestationId, Principal, PrincipalKind};
use statecraft_envelope::cbor::encode;
use statecraft_envelope::dimensions::{
    Admission, AdmissionPolicy, Integrity, IssuerTrust, RefusalCode, Signature, SubjectBinding,
};
use statecraft_envelope::hash::Hash;
use statecraft_envelope::reference::Reference;
use statecraft_envelope::roots::RootSet;
use statecraft_envelope::verdict::{Evidence, EvidenceVerdict, verify_artifact};

const INTEGRITY: [Integrity; 3] = [Integrity::Pass, Integrity::Fail, Integrity::Unknown];
const SIGNATURE: [Signature; 4] = [
    Signature::Pass,
    Signature::Fail,
    Signature::Unsigned,
    Signature::Unknown,
];
const ISSUER: [IssuerTrust; 3] = [IssuerTrust::Pass, IssuerTrust::Fail, IssuerTrust::Unknown];
const BINDING: [SubjectBinding; 4] = [
    SubjectBinding::Pass,
    SubjectBinding::Fail,
    SubjectBinding::Unknown,
    SubjectBinding::NotApplicable,
];
const TYPES: [&str; 3] = ["a", "b", "c"];

/// One verdict: an artifact of `evidence_type`, or a native attestation when
/// `None`, with the four statuses given.
fn verdict(
    evidence_type: Option<&str>,
    i: Integrity,
    s: Signature,
    t: IssuerTrust,
    b: SubjectBinding,
) -> EvidenceVerdict {
    thread_local! {
        static BASE: EvidenceVerdict = verify_artifact(
            &Reference::over_file_bytes("a", "1", b"bytes"),
            Some(b"bytes"),
            None,
            &RootSet::empty(),
        );
    }
    let mut v = BASE.with(Clone::clone);
    if let (Some(ty), Evidence::Artifact { reference }) = (evidence_type, &mut v.evidence) {
        reference.evidence_type = ty.into();
    }
    if evidence_type.is_none() {
        v.evidence = Evidence::Attestation {
            id: Hash::of(b"native"),
            predicate: "statecraft/approval/v1".into(),
        };
    }
    v.integrity.status = i;
    v.signature.status = s;
    v.issuer_trust.status = t;
    v.subject_binding.status = b;
    v
}

fn principal(name: &str) -> Principal {
    Principal::new(PrincipalKind::Human, name)
}

fn decision(n: usize, who: &str, approves: bool) -> Decision {
    Decision {
        id: AttestationId(Hash::of(format!("d{n}").as_bytes())),
        approves,
        reviewer: principal(who),
    }
}

fn kind(r: &RefusalCode) -> &'static str {
    match r {
        RefusalCode::RequiredDimensionNotPassed { .. } => "required-dimension-not-passed",
        RefusalCode::IncompleteEvidence { .. } => "incomplete-evidence",
        RefusalCode::SubjectMismatch => "subject-mismatch",
        RefusalCode::IssuerTrustRequired => "issuer-trust-required",
        RefusalCode::ApprovalsInsufficient { .. } => "approvals-insufficient",
        RefusalCode::ApproverIsSubmitter => "approver-is-submitter",
        RefusalCode::MissingRequiredArtifact { .. } => "missing-required-artifact",
        RefusalCode::DimensionUnknown { .. } => "dimension-unknown",
        RefusalCode::AmbiguousJson => "ambiguous-json",
        RefusalCode::NoPolicy => "no-policy",
        RefusalCode::Unknown(_) => "unknown",
    }
}

/// What the sweep saw, so it can prove its own coverage.
#[derive(Default)]
struct Seen {
    cases: usize,
    admits: usize,
    alone: BTreeMap<&'static str, usize>,
    among: BTreeMap<&'static str, usize>,
}

/// Evaluate with both and require the same value, bytes and claim.
fn agree(
    seen: &mut Seen,
    policy: &AdmissionPolicy,
    verdicts: &[(AttestationId, EvidenceVerdict)],
    decisions: &[Decision],
    submitter: &Principal,
) -> Admission {
    let input = AdmissionInput {
        verdicts,
        decisions,
        submitter,
    };
    let old = admission_oracle::evaluate(policy, &input);
    let new = evaluate(policy, &input);
    assert_eq!(new, old, "policy {policy:?}, input {input:?}");
    assert_eq!(
        serde_json::to_vec(&new).unwrap(),
        serde_json::to_vec(&old).unwrap(),
        "serialized admission"
    );
    let ids: Vec<AttestationId> = verdicts.iter().map(|(id, _)| *id).collect();
    // The digest is an input to the claim, the same for both sides; computing
    // it per case would only slow the sweep.
    let digest = Hash::of(b"policy");
    let (c_new, c_old) = (
        policy_eval_claim(digest, &ids, &new),
        policy_eval_claim(digest, &ids, &old),
    );
    assert_eq!(c_new, c_old, "policy-eval claim");
    assert_eq!(encode(&c_new), encode(&c_old), "policy-eval claim bytes");

    seen.cases += 1;
    let reasons = new.reasons();
    match reasons.len() {
        0 => seen.admits += 1,
        1 => *seen.alone.entry(kind(reasons[0])).or_default() += 1,
        _ => {
            for r in reasons {
                *seen.among.entry(kind(r)).or_default() += 1;
            }
        }
    }
    new
}

fn ids(n: usize) -> impl Iterator<Item = AttestationId> {
    (0..n).map(|i| AttestationId(Hash::of(format!("v{i}").as_bytes())))
}

#[test]
fn named_cases_agree() {
    let mut seen = Seen::default();
    let sam = principal("sam");
    let clean = verdict(
        Some("a"),
        Integrity::Pass,
        Signature::Pass,
        IssuerTrust::Pass,
        SubjectBinding::Pass,
    );
    let one = |v: EvidenceVerdict| vec![(ids(1).next().unwrap(), v)];
    let strict_all = AdmissionPolicy {
        require_artifacts: vec!["a".into(), "z".into()],
        require_subject_binding: true,
        min_approvals: 2,
        approvers_distinct: true,
        approver_may_not_be_submitter: true,
        ..AdmissionPolicy::strict()
    };

    // Permissive and empty admit.
    let permissive = AdmissionPolicy::permissive();
    assert_eq!(
        agree(&mut seen, &permissive, &[], &[], &sam),
        Admission::Admit
    );
    assert_eq!(
        agree(&mut seen, &permissive, &one(clean.clone()), &[], &sam),
        Admission::Admit
    );
    let bad = verdict(
        None,
        Integrity::Fail,
        Signature::Fail,
        IssuerTrust::Fail,
        SubjectBinding::Fail,
    );
    assert_eq!(
        agree(&mut seen, &permissive, &one(bad.clone()), &[], &sam),
        Admission::Admit,
        "a permissive policy admits whatever the dimensions say"
    );
    assert!(!agree(&mut seen, &strict_all, &[], &[], &sam).is_admit());

    // Every single refusal.
    let single = |seen: &mut Seen, policy: AdmissionPolicy, v: EvidenceVerdict, d: &[Decision]| {
        let a = agree(seen, &policy, &one(v), d, &sam);
        assert_eq!(a.reasons().len(), 1, "{a:?}");
        a.reasons()[0].clone()
    };
    let with = |i, s, t, b| verdict(Some("a"), i, s, t, b);
    let p = |f: fn(&mut AdmissionPolicy)| {
        let mut p = AdmissionPolicy::default();
        f(&mut p);
        p
    };
    use IssuerTrust as T;
    use Signature as S;
    use SubjectBinding as B;
    let cases: Vec<(AdmissionPolicy, EvidenceVerdict, Vec<Decision>, RefusalCode)> = vec![
        (
            p(|p| p.require_artifacts = vec!["z".into()]),
            clean.clone(),
            vec![],
            RefusalCode::MissingRequiredArtifact {
                evidence_type: "z".into(),
            },
        ),
        (
            p(|p| p.require_integrity = true),
            with(Integrity::Fail, S::Pass, T::Pass, B::Pass),
            vec![],
            RefusalCode::RequiredDimensionNotPassed {
                dimension: "integrity".into(),
                value: "fail".into(),
            },
        ),
        (
            p(|p| p.require_integrity = true),
            with(Integrity::Unknown, S::Pass, T::Pass, B::Pass),
            vec![],
            RefusalCode::DimensionUnknown {
                dimension: "integrity".into(),
            },
        ),
        (
            p(|p| p.require_signature = true),
            with(Integrity::Pass, S::Fail, T::Pass, B::Pass),
            vec![],
            RefusalCode::RequiredDimensionNotPassed {
                dimension: "signature".into(),
                value: "fail".into(),
            },
        ),
        (
            p(|p| p.require_signature = true),
            with(Integrity::Pass, S::Unknown, T::Pass, B::Pass),
            vec![],
            RefusalCode::DimensionUnknown {
                dimension: "signature".into(),
            },
        ),
        (
            p(|p| p.require_issuer_trust = true),
            with(Integrity::Pass, S::Unsigned, T::Pass, B::Pass),
            vec![],
            RefusalCode::RequiredDimensionNotPassed {
                dimension: "signature".into(),
                value: "unsigned".into(),
            },
        ),
        (
            p(|p| p.require_issuer_trust = true),
            with(Integrity::Pass, S::Unknown, T::Pass, B::Pass),
            vec![],
            RefusalCode::RequiredDimensionNotPassed {
                dimension: "signature".into(),
                value: "unknown".into(),
            },
        ),
        (
            p(|p| p.require_issuer_trust = true),
            with(Integrity::Pass, S::Pass, T::Fail, B::Pass),
            vec![],
            RefusalCode::RequiredDimensionNotPassed {
                dimension: "issuerTrust".into(),
                value: "fail".into(),
            },
        ),
        (
            p(|p| p.require_issuer_trust = true),
            with(Integrity::Pass, S::Pass, T::Unknown, B::Pass),
            vec![],
            RefusalCode::IssuerTrustRequired,
        ),
        (
            p(|p| p.require_subject_binding = true),
            with(Integrity::Pass, S::Pass, T::Pass, B::Fail),
            vec![],
            RefusalCode::SubjectMismatch,
        ),
        (
            p(|p| p.require_subject_binding = true),
            with(Integrity::Pass, S::Pass, T::Pass, B::Unknown),
            vec![],
            RefusalCode::DimensionUnknown {
                dimension: "subjectBinding".into(),
            },
        ),
        (
            p(|p| p.approver_may_not_be_submitter = true),
            clean.clone(),
            vec![decision(0, "sam", true)],
            RefusalCode::ApproverIsSubmitter,
        ),
        (
            p(|p| p.min_approvals = 2),
            clean.clone(),
            vec![decision(0, "dana", true), decision(1, "lee", false)],
            RefusalCode::ApprovalsInsufficient { have: 1, need: 2 },
        ),
        (
            p(|p| {
                p.min_approvals = 2;
                p.approvers_distinct = true
            }),
            clean.clone(),
            vec![decision(0, "dana", true), decision(1, "dana", true)],
            RefusalCode::ApprovalsInsufficient { have: 1, need: 2 },
        ),
    ];
    for (policy, v, d, want) in cases {
        assert_eq!(single(&mut seen, policy, v, &d), want);
    }

    // Multi-refusals, in the reported order.
    let a = agree(
        &mut seen,
        &strict_all,
        &[
            (ids(1).next().unwrap(), bad.clone()),
            (
                ids(2).nth(1).unwrap(),
                with(Integrity::Unknown, S::Unsigned, T::Unknown, B::Unknown),
            ),
        ],
        &[decision(0, "sam", true), decision(1, "dana", true)],
        &sam,
    );
    // One missing artifact, four per verdict, then the submitter and the count.
    assert_eq!(a.reasons().len(), 11, "{a:?}");
    let a = agree(
        &mut seen,
        &p(|p| {
            p.approver_may_not_be_submitter = true;
            p.min_approvals = 1
        }),
        &[],
        &[decision(0, "sam", true)],
        &sam,
    );
    assert_eq!(
        a.reasons(),
        vec![
            &RefusalCode::ApproverIsSubmitter,
            &RefusalCode::ApprovalsInsufficient { have: 0, need: 1 }
        ]
    );
}

/// Every evidence requirement (the four dimension flags and four artifact
/// sets) against every single verdict, artifact and native.
#[test]
fn every_evidence_policy_against_every_single_verdict_agrees() {
    let mut seen = Seen::default();
    let sam = principal("sam");
    let artifact_sets: [&[&str]; 4] = [&[], &["a"], &["a", "b"], &["b", "b"]];
    for flags in 0u32..16 {
        for required in artifact_sets {
            let policy = AdmissionPolicy {
                require_integrity: flags & 1 != 0,
                require_signature: flags & 2 != 0,
                require_issuer_trust: flags & 4 != 0,
                require_subject_binding: flags & 8 != 0,
                require_artifacts: required.iter().map(|s| s.to_string()).collect(),
                ..AdmissionPolicy::default()
            };
            for ty in [Some("a"), None] {
                for i in INTEGRITY {
                    for s in SIGNATURE {
                        for t in ISSUER {
                            for b in BINDING {
                                let v = vec![(ids(1).next().unwrap(), verdict(ty, i, s, t, b))];
                                agree(&mut seen, &policy, &v, &[], &sam);
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(seen.cases, 16 * 4 * 2 * 144);
    assert!(seen.admits > 0);
}

/// Every approval requirement (both flags, zero to three approvals) against
/// every sequence of up to three decisions by a population of three, approving
/// or not, submitted by one of them.
#[test]
fn every_approval_policy_against_every_decision_sequence_agrees() {
    let mut seen = Seen::default();
    let sam = principal("sam");
    let people = ["sam", "dana", "lee"];
    let mut sequences: Vec<Vec<Decision>> = vec![vec![]];
    for len in 1..=3u32 {
        for code in 0..6usize.pow(len) {
            let mut c = code;
            sequences.push(
                (0..len as usize)
                    .map(|n| {
                        let d = decision(n, people[c % 3], (c / 3) % 2 == 0);
                        c /= 6;
                        d
                    })
                    .collect(),
            );
        }
    }
    assert_eq!(sequences.len(), 1 + 6 + 36 + 216);
    for flags in 0u32..4 {
        for min_approvals in 0..4 {
            let policy = AdmissionPolicy {
                approvers_distinct: flags & 1 != 0,
                approver_may_not_be_submitter: flags & 2 != 0,
                min_approvals,
                ..AdmissionPolicy::default()
            };
            for d in &sequences {
                agree(&mut seen, &policy, &[], d, &sam);
            }
        }
    }
    assert!(seen.admits > 0);
}

/// A small deterministic generator (SplitMix64), so the sweep needs no
/// dependency and reproduces exactly.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn flip(&mut self) -> bool {
        self.next() & 1 == 1
    }
}

/// Mixed inputs: several verdicts of several types, several decisions by a
/// small population, and policies that combine every requirement.
#[test]
fn seeded_mixed_inputs_agree() {
    let mut seen = Seen::default();
    let mut rng = Rng(0x5743_0037);
    let people = ["sam", "dana", "lee"];
    for _ in 0..20_000 {
        let policy = AdmissionPolicy {
            require_integrity: rng.flip(),
            require_signature: rng.flip(),
            require_issuer_trust: rng.flip(),
            require_subject_binding: rng.flip(),
            approvers_distinct: rng.flip(),
            approver_may_not_be_submitter: rng.flip(),
            min_approvals: rng.below(4) as u32,
            require_artifacts: (0..rng.below(4))
                .map(|_| TYPES[rng.below(3)].to_string())
                .collect(),
            policy_version: rng.flip().then_some(1),
        };
        let verdicts: Vec<(AttestationId, EvidenceVerdict)> = ids(rng.below(5))
            .map(|id| {
                let ty = (rng.below(4) != 0).then(|| TYPES[rng.below(3)]);
                let v = verdict(
                    ty,
                    INTEGRITY[rng.below(3)],
                    SIGNATURE[rng.below(4)],
                    ISSUER[rng.below(3)],
                    BINDING[rng.below(4)],
                );
                (id, v)
            })
            .collect();
        let decisions: Vec<Decision> = (0..rng.below(6))
            .map(|n| decision(n, people[rng.below(3)], rng.below(4) != 0))
            .collect();
        let submitter = principal(people[rng.below(3)]);
        agree(&mut seen, &policy, &verdicts, &decisions, &submitter);
    }
    assert_covered(&seen);
}

/// The seven codes the evaluator can produce, each seen alone and in company,
/// and admissions seen too.
fn assert_covered(seen: &Seen) {
    assert!(seen.admits > 0, "no admission was generated");
    for code in [
        "missing-required-artifact",
        "required-dimension-not-passed",
        "dimension-unknown",
        "issuer-trust-required",
        "subject-mismatch",
        "approver-is-submitter",
        "approvals-insufficient",
    ] {
        assert!(seen.alone.contains_key(code), "{code} never refused alone");
        assert!(
            seen.among.contains_key(code),
            "{code} never in a multi-refusal"
        );
    }
}
