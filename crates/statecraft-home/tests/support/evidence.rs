// A synthetic but well-formed body of qualification evidence.
//
// Spec 002 sections 3.29 and 3.30. One builder, shared by this crate's unit
// tests and its integration tests and by the command surface's workflow test,
// so a test that weakens one control is visibly weakening the same fixture
// every other test passes against.
//
// **These captures are synthetic and the module says so in its name.** They
// exercise the admission's reading of the harness's structured output; they
// are not a live observation and nothing built here qualifies a real session.
// Every event shape is one the recorded Claude Code 2.1.267 streams under
// `crates/statecraft-adapter-claude-code/testdata/stream/` carry: the session id
// on every event, `tool_use` blocks with an id, a name and an input, the
// mid-stream `permission_denied` event, a `tool_result` with the
// `tool_result_meta` non-execution note beside a denial and without it beside
// an execution, and the turn-capped terminal event. No field is invented to
// make a control pass.
//
// The launch records say `launched` rather than `synthetic` because what the
// tests below exercise is the admission's judgement of a launch, and the
// synthetic mark is tested on its own. Nothing here reaches an operator.
//
// Included rather than linked, by `include!` from this crate's unit tests and
// by a `#[path]` module from the integration tests, so all of them see one
// copy.

use serde_json::{Value, json};
use statecraft_home::admission::{
    self, ALLOWED_COMMAND, ALLOWED_OUTPUT, Capture, Control, Evidence, Invocation, Launch,
    Measurement, Origin, ProcessEnd, REFUSED_COMMAND,
};

/// The version every capture in the fixture reports.
pub const VERSION: &str = "2.1.267";
/// A command the deny floor claims.
pub const REFUSED: &str = REFUSED_COMMAND;
/// A command no floor entry claims.
pub const ALLOWED: &str = ALLOWED_COMMAND;
/// The directory every control ran in.
pub const CWD: &str = "/fixture/project";
/// Where the launches wrote their records.
pub const CAPTURES: &str = "/fixture/capture";
/// The resolved provider.
pub const PROGRAM: &str = "/fixture/bin/claude";

/// A tool input the way the recorded streams carry one.
pub fn input(command: &str) -> Value {
    json!({ "command": command, "description": format!("Run {command}") })
}

/// The init event.
pub fn init(session: &str) -> Value {
    json!({
        "type": "system", "subtype": "init", "session_id": session,
        "cwd": CWD, "claude_code_version": VERSION, "model": "claude-haiku-4-5-20251001",
        "permissionMode": "default", "tools": ["Bash", "Read", "Edit"], "apiKeySource": "none"
    })
}

/// A hook event, which the recorded streams carry before the init event.
pub fn hook(session: &str) -> Value {
    json!({"type": "system", "subtype": "hook_started", "session_id": session,
           "hook_name": "SessionStart:startup", "hook_event": "SessionStart"})
}

/// An assistant turn requesting one tool use.
pub fn request(session: &str, id: &str, tool: &str, command: &str) -> Value {
    json!({
        "type": "assistant", "session_id": session, "parent_tool_use_id": null,
        "message": { "role": "assistant", "content": [{
            "type": "tool_use", "id": id, "name": tool, "input": input(command),
            "caller": { "type": "direct" }
        }]}
    })
}

/// The two events a denied tool use produces before the terminal event: the
/// mid-stream notification and the result the harness marks not executed.
pub fn denial(session: &str, id: &str, tool: &str, command: &str) -> Vec<Value> {
    let message = format!("Permission to use {tool} with command {command} has been denied.");
    vec![
        json!({"type": "system", "subtype": "permission_denied", "session_id": session,
               "tool_name": tool, "tool_use_id": id,
               "decision_reason_type": "subcommandResults", "message": message}),
        json!({
            "type": "user", "session_id": session, "parent_tool_use_id": null,
            "tool_use_result": format!("Error: {message}"),
            "tool_result_meta": [{ "id": id, "non_execution_kind": "permission-rule" }],
            "message": { "role": "user", "content": [{
                "type": "tool_result", "content": message, "is_error": true, "tool_use_id": id
            }]}
        }),
    ]
}

/// The result a tool use that ran produces: no non-execution note.
pub fn execution(session: &str, id: &str, output: &str, is_error: bool) -> Value {
    json!({
        "type": "user", "session_id": session, "parent_tool_use_id": null,
        "message": { "role": "user", "content": [{
            "type": "tool_result", "content": output, "is_error": is_error, "tool_use_id": id
        }]}
    })
}

/// The turn-capped terminal event, carrying the named denials.
pub fn capped(session: &str, denials: &[(&str, &str, &str)]) -> Value {
    json!({
        "type": "result", "subtype": "error_max_turns", "session_id": session,
        "is_error": true, "terminal_reason": "max_turns", "num_turns": 2,
        "total_cost_usd": 0.01,
        "permission_denials": denials.iter().map(|(tool, id, command)| json!({
            "tool_name": tool, "tool_use_id": id, "tool_input": input(command)
        })).collect::<Vec<_>>()
    })
}

/// Events as `stream-json`.
pub fn jsonl(events: &[Value]) -> String {
    events.iter().map(|e| format!("{e}\n")).collect()
}

/// The refusal control's session: requested, denied, marked not executed.
pub fn refusal_events(session: &str) -> Vec<Value> {
    let mut e = vec![
        hook(session),
        init(session),
        request(session, "toolu_r1", "Bash", REFUSED),
    ];
    e.extend(denial(session, "toolu_r1", "Bash", REFUSED));
    e.push(capped(session, &[("Bash", "toolu_r1", REFUSED)]));
    e
}

/// The allowed control's session: requested, executed, its output returned.
pub fn allowed_events(session: &str) -> Vec<Value> {
    vec![
        hook(session),
        init(session),
        request(session, "toolu_a1", "Bash", ALLOWED),
        execution(session, "toolu_a1", &format!("{ALLOWED_OUTPUT}\n"), false),
        capped(session, &[]),
    ]
}

/// The output cargo gives for the refused command once it runs: it fails, as
/// it was chosen to.
pub const CARGO_FAILS: &str =
    "Exit code 101\nerror: manifest path `statecraft-absent/Cargo.toml` does not exist\n";

/// The absent-payload control's session: requested and executed, and the
/// command itself failed, which is what it was chosen to do.
pub fn without_payload_events(session: &str) -> Vec<Value> {
    vec![
        hook(session),
        init(session),
        request(session, "toolu_w1", "Bash", REFUSED),
        execution(session, "toolu_w1", CARGO_FAILS, true),
        capped(session, &[]),
    ]
}

/// The launch record the launching operation would have written.
pub fn launch(control: Control, stdout: &str) -> Launch {
    let command = match control {
        Control::Allowed => ALLOWED,
        _ => REFUSED,
    };
    let payload = statecraft_home::session::payload_json();
    let digest = statecraft_environment::digest::digest_bytes(payload.as_bytes());
    Launch {
        capture_id: format!("capture-{}", control.word()),
        control,
        origin: Origin::Launched,
        command: command.to_string(),
        prompt: admission::prompt(command),
        requested_program: "claude".to_string(),
        program_digest: Some("0".repeat(64)),
        probe: format!("{VERSION} (Claude Code)\n"),
        probe_version: Some(VERSION.to_string()),
        settings_path: control
            .carries_the_payload()
            .then(|| format!("{CAPTURES}/{}.settings.json", control.word())),
        settings_digest_after: control.carries_the_payload().then_some(digest),
        stderr: String::new(),
        stdout_digest: statecraft_environment::digest::digest_bytes(stdout.as_bytes()),
        stderr_digest: statecraft_environment::digest::digest_bytes(b""),
        undecodable: Vec::new(),
        // The turn cap: exit 1, which the terminal event's error flag agrees
        // with, as `max-turns.jsonl` records.
        process: ProcessEnd {
            code: Some(1),
            signal: None,
            timed_out: false,
            surviving_processes: None,
        },
        deadline_seconds: 300,
    }
}

/// One control, launched as this build launches it.
pub fn measurement(control: Control, stdout: String) -> Measurement {
    let launch = launch(control, &stdout);
    Measurement {
        invocation: Invocation {
            program: PROGRAM.to_string(),
            arguments: admission::arguments(
                control,
                REFUSED,
                ALLOWED,
                launch.settings_path.as_deref(),
            ),
            working_directory: CWD.to_string(),
        },
        settings: control
            .carries_the_payload()
            .then(statecraft_home::session::payload_json),
        capture: Capture {
            source: format!("{CAPTURES}/{}.stdout", control.word()),
            bytes: stdout,
        },
        launch: Some(launch),
    }
}

/// Replace a measurement's captured output, keeping the launch record's digest
/// of it in step, so a test changes the events and nothing else.
pub fn set_capture(m: &mut Measurement, bytes: String) {
    if let Some(l) = m.launch.as_mut() {
        l.stdout_digest = statecraft_environment::digest::digest_bytes(bytes.as_bytes());
    }
    m.capture.bytes = bytes;
}

/// Replace a measurement's captured events.
pub fn set_events(m: &mut Measurement, events: &[Value]) {
    set_capture(m, jsonl(events));
}

/// The launch record of a measurement, for a test that changes one field.
pub fn launch_of(m: &mut Measurement) -> &mut Launch {
    m.launch
        .as_mut()
        .expect("the fixture launches every control")
}

/// The fully favourable body of evidence: every control present and each one
/// behaving as sections 3.29 and 3.30 require.
pub fn admissible() -> Evidence {
    Evidence {
        version: VERSION.to_string(),
        payload_digest: statecraft_home::startup::payload_identity(),
        refused_command: REFUSED.to_string(),
        allowed_command: ALLOWED.to_string(),
        allowed_output: ALLOWED_OUTPUT.to_string(),
        refusal: measurement(Control::Refusal, jsonl(&refusal_events("session-r"))),
        allowed: measurement(Control::Allowed, jsonl(&allowed_events("session-a"))),
        without_payload: measurement(
            Control::WithoutPayload,
            jsonl(&without_payload_events("session-w")),
        ),
    }
}

/// The same evidence, stated as launched against a local fake.
pub fn synthetic() -> Evidence {
    let mut e = admissible();
    for m in [&mut e.refusal, &mut e.allowed, &mut e.without_payload] {
        launch_of(m).origin = Origin::Synthetic;
    }
    e
}
