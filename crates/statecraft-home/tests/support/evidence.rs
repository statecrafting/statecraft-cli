// A synthetic but well-formed body of qualification evidence.
//
// Spec 002 section 3.29. One builder, shared by this crate's unit tests and
// its integration tests through a `#[path]` module, so a test that weakens
// one control is visibly weakening the same fixture every other test passes
// against.
//
// **These captures are synthetic and the module says so in its name.** They
// exercise the admission's reading of the harness's structured output; they
// are not a live observation and nothing built here qualifies a real session.
// Section 3.29's closing paragraph is the reason this is honest rather than
// circular: the admission establishes what the captured bytes show, and a
// test supplies bytes on purpose.
//
// Included rather than linked, by `include!` from this crate's unit tests and
// by a `#[path]` module from its integration tests, so both see one copy.

use statecraft_home::admission::{Capture, Evidence, Invocation, Measurement};

/// The version every capture in the fixture reports.
pub const VERSION: &str = "2.1.267";
/// A command the deny floor claims.
pub const REFUSED: &str = "cargo publish --dry-run";
/// A command no floor entry claims.
pub const ALLOWED: &str = "ls -la";

/// One `stream-json` capture: an init event, one attempted tool use, and a
/// terminal result carrying whatever denials are asked for.
pub fn capture(source: &str, attempted: &[&str], denied: &[&str]) -> Capture {
    let mut lines = vec![
        serde_json::json!({
            "type": "system",
            "subtype": "init",
            // A real init event carries a session id, so two captures of two
            // sessions are never the same bytes. The fixture carries one for
            // the same reason: without it, two controls that happened to run
            // the same commands would look like substituted evidence.
            "session_id": source,
            "claude_code_version": VERSION,
            "model": "claude-opus-5",
            "permissionMode": "default",
            "tools": ["Bash", "Read", "Edit"],
            "cwd": "/fixture"
        })
        .to_string(),
    ];
    for (i, command) in attempted.iter().enumerate() {
        lines.push(
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": format!("toolu_{i}"),
                        "name": "Bash",
                        "input": { "command": command }
                    }]
                }
            })
            .to_string(),
        );
    }
    let denials: Vec<serde_json::Value> = denied
        .iter()
        .enumerate()
        .map(|(i, command)| {
            serde_json::json!({
                "tool_name": "Bash",
                "tool_use_id": format!("toolu_{i}"),
                "tool_input": { "command": command }
            })
        })
        .collect();
    lines.push(
        serde_json::json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "num_turns": 1,
            "permission_denials": denials
        })
        .to_string(),
    );
    Capture {
        source: source.to_string(),
        bytes: format!("{}\n", lines.join("\n")),
    }
}

/// An invocation, with or without the settings argument.
pub fn invocation(command: &str, with_payload: bool) -> Invocation {
    let mut arguments = vec!["--max-turns".to_string(), "1".to_string()];
    if with_payload {
        arguments.push("--settings".to_string());
        arguments.push("/fixture/floor.json".to_string());
    }
    arguments.push("-p".to_string());
    arguments.push(format!(
        "run this shell command and show its output: {command}"
    ));
    Invocation {
        program: "claude".to_string(),
        arguments,
        working_directory: "/fixture".to_string(),
    }
}

/// One control.
pub fn measurement(command: &str, with_payload: bool, capture: Capture) -> Measurement {
    Measurement {
        invocation: invocation(command, with_payload),
        settings: with_payload.then(statecraft_home::session::payload_json),
        capture,
    }
}

/// The fully favourable body of evidence: every control present and each one
/// behaving as section 3.29's table requires.
pub fn admissible() -> Evidence {
    Evidence {
        version: VERSION.to_string(),
        payload_digest: statecraft_home::startup::payload_identity(),
        refused_command: REFUSED.to_string(),
        allowed_command: ALLOWED.to_string(),
        refusal: measurement(REFUSED, true, capture("b1.jsonl", &[REFUSED], &[REFUSED])),
        allowed: measurement(ALLOWED, true, capture("b3.jsonl", &[ALLOWED], &[])),
        without_payload: measurement(REFUSED, false, capture("b4.jsonl", &[REFUSED], &[])),
    }
}
