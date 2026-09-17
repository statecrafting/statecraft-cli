//! Spec 003's observable negative cases that the shared crate answers.

use std::collections::BTreeMap;

use statecraft_envelope::admission::{AdmissionInput, Decision, evaluate};
use statecraft_envelope::attestation::{
    APPROVAL, ARTIFACT, AttestationId, PredicateRegistry, PredicateType, Principal, PrincipalKind,
    UnsignedAttestation,
};
use statecraft_envelope::cbor::{cid_of, encode};
use statecraft_envelope::dimensions::{
    Admission, AdmissionPolicy, Integrity, IssuerTrust, RefusalCode, Signature, SubjectBinding,
};
use statecraft_envelope::entry::{Entry, LinearScope, UnsignedEntry};
use statecraft_envelope::fact::{
    EraseReason, FactEnvelope, Resolved, Tombstone, TombstoneScope, TombstoneSet, resolve_payload,
};
use statecraft_envelope::hash::{Cid, Codec, Hash, KeyId};
use statecraft_envelope::hlc::Hlc;
use statecraft_envelope::reference::{GitOid, GitSubject, Reference};
use statecraft_envelope::roots::{AnchorOrigin, Root, RootSet, RootStatus};
use statecraft_envelope::sign::{PublicKey, Signer};
use statecraft_envelope::value::Value;
use statecraft_envelope::verdict::{
    EvidenceVerdict, SubjectQuestion, verify_artifact, verify_attestation,
};

fn signer() -> Signer {
    Signer::from_seed(&[9u8; 32])
}

fn pinned(signer: &Signer, status: RootStatus, since: Option<Hlc>) -> RootSet {
    RootSet {
        root_set_version: 1,
        roots: vec![Root {
            key_id: signer.public().id(),
            public_key: hex::encode(signer.public().0),
            scope: "platform".into(),
            not_before: None,
            not_after: None,
            status,
            since,
        }],
        signed_by: "offline-root".into(),
        origin: AnchorOrigin::Pinned,
    }
}

fn genesis(signer: &Signer) -> Entry {
    let f = FactEnvelope::new("repository.registered", 1, BTreeMap::new());
    UnsignedEntry::new(
        vec![],
        signer.public().id(),
        Hlc::new(1, 0),
        f.cid(),
        BTreeMap::new(),
    )
    .unwrap()
    .sign(signer)
    .unwrap()
}

fn approval(
    signer: &Signer,
    subject: Hash,
    extra_claim_key: Option<&str>,
) -> (statecraft_envelope::attestation::Attestation, Vec<u8>) {
    let mut claim = Value::map()
        .with("revision", Value::text(subject.to_hex()))
        .unwrap()
        .with(
            "reviewer",
            Principal::new(PrincipalKind::Human, "dana").to_value(),
        )
        .unwrap()
        .with("basis", Value::Array(vec![]))
        .unwrap();
    if let Some(k) = extra_claim_key {
        claim = claim.with(k, Value::text("request-changes")).unwrap();
    }
    let att = UnsignedAttestation {
        subject,
        predicate: PredicateType::new(APPROVAL).unwrap(),
        issuer: Principal::new(PrincipalKind::Service, "issuer"),
        claim: cid_of(&claim),
        at: Hlc::new(5, 0),
        extra: BTreeMap::new(),
    }
    .issue(signer)
    .unwrap();
    (att, encode(&claim))
}

#[test]
fn an_entry_not_naming_the_head_is_refused_with_the_head_named() {
    let s = signer();
    let mut scope = LinearScope::new(genesis(&s)).unwrap();
    let head = scope.head();
    let other = UnsignedEntry::new(
        vec![],
        s.public().id(),
        Hlc::new(2, 0),
        genesis(&s).body.payload,
        BTreeMap::new(),
    )
    .unwrap()
    .sign(&s)
    .unwrap();
    let e = scope.append(other).unwrap_err();
    assert!(e.to_string().contains(&head.0.to_hex()));
}

#[test]
fn a_duplicate_entry_is_a_no_op_and_a_stale_clock_is_refused() {
    let s = signer();
    let g = genesis(&s);
    let mut scope = LinearScope::new(g.clone()).unwrap();
    assert_eq!(scope.append(g.clone()).unwrap(), g.hash());
    let stale = UnsignedEntry::new(
        vec![g.hash()],
        s.public().id(),
        Hlc::new(1, 0),
        g.body.payload,
        BTreeMap::new(),
    )
    .unwrap()
    .sign(&s)
    .unwrap();
    assert!(scope.append(stale).is_err());
    let next = UnsignedEntry::new(
        vec![g.hash()],
        s.public().id(),
        Hlc::new(1, 1),
        g.body.payload,
        BTreeMap::new(),
    )
    .unwrap()
    .sign(&s)
    .unwrap();
    scope.append(next).unwrap();
    assert!(
        scope
            .verify(&|k: &KeyId| if *k == s.public().id() {
                Some(s.public())
            } else {
                None
            })
            .is_empty()
    );
}

#[test]
fn an_entry_decodes_from_its_bytes_and_a_mutated_byte_is_refused() {
    let s = signer();
    let g = genesis(&s);
    let bytes = g.canonical_bytes();
    assert_eq!(Entry::decode(&bytes).unwrap(), g);
    let mut bad = bytes.clone();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert!(
        Entry::decode(&bad)
            .map(|e| e.verify_signature(&s.public()))
            .unwrap_or(Err(statecraft_envelope::Error::Decode("x".into())))
            .is_err()
    );
}

#[test]
fn a_wrong_signer_cannot_produce_an_entry_for_another_issuer() {
    let s = signer();
    let other = Signer::from_seed(&[1u8; 32]);
    let f = FactEnvelope::new("x", 1, BTreeMap::new());
    let u = UnsignedEntry::new(
        vec![],
        s.public().id(),
        Hlc::new(1, 0),
        f.cid(),
        BTreeMap::new(),
    )
    .unwrap();
    assert!(u.sign(&other).is_err());
}

#[test]
fn unsigned_receipt_under_two_policies_admits_or_refuses_and_is_never_trusted() {
    let bytes = b"receipt bytes\n";
    let reference = Reference {
        subject: Some(GitSubject {
            repository: "r".into(),
            commit: GitOid {
                format: "sha1".into(),
                oid: "a".into(),
            },
            tree: None,
        }),
        ..Reference::over_file_bytes("statecraft-cli/receipt", "1", bytes)
    };
    let q = SubjectQuestion {
        subject: reference.subject.clone().unwrap(),
    };
    let v = verify_artifact(&reference, Some(bytes), Some(&q), &RootSet::empty());
    assert_eq!(v.integrity.status, Integrity::Pass);
    assert_eq!(v.signature.status, Signature::Unsigned);
    assert_eq!(v.issuer_trust.status, IssuerTrust::Unknown);
    assert_eq!(v.subject_binding.status, SubjectBinding::Pass);
    let submitter = Principal::new(PrincipalKind::Human, "sam");
    let verdicts = vec![(AttestationId(Hash::of(b"a")), v.clone())];
    let input = AdmissionInput {
        verdicts: &verdicts,
        decisions: &[],
        submitter: &submitter,
    };
    let allow = AdmissionPolicy {
        require_integrity: true,
        require_signature: true,
        require_subject_binding: true,
        ..AdmissionPolicy::default()
    };
    assert_eq!(evaluate(&allow, &input), Admission::Admit);
    let trusted = AdmissionPolicy {
        require_issuer_trust: true,
        ..allow.clone()
    };
    match evaluate(&trusted, &input) {
        Admission::Refuse { reasons, .. } => {
            assert!(reasons.contains(&RefusalCode::IssuerTrustRequired))
        }
        other => panic!("{other:?}"),
    }
    EvidenceVerdict::validate_shape(&serde_json::to_value(&v).unwrap()).unwrap();
}

#[test]
fn changed_bytes_fail_integrity_and_wrong_revision_fails_binding() {
    let bytes = b"receipt bytes\n";
    let reference = Reference {
        subject: Some(GitSubject {
            repository: "r".into(),
            commit: GitOid {
                format: "sha1".into(),
                oid: "a".into(),
            },
            tree: None,
        }),
        ..Reference::over_file_bytes("statecraft-cli/receipt", "1", bytes)
    };
    let v = verify_artifact(
        &reference,
        Some(b"receipt bytez\n"),
        None,
        &RootSet::empty(),
    );
    assert_eq!(v.integrity.status, Integrity::Fail);
    let q = SubjectQuestion {
        subject: GitSubject {
            repository: "r".into(),
            commit: GitOid {
                format: "sha1".into(),
                oid: "b".into(),
            },
            tree: None,
        },
    };
    let v = verify_artifact(&reference, Some(bytes), Some(&q), &RootSet::empty());
    assert_eq!(v.subject_binding.status, SubjectBinding::Fail);
    let q = SubjectQuestion {
        subject: GitSubject {
            repository: "r".into(),
            commit: GitOid {
                format: "sha256".into(),
                oid: "a".into(),
            },
            tree: None,
        },
    };
    let v = verify_artifact(&reference, Some(bytes), Some(&q), &RootSet::empty());
    assert_eq!(
        v.subject_binding.status,
        SubjectBinding::Fail,
        "same oid under another format fails"
    );
    let q = SubjectQuestion {
        subject: GitSubject {
            repository: "r".into(),
            commit: GitOid {
                format: "sha1".into(),
                oid: "a".into(),
            },
            tree: Some(GitOid {
                format: "sha1".into(),
                oid: "t".into(),
            }),
        },
    };
    let v = verify_artifact(&reference, Some(bytes), Some(&q), &RootSet::empty());
    assert_eq!(
        v.subject_binding.status,
        SubjectBinding::Unknown,
        "a tree asked of evidence that names none is unknown"
    );
    let none = Reference::over_file_bytes("statecraft-cli/receipt", "1", bytes);
    let v = verify_artifact(&none, Some(bytes), Some(&q), &RootSet::empty());
    assert_eq!(
        v.subject_binding.status,
        SubjectBinding::Unknown,
        "a missing expected subject is unknown, never not-applicable"
    );
}

#[test]
fn issuer_trust_missing_revoked_and_self_anchored_roots() {
    let s = signer();
    let subject = Hash::of(b"rev");
    let (att, claim) = approval(&s, subject, None);
    let reg = PredicateRegistry::first_slice();
    let no_keys = |_: &KeyId| None::<PublicKey>;
    let ledger_keys = |k: &KeyId| {
        if *k == s.public().id() {
            Some(s.public())
        } else {
            None
        }
    };

    let v = verify_attestation(
        &att,
        &Resolved::Present(claim.clone()),
        Some(&subject),
        &pinned(&s, RootStatus::Active, None),
        &no_keys,
        &reg,
    );
    assert_eq!(
        (
            v.integrity.status,
            v.signature.status,
            v.issuer_trust.status,
            v.subject_binding.status
        ),
        (
            Integrity::Pass,
            Signature::Pass,
            IssuerTrust::Pass,
            SubjectBinding::Pass
        )
    );

    let v = verify_attestation(
        &att,
        &Resolved::Present(claim.clone()),
        Some(&subject),
        &RootSet::empty(),
        &no_keys,
        &reg,
    );
    assert_eq!(
        v.signature.status,
        Signature::Unknown,
        "no key anywhere: not anchored"
    );
    assert_eq!(v.issuer_trust.status, IssuerTrust::Unknown);
    assert_eq!(
        v.integrity.status,
        Integrity::Unknown,
        "later dimensions are unknown after a stop"
    );
    assert_eq!(v.stop.as_deref(), Some("signature"));

    let v = verify_attestation(
        &att,
        &Resolved::Present(claim.clone()),
        Some(&subject),
        &RootSet::empty(),
        &ledger_keys,
        &reg,
    );
    assert_eq!(
        v.signature.status,
        Signature::Pass,
        "a key from the ledger checks the signature"
    );
    assert_eq!(v.issuer_trust.status, IssuerTrust::Unknown);
    assert_eq!(v.issuer_trust.reason.as_deref(), Some("self-anchored"));

    let v = verify_attestation(
        &att,
        &Resolved::Present(claim.clone()),
        Some(&subject),
        &pinned(&s, RootStatus::Revoked, Some(Hlc::new(4, 0))),
        &no_keys,
        &reg,
    );
    assert_eq!(v.issuer_trust.status, IssuerTrust::Fail);
    assert_eq!(v.issuer_trust.reason.as_deref(), Some("key-revoked"));

    let v = verify_attestation(
        &att,
        &Resolved::Present(claim.clone()),
        Some(&subject),
        &pinned(&s, RootStatus::Revoked, Some(Hlc::new(6, 0))),
        &no_keys,
        &reg,
    );
    assert_eq!(
        v.issuer_trust.status,
        IssuerTrust::Pass,
        "a signature before the compromise window still verifies"
    );

    let mut ledger = pinned(&s, RootStatus::Active, None);
    ledger.origin = AnchorOrigin::EvidenceLedger;
    let v = verify_attestation(
        &att,
        &Resolved::Present(claim.clone()),
        Some(&subject),
        &ledger,
        &no_keys,
        &reg,
    );
    assert_eq!(
        v.issuer_trust.status,
        IssuerTrust::Unknown,
        "anchors folded from the evidence ledger never establish trust"
    );

    let v = verify_attestation(
        &att,
        &Resolved::Present(claim),
        Some(&Hash::of(b"other")),
        &pinned(&s, RootStatus::Active, None),
        &no_keys,
        &reg,
    );
    assert_eq!(
        v.subject_binding.status,
        SubjectBinding::Fail,
        "a decision for another revision is never reused"
    );
}

#[test]
fn an_approval_carrying_a_verdict_key_is_not_an_approval_and_an_erased_claim_is_unknown() {
    let s = signer();
    let subject = Hash::of(b"rev");
    let reg = PredicateRegistry::first_slice();
    let roots = pinned(&s, RootStatus::Active, None);
    let (att, claim) = approval(&s, subject, Some("verdict"));
    let v = verify_attestation(
        &att,
        &Resolved::Present(claim),
        Some(&subject),
        &roots,
        &|_| None,
        &reg,
    );
    assert_eq!(v.integrity.status, Integrity::Fail);
    let (att, _) = approval(&s, subject, None);
    let t = Tombstone {
        target: att.body.claim,
        reason: EraseReason::Erasure,
        scope: TombstoneScope::Object,
    };
    let v = verify_attestation(
        &att,
        &Resolved::Erased(t),
        Some(&subject),
        &roots,
        &|_| None,
        &reg,
    );
    assert_eq!(v.integrity.status, Integrity::Unknown);
    assert_eq!(v.integrity.reason.as_deref(), Some("erased"));
    assert_eq!(
        v.signature.status,
        Signature::Pass,
        "the commitment is intact; only the claim is gone"
    );
}

#[test]
fn payload_resolution_has_three_answers_and_verifies_hashes() {
    let bytes = b"claim".to_vec();
    let cid = Cid::raw(&bytes);
    let mut ts = TombstoneSet::default();
    let lookup = |c: &Cid| if *c == cid { Some(bytes.clone()) } else { None };
    assert_eq!(
        resolve_payload(&cid, None, &ts, &lookup).unwrap(),
        Resolved::Present(bytes.clone())
    );
    assert_eq!(
        resolve_payload(&Cid::raw(b"other"), None, &ts, &lookup).unwrap(),
        Resolved::Missing
    );
    ts.add(Tombstone {
        target: cid,
        reason: EraseReason::Retention,
        scope: TombstoneScope::Object,
    });
    assert!(matches!(
        resolve_payload(&cid, None, &ts, &lookup).unwrap(),
        Resolved::Erased(_)
    ));
    let corrupt = |_: &Cid| Some(b"not the bytes".to_vec());
    assert!(
        resolve_payload(
            &Cid {
                codec: Codec::Raw,
                hash: Hash::of(b"x")
            },
            None,
            &TombstoneSet::default(),
            &corrupt
        )
        .is_err()
    );
}

#[test]
fn admission_counts_approvals_by_predicate_and_refuses_the_submitter() {
    let dana = Principal::new(PrincipalKind::Human, "dana");
    let sam = Principal::new(PrincipalKind::Human, "sam");
    let policy = AdmissionPolicy {
        min_approvals: 1,
        approver_may_not_be_submitter: true,
        approvers_distinct: true,
        ..AdmissionPolicy::default()
    };
    let input = AdmissionInput {
        verdicts: &[],
        decisions: &[],
        submitter: &sam,
    };
    assert!(matches!(
        evaluate(&policy, &input),
        Admission::Refuse {
            reason: RefusalCode::ApprovalsInsufficient { have: 0, need: 1 },
            ..
        }
    ));
    let by_sam = vec![Decision {
        id: AttestationId(Hash::of(b"1")),
        approves: true,
        reviewer: sam.clone(),
    }];
    let input = AdmissionInput {
        verdicts: &[],
        decisions: &by_sam,
        submitter: &sam,
    };
    match evaluate(&policy, &input) {
        Admission::Refuse { reasons, .. } => {
            assert!(reasons.contains(&RefusalCode::ApproverIsSubmitter));
            assert!(reasons.contains(&RefusalCode::ApprovalsInsufficient { have: 0, need: 1 }));
        }
        other => panic!("{other:?}"),
    }
    let by_dana = vec![
        Decision {
            id: AttestationId(Hash::of(b"2")),
            approves: true,
            reviewer: dana.clone(),
        },
        Decision {
            id: AttestationId(Hash::of(b"3")),
            approves: false,
            reviewer: dana.clone(),
        },
    ];
    let input = AdmissionInput {
        verdicts: &[],
        decisions: &by_dana,
        submitter: &sam,
    };
    assert_eq!(
        evaluate(&policy, &input),
        Admission::Admit,
        "a change request does not cancel an approval; policy counts predicates"
    );
    let required = AdmissionPolicy {
        require_artifacts: vec!["statecraft-cli/receipt".into()],
        ..AdmissionPolicy::default()
    };
    assert!(matches!(
        evaluate(&required, &input),
        Admission::Refuse {
            reason: RefusalCode::MissingRequiredArtifact { .. },
            ..
        }
    ));
}

#[test]
fn a_predicate_that_does_not_match_the_grammar_is_refused() {
    assert!(PredicateType::new("Statecraft/Artifact/v1").is_err());
    assert!(PredicateType::new("statecraft/artifact").is_err());
    assert!(PredicateType::new(ARTIFACT).is_ok());
}
