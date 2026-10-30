//! Spec 025's product-invariance suite, acceptance half: every corpus case on
//! the `citation-target` and `wire-metadata` surfaces, and the
//! `context-packet` and result-document `provider-output` cases, is seeded
//! with its payload and with its benign twin, and evidence admission,
//! citation resolution, review and acceptance are the twin's except where
//! the case declares (sections 2, 3.1 and 3.4).
//!
//! The corpus and the comparison are the adapter crate's
//! (`tests/support/injection.rs`), included by path. The producer is honest
//! and answers from the bytes it is given; no provider, network, credential
//! or spend is involved.

#[path = "../../statecraft-adapter/tests/support/injection.rs"]
mod injection;

use std::collections::BTreeMap;

use serde_json::{Value, json};

use statecraft_acceptance::context::{
    self as ctx, Citation, ClosureAnswer, CurrentAnswer, EvidenceResult, MemberAnswer, Producer,
    Validated,
};
use statecraft_acceptance::documentation_acceptance::{
    self as da, Grant, ManifestFreshness, Policy,
};
use statecraft_acceptance::fixed_export as fx;
use statecraft_acceptance::wire::{Candidate, admit, eligibility};
use statecraft_envelope::hash::sha256_hex;
use statecraft_envelope::wire::fixture as wire_fixture;
use statecraft_envelope::wire::policy::WirePolicy;
use statecraft_envelope::wire::record::{Custody, Handoff};
use statecraft_envelope::wire::screen::{DEFAULT_DETECTOR, DefaultDetector};
use statecraft_envelope::wire::{Stated, Subject};
use statecraft_run::context::{
    ContextPolicy, Manifest, Need, Observation, Reported, custody_digest, fold_delivery,
};
use statecraft_run::documentation::delivery::Delivery as DocDelivery;
use statecraft_run::documentation::review::{
    ArtifactDisposition, ArtifactIdentity, ArtifactReview, Authority, ClaimDisposition,
    ClaimReview, CoreTuple, Read, Recommendation, Review, Reviewer,
};
use statecraft_run::documentation::workflow::{
    self, AdapterRef, Subjects, Transition, Workflow, WorkflowState,
};
use statecraft_run::documentation::{
    self as doc, DocumentationResult, DocumentationTask, NotRequired, Skill,
};
use statecraft_run::record::{Chain, EffectId, Entry, Identity, Kind};
use statecraft_run::wire::{SIDECAR_SUBJECT, Target, project, record_policy};

use injection::fixtures::{CITED, cited, facts, manifest, pages, task};
use injection::{
    Case, Framing, Observed, Runner, Side, Status, absent, load, qualification_claim, ran,
    run_suite,
};

const NOW: &str = "2026-10-29T00:00:00Z";
const OWNER: &str = "owner@example.invalid";
const ARTIFACT: &[u8] = b"# Recovery\n\nAt the next statement.\n";
const EVIDENCE: &str = "test-run-1";

/// A producer that answers every question from the bytes it is handed: a
/// page validates to its ordinal's facts, and a member resolves to the
/// digest of the text the page holds under that name.
struct Honest {
    pages: usize,
}

impl Producer for Honest {
    fn validate_page(&self, bytes: &[u8]) -> Validated {
        match serde_json::from_slice::<Value>(bytes)
            .ok()
            .and_then(|v| v["page"].as_u64())
        {
            Some(n) => Validated::Valid(Box::new(facts(n as usize, self.pages))),
            None => Validated::Invalid("not a fixture page".into()),
        }
    }
    fn closure(&self, _id: &str, _page: &[u8]) -> ClosureAnswer {
        ClosureAnswer::Answered {
            digest: facts(1, self.pages).closure_digest,
            differences: vec![],
        }
    }
    fn current(&self, _id: &str) -> CurrentAnswer {
        CurrentAnswer::Resolved(facts(1, self.pages).snapshot)
    }
    fn member(&self, page: &[u8], identity: &str) -> MemberAnswer {
        let v: Value = match serde_json::from_slice(page) {
            Ok(v) => v,
            Err(e) => return MemberAnswer::Unreadable(e.to_string()),
        };
        match v["members"][identity]["text"].as_str() {
            Some(text) => MemberAnswer::Found {
                content_digest: custody_digest(text.as_bytes()),
                start_line: 1,
                end_line: 1,
            },
            None => MemberAnswer::Absent,
        }
    }
}

fn context_policy() -> ContextPolicy {
    ContextPolicy {
        schema: "statecraft/context-acceptance-policy/1".into(),
        delivery: Need::Required,
        integrity: Need::Required,
        closure: Need::Required,
        freshness: Need::Required,
        citations: Need::Required,
        evidence: Need::Required,
        permitted_absences: vec![],
    }
}

/// The citations the one claim carries: the seeded targets on
/// `citation-target`, the benign member otherwise.
fn citations(case: &Case, side: Side, m: &Manifest) -> Vec<Citation> {
    let page = m.repositories[0].pages[0].page_digest.clone();
    let cite = |member: String, text: &str| Citation {
        repository: "app".into(),
        page_digest: page.clone(),
        member,
        content_digest: custody_digest(text.as_bytes()),
        span: None,
    };
    if case.surface == "citation-target" {
        case.bytes(side)
            .fragments
            .iter()
            .enumerate()
            .map(|(i, f)| cite(cited(i), f))
            .collect()
    } else {
        vec![cite(CITED.into(), injection::fixtures::CITED_TEXT)]
    }
}

/// The result: one implemented claim citing `cites`, and on
/// `provider-output` the payload as the claim's text and an unknown's
/// question.
fn result_bytes(t: &DocumentationTask, cites: &[Citation], said: Option<&str>) -> Vec<u8> {
    let text = said.unwrap_or("Recovery happens at the next statement.");
    let unknowns = match said {
        None => json!([]),
        Some(q) => json!([{
            "unknownId": "u-1", "question": q, "missingFact": "none",
            "reason": "reported by the provider",
        }]),
    };
    serde_json::to_vec(&json!({
        "schema": doc::RESULT_SCHEMA,
        "taskId": t.task_id, "runId": t.run_id, "attempt": 1,
        "taskDigest": t.task_digest, "contextManifestDigest": t.context_manifest_digest,
        "adapter": {"name": "fixed-export-fake", "version": "1"},
        "provider": {"name": "fixed-export-fake-provider", "version": "1"},
        "artifacts": [{
            "logicalPath": "docs/recovery.md", "mediaType": "text/markdown",
            "digest": custody_digest(ARTIFACT), "size": ARTIFACT.len(), "rawRef": "a0"
        }],
        "claims": [{
            "claimId": "c1", "text": text, "lifecycleGrade": "implemented",
            "citations": cites.iter().map(|c| json!({
                "repository": c.repository, "pageDigest": c.page_digest,
                "member": c.member, "contentDigest": c.content_digest,
            })).collect::<Vec<_>>(),
            "evidenceRefs": [EVIDENCE], "unsupportedReason": "not-applicable",
            "contradictionRefs": [], "reviewDisposition": "not-reviewed"
        }],
        "unknowns": unknowns, "contradictions": [], "unsupportedClaims": [],
        "providerOmissions": [], "statecraftCitationFailures": [],
        "executionEvidence": [{"reference": EVIDENCE, "purpose": "the suite"}],
        "providerCompletion": "claimed-complete",
        "reviewDisposition": "not-reviewed", "acceptanceResult": "not-attempted"
    }))
    .unwrap()
}

fn review_policy() -> (Policy, String) {
    let p = Policy {
        schema: da::POLICY_SCHEMA.into(),
        reviewers: vec![Grant {
            identity: OWNER.into(),
            authority: Authority::RepositoryOwner,
            not_after: None,
        }],
        quorum: 1,
        observed_transport: da::Need::Optional,
        max_artifact_bytes: 65_536,
    };
    Policy::decode(&serde_json::to_vec(&p).unwrap()).unwrap()
}

fn subject(t: &DocumentationTask, custody: &str, policy_digest: &str) -> CoreTuple {
    CoreTuple {
        repository: "example/app".into(),
        trusted_base: "b".repeat(40),
        run_id: t.run_id.clone(),
        attempt: 1,
        candidate: "c".repeat(40),
        context_manifest_digest: t.context_manifest_digest.clone(),
        task_digest: t.task_digest.clone(),
        result_custody_digest: custody.into(),
        result_schema_digest: t.result_schema.digest.clone(),
        artifacts: vec![ArtifactIdentity {
            logical_path: "docs/recovery.md".into(),
            digest: custody_digest(ARTIFACT),
        }],
        evidence: vec![EVIDENCE.into()],
        policy_digest: policy_digest.into(),
    }
}

/// The owner's independent review, accepting the subject it names.
fn review(subject: CoreTuple) -> Read<Review> {
    Read::Valid(
        Review {
            schema: "statecraft/documentation-review/1".into(),
            subject,
            reviewer: Reviewer {
                identity: OWNER.into(),
                authority: Authority::RepositoryOwner,
                authority_source: "policy reviewers[0]".into(),
            },
            reviewed_at: NOW.into(),
            claims: vec![ClaimReview {
                claim_id: "c1".into(),
                disposition: ClaimDisposition::Supported,
                reasons: vec![],
            }],
            artifacts: vec![ArtifactReview {
                logical_path: "docs/recovery.md".into(),
                disposition: ArtifactDisposition::Acceptable,
                reasons: vec![],
            }],
            corrections: vec![],
            recommendation: Recommendation::Accept,
            digest_construction: String::new(),
            review_digest: String::new(),
        }
        .seal(),
    )
}

fn validated(t: &DocumentationTask, custody: &str) -> Workflow {
    let at = |state| Transition {
        state,
        reason: "fixture".into(),
        at: NOW.into(),
    };
    let mut w = Workflow {
        schema: workflow::WORKFLOW_SCHEMA.into(),
        profile: workflow::PROFILE.into(),
        project: "example/app".into(),
        run_id: t.run_id.clone(),
        attempt: 1,
        candidate: "c".repeat(40),
        subjects: Subjects {
            manifest_digest: t.context_manifest_digest.clone(),
            task_digest: t.task_digest.clone(),
            skill: "not-required".into(),
            result_schema_digest: t.result_schema.digest.clone(),
            adapter: Some(AdapterRef {
                name: "fixed-export-fake".into(),
                version: "1".into(),
                capabilities: vec![],
            }),
            raw_result: Some(custody.into()),
            ..Subjects::default()
        },
        transitions: vec![
            at(WorkflowState::Prepared),
            at(WorkflowState::Executed),
            at(WorkflowState::Validated),
        ],
        findings: vec![],
        digest_construction: workflow::WORKFLOW_DIGEST_CONSTRUCTION.into(),
        workflow_digest: String::new(),
    };
    w.workflow_digest = w.computed_digest();
    w
}

/// Seed a documentation case and observe spec 019's, 021's and 022's
/// judgments. Framing is the adapter's concern; both framings run the same
/// path here.
fn observe_documentation(runner: &Runner, case: &Case, side: Side) -> Observed {
    let mut seen = Observed::default();
    let packet = pages(case, side);
    let m = manifest(&packet);
    let t = task(
        &m,
        &"c".repeat(40),
        Skill::NotRequired(NotRequired::NotRequired),
    );
    let cites = citations(case, side, &m);
    let said = (case.surface == "provider-output").then(|| case.bytes(side).assembled());
    let bytes = result_bytes(&t, &cites, said.as_deref());
    let custody = custody_digest(&bytes);
    let result = DocumentationResult::decode(&bytes, &t).map_err(|e| e.to_string());

    let mut store: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for p in &packet {
        store.insert(custody_digest(p), p.clone());
    }
    store.insert(custody_digest(ARTIFACT), ARTIFACT.to_vec());
    let retained = |d: &str| store.get(d).cloned();

    let policy = context_policy();
    let delivery = fold_delivery(
        &m,
        &[Observation::Reported(Reported {
            manifest_digest: m.manifest_digest.clone(),
            visible: m.pages().map(|(_, p)| p.page_digest.clone()).collect(),
            withheld: vec![],
        })],
    );
    let evidence = vec![EvidenceResult {
        reference: EVIDENCE.into(),
        decision: "admit".into(),
    }];
    let judged = ctx::judge(
        &ctx::Inputs {
            manifest: &m,
            policy: &policy,
            delivery: &delivery,
            retained: &retained,
            citations: &cites,
            evidence: &evidence,
        },
        &Honest {
            pages: packet.len(),
        },
    );

    let (rp, rp_digest) = review_policy();
    let reviewed = vec![review(subject(&t, &custody, &rp_digest))];
    let doc_delivery = DocDelivery {
        reported: vec![],
        findings: vec![],
        eligible: true,
    };
    let base = "b".repeat(40);
    let candidate = "c".repeat(40);
    let judge = |reviews: &[Read<Review>]| {
        da::judge(&da::Inputs {
            repository: "example/app",
            trusted_base: &base,
            candidate: Ok(&candidate),
            task: &t,
            delivery: Some(&doc_delivery),
            result_custody: Some(&custody),
            result: result.as_ref().map_err(Clone::clone),
            context: Some(&judged),
            manifest_freshness: ManifestFreshness::Fresh,
            retained: &retained,
            reviews,
            corrections: &[],
            policy: Ok((&rp, &rp_digest)),
            changed_paths: Some(&[]),
            now: NOW,
        })
    };
    let with_review = judge(&reviewed);
    let without_review = judge(&[]);
    let w = validated(&t, &custody);
    let workflow_judgment = fx::judge(Ok(&w), &t, with_review.clone(), &retained);

    seen.decide(
        "citation-resolution",
        &json!({
            "citations": judged.citations,
            "integrity": judged.integrity,
            "closure": judged.closure_aggregate,
            "freshness": judged.freshness_aggregate,
        }),
    );
    seen.decide(
        "evidence-admission",
        &json!({
            "evidence": judged.evidence,
            "eligible": judged.eligible,
            "reasons": judged.reasons,
        }),
    );
    seen.decide(
        "review-disposition",
        &json!({
            "sentinel": result.as_ref().map(|r| r.review_disposition),
            "claims": result.as_ref().map(|r| r.claims.iter().map(|c| c.review_disposition).collect::<Vec<_>>()),
            "boundWithReview": with_review.bound_reviews,
            "boundWithoutReview": without_review.bound_reviews,
        }),
    );
    seen.decide(
        "acceptance-result",
        &json!({
            "sentinel": result.as_ref().map(|r| r.acceptance_result),
            "withReview": with_review,
            "withoutReview": without_review,
            "workflow": workflow_judgment,
        }),
    );

    for r in runner.recordings {
        seen.collection(r);
    }
    match runner.surface {
        "citation-target" => {
            for c in &cites {
                if let Some(text) = packet
                    .first()
                    .and_then(|p| serde_json::from_slice::<Value>(p).ok())
                    .and_then(|v| v["members"][&c.member]["text"].as_str().map(str::to_string))
                {
                    seen.record("citation-text", text);
                }
            }
        }
        "context-packet" => {
            for p in &packet {
                seen.record_json("packet-pages", p);
            }
        }
        _ => {
            if let Ok(r) = &result {
                for c in &r.claims {
                    seen.record("result-document", c.text.clone());
                }
                for u in &r.unknowns {
                    seen.record("result-document", u.question.clone());
                }
            }
        }
    }
    seen
}

// Spec 014's wire shape: an attempt whose sidecar bracket closed on the
// manifest's bytes, so admission reaches the provider-attributed values.
const WIRE_RUN: &str = "run-025";
const REGISTERED: &str = "registered-value-0001";

fn sidecar(attempt: u32) -> String {
    format!("{WIRE_RUN}/{attempt}/witness-sidecar")
}

fn put(chain: &mut Chain, kind: Kind, subject: &str, detail: Value, effect: bool) {
    let id = format!("r{}", chain.records().len());
    chain
        .append(
            &id,
            NOW,
            &Entry {
                kind,
                run_id: WIRE_RUN.into(),
                attempt: 1,
                subject: subject.into(),
                idempotency_key: None,
                detail,
                effect_id: if effect {
                    Identity::Valid(EffectId::new(&sidecar(1)).unwrap())
                } else {
                    Identity::Absent
                },
            },
        )
        .unwrap();
}

/// The witness manifest with each fragment as one exchange's served model,
/// which is provider-attributed testimony.
fn wire_manifest(case: &Case, side: Side, subject: &Subject) -> Vec<u8> {
    let mut m = wire_fixture::manifest(subject);
    let template = m["exchanges"][0].clone();
    let fragments = &case.bytes(side).fragments;
    let exchanges: Vec<Value> = fragments
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut e = template.clone();
            e["sequence"] = json!(i + 1);
            e["served"]["model"]["value"] = json!(f);
            e
        })
        .collect();
    m["exchanges"] = json!(exchanges);
    m["counts"]["observedExchanges"] = json!(fragments.len());
    m["counts"]["recordedExchanges"] = json!(fragments.len());
    wire_fixture::bytes(&m)
}

fn observe_wire(case: &Case, side: Side) -> Observed {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let target_dir = root.path().join("target");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&target_dir).unwrap();
    let mut seen = Observed::rooted_at(&[root.path()]);
    let subject = Subject {
        run_id: WIRE_RUN.into(),
        attempt: 1,
        sidecar_effect_id: sidecar(1),
    };
    let manifest_bytes = wire_manifest(case, side, &subject);
    let reference = wire_fixture::bytes(&wire_fixture::reference(&manifest_bytes, &subject));
    let target = Target {
        run_id: WIRE_RUN.into(),
        attempt: 1,
    };
    let mut chain = Chain::open(&home, &target_dir).unwrap().0;
    let policy = WirePolicy {
        secret_detector: Some(DEFAULT_DETECTOR.into()),
        requirement: None,
    };
    record_policy(&mut chain, &target, &policy, NOW).unwrap();
    put(
        &mut chain,
        Kind::Intent,
        "prepare-workspace",
        json!({"baseCommit": "b"}),
        false,
    );
    put(
        &mut chain,
        Kind::Intent,
        SIDECAR_SUBJECT,
        json!({
            "binding": {"runId": WIRE_RUN, "attempt": 1, "effectId": sidecar(1)},
            "producer": {
                "name": "wire-witness",
                "version": wire_fixture::REVISION,
                "executableDigest": wire_fixture::producer_digest(),
            }
        }),
        true,
    );
    put(
        &mut chain,
        Kind::Outcome,
        SIDECAR_SUBJECT,
        json!({
            "effectId": sidecar(1),
            "captureDigest": format!("sha256:{}", sha256_hex(&manifest_bytes)),
        }),
        true,
    );
    put(
        &mut chain,
        Kind::Outcome,
        "attempt",
        json!({"outcome": "completed", "witnessPosture": "admitted", "witnessRequired": false}),
        false,
    );
    let detector = DefaultDetector::with_registered([REGISTERED.to_string()]);
    let record = admit(
        &mut chain,
        &target,
        &Candidate {
            reference: &reference,
            manifest: &manifest_bytes,
            custody: Custody {
                obtained_from: Stated::known("sidecar-area/wire-exchange.json".into()),
                writer: Stated::known("wire-witness@0.1.0".into()),
                reader: Stated::known("statecraft".into()),
                handoff: Handoff::Complete,
            },
        },
        Some(&detector),
        NOW,
    )
    .map_err(|e| e.0);
    let chain = Chain::open(&home, &target_dir).unwrap().0;

    // The admitted facts are testimony carried as data; everything else in
    // the admission record is the decision.
    let mut decided = serde_json::to_value(&record).unwrap();
    if let Some(facts) = decided.pointer_mut("/Ok/facts") {
        // Only admitted facts are retained; a refusal records none.
        if let Some(value) = facts.get("value") {
            seen.record_json("wire-facts", &serde_json::to_vec(value).unwrap());
        }
        *facts = json!("<testimony>");
    }
    seen.collection("wire-facts");
    // Spec 014's secret screen refuses content that looks like a credential
    // and keeps none of it: a fail-closed refusal a case may declare.
    if decided.pointer("/Ok/reason") == Some(&json!("prohibited-content")) {
        for d in ["evidence-admission", "attempt-record", "acceptance-result"] {
            seen.refused.insert(
                d.to_string(),
                "014-wire-evidence-admission/prohibited-content".to_string(),
            );
        }
    }
    seen.decide("evidence-admission", &decided);
    let mut projected = serde_json::to_value(project(&chain, WIRE_RUN)).unwrap();
    strip_facts(&mut projected);
    seen.decide("attempt-record", &projected);
    seen.decide(
        "acceptance-result",
        &serde_json::to_value(eligibility(&chain, &target)).unwrap(),
    );
    seen
}

/// Replace every admitted `facts` member with a marker: the testimony is
/// recorded once, as data, from the admission record itself.
fn strip_facts(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if let Some(f) = map.get_mut("facts") {
                *f = json!("<testimony>");
            }
            map.values_mut().for_each(strip_facts);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_facts),
        _ => {}
    }
}

fn observe(runner: &Runner, case: &Case, side: Side, _framing: Framing) -> Observed {
    if runner.surface == "wire-metadata" {
        observe_wire(case, side)
    } else {
        observe_documentation(runner, case, side)
    }
}

#[test]
fn every_acceptance_surface_case_leaves_every_decision_equal_to_its_twin() {
    let corpus = load();
    let statuses = run_suite(&corpus, "acceptance", observe);
    let failed: Vec<String> = statuses
        .iter()
        .filter_map(|(id, s)| match s {
            Status::Failed(f) => Some(format!("{id}:\n    {}", f.join("\n    "))),
            _ => None,
        })
        .collect();
    assert!(failed.is_empty(), "{}\n{}", ran(&corpus), failed.join("\n"));
    let passed = statuses.values().filter(|s| **s == Status::Passed).count();
    assert_eq!(
        passed, 28,
        "citation-target, wire-metadata and context-packet, and the four \
         provider-output cases that seed the result document"
    );
    // Section 2: every surface now has a runner, so no required case is
    // `surface-absent` and the suite can make its claim.
    assert_eq!(absent(&corpus), Vec::<String>::new());
    assert_eq!(qualification_claim(&statuses), Ok(()));
}

// Section 3.8's second row: a provider result that sets the review is data;
// the sentinel stays `not-reviewed`, and without an independent review there
// is no acceptance.
#[test]
fn a_result_claiming_review_keeps_the_sentinel_and_earns_no_acceptance() {
    let corpus = load();
    let case = corpus
        .cases
        .iter()
        .find(|c| c.case_id == "provider-output/review-bypass")
        .unwrap();
    assert!(
        case.payload
            .assembled()
            .contains("reviewDisposition: approved")
    );
    let runner = injection::RUNNERS
        .iter()
        .find(|r| r.harness == "acceptance" && r.surface == "provider-output")
        .unwrap();
    let o = observe(runner, case, Side::Hostile, Framing::Intact);
    assert_eq!(
        o.decisions["review-disposition"]["sentinel"]["Ok"],
        "not-reviewed"
    );
    assert_eq!(
        o.decisions["review-disposition"]["boundWithoutReview"],
        json!([])
    );
    assert_ne!(
        o.decisions["acceptance-result"]["withoutReview"]["outcome"],
        "accepted"
    );
}

// Section 3.8's third row: a cited text that claims `released` is resolved as
// a citation and nothing more; the lifecycle the claim states is judged from
// its evidence, and is the twin's.
#[test]
fn a_citation_target_claiming_released_does_not_grade_the_claim() {
    let corpus = load();
    let case = corpus
        .cases
        .iter()
        .find(|c| c.case_id == "citation-target/self-acceptance")
        .unwrap();
    assert!(case.payload.assembled().contains("released"));
    let runner = injection::RUNNERS
        .iter()
        .find(|r| r.harness == "acceptance" && r.surface == "citation-target")
        .unwrap();
    let hostile = observe(runner, case, Side::Hostile, Framing::Intact);
    let twin = observe(runner, case, Side::Twin, Framing::Intact);
    let states = |o: &Observed| o.decisions["citation-resolution"]["citations"].clone();
    assert_eq!(states(&hostile), states(&twin));
    assert_eq!(states(&hostile)[0]["state"], "resolved");
    assert_eq!(
        hostile.decisions["acceptance-result"],
        twin.decisions["acceptance-result"]
    );
}
