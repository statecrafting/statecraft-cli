//! Spec 011: initialization never scaffolds a second `000` spec and never
//! writes an approved one.
//!
//! Each test runs `init apply` against a temporary home and repository, with
//! the real corpus tool and the real library's answer restricted to the
//! contract set (`support::conforming_producer`), as `negative_cases.rs` does.

mod support;

use statecraft_environment::manifest::Manifest;
use statecraft_home::authority::StaticRevision;
use statecraft_home::flow::{Outcome, Report};
use statecraft_home::service::{Operation, Severity};
use statecraft_home::team::Unreachable;
use support::{FixedClock, Harness, Sandbox, conforming_producer, init_report};

const CLOCK: FixedClock = FixedClock(1_760_000_000);
const BOOTSTRAP: &str = "specs/000-bootstrap/spec.md";
const OTHER: &str = "specs/000-rahi-bootstrap/spec.md";
const OTHER_TEXT: &str = "---\nid: \"000-rahi-bootstrap\"\ntitle: \"Rahi bootstrap\"\nstatus: approved\nimplementation: n-a\ncreated: \"2026-09-01\"\nsummary: >\n  The corpus contract of a repository with its own bootstrap spec.\n---\n\n# 000: Rahi bootstrap\n";

/// `init apply` once, with the real corpus tool; the report and whether the
/// answer was ok.
fn apply(sandbox: &Sandbox) -> (Report, Severity) {
    let producer = conforming_producer();
    let corpus = support::corpus_tool();
    let probe = statecraft_environment::probe::CommandProbe {
        git: "git".into(),
        spec_spine: support::spec_spine_program(),
    };
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = Harness {
        layout: sandbox.layout(),
        producer: &producer,
        corpus: &corpus,
        probe: &probe,
        authority: &authority,
        revisions: &revisions,
        clock: CLOCK,
        native_root: sandbox.native_root(),
    };
    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    (init_report(&answer).clone(), answer.severity())
}

/// The frontmatter's `status` value, comments stripped.
fn status(text: &str) -> String {
    text.split("\n---")
        .next()
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("status:"))
        .map(|v| v.split('#').next().unwrap().trim().to_string())
        .expect("a status line in the frontmatter")
}

/// Section 3.2: a bootstrap spec this product writes is a draft, whatever the
/// producer's bytes say.
#[test]
fn a_scaffolded_bootstrap_spec_is_a_draft() {
    let produced = support::conforming_producer();
    let starter = statecraft_home::producer::produce(&produced).unwrap();
    let from_producer = &starter
        .governance
        .iter()
        .find(|f| f.rel_path == BOOTSTRAP)
        .expect("the producer returns the bootstrap spec")
        .contents;
    assert_eq!(
        status(from_producer),
        "approved",
        "the linked producer's bytes still say approved; if this fails, section 3.2 has nothing to do"
    );

    let sandbox = Sandbox::new();
    let (report, severity) = apply(&sandbox);
    assert_eq!(report.outcome, Outcome::Complete, "{:#?}", report.steps);
    assert_eq!(severity, Severity::Ok);
    let written = sandbox
        .read(BOOTSTRAP)
        .expect("the bootstrap spec is written");
    assert_eq!(status(&written), "draft", "{written}");
    assert!(!written.contains("status: approved"), "{written}");
    assert!(written.contains("never a tool's"), "{written}");
    // Only the status line and the comment above it differ.
    let rest = |t: &str| {
        t.lines()
            .filter(|l| !l.starts_with("status:") && !l.starts_with("# "))
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(rest(&written), rest(from_producer));

    // A repeat writes nothing and leaves it a draft.
    let (again, _) = apply(&sandbox);
    assert_eq!(again.outcome, Outcome::Complete, "{:#?}", again.steps);
    assert!(
        !again.writes.contains(&BOOTSTRAP.to_string()),
        "{:?}",
        again.writes
    );
    assert_eq!(sandbox.read(BOOTSTRAP).unwrap(), written);
}

/// Section 3.1: a corpus that already has a `000` spec gets no second one,
/// the report says why, and nothing about it is partial.
#[test]
fn a_corpus_with_its_own_000_spec_gets_no_bootstrap_spec() {
    let sandbox = Sandbox::new();
    sandbox.write(OTHER, OTHER_TEXT);
    let (report, severity) = apply(&sandbox);
    assert_eq!(report.outcome, Outcome::Complete, "{:#?}", report.steps);
    assert_eq!(severity, Severity::Ok);
    assert!(!sandbox.exists(BOOTSTRAP));
    assert!(
        !report.writes.contains(&BOOTSTRAP.to_string()),
        "{:?}",
        report.writes
    );
    assert!(
        report
            .kept
            .iter()
            .any(|k| k.starts_with(BOOTSTRAP) && k.contains("not scaffolded") && k.contains(OTHER)),
        "{:?}",
        report.kept
    );
    let manifest = Manifest::read(&sandbox.project()).unwrap().unwrap();
    assert!(!manifest.records(BOOTSTRAP));
    assert_eq!(sandbox.read(OTHER).unwrap(), OTHER_TEXT, "never touched");

    let (again, _) = apply(&sandbox);
    assert_eq!(again.outcome, Outcome::Complete, "{:#?}", again.steps);
    assert!(!sandbox.exists(BOOTSTRAP));
}

/// Section 3.3: an earlier initialization recorded the bootstrap spec beside
/// another `000` spec and the file was never kept; the stale record is
/// dropped, so the apply is complete rather than partial.
#[test]
fn a_stale_record_of_an_uncommitted_bootstrap_spec_is_removed() {
    let sandbox = Sandbox::new();
    let (first, _) = apply(&sandbox);
    assert_eq!(first.outcome, Outcome::Complete, "{:#?}", first.steps);
    let manifest = Manifest::read(&sandbox.project()).unwrap().unwrap();
    assert!(manifest.records(BOOTSTRAP));

    // As hiqlite #47 and rahi #90 left it: the file deleted, the record kept,
    // and the corpus's own 000 spec beside it.
    std::fs::remove_file(sandbox.project().join(BOOTSTRAP)).unwrap();
    sandbox.write(OTHER, OTHER_TEXT);
    let (report, severity) = apply(&sandbox);
    assert_eq!(report.outcome, Outcome::Complete, "{:#?}", report.steps);
    assert_eq!(severity, Severity::Ok);
    assert!(!sandbox.exists(BOOTSTRAP), "not restored");
    assert!(
        report
            .kept
            .iter()
            .any(|k| k.starts_with(BOOTSTRAP) && k.contains("managed record is removed")),
        "{:?}",
        report.kept
    );
    let manifest = Manifest::read(&sandbox.project()).unwrap().unwrap();
    assert!(!manifest.records(BOOTSTRAP));
}

/// Section 3.1, the other side: an existing `000-bootstrap` spec is adopted
/// as before and never rewritten, whatever its status.
#[test]
fn an_existing_bootstrap_spec_is_adopted_and_never_rewritten() {
    let sandbox = Sandbox::new();
    let mine =
        "---\nid: \"000-bootstrap\"\nstatus: approved\n---\n\n# mine, ratified by its owner\n";
    sandbox.write(BOOTSTRAP, mine);
    let (report, _) = apply(&sandbox);
    assert!(
        report.adopted.contains(&BOOTSTRAP.to_string()),
        "{:?}",
        report.adopted
    );
    assert_eq!(sandbox.read(BOOTSTRAP).unwrap(), mine);
}
