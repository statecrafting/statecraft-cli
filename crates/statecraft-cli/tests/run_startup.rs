//! A run's startup evidence, through the built binary, from initialization to
//! the judgement an operator reads.
//!
//! Spec 002 section 3.31 and spec 006 section 3.11.3. Every run is given a
//! temporary `STATECRAFT_HOME`, `STATECRAFT_NATIVE_ROOT` and `HOME`, and a
//! `PATH` holding only a fake `spec-spine`, a fake `claude` and the system
//! directories. **No provider is spawned, and nothing here is live evidence.**
//!
//! The fake provider does what section 3.31 needs a provider to do and nothing
//! more: it checks that `--settings` carries this build's payload byte for
//! byte, runs whichever `SessionStart` hook the test has "registered" (a file
//! naming a script, standing in for the operator's global registration), and
//! reports that hook's output as a `hook_response` event in the shape the
//! recorded Claude Code 2.1.267 streams carry, followed by a recorded session.
//! Whether the live provider does the same in a managed run is the premise
//! section 3.31 rule 20 records as unobserved.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RUN: &str = "replay";

fn executable(path: &Path, script: &str) {
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("exited by itself")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(out)))
}

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }
    fn root(&self) -> String {
        self.project().display().to_string()
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(self.project())
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", text(&out));
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    /// Initialization, the explicit harness requirement, the instruction
    /// bridge, a commit so the workspace holds all of it, and the consent to
    /// be driven: the operator's route, in order.
    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(f.project()).unwrap();
        std::fs::create_dir_all(f.bin()).unwrap();
        f.git(&["init", "--quiet", "--initial-branch=main"]);
        f.git(&["config", "user.email", "fixture@example.invalid"]);
        f.git(&["config", "user.name", "fixture"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(f.project().join("README.md"), "x\n").unwrap();
        f.git(&["add", "."]);
        f.git(&["commit", "--quiet", "-m", "base"]);

        // A synthetic scheduler input, so `run` has a unit of work. It creates
        // and ratifies no spec anywhere.
        executable(
            &f.bin().join("spec-spine"),
            r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"replay","title":"recorded stream"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"replay","status":"approved","implementation":"pending"}]}' ;;
  *) exit 3 ;;
esac
"#,
        );
        std::fs::write(
            f.bin().join("expected-settings"),
            statecraft_home::session::payload_json(),
        )
        .unwrap();
        let recorded = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl");
        std::fs::copy(recorded, f.bin().join("native.jsonl")).unwrap();
        executable(&f.bin().join("claude"), FAKE_PROVIDER);

        for (args, ok) in [
            (vec!["home", "apply"], &[0, 1][..]),
            (vec!["init", "apply", &f.root()], &[0, 1]),
            (vec!["harness", "upgrade", &f.root()], &[0]),
        ] {
            let out = f.cli(&args);
            assert!(ok.contains(&code(&out)), "{args:?}: {}", text(&out));
        }
        std::fs::write(
            f.project().join("AGENTS.md"),
            "@.statecraft/AGENTS.md\n\n# the fixture project\n",
        )
        .unwrap();
        f.git(&["add", "-A"]);
        f.git(&["commit", "--quiet", "-m", "managed"]);
        for args in [["project", "register"], ["project", "arm"]] {
            let out = f.cli(&[args[0], args[1], &f.root()]);
            assert!(code(&out) <= 1, "{args:?}: {}", text(&out));
        }
        f
    }

    /// The installed revision directory the requirement names.
    fn required_revision(&self) -> PathBuf {
        let out = self.cli(&["harness", "show", &self.root(), "--json"]);
        let v = json(&out);
        let display = v["value"]["value"]["inspection"]["requiredDisplay"]
            .as_str()
            .unwrap_or_else(|| panic!("{v}"))
            .to_string();
        self.home().join("harness").join(display)
    }

    /// Stand in for the operator's global registration of a SessionStart hook.
    fn register(&self, revision: &Path) {
        std::fs::write(
            self.bin().join("registered-hook"),
            revision
                .join("hooks/statecraft-session-start.sh")
                .display()
                .to_string(),
        )
        .unwrap();
    }

    fn unregister(&self) {
        let _ = std::fs::remove_file(self.bin().join("registered-hook"));
    }

    fn launches(&self) -> usize {
        std::fs::read_to_string(self.bin().join("launches"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    fn run(&self) -> Output {
        self.cli(&["run", &self.root(), RUN, "--json"])
    }

    fn show(&self, attempt: Option<u32>) -> Output {
        let mut args = vec![
            "startup".to_string(),
            "show".to_string(),
            self.root(),
            RUN.to_string(),
        ];
        if let Some(n) = attempt {
            args.extend(["--attempt".to_string(), n.to_string()]);
        }
        args.push("--json".to_string());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.cli(&args)
    }

    fn attempt_dir(&self, n: u32) -> PathBuf {
        self.project()
            .join(".statecraft/state/startup/runs")
            .join(RUN)
            .join(n.to_string())
    }
}

/// The fake provider. See the module comment for what it does and does not
/// stand in for.
const FAKE_PROVIDER: &str = r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267 (Claude Code)'; exit 0; fi
here="$(dirname "$0")"
settings=""
while [ "$#" -gt 0 ]; do
  case "$1" in --settings) settings="$2"; shift ;; esac
  shift
done
/usr/bin/cmp -s "$settings" "$here/expected-settings" || exit 3
/bin/cat > /dev/null
echo launched >> "$here/launches"
session=11111111-1111-1111-1111-111111111111
escape() { /usr/bin/awk 'BEGIN { ORS = "" } { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); if (NR > 1) printf "\\n"; print }'; }
respond() {
  printf '{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup","hook_event":"SessionStart","session_id":"%s"}\n' "$session"
  printf '{"type":"system","subtype":"hook_response","hook_name":"SessionStart:startup","hook_event":"SessionStart","stdout":"%s","stderr":"","exit_code":%s,"outcome":"success","session_id":"%s"}\n' "$(printf '%s' "$1" | escape)" "$2" "$session"
}
if [ -f "$here/registered-hook" ]; then
  out="$(CLAUDE_PROJECT_DIR="$PWD" "$(cat "$here/registered-hook")" "$PWD" 2>/dev/null)"
  respond "$out" "$?"
fi
if [ -f "$here/replay-line" ]; then
  respond "$(cat "$here/replay-line")" 0
fi
if [ -f "$here/block-record" ]; then
  /bin/mkdir -p "$PWD/../../startup/runs/$STATECRAFT_RUN_ID/$STATECRAFT_ATTEMPT/record.json"
fi
/bin/cat "$here/native.jsonl"
"#;

/// The whole route: initialization, the explicit requirement, a run, and the
/// judgement read back, with every identity an operator asks about.
#[test]
fn a_run_records_what_was_required_selected_observed_and_delivered() {
    let f = Fixture::new();
    let revision = f.required_revision();
    f.register(&revision);

    let out = f.run();
    // Completed. Says nothing about acceptance and nothing about qualification.
    assert_eq!(code(&out), 0, "{}", text(&out));
    let answer = json(&out);
    let startup = &answer["value"]["startup"];
    assert_eq!(startup["managed"], true, "{answer}");
    assert_eq!(startup["launched"], true);
    assert_eq!(startup["verdict"], "unverified");
    assert_eq!(startup["error"], serde_json::Value::Null);
    assert_eq!(f.launches(), 1);
    assert!(f.attempt_dir(1).join("intent.json").is_file());
    assert!(f.attempt_dir(1).join("record.json").is_file());

    let shown = f.show(None);
    assert_eq!(code(&shown), 1, "unverified is a finding: {}", text(&shown));
    let v = json(&shown)["value"]["value"].clone();
    assert_eq!(v["verdict"], "unverified");
    assert_eq!(v["launched"], true);
    let intent = &v["intent"];
    let record = &v["record"];
    let launch = &record["launch"];
    // Required, selected and observed: three different facts that agree here.
    let required = intent["requiredHarness"].as_str().unwrap();
    assert_eq!(intent["selected"]["digest"], required);
    assert_eq!(launch["harness"]["grade"], "acknowledged");
    assert_eq!(launch["harness"]["digest"], required);
    assert_eq!(record["resolvedHarness"], required);
    assert_eq!(
        Path::new(launch["harness"]["root"].as_str().unwrap()),
        revision.canonicalize().unwrap()
    );
    // The payload the run records is this build's payload, the bytes the fake
    // compared byte for byte, and the bytes the adapter says it wrote.
    let payload = json(&f.cli(&["session", "payload", "--json"]));
    let digest = payload["value"]["value"]["digest"].as_str().unwrap();
    assert_eq!(intent["payload"]["digest"], digest);
    assert_eq!(launch["settingsWritten"]["digest"], digest);
    assert_eq!(record["supply"]["supply"], "supplied");
    assert_eq!(record["delivery"]["verdict"], "reached");
    assert_eq!(record["observation"]["observation"], "not-observed");
    // Why it is not qualified, named.
    let reasons = v["reasons"].as_array().unwrap();
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(reasons[0].as_str().unwrap().contains("no live observation"));

    // The human rendering answers the same questions.
    let human = f.cli(&["startup", "show", &f.root(), RUN]);
    let h = text(&human);
    for line in [
        "launched  yes",
        "required  h-",
        "selected  h-",
        "observed  h-",
        "acknowledged by the SessionStart hook",
        "payload   ",
        "supply    supplied",
        "verdict   unverified",
        "intent.json",
        "record.json",
    ] {
        assert!(h.contains(line), "missing {line:?} in:\n{h}");
    }
    for overclaim in ["qualified", "enforced", "live"] {
        assert!(
            !h.to_lowercase().contains(&format!("verdict   {overclaim}")),
            "{h}"
        );
    }
}

/// A later attempt answered by another revision is refused, is shown as
/// mismatched, and leaves the first attempt's evidence byte for byte intact.
#[test]
fn a_later_attempt_under_another_revision_is_refused_and_disturbs_nothing() {
    let f = Fixture::new();
    let required = f.required_revision();
    f.register(&required);
    assert_eq!(code(&f.run()), 0);
    let first = std::fs::read(f.attempt_dir(1).join("record.json")).unwrap();

    // Another revision installed in the same home, which the "registration"
    // now points at: the hook that answers is not the required one.
    let other = f.home().join("harness/h-000000000000");
    std::fs::create_dir_all(other.join("hooks")).unwrap();
    let hook = std::fs::read_to_string(required.join("hooks/statecraft-session-start.sh")).unwrap();
    executable(
        &other.join("hooks/statecraft-session-start.sh"),
        &format!("{hook}# an older revision\n"),
    );
    f.register(&other);

    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let answer = json(&out);
    assert_eq!(answer["value"]["attempt"], 2);
    assert_eq!(answer["value"]["outcome"], "refused", "{answer}");
    assert_eq!(answer["value"]["startup"]["verdict"], "mismatched");

    let second = json(&f.show(Some(2)))["value"]["value"].clone();
    assert_eq!(second["verdict"], "mismatched");
    assert_ne!(
        second["record"]["resolvedHarness"],
        second["intent"]["requiredHarness"]
    );
    assert_eq!(
        std::fs::read(f.attempt_dir(1).join("record.json")).unwrap(),
        first
    );
    assert_eq!(
        json(&f.show(Some(1)))["value"]["value"]["verdict"],
        "unverified"
    );
}

/// No registered hook: nothing measured which revision answered, and the
/// record says so instead of reporting the required one.
#[test]
fn with_no_acknowledgment_the_observed_revision_is_unverified_and_not_the_required_one() {
    let f = Fixture::new();
    f.unregister();
    assert_eq!(code(&f.run()), 0);
    let v = json(&f.show(None))["value"]["value"].clone();
    assert_eq!(v["verdict"], "unverified");
    let harness = &v["record"]["launch"]["harness"];
    assert_eq!(harness["grade"], "unverified");
    assert_eq!(harness["kind"], "absent");
    assert_eq!(v["record"]["resolvedHarness"], serde_json::Value::Null);
    assert_eq!(v["record"]["standing"]["standing"], "exact");
    assert_eq!(
        v["record"]["standing"]["resolved"],
        serde_json::Value::Null,
        "a required revision recorded as resolved"
    );
}

/// An acknowledgment replayed from an earlier attempt does not qualify, and
/// does not become this attempt's observation.
#[test]
fn a_replayed_acknowledgment_does_not_become_a_later_attempts_observation() {
    let f = Fixture::new();
    f.register(&f.required_revision());
    assert_eq!(code(&f.run()), 0);
    let first = json(&f.show(Some(1)))["value"]["value"].clone();
    let intent = &first["intent"];
    let stale = format!(
        "statecraft-startup\tv1\tnonce={}\trun={}\tattempt=1\tselected={}\tproject={}\troot={}",
        intent["nonce"].as_str().unwrap(),
        RUN,
        intent["selected"]["digest"].as_str().unwrap(),
        intent["workspace"].as_str().unwrap(),
        first["record"]["launch"]["harness"]["root"]
            .as_str()
            .unwrap(),
    );
    f.unregister();
    std::fs::write(f.bin().join("replay-line"), stale).unwrap();

    assert_eq!(code(&f.run()), 0);
    let second = json(&f.show(Some(2)))["value"]["value"].clone();
    assert_eq!(second["verdict"], "unverified");
    assert_eq!(second["record"]["launch"]["harness"]["kind"], "replayed");
    assert_eq!(second["record"]["resolvedHarness"], serde_json::Value::Null);
}

/// Rule 15, first half: an intent that cannot be written launches nothing.
#[test]
fn an_intent_that_cannot_be_written_refuses_the_attempt_and_launches_nothing() {
    let f = Fixture::new();
    f.register(&f.required_revision());
    let state = f.project().join(".statecraft/state/startup");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("runs"), "not a directory").unwrap();

    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let answer = json(&out);
    assert_eq!(answer["value"]["outcome"], "refused");
    assert_eq!(answer["value"]["startup"]["launched"], false);
    assert!(
        answer["value"]["startup"]["error"]
            .as_str()
            .unwrap()
            .contains("could not be recorded"),
        "{answer}"
    );
    assert_eq!(f.launches(), 0, "a process was created with no intent");
}

/// Rule 15, second half: a launch whose record cannot be stored fails the run,
/// and the evidence that was stored is not dressed up as a complete record.
#[test]
fn a_record_that_cannot_be_stored_fails_the_run_and_reads_back_as_unreadable() {
    let f = Fixture::new();
    f.register(&f.required_revision());
    std::fs::write(f.bin().join("block-record"), "").unwrap();

    let out = f.run();
    assert_eq!(code(&out), 4, "{}", text(&out));
    let answer = json(&out);
    assert_eq!(answer["value"]["startup"]["launched"], true);
    assert_eq!(
        answer["value"]["startup"]["record"],
        serde_json::Value::Null
    );
    assert!(answer["value"]["startup"]["error"].as_str().is_some());
    assert!(f.attempt_dir(1).join("intent.json").is_file());
    // The path the record belongs at holds something that is not a record,
    // and reading it back is a failure to read, not an absence.
    let shown = f.show(Some(1));
    assert_eq!(code(&shown), 4, "{}", text(&shown));
}

/// A requirement whose content cannot be established refuses before any
/// attempt, so there is no startup evidence to read, and the read says so.
#[test]
fn a_corrupt_requirement_refuses_before_any_attempt() {
    let f = Fixture::new();
    let revision = f.required_revision();
    std::fs::write(
        revision.join("rules/statecraft-governed-work.md"),
        "changed\n",
    )
    .unwrap();
    let out = f.run();
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert_eq!(f.launches(), 0);
    let shown = f.show(None);
    assert_eq!(code(&shown), 2, "{}", text(&shown));
    assert!(text(&shown).contains("has no attempt"), "{}", text(&shown));
}
