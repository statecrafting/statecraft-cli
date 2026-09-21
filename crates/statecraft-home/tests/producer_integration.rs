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
/// This test asserts the boundary is **not** yet satisfied. It is the test that
/// changes when the trimmed producer releases: at that point the assertion
/// inverts to `conforming`, and nothing else in this repository has to move
/// except the version pin.
#[test]
fn the_producer_is_not_yet_conforming() {
    let starter = producer::produce(&Library).expect("the real library answers");
    assert!(
        !starter.conformance.conforming,
        "the producer is now conforming: invert this assertion, and record the \
         release that made it true"
    );
    assert_eq!(
        starter.conformance.out_of_contract,
        [
            ".claude/rules/orchestrator-rules.md",
            ".claude/rules/governed-artifact-reads.md",
            ".claude/rules/adversarial-prompt-refusal.md",
            "AGENTS.md",
        ],
        "the out-of-contract set is not the one this build measured; \
         re-measure it and say what changed"
    );
    assert!(starter.conformance.describe().contains("non-conforming"));
}

#[test]
fn an_out_of_contract_path_is_carried_so_it_can_be_recognized_and_never_placed() {
    let starter = producer::produce(&Library).expect("the real library answers");
    let generated = starter
        .out_of_contract
        .iter()
        .find(|f| f.rel_path == "AGENTS.md")
        .expect("this producer still returns one");
    // Its bytes are what lets the bridge classify an untouched generated root
    // file as generated rather than as the user's own. They are carried in
    // memory and written nowhere.
    assert!(!generated.contents.is_empty());
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
fn the_real_library_end_to_end_is_partial_and_names_the_producer_as_the_reason() {
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
        Outcome::Partial,
        "the real producer is non-conforming, so a complete outcome would be a \
         claim this boundary does not support: {:#?}",
        report.steps
    );
    let governance = report
        .steps
        .iter()
        .find(|s| s.step == Step::Governance)
        .expect("the governance step ran");
    assert!(
        format!("{:?}", governance.state).contains("non-conforming"),
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
