//! Spec 019's run-record half: the task manifest, its custody and bounds,
//! the intent that records it before any effect, and intended beside
//! adapter-observed delivery. Every fact here is a portable fixture: no
//! producer runs, no provider runs, and every chain read is reopened from
//! disk.

use std::path::Path;
use std::process::Command;

use statecraft_environment::time::FixedClock;
use statecraft_run::context::{
    self, AcceptedPage, AcceptedRepository, BOUNDS, BoundsError, BuildError, Completeness,
    DeliveryPolicy, Intended, Manifest, Observation, OmissionFact, OptionalOmission, PageFacts,
    Producer, ProducerBuild, Projection, Reported, RequestMismatch, Requirement, Snapshot, Task,
    Visibility, WarningFact, Withheld,
};
use statecraft_run::record::{Chain, Kind};
use statecraft_run::session::{self, IntentDetail, SessionError};

const RUN: &str = "run-019";

fn producer() -> Producer {
    Producer {
        package: "spec-spine".into(),
        version: "0.31.0".into(),
        packet_interface: "1.1.0".into(),
    }
}

fn digest(tag: &str) -> String {
    context::custody_digest(tag.as_bytes())
}

fn facts(repo: &str, revision: &str, page: &str, continues: bool) -> PageFacts {
    PageFacts {
        schema_version: "1.1.0".into(),
        producer: ProducerBuild {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            build: "unverified".into(),
        },
        snapshot: Snapshot {
            repository: format!("example/{repo}"),
            revision: revision.into(),
            tree: format!("tree-{revision}"),
            dirty_state: "clean-export".into(),
        },
        request_digest: digest(&format!("{repo}-request")),
        closure_digest: digest(&format!("{repo}-closure")),
        packet_digest: digest(page),
        completeness: Completeness::Complete,
        continues,
        omissions: vec![],
        warnings: vec![],
    }
}

fn page(facts: PageFacts) -> AcceptedPage {
    AcceptedPage {
        bytes: format!("{{\"page\":\"{}\"}}", facts.packet_digest).into_bytes(),
        source_operation: "context_packet_pages".into(),
        facts,
        delivery: Intended::Required,
    }
}

fn repository(id: &str, requirement: Requirement, pages: Vec<AcceptedPage>) -> AcceptedRepository {
    AcceptedRepository {
        id: id.into(),
        role: "source".into(),
        requirement,
        pages,
    }
}

fn two_repositories() -> Vec<AcceptedRepository> {
    vec![
        repository(
            "app",
            Requirement::Required,
            vec![
                page(facts("app", "rev-a", "app-1", true)),
                page(facts("app", "rev-a", "app-2", false)),
            ],
        ),
        repository(
            "lib",
            Requirement::Optional,
            vec![page(facts("lib", "rev-b", "lib-1", false))],
        ),
    ]
}

fn build(attempt: u32, repos: &[AcceptedRepository], rule: OptionalOmission) -> Manifest {
    Manifest::build(
        RUN,
        attempt,
        Task::NotRecorded,
        &producer(),
        repos,
        DeliveryPolicy::new(rule),
    )
    .unwrap()
}

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
    git(dir.path(), &["init", "--quiet", "--initial-branch=main"]);
    git(
        dir.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    git(dir.path(), &["config", "user.name", "test"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.path().join("README.md"), b"x").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "one"]);
    dir
}

fn begin(
    chain: &mut Chain,
    target: &Path,
    manifest: Option<&Manifest>,
) -> Result<u32, SessionError> {
    session::begin_with(
        chain,
        target,
        RUN,
        "main",
        &FixedClock(1_790_000_000),
        &IntentDetail {
            context_manifest: manifest,
            ..IntentDetail::default()
        },
    )
    .map(|s| s.attempt)
}

// Section 3.10 row 1, section 3.2.
#[test]
fn two_packets_at_different_revisions_keep_both_identities_and_no_shared_snapshot() {
    let manifest = build(1, &two_repositories(), OptionalOmission::Refuse);
    assert_eq!(manifest.schema, context::MANIFEST_SCHEMA);
    assert_eq!(manifest.repositories[0].snapshot.revision, "rev-a");
    assert_eq!(manifest.repositories[1].snapshot.revision, "rev-b");
    let doc: serde_json::Value = serde_json::from_slice(&manifest.to_bytes()).unwrap();
    let top: Vec<&str> = doc
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for shared in ["snapshot", "revision", "tree", "dirtyState", "atomic"] {
        assert!(
            !top.contains(&shared),
            "no shared {shared} at the top level"
        );
    }
    assert_eq!(doc["task"], "not-recorded");
    assert_eq!(doc["repositories"][0]["continuation"], "terminal");
    assert!(manifest.eligibility().is_ok());
}

// Section 3.10 row 2.
#[test]
fn a_required_omission_makes_execution_ineligible_and_stays_visible() {
    let mut repos = two_repositories();
    let last = &mut repos[0].pages[1].facts;
    last.completeness = Completeness::Incomplete;
    last.omissions.push(OmissionFact {
        identity: "spec:042".into(),
        requirement: Requirement::Required,
        reason: "missing".into(),
    });
    last.warnings.push(WarningFact {
        code: "unverified-producer-build".into(),
        identity: None,
    });
    let manifest = build(1, &repos, OptionalOmission::Permit);
    let refused = manifest.eligibility().unwrap_err();
    assert_eq!(refused.reason, "required-incomplete");
    assert_eq!(refused.repository, "app");
    let page = &manifest.repositories[0].pages[1];
    assert_eq!(page.omissions.len(), 1);
    assert_eq!(page.omissions[0].index, 0);
    assert_eq!(page.omissions[0].fact.reason, "missing");
    assert_eq!(page.warnings[0].fact.code, "unverified-producer-build");
    // Every collection is explicit, including the empty ones.
    let doc: serde_json::Value = serde_json::from_slice(&manifest.to_bytes()).unwrap();
    assert_eq!(
        doc["repositories"][1]["pages"][0]["omissions"],
        serde_json::json!([])
    );
    assert_eq!(
        doc["repositories"][1]["pages"][0]["warnings"],
        serde_json::json!([])
    );
}

#[test]
fn an_incomplete_optional_repository_is_eligible_only_where_the_policy_permits() {
    let mut repos = two_repositories();
    repos[1].pages[0].facts.completeness = Completeness::Partial;
    assert!(
        build(1, &repos, OptionalOmission::Permit)
            .eligibility()
            .is_ok()
    );
    let refused = build(1, &repos, OptionalOmission::Refuse)
        .eligibility()
        .unwrap_err();
    assert_eq!(refused.reason, "optional-incomplete");
}

// Section 3.3: identities repeat consistently, or nothing is composed.
#[test]
fn inconsistent_unterminated_duplicate_or_foreign_pages_refuse() {
    let refuse = |repos: Vec<AcceptedRepository>| {
        Manifest::build(
            RUN,
            1,
            Task::NotRecorded,
            &producer(),
            &repos,
            DeliveryPolicy::new(OptionalOmission::Refuse),
        )
        .unwrap_err()
    };

    let mut repos = two_repositories();
    repos[0].pages[1].facts.snapshot.tree = "another-tree".into();
    assert!(matches!(
        refuse(repos),
        BuildError::Inconsistent {
            field: "snapshot",
            ordinal: 2,
            ..
        }
    ));

    let mut repos = two_repositories();
    repos[0].pages[1].facts.closure_digest = digest("other");
    assert!(matches!(
        refuse(repos),
        BuildError::Inconsistent {
            field: "closureDigest",
            ..
        }
    ));

    let mut repos = two_repositories();
    repos[0].pages[1].facts.continues = true;
    assert!(matches!(refuse(repos), BuildError::NotTerminal(r) if r == "app"));

    let mut repos = two_repositories();
    repos[1].pages[0].facts.packet_digest = digest("app-1");
    assert!(matches!(refuse(repos), BuildError::DuplicatePage(_)));

    let mut repos = two_repositories();
    repos[1].pages[0].facts.producer.version = "0.30.0".into();
    assert!(matches!(refuse(repos), BuildError::ProducerMismatch(r) if r == "lib"));

    let mut repos = two_repositories();
    repos[1].id = "app".into();
    assert!(matches!(refuse(repos), BuildError::RepositoryId(_)));

    assert!(matches!(refuse(vec![]), BuildError::Empty));
    assert!(matches!(
        refuse(vec![repository("app", Requirement::Required, vec![])]),
        BuildError::NoPages(_)
    ));
}

// Section 3.4: bounds are applied to bytes, before any decoding.
#[test]
fn bounds_refuse_before_decoding() {
    let deep = "[".repeat(BOUNDS.depth + 1) + &"]".repeat(BOUNDS.depth + 1);
    assert!(matches!(
        context::scan(&[deep.as_bytes()], &BOUNDS),
        Err(BoundsError::Depth { page: 0, .. })
    ));
    let long = format!("\"{}\"", "x".repeat(BOUNDS.string_bytes + 1));
    assert!(matches!(
        context::scan(&[b"{}", long.as_bytes()], &BOUNDS),
        Err(BoundsError::StringBytes { page: 1, .. })
    ));
    let wide = format!("[{}0]", "0,".repeat(BOUNDS.members));
    assert!(matches!(
        context::scan(&[wide.as_bytes()], &BOUNDS),
        Err(BoundsError::Members { .. })
    ));
    let pages: Vec<&[u8]> = vec![b"{}"; BOUNDS.pages + 1];
    assert!(matches!(
        context::scan(&pages, &BOUNDS),
        Err(BoundsError::Pages { .. })
    ));
    assert!(matches!(
        context::scan(&[b"{\"a\":\"open"], &BOUNDS),
        Err(BoundsError::Malformed { page: 0 })
    ));
    // An escaped quote does not end a string; brackets inside one do not nest.
    assert!(context::scan(&[br#"{"a":"\"[[[["}"#], &BOUNDS).is_ok());

    let mut repos = two_repositories();
    repos[0].pages[0].bytes = deep.into_bytes();
    let refused = Manifest::build(
        RUN,
        1,
        Task::NotRecorded,
        &producer(),
        &repos,
        DeliveryPolicy::new(OptionalOmission::Refuse),
    )
    .unwrap_err();
    assert!(matches!(
        refused,
        BuildError::Bounds(BoundsError::Depth { .. })
    ));
}

// Section 3.4 and section 3.10 row 3: custody names exactly the retained bytes.
#[test]
fn retained_bytes_are_write_once_and_read_back_only_under_their_digest() {
    let home = tempfile::tempdir().unwrap();
    let manifest = build(1, &two_repositories(), OptionalOmission::Refuse);
    let bytes = two_repositories()[0].pages[0].bytes.clone();
    let custody = context::retain(home.path(), &bytes).unwrap();
    assert_eq!(custody, manifest.repositories[0].pages[0].custody_digest);
    assert_eq!(manifest.repositories[0].pages[0].byte_length, bytes.len());
    // Retaining again names the same bytes and replaces nothing.
    assert_eq!(context::retain(home.path(), &bytes).unwrap(), custody);
    assert_eq!(
        context::read_retained(home.path(), &custody).unwrap(),
        Some(bytes)
    );
    // Bytes that differ from their custody digest are never read back.
    std::fs::write(context::custody_path(home.path(), &custody), b"changed").unwrap();
    assert_eq!(context::read_retained(home.path(), &custody).unwrap(), None);
    assert!(context::retain(home.path(), &two_repositories()[0].pages[0].bytes).is_err());
    assert_eq!(
        context::read_retained(home.path(), &digest("never-kept")).unwrap(),
        None
    );
}

// Section 3.1: the digest names this composition and changes with any of it.
#[test]
fn the_manifest_digest_is_fixed_by_the_composition() {
    let a = build(1, &two_repositories(), OptionalOmission::Refuse);
    let b = build(1, &two_repositories(), OptionalOmission::Refuse);
    assert_eq!(a.manifest_digest, b.manifest_digest);
    assert_eq!(a.manifest_digest, a.computed_digest());
    assert_ne!(
        a.manifest_digest,
        build(1, &two_repositories(), OptionalOmission::Permit).manifest_digest
    );
    let mut repos = two_repositories();
    repos[1].pages[0].bytes.push(b' ');
    assert_ne!(
        a.manifest_digest,
        build(1, &repos, OptionalOmission::Refuse).manifest_digest
    );
    let mut repos = two_repositories();
    repos[0].pages[0].delivery = Intended::Withheld;
    assert_ne!(
        a.manifest_digest,
        build(1, &repos, OptionalOmission::Refuse).manifest_digest
    );
}

// Section 3.6: a request that does not carry the recorded manifest refuses.
#[test]
fn a_request_carrying_another_manifest_refuses() {
    let recorded = build(1, &two_repositories(), OptionalOmission::Refuse);
    let bytes = recorded.to_bytes();
    assert!(context::verify_request(&recorded, &bytes, &recorded.manifest_digest).is_ok());
    assert_eq!(
        context::verify_request(&recorded, &bytes, &digest("other")),
        Err(RequestMismatch::Digest)
    );
    let other = build(1, &two_repositories(), OptionalOmission::Permit);
    assert_eq!(
        context::verify_request(&recorded, &other.to_bytes(), &recorded.manifest_digest),
        Err(RequestMismatch::Manifest)
    );
    assert!(matches!(
        context::verify_request(&recorded, b"not json", &recorded.manifest_digest),
        Err(RequestMismatch::Unreadable(_))
    ));
}

// Sections 3.6 and 3.9: written with the intent, read back from the chain.
#[test]
fn the_intent_records_the_manifest_before_any_effect_and_the_projection_reads_it() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let manifest = build(1, &two_repositories(), OptionalOmission::Refuse);
    {
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        assert_eq!(
            begin(&mut chain, target.path(), Some(&manifest)).unwrap(),
            1
        );
    }
    let (chain, _) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(
        context::recorded_manifest(&chain, RUN, 1).unwrap(),
        Some(manifest.clone())
    );
    match context::project(&chain, RUN, 1) {
        Projection::Recorded {
            manifest: m,
            delivery,
        } => {
            assert_eq!(*m, manifest);
            assert_eq!(delivery.intended.len(), 3);
            assert!(
                delivery
                    .observed
                    .iter()
                    .all(|p| p.state == Visibility::NotReported)
            );
            assert_eq!(delivery.transport, "not-recorded");
        }
        other => panic!("expected a recorded projection, got {other:?}"),
    }
}

#[test]
fn a_manifest_for_another_attempt_refuses_before_any_record() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let manifest = build(2, &two_repositories(), OptionalOmission::Refuse);
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let refused = begin(&mut chain, target.path(), Some(&manifest)).unwrap_err();
    assert!(refused.to_string().contains("context-manifest-binding"));
    assert!(chain.entries().is_empty(), "nothing was recorded");
}

#[test]
fn a_historical_intent_renders_not_recorded() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    begin(&mut chain, target.path(), None).unwrap();
    assert_eq!(context::project(&chain, RUN, 1), Projection::NotRecorded);
    assert_eq!(context::project(&chain, RUN, 9), Projection::NotRecorded);
}

fn report(manifest: &Manifest, visible: &[&str], withheld: &[&str]) -> Observation {
    Observation::Reported(Reported {
        manifest_digest: manifest.manifest_digest.clone(),
        visible: visible.iter().map(|p| digest(p)).collect(),
        withheld: withheld
            .iter()
            .map(|p| Withheld {
                page_digest: digest(p),
                reason: "policy".into(),
            })
            .collect(),
    })
}

fn states(delivery: &context::Delivery) -> Vec<Visibility> {
    delivery.observed.iter().map(|p| p.state).collect()
}

// Section 3.5 and section 3.10 rows 5 and 6.
#[test]
fn observed_delivery_never_copies_the_intended_set() {
    use Visibility::*;
    let m = build(1, &two_repositories(), OptionalOmission::Refuse);

    let none = context::fold_delivery(&m, &[]);
    assert_eq!(states(&none), [NotReported, NotReported, NotReported]);
    assert_eq!(none.intended.len(), 3);
    assert!(
        none.intended
            .iter()
            .all(|p| p.delivery == Intended::Required)
    );

    let partial = context::fold_delivery(&m, &[report(&m, &["app-1"], &["lib-1"])]);
    assert_eq!(
        states(&partial),
        [ReportedVisible, NotReported, ReportedWithheld]
    );
    assert_eq!(partial.observed[2].reason.as_deref(), Some("policy"));
    assert!(partial.findings.is_empty());

    let mut foreign = report(&m, &["app-1", "app-2", "lib-1"], &[]);
    if let Observation::Reported(r) = &mut foreign {
        r.manifest_digest = digest("another-manifest");
    }
    let foreign = context::fold_delivery(&m, &[foreign]);
    assert_eq!(
        states(&foreign),
        [Contradictory, Contradictory, Contradictory]
    );
    assert!(
        foreign
            .findings
            .contains(&"manifest-digest-mismatch".to_string())
    );

    let both = context::fold_delivery(&m, &[report(&m, &["app-1"], &["app-1"])]);
    assert_eq!(both.observed[0].state, Contradictory);

    let unreadable = context::fold_delivery(&m, &[Observation::Unreadable("cut".into())]);
    assert_eq!(states(&unreadable), [Unreadable, Unreadable, Unreadable]);

    let stranger = context::fold_delivery(&m, &[report(&m, &["elsewhere"], &[])]);
    assert!(stranger.findings[0].starts_with("unexpected-page"));
}

// Section 3.6: duplicates are findings and never collapse favorably.
#[test]
fn duplicate_or_contradictory_observations_are_findings() {
    let m = build(1, &two_repositories(), OptionalOmission::Refuse);
    let agree = context::fold_delivery(
        &m,
        &[report(&m, &["app-1"], &[]), report(&m, &["app-1"], &[])],
    );
    assert_eq!(agree.observed[0].state, Visibility::ReportedVisible);
    assert!(agree.findings[0].starts_with("duplicate-observation"));
    let disagree = context::fold_delivery(
        &m,
        &[report(&m, &["app-1"], &[]), report(&m, &[], &["app-1"])],
    );
    assert_eq!(disagree.observed[0].state, Visibility::Contradictory);
    let half = context::fold_delivery(
        &m,
        &[
            report(&m, &["app-1"], &[]),
            Observation::Unreadable("cut".into()),
        ],
    );
    assert_eq!(half.observed[0].state, Visibility::Contradictory);
    assert_eq!(half.findings.len(), 1);
    assert!(half.findings[0].starts_with("contradictory-observation"));
}

#[test]
fn delivery_records_bind_only_their_attempt_and_manifest() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let m1 = build(1, &two_repositories(), OptionalOmission::Refuse);
    {
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        begin(&mut chain, target.path(), Some(&m1)).unwrap();
        let t = "2026-10-21T00:00:00Z";
        context::record_delivery(&mut chain, &m1, &report(&m1, &["app-1"], &[]), t).unwrap();
        // A record for the same attempt under another manifest does not bind.
        let other = build(1, &two_repositories(), OptionalOmission::Permit);
        context::record_delivery(&mut chain, &other, &report(&other, &["app-2"], &[]), t).unwrap();
    }
    let (chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let observed = context::recorded_deliveries(&chain, &m1);
    assert_eq!(observed, vec![report(&m1, &["app-1"], &[])]);
    let evidence = chain
        .entries()
        .into_iter()
        .filter(|e| e.kind == Kind::Evidence && e.subject == context::DELIVERY_SUBJECT)
        .count();
    assert_eq!(evidence, 2);
    let Projection::Recorded { delivery, .. } = context::project(&chain, RUN, 1) else {
        panic!("recorded");
    };
    assert_eq!(delivery.observed[0].state, Visibility::ReportedVisible);
    assert_eq!(delivery.observed[1].state, Visibility::NotReported);
}

// Section 3.10 row 10.
#[test]
fn a_retry_with_the_same_pages_writes_a_new_manifest_and_leaves_the_first() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let first = build(1, &two_repositories(), OptionalOmission::Refuse);
    let second = build(2, &two_repositories(), OptionalOmission::Refuse);
    assert_ne!(first.manifest_digest, second.manifest_digest);
    let page_digests = |m: &Manifest| {
        m.pages()
            .map(|(_, p)| p.page_digest.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(page_digests(&first), page_digests(&second));
    {
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        let clock = FixedClock(1_790_000_000);
        let session = session::begin_with(
            &mut chain,
            target.path(),
            RUN,
            "main",
            &clock,
            &IntentDetail {
                context_manifest: Some(&first),
                ..IntentDetail::default()
            },
        )
        .unwrap();
        session::conclude(
            &mut chain,
            target.path(),
            &session,
            statecraft_run::attempt::Outcome::Failed,
            &statecraft_run::refusal::Accounting::default(),
            serde_json::json!({}),
            &clock,
        )
        .unwrap();
        assert_eq!(begin(&mut chain, target.path(), Some(&second)).unwrap(), 2);
    }
    let (chain, _) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(
        context::recorded_manifest(&chain, RUN, 1).unwrap(),
        Some(first)
    );
    assert_eq!(
        context::recorded_manifest(&chain, RUN, 2).unwrap(),
        Some(second)
    );
}

// Section 3.8: the policy is recorded with the intent and read back exactly.
#[test]
fn the_acceptance_policy_is_recorded_with_the_intent_and_never_replaced() {
    use statecraft_run::context::{ContextPolicy, Need, POLICY_SCHEMA};
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let manifest = build(1, &two_repositories(), OptionalOmission::Refuse);
    let policy = ContextPolicy {
        schema: POLICY_SCHEMA.into(),
        delivery: Need::Required,
        integrity: Need::Required,
        closure: Need::Required,
        freshness: Need::Optional,
        citations: Need::Optional,
        evidence: Need::Required,
        permitted_absences: vec![],
    };
    {
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        session::begin_with(
            &mut chain,
            target.path(),
            RUN,
            "main",
            &FixedClock(1_790_000_000),
            &IntentDetail {
                context_manifest: Some(&manifest),
                context_policy: Some(&policy),
                ..IntentDetail::default()
            },
        )
        .unwrap();
    }
    let (chain, _) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(
        context::recorded_policy(&chain, RUN, 1).unwrap(),
        Some(policy)
    );
    assert_eq!(context::recorded_policy(&chain, RUN, 2).unwrap(), None);
}
