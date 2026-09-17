//! Spec 008 section 3.9's observable negative cases, nine of eleven.
//!
//! Each test is named after the row it covers. Two rows are the environment half
//! (section 3.7) and live in the crate spec 002 owns, under the `extends` edge
//! spec 008's frontmatter declares: `crates/statecraft-environment/tests/
//! negative_cases.rs` holds the absent prerequisite and the colliding declared
//! path. Spec 008's `## Verification` block runs both suites for that reason.
//!
//! Every row here is checked against the **recorded** streams under
//! `testdata/stream/`, captured from Claude Code 2.1.267. See that directory's
//! `README.md`.

use statecraft_adapter::capability::{Capability, Requested, negotiate};
use statecraft_adapter::manifest::Qualification;
use statecraft_adapter::protocol::{Classification, Event, ReportedCost, read_stream};
use statecraft_adapter::supervisor::{SpawnRefusal, preflight};
use statecraft_adapter_claude_code as provider;
use statecraft_adapter_claude_code::denial::{DenialMechanism, ToolRestriction, refusal_bearing};
use statecraft_adapter_claude_code::stream::{MapError, ProviderEvent, map_stream, read_jsonl};
use statecraft_run::attempt::Outcome;
use statecraft_run::refusal::Accounting;

fn recorded(name: &str) -> Vec<ProviderEvent> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/stream")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is committed testdata: {e}", path.display()));
    read_jsonl(&text).expect("a recorded stream is well formed")
}

// Replay recorded native bytes through a real child, including its exit code.
fn replay(text: &str, code: u8) -> provider::execution::Execution {
    let workspace = tempfile::tempdir().unwrap();
    let fixture = workspace.path().join("stream.jsonl");
    std::fs::write(&fixture, text).unwrap();
    let request = statecraft_adapter::Request {
        workspace: workspace.path().to_path_buf(),
        base_commit: "recorded".into(),
        prompt: b"prompt through stdin".to_vec(),
        capabilities: Requested::none(),
        deadline_seconds: 5,
        attempt: statecraft_adapter::protocol::AttemptIdentity {
            run_id: "native-replay".into(),
            number: 1,
        },
    };
    let environment = statecraft_adapter::environment::construct(
        &statecraft_adapter::environment::Blueprint::empty(),
        &statecraft_adapter::environment::CheckSuiteCommands::default(),
    );
    let execution = provider::execution::supervise(
        std::path::Path::new("/bin/sh"),
        &[
            "-c",
            "/bin/cat > prompt.txt; /bin/cat stream.jsonl; exit \"$1\"",
            "replay",
            &code.to_string(),
        ],
        &request,
        &environment,
        &granted_everything(),
    )
    .unwrap();
    // The child consumed stdin and used relative paths in the request's cwd.
    assert_eq!(
        std::fs::read(workspace.path().join("prompt.txt")).unwrap(),
        request.prompt
    );
    execution
}

fn recorded_text(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/stream")
            .join(name),
    )
    .unwrap()
}

#[test]
fn native_recorded_success_crosses_the_process_seam() {
    let text = recorded_text("success.jsonl");
    // A native line remains invalid at the strict generic adapter boundary.
    assert!(statecraft_adapter::protocol::parse_event(text.lines().next().unwrap(), 1).is_err());
    let execution = replay(&text, 0);
    assert_eq!(execution.supervised.stream_error, None);
    assert_eq!(execution.supervised.outcome, Outcome::Completed);
    assert_eq!(
        execution.supervised.events,
        map_stream(&recorded("success.jsonl"), &granted_everything())
            .unwrap()
            .events
    );
    let result = read_stream(&execution.supervised.events, &[]).unwrap();
    assert_eq!(result.provider_version, provider::MEASURED_PROVIDER_VERSION);
    assert!(result.cost.is_known());
}

#[test]
fn native_denied_success_keeps_progress_claim_turns_and_exactly_one_refusal() {
    let execution = replay(&recorded_text("denied.jsonl"), 0);
    assert_eq!(execution.supervised.stream_error, None);
    assert_eq!(execution.supervised.outcome, Outcome::Refused);
    assert_eq!(execution.termination().adapter_claimed, Outcome::Completed);
    let refusals = statecraft_adapter::protocol::refusals(&execution.supervised.events);
    assert_eq!(refusals.len(), 1);
    let denial: serde_json::Value = serde_json::from_str(&refusals[0].detail).unwrap();
    assert_eq!(
        denial,
        serde_json::to_value(&execution.result.as_ref().unwrap().permission_denials[0]).unwrap()
    );
    assert!(execution.supervised.events.iter().any(
        |e| matches!(e, Event::Progress { message } if message == "system/permission_denied")
    ));
    assert!(
        execution.evidence()["providerTerminal"]["num_turns"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn native_turn_cap_is_interrupted_even_when_the_process_exits_one() {
    let execution = replay(&recorded_text("max-turns.jsonl"), 1);
    assert_eq!(execution.supervised.stream_error, None);
    assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
    assert_eq!(execution.termination().observed, Outcome::Interrupted);
    assert_eq!(
        execution.terminal.unwrap().provider_claim,
        Classification::Stopped
    );
}

#[test]
fn native_tool_removal_produces_no_invented_refusal() {
    let execution = replay(&recorded_text("tool-removed.jsonl"), 0);
    assert_eq!(execution.supervised.stream_error, None);
    assert_eq!(execution.supervised.outcome, Outcome::Completed);
    assert!(statecraft_adapter::protocol::refusals(&execution.supervised.events).is_empty());
}

#[test]
fn native_malformed_and_truncated_streams_retain_progress_and_report_the_error() {
    let text = recorded_text("success.jsonl");
    let mut lines: Vec<_> = text.lines().collect();
    lines.pop();
    let prefix = lines.join("\n");
    let truncated = replay(&prefix, 0);
    assert_eq!(truncated.supervised.outcome, Outcome::Interrupted);
    assert!(matches!(
        truncated.supervised.stream_error,
        Some(statecraft_adapter::StreamError::NoResult { .. })
    ));
    assert!(!truncated.supervised.events.is_empty());
    let malformed = replay(&format!("{prefix}\n\nnot json\n"), 0);
    assert_eq!(malformed.supervised.outcome, Outcome::Interrupted);
    assert_eq!(malformed.supervised.events, truncated.supervised.events);
    assert!(
        matches!(malformed.supervised.stream_error, Some(statecraft_adapter::StreamError::Malformed { line, .. }) if line == lines.len() + 2)
    );
}

#[test]
fn native_unknown_terminal_is_reported_with_its_claim_and_physical_line() {
    let text = recorded_text("success.jsonl");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut result: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    result["subtype"] = "unmeasured".into();
    lines.pop();
    lines.push(String::new());
    lines.push(result.to_string());
    let execution = replay(&lines.join("\n"), 0);
    assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
    assert!(
        matches!(execution.supervised.stream_error, Some(statecraft_adapter::StreamError::Malformed { line, .. }) if line == lines.len())
    );
    assert!(
        execution.evidence()["streamError"]
            .as_str()
            .unwrap()
            .contains("not in spec 008")
    );
    assert_eq!(
        execution.evidence()["providerTerminal"]["subtype"],
        "unmeasured"
    );
    assert!(execution.terminal.is_none());
}

#[test]
fn native_stream_without_init_is_never_completed() {
    let text = recorded_text("success.jsonl");
    let execution = replay(text.lines().last().unwrap(), 0);
    assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
    assert_eq!(
        execution.supervised.stream_error,
        Some(statecraft_adapter::StreamError::NoInit)
    );
    assert!(execution.result.is_some());
    assert!(statecraft_adapter::protocol::refusals(&execution.supervised.events).is_empty());
    assert_eq!(execution.termination().adapter_claimed, Outcome::Completed);
}

#[test]
fn native_denied_success_without_init_keeps_refusals_and_the_stream_error() {
    let text = recorded_text("denied.jsonl");
    // Retain only the recorded terminal event: no init and no mid-stream denial.
    let execution = replay(text.lines().last().unwrap(), 0);
    assert_eq!(execution.termination().observed, Outcome::Interrupted);
    assert_eq!(execution.termination().adapter_claimed, Outcome::Completed);
    assert_eq!(
        execution.supervised.stream_error,
        Some(statecraft_adapter::StreamError::NoInit)
    );
    assert!(execution.terminal.is_none());
    assert!(
        !execution
            .supervised
            .events
            .iter()
            .any(|e| matches!(e, Event::Init { .. }))
    );
    let refusals = statecraft_adapter::protocol::refusals(&execution.supervised.events);
    assert_eq!(refusals.len(), 1);
    assert_eq!(refusals[0].guard, "permission-deny-rule/Bash");
    let denial: serde_json::Value = serde_json::from_str(&refusals[0].detail).unwrap();
    assert_eq!(
        denial,
        execution.evidence()["providerTerminal"]["permission_denials"][0]
    );
    let mut accounting = Accounting::default();
    for refusal in refusals {
        accounting.observe(refusal);
    }
    assert_eq!(accounting.count, 1);
    assert_eq!(
        statecraft_run::refusal::decide(execution.termination().observed, &accounting),
        Outcome::Refused
    );
}

fn granted_everything() -> Vec<Capability> {
    provider::supported()
}

// Row 1. A result with `permission_denials` non-empty and `subtype: "success"`.
#[test]
fn a_denied_session_that_calls_itself_a_success_is_refused_and_never_completed() {
    let events = recorded("denied.jsonl");
    let mapped = map_stream(&events, &granted_everything()).unwrap();
    let result = mapped.result.expect("the recorded stream reached a result");

    assert_eq!(result.subtype, "success");
    assert!(!result.is_error);
    assert_eq!(result.terminal_reason.as_deref(), Some("completed"));
    assert_eq!(result.permission_denials.len(), 1);

    let reading = provider::outcome(&result).unwrap();
    assert_eq!(reading.outcome, Outcome::Refused);
    assert_ne!(reading.outcome, Outcome::Completed);

    // The provider's claim is retained beside the correction, not replaced by it.
    assert_eq!(reading.provider_claim, Classification::Completed);
    // And the completed turns are retained beside the refusal.
    assert_eq!(reading.completed_turns, result.num_turns);
    assert!(reading.completed_turns > 0);

    // The denial entries are recorded verbatim: the tool, the id and the input.
    let denial = &result.permission_denials[0];
    assert_eq!(denial.tool_name, "Bash");
    assert!(!denial.tool_use_id.is_empty());
    assert_eq!(denial.tool_input["command"], "echo hello");
}

// Row 2. A required capability token this manifest does not declare.
#[test]
fn a_required_token_this_manifest_does_not_declare_refuses_before_any_spawn() {
    let environment = statecraft_adapter::environment::construct(
        &statecraft_adapter::environment::Blueprint::empty(),
        &statecraft_adapter::environment::CheckSuiteCommands::default(),
    );
    let requested = Requested::none().requiring(Capability::WorkspaceWrite);
    match preflight(&provider::manifest(), &requested, &environment) {
        Err(SpawnRefusal::MissingRequired { adapter, token, .. }) => {
            assert_eq!(adapter, provider::ADAPTER_NAME);
            assert_eq!(token, "workspace-write");
        }
        other => panic!("expected a refusal naming the token, got {other:?}"),
    }
}

// Row 3. A run prefers `workspace-write`.
#[test]
fn a_run_that_prefers_workspace_write_runs_and_the_degradation_is_recorded() {
    let requested = Requested::none().preferring(Capability::WorkspaceWrite);
    let negotiation = negotiate(&requested, &provider::supported());
    assert!(!negotiation.refuses());
    assert_eq!(negotiation.degraded, [Capability::WorkspaceWrite]);

    // And it is carried in the result, not merely observed at negotiation time.
    let events = recorded("success.jsonl");
    let mapped = map_stream(&events, &negotiation.granted).unwrap();
    let adapter_result = read_stream(&mapped.events, &negotiation.degraded).unwrap();
    assert_eq!(adapter_result.degraded, [Capability::WorkspaceWrite]);
}

// Row 4. `terminal_reason: "max_turns"`.
#[test]
fn a_turn_cap_is_interrupted_and_not_failed() {
    let events = recorded("max-turns.jsonl");
    let mapped = map_stream(&events, &granted_everything()).unwrap();
    let result = mapped.result.unwrap();

    assert_eq!(result.subtype, "error_max_turns");
    assert_eq!(result.terminal_reason.as_deref(), Some("max_turns"));
    // The provider calls it an error.
    assert!(result.is_error);

    let reading = provider::outcome(&result).unwrap();
    // That reading is not adopted.
    assert_eq!(reading.outcome, Outcome::Interrupted);
    assert_ne!(reading.outcome, Outcome::Failed);
}

// Row 5. The provider exits 0 with a denial recorded.
// Row 6. The provider exits 1 on a turn cap.
#[test]
fn the_exit_code_is_read_nowhere_in_this_crate() {
    // Structural, not behavioural: `outcome` takes a result event and no exit
    // status, so there is no parameter through which a code could be read. A
    // test that passed an exit code in would be testing a signature this crate
    // deliberately does not have.
    let denied = provider::outcome(
        &map_stream(&recorded("denied.jsonl"), &[])
            .unwrap()
            .result
            .unwrap(),
    )
    .unwrap();
    let capped = provider::outcome(
        &map_stream(&recorded("max-turns.jsonl"), &[])
            .unwrap()
            .result
            .unwrap(),
    )
    .unwrap();
    // The refused session exited 0 and the capped one exited 1, and the two
    // outcomes disagree with both codes.
    assert_eq!(denied.outcome, Outcome::Refused);
    assert_eq!(capped.outcome, Outcome::Interrupted);

    let sources = std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<String>();
    for forbidden in ["exit_status", "ExitStatus", "exit_code()"] {
        assert!(
            !sources.contains(forbidden),
            "`{forbidden}` in this crate would be the exit code reaching an outcome"
        );
    }
}

// Row 7. The applied tool allowlist is asked for.
#[test]
fn an_applied_allowlist_answers_not_recorded_and_never_the_request_restated() {
    let restriction = ToolRestriction::observed(
        DenialMechanism::Allowlist,
        vec!["Read".to_string()],
        // What the init event listed: 88 entries including Bash and Edit, which
        // is section 3.4's measurement of `--allowedTools Read`.
        vec!["Bash".to_string(), "Edit".to_string(), "Read".to_string()],
    );
    assert_eq!(
        serde_json::to_value(&restriction.applied).unwrap(),
        serde_json::json!("not-recorded")
    );
    assert_eq!(restriction.removal_took_effect(), None);
    // The request survives as the request, in its own field.
    assert_eq!(restriction.requested, ["Read"]);
}

// Row 8. A tool restriction expressed as tool-set removal where a refusal
// record is required.
#[test]
fn a_refusal_that_must_be_evidence_may_not_be_expressed_as_tool_set_removal() {
    let e = refusal_bearing(&["Bash".to_string()], DenialMechanism::ToolSetRemoval).unwrap_err();
    assert!(e.to_string().contains("fails qualification"));
    assert!(e.to_string().contains("structured-refusals"));

    // And the measurement behind it: the removal stream is observable in the
    // init event and produces no refusal record at all.
    let mapped = map_stream(&recorded("tool-removed.jsonl"), &granted_everything()).unwrap();
    assert_eq!(mapped.applied_tools.len(), 89);
    assert!(!mapped.applied_tools.contains(&"Bash".to_string()));
    assert!(mapped.result.unwrap().permission_denials.is_empty());

    // Where the deny rule is the opposite: unobservable and recorded.
    let deny = map_stream(&recorded("denied.jsonl"), &granted_everything()).unwrap();
    assert_eq!(deny.applied_tools.len(), 88);
    assert!(deny.applied_tools.contains(&"Bash".to_string()));
    assert_eq!(deny.result.unwrap().permission_denials.len(), 1);
}

// Row 10. The provider binary version differs from the qualification record.
#[test]
fn a_provider_version_the_record_does_not_name_is_unqualified_and_still_runs() {
    let record = provider::record(
        provider::MEASURED_PROVIDER_VERSION,
        "008.3.9",
        "2026-09-17T00:00:00Z",
    );
    let manifest = provider::manifest();

    assert_eq!(
        statecraft_adapter_claude_code::qualification::qualification_for_pair(
            &manifest,
            provider::MEASURED_PROVIDER_VERSION,
            std::slice::from_ref(&record),
        ),
        Qualification::Qualified
    );

    let newer = statecraft_adapter_claude_code::qualification::qualification_for_pair(
        &manifest,
        "2.1.268",
        std::slice::from_ref(&record),
    );
    assert_eq!(newer, Qualification::Unqualified);

    // Labelled in the posture, and the attempt still proceeds: preflight
    // refuses for a missing required token and for nothing else.
    let environment = statecraft_adapter::environment::construct(
        &statecraft_adapter::environment::Blueprint::empty(),
        &statecraft_adapter::environment::CheckSuiteCommands::default(),
    );
    let requested = Requested::none().requiring(Capability::TurnLimit);
    let negotiation = preflight(&manifest, &requested, &environment).expect("still runs");
    let posture = statecraft_adapter::posture::Posture::new(
        &manifest,
        newer,
        &requested,
        &negotiation,
        &[Capability::TurnLimit],
        &environment,
    );
    assert!(posture.render().contains("unqualified"));
}

// A malformed stream, which spec 004 section 3.5 case 4 and spec 008 section 3.5
// both route to "reported as malformed" rather than to an outcome.
#[test]
fn a_malformed_stream_is_reported_as_malformed_and_never_a_clean_completion() {
    match read_jsonl("{\"type\":\"system\",\"subtype\":\"init\"}\nnot an event\n") {
        Err(MapError::Malformed { line, .. }) => assert_eq!(line, 2),
        other => panic!("expected Malformed, got {other:?}"),
    }
}

// The cost half of section 3.2, and spec 004 section 3.5 case 5.
#[test]
fn a_reported_cost_is_carried_and_an_absent_one_is_unknown_and_never_zero() {
    let mapped = map_stream(&recorded("success.jsonl"), &granted_everything()).unwrap();
    let result = read_stream(&mapped.events, &[]).unwrap();
    match &result.cost {
        ReportedCost::Known(cost) => {
            assert_eq!(cost.amount, 0.044009);
            assert_eq!(cost.unit, "USD");
        }
        other => panic!("the recorded stream reports a cost, got {other:?}"),
    }

    // And the absence, from a stream with no cost field at all.
    let no_cost = read_jsonl(
        "{\"type\":\"system\",\"subtype\":\"init\",\"claude_code_version\":\"2.1.267\"}\n\
         {\"type\":\"result\",\"subtype\":\"success\",\"num_turns\":1}\n",
    )
    .unwrap();
    let mapped = map_stream(&no_cost, &[]).unwrap();
    let result = read_stream(&mapped.events, &[]).unwrap();
    assert_eq!(result.cost, ReportedCost::Unknown("unknown".to_string()));
    assert_eq!(serde_json::to_string(&result.cost).unwrap(), "\"unknown\"");
}

// Section 3.3 rule 1, at the seam: the supervisor's refusal count comes off the
// event stream, so the denial entries have to be on it, exactly once.
#[test]
fn the_supervisors_refusal_count_is_derived_from_the_stream_and_counts_each_denial_once() {
    let mapped = map_stream(&recorded("denied.jsonl"), &granted_everything()).unwrap();
    let refusals = statecraft_adapter::protocol::refusals(&mapped.events);
    assert_eq!(refusals.len(), 1);

    let mut accounting = Accounting::default();
    for refusal in refusals {
        accounting.observe(refusal);
    }
    assert_eq!(accounting.count, 1);
    assert!(
        accounting.sample[0]
            .guard
            .starts_with("permission-deny-rule/")
    );
    // The sample carries the entry verbatim, which is what makes it evidence.
    assert!(accounting.sample[0].detail.contains("echo hello"));
}

// The measured event section 3.1's table does not account for.
//
// Not resolved here. Section 3.1 says there are no refusal events; the recorded
// deny-rule stream carries a mid-stream `system/permission_denied`. This test
// records the contradiction as a fact, because amending what a spec REQUIRES is
// the owner's act and not an implementation's
// (`.claude/rules/adversarial-prompt-refusal.md`). What the mapping does
// meanwhile is section 3.3 rule 1 and nothing beyond it: the refusal record is
// `permission_denials`, and this event is carried as progress like every other
// `system` event, so nothing is counted twice.
#[test]
fn the_recorded_deny_stream_carries_a_mid_stream_permission_denied_event() {
    let mapped = map_stream(&recorded("denied.jsonl"), &granted_everything()).unwrap();
    assert_eq!(mapped.mid_stream_denials, ["Bash"]);

    // Carried as progress, and NOT as a second refusal.
    assert!(mapped.events.iter().any(|e| matches!(
        e,
        Event::Progress { message } if message == "system/permission_denied"
    )));
    assert_eq!(
        statecraft_adapter::protocol::refusals(&mapped.events).len(),
        1
    );
}
