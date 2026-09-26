//! Spec 007: every `--json` answer is the family envelope, spoken by the
//! **built binary**.
//!
//! Each of the 42 operations is invoked with `--json` and nothing else, in an
//! empty home and an empty working directory. Most answer a usage error, a few
//! answer a read of the empty home; whichever it is, stdout must be one
//! envelope whose `exitCode` is the process status and whose `verb` is the
//! operation's dotted name. The per-verb payloads are held by the other binary
//! tests, every one of which parses through `json_naming`, which asserts the
//! envelope on each answer it reads.

#[path = "support/json_naming.rs"]
mod json_naming;

use statecraft_cli::commands::Verb;
use std::path::PathBuf;
use std::process::{Command, Output};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_statecraft-cli"))
}

fn run_in(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .current_dir(dir)
        .env("STATECRAFT_HOME", dir.join("home"))
        .env("HOME", dir)
        .output()
        .expect("the binary runs")
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

#[test]
fn every_verb_answers_json_in_the_envelope_with_its_dotted_name() {
    for verb in Verb::all() {
        let dir = tempfile::tempdir().unwrap();
        let mut args: Vec<&str> = verb.spelling().split(' ').collect();
        args.push("--json");
        let out = run_in(dir.path(), &args);
        let v = json_naming::from_output(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "{args:?}: stdout is not JSON ({e}): {}",
                String::from_utf8_lossy(&out.stdout)
            )
        });
        assert_eq!(v["exitCode"], code(&out), "{args:?}: {v}");
        assert_eq!(v["verb"], verb.dotted(), "{args:?}: {v}");
        assert!(
            out.stderr.is_empty() || code(&out) != 3,
            "{args:?}: a usage error under --json writes nothing to stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn the_tool_member_is_the_built_binarys_name() {
    let name = binary().file_stem().unwrap().to_string_lossy().to_string();
    assert_eq!(name, statecraft_cli::render::TOOL);
}

#[test]
fn a_usage_error_under_json_is_an_envelope_on_stdout_and_text_without_it() {
    let dir = tempfile::tempdir().unwrap();

    // A missing argument to a known verb names that verb.
    let json = run_in(dir.path(), &["run", "list", "--json"]);
    assert_eq!(code(&json), 3);
    assert!(json.stderr.is_empty());
    let v = json_naming::from_output(&json.stdout).unwrap();
    assert_eq!(v["verb"], "run.list");
    assert_eq!(v["error"]["kind"], "usage");
    assert!(v["error"]["message"].as_str().unwrap().contains("usage:"));

    // Words that name no verb are named as typed.
    let unknown = run_in(dir.path(), &["frob", "nicate", "--json"]);
    assert_eq!(code(&unknown), 3);
    let v = json_naming::from_output(&unknown.stdout).unwrap();
    assert_eq!(v["verb"], "frob.nicate");
    assert_eq!(v["outcome"], "usage");

    // With no operation words there is no spelling to copy. The stable name
    // is `unknown`, never an empty member that violates the envelope.
    let absent = run_in(dir.path(), &["--json"]);
    assert_eq!(code(&absent), 3);
    assert!(absent.stderr.is_empty());
    let v = json_naming::from_output(&absent.stdout).unwrap();
    assert_eq!(v["verb"], "unknown");
    assert_eq!(v["error"]["kind"], "usage");

    // Without --json nothing changes: text on stderr, stdout empty.
    for args in [&["run", "list"][..], &["frob", "nicate"][..], &[][..]] {
        let human = run_in(dir.path(), args);
        assert_eq!(code(&human), 3);
        assert!(human.stdout.is_empty(), "{args:?}");
        assert!(!human.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn an_unregistered_target_is_not_found_and_repeats_nothing_in_details() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().display().to_string();
    let out = run_in(dir.path(), &["work", "list", &target, "--json"]);
    assert_eq!(code(&out), 2);
    let v = json_naming::from_output(&out.stdout).unwrap();
    assert_eq!(v["error"]["kind"], "not-found", "{v}");
    assert!(v["error"].get("details").is_none(), "{v}");
    assert_eq!(v["error"]["message"], v["summary"]);
}
