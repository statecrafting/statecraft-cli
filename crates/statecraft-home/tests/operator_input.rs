//! Spec 053 section 3.3: the admission gate, run as the program it is,
//! returns a note the supervisor placed in the exchange directory as the
//! governed call's context, once, and never after the note's expiry; and the
//! supervisor reads back from the gate log which notes it returned.

use statecraft_home::input::{ExchangePlacement, NOTES, hook_answer};
use statecraft_home::launch::{GATE_SCRIPT, files};
use statecraft_run::input::{Input, InputKind, Origin, Placement, State, Submission, Transition};
use std::path::Path;
use std::process::{Command, Stdio};

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// An attempt's exchange directory whose startup decision admitted.
fn exchange() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(files::GATE), GATE_SCRIPT).unwrap();
    std::fs::write(dir.path().join(files::GATE_LOG), "").unwrap();
    std::fs::write(
        dir.path().join(files::ADMISSION),
        "{\"decision\":\"admitted\"}\n",
    )
    .unwrap();
    dir
}

fn input(id: &str, expires_at: i64) -> Input {
    Input {
        id: id.to_string(),
        submission: Submission {
            kind: InputKind::Note,
            body: "the fix is in the parser, not the lexer".to_string(),
            author: "bart".to_string(),
            reason: "wrong module".to_string(),
            run_id: "run-1".to_string(),
            attempt: 1,
            expires_at,
            origin: Origin::Cli,
        },
        state: State::Pending,
        history: vec![Transition {
            state: State::Pending,
            at: "t".to_string(),
            position: 0,
            attestation: None,
            why: None,
        }],
    }
}

/// One governed call: the gate, given a tool call on stdin. Run through `sh`
/// rather than executed, so a script just written is never exec'd while
/// another test thread's fork still holds it open for writing.
fn call(dir: &Path) -> (i32, String) {
    let mut child = Command::new("sh")
        .arg(dir.join(files::GATE))
        .arg("10")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().unwrap();
        let _ = stdin.write_all(b"{\"tool_name\":\"Bash\"}");
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
    )
}

fn log(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(files::GATE_LOG)).unwrap()
}

#[test]
fn with_no_note_the_gate_admits_exactly_as_before() {
    let dir = exchange();
    assert_eq!(call(dir.path()), (0, String::new()));
    assert_eq!(log(dir.path()), "admitted\n");
    assert!(!dir.path().join(NOTES).exists(), "nothing is created early");
}

#[test]
fn a_placed_note_is_returned_once_as_context_and_read_back_from_the_log() {
    let dir = exchange();
    let mut placement = ExchangePlacement::new(dir.path());
    let note = input("input-000001", now() + 600);
    placement.place(&note).unwrap();

    let (code, stdout) = call(dir.path());
    assert_eq!(code, 0, "a note never withholds the call");
    assert_eq!(stdout, hook_answer(&note));
    let answer: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(answer["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    let context = answer["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.contains("the fix is in the parser"), "{context}");
    assert!(context.contains("bart"), "{context}");
    assert_eq!(log(dir.path()), "admitted\nnote input-000001\n");
    assert_eq!(placement.returned(), ["input-000001"]);

    // The next call returns nothing: the log already names it.
    assert_eq!(call(dir.path()), (0, String::new()));
    placement.withdraw("input-000001").unwrap();
    assert_eq!(
        std::fs::read_dir(dir.path().join(NOTES)).unwrap().count(),
        0
    );
}

#[test]
fn two_notes_are_returned_one_per_call_oldest_first() {
    let dir = exchange();
    let mut placement = ExchangePlacement::new(dir.path());
    placement
        .place(&input("input-000002", now() + 600))
        .unwrap();
    placement
        .place(&input("input-000001", now() + 600))
        .unwrap();
    call(dir.path());
    call(dir.path());
    assert_eq!(placement.returned(), ["input-000001", "input-000002"]);
}

#[test]
fn a_note_past_its_expiry_is_never_returned() {
    let dir = exchange();
    let mut placement = ExchangePlacement::new(dir.path());
    placement.place(&input("input-000001", now() - 1)).unwrap();
    assert_eq!(call(dir.path()), (0, String::new()));
    assert!(placement.returned().is_empty());
}

#[test]
fn a_refused_decision_returns_no_note() {
    let dir = exchange();
    std::fs::write(
        dir.path().join(files::ADMISSION),
        "{\"decision\":\"refused\"}\n",
    )
    .unwrap();
    let mut placement = ExchangePlacement::new(dir.path());
    placement
        .place(&input("input-000001", now() + 600))
        .unwrap();
    let (code, stdout) = call(dir.path());
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert!(placement.returned().is_empty());
}
