//! What admits a live observation, and the routes that must not bypass it.
//!
//! Spec 002 section 3.29. Every test here is a **negative control** on the
//! admission itself: it supplies evidence that is wrong in exactly one way and
//! asserts the claim is refused, plus the one positive case that everything
//! else is a mutation of.
//!
//! The first test is the transcript that defeated the previous implementation,
//! quoted in section 3.29. It was written and run before the repair, where it
//! failed with `admit` returning `Ok(Observed { .. })`.

/// The one body of evidence, shared with this crate's unit tests.
#[path = "support/evidence.rs"]
#[allow(dead_code)]
mod evidence;

use statecraft_home::admission::{self, Capture, Control, NotAdmitted};
use statecraft_home::startup::{self, Observation};

/// The favourable case, so every refusal below is one mutation away from a
/// claim that is admitted rather than from one that was never plausible.
#[test]
fn the_three_controls_together_admit_the_observation() {
    let e = evidence::admissible();
    assert!(admission::admit(&e).is_ok(), "{:?}", admission::admit(&e));
    let observation = startup::admit(&e).unwrap();
    assert!(observation.observed());
    assert!(observation.admitted().is_ok());
}

/// Section 3.29's named defect.
///
/// The transcript contains the command and the word `permission`, which is what
/// the previous predicate asked for, and states that the command **succeeded**.
#[test]
fn provider_prose_claiming_permission_was_granted_is_not_a_refusal() {
    let prose = "cargo publish --dry-run: permission granted; command executed successfully\n";
    let mut e = evidence::admissible();
    e.refusal.capture = Capture {
        source: "b1.txt".into(),
        bytes: prose.to_string(),
    };
    let refused = admission::admit(&e).unwrap_err();
    assert!(
        matches!(refused, NotAdmitted::Unreadable { .. }),
        "prose was read as evidence of enforcement: {refused:?}"
    );
    assert!(startup::admit(&e).is_err());
}

/// The same sentence with every word the old predicate looked for, and still
/// no structured record behind it.
#[test]
fn a_capture_that_parses_but_carries_no_structured_denial_is_refused() {
    let mut e = evidence::admissible();
    // Well-formed harness output in which the command ran and nothing denied it.
    e.refusal.capture = evidence::capture("b1.jsonl", &[evidence::REFUSED], &[]);
    let refused = admission::admit(&e).unwrap_err();
    assert!(
        matches!(refused, NotAdmitted::NoStructuredRefusal { .. }),
        "{refused:?}"
    );
    assert!(
        refused
            .to_string()
            .contains("has said so, and nothing more")
    );
}

/// A denial for some other command is not a denial for this one.
#[test]
fn a_structured_denial_naming_another_command_does_not_qualify_this_one() {
    let mut e = evidence::admissible();
    e.refusal.capture = evidence::capture("b1.jsonl", &[evidence::REFUSED], &["npm publish"]);
    assert!(matches!(
        admission::admit(&e).unwrap_err(),
        NotAdmitted::NoStructuredRefusal { found: 1, .. }
    ));
}

/// Rule 2: the positive control is required, and it has to have behaved.
#[test]
fn the_allowed_command_control_is_enforced_by_the_boundary() {
    // It ran, and it was refused: the payload refuses things off the floor, so
    // the refusal says nothing about the floor.
    let mut refused_everything = evidence::admissible();
    refused_everything.allowed.capture =
        evidence::capture("b3.jsonl", &[evidence::ALLOWED], &[evidence::ALLOWED]);
    assert!(matches!(
        admission::admit(&refused_everything).unwrap_err(),
        NotAdmitted::ControlRefused {
            control: "allowed-command",
            ..
        }
    ));

    // It never ran, so it measured nothing.
    let mut never_ran = evidence::admissible();
    never_ran.allowed.capture = evidence::capture("b3.jsonl", &[], &[]);
    assert!(matches!(
        admission::admit(&never_ran).unwrap_err(),
        NotAdmitted::ControlNotAttempted {
            control: "allowed-command",
            ..
        }
    ));

    // It named a command the floor claims, so it could not have demonstrated
    // what it exists to demonstrate.
    let mut on_the_floor = evidence::admissible();
    on_the_floor.allowed_command = "npm publish".into();
    assert!(matches!(
        admission::admit(&on_the_floor).unwrap_err(),
        NotAdmitted::ControlOnTheFloor { .. }
    ));
}

/// Rule 2: the absent-payload control is required, and it has to have behaved.
#[test]
fn the_no_payload_control_is_enforced_by_the_boundary() {
    // Refused without the payload too: the refusal is evidence for the
    // operator's own configuration.
    let mut refused_anyway = evidence::admissible();
    refused_anyway.without_payload.capture =
        evidence::capture("b4.jsonl", &[evidence::REFUSED], &[evidence::REFUSED]);
    let err = admission::admit(&refused_anyway).unwrap_err();
    assert!(
        matches!(
            err,
            NotAdmitted::ControlRefused {
                control: "without-payload",
                ..
            }
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("operator's own configuration"));

    // Never attempted.
    let mut never_ran = evidence::admissible();
    never_ran.without_payload.capture = evidence::capture("b4.jsonl", &[], &[]);
    assert!(matches!(
        admission::admit(&never_ran).unwrap_err(),
        NotAdmitted::ControlNotAttempted {
            control: "without-payload",
            ..
        }
    ));

    // Run WITH the payload, which makes it a second refusal control and not a
    // control at all.
    let mut carried_the_payload = evidence::admissible();
    carried_the_payload.without_payload = evidence::measurement(
        evidence::REFUSED,
        true,
        evidence::capture("b4.jsonl", &[evidence::REFUSED], &[]),
    );
    assert!(matches!(
        admission::admit(&carried_the_payload).unwrap_err(),
        NotAdmitted::PayloadInTheControl { .. }
    ));
}

/// Rule 3: one capture is one measurement.
#[test]
fn two_controls_presenting_the_same_capture_are_substituted_evidence() {
    let mut e = evidence::admissible();
    e.allowed.capture = e.refusal.capture.clone();
    assert!(matches!(
        admission::admit(&e).unwrap_err(),
        NotAdmitted::SubstitutedEvidence { .. }
    ));
}

/// Rule 3: an absent or unfinished capture refuses, and is not a weaker pass.
#[test]
fn missing_and_unfinished_captures_refuse_the_claim() {
    let mut empty = evidence::admissible();
    empty.refusal.capture = Capture {
        source: "b1.jsonl".into(),
        bytes: "   \n".into(),
    };
    assert!(matches!(
        admission::admit(&empty).unwrap_err(),
        NotAdmitted::EmptyCapture { .. }
    ));

    // Init and an attempt, and the session never reached its terminal event,
    // so the refusal record it would have carried does not exist.
    let full = evidence::capture("b1.jsonl", &[evidence::REFUSED], &[evidence::REFUSED]);
    let truncated: Vec<&str> = full
        .bytes
        .lines()
        .filter(|l| !l.contains("\"type\":\"result\""))
        .collect();
    let mut unfinished = evidence::admissible();
    unfinished.refusal.capture = Capture {
        source: "b1.jsonl".into(),
        bytes: format!("{}\n", truncated.join("\n")),
    };
    assert!(matches!(
        admission::admit(&unfinished).unwrap_err(),
        NotAdmitted::NoTerminalResult { .. }
    ));
}

/// Rule 4: the observation is bound to its invocation, version and payload.
#[test]
fn evidence_for_another_invocation_or_payload_cannot_qualify_this_one() {
    // Another payload.
    let mut other_payload = evidence::admissible();
    other_payload.payload_digest = "f".repeat(64);
    assert!(matches!(
        admission::admit(&other_payload).unwrap_err(),
        NotAdmitted::PayloadMismatch { .. }
    ));

    // The claim names this build's payload and the control was handed other
    // bytes.
    let mut other_settings = evidence::admissible();
    other_settings.refusal.settings = Some("{\"permissions\":{\"deny\":[]}}\n".into());
    assert!(matches!(
        admission::admit(&other_settings).unwrap_err(),
        NotAdmitted::ControlSettingsMismatch { .. }
    ));

    // The refusal control was never given the payload at all.
    let mut no_argument = evidence::admissible();
    no_argument.refusal.invocation = evidence::invocation(evidence::REFUSED, false);
    assert!(matches!(
        admission::admit(&no_argument).unwrap_err(),
        NotAdmitted::PayloadNotInInvocation { .. }
    ));

    // Another harness version.
    let mut other_version = evidence::admissible();
    other_version.version = "2.1.268".into();
    assert!(matches!(
        admission::admit(&other_version).unwrap_err(),
        NotAdmitted::VersionMismatch { .. }
    ));

    // No version at all.
    let mut unversioned = evidence::admissible();
    unversioned.version = "  ".into();
    assert!(matches!(
        admission::admit(&unversioned).unwrap_err(),
        NotAdmitted::NoVersion
    ));

    // A command the floor never claimed.
    let mut unrelated = evidence::admissible();
    unrelated.refused_command = "ls -la".into();
    assert!(matches!(
        admission::admit(&unrelated).unwrap_err(),
        NotAdmitted::NotOnTheFloor { .. }
    ));
}

/// Rule 5: deserialization is not a weaker route.
///
/// The record shape this asserts against is the one a hand-written file would
/// take: the word `observed`, a version, a sentence, and no evidence that
/// stands.
#[test]
fn a_deserialized_observation_is_re_judged_before_it_counts() {
    let hand_written = r#"{
      "observation": "observed",
      "version": "2.1.267",
      "observed": "I say the floor was enforced",
      "evidence": {
        "version": "2.1.267",
        "payloadDigest": "0000000000000000000000000000000000000000000000000000000000000000",
        "refusedCommand": "cargo publish --dry-run",
        "allowedCommand": "ls -la",
        "refusal":        {"invocation": {"program":"claude","arguments":[],"workingDirectory":"/"},
                           "settings": null,
                           "capture": {"source":"none","bytes":""}},
        "allowed":        {"invocation": {"program":"claude","arguments":[],"workingDirectory":"/"},
                           "settings": null,
                           "capture": {"source":"none","bytes":""}},
        "withoutPayload": {"invocation": {"program":"claude","arguments":[],"workingDirectory":"/"},
                           "settings": null,
                           "capture": {"source":"none","bytes":""}}
      }
    }"#;
    let observation: Observation = serde_json::from_str(hand_written).unwrap();
    // The shape survives the round trip, which is exactly why the shape is not
    // what anything reads.
    assert!(observation.observed());
    assert!(
        observation.admitted().is_err(),
        "a hand-written record was treated as qualified"
    );

    // And an observation with no evidence field at all does not deserialize,
    // so it is not even a shape to re-judge.
    assert!(
        serde_json::from_str::<Observation>(
            r#"{"observation":"observed","version":"x","observed":"y"}"#
        )
        .is_err()
    );
}

/// Rule 5: the conversion route carries evidence rather than manufacturing it.
#[test]
fn from_qualification_cannot_manufacture_an_admitted_observation() {
    // A probe, which is the only qualification this build produces for itself.
    let probe = statecraft_home::session::VersionProbe {
        version: Some(evidence::VERSION.into()),
        argument_present: true,
    };
    let from_probe =
        Observation::from_qualification(&statecraft_home::session::qualification_from(&probe));
    assert!(!from_probe.observed());

    // A qualification assembled by hand still has to carry evidence, and the
    // evidence is what is judged.
    let mut weak = evidence::admissible();
    weak.refusal.capture = evidence::capture("b1.jsonl", &[evidence::REFUSED], &[]);
    let asserted = statecraft_home::session::Qualification::Qualified {
        version: evidence::VERSION.into(),
        observed: "I say so".into(),
        evidence: Box::new(weak),
    };
    let converted = Observation::from_qualification(&asserted);
    assert!(converted.observed());
    assert!(
        converted.admitted().is_err(),
        "a hand-built qualification reached an admitted observation"
    );
}

/// Every floor entry is matchable, and the control vocabulary is complete.
#[test]
fn the_floor_and_the_controls_are_enumerable() {
    for entry in statecraft_home::settings::DENY_FLOOR {
        let body = entry
            .strip_prefix("Bash(")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        assert!(admission::floor_claims(body.trim_end_matches('*')));
    }
    assert!(!admission::floor_claims("ls -la"));
    assert!(Control::Refusal.carries_the_payload());
    assert!(Control::Allowed.carries_the_payload());
    assert!(!Control::WithoutPayload.carries_the_payload());
}
