//! What admits a live observation, and the routes that must not bypass it.
//!
//! Spec 002 sections 3.29 and 3.30. Every test here is a **negative control** on
//! the admission itself: it supplies evidence that is wrong in exactly one way,
//! with every other prerequisite satisfied, and asserts the claim is refused for
//! that reason, plus the positive cases that everything else is a mutation of.
//!
//! The first negative test is the transcript that defeated the first
//! implementation, quoted in section 3.29. The tests grouped under section 3.30
//! were each run as a probe against the section 3.29 implementation first, where
//! every one of them was admitted; section 5's 2026-09-22 entry records that run.

/// The one body of evidence, shared with this crate's unit tests.
#[path = "support/evidence.rs"]
#[allow(dead_code)]
mod evidence;

use evidence::{
    ALLOWED, CARGO_FAILS, REFUSED, capped, denial, execution, hook, init, jsonl, launch_of,
    request, set_capture, set_events,
};
use serde_json::json;
use statecraft_home::admission::{self, Control, Evidence, NotAdmitted, Origin};
use statecraft_home::startup::{self, Observation};

/// The refusal, for a mutation that must be refused.
fn refused(e: &Evidence) -> NotAdmitted {
    admission::admit(e).expect_err("the mutation was admitted")
}

// ------------------------------------------------------------ the positives --

/// The favourable case, so every refusal below is one mutation away from a
/// claim that is admitted rather than from one that was never plausible.
#[test]
fn the_three_controls_together_admit_the_observation() {
    let e = evidence::admissible();
    assert!(admission::admit(&e).is_ok(), "{:?}", admission::admit(&e));
    let observation = startup::admit(&e).unwrap();
    assert!(observation.observed());
    assert!(observation.admitted().is_ok());
    assert!(observation.live());
}

/// A session that completed rather than hitting the turn cap is as measurable,
/// with the exit code the recorded completed sessions end with.
#[test]
fn a_completed_session_exiting_zero_is_measurable_too() {
    let mut e = evidence::admissible();
    let mut events = evidence::allowed_events("session-a");
    let last = events.len() - 1;
    events[last] = json!({"type": "result", "subtype": "success", "session_id": "session-a",
        "is_error": false, "terminal_reason": "completed", "num_turns": 2,
        "permission_denials": []});
    set_events(&mut e.allowed, &events);
    launch_of(&mut e.allowed).process.code = Some(0);
    assert!(admission::admit(&e).is_ok(), "{:?}", admission::admit(&e));
}

/// Section 3.30 rule 10: the absent-payload control proves the command reached
/// execution, and the command failing once it ran is what it was chosen to do.
/// The fixture's cargo result is an error, and the claim is admitted.
#[test]
fn permission_success_is_not_command_success() {
    let e = evidence::admissible();
    assert!(
        e.without_payload
            .capture
            .bytes
            .contains("\"is_error\":true")
    );
    assert!(e.without_payload.capture.bytes.contains("does not exist"));
    assert!(admission::admit(&e).is_ok());
}

// ------------------------------------------------- section 3.29's controls --

/// Section 3.29's named defect.
///
/// The transcript contains the command and the word `permission`, which is what
/// the first predicate asked for, and states that the command **succeeded**.
#[test]
fn provider_prose_claiming_permission_was_granted_is_not_a_refusal() {
    let prose = "cargo publish --dry-run: permission granted; command executed successfully\n";
    let mut e = evidence::admissible();
    set_capture(&mut e.refusal, prose.to_string());
    let err = refused(&e);
    assert!(
        matches!(err, NotAdmitted::Unreadable { .. }),
        "prose was read as evidence of enforcement: {err:?}"
    );
    assert!(startup::admit(&e).is_err());
}

/// Well-formed output in which the claimed command ran and nothing denied it.
#[test]
fn a_capture_that_parses_but_carries_no_structured_denial_is_refused() {
    let mut e = evidence::admissible();
    set_events(
        &mut e.refusal,
        &[
            init("session-r"),
            request("session-r", "toolu_r1", "Bash", REFUSED),
            execution("session-r", "toolu_r1", CARGO_FAILS, true),
            capped("session-r", &[]),
        ],
    );
    let err = refused(&e);
    assert!(
        matches!(err, NotAdmitted::NoStructuredRefusal { found: 0, .. }),
        "{err:?}"
    );
    assert!(err.to_string().contains("has said so, and nothing more"));
}

/// The allowed control is required, and it has to have behaved.
#[test]
fn the_allowed_command_control_is_enforced_by_the_boundary() {
    // Refused: the payload refuses something off the floor.
    let mut e = evidence::admissible();
    let mut events = vec![
        init("session-a"),
        request("session-a", "toolu_a1", "Bash", ALLOWED),
    ];
    events.extend(denial("session-a", "toolu_a1", "Bash", ALLOWED));
    events.push(capped("session-a", &[("Bash", "toolu_a1", ALLOWED)]));
    set_events(&mut e.allowed, &events);
    assert!(matches!(
        refused(&e),
        NotAdmitted::ControlRefused {
            control: "allowed-command",
            ..
        }
    ));

    // Never attempted.
    let mut e = evidence::admissible();
    set_events(
        &mut e.allowed,
        &[init("session-a"), capped("session-a", &[])],
    );
    assert!(matches!(
        refused(&e),
        NotAdmitted::ControlNotAttempted {
            control: "allowed-command",
            ..
        }
    ));

    // A command the floor claims cannot show the payload refuses selectively.
    let mut e = evidence::admissible();
    e.allowed_command = "npm publish".into();
    assert!(matches!(refused(&e), NotAdmitted::ControlOnTheFloor { .. }));
}

/// The absent-payload control is required, and it has to have behaved.
#[test]
fn the_no_payload_control_is_enforced_by_the_boundary() {
    let mut e = evidence::admissible();
    let mut events = vec![
        init("session-w"),
        request("session-w", "toolu_w1", "Bash", REFUSED),
    ];
    events.extend(denial("session-w", "toolu_w1", "Bash", REFUSED));
    events.push(capped("session-w", &[("Bash", "toolu_w1", REFUSED)]));
    set_events(&mut e.without_payload, &events);
    let err = refused(&e);
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

    let mut e = evidence::admissible();
    set_events(
        &mut e.without_payload,
        &[init("session-w"), capped("session-w", &[])],
    );
    assert!(matches!(
        refused(&e),
        NotAdmitted::ControlNotAttempted {
            control: "without-payload",
            ..
        }
    ));
}

/// One capture is one measurement.
#[test]
fn two_controls_presenting_the_same_capture_are_substituted_evidence() {
    let mut e = evidence::admissible();
    let bytes = e.refusal.capture.bytes.clone();
    set_capture(&mut e.allowed, bytes);
    assert!(matches!(
        refused(&e),
        NotAdmitted::SubstitutedEvidence { .. }
    ));
}

/// An absent or unfinished capture refuses, and is not a weaker pass.
#[test]
fn missing_and_unfinished_captures_refuse_the_claim() {
    let mut e = evidence::admissible();
    set_capture(&mut e.refusal, "   \n".into());
    assert!(matches!(refused(&e), NotAdmitted::EmptyCapture { .. }));

    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events.pop();
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::NoTerminalResult { .. }));

    // A session that never initialized: only the hook that precedes init.
    let mut e = evidence::admissible();
    set_events(&mut e.refusal, &[hook("session-r")]);
    assert!(matches!(
        refused(&e),
        NotAdmitted::NoInitEvent { control: "refusal" }
    ));
}

/// The claim is bound to this build's payload, a named version, and the floor.
#[test]
fn evidence_for_another_payload_version_or_command_cannot_qualify_this_one() {
    let mut e = evidence::admissible();
    e.payload_digest = "f".repeat(64);
    assert!(matches!(refused(&e), NotAdmitted::PayloadMismatch { .. }));

    let mut e = evidence::admissible();
    e.refusal.settings = Some("{\"permissions\":{\"deny\":[]}}\n".into());
    assert!(matches!(
        refused(&e),
        NotAdmitted::ControlSettingsMismatch { .. }
    ));

    let mut e = evidence::admissible();
    e.version = "2.1.268".into();
    assert!(matches!(
        refused(&e),
        NotAdmitted::VersionMismatch {
            source_of_it: "launch's version probe",
            ..
        }
    ));

    let mut e = evidence::admissible();
    e.version = "  ".into();
    assert!(matches!(refused(&e), NotAdmitted::NoVersion));

    let mut e = evidence::admissible();
    e.refused_command = "ls -la".into();
    assert!(matches!(refused(&e), NotAdmitted::NotOnTheFloor { .. }));
}

// ------------------------- section 3.30 rules 7 to 10: correlated outcomes --

/// A request is not an execution: the allowed command was requested and no
/// result ever came back.
#[test]
fn a_tool_request_without_a_result_is_not_an_execution() {
    for which in [Control::Allowed, Control::WithoutPayload] {
        let mut e = evidence::admissible();
        let (session, id, command) = match which {
            Control::Allowed => ("session-a", "toolu_a1", ALLOWED),
            _ => ("session-w", "toolu_w1", REFUSED),
        };
        let m = if which == Control::Allowed {
            &mut e.allowed
        } else {
            &mut e.without_payload
        };
        set_events(
            m,
            &[
                init(session),
                request(session, id, "Bash", command),
                capped(session, &[]),
            ],
        );
        let err = refused(&e);
        assert!(
            matches!(
                err,
                NotAdmitted::Unresolved {
                    why: "the request has no result",
                    ..
                }
            ),
            "{which:?}: {err:?}"
        );
    }
}

/// The expected command text under another tool's name is not a use of the
/// governed tool, and a denial on it proves nothing about the floor.
#[test]
fn the_expected_command_under_another_tool_is_not_a_governed_use() {
    let mut e = evidence::admissible();
    let mut events = vec![
        init("session-r"),
        request("session-r", "toolu_r1", "Task", REFUSED),
    ];
    events.extend(denial("session-r", "toolu_r1", "Task", REFUSED));
    events.push(capped("session-r", &[("Task", "toolu_r1", REFUSED)]));
    set_events(&mut e.refusal, &events);
    let err = refused(&e);
    assert!(
        matches!(
            err,
            NotAdmitted::ControlNotAttempted {
                control: "refusal",
                ..
            }
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("under another tool"), "{err}");
}

/// A denial naming a tool-use id the capture never requested.
#[test]
fn a_denial_for_another_tool_use_id_does_not_correlate() {
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    let last = events.len() - 1;
    events[last] = capped("session-r", &[("Bash", "toolu_elsewhere", REFUSED)]);
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::Uncorrelated { .. }));
}

/// A tool-use block with no id cannot be correlated with anything.
#[test]
fn a_tool_use_without_an_id_is_unreadable() {
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events[2]["message"]["content"][0]
        .as_object_mut()
        .unwrap()
        .remove("id");
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::Unreadable { .. }));
}

/// A tool result answering a request the capture does not carry.
#[test]
fn a_mismatched_tool_result_does_not_correlate() {
    let mut e = evidence::admissible();
    let mut events = evidence::allowed_events("session-a");
    events[3] = execution(
        "session-a",
        "toolu_other",
        "statecraft-allowed-control\n",
        false,
    );
    set_events(&mut e.allowed, &events);
    assert!(matches!(refused(&e), NotAdmitted::Uncorrelated { .. }));
}

/// A denial whose input is not the request's input verbatim.
#[test]
fn a_denial_whose_input_differs_from_the_request_does_not_correlate() {
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    let last = events.len() - 1;
    events[last]["permission_denials"][0]["tool_input"]["command"] = json!("cargo publish");
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::Uncorrelated { .. }));
}

/// A denial whose result the harness did not mark as not executed, and a
/// result marked not executed with no denial behind it.
#[test]
fn a_refusal_and_its_non_execution_note_must_agree() {
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events[4]
        .as_object_mut()
        .unwrap()
        .remove("tool_result_meta");
    set_events(&mut e.refusal, &events);
    assert!(matches!(
        refused(&e),
        NotAdmitted::Unresolved {
            why: "it is denied and its result is not marked as not executed",
            ..
        }
    ));

    let mut e = evidence::admissible();
    let mut events = evidence::allowed_events("session-a");
    events[3]["tool_result_meta"] =
        json!([{"id": "toolu_a1", "non_execution_kind": "permission-rule"}]);
    set_events(&mut e.allowed, &events);
    assert!(matches!(refused(&e), NotAdmitted::Unresolved { .. }));
}

/// Section 3.30 rule 10: a command that was denied and also executed was not
/// prevented.
#[test]
fn a_denied_command_that_also_executed_is_not_a_refusal() {
    let mut e = evidence::admissible();
    let mut events = vec![
        init("session-r"),
        request("session-r", "toolu_r1", "Bash", REFUSED),
    ];
    events.extend(denial("session-r", "toolu_r1", "Bash", REFUSED));
    events.push(request("session-r", "toolu_r2", "Bash", REFUSED));
    events.push(execution("session-r", "toolu_r2", CARGO_FAILS, true));
    events.push(capped("session-r", &[("Bash", "toolu_r1", REFUSED)]));
    set_events(&mut e.refusal, &events);
    assert!(matches!(
        refused(&e),
        NotAdmitted::ExecutedDespiteRefusal { .. }
    ));
}

/// A session that also did something else is not evidence about this control.
#[test]
fn a_control_that_used_another_tool_as_well_is_refused() {
    let mut e = evidence::admissible();
    let mut events = evidence::allowed_events("session-a");
    events.insert(4, request("session-a", "toolu_a2", "Bash", "ls -la"));
    events.insert(5, execution("session-a", "toolu_a2", "total 0\n", false));
    set_events(&mut e.allowed, &events);
    assert!(matches!(refused(&e), NotAdmitted::UnexpectedToolUse { .. }));
}

/// The allowed command has to produce its output, and not as an error.
#[test]
fn the_allowed_command_must_produce_its_expected_output() {
    for (output, is_error) in [
        ("something else\n", false),
        ("statecraft-allowed-control\n", true),
    ] {
        let mut e = evidence::admissible();
        let mut events = evidence::allowed_events("session-a");
        events[3] = execution("session-a", "toolu_a1", output, is_error);
        set_events(&mut e.allowed, &events);
        assert!(
            matches!(refused(&e), NotAdmitted::WrongOutput { .. }),
            "{output:?} {is_error}"
        );
    }
}

// ----------------------------------- section 3.30 rule 8: one session, whole --

#[test]
fn events_from_two_sessions_are_not_one_capture() {
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events[2]["session_id"] = json!("session-other");
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::MixedSession { .. }));

    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events[2].as_object_mut().unwrap().remove("session_id");
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::MixedSession { .. }));
}

#[test]
fn two_init_events_or_two_terminal_events_are_refused() {
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events.insert(0, init("session-r"));
    set_events(&mut e.refusal, &events);
    assert!(matches!(
        refused(&e),
        NotAdmitted::DuplicateEvent { event: "init", .. }
    ));

    // Two terminal events that disagree: the first says nothing was denied.
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    let last = events.len() - 1;
    events.insert(last, capped("session-r", &[]));
    set_events(&mut e.refusal, &events);
    let err = refused(&e);
    assert!(
        matches!(
            err,
            NotAdmitted::DuplicateEvent {
                event: "terminal",
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn events_out_of_a_sessions_order_are_refused() {
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events.push(hook("session-r"));
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::OutOfOrder { .. }));

    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    let turn = events.remove(2);
    events.insert(0, turn);
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::OutOfOrder { .. }));
}

/// The formatting differs and an irrelevant event is added, and it is the same
/// session: substituted evidence is caught by identity, not by bytes.
#[test]
fn a_reformatted_copy_of_one_session_is_still_one_session() {
    let mut e = evidence::admissible();
    let mut events = evidence::without_payload_events("session-r");
    events.insert(
        0,
        json!({"type": "rate_limit_event", "session_id": "session-r"}),
    );
    let reformatted: String = events
        .iter()
        .map(|v| {
            format!(
                "{}\n",
                serde_json::to_string_pretty(v).unwrap().replace('\n', " ")
            )
        })
        .collect();
    set_capture(&mut e.without_payload, reformatted);
    let err = refused(&e);
    assert!(
        matches!(
            err,
            NotAdmitted::SharedIdentity {
                what: "the session",
                ..
            }
        ),
        "{err:?}"
    );

    // The same tool use presented under a second session id.
    let mut e = evidence::admissible();
    let mut events = evidence::without_payload_events("session-w");
    events[2]["message"]["content"][0]["id"] = json!("toolu_a1");
    events[3]["message"]["content"][0]["tool_use_id"] = json!("toolu_a1");
    set_events(&mut e.without_payload, &events);
    assert!(matches!(
        refused(&e),
        NotAdmitted::SharedIdentity {
            what: "the tool-use id",
            ..
        }
    ));

    // One launch presented as two.
    let mut e = evidence::admissible();
    launch_of(&mut e.allowed).capture_id = "capture-refusal".into();
    assert!(matches!(
        refused(&e),
        NotAdmitted::SharedIdentity {
            what: "the capture identity",
            ..
        }
    ));
}

// -------------------------------- section 3.30 rule 11: process and terminal --

#[test]
fn an_interrupted_or_incomplete_control_is_unmeasured() {
    type Mutation = Box<dyn Fn(&mut admission::Launch)>;
    let cases: Vec<(&str, Mutation)> = vec![
        ("timeout", Box::new(|l| l.process.timed_out = true)),
        (
            "signal",
            Box::new(|l| {
                l.process.code = None;
                l.process.signal = Some(9);
            }),
        ),
        (
            "survivor",
            Box::new(|l| l.process.surviving_processes = Some("pid 7".into())),
        ),
        ("no exit code", Box::new(|l| l.process.code = None)),
        ("exit 2", Box::new(|l| l.process.code = Some(2))),
    ];
    for (name, mutate) in cases {
        let mut e = evidence::admissible();
        mutate(launch_of(&mut e.refusal));
        assert!(
            matches!(
                refused(&e),
                NotAdmitted::Unmeasured {
                    control: "refusal",
                    ..
                }
            ),
            "{name}"
        );
    }

    // A terminal state that is neither completed nor the turn cap.
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    let last = events.len() - 1;
    events[last]["subtype"] = json!("success");
    events[last]["terminal_reason"] = json!("api_error");
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::Unmeasured { .. }));

    // An exit code the terminal event disagrees with.
    let mut e = evidence::admissible();
    launch_of(&mut e.refusal).process.code = Some(0);
    assert!(matches!(
        refused(&e),
        NotAdmitted::ExitDisagrees {
            code: 0,
            is_error: true,
            ..
        }
    ));
}

// ---------------------------------- section 3.30 rule 12: bound to the launch --

#[test]
fn a_record_without_a_launch_is_refused() {
    let mut e = evidence::admissible();
    e.allowed.launch = None;
    assert!(matches!(
        refused(&e),
        NotAdmitted::NoLaunchRecord {
            control: "allowed-command"
        }
    ));
}

#[test]
fn the_settings_argument_must_bind_the_control_to_its_settings() {
    let args = |e: &mut Evidence| -> Vec<String> { e.refusal.invocation.arguments.clone() };
    let settings = |a: &[String]| a.iter().position(|x| x == "--settings").unwrap();

    // Names a different file than the one the launch wrote.
    let mut e = evidence::admissible();
    let mut a = args(&mut e);
    let i = settings(&a);
    a[i + 1] = "/elsewhere/other.json".into();
    e.refusal.invocation.arguments = a;
    assert!(matches!(refused(&e), NotAdmitted::SettingsArgument { .. }));

    // The other spelling.
    let mut e = evidence::admissible();
    let mut a = args(&mut e);
    let i = settings(&a);
    let path = a.remove(i + 1);
    a[i] = format!("--settings={path}");
    e.refusal.invocation.arguments = a;
    let err = refused(&e);
    assert!(err.to_string().contains("spelled"), "{err}");

    // Two settings inputs.
    let mut e = evidence::admissible();
    e.refusal.invocation.arguments.push("--settings".into());
    e.refusal.invocation.arguments.push("/other.json".into());
    let err = refused(&e);
    assert!(err.to_string().contains("2 settings arguments"), "{err}");

    // None at all.
    let mut e = evidence::admissible();
    let mut a = args(&mut e);
    let i = settings(&a);
    a.drain(i..i + 2);
    e.refusal.invocation.arguments = a;
    assert!(matches!(refused(&e), NotAdmitted::SettingsArgument { .. }));
}

#[test]
fn the_absent_payload_control_carries_no_settings_in_either_spelling() {
    for arg in [
        vec!["--settings".to_string(), "/fixture/x.json".to_string()],
        vec!["--settings=/fixture/x.json".to_string()],
    ] {
        let mut e = evidence::admissible();
        e.without_payload.invocation.arguments.extend(arg);
        assert!(matches!(
            refused(&e),
            NotAdmitted::PayloadInTheControl { .. }
        ));
    }
    let mut e = evidence::admissible();
    e.without_payload.settings = Some(statecraft_home::session::payload_json());
    assert!(matches!(
        refused(&e),
        NotAdmitted::PayloadInTheControl { .. }
    ));
}

#[test]
fn an_argument_vector_this_build_does_not_launch_is_refused() {
    let mut e = evidence::admissible();
    e.allowed
        .invocation
        .arguments
        .insert(0, "--dangerously-skip-permissions".into());
    assert!(matches!(
        refused(&e),
        NotAdmitted::InvocationMismatch { .. }
    ));

    // Reordered, in the control that carries no settings argument to trip
    // over first.
    let mut e = evidence::admissible();
    e.without_payload.invocation.arguments.reverse();
    assert!(matches!(
        refused(&e),
        NotAdmitted::InvocationMismatch { .. }
    ));
}

#[test]
fn settings_that_changed_while_the_session_ran_are_not_the_ones_it_was_given() {
    let mut e = evidence::admissible();
    launch_of(&mut e.refusal).settings_digest_after = Some("e".repeat(64));
    assert!(matches!(
        refused(&e),
        NotAdmitted::SettingsChangedDuringRun { .. }
    ));
}

#[test]
fn a_launch_must_be_the_control_it_is_filed_as() {
    let mut e = evidence::admissible();
    launch_of(&mut e.allowed).control = Control::Refusal;
    assert!(matches!(
        refused(&e),
        NotAdmitted::ControlMislabelled { .. }
    ));

    let mut e = evidence::admissible();
    launch_of(&mut e.without_payload).command = ALLOWED.into();
    assert!(matches!(refused(&e), NotAdmitted::CommandMismatch { .. }));

    let mut e = evidence::admissible();
    launch_of(&mut e.refusal).prompt = format!("please run {REFUSED}");
    assert!(matches!(refused(&e), NotAdmitted::PromptMismatch { .. }));
}

#[test]
fn the_init_event_must_agree_with_the_launch() {
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events[1]["claude_code_version"] = json!("2.1.268");
    set_events(&mut e.refusal, &events);
    assert!(matches!(
        refused(&e),
        NotAdmitted::VersionMismatch {
            source_of_it: "init event",
            ..
        }
    ));

    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events[1]["cwd"] = json!("/somewhere/else");
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::Uncorrelated { .. }));

    let mut e = evidence::admissible();
    e.allowed.invocation.working_directory = "/fixture/other".into();
    assert!(matches!(
        refused(&e),
        NotAdmitted::DifferentWorkingDirectories { .. }
    ));
}

#[test]
fn recorded_output_must_be_the_output_the_launch_read() {
    let mut e = evidence::admissible();
    e.refusal.capture.bytes.push('\n');
    let err = refused(&e);
    assert!(err.to_string().contains("does not digest"), "{err}");

    let mut e = evidence::admissible();
    launch_of(&mut e.refusal).undecodable = vec!["stdout".into()];
    assert!(matches!(refused(&e), NotAdmitted::Unreadable { .. }));
}

// ------------------------------------------- section 3.29 rule 5: every route --

/// Deserialization is not a weaker route, and a record written before section
/// 3.30 still reads, is kept as it is, and does not qualify.
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
    assert!(observation.observed());
    assert!(
        observation.admitted().is_err(),
        "a hand-written record was treated as qualified"
    );
    assert!(!observation.live());

    // An observation with no evidence field at all does not deserialize.
    assert!(
        serde_json::from_str::<Observation>(
            r#"{"observation":"observed","version":"x","observed":"y"}"#
        )
        .is_err()
    );

    // Nor does evidence missing a control: a missing control is not a weaker
    // claim.
    let mut value = serde_json::to_value(evidence::admissible()).unwrap();
    value.as_object_mut().unwrap().remove("allowed");
    assert!(serde_json::from_value::<Evidence>(value).is_err());
}

/// A record written by the section 3.29 build, whose evidence would have been
/// admitted then, reads now and is refused for the missing launch.
#[test]
fn evidence_written_before_section_3_30_reads_and_is_unverified() {
    let mut value = serde_json::to_value(evidence::admissible()).unwrap();
    for control in ["refusal", "allowed", "withoutPayload"] {
        value[control].as_object_mut().unwrap().remove("launch");
    }
    value.as_object_mut().unwrap().remove("allowedOutput");
    let old: Evidence = serde_json::from_value(value.clone()).expect("an old record still reads");
    assert!(matches!(
        admission::admit(&old).unwrap_err(),
        NotAdmitted::NoLaunchRecord { control: "refusal" }
    ));
    // Nothing about reading it changed its bytes.
    assert_eq!(
        serde_json::to_value(&old).unwrap()["refusal"],
        value["refusal"]
    );
}

/// The conversion route carries evidence rather than manufacturing it.
#[test]
fn from_qualification_cannot_manufacture_an_admitted_observation() {
    let probe = statecraft_home::session::VersionProbe {
        version: Some(evidence::VERSION.into()),
        argument_present: true,
    };
    let from_probe =
        Observation::from_qualification(&statecraft_home::session::qualification_from(&probe));
    assert!(!from_probe.observed());

    let mut weak = evidence::admissible();
    weak.allowed.launch = None;
    let asserted = statecraft_home::session::Qualification::Qualified {
        version: evidence::VERSION.into(),
        observed: "I say so".into(),
        evidence: Box::new(weak),
    };
    assert!(
        !asserted.qualified(),
        "a hand-built qualification reported itself qualified"
    );
    let converted = Observation::from_qualification(&asserted);
    assert!(converted.observed());
    assert!(converted.admitted().is_err());
    assert!(!converted.live());
}

/// Section 3.30: synthetic evidence runs the whole admission, and is never a
/// live observation by any route.
#[test]
fn synthetic_evidence_is_admitted_and_never_live() {
    let e = evidence::synthetic();
    assert!(admission::admit(&e).is_ok());
    assert!(e.synthetic());
    let observation = startup::admit(&e).unwrap();
    assert!(observation.observed());
    assert!(!observation.live());
    assert_eq!(observation.word(), "observed-synthetic");
    let Observation::Observed { observed, .. } = &observation else {
        unreachable!()
    };
    assert!(observed.starts_with("SYNTHETIC"), "{observed}");
    let as_qualification = statecraft_home::session::Qualification::Qualified {
        version: evidence::VERSION.into(),
        observed: observed.clone(),
        evidence: Box::new(e.clone()),
    };
    assert!(!as_qualification.qualified());

    // One synthetic control is enough to make the whole observation synthetic.
    let mut one = evidence::admissible();
    launch_of(&mut one.without_payload).origin = Origin::Synthetic;
    assert!(one.synthetic());
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
    for c in Control::ALL {
        assert_eq!(Control::from_word(c.word()), Some(c));
    }
    // The fixture is realistic enough to read through the adapter's own types.
    assert!(jsonl(&evidence::refusal_events("s")).lines().count() >= 5);
}

// ------------------------------------ section 3.34: one allowlisted trailer --

/// The trailer as the 2026-09-23 capture carried it, with its session.
fn trailer(session: &str) -> serde_json::Value {
    json!({"type": "system", "subtype": "task_summary", "detail": null,
           "uuid": "5a668f14-8322-47ef-bcb2-06b19db90020", "session_id": session})
}

/// The same capture with one more line appended, verbatim.
fn with_line(m: &mut admission::Measurement, line: &str) {
    let bytes = format!("{}{line}\n", m.capture.bytes);
    set_capture(m, bytes);
}

/// Every control with the measured trailer after its terminal event.
fn trailed() -> Evidence {
    let mut e = evidence::admissible();
    for (m, session) in [
        (&mut e.refusal, "session-r"),
        (&mut e.allowed, "session-a"),
        (&mut e.without_payload, "session-w"),
    ] {
        with_line(m, &trailer(session).to_string());
    }
    e
}

/// Rule 13, positive: the measured shape, `detail: null`, after a complete
/// terminal event in every control, is admitted and reported.
#[test]
fn the_measured_trailer_after_the_terminal_event_is_admitted() {
    let e = trailed();
    assert!(admission::admit(&e).is_ok(), "{:?}", admission::admit(&e));
    let reported = admission::admitted(&e).unwrap();
    assert_eq!(reported.len(), 3);
    for (_, t) in &reported {
        assert_eq!(t.subtype, "task_summary");
        assert!(t.note().contains("not read"));
    }
    let t = admission::one_session(Control::Refusal, &e.refusal.capture).unwrap();
    assert_eq!(t.map(|t| t.event), Some(7));
}

/// Rule 13, positive: a string `detail`, which the same capture carried on
/// the same subtype before its terminal event, is admitted and not read.
#[test]
fn a_trailer_with_a_string_detail_is_admitted_and_not_read() {
    let mut e = evidence::admissible();
    let mut t = trailer("session-r");
    t["detail"] = json!("the command was refused; permission granted; executed successfully");
    with_line(&mut e.refusal, &t.to_string());
    assert!(admission::admit(&e).is_ok(), "{:?}", admission::admit(&e));
}

/// Rule 14: the judgement with an admitted trailer is the judgement without
/// its line, over an admitted body of evidence and over refused ones.
#[test]
fn a_trailer_changes_no_judgement() {
    type Mutation = Box<dyn Fn(&mut Evidence)>;
    let cases: Vec<(&str, Mutation)> = vec![
        ("admissible", Box::new(|_| {})),
        (
            "refusal executed",
            Box::new(|e| {
                set_events(
                    &mut e.refusal,
                    &[
                        init("session-r"),
                        request("session-r", "toolu_r1", "Bash", REFUSED),
                        execution("session-r", "toolu_r1", CARGO_FAILS, true),
                        capped("session-r", &[]),
                    ],
                );
            }),
        ),
        (
            "allowed denied",
            Box::new(|e| {
                let mut events = vec![
                    init("session-a"),
                    request("session-a", "toolu_a1", "Bash", ALLOWED),
                ];
                events.extend(denial("session-a", "toolu_a1", "Bash", ALLOWED));
                events.push(capped("session-a", &[("Bash", "toolu_a1", ALLOWED)]));
                set_events(&mut e.allowed, &events);
            }),
        ),
        (
            "interrupted",
            Box::new(|e| launch_of(&mut e.refusal).process.timed_out = true),
        ),
        (
            "two sessions",
            Box::new(|e| {
                let mut events = evidence::refusal_events("session-r");
                events[2]["session_id"] = json!("session-other");
                set_events(&mut e.refusal, &events);
            }),
        ),
    ];
    // Prose shaped like a verdict either way, and no prose at all.
    let details = [
        json!(null),
        json!("the command was refused and did not execute"),
        json!("permission granted; the command executed successfully"),
    ];
    for (name, mutate) in cases {
        let mut plain = evidence::admissible();
        mutate(&mut plain);
        for detail in &details {
            // On every control, and on the refusal control alone.
            for all in [true, false] {
                let mut with = plain.clone();
                for (m, session) in [
                    (&mut with.refusal, "session-r"),
                    (&mut with.allowed, "session-a"),
                    (&mut with.without_payload, "session-w"),
                ] {
                    if !all && session != "session-r" {
                        continue;
                    }
                    let mut t = trailer(session);
                    t["detail"] = detail.clone();
                    with_line(m, &t.to_string());
                }
                assert_eq!(
                    format!("{:?}", admission::admit(&plain)),
                    format!("{:?}", admission::admit(&with)),
                    "{name}, detail {detail}, all {all}: a trailer changed the judgement"
                );
            }
        }
    }
}

/// Rule 13, negative: everything else after the terminal event refuses the
/// capture, and so does a trailer that is not exactly the closed shape.
#[test]
fn anything_else_after_the_terminal_event_is_refused() {
    let t = trailer("session-r");
    let mut extra = t.clone();
    extra["tool_use_id"] = json!("toolu_r1");
    let mut missing_detail = t.clone();
    missing_detail.as_object_mut().unwrap().remove("detail");
    let mut missing_uuid = t.clone();
    missing_uuid.as_object_mut().unwrap().remove("uuid");
    let mut number_detail = t.clone();
    number_detail["detail"] = json!(3);
    let mut object_detail = t.clone();
    object_detail["detail"] = json!({"refused": true});
    let mut empty_uuid = t.clone();
    empty_uuid["uuid"] = json!("");
    let mut other_session = t.clone();
    other_session["session_id"] = json!("session-other");
    let mut no_session = t.clone();
    no_session.as_object_mut().unwrap().remove("session_id");
    let mut other_subtype = t.clone();
    other_subtype["subtype"] = json!("status");
    let mut not_system = t.clone();
    not_system["type"] = json!("rate_limit_event");

    let single: Vec<(&str, String)> = vec![
        ("added member", extra.to_string()),
        ("missing detail", missing_detail.to_string()),
        ("missing uuid", missing_uuid.to_string()),
        ("numeric detail", number_detail.to_string()),
        ("object detail", object_detail.to_string()),
        ("empty uuid", empty_uuid.to_string()),
        ("another session", other_session.to_string()),
        ("no session", no_session.to_string()),
        ("another subtype", other_subtype.to_string()),
        ("a rate-limit event", not_system.to_string()),
        (
            "a rate-limit event as measured",
            json!({"type": "rate_limit_event", "session_id": "session-r"}).to_string(),
        ),
        ("a hook event", hook("session-r").to_string()),
        (
            "a mid-stream denial",
            denial("session-r", "toolu_r1", "Bash", REFUSED)[0].to_string(),
        ),
        (
            "an assistant turn",
            request("session-r", "toolu_r9", "Bash", ALLOWED).to_string(),
        ),
        (
            "a tool result",
            execution("session-r", "toolu_r1", CARGO_FAILS, true).to_string(),
        ),
        (
            "a member given twice",
            r#"{"type":"system","subtype":"task_summary","detail":null,"uuid":"u","session_id":"session-r","session_id":"session-other"}"#.to_string(),
        ),
    ];
    for (name, line) in single {
        let mut e = evidence::admissible();
        with_line(&mut e.refusal, &line);
        let err = admission::admit(&e).expect_err(name);
        assert!(
            matches!(
                err,
                NotAdmitted::OutOfOrder { .. }
                    | NotAdmitted::MixedSession { .. }
                    | NotAdmitted::Unreadable { .. }
            ),
            "{name}: {err:?}"
        );
        assert!(
            admission::one_session(Control::Refusal, &e.refusal.capture).is_err(),
            "{name}: the launch read it as one complete session"
        );
    }

    // Two trailers.
    let mut e = evidence::admissible();
    with_line(&mut e.refusal, &t.to_string());
    with_line(&mut e.refusal, &t.to_string());
    assert!(matches!(refused(&e), NotAdmitted::OutOfOrder { .. }));

    // A second terminal event after the trailer.
    let mut e = evidence::admissible();
    with_line(&mut e.refusal, &t.to_string());
    with_line(&mut e.refusal, &capped("session-r", &[]).to_string());
    assert!(matches!(refused(&e), NotAdmitted::OutOfOrder { .. }));

    // A trailer with no terminal event before it never completes a capture.
    let mut e = evidence::admissible();
    let mut events = evidence::refusal_events("session-r");
    events.pop();
    events.push(t.clone());
    set_events(&mut e.refusal, &events);
    assert!(matches!(refused(&e), NotAdmitted::NoTerminalResult { .. }));
}

/// Before the terminal event nothing changed: a `task_summary` with a string
/// detail and a `rate_limit_event` are carried as the live capture carried
/// them, and neither is read.
#[test]
fn a_summary_and_a_rate_limit_event_before_the_terminal_event_are_unchanged() {
    let mut e = trailed();
    let mut events = evidence::refusal_events("session-r");
    events.insert(
        3,
        json!({"type": "rate_limit_event", "session_id": "session-r"}),
    );
    events.insert(
        4,
        json!({"type": "system", "subtype": "task_summary",
               "detail": "Running cargo publish dry run",
               "uuid": "72c614d5-bd6b-4030-9a62-655f304d129a", "session_id": "session-r"}),
    );
    events.push(trailer("session-r"));
    set_events(&mut e.refusal, &events);
    assert!(admission::admit(&e).is_ok(), "{:?}", admission::admit(&e));
}
