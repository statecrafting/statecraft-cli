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
  'verify '*' --plan --json') printf '{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"%s"},"schemaVersion":"0.6.0","verb":"verify"}' $2 ;;
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

    fn places(&self) -> statecraft_home::launch::Places {
        statecraft_home::launch::Places::of(&self.home(), &self.project())
    }

    fn identity(n: u32) -> statecraft_home::launch::AttemptIdentity {
        statecraft_home::launch::AttemptIdentity {
            run_id: RUN.to_string(),
            attempt: n,
        }
    }

    /// The attempt's launch records, in the product home (spec 002 section
    /// 3.37 rule 1).
    fn attempt_dir(&self, n: u32) -> PathBuf {
        Self::identity(n).records_dir(&self.places())
    }

    /// The attempt's exchange directory, in the product home (rule 2).
    fn exchange_dir(&self, n: u32) -> PathBuf {
        Self::identity(n).exchange_dir(&self.places())
    }

    /// Where section 3.32 put an attempt's records, inside the target
    /// (rule 5).
    fn legacy_dir(&self, n: u32) -> PathBuf {
        Self::identity(n).legacy_dir(&self.project())
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
echo "$settings" > "$here/received-settings-path"
/bin/ls -A "$(dirname "$settings")" > "$here/exchange-listing"
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
  /bin/mkdir -p "$(/bin/cat "$here/block-record")"
fi
# A child writing the decision before the supervisor does (spec 002 section
# 3.37 rule 2), which the confinement of spec 004 section 3.18 will refuse and
# this build does not yet: the gate's copy is beside the settings file.
if [ "$mode" = plant-decision ]; then
  printf '{"decision":"admitted"}\n' > "$(dirname "$settings")/admission.json"
  log "planted"
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
if [ "$mode" = tool-then-block ]; then
  tool sentinel-released
fi
if [ "$mode" = block ] || [ "$mode" = tool-then-block ]; then
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
        "gate.log",
    ] {
        assert!(f.attempt_dir(1).join(file).is_file(), "{file}");
    }
    // Spec 002 section 3.37: the launch records are in the product home, keyed
    // as the run record is, and nothing of the attempt is in the target.
    assert!(f.attempt_dir(1).starts_with(f.home().join("records")));
    assert!(!f.legacy_dir(1).exists());
    assert!(!f.project().join(".statecraft/state/startup").exists());
    // The child was given its exchange directory: the gate, the gate log
    // created empty before launch, and the settings document it was handed.
    let given = String::from_utf8(f.received("received-settings-path")).unwrap();
    let given = Path::new(given.trim());
    assert_eq!(
        given.parent().unwrap(),
        f.exchange_dir(1).canonicalize().unwrap()
    );
    let listing = String::from_utf8(f.received("exchange-listing")).unwrap();
    let mut listing: Vec<&str> = listing.lines().collect();
    listing.sort_unstable();
    assert_eq!(listing.len(), 3, "{listing:?}");
    assert_eq!(&listing[..2], ["admission-gate", "gate.log"]);
    assert!(
        listing[2].starts_with("statecraft-settings-"),
        "{listing:?}"
    );
    // After the run: the decision's copy the gate read, identical to the
    // launch records' own, and the log the supervisor copied from.
    assert_eq!(
        std::fs::read(f.exchange_dir(1).join("admission.json")).unwrap(),
        std::fs::read(f.attempt_dir(1).join("admission.json")).unwrap()
    );
    assert_eq!(
        std::fs::read(f.exchange_dir(1).join("gate.log")).unwrap(),
        std::fs::read(f.attempt_dir(1).join("gate.log")).unwrap()
    );
    assert!(!f.attempt_dir(1).join("admission-gate").exists());

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
    assert_eq!(v["placement"], "home");
    assert_eq!(launch["gateLog"]["attestation"], "child-attested");
    assert_eq!(
        Path::new(launch["gateLog"]["copy"].as_str().unwrap()),
        f.attempt_dir(1).join("gate.log")
    );

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
        "gate      consulted 1 time(s): admitted (child-attested",
        "placement launch records in the product home",
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
    assert!(!f.exchange_dir(1).join("admission-gate").exists());
    assert!(!f.exchange_dir(1).join("gate.log").exists());
    assert!(!f.exchange_dir(1).join("admission.json").exists());
    // The floor alone was still handed over from the exchange directory.
    let given = String::from_utf8(f.received("received-settings-path")).unwrap();
    assert!(Path::new(given.trim()).starts_with(f.exchange_dir(1).canonicalize().unwrap()));
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
    // The repository's launch records directory, in the product home, is
    // not a directory.
    let records = f.places().records;
    std::fs::create_dir_all(records.parent().unwrap()).unwrap();
    std::fs::write(&records, "not a directory").unwrap();

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
    // Something at the path the record belongs at, placed while the session
    // runs. A stand-in for any failure of the record's write; not a claim
    // that a child can reach the launch records.
    std::fs::write(
        f.bin().join("block-record"),
        f.attempt_dir(1).join("record.json").display().to_string(),
    )
    .unwrap();

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

// ------------------------------------------- spec 003 section 3.6.1, through the binary

/// Whatever a test's assertions do, a blocked fake provider is killed and
/// released and a launcher still running is killed when this drops, so a
/// failing assertion cannot leave a provider looping.
struct Unblock {
    bin: PathBuf,
    launched: PathBuf,
    launcher: Option<std::process::Child>,
}

impl Unblock {
    fn new(f: &Fixture, launcher: Option<std::process::Child>) -> Self {
        Self {
            bin: f.bin(),
            launched: f.attempt_dir(1).join("launched.json"),
            launcher,
        }
    }
}

impl Drop for Unblock {
    fn drop(&mut self) {
        if let Some(l) = self.launcher.as_mut() {
            let _ = l.kill();
            let _ = l.wait();
        }
        // A test that already released the fake has killed its group; a
        // second kill could hit a recycled group leader, so skip it.
        if self.bin.join("release").exists() {
            return;
        }
        let pid = std::fs::read(&self.launched)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v["pid"].as_u64());
        if let Some(pid) = pid {
            let _ = Command::new("kill")
                .args(["-s", "KILL", "--", &format!("-{pid}")])
                .output();
        }
        let _ = std::fs::write(self.bin.join("release"), "");
    }
}

/// Start `run` in the fake's `mode` and wait until the fake blocks after the
/// startup decision. The launcher is still running, and holds the repository
/// lock.
fn run_until_blocked(f: &Fixture, mode: &str) -> Unblock {
    f.mode(mode);
    let launcher = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
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
    let guard = Unblock::new(f, Some(launcher));
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
    guard
}

/// Start `run` in the fake's `mode`, wait until the fake blocks after the
/// startup decision, and kill the launcher: an intent with no outcome, the
/// crash boundary reconciliation exists for. Returns the provider's pid, and
/// the guard that releases it however the test ends.
fn crash_mid_session(f: &Fixture, mode: &str) -> (String, Unblock) {
    let mut guard = run_until_blocked(f, mode);
    let mut launcher = guard.launcher.take().expect("a launcher");
    launcher.kill().unwrap();
    launcher.wait().unwrap();
    let launched: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.attempt_dir(1).join("launched.json")).unwrap())
            .unwrap();
    (launched["pid"].as_u64().unwrap().to_string(), guard)
}

/// The repository's run record, as bytes.
fn chain_bytes(f: &Fixture) -> Vec<u8> {
    std::fs::read(statecraft_run::record::chain_path(&f.home(), &f.project())).unwrap_or_default()
}

fn release(f: &Fixture, pid: &str) {
    let _ = Command::new("kill")
        .args(["-s", "KILL", "--", &format!("-{pid}")])
        .output();
    std::fs::write(f.bin().join("release"), "").unwrap();
}

fn reconcile(f: &Fixture, finding: &str, state: &str, extra: &[&str]) -> Output {
    let root = f.root();
    let mut args = vec![
        "run",
        "reconcile",
        &root,
        RUN,
        "1",
        finding,
        state,
        "alice",
        "inspected",
        "the",
        "workspace",
    ];
    args.extend_from_slice(extra);
    f.cli(&args)
}

#[test]
fn an_unknown_attempt_stays_live_until_an_operator_reconciles_it_and_nothing_is_replayed() {
    let f = Fixture::new();
    let (pid, _unblock) = crash_mid_session(&f, "block");
    let released = || release(&f, &pid);

    // Inspection releases nothing.
    assert_eq!(code(&f.show(None)), 1);
    assert_eq!(code(&f.cli(&["run", "show", &f.root(), RUN])), 0);
    let again = f.run();
    assert_eq!(code(&again), 2, "{}", text(&again));
    assert!(text(&again).contains("run reconcile"), "{}", text(&again));

    // Stale: the operator names a state the records do not show.
    let out = reconcile(&f, "absent", "launch-unknown", &[]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("stale"), "{}", text(&out));

    // Usage: a finding or state word the verb does not have.
    assert_eq!(code(&reconcile(&f, "probably", "outcome-unknown", &[])), 3);
    assert_eq!(code(&reconcile(&f, "absent", "finished", &[])), 3);
    // An evidence file that cannot be read refuses and writes nothing.
    let out = reconcile(
        &f,
        "absent",
        "outcome-unknown",
        &["--evidence", "/nonexistent/evidence"],
    );
    assert_eq!(code(&out), 2, "{}", text(&out));

    // `unknown` keeps the attempt live, and `run` refused.
    let out = reconcile(&f, "unknown", "outcome-unknown", &[]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(text(&out).contains("stays live"), "{}", text(&out));
    assert!(text(&out).contains("stops nothing"), "{}", text(&out));
    // `run`'s refusal names the attempt and its `unknown` reconciliation.
    let refused = f.run();
    assert_eq!(code(&refused), 2, "{}", text(&refused));
    let v = json(&refused)["value"].clone();
    assert_eq!(v["attempt"], 1, "{v}");
    assert_eq!(v["reconciliation"]["verdict"], "unknown", "{v}");
    assert_eq!(v["reconciliation"]["operator"], "alice", "{v}");
    let refused = f.cli(&["run", &f.root(), RUN]);
    assert_eq!(code(&refused), 2);
    assert!(
        text(&refused).contains("reconciliation: reconciled unknown by alice"),
        "{}",
        text(&refused)
    );
    // `run list` shows the live attempt with its `unknown` beside it.
    let listed = f.cli(&["run", "list", &f.root(), "--json"]);
    let row = json(&listed)["value"]["runs"][0]["attempts"][0].clone();
    assert_eq!(row["outcome"], serde_json::Value::Null, "{row}");
    assert_eq!(row["reconciliation"]["verdict"], "unknown", "{row}");

    // `absent` with no released tool call is a declaration, not corroborated,
    // and it replaces the unknown.
    let evidence = f.dir.path().join("notes.txt");
    std::fs::write(&evidence, "workspace unchanged\n").unwrap();
    let ev = evidence.display().to_string();
    let out = reconcile(
        &f,
        "absent",
        "outcome-unknown",
        &["--evidence", &ev, "--json"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    let r = json(&out)["value"].clone();
    assert_eq!(r["verdict"], "absent");
    assert_eq!(r["basis"], "operator-declared");
    assert_eq!(r["corroborated"], false);
    assert_eq!(r["operatorProvenance"], "operator-supplied");
    assert_eq!(r["observed"]["launchState"], "outcome-unknown");
    assert_eq!(r["observed"]["confirmedPid"].to_string(), pid);
    assert_eq!(r["evidence"][0]["bytes"], 20);
    assert!(r["replaces"].is_number(), "{r}");
    assert_eq!(r["idempotentByKey"], true);

    // Resolved as interrupted; a second reconciliation is refused.
    let listed = f.cli(&["run", "list", &f.root()]);
    assert!(text(&listed).contains("interrupted"), "{}", text(&listed));
    assert!(
        text(&listed).contains("reconciled absent by alice"),
        "{}",
        text(&listed)
    );
    // `run show` renders each reconciliation: basis, corroboration, and the
    // observed launch state, and the second names the one it replaced.
    let shown = f.cli(&["run", "show", &f.root(), RUN, "--json"]);
    assert_eq!(code(&shown), 0, "{}", text(&shown));
    let recs = json(&shown)["value"]["reconciliations"].clone();
    assert_eq!(recs.as_array().map(Vec::len), Some(2), "{recs}");
    assert_eq!(recs[0]["verdict"], "unknown");
    assert_eq!(recs[1]["verdict"], "absent");
    for r in recs.as_array().unwrap() {
        assert_eq!(r["basis"], "operator-declared", "{r}");
        assert_eq!(r["corroborated"], false, "{r}");
        assert_eq!(r["observedLaunchState"], "outcome-unknown", "{r}");
    }
    assert_eq!(recs[1]["record"]["replaces"], recs[0]["position"], "{recs}");
    let shown = f.cli(&["run", "show", &f.root(), RUN]);
    for needle in [
        "reconciled unknown by alice",
        "reconciled absent by alice",
        "operator-declared",
        "not corroborated",
        "observed outcome-unknown",
    ] {
        assert!(text(&shown).contains(needle), "{needle}: {}", text(&shown));
    }
    let out = reconcile(&f, "confirmed", "outcome-unknown", &[]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("not live"), "{}", text(&out));
    assert_eq!(f.launches(), 1, "reconciliation replayed a launch");

    // The next attempt is the operator's, and it names what it follows.
    released();
    f.mode("faithful");
    let next = f.run();
    assert!(code(&next) <= 1, "{}", text(&next));
    let (chain, _) =
        statecraft_run::record::Chain::open(&f.home(), &f.project()).expect("the record");
    let second = chain
        .entries()
        .into_iter()
        .find(|e| e.kind == statecraft_run::record::Kind::Intent && e.attempt == 2)
        .expect("attempt 2's intent");
    assert_eq!(
        second.detail["follows"][0]["attempt"], 1,
        "{}",
        second.detail
    );
    assert_eq!(second.detail["follows"][0]["finding"], "absent");
}

#[test]
fn absent_is_refused_when_the_gate_released_a_tool_call_and_confirmed_is_recorded() {
    let f = Fixture::new();
    let (pid, _unblock) = crash_mid_session(&f, "tool-then-block");
    // Not concluded, so no copy: the exchange directory's log is what the
    // reconciliation reads (spec 003 section 3.6.1's note under 002 3.37).
    assert!(!f.attempt_dir(1).join("gate.log").exists());
    let gate = std::fs::read_to_string(f.exchange_dir(1).join("gate.log")).unwrap_or_default();
    assert!(gate.lines().any(|l| l == "admitted"), "{gate}");
    let out = reconcile(&f, "absent", "outcome-unknown", &[]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("conflicting"), "{}", text(&out));
    let out = reconcile(&f, "confirmed", "outcome-unknown", &["--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert_eq!(
        json(&out)["value"]["observed"]["gateReleasedToolCall"],
        true
    );
    release(&f, &pid);
}

#[test]
fn a_reconciliation_while_a_run_holds_the_lock_is_refused() {
    let f = Fixture::new();
    let (pid, _unblock) = crash_mid_session(&f, "block");
    let held = statecraft_run::lock::try_acquire(&f.home(), &f.project()).unwrap();
    let out = reconcile(&f, "unknown", "outcome-unknown", &[]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("lock"), "{}", text(&out));
    drop(held);
    release(&f, &pid);
}

#[test]
fn absent_against_an_attempt_that_never_launched_is_corroborated() {
    let f = Fixture::new();
    // An intent in the run record and no launch intent: the crash boundary
    // between the two, which the write order makes a guarantee.
    let (mut chain, _) =
        statecraft_run::record::Chain::open(&f.home(), &f.project()).expect("the record");
    statecraft_run::session::begin(
        &mut chain,
        &f.project(),
        RUN,
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();
    let out = reconcile(&f, "absent", "not-launched", &["--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert_eq!(json(&out)["value"]["corroborated"], true);
    assert_eq!(f.launches(), 0);
}

fn reconcile_as(
    f: &Fixture,
    attempt: &str,
    finding: &str,
    state: &str,
    operator: &str,
    reason: &str,
) -> Output {
    let root = f.root();
    f.cli(&[
        "run",
        "reconcile",
        &root,
        RUN,
        attempt,
        finding,
        state,
        operator,
        reason,
        "--json",
    ])
}

/// An intent in the run record and nothing else: `begin` called directly, the
/// crash boundary before any launch intent.
fn intent_only(f: &Fixture) {
    let (mut chain, _) =
        statecraft_run::record::Chain::open(&f.home(), &f.project()).expect("the record");
    statecraft_run::session::begin(
        &mut chain,
        &f.project(),
        RUN,
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();
}

#[test]
fn a_reconciliation_while_a_real_run_is_blocked_is_refused_and_writes_nothing() {
    let f = Fixture::new();
    let mut guard = run_until_blocked(&f, "block");
    let before = chain_bytes(&f);
    let entries = |f: &Fixture| {
        statecraft_run::record::Chain::open(&f.home(), &f.project())
            .expect("the record")
            .0
            .records()
            .len()
    };
    let count = entries(&f);
    for finding in ["unknown", "confirmed", "absent"] {
        let out = reconcile(&f, finding, "outcome-unknown", &[]);
        assert_eq!(code(&out), 2, "{finding}: {}", text(&out));
        assert!(text(&out).contains("lock"), "{}", text(&out));
    }
    assert_eq!(entries(&f), count, "a refused reconciliation appended");
    assert_eq!(chain_bytes(&f), before, "a refused reconciliation wrote");

    // The supervising run was never disturbed: released, it concludes.
    std::fs::write(f.bin().join("release"), "").unwrap();
    let mut launcher = guard.launcher.take().expect("the launcher");
    let status = launcher.wait().unwrap();
    assert!(status.code().is_some_and(|c| c <= 1), "{status:?}");
    let listed = f.cli(&["run", "list", &f.root(), "--json"]);
    let row = json(&listed)["value"]["runs"][0]["attempts"][0].clone();
    assert_eq!(row["outcome"], "completed", "{row}");
    assert_eq!(row["reconciliation"], serde_json::Value::Null, "{row}");
}

#[test]
fn absent_against_launch_unknown_is_declared_only_and_releases_the_attempt() {
    let f = Fixture::new();
    let (pid, _unblock) = crash_mid_session(&f, "block");
    release(&f, &pid);
    // The crash boundary between the launch intent and the spawn
    // confirmation: `intent.json` and nothing after it.
    for file in ["launched.json", "admission.json", "record.json", "gate.log"] {
        let _ = std::fs::remove_file(f.attempt_dir(1).join(file));
        let _ = std::fs::remove_file(f.exchange_dir(1).join(file));
    }
    assert_eq!(value(&f.show(Some(1)))["verdict"], "launch-unknown");

    let out = reconcile(&f, "absent", "launch-unknown", &["--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let r = json(&out)["value"].clone();
    assert_eq!(r["corroborated"], false, "{r}");
    assert_eq!(r["observed"]["launchState"], "launch-unknown");
    assert_eq!(r["observed"]["confirmedPid"], serde_json::Value::Null);
    assert!(
        r["observed"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str().is_some_and(|p| p.ends_with("gate.log"))),
        "{r}"
    );
    let listed = f.cli(&["run", "list", &f.root(), "--json"]);
    let row = json(&listed)["value"]["runs"][0]["attempts"][0].clone();
    assert_eq!(row["outcome"], "interrupted", "{row}");
    assert_eq!(row["reconciliation"]["verdict"], "absent", "{row}");
    assert_eq!(
        row["reconciliation"]["observed"]["launchState"],
        "launch-unknown"
    );
    assert_eq!(f.launches(), 1, "reconciliation replayed a launch");
}

#[test]
fn confirmed_releases_the_attempt_and_the_next_intent_follows_it() {
    let f = Fixture::new();
    let (pid, _unblock) = crash_mid_session(&f, "block");
    release(&f, &pid);
    let out = reconcile(&f, "confirmed", "outcome-unknown", &["--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert_eq!(json(&out)["value"]["retryAllowed"], true);
    assert!(text(&out).contains("confirmed"), "{}", text(&out));

    let shown = f.cli(&["run", "show", &f.root(), RUN, "--json"]);
    let recs = json(&shown)["value"]["reconciliations"].clone();
    assert_eq!(recs[0]["verdict"], "confirmed", "{recs}");
    assert_eq!(recs[0]["corroborated"], false);

    f.mode("faithful");
    let next = f.run();
    assert!(code(&next) <= 1, "{}", text(&next));
    let (chain, _) =
        statecraft_run::record::Chain::open(&f.home(), &f.project()).expect("the record");
    let second = chain
        .entries()
        .into_iter()
        .find(|e| e.kind == statecraft_run::record::Kind::Intent && e.attempt == 2)
        .expect("attempt 2's intent");
    let follows = &second.detail["follows"];
    assert_eq!(follows.as_array().map(Vec::len), Some(1), "{follows}");
    assert_eq!(follows[0]["runId"], RUN);
    assert_eq!(follows[0]["attempt"], 1);
    assert_eq!(follows[0]["finding"], "confirmed");
    assert_eq!(f.launches(), 2, "one launch per operator `run`");
}

#[test]
fn a_concluded_attempt_an_unknown_attempt_number_and_an_empty_reason_are_refused() {
    let f = Fixture::new();
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let before = chain_bytes(&f);

    // A concluded attempt.
    let out = reconcile_as(&f, "1", "absent", "unverified", "alice", "done");
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("not live"), "{}", text(&out));
    // Another spelling of the registered root reads the same chain: the
    // concluded attempt is found and refused as concluded, not missing.
    let spelled = format!("{}/", f.root());
    let out = f.cli(&[
        "run",
        "reconcile",
        &spelled,
        RUN,
        "1",
        "absent",
        "not-launched",
        "alice",
        "none",
    ]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(!text(&out).contains("no attempt 1"), "{}", text(&out));
    assert_eq!(chain_bytes(&f), before);

    // An attempt the record does not carry.
    let out = reconcile_as(&f, "7", "absent", "not-launched", "alice", "none");
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("no attempt 7"), "{}", text(&out));
    assert_eq!(chain_bytes(&f), before);
    // Every refusal of this verb answers in one JSON shape.
    let out = f.cli(&[
        "run",
        "reconcile",
        &f.root(),
        RUN,
        "7",
        "absent",
        "not-launched",
        "alice",
        "none",
        "--json",
    ]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    let answer: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(answer["value"]["refused"].is_string(), "{answer}");

    // An empty reason or operator, against a live attempt.
    let g = Fixture::new();
    intent_only(&g);
    let before = chain_bytes(&g);
    for (operator, reason) in [("alice", ""), ("alice", "   "), ("", "looked")] {
        let out = reconcile_as(&g, "1", "absent", "not-launched", operator, reason);
        assert_eq!(code(&out), 2, "{operator:?} {reason:?}: {}", text(&out));
        assert!(text(&out).contains("non-empty"), "{}", text(&out));
    }
    assert_eq!(chain_bytes(&g), before, "a refused reconciliation wrote");
}

#[test]
fn a_released_tool_call_refuses_absent_even_with_no_manifest() {
    let f = Fixture::new();
    intent_only(&f);
    std::fs::remove_file(f.project().join(".statecraft/environment.json")).unwrap();
    std::fs::create_dir_all(f.exchange_dir(1)).unwrap();
    std::fs::write(f.exchange_dir(1).join("gate.log"), "admitted\n").unwrap();
    let before = chain_bytes(&f);
    let out = reconcile(&f, "absent", "unrecorded", &[]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("conflicting"), "{}", text(&out));
    assert_eq!(chain_bytes(&f), before);

    let out = reconcile(&f, "unknown", "unrecorded", &["--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let observed = json(&out)["value"]["observed"].clone();
    assert_eq!(observed["launchState"], "unrecorded");
    assert_eq!(observed["gateReleasedToolCall"], true, "{observed}");
    assert!(
        observed["files"][0]
            .as_str()
            .is_some_and(|p| p.ends_with("gate.log")),
        "{observed}"
    );
}

#[test]
fn a_gate_log_that_exists_and_cannot_be_read_fails_and_writes_nothing() {
    let f = Fixture::new();
    intent_only(&f);
    // A gate log that is there and cannot be read as a file.
    std::fs::create_dir_all(f.exchange_dir(1).join("gate.log")).unwrap();
    let before = chain_bytes(&f);
    let out = reconcile(&f, "absent", "not-launched", &[]);
    assert_eq!(code(&out), 4, "{}", text(&out));
    assert!(text(&out).contains("gate.log"), "{}", text(&out));
    assert_eq!(
        chain_bytes(&f),
        before,
        "an unreadable gate log was read as nothing"
    );
}

// ------------------------------------------- spec 002 section 3.37, through the binary

/// A gate log that is a link is not followed: reconciliation fails rather
/// than reading what the link names, and writes nothing.
#[test]
fn a_gate_log_that_is_a_link_is_not_followed_and_writes_nothing() {
    let f = Fixture::new();
    intent_only(&f);
    let target = f.dir.path().join("elsewhere.log");
    std::fs::write(&target, "admitted\n").unwrap();
    std::fs::create_dir_all(f.exchange_dir(1)).unwrap();
    std::os::unix::fs::symlink(&target, f.exchange_dir(1).join("gate.log")).unwrap();
    let before = chain_bytes(&f);
    let out = reconcile(&f, "absent", "not-launched", &[]);
    assert_eq!(code(&out), 4, "{}", text(&out));
    assert!(text(&out).contains("gate.log"), "{}", text(&out));
    assert_eq!(chain_bytes(&f), before);
}

/// Rule 2: a child that writes the decision into its exchange directory
/// before the supervisor does has not made the decision. The launch records
/// hold the supervisor's; its copy is refused rather than adopted or silently
/// put over the plant; and the attempt is refused with the failure named.
#[test]
fn a_decision_the_child_planted_before_the_supervisors_is_not_the_decision() {
    let f = Fixture::new();
    f.mode("plant-decision");
    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let answer = json(&out);
    assert_eq!(answer["value"]["outcome"], "refused", "{answer}");
    assert!(f.order().iter().any(|l| l == "planted"), "{:?}", f.order());
    // No tool call after the decision ran: the process was stopped there.
    assert!(!f.workspace().join("sentinel-after").exists());
    // The plant is gone, and the records say what the supervisor decided.
    assert!(!f.exchange_dir(1).join("admission.json").exists());
    let v = value(&f.show(None));
    assert_eq!(v["verdict"], "not-admitted", "{v}");
    assert_eq!(
        v["admission"]["intentDigest"].as_str().map(str::len),
        Some(64)
    );
    let error = v["record"]["launch"]["admissionError"].as_str().unwrap();
    assert!(error.contains("did not write"), "{error}");
    assert!(error.contains("gate withholds"), "{error}");
    assert!(v["reasons"].to_string().contains("did not write"), "{v}");
    let (chain, _) =
        statecraft_run::record::Chain::open(&f.home(), &f.project()).expect("the record");
    let outcome = chain
        .entries()
        .into_iter()
        .find(|e| e.kind == statecraft_run::record::Kind::Outcome && e.attempt == 1)
        .expect("attempt 1's outcome");
    assert_eq!(outcome.detail["refusals"], 1, "{}", outcome.detail);
}

/// Move an attempt's records, byte for byte, from the product home to where
/// section 3.32 wrote them inside the target, and remove the home's: an
/// attempt recorded before section 3.37.
fn move_to_legacy(f: &Fixture, n: u32) {
    let legacy = f.legacy_dir(n);
    std::fs::create_dir_all(&legacy).unwrap();
    for dir in [f.exchange_dir(n), f.attempt_dir(n)] {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with("statecraft-settings-")
            {
                continue;
            }
            let to = legacy.join(entry.file_name());
            if !to.exists() {
                std::fs::copy(entry.path(), to).unwrap();
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// Rule 5: records written inside the target before section 3.37 are read
/// where they are, judged as before, and labelled as written where the child
/// could reach them; nothing moves them back or says they were confined.
#[test]
fn an_attempt_recorded_inside_the_target_is_read_where_it_is_and_labelled() {
    let f = Fixture::new();
    assert_eq!(code(&f.run()), 0);
    let home = value(&f.show(None));
    move_to_legacy(&f, 1);
    let before: Vec<Vec<u8>> = ["intent.json", "record.json", "admission.json"]
        .iter()
        .map(|n| std::fs::read(f.legacy_dir(1).join(n)).unwrap())
        .collect();

    let shown = f.show(None);
    assert_eq!(code(&shown), 1, "{}", text(&shown));
    let v = value(&shown);
    assert_eq!(v["placement"], "target");
    assert_eq!(v["verdict"], home["verdict"]);
    assert_eq!(v["reasons"], home["reasons"]);
    assert_eq!(v["gate"], serde_json::json!(["admitted"]));
    assert!(
        Path::new(v["recordPath"].as_str().unwrap()).starts_with(f.project()),
        "{v}"
    );
    let human = text(&f.cli(&["startup", "show", &f.root(), RUN]));
    assert!(
        human.contains("where the child could reach them"),
        "{human}"
    );
    assert!(
        human.contains("nothing here says the attempt was confined"),
        "{human}"
    );
    // Read, never moved or rewritten.
    assert!(!f.attempt_dir(1).exists());
    let after: Vec<Vec<u8>> = ["intent.json", "record.json", "admission.json"]
        .iter()
        .map(|n| std::fs::read(f.legacy_dir(1).join(n)).unwrap())
        .collect();
    assert_eq!(before, after);

    // A later attempt is recorded in the home, and each is read from its own
    // layout.
    assert!(code(&f.run()) <= 1);
    assert!(f.attempt_dir(2).join("record.json").is_file());
    assert!(!f.legacy_dir(2).exists());
    assert_eq!(value(&f.show(Some(2)))["placement"], "home");
    assert_eq!(value(&f.show(Some(1)))["placement"], "target");
}

/// Reconciliation reads a live attempt in either layout: the home's exchange
/// gate log for one recorded after section 3.37, the target's for one
/// recorded before it.
#[test]
fn reconciliation_reads_an_attempt_recorded_inside_the_target() {
    let f = Fixture::new();
    let (pid, _unblock) = crash_mid_session(&f, "tool-then-block");
    release(&f, &pid);
    move_to_legacy(&f, 1);
    assert_eq!(value(&f.show(Some(1)))["placement"], "target");
    // The gate released a tool call, as the target's log says.
    let out = reconcile(&f, "absent", "outcome-unknown", &[]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("conflicting"), "{}", text(&out));
    let out = reconcile(&f, "confirmed", "outcome-unknown", &["--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let observed = json(&out)["value"]["observed"].clone();
    assert_eq!(observed["gateReleasedToolCall"], true, "{observed}");
    assert_eq!(observed["confirmedPid"].to_string(), pid);
    for file in observed["files"].as_array().unwrap() {
        assert!(
            Path::new(file.as_str().unwrap()).starts_with(f.project()),
            "{observed}"
        );
    }
}
