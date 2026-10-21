//! Spec 019 sections 3.7 and 3.8: delivery and five independent checks over
//! a fixture producer, and eligibility under the recorded policy alone. No
//! producer binary runs and no provider runs; the fixture answers exactly what
//! each row needs.

use std::collections::BTreeMap;

use statecraft_acceptance::context::{
    Citation, CitationState, ClosureAnswer, ClosureState, CurrentAnswer, EvidenceResult,
    FreshnessState, Inputs, IntegrityState, MemberAnswer, Producer, Validated, judge,
};
use statecraft_run::context::{
    AcceptedPage, AcceptedRepository, Completeness, ContextPolicy, DeliveryPolicy, Dimension,
    Intended, Manifest, Need, Observation, OptionalOmission, POLICY_SCHEMA, PageFacts,
    PermittedAbsence, Producer as Adopted, ProducerBuild, Reported, Requirement, Snapshot, Task,
    custody_digest, fold_delivery,
};

fn facts(repo: &str, page: &str, continues: bool) -> PageFacts {
    PageFacts {
        schema_version: "1.1.0".into(),
        producer: ProducerBuild {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            build: "unverified".into(),
        },
        snapshot: snapshot(repo, "tree-1"),
        request_digest: custody_digest(format!("{repo}-request").as_bytes()),
        closure_digest: custody_digest(format!("{repo}-closure").as_bytes()),
        packet_digest: custody_digest(page.as_bytes()),
        completeness: Completeness::Complete,
        continues,
        omissions: vec![],
        warnings: vec![],
    }
}

fn snapshot(repo: &str, tree: &str) -> Snapshot {
    Snapshot {
        repository: format!("example/{repo}"),
        revision: format!("rev-{repo}"),
        tree: tree.into(),
        dirty_state: "clean-export".into(),
    }
}

struct Fixture {
    repos: Vec<AcceptedRepository>,
    manifest: Manifest,
}

fn fixture() -> Fixture {
    let page = |repo: &str, name: &str, continues: bool| AcceptedPage {
        bytes: format!("{{\"page\":\"{name}\"}}").into_bytes(),
        source_operation: "context packet".into(),
        facts: facts(repo, name, continues),
        delivery: Intended::Required,
    };
    let repos = vec![
        AcceptedRepository {
            id: "app".into(),
            role: "source".into(),
            requirement: Requirement::Required,
            pages: vec![page("app", "app-1", true), page("app", "app-2", false)],
        },
        AcceptedRepository {
            id: "lib".into(),
            role: "dependency".into(),
            requirement: Requirement::Optional,
            pages: vec![page("lib", "lib-1", false)],
        },
    ];
    let manifest = Manifest::build(
        "run-019",
        1,
        Task::NotRecorded,
        &Adopted {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            packet_interface: "1.1.0".into(),
        },
        &repos,
        DeliveryPolicy::new(OptionalOmission::Permit),
    )
    .unwrap();
    Fixture { repos, manifest }
}

/// A producer whose every answer a row can override.
struct Fake {
    pages: BTreeMap<Vec<u8>, Validated>,
    closure: BTreeMap<String, ClosureAnswer>,
    current: BTreeMap<String, CurrentAnswer>,
    members: BTreeMap<String, MemberAnswer>,
}

impl Fake {
    fn honest(f: &Fixture) -> Self {
        let mut pages = BTreeMap::new();
        let mut closure = BTreeMap::new();
        let mut current = BTreeMap::new();
        for r in &f.repos {
            for p in &r.pages {
                pages.insert(p.bytes.clone(), Validated::Valid(Box::new(p.facts.clone())));
            }
            closure.insert(
                r.id.clone(),
                ClosureAnswer::Answered {
                    digest: r.pages[0].facts.closure_digest.clone(),
                    differences: vec![],
                },
            );
            current.insert(
                r.id.clone(),
                CurrentAnswer::Resolved(r.pages[0].facts.snapshot.clone()),
            );
        }
        let mut members = BTreeMap::new();
        members.insert(
            "spec:019".to_string(),
            MemberAnswer::Found {
                content_digest: custody_digest(b"member"),
                start_line: 10,
                end_line: 20,
            },
        );
        members.insert("spec:042".to_string(), MemberAnswer::Omitted);
        Fake {
            pages,
            closure,
            current,
            members,
        }
    }
}

impl Producer for Fake {
    fn validate_page(&self, bytes: &[u8]) -> Validated {
        self.pages
            .get(bytes)
            .cloned()
            .unwrap_or_else(|| Validated::Invalid("unknown page".into()))
    }
    fn closure(&self, id: &str, _page: &[u8]) -> ClosureAnswer {
        self.closure[id].clone()
    }
    fn current(&self, id: &str) -> CurrentAnswer {
        self.current[id].clone()
    }
    fn member(&self, _page: &[u8], identity: &str) -> MemberAnswer {
        self.members
            .get(identity)
            .cloned()
            .unwrap_or(MemberAnswer::Absent)
    }
}

fn policy(need: Need) -> ContextPolicy {
    ContextPolicy {
        schema: POLICY_SCHEMA.into(),
        delivery: need,
        integrity: need,
        closure: need,
        freshness: need,
        citations: need,
        evidence: need,
        permitted_absences: vec![],
    }
}

fn store(f: &Fixture) -> BTreeMap<String, Vec<u8>> {
    f.repos
        .iter()
        .flat_map(|r| r.pages.iter())
        .map(|p| (custody_digest(&p.bytes), p.bytes.clone()))
        .collect()
}

fn citation(member: &str, digest: &[u8]) -> Citation {
    let f = fixture();
    Citation {
        repository: "app".into(),
        page_digest: f.manifest.repositories[0].pages[0].page_digest.clone(),
        member: member.into(),
        content_digest: custody_digest(digest),
        span: None,
    }
}

fn admitted() -> Vec<EvidenceResult> {
    vec![EvidenceResult {
        reference: "evidence-1".into(),
        decision: "admit".into(),
    }]
}

struct Row {
    f: Fixture,
    producer: Fake,
    store: BTreeMap<String, Vec<u8>>,
    observations: Vec<Observation>,
    citations: Vec<Citation>,
    evidence: Vec<EvidenceResult>,
    policy: ContextPolicy,
}

impl Row {
    fn new() -> Self {
        let f = fixture();
        let producer = Fake::honest(&f);
        let store = store(&f);
        let observations = vec![Observation::Reported(Reported {
            manifest_digest: f.manifest.manifest_digest.clone(),
            visible: f
                .manifest
                .pages()
                .map(|(_, p)| p.page_digest.clone())
                .collect(),
            withheld: vec![],
        })];
        Row {
            f,
            producer,
            store,
            observations,
            citations: vec![citation("spec:019", b"member")],
            evidence: admitted(),
            policy: policy(Need::Required),
        }
    }

    fn judge(&self) -> statecraft_acceptance::context::ContextResult {
        let delivery = fold_delivery(&self.f.manifest, &self.observations);
        let retained = |c: &str| self.store.get(c).cloned();
        judge(
            &Inputs {
                manifest: &self.f.manifest,
                policy: &self.policy,
                delivery: &delivery,
                retained: &retained,
                citations: &self.citations,
                evidence: &self.evidence,
            },
            &self.producer,
        )
    }
}

fn reasons(r: &statecraft_acceptance::context::ContextResult) -> Vec<String> {
    r.reasons.iter().map(|r| r.to_string()).collect()
}

#[test]
fn every_dimension_passing_is_eligible_and_every_result_is_returned() {
    let r = Row::new().judge();
    assert!(r.eligible, "{:?}", reasons(&r));
    assert_eq!(r.integrity.len(), 3);
    assert!(r.integrity.iter().all(|p| p.state == IntegrityState::Pass));
    assert_eq!(r.closure_aggregate, ClosureState::Current);
    assert_eq!(r.freshness_aggregate, FreshnessState::Fresh);
    assert_eq!(r.citations[0].state, CitationState::Resolved);
    assert_eq!(r.policy, policy(Need::Required));
}

// Section 3.10 row 3.
#[test]
fn bytes_that_differ_from_custody_fail_integrity_before_the_producer_sees_them() {
    let mut row = Row::new();
    let custody = row.f.manifest.repositories[0].pages[0]
        .custody_digest
        .clone();
    row.store
        .insert(custody, b"{\"page\":\"tampered\"}".to_vec());
    // A producer that would call the tampered bytes valid is never asked.
    row.producer.pages.insert(
        b"{\"page\":\"tampered\"}".to_vec(),
        Validated::Valid(Box::new(row.f.repos[0].pages[0].facts.clone())),
    );
    let r = row.judge();
    assert_eq!(r.integrity[0].state, IntegrityState::Fail);
    assert_eq!(r.integrity[0].custody, "mismatch");
    assert!(!r.eligible);
    assert!(
        reasons(&r)
            .iter()
            .any(|s| s.starts_with("context-integrity-fail"))
    );
}

// Section 3.10 row 4.
#[test]
fn unavailable_page_validation_is_unsupported_and_never_reconstructed() {
    let mut row = Row::new();
    for v in row.producer.pages.values_mut() {
        *v = Validated::Unsupported("no validator".into());
    }
    let r = row.judge();
    assert!(
        r.integrity
            .iter()
            .all(|p| p.state == IntegrityState::Unsupported)
    );
    assert!(!r.eligible);
}

#[test]
fn a_page_whose_producer_facts_disagree_with_the_manifest_fails() {
    let mut row = Row::new();
    let bytes = row.f.repos[0].pages[1].bytes.clone();
    let mut lied = row.f.repos[0].pages[1].facts.clone();
    lied.closure_digest = custody_digest(b"another closure");
    row.producer
        .pages
        .insert(bytes, Validated::Valid(Box::new(lied)));
    let r = row.judge();
    assert_eq!(r.integrity[1].state, IntegrityState::Fail);
    assert_eq!(
        r.integrity[1].reason.as_deref(),
        Some("recorded closure digest")
    );
    // Integrity says nothing about closure: that check is separate.
    assert_eq!(r.closure[0].state, ClosureState::Current);
}

#[test]
fn absent_retained_bytes_are_not_recorded_never_a_pass() {
    let mut row = Row::new();
    row.store.clear();
    let r = row.judge();
    assert!(
        r.integrity
            .iter()
            .all(|p| p.state == IntegrityState::NotRecorded)
    );
    assert!(
        r.closure
            .iter()
            .all(|c| c.state == ClosureState::NotRecorded)
    );
    assert_eq!(r.citations[0].state, CitationState::NotRecorded);
    assert!(!r.eligible);
}

#[test]
fn closure_reports_every_producer_difference_and_never_merges_repositories() {
    let mut row = Row::new();
    row.producer.closure.insert(
        "app".into(),
        ClosureAnswer::Answered {
            digest: custody_digest(b"newer"),
            differences: vec!["spec:007".into()],
        },
    );
    row.producer
        .closure
        .insert("lib".into(), ClosureAnswer::Stale("index stale".into()));
    let r = row.judge();
    assert_eq!(r.closure.len(), 2);
    assert_eq!(r.closure[0].state, ClosureState::Changed);
    assert_eq!(r.closure[0].differences, ["spec:007"]);
    assert_eq!(r.closure[1].state, ClosureState::Stale);
    assert_eq!(r.closure_aggregate, ClosureState::Changed);
}

// Section 3.10 row 7.
#[test]
fn one_dirty_checkout_is_unverified_and_no_other_result_hides_it() {
    let mut row = Row::new();
    row.producer.current.insert(
        "app".into(),
        CurrentAnswer::Unverified("dirty working tree".into()),
    );
    let r = row.judge();
    assert_eq!(r.freshness[0].state, FreshnessState::Unverified);
    assert_eq!(r.freshness[1].state, FreshnessState::Fresh);
    assert_eq!(r.freshness_aggregate, FreshnessState::Unverified);
    assert!(reasons(&r).contains(&"context-freshness-unverified: app".to_string()));
}

#[test]
fn another_tree_is_stale_and_an_incomplete_packet_is_incomplete() {
    let mut row = Row::new();
    row.producer.current.insert(
        "app".into(),
        CurrentAnswer::Resolved(snapshot("app", "tree-2")),
    );
    assert_eq!(row.judge().freshness[0].state, FreshnessState::Stale);

    let mut row = Row::new();
    row.f.repos[0].pages[1].facts.completeness = Completeness::Incomplete;
    row.f = Fixture {
        manifest: Manifest::build(
            "run-019",
            1,
            Task::NotRecorded,
            &Adopted {
                package: "spec-spine".into(),
                version: "0.31.0".into(),
                packet_interface: "1.1.0".into(),
            },
            &row.f.repos,
            DeliveryPolicy::new(OptionalOmission::Permit),
        )
        .unwrap(),
        repos: row.f.repos.clone(),
    };
    row.producer = Fake::honest(&row.f);
    row.observations = vec![];
    assert_eq!(row.judge().freshness[0].state, FreshnessState::Incomplete);
}

// Section 3.10 row 8.
#[test]
fn equal_text_under_another_identity_never_resolves_a_citation() {
    let mut row = Row::new();
    row.citations = vec![
        citation("spec:019", b"other text"),
        citation("spec:999", b"member"),
        citation("spec:042", b"member"),
        Citation {
            span: Some((5, 12)),
            ..citation("spec:019", b"member")
        },
        Citation {
            repository: "lib".into(),
            ..citation("spec:019", b"member")
        },
    ];
    let states: Vec<CitationState> = row.judge().citations.iter().map(|c| c.state).collect();
    assert_eq!(
        states,
        [
            CitationState::DigestMismatch,
            CitationState::OutsidePacket,
            CitationState::Missing,
            CitationState::OutsidePacket,
            CitationState::OutsidePacket,
        ]
    );
}

// Section 3.10 rows 5 and 9.
#[test]
fn provider_testimony_is_not_evidence_and_unreported_delivery_is_ineligible() {
    let mut row = Row::new();
    row.observations = vec![Observation::NotReported];
    row.evidence = vec![];
    let r = row.judge();
    assert!(!r.eligible);
    let all = reasons(&r);
    assert!(
        all.iter()
            .any(|s| s.starts_with("context-delivery-not-reported"))
    );
    // Every other check passed; none of them stood in for evidence.
    assert!(all.contains(&"context-evidence-not-recorded: evidence".to_string()));
    assert_eq!(r.evidence, vec![]);
}

#[test]
fn an_optional_fact_coexists_with_acceptance_only_under_its_exact_permitted_absence() {
    let mut row = Row::new();
    row.producer
        .current
        .insert("lib".into(), CurrentAnswer::Unverified("absent".into()));
    let r = row.judge();
    assert!(
        !r.eligible,
        "an optional repository's absence must be permitted"
    );
    assert_eq!(reasons(&r), ["context-freshness-unverified: lib"]);
    row.policy.permitted_absences.push(PermittedAbsence {
        dimension: Dimension::Freshness,
        state: "unverified".into(),
    });
    assert!(row.judge().eligible);
    // The permission is exact: it does not admit another state.
    row.producer.current.insert(
        "lib".into(),
        CurrentAnswer::Resolved(snapshot("lib", "tree-9")),
    );
    assert_eq!(reasons(&row.judge()), ["context-freshness-stale: lib"]);
}

#[test]
fn a_required_fact_is_never_rescued_by_a_permitted_absence() {
    let mut row = Row::new();
    row.producer
        .current
        .insert("app".into(), CurrentAnswer::Unverified("absent".into()));
    row.policy.permitted_absences.push(PermittedAbsence {
        dimension: Dimension::Freshness,
        state: "unverified".into(),
    });
    assert!(!row.judge().eligible);
    // Under an optional dimension the same permission applies.
    row.policy.freshness = Need::Optional;
    assert!(row.judge().eligible);
}

#[test]
fn a_withheld_page_is_not_a_delivery_fact() {
    let mut row = Row::new();
    row.f.repos[1].pages[0].delivery = Intended::Withheld;
    let repos = row.f.repos.clone();
    row.f.manifest = Manifest::build(
        "run-019",
        1,
        Task::NotRecorded,
        &Adopted {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            packet_interface: "1.1.0".into(),
        },
        &repos,
        DeliveryPolicy::new(OptionalOmission::Permit),
    )
    .unwrap();
    row.observations = vec![Observation::Reported(Reported {
        manifest_digest: row.f.manifest.manifest_digest.clone(),
        visible: row.f.manifest.repositories[0]
            .pages
            .iter()
            .map(|p| p.page_digest.clone())
            .collect(),
        withheld: vec![],
    })];
    assert!(row.judge().eligible, "{:?}", reasons(&row.judge()));
}
