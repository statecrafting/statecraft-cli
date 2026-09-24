//! The real producer boundary, asserted against the real library.
//!
//! Spec 002 section 3.15. Everything here calls
//! `spec_spine_core::scaffold_init_json` in process, through
//! [`statecraft_home::producer::Library`], and asserts what it actually
//! returns. No fixture appears in this file, and nothing in it is satisfied by
//! a recorded answer.
//!
//! **The boundary is not yet satisfied, and this suite is where that is
//! recorded rather than hidden.** The producer contract restricts the answer to
//! the closed contract set of section 3.5. The pinned release this build
//! depends on still returns an `AGENTS.md` and three `.claude/rules/` files
//! alongside it, because the trimming of the producer is another repository's
//! change and has not shipped. This product refuses to place them and reports
//! the producer as non-conforming, which is the honest state of the
//! integration. When the trimmed producer releases, the pin moves and
//! [`the_producer_is_not_yet_conforming`] is the test that changes.

mod support;

use statecraft_home::producer::{self, Library, Placement, Producer};

/// Every path the contract set admits, as the spec lists them.
const CONTRACT_SET: [&str; 6] = [
    "spec-spine.toml",
    "standards/spec/constitution.md",
    "standards/spec/contract.md",
    "standards/spec/templates/spec-template.md",
    "standards/spec/templates/constitution-template.md",
    "specs/000-bootstrap/spec.md",
];

#[test]
fn the_library_is_called_in_process_and_answers() {
    let answer = Library
        .scaffold(&producer::config_json())
        .expect("the real library answers");
    assert!(
        answer.contains("\"files\""),
        "the answer is the files-as-data shape"
    );
    // The exact producer identity this build depends on, recorded rather than
    // described.
    assert_eq!(Library.identity().name, "spec-spine-core");
    assert_eq!(Library.identity().version, producer::PRODUCER_VERSION);
}

#[test]
fn the_declared_layout_reaches_the_producer_and_shapes_what_it_returns() {
    let starter = producer::produce(&Library).expect("the real library answers");
    let toml = starter
        .governance
        .iter()
        .find(|f| f.rel_path == "spec-spine.toml")
        .expect("the configuration is in contract");
    assert!(toml.contents.contains(producer::DERIVED_DIR));
    assert!(toml.contents.contains(producer::STATE_DIR));
    assert!(
        !toml.contents.contains("derived_dir   = \".derived\""),
        "the default location survived the explicit layout"
    );

    let fragment = starter
        .ignore_fragment
        .expect("the producer returns an ignore fragment");
    assert!(fragment.contains(producer::DERIVED_DIR));
    assert!(fragment.contains(producer::STATE_DIR));
    // And it excludes only the state root, never the whole area.
    assert!(statecraft_home::ignore::area_ignored(&fragment).is_none());
}

/// Spec 002 section 5, 2026-09-24, provenance item 5: `[index]` is passed
/// explicitly, so the derived directory the project declares is the only one
/// its resolver excludes, and the producer's own default is not in the list.
#[test]
fn the_resolver_exclusions_come_from_the_declared_layout_and_name_no_other_derived_directory() {
    let starter = producer::produce(&Library).expect("the real library answers");
    let toml = starter
        .governance
        .iter()
        .find(|f| f.rel_path == "spec-spine.toml")
        .expect("the configuration is in contract");
    let line = toml
        .contents
        .lines()
        .find(|l| l.trim_start().starts_with("resolver_exclusions"))
        .unwrap_or_else(|| panic!("no resolver_exclusions line in:\n{}", toml.contents));
    let listed: Vec<String> = line
        .split('[')
        .nth(1)
        .and_then(|r| r.split(']').next())
        .expect("a list")
        .split(',')
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .collect();
    let mut expected: Vec<String> = producer::resolver_exclusions()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut got = listed.clone();
    expected.sort();
    got.sort();
    assert_eq!(got, expected, "the rendered line: {line}");
    assert!(
        !listed.iter().any(|v| v == ".derived"),
        "the producer default derived directory is excluded: {line}"
    );
    assert!(listed.iter().any(|v| v == producer::DERIVED_DIR));
}

#[test]
fn every_in_contract_path_the_producer_returns_is_one_the_contract_set_admits() {
    let starter = producer::produce(&Library).expect("the real library answers");
    for file in &starter.governance {
        assert!(
            CONTRACT_SET.contains(&file.rel_path.as_str()),
            "{} is placed and is not in the contract set",
            file.rel_path
        );
        assert_eq!(producer::classify(&file.rel_path), Placement::Governance);
    }
    // And the producer still returns the whole set, so nothing this product
    // needs is silently absent.
    for expected in CONTRACT_SET {
        assert!(
            starter.governance.iter().any(|f| f.rel_path == expected),
            "the producer returned no {expected}"
        );
    }
}

/// The state of the integration, measured rather than described.
///
/// Inverted when the published `spec-spine-core` 0.23.0 was adopted (spec 002
/// section 5, 2026-09-23): it returns only in-contract paths, so the boundary
/// is satisfied. Against the released 0.21.0 this asserted non-conformance and
/// named the four out-of-contract paths; that measurement stays in spec 002's
/// section 5 and its handoff, section 8.
#[test]
fn the_producer_is_conforming() {
    let starter = producer::produce(&Library).expect("the real library answers");
    assert!(
        starter.conformance.conforming,
        "the producer is not conforming: {}",
        starter.conformance.describe()
    );
    assert!(starter.conformance.out_of_contract.is_empty());
    assert!(starter.out_of_contract.is_empty());
    // A path outside the contract is still classified as one, and would
    // still never be placed.
    assert_eq!(producer::classify("AGENTS.md"), Placement::OutOfContract);
}

/// The whole boundary, end to end, against the real library and the real
/// `spec-spine` binary.
///
/// The real library is non-conforming today, so this initialization is
/// `partial` and names why. Every in-contract path still reconciles, the
/// corpus still compiles at the new derived path, and the project is still
/// registered and qualified: a non-conforming producer is a finding about the
/// producer, not a failure of the project.
#[test]
fn the_real_library_end_to_end_is_complete_under_a_conforming_producer() {
    use statecraft_home::flow::{Outcome, Step};
    use statecraft_home::service::Operation;
    use support::{FixedClock, Harness, Sandbox};

    let sandbox = Sandbox::new();
    let corpus = support::corpus_tool();
    let probe = statecraft_environment::probe::CommandProbe {
        git: "git".into(),
        spec_spine: support::spec_spine_program(),
    };
    let authority = statecraft_home::team::Unreachable::default();
    let revisions = statecraft_home::authority::StaticRevision::default();
    let h = Harness {
        layout: sandbox.layout(),
        producer: &Library,
        corpus: &corpus,
        probe: &probe,
        authority: &authority,
        revisions: &revisions,
        clock: FixedClock(1_760_000_000),
        native_root: sandbox.native_root(),
    };

    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = support::init_report(&answer);

    assert_eq!(
        report.outcome,
        Outcome::Complete,
        "a conforming producer leaves nothing withheld: {:#?}",
        report.steps
    );
    let governance = report
        .steps
        .iter()
        .find(|s| s.step == Step::Governance)
        .expect("the governance step ran");
    assert!(
        !format!("{:?}", governance.state).contains("non-conforming"),
        "{governance:?}"
    );

    // Nothing out of contract was written.
    assert!(!statecraft_environment::claimant::resolve(&sandbox.project(), ".claude").exists());
    // Everything in contract was, and the corpus compiles at the new path.
    for expected in CONTRACT_SET {
        assert!(
            statecraft_environment::claimant::resolve(&sandbox.project(), expected).is_file(),
            "{expected} was not placed"
        );
    }
    assert!(
        statecraft_environment::claimant::resolve(
            &sandbox.project(),
            ".statecraft/derived/spec-registry"
        )
        .is_dir()
    );
    let corpus_step = report
        .steps
        .iter()
        .find(|s| s.step == Step::Corpus)
        .expect("the corpus step ran");
    assert!(corpus_step.state.done(), "{corpus_step:?}");
    let register = report
        .steps
        .iter()
        .find(|s| s.step == Step::Register)
        .expect("the register step ran");
    assert!(register.state.done(), "{register:?}");
}
