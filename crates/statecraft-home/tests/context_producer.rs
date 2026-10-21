//! Spec 019 section 3.3: the adopted producer's packet reads, run against
//! the real pinned executable over a clone of this repository's own corpus.
//! No provider runs; the only process is the producer.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use statecraft_home::context::{self, Closure, Decoded, Member, Source};
use statecraft_run::context::{
    AcceptedRepository, DeliveryPolicy, Intended, Manifest, OptionalOmission, Producer,
    Requirement, Task,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn spec_spine() -> PathBuf {
    let local = repo_root().join(".bin/spec-spine");
    if local.is_file() {
        return local;
    }
    if let Some(installer) = std::env::var_os("STATECRAFT_INSTALL")
        && let Ok(script) = std::fs::read_to_string(installer)
        && let Some(value) = script
            .lines()
            .find_map(|line| line.trim().strip_prefix("bin="))
    {
        let declared = Path::new(value.trim_matches(['\'', '"']));
        if declared.is_relative() {
            let installed = repo_root().join(declared);
            if installed.is_file() {
                return installed;
            }
        }
    }
    panic!("no .bin/spec-spine; run `make tools`. This suite runs the real pinned tool.");
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A clean clone of this repository's committed corpus at HEAD.
fn corpus() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("file://{}", repo_root().display());
    let out = Command::new("git")
        .args(["clone", "--quiet", "--depth", "1", &url])
        .arg(dir.path().join("corpus"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let repo = dir.path().join("corpus");
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["config", "user.name", "test"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    dir
}

fn request(max_items: usize) -> Value {
    json!({
        "closure": { "root": "019-context-packet-consumption" },
        "members": [
            { "selector": { "kind": "spec", "spec": "003-work-and-run-semantics" }, "requirement": "required" },
            { "selector": { "kind": "spec", "spec": "004-execution-adapter" }, "requirement": "required" },
            { "selector": { "kind": "spec", "spec": "999-absent" }, "requirement": "optional" }
        ],
        "maxItems": max_items,
        "consumerSchemaVersion": "1.1.0"
    })
}

fn assemble(repo: &Path, revision: Option<&str>) -> Vec<statecraft_run::context::AcceptedPage> {
    let program = spec_spine();
    context::assemble(
        &Source {
            program: &program,
            repo,
            repository: "example/statecraft",
            revision,
        },
        &request(1),
        Intended::Required,
    )
    .unwrap()
}

#[test]
fn a_paged_packet_is_assembled_to_its_terminal_page_and_composes_a_manifest() {
    let dir = corpus();
    let repo = dir.path().join("corpus");
    let pages = assemble(&repo, None);
    assert!(pages.len() >= 2, "a one-item budget pages the request");
    for (i, page) in pages.iter().enumerate() {
        assert_eq!(page.facts.continues, i + 1 < pages.len());
        assert_eq!(page.facts.snapshot.tree, pages[0].facts.snapshot.tree);
        assert_eq!(page.source_operation, context::SOURCE_OPERATION);
        assert!(matches!(context::decode(&page.bytes), Decoded::Valid(_)));
    }
    // The optional absent member is the producer's omission, carried as such.
    let omitted: Vec<_> = pages.iter().flat_map(|p| &p.facts.omissions).collect();
    assert!(
        omitted
            .iter()
            .any(|o| o.requirement == Requirement::Optional),
        "{omitted:?}"
    );
    let head = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(pages[0].facts.snapshot.revision, head);
    assert_eq!(pages[0].facts.producer.version, "0.31.0");

    let manifest = Manifest::build(
        "run-019",
        1,
        Task::NotRecorded,
        &Producer {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            packet_interface: "1.1.0".into(),
        },
        &[AcceptedRepository {
            id: "statecraft".into(),
            role: "source".into(),
            requirement: Requirement::Required,
            pages,
        }],
        DeliveryPolicy::new(OptionalOmission::Permit),
    )
    .unwrap();
    assert_eq!(manifest.repositories[0].snapshot.revision, head);
}

#[test]
fn a_page_whose_digest_does_not_reproduce_is_invalid_and_another_schema_unsupported() {
    let dir = corpus();
    let pages = assemble(&dir.path().join("corpus"), None);
    let mut value: Value = serde_json::from_slice(&pages[0].bytes).unwrap();
    value["requestDigest"] = json!(format!("sha256:{}", "0".repeat(64)));
    let tampered = serde_json::to_vec(&value).unwrap();
    assert!(matches!(context::decode(&tampered), Decoded::Invalid(_)));
    value["schemaVersion"] = json!("9.0.0");
    assert!(matches!(
        context::decode(&serde_json::to_vec(&value).unwrap()),
        Decoded::Unsupported(_)
    ));
    value["unexpected"] = json!(true);
    value["schemaVersion"] = json!("1.1.0");
    assert!(matches!(
        context::decode(&serde_json::to_vec(&value).unwrap()),
        Decoded::Unreadable(_)
    ));
    assert!(matches!(context::decode(b"[[[["), Decoded::Unreadable(_)));
}

#[test]
fn the_recorded_closure_is_re_resolved_at_its_recorded_revision() {
    let dir = corpus();
    let repo = dir.path().join("corpus");
    let pages = assemble(&repo, None);
    let program = spec_spine();
    // A later commit does not move the recorded revision's answer.
    std::fs::write(repo.join("README.md"), "changed\n").unwrap();
    git(&repo, &["commit", "--quiet", "-am", "later"]);
    match context::closure(&program, &repo, &pages[0].bytes) {
        Closure::Answered {
            digest,
            differences,
        } => {
            assert_eq!(digest, pages[0].facts.closure_digest);
            assert!(differences.is_empty());
        }
        other => panic!("expected an answer, got {other:?}"),
    }
}

#[test]
fn a_closure_whose_revision_is_gone_is_refused_by_the_producer() {
    let dir = corpus();
    let pages = assemble(&dir.path().join("corpus"), None);
    let program = spec_spine();
    // Another repository that never held the recorded revision.
    let other = tempfile::tempdir().unwrap();
    git(other.path(), &["init", "--quiet"]);
    match context::closure(&program, other.path(), &pages[0].bytes) {
        Closure::Refused(r) => assert_ne!(r.kind, "", "{r}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn the_current_snapshot_is_bound_by_the_producer_and_a_dirty_tree_is_unverified() {
    let dir = corpus();
    let repo = dir.path().join("corpus");
    let pages = assemble(&repo, None);
    let program = spec_spine();
    let now = context::current(&program, &repo, "example/statecraft", &pages[0].bytes).unwrap();
    assert_eq!(now, pages[0].facts.snapshot);

    std::fs::write(repo.join("README.md"), "dirty\n").unwrap();
    let dirty =
        context::current(&program, &repo, "example/statecraft", &pages[0].bytes).unwrap_err();
    assert!(dirty.contains("dirty"), "{dirty}");

    git(&repo, &["commit", "--quiet", "-am", "moved"]);
    let moved = context::current(&program, &repo, "example/statecraft", &pages[0].bytes).unwrap();
    assert_ne!(moved.tree, pages[0].facts.snapshot.tree);
}

#[test]
fn a_member_is_found_only_under_its_own_identity() {
    let dir = corpus();
    let pages = assemble(&dir.path().join("corpus"), None);
    let carried = pages
        .iter()
        .find_map(|p| {
            let v: Value = serde_json::from_slice(&p.bytes).unwrap();
            v["members"][0]["identity"]
                .as_str()
                .map(|id| (p.bytes.clone(), id.to_string()))
        })
        .unwrap();
    assert!(matches!(
        context::member(&carried.0, &carried.1),
        Member::Found { .. }
    ));
    assert_eq!(context::member(&carried.0, "spec:nowhere"), Member::Absent);
    let omitted = pages
        .iter()
        .find(|p| !p.facts.omissions.is_empty())
        .unwrap();
    assert_eq!(
        context::member(&omitted.bytes, &omitted.facts.omissions[0].identity),
        Member::Omitted
    );
}

// An incomplete page is the producer's finding, not a failure: its page is
// the verdict's report, retained as the producer's canonical document.
#[test]
fn an_incomplete_page_is_assembled_from_the_producers_finding() {
    let dir = corpus();
    let program = spec_spine();
    let mut incomplete = request(10);
    incomplete["members"][2]["requirement"] = json!("required");
    let pages = context::assemble(
        &Source {
            program: &program,
            repo: &dir.path().join("corpus"),
            repository: "example/statecraft",
            revision: None,
        },
        &incomplete,
        Intended::Required,
    )
    .unwrap();
    let last = pages.last().unwrap();
    assert_eq!(
        last.facts.completeness,
        statecraft_run::context::Completeness::Incomplete
    );
    assert!(
        last.facts
            .omissions
            .iter()
            .any(|o| o.requirement == Requirement::Required)
    );
    assert!(matches!(context::decode(&last.bytes), Decoded::Valid(_)));
    let reparsed: Value = serde_json::from_slice(&last.bytes).unwrap();
    let canonical = serde_json::to_string_pretty(&reparsed).unwrap() + "\n";
    assert_eq!(last.bytes, canonical.into_bytes());
}

// A tampered page never drives a producer call: closure and current refuse a
// page whose digest does not reproduce, as member does.
#[test]
fn a_tampered_page_drives_no_producer_call() {
    let dir = corpus();
    let repo = dir.path().join("corpus");
    let pages = assemble(&repo, None);
    let program = spec_spine();
    let mut value: Value = serde_json::from_slice(&pages[0].bytes).unwrap();
    value["snapshot"]["revision"] = json!("0".repeat(40));
    let tampered = serde_json::to_vec(&value).unwrap();
    match context::closure(&program, &repo, &tampered) {
        Closure::Refused(r) => assert_eq!(r.kind, "unreadable", "{r}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    let why = context::current(&program, &repo, "example/statecraft", &tampered).unwrap_err();
    assert!(why.contains("Invalid"), "{why}");
    assert!(matches!(
        context::member(&tampered, "spec:anything"),
        Member::Unreadable(_)
    ));
}
