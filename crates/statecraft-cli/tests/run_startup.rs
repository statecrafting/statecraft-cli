//! A run's startup evidence, through the built binary, from initialization to
//! the judgement an operator reads.
//!
//! Spec 002 sections 3.31 and 3.32 and spec 006 section 3.11.3. Every run is
//! given a temporary `STATECRAFT_HOME`, `STATECRAFT_NATIVE_ROOT` and `HOME`, and
//! a `PATH` holding only a fake `spec-spine`, a fake `claude` and the system
//! directories. **No provider is spawned, and nothing here is live evidence.**
//!
//! The fake provider does what section 3.32 needs a provider to do and nothing
//! more. It keeps the settings bytes, environment and prompt it was given, so
//! a test can compare them with the record. It reads the `SessionStart` and
//! `PreToolUse` commands **from the settings document it was given**, the way
//! a provider honoring `--settings` hooks would, runs the first and reports its
//! output as a `hook_response` in the shape the recorded Claude Code 2.1.267
//! streams carry, emits the recorded session up to its init event, and then
//! attempts one harmless tool call, a file named `sentinel-after` in its
//! workspace, through the second. Whether the live provider does any of this
//! with hooks it was given this way is the premise section 3.32 rule 25 records
//! as unobserved. A mode file selects the deviations: a tool call begun before
//! the decision, a provider that ignores the registration, a substituted
//! startup hook, and a launcher killed mid-session.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RUN: &str = "replay";

fn executable(path: &Path, script: &str) {
    // Staged and copied, never written in place: a script this process wrote
    // and then exec'd is the `ETXTBSY` race spec 004 records.
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
        Self::with(true)
    }

    /// The same route, with or without the explicit requirement.
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

    /// Select one of the fake provider's deviations.
    fn mode(&self, mode: &str) {
        std::fs::write(self.bin().join("mode"), mode).unwrap();
    }

    /// Make the fake run this script in place of the `SessionStart` command
    /// it was given, as a provider honoring some other registration would.
    fn substitute_start_hook(&self, script: &Path) {
        std::fs::write(
            self.bin().join("substitute-hook"),
            script.display().to_string(),
        )
        .unwrap();
    }

    fn received(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.bin().join(name)).unwrap_or_default()
    }

    /// What the fake did, in order.
    fn order(&self) -> Vec<String> {
        String::from_utf8(self.received("order"))
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn workspace(&self) -> PathBuf {
        self.project()
            .join(".statecraft/state/workspaces")
            .join(RUN)
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
mode="$(/bin/cat "$here/mode" 2>/dev/null || echo faithful)"
/bin/cp "$settings" "$here/received-settings"
/usr/bin/env > "$here/received-env"
/bin/cat > "$here/received-prompt"
echo launched >> "$here/launches"
log() { echo "$1" >> "$here/order"; }
session=11111111-1111-1111-1111-111111111111
escape() { /usr/bin/awk 'BEGIN { ORS = "" } { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); if (NR > 1) printf "\\n"; print }'; }
respond() {
  printf '{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup","hook_event":"SessionStart","session_id":"%s"}\n' "$session"
  printf '{"type":"system","subtype":"hook_response","hook_name":"SessionStart:startup","hook_event":"SessionStart","stdout":"%s","stderr":"","exit_code":%s,"outcome":"success","session_id":"%s"}\n' "$(printf '%s' "$1" | escape)" "$2" "$session"
}
# The first command registered for an event, read from the document given.
command_for() {
  /usr/bin/awk -v ev="\"$1\": [" 'index($0, ev) { inside = 1 } inside && /"command": / { sub(/^[^"]*"command": "/, ""); sub(/",?[ \t]*$/, ""); gsub(/\\\\/, "\001"); gsub(/\\"/, "\""); gsub(/\001/, "\\"); print; exit }' "$settings"
}
start_cmd="$(command_for SessionStart)"
gate_cmd="$(command_for PreToolUse)"
# One tool call: through the registered gate, then its effect if released.
tool() {
  rc=0
  if [ -n "$gate_cmd" ] && [ "$mode" != ignores-hooks ]; then
    printf '{"tool_name":"Bash","tool_input":{"command":"touch %s"}}' "$1" | /bin/sh -c "$gate_cmd" 2>> "$here/gate-stderr"
    rc=$?
  fi
  log "gate $1 exit=$rc"
  if [ "$rc" = 0 ]; then : > "$PWD/$1"; log "effect $1"; fi
}
if [ "$mode" != ignores-hooks ] && [ ! -f "$here/skip-start-hook" ]; then
  if [ -f "$here/substitute-hook" ]; then start_cmd="'$(/bin/cat "$here/substitute-hook")'"; fi
  if [ -n "$start_cmd" ]; then
    out="$(CLAUDE_PROJECT_DIR="$PWD" /bin/sh -c "$start_cmd" 2>/dev/null)"
    respond "$out" "$?"
  fi
fi
if [ -f "$here/replay-line" ]; then respond "$(/bin/cat "$here/replay-line")" 0; fi
if [ -f "$here/block-record" ]; then
  /bin/mkdir -p "$PWD/../../startup/runs/$STATECRAFT_RUN_ID/$STATECRAFT_ATTEMPT/record.json"
fi
if [ "$mode" = ignores-hooks ]; then
  : > "$PWD/sentinel-before-decision"
  log "effect sentinel-before-decision"
fi
if [ "$mode" = early-tool ]; then
  tool sentinel-early &
  early=$!
  /bin/sleep 0.5
  if kill -0 "$early" 2>/dev/null; then log "gate waiting before init"; fi
fi
/usr/bin/sed -n '1,3p' "$here/native.jsonl"
log "init emitted"
if [ -n "${early:-}" ]; then wait "$early"; fi
if [ "$mode" = block ]; then
  log blocked
  while [ ! -f "$here/release" ]; do /bin/sleep 0.05; done
fi
tool sentinel-after
/usr/bin/sed -n '4,$p' "$here/native.jsonl"
"#;

fn value(out: &Output) -> serde_json::Value {
    json(out)["value"]["value"].clone()
}

fn position(order: &[String], line: &str) -> usize {
    order
        .iter()
        .position(|l| l == line)
        .unwrap_or_else(|| panic!("{line:?} is not in {order:?}"))
}

/// The whole route: initialization, the explicit requirement, a run whose
/// startup hook and gate the run supplied itself, and the judgement read back,
/// with every identity an operator asks about compared with what the provider
/// was actually given.
#[test]
fn a_managed_run_supplies_its_startup_hook_and_gate_and_releases_work_on_admission() {
    let f = Fixture::new();
    let revision = f.required_revision();

    let out = f.run();
    // Completed. Says nothing about acceptance and nothing about qualification.
    assert_eq!(code(&out), 0, "{}", text(&out));
    let answer = json(&out);
    let startup = &answer["value"]["startup"];
    assert_eq!(startup["managed"], true, "{answer}");
    assert_eq!(startup["launched"], true);
    assert_eq!(startup["verdict"], "unverified");
    assert!(
        startup["admission"]
            .as_str()
            .unwrap()
            .starts_with("admitted")
    );
    assert_eq!(startup["error"], serde_json::Value::Null);
    assert_eq!(f.launches(), 1);
    for file in [
        "intent.json",
        "launched.json",
        "admission.json",
        "record.json",
    ] {
        assert!(f.attempt_dir(1).join(file).is_file(), "{file}");
    }

    let shown = f.show(None);
    assert_eq!(code(&shown), 1, "unverified is a finding: {}", text(&shown));
    let v = value(&shown);
    assert_eq!(v["verdict"], "unverified");
    assert_eq!(v["spawn"], "confirmed");
    let intent = &v["intent"];
    let record = &v["record"];
    let launch = &record["launch"];
    // Required, selected and correlated: three different facts that agree.
    let required = intent["requiredHarness"].as_str().unwrap();
    assert_eq!(intent["selected"]["digest"], required);
    assert_eq!(launch["harness"]["grade"], "correlated");
    assert_eq!(launch["harness"]["digest"], required);
    assert_eq!(record["resolvedHarness"], required);
    assert_eq!(
        Path::new(launch["harness"]["root"].as_str().unwrap()),
        revision.canonicalize().unwrap()
    );
    assert_eq!(v["admission"]["decision"], "admitted");
    assert_eq!(launch["admission"], v["admission"]);
    assert_eq!(launch["process"]["pid"], v["launched"]["pid"]);
    assert_eq!(launch["process"]["confirmed"], true);

    // What the provider was given is what the record says it was given: the
    // settings bytes exactly, the attempt binding in its environment, and a
    // prompt.
    let document = intent["settingsDocument"].as_str().unwrap();
    assert_eq!(f.received("received-settings"), document.as_bytes());
    let digest = intent["payload"]["digest"].as_str().unwrap();
    assert_eq!(launch["settingsWritten"]["digest"], digest);
    let floor = json(&f.cli(&["session", "payload", "--json"]));
    let floor = floor["value"]["value"]["digest"].as_str().unwrap();
    assert_eq!(intent["payload"]["floorDigest"], floor);
    assert_ne!(digest, floor, "a run's document is not the floor's bytes");
    let env = String::from_utf8(f.received("received-env")).unwrap();
    for (name, want) in [
        ("STATECRAFT_RUN_ID", RUN.to_string()),
        ("STATECRAFT_ATTEMPT", "1".to_string()),
        (
            "STATECRAFT_STARTUP_NONCE",
            intent["nonce"].as_str().unwrap().to_string(),
        ),
        ("STATECRAFT_HARNESS_SELECTED", required.to_string()),
    ] {
        assert!(
            env.lines().any(|l| l == format!("{name}={want}")),
            "{name}: {env}"
        );
    }
    assert!(!f.received("received-prompt").is_empty());
    // The registrations name the selected revision's hook and this attempt's
    // gate, and nothing in the operator's home was written to make that so.
    let registrations = intent["registrations"].as_array().unwrap();
    assert_eq!(registrations.len(), 2);
    assert_eq!(
        Path::new(registrations[0]["script"].as_str().unwrap()),
        revision.join("hooks/statecraft-session-start.sh")
    );
    assert!(!f.dir.path().join(".claude/settings.json").exists());

    // The tool call ran after the decision, through the gate.
    let order = f.order();
    assert!(position(&order, "init emitted") < position(&order, "effect sentinel-after"));
    assert!(f.workspace().join("sentinel-after").exists());
    assert_eq!(v["gate"], serde_json::json!(["admitted"]));

    assert_eq!(record["supply"]["supply"], "supplied");
    assert_eq!(record["delivery"]["verdict"], "reached");
    assert_eq!(record["observation"]["observation"], "not-observed");
    let reasons = v["reasons"].as_array().unwrap();
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(reasons[0].as_str().unwrap().contains("no live observation"));

    // The human rendering answers the same questions, in the corrected words.
    let human = f.cli(&["startup", "show", &f.root(), RUN]);
    let h = text(&human);
    for line in [
        "spawn     confirmed: process ",
        "required  h-",
        "selected  h-",
        "hook      SessionStart startup -> ",
        "hook      PreToolUse * -> ",
        "admission admitted at ",
        "gate      consulted 1 time(s): admitted",
        "observed  h-",
        "correlated",
        "which process printed it is not established",
        "settings  ",
        "the deny floor alone is ",
        "supply    supplied",
        "verdict   unverified",
        "launched.json",
        "admission.json",
    ] {
        assert!(h.contains(line), "missing {line:?} in:\n{h}");
    }
    for overclaim in ["executed", "acknowledged by", "verdict   qualified"] {
        assert!(!h.contains(overclaim), "{overclaim:?} in:\n{h}");
    }
}

/// A tool call begun before the startup decision waits at the gate, and runs
/// only after the decision admitted it.
#[test]
fn a_tool_call_begun_before_the_decision_waits_for_it_and_runs_after_admission() {
    let f = Fixture::new();
    f.mode("early-tool");
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let order = f.order();
    let waiting = position(&order, "gate waiting before init");
    let init = position(&order, "init emitted");
    let effect = position(&order, "effect sentinel-early");
    assert!(waiting < init && init < effect, "{order:?}");
    let v = value(&f.show(None));
    assert_eq!(v["admission"]["decision"], "admitted");
    assert!(v["admission"]["atLine"].as_u64().is_some());
    assert_eq!(v["gate"], serde_json::json!(["admitted", "admitted"]));
}

/// A revision other than the required one answers: governed work is refused
/// at the decision, the tool call begun before it never runs, the process is
/// stopped, and an earlier attempt's evidence is untouched.
#[test]
fn a_mismatched_revision_is_refused_before_its_tool_calls_run() {
    let f = Fixture::new();
    let required = f.required_revision();
    assert_eq!(code(&f.run()), 0);
    let first = std::fs::read(f.attempt_dir(1).join("record.json")).unwrap();

    // Another revision installed in the same home, whose startup hook the
    // fake runs in place of the one it was given.
    let other = f.home().join("harness/h-000000000000");
    std::fs::create_dir_all(other.join("hooks")).unwrap();
    let hook = std::fs::read_to_string(required.join("hooks/statecraft-session-start.sh")).unwrap();
    let substitute = other.join("hooks/statecraft-session-start.sh");
    executable(&substitute, &format!("{hook}# an older revision\n"));
    f.substitute_start_hook(&substitute);
    f.mode("early-tool");
    let _ = std::fs::remove_file(f.bin().join("order"));
    let _ = std::fs::remove_file(f.workspace().join("sentinel-after"));

    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let answer = json(&out);
    assert_eq!(answer["value"]["attempt"], 2);
    assert_eq!(answer["value"]["outcome"], "refused", "{answer}");
    assert_eq!(answer["value"]["startup"]["verdict"], "mismatched");

    // The boundary, not the label: a tool call was waiting before the
    // decision, and neither it nor any later one produced its effect.
    let order = f.order();
    assert!(position(&order, "gate waiting before init") < position(&order, "init emitted"));
    assert!(!order.iter().any(|l| l.starts_with("effect")), "{order:?}");
    assert!(!f.workspace().join("sentinel-early").exists());
    assert!(!f.workspace().join("sentinel-after").exists());

    let second = value(&f.show(Some(2)));
    assert_eq!(second["verdict"], "mismatched");
    assert_eq!(second["admission"]["decision"], "mismatched");
    assert!(
        second["record"]["launch"]["process"]["stopped"]
            .as_str()
            .unwrap()
            .contains("refused governed work")
    );
    assert_ne!(
        second["record"]["resolvedHarness"],
        second["intent"]["requiredHarness"]
    );
    let reasons = second["reasons"].to_string();
    assert!(reasons.contains("stopped the process group"), "{reasons}");
    assert!(reasons.contains("does not prove"), "{reasons}");
    assert_eq!(
        std::fs::read(f.attempt_dir(1).join("record.json")).unwrap(),
        first
    );
    assert_eq!(value(&f.show(Some(1)))["verdict"], "unverified");
}

/// A provider that ignores the registration runs neither the acknowledgment
/// nor the gate. The decision refuses, the process is stopped, and the
/// refusal says it is retrospective: the effect the provider produced before
/// the decision exists, and the record does not pretend otherwise.
#[test]
fn a_provider_that_ignores_the_registration_is_refused_after_the_fact_and_says_so() {
    let f = Fixture::new();
    f.mode("ignores-hooks");
    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    assert_eq!(json(&out)["value"]["outcome"], "refused");
    assert!(f.workspace().join("sentinel-before-decision").exists());
    let v = value(&f.show(None));
    assert_eq!(v["verdict"], "not-admitted");
    assert_eq!(v["admission"]["decision"], "not-established");
    assert_eq!(v["record"]["launch"]["harness"]["kind"], "absent");
    assert_eq!(v["record"]["launch"]["gate"], serde_json::json!([]));
    let reasons = v["reasons"].to_string();
    assert!(reasons.contains("never consulted"), "{reasons}");
    assert!(reasons.contains("not excluded"), "{reasons}");
    assert!(reasons.contains("retrospective"), "{reasons}");
    assert!(v["next"].as_str().unwrap().contains("for effects"));
}

/// No acknowledgment: nothing correlated which revision answered, the
/// required one is not recorded as the resolved one, and work is withheld.
#[test]
fn with_no_acknowledgment_nothing_is_resolved_and_work_is_withheld() {
    let f = Fixture::new();
    std::fs::write(f.bin().join("skip-start-hook"), "").unwrap();
    assert_eq!(code(&f.run()), 1);
    assert!(!f.workspace().join("sentinel-after").exists());
    let v = value(&f.show(None));
    assert_eq!(v["verdict"], "not-admitted");
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

/// An acknowledgment replayed from an earlier attempt does not become this
/// attempt's, and releases nothing.
#[test]
fn a_replayed_acknowledgment_does_not_become_a_later_attempts_observation() {
    let f = Fixture::new();
    assert_eq!(code(&f.run()), 0);
    let first = value(&f.show(Some(1)));
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
    std::fs::write(f.bin().join("skip-start-hook"), "").unwrap();
    std::fs::write(f.bin().join("replay-line"), stale).unwrap();

    assert_eq!(code(&f.run()), 1);
    let second = value(&f.show(Some(2)));
    assert_eq!(second["verdict"], "not-admitted");
    assert_eq!(second["record"]["launch"]["harness"]["kind"], "replayed");
    assert_eq!(second["record"]["resolvedHarness"], serde_json::Value::Null);
}

/// A project that commits no requirement selects nothing, registers nothing,
/// is given the floor alone, and its work is not gated by a decision.
#[test]
fn an_unrequired_project_is_given_the_floor_alone_and_is_not_gated() {
    let f = Fixture::with(false);
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let floor = statecraft_home::session::payload_json();
    assert_eq!(f.received("received-settings"), floor.as_bytes());
    let v = value(&f.show(None));
    assert_eq!(v["intent"]["selected"], serde_json::Value::Null);
    assert_eq!(v["admission"]["decision"], "not-gated");
    assert_eq!(v["gate"], serde_json::Value::Null);
    assert!(!f.attempt_dir(1).join("admission-gate").exists());
    assert!(f.workspace().join("sentinel-after").exists());
    assert_eq!(v["verdict"], "unverified");
}

/// The launcher dies mid-session. What is on disk is a confirmed spawn and a
/// decision; the outcome is unknown, not interrupted; and the next run is
/// refused rather than replayed, naming what to inspect.
#[test]
fn a_launcher_killed_mid_session_leaves_the_outcome_unknown_and_nothing_is_replayed() {
    let f = Fixture::new();
    f.mode("block");
    let mut launcher = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
        .args(["run", &f.root(), RUN, "--json"])
        .env_clear()
        .env("STATECRAFT_HOME", f.home())
        .env("STATECRAFT_NATIVE_ROOT", f.dir.path().join("native"))
        .env("HOME", f.dir.path())
        .env("PATH", format!("{}:/usr/bin:/bin", f.bin().display()))
        .env("USER", "fixture-operator")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    while !(f.order().iter().any(|l| l == "blocked")
        && f.attempt_dir(1).join("admission.json").is_file())
    {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(60),
            "the fake never blocked: {:?}",
            f.order()
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    launcher.kill().unwrap();
    launcher.wait().unwrap();
    let launched: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.attempt_dir(1).join("launched.json")).unwrap())
            .unwrap();
    let pid = launched["pid"].as_u64().unwrap().to_string();

    let shown = f.show(None);
    assert_eq!(code(&shown), 1, "{}", text(&shown));
    let v = value(&shown);
    assert_eq!(v["outcome"], serde_json::Value::Null);
    assert_eq!(v["verdict"], "outcome-unknown");
    assert!(v["reasons"][0].as_str().unwrap().contains(&pid));
    assert!(!v["reasons"].to_string().contains("interrupted"));
    assert!(!f.attempt_dir(1).join("record.json").exists());

    let again = f.run();
    assert_eq!(code(&again), 2, "{}", text(&again));
    let refusal = json(&again)["value"].clone();
    assert_eq!(refusal["launchState"], "outcome-unknown");
    assert!(refusal["next"].as_str().unwrap().contains("startup show"));
    assert!(refusal["next"].as_str().unwrap().contains(&pid));
    assert_eq!(f.launches(), 1, "an uncertain launch was replayed");

    // Clean up the provider this test orphaned on purpose.
    let _ = Command::new("kill")
        .args(["-s", "KILL", "--", &format!("-{pid}")])
        .output();
    std::fs::write(f.bin().join("release"), "").unwrap();
}

/// Rule 15, first half: an intent that cannot be written launches nothing.
#[test]
fn an_intent_that_cannot_be_written_refuses_the_attempt_and_launches_nothing() {
    let f = Fixture::new();
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
