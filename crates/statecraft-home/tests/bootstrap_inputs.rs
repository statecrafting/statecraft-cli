//! Spec 018 section 3.3 at the flow: a `spec-spine.toml` initialization would
//! create carries the producer's exact pin of its own identity, and a
//! producer whose pin disagrees with that identity is refused before every
//! write. The producer here is a recorded answer, because the case is a
//! producer misbehaving; that the real library pins itself is asserted in
//! `producer::tests::the_real_library_pins_its_own_version_exactly`.

mod support;

use statecraft_home::flow::{Outcome, Step, StepState};
use statecraft_home::producer::{Identity, Recorded};
use statecraft_home::service::Operation;
use statecraft_home::team::Unreachable;
use statecraft_home::{authority::StaticRevision, producer};
use support::{FixedClock, Harness, Sandbox, StatedProbe, conforming_producer, init_report};

/// Every path under the project outside `.git`.
fn tree(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .display()
                .to_string();
            if rel == ".git" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                stack.push(entry.path());
            } else {
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

/// The conforming answer, claimed by a producer of another version.
fn misidentified(version: &str) -> Recorded {
    Recorded {
        identity: Identity {
            name: "spec-spine-core (misidentified, a test fixture)".into(),
            version: version.into(),
        },
        ..conforming_producer()
    }
}

fn init_apply(sandbox: &Sandbox, producer: &Recorded) -> statecraft_home::flow::Report {
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = Harness {
        layout: sandbox.layout(),
        producer,
        corpus: &corpus,
        probe: &probe,
        authority: &authority,
        revisions: &revisions,
        clock: FixedClock(1_760_000_000),
        native_root: sandbox.native_root(),
    };
    init_report(&h.execute(Operation::InitApply {
        root: sandbox.project(),
    }))
    .clone()
}

#[test]
fn a_producer_pin_other_than_its_identity_is_refused_before_every_write() {
    let sandbox = Sandbox::new();
    let before = tree(&sandbox.project());
    let report = init_apply(&sandbox, &misidentified("9.9.9"));

    assert_eq!(report.outcome, Outcome::Refused, "{}", report.render());
    let plan = report.steps.last().unwrap();
    assert_eq!(plan.step, Step::Plan);
    let StepState::Refused { reason } = &plan.state else {
        panic!("{plan:?}");
    };
    assert!(
        reason.contains(&format!("={}", producer::PRODUCER_VERSION)) && reason.contains("@9.9.9"),
        "the returned pin and the identity are named: {reason}"
    );
    // Nothing but the manifest lock: no governance file, no declaration.
    let after: Vec<String> = tree(&sandbox.project())
        .into_iter()
        .filter(|p| p != statecraft_environment::manifest::LOCK_PATH)
        .collect();
    assert_eq!(after, before, "refused before every write");
    assert!(!sandbox.exists("spec-spine.toml"));
}

#[test]
fn an_adopted_configuration_is_not_the_producer_s_to_pin() {
    let sandbox = Sandbox::new();
    let adopted = "[meta]\n# no pin: the project's own file\n";
    sandbox.write("spec-spine.toml", adopted);
    let report = init_apply(&sandbox, &misidentified("9.9.9"));

    assert_ne!(report.outcome, Outcome::Refused, "{}", report.render());
    assert_eq!(
        sandbox.read("spec-spine.toml").as_deref(),
        Some(adopted),
        "an adopted file is never rewritten"
    );
}

#[test]
fn a_conforming_producer_scaffolds_its_exact_pin() {
    let sandbox = Sandbox::new();
    let report = init_apply(&sandbox, &conforming_producer());
    assert_ne!(report.outcome, Outcome::Refused, "{}", report.render());
    let toml = sandbox.read("spec-spine.toml").unwrap();
    assert_eq!(
        statecraft_home::setup::exact_pin(&toml).unwrap(),
        producer::PRODUCER_VERSION
    );
}
