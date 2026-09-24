//! The managed-startup trial, through the built binary, against local fakes.
//!
//! Spec 002 section 3.33 and spec 006 section 3.11.4. Every trial here is
//! given a temporary `STATECRAFT_HOME`, `STATECRAFT_NATIVE_ROOT` and `HOME`,
//! and a `PATH` holding only a fake `spec-spine`, a fake `claude` and the
//! system directories. **No provider is spawned. These tests exercise the
//! procedure; they observe nothing about a provider**, and every trial is run
//! with `--synthetic`, which the records carry.
//!
//! The fake provider reads the `SessionStart` and `PreToolUse` commands from
//! the settings document it was given and streams events in the shapes the
//! recorded Claude Code 2.1.267 streams carry: a `SessionStart` response, the
//! recorded init event, one `Read` request for the sentinel, its result, and
//! the recorded terminal event. A mode file selects how faithfully. Where a
//! mode makes the launcher refuse and stop the process, what the stream held
//! after the decision point is read or not depending on scheduling, so those
//! tests assert only what the decision fixes and say so.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TRIAL: &str = "statecraft-startup-trial";

fn executable(path: &Path, script: &str) {
    statecraft_adapter::fixture::install_script(path, script, 0o755).unwrap();
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
            .args(["-c", "maintenance.auto=false", "-c", "gc.auto=0"])
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

    fn new() -> Self {
        Self::with(true)
    }

    /// Initialization, optionally the explicit requirement, the bridge, a
    /// commit, registration and arming: the operator's route.
    fn with(require: bool) -> Self {
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
        executable(
            &f.bin().join("spec-spine"),
            "#!/bin/sh\ncase \"$*\" in\n  --version) echo 'spec-spine 0.20.0' ;;\n  check|'check --help') exit 0 ;;\n  compile|index) exit 0 ;;\n  *) exit 3 ;;\nesac\n",
        );
        let recorded = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl");
        std::fs::copy(recorded, f.bin().join("native.jsonl")).unwrap();
        executable(&f.bin().join("claude"), FAKE_PROVIDER);

        let root = f.root();
        let mut steps = vec![
            (vec!["home", "apply"], &[0, 1][..]),
            (vec!["init", "apply", &root], &[0, 1]),
        ];
        if require {
            steps.push((vec!["harness", "upgrade", &root], &[0]));
        }
        for (args, ok) in steps {
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

    fn mode(&self, mode: &str) {
        std::fs::write(self.bin().join("mode"), mode).unwrap();
    }

    fn launches(&self) -> usize {
        std::fs::read_to_string(self.bin().join("launches"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    fn trial(&self, extra: &[&str]) -> Output {
        let root = self.root();
        let mut args = vec!["startup", "trial", &root];
        args.extend_from_slice(extra);
        args.push("--json");
        self.cli(&args)
    }

    /// The trial attempt's launch records, in the product home (spec 002
    /// section 3.37 rule 1).
    fn attempt_dir(&self) -> PathBuf {
        statecraft_home::launch::AttemptIdentity {
            run_id: TRIAL.to_string(),
            attempt: 1,
        }
        .records_dir(&statecraft_home::launch::Places::of(
            &self.home(),
            &self.project(),
        ))
    }

    fn workspace(&self) -> PathBuf {
        self.project()
            .join(".statecraft/state/workspaces")
            .join(TRIAL)
    }
}

/// The fake provider: see the module comment.
const FAKE_PROVIDER: &str = r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267 (Claude Code)'; echo probed >> "$(dirname "$0")/probes"; exit 0; fi
here="$(dirname "$0")"
settings=""
while [ "$#" -gt 0 ]; do
  case "$1" in --settings) settings="$2"; shift ;; esac
  shift
done
mode="$(/bin/cat "$here/mode" 2>/dev/null || echo faithful)"
/bin/cp "$settings" "$here/received-settings"
/bin/cat > "$here/received-prompt"
echo launched >> "$here/launches"
session=11111111-1111-1111-1111-111111111111
escape() { /usr/bin/awk 'BEGIN { ORS = "" } { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); if (NR > 1) printf "\\n"; print }'; }
command_for() {
  /usr/bin/awk -v ev="\"$1\": [" 'index($0, ev) { inside = 1 } inside && /"command": / { sub(/^[^"]*"command": "/, ""); sub(/",?[ \t]*$/, ""); gsub(/\\\\/, "\001"); gsub(/\\"/, "\""); gsub(/\001/, "\\"); print; exit }' "$settings"
}
start_cmd="$(command_for SessionStart)"
gate_cmd="$(command_for PreToolUse)"
respond() {
  printf '{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup","hook_event":"SessionStart","session_id":"%s"}\n' "$session"
  printf '{"type":"system","subtype":"hook_response","hook_name":"SessionStart:startup","hook_event":"SessionStart","stdout":"%s","stderr":"","exit_code":%s,"outcome":"success","session_id":"%s"}\n' "$(printf '%s' "$1" | escape)" "$2" "$session"
}
start() { out="$(CLAUDE_PROJECT_DIR="$PWD" /bin/sh -c "$start_cmd" 2>/dev/null)"; respond "$out" "$?"; }
start_without_environment() {
  out="$(/usr/bin/env -i PATH=/usr/bin:/bin CLAUDE_PROJECT_DIR="$PWD" /bin/sh -c "$start_cmd" 2>/dev/null)"
  respond "$out" "$?"
}
init() { /usr/bin/sed -n '3p' "$here/native.jsonl"; }
finish() { /usr/bin/sed -n '4,$p' "$here/native.jsonl"; }
request() {
  printf '{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_trial","name":"Read","input":{"file_path":"%s/STATECRAFT-TRIAL-SENTINEL"}}]},"session_id":"%s"}\n' "$PWD" "$session"
}
consult() { printf '{"tool_name":"Read","tool_input":{}}' | /bin/sh -c "$gate_cmd" 2>> "$here/gate-stderr"; }
execute() {
  content="$(/usr/bin/tr -d '\n' < "$PWD/STATECRAFT-TRIAL-SENTINEL")"
  printf '{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_trial","content":"%s","is_error":false}]},"session_id":"%s"}\n' "$content" "$session"
}
blocked() {
  printf '{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_trial","content":"blocked by a PreToolUse hook","is_error":true}]},"session_id":"%s"}\n' "$session"
}
gated() { if consult; then execute; else blocked; fi; }
case "$mode" in
  faithful) start; init; request; gated; finish ;;
  gate-ignored) start; init; request; execute; finish ;;
  no-tool) start; init; finish ;;
  no-environment) start_without_environment; init; request; gated; finish ;;
  late-hook) init; start; request; gated; finish ;;
  tool-before-init) start; request; gated; init; finish ;;
  ignores-hooks) request; execute; init; finish ;;
  hangs) start; init; request; gated; /bin/sleep 60 & /bin/sleep 60 ;;
  silent) /bin/sleep 60 & /bin/sleep 60 ;;
esac
"#;

fn trial_of(out: &Output) -> serde_json::Value {
    json(out)["value"]["startup"]["trial"].clone()
}

/// A faithful fake: the hook is correlated before the decision, the gate is
/// consulted and releases the read after admission, the sentinel's nonce comes
/// back, and the trial is established, as a statement about the procedure.
#[test]
fn a_faithful_synthetic_trial_is_established_and_says_it_is_synthetic() {
    let f = Fixture::new();
    let out = f.trial(&["--synthetic"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let t = trial_of(&out);
    assert_eq!(t["agrees"], true, "{t}");
    let j = &t["judgement"];
    assert_eq!(j["verdict"], "established", "{j}");
    assert_eq!(j["origin"], "synthetic");
    assert_eq!(j["hook"]["word"], "correlated");
    assert_eq!(j["effects"], "excluded");
    assert_eq!(j["sentinelRead"], true);
    assert_eq!(j["toolRequests"], 1);
    assert_eq!(j["consultations"], 1);
    assert_eq!(j["released"], 1);
    assert!(j["scope"].as_str().unwrap().contains("tool calls only"));
    // One session and one version probe: the budget.
    assert_eq!(f.launches(), 1);
    assert_eq!(
        std::fs::read_to_string(f.bin().join("probes"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    // The records, all of them, and the settings bytes kept verbatim.
    for file in [
        "intent.json",
        "launched.json",
        "admission.json",
        "record.json",
        "gate.log",
        "trial.json",
    ] {
        assert!(f.attempt_dir().join(file).is_file(), "{file}");
    }
    assert!(!f.project().join(".statecraft/state/startup").exists());
    // Section 3.37 rule 3: a trial recorded now says its consultations are
    // the child's.
    assert_eq!(t["record"]["consultations"], "child-attested", "{t}");
    let facts = &t["record"]["facts"];
    assert_eq!(
        facts["settingsWritten"].as_str().unwrap().as_bytes(),
        std::fs::read(f.bin().join("received-settings")).unwrap()
    );
    assert_eq!(facts["decision"]["event"], "init");
    assert_eq!(facts["probedVersion"], "2.1.267");
    assert_eq!(facts["maxTurns"], 3);
    // The prompt delivered is the trial's, and it names the sentinel.
    let prompt = std::fs::read_to_string(f.bin().join("received-prompt")).unwrap();
    assert!(prompt.contains("STATECRAFT-TRIAL-SENTINEL"), "{prompt}");
    assert!(f.workspace().join("STATECRAFT-TRIAL-SENTINEL").is_file());

    // `startup show` renders the same judgement, recomputed from disk.
    let human = f.cli(&["startup", "show", &f.root(), TRIAL]);
    let h = text(&human);
    for line in [
        "trial     established (synthetic): a local fake",
        "hook evidence  correlated",
        "effects        excluded before admission; scope: tool calls only",
        "sentinel read after admission",
        "consultations  child-attested",
    ] {
        assert!(h.contains(line), "missing {line:?} in:\n{h}");
    }
    assert!(!h.contains("RELOAD"), "{h}");

    // Spec 004 section 3.17: the trial has no spec, so its coverage is
    // `not-applicable` and nothing was compared, and the record says so.
    let (chain, _) = statecraft_run::record::Chain::open(&f.home(), &f.project()).unwrap();
    let outcome = chain
        .entries()
        .into_iter()
        .rfind(|e| e.subject == "attempt")
        .expect("the trial's outcome");
    let coverage = &outcome.detail["posture"]["coverage"];
    assert_eq!(coverage["verdict"], "not-applicable", "{coverage}");
    assert_eq!(coverage["commands"], serde_json::json!([]));
    assert!(coverage["spec"].is_null());
}

/// The budget is spent once. A second trial in the same project is refused
/// before anything is prepared, and the fake was launched once.
#[test]
fn a_trial_happens_once_per_project_and_is_never_replayed() {
    let f = Fixture::new();
    assert_eq!(code(&f.trial(&["--synthetic"])), 0);
    let again = f.trial(&["--synthetic"]);
    assert_eq!(code(&again), 2, "{}", text(&again));
    assert!(text(&again).contains("trial is spent"), "{}", text(&again));
    assert_eq!(f.launches(), 1);
}

/// Provider execution is a stated act: neither flag, both flags, an
/// over-long deadline, a project with no requirement, an unarmed project and
/// a sentinel already present are each refused, and nothing is launched.
#[test]
fn every_refusal_comes_before_an_attempt_and_launches_nothing() {
    let f = Fixture::new();
    for (args, needle) in [
        (vec![], "--provider-session or --synthetic"),
        (vec!["--synthetic", "--provider-session"], "contradict"),
        (
            vec!["--synthetic", "--deadline", "301"],
            "between 1 and 300",
        ),
    ] {
        let out = f.trial(&args);
        assert_eq!(code(&out), 2, "{args:?}: {}", text(&out));
        assert!(text(&out).contains(needle), "{args:?}: {}", text(&out));
    }
    std::fs::write(f.project().join("STATECRAFT-TRIAL-SENTINEL"), "x\n").unwrap();
    let out = f.trial(&["--synthetic"]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    std::fs::remove_file(f.project().join("STATECRAFT-TRIAL-SENTINEL")).unwrap();
    let out = f.cli(&["project", "disarm", &f.root()]);
    assert!(code(&out) <= 1);
    assert_eq!(code(&f.trial(&["--synthetic"])), 2);
    assert_eq!(f.launches(), 0);
    assert!(!f.attempt_dir().exists());

    let unrequired = Fixture::with(false);
    let out = unrequired.trial(&["--synthetic"]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("commits no harness requirement"));
    assert_eq!(unrequired.launches(), 0);
}

/// A provider that runs the startup hook and is admitted, and then runs the
/// read without consulting the gate: the execution was not held by anything,
/// which is `demonstrated-possible`, and the trial is not established.
#[test]
fn a_read_that_never_consulted_the_gate_demonstrates_effects_were_possible() {
    let f = Fixture::new();
    f.mode("gate-ignored");
    let out = f.trial(&["--synthetic"]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    let j = &trial_of(&out)["judgement"];
    assert_eq!(j["verdict"], "not-established");
    assert_eq!(j["hook"]["word"], "correlated");
    assert_eq!(j["decision"], "admitted");
    assert_eq!(j["effects"], "demonstrated-possible");
    assert_eq!(j["consultations"], 0);
    assert!(
        j["reasons"]
            .to_string()
            .contains("did not consult the gate")
    );
}

/// A session that requests no tool leaves nothing for the gate to hold, so
/// effects are unobserved and the trial is not established.
#[test]
fn a_session_that_requests_no_tool_leaves_effects_unobserved() {
    let f = Fixture::new();
    f.mode("no-tool");
    let out = f.trial(&["--synthetic"]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    let j = &trial_of(&out)["judgement"];
    assert_eq!(j["verdict"], "not-established");
    assert_eq!(j["effects"], "unobserved");
    assert!(j["reasons"].to_string().contains("requested no tool"));
}

/// Premise P2 failing: the hook runs without the session's environment, so
/// its output carries no binding. Refused at the decision; `unbound`.
#[test]
fn a_hook_run_without_the_session_environment_is_unbound() {
    let f = Fixture::new();
    f.mode("no-environment");
    let out = f.trial(&["--synthetic"]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    let j = &trial_of(&out)["judgement"];
    assert_eq!(j["hook"]["word"], "unbound", "{j}");
    assert_eq!(j["decision"], "not-established");
    assert_eq!(j["sentinelRead"], false);
    assert_eq!(j["verdict"], "not-established");
}

/// Premise P1 failing late: the startup hook reports after `init`. The
/// decision is made at `init` and refuses; it never waits. What the stream
/// held after the refusal stopped the process depends on scheduling, so the
/// hook evidence is `late` when the response was read and `absent` when it
/// was not; either way the read never ran after admission.
#[test]
fn a_startup_hook_reported_after_init_is_refused_at_init_and_never_waited_for() {
    let f = Fixture::new();
    f.mode("late-hook");
    let out = f.trial(&["--synthetic"]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    let t = trial_of(&out);
    let j = &t["judgement"];
    assert!(
        ["late", "absent"].contains(&j["hook"]["word"].as_str().unwrap()),
        "{j}"
    );
    assert_eq!(j["decision"], "not-established");
    assert_eq!(t["record"]["facts"]["decision"]["event"], "init");
    assert_eq!(j["sentinelRead"], false);
    assert_eq!(j["verdict"], "not-established");
}

/// Premise P5 failing: a tool request is streamed before `init`. The
/// decision is made at that request, with no init session read, the
/// acknowledgment is judged `wrong-session`, and the attempt is refused. Fail
/// closed, and the event the decision was made at is recorded.
#[test]
fn a_tool_request_streamed_before_init_decides_there_and_fails_closed() {
    let f = Fixture::new();
    f.mode("tool-before-init");
    let out = f.trial(&["--synthetic"]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    let t = trial_of(&out);
    let j = &t["judgement"];
    assert_eq!(t["record"]["facts"]["decision"]["event"], "tool-use", "{t}");
    assert_eq!(
        j["hook"],
        serde_json::json!({"word": "unbound", "kind": "wrong-session"})
    );
    assert_eq!(j["decision"], "not-established");
    assert_eq!(j["sentinelRead"], false);
}

/// A provider that ignores the registration: no hook response anywhere, and
/// the decision refuses at its first event. The execution it streamed next
/// is read or not depending on when the stop landed; if it was read, effects
/// are `demonstrated-possible`, and if not, `unobserved`. Never `excluded`.
#[test]
fn a_provider_that_ignores_the_registration_has_absent_hook_evidence() {
    let f = Fixture::new();
    f.mode("ignores-hooks");
    let out = f.trial(&["--synthetic"]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    let j = &trial_of(&out)["judgement"];
    assert_eq!(j["hook"]["word"], "absent", "{j}");
    assert_eq!(j["decision"], "not-established");
    assert_ne!(j["effects"], "excluded");
    assert_eq!(j["verdict"], "not-established");
}

/// A session that never ends is stopped at the deadline with its descendant,
/// the trial is `uncertain`, the evidence stays, and nothing is replayed.
#[test]
fn a_session_stopped_at_its_deadline_is_uncertain_and_its_evidence_stays() {
    let f = Fixture::new();
    f.mode("hangs");
    let started = std::time::Instant::now();
    let out = f.trial(&["--synthetic", "--deadline", "3"]);
    assert!(started.elapsed() < std::time::Duration::from_secs(45));
    assert_eq!(code(&out), 1, "{}", text(&out));
    let t = trial_of(&out);
    let j = &t["judgement"];
    assert_eq!(j["verdict"], "uncertain", "{j}");
    assert!(j["reasons"][0].as_str().unwrap().contains("deadline"));
    assert_eq!(
        t["record"]["facts"]["process"]["surviving"],
        serde_json::Value::Null
    );
    assert!(f.attempt_dir().join("trial.json").is_file());
    assert_eq!(code(&f.trial(&["--synthetic"])), 2);
    assert_eq!(f.launches(), 1);
}

/// A session the deadline stops before it writes a single event is uncertain,
/// not a completed launch that failed: the adapter's missing-init error does
/// not hide that the deadline stopped it (spec 002 section 3.33 rule 37). The
/// fake writes nothing at all, so this holds however slowly it is scheduled.
#[test]
fn a_session_stopped_before_its_first_event_is_uncertain_not_failed() {
    let f = Fixture::new();
    f.mode("silent");
    let out = f.trial(&["--synthetic", "--deadline", "3"]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    let t = trial_of(&out);
    let j = &t["judgement"];
    assert_eq!(j["verdict"], "uncertain", "{j}");
    assert!(
        j["reasons"][0].as_str().unwrap().contains("deadline"),
        "{j}"
    );
    assert_eq!(t["record"]["facts"]["process"]["timedOut"], true);
    assert_eq!(f.launches(), 1);
}

/// A trial record edited after it was written does not survive being read:
/// the judgement recomputed from disk differs, and `startup show` says so.
#[test]
fn a_trial_record_edited_after_writing_reads_back_as_disagreeing() {
    let f = Fixture::new();
    f.mode("no-tool");
    assert_eq!(code(&f.trial(&["--synthetic"])), 1);
    let path = f.attempt_dir().join("trial.json");
    let edited = std::fs::read_to_string(&path).unwrap().replacen(
        "\"not-established\"",
        "\"established\"",
        1,
    );
    std::fs::write(&path, edited).unwrap();
    let shown = f.cli(&["startup", "show", &f.root(), TRIAL]);
    let h = text(&shown);
    assert!(h.contains("RELOAD"), "{h}");
    assert!(h.contains("trial     not-established"), "{h}");
}
