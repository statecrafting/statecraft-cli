//! Spec 004 section 3.17: the command allowance, the suite's programs, and the
//! comparison between them, through the built binary.
//!
//! Every run is given a temporary `STATECRAFT_HOME`, `STATECRAFT_NATIVE_ROOT`
//! and `HOME`, and a `PATH` holding only a fake `spec-spine`, a fake `claude`
//! and the system directories. **No provider is spawned and nothing here is
//! live evidence.**
//!
//! The fake `spec-spine` answers `verify <spec> --plan --json` from a file,
//! `suite-plan.json`, in **the directory it is run in**, and logs that
//! directory. So the suite's programs come from the tree being read and never
//! from the allowance: the planning reading sees the working tree, the launch
//! reading sees an export of the base commit, and a test can tell the two
//! apart. The allowance is `project.commands` in `.statecraft/environment.json`,
//! written by the product's own `init apply` and then edited by the test, the
//! way an operator declares one.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RUN: &str = "fixture";

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

fn json_of(out: &Output) -> Value {
    json_naming::from_output(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(out)))
}

/// The fake spec-spine. `verify fixture --plan --json` reads the suite from
/// the directory it runs in; `plan-exit` in its own directory makes it fail.
const FAKE_SPEC_SPINE: &str = r#"#!/bin/sh
here="$(dirname "$0")"
case "$*" in
  --version) echo 'spec-spine 0.23.0' ;;
  check|'check --help') exit 0 ;;
  compile|index) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"fixture","title":"coverage fixture"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"fixture","status":"approved","implementation":"pending"}]}' ;;
  'verify fixture --plan --json')
    /bin/pwd -P >> "$here/verify-dirs"
    if [ -f "$here/plan-exit" ]; then echo 'error: the plan failed' >&2; exit "$(/bin/cat "$here/plan-exit")"; fi
    if [ -f "$here/fail-outside-a-checkout" ] && [ ! -e .git ]; then echo 'error: no plan in the export' >&2; exit 4; fi
    [ -f suite-plan.json ] || exit 3
    /bin/cat suite-plan.json ;;
  *) exit 3 ;;
esac
"#;

/// The fake provider: runs the `SessionStart` command the settings document
/// registers, reports it, then replays the recorded 2.1.267 success stream.
const FAKE_PROVIDER: &str = r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267 (Claude Code)'; exit 0; fi
here="$(dirname "$0")"
settings=""
while [ "$#" -gt 0 ]; do
  case "$1" in --settings) settings="$2"; shift ;; esac
  shift
done
/bin/cat > /dev/null
echo launched >> "$here/launches"
session=11111111-1111-1111-1111-111111111111
escape() { /usr/bin/awk 'BEGIN { ORS = "" } { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); if (NR > 1) printf "\\n"; print }'; }
command_for() {
  /usr/bin/awk -v ev="\"$1\": [" 'index($0, ev) { inside = 1 } inside && /"command": / { sub(/^[^"]*"command": "/, ""); sub(/",?[ \t]*$/, ""); gsub(/\\\\/, "\001"); gsub(/\\"/, "\""); gsub(/\001/, "\\"); print; exit }' "$settings"
}
start_cmd="$(command_for SessionStart)"
if [ -n "$start_cmd" ]; then
  out="$(CLAUDE_PROJECT_DIR="$PWD" /bin/sh -c "$start_cmd" 2>/dev/null)"
  rc=$?
  printf '{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup","hook_event":"SessionStart","session_id":"%s"}\n' "$session"
  printf '{"type":"system","subtype":"hook_response","hook_name":"SessionStart:startup","hook_event":"SessionStart","stdout":"%s","stderr":"","exit_code":%s,"outcome":"success","session_id":"%s"}\n' "$(printf '%s' "$out" | escape)" "$rc" "$session"
fi
/bin/cat "$here/native.jsonl"
"#;

/// A `verify --plan --json` envelope holding these commands.
fn plan(commands: &[&str]) -> String {
    json!({
        "exitCode": 0,
        "ok": true,
        "report": { "commands": commands, "skipped": [], "specId": RUN },
        "schemaVersion": "0.6.0",
        "verb": "verify",
    })
    .to_string()
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

    /// A managed target whose committed suite is `suite`, with `declared` as
    /// its committed allowance (`None`: the member is absent).
    fn new(suite: &[&str], declared: Option<Value>) -> Self {
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
        executable(&f.bin().join("spec-spine"), FAKE_SPEC_SPINE);
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl"),
            f.bin().join("native.jsonl"),
        )
        .unwrap();
        executable(&f.bin().join("claude"), FAKE_PROVIDER);

        let root = f.root();
        for (args, ok) in [
            (vec!["home", "apply"], &[0, 1][..]),
            (vec!["init", "apply", &root], &[0, 1]),
            (vec!["harness", "upgrade", &root], &[0]),
        ] {
            let out = f.cli(&args);
            assert!(ok.contains(&code(&out)), "{args:?}: {}", text(&out));
        }
        std::fs::write(
            f.project().join("AGENTS.md"),
            "@.statecraft/AGENTS.md\n\n# the fixture project\n",
        )
        .unwrap();
        f.write_suite(suite);
        if let Some(commands) = declared {
            f.declare(commands);
        }
        f.git(&["add", "-A"]);
        f.git(&["commit", "--quiet", "-m", "managed"]);
        for args in [["project", "register"], ["project", "arm"]] {
            let out = f.cli(&[args[0], args[1], &f.root()]);
            assert!(code(&out) <= 1, "{args:?}: {}", text(&out));
        }
        f
    }

    /// Write the suite the fake spec-spine reports, in the working tree.
    fn write_suite(&self, suite: &[&str]) {
        std::fs::write(self.project().join("suite-plan.json"), plan(suite)).unwrap();
    }

    fn manifest_path(&self) -> PathBuf {
        self.project().join(".statecraft/environment.json")
    }

    /// Set `project.commands` in the working tree's declaration.
    fn declare(&self, commands: Value) {
        let mut manifest: Value =
            serde_json::from_slice(&std::fs::read(self.manifest_path()).unwrap()).unwrap();
        manifest["project"]["commands"] = commands;
        std::fs::write(
            self.manifest_path(),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn run(&self) -> Output {
        self.cli(&["run", &self.root(), RUN, "--json"])
    }

    fn launches(&self) -> usize {
        std::fs::read_to_string(self.bin().join("launches"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    fn verify_dirs(&self) -> Vec<PathBuf> {
        std::fs::read_to_string(self.bin().join("verify-dirs"))
            .unwrap_or_default()
            .lines()
            .map(PathBuf::from)
            .collect()
    }

    fn entries(&self) -> Vec<statecraft_run::record::Entry> {
        match statecraft_run::record::Chain::open(&self.home(), &self.project()) {
            Ok((chain, _)) => chain.entries(),
            Err(_) => Vec::new(),
        }
    }

    fn head(&self) -> String {
        head_of(&self.project())
    }

    /// The run's retained workspace, reused by every attempt of the run.
    fn workspace(&self) -> PathBuf {
        self.project()
            .join(".statecraft/state/workspaces")
            .join(RUN)
    }
}

fn head_of(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

impl Fixture {}

/// A planning refusal: exit 2, the guard named, nothing appended, nothing
/// launched.
fn assert_refused_at_planning(f: &Fixture, out: &Output) -> Value {
    assert_eq!(code(out), 2, "{}", text(out));
    let v = json_of(out);
    assert_eq!(v["value"]["guard"], "posture-coverage", "{v}");
    assert_eq!(v["value"]["phase"], "planning", "{v}");
    assert!(f.entries().is_empty(), "an attempt was appended");
    assert_eq!(f.launches(), 0, "a provider was launched");
    v
}

/// The attempt's recorded outcome detail and its intent detail.
fn attempt_details(f: &Fixture) -> (Value, Value) {
    let entries = f.entries();
    let outcome = entries
        .iter()
        .rfind(|e| e.subject == "attempt")
        .expect("an outcome")
        .detail
        .clone();
    let intent = entries
        .iter()
        .rfind(|e| e.subject != "attempt" && e.detail.get("baseCommit").is_some())
        .expect("an intent")
        .detail
        .clone();
    (outcome, intent)
}

// The acceptance of section 3.17, row by row.

/// A spec whose verification names `cargo`, with no declared allowance, is
/// refused before any process is created, naming `cargo`.
#[test]
fn an_undeclared_program_the_suite_names_is_refused_at_planning_naming_it() {
    let f = Fixture::new(&["cargo test --workspace --locked", "git status"], None);
    let out = f.run();
    let v = assert_refused_at_planning(&f, &out);
    let coverage = &v["value"]["coverage"];
    assert_eq!(coverage["verdict"], "refused", "{coverage}");
    assert_eq!(coverage["missing"][0]["program"], "cargo");
    assert_eq!(
        coverage["missing"][0]["commands"],
        json!(["cargo test --workspace --locked"])
    );
    assert_eq!(coverage["declaration"]["state"], "absent");
    let reason = v["value"]["reason"].as_str().unwrap();
    assert!(reason.contains("`cargo`"), "{reason}");
    assert!(reason.contains("project.commands"), "{reason}");
    assert!(!reason.contains("`git`"), "{reason}");
    // The requirement came from the suite reader, run once, in the working
    // tree, and never from the allowance.
    assert_eq!(f.verify_dirs().len(), 1);
    assert_eq!(
        f.verify_dirs()[0],
        f.project().canonicalize().unwrap(),
        "planning reads the working tree"
    );
    let human = f.cli(&["run", &f.root(), RUN]);
    assert_eq!(code(&human), 2);
    assert!(text(&human).contains("missing: cargo"), "{}", text(&human));
}

/// The same suite with `cargo` declared runs, verdict `direct`, and the
/// recorded coverage is what `run show` renders.
#[test]
fn a_declared_program_covers_the_suite_and_run_show_renders_the_record() {
    let f = Fixture::new(
        &["cargo test --workspace --locked", "git status"],
        Some(json!(["cargo"])),
    );
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let v = json_of(&out);
    let coverage = &v["value"]["posture"]["value"]["coverage"];
    assert_eq!(coverage["verdict"], "direct", "{coverage}");
    assert_eq!(coverage["spec"], RUN);
    assert_eq!(coverage["baseCommit"], f.head());
    assert_eq!(coverage["missing"], json!([]));
    assert_eq!(coverage["enforcement"], "checked, not enforced");
    assert_eq!(coverage["limits"].as_array().unwrap().len(), 2);
    assert_eq!(
        coverage["commands"],
        json!([
            {"command": "cargo test --workspace --locked", "reading": {"kind": "program", "program": "cargo"}},
            {"command": "git status", "reading": {"kind": "program", "program": "git"}},
        ])
    );
    let allowance = coverage["allowance"].as_array().unwrap();
    assert!(allowance.contains(&json!({"program": "cargo", "source": "declared"})));
    assert!(allowance.contains(&json!({"program": "git", "source": "adapter"})));
    assert_eq!(coverage["declaration"]["state"], "declared");
    assert_eq!(
        coverage["declaration"]["digest"].as_str().unwrap().len(),
        64
    );
    assert_eq!(f.launches(), 1);

    // Launch read an export of the base commit: neither the working tree nor
    // the workspace the session runs in.
    let dirs = f.verify_dirs();
    assert_eq!(dirs.len(), 2, "{dirs:?}");
    let root = f.project().canonicalize().unwrap();
    assert_eq!(dirs[0], root);
    assert!(!dirs[1].starts_with(&root), "{dirs:?}");
    assert!(!dirs[1].exists(), "the export is removed after it is read");

    // The outcome carries it under posture.coverage, and the intent records
    // the planning verdict with the digests that launch was compared against.
    let (outcome, intent) = attempt_details(&f);
    assert_eq!(outcome["posture"]["coverage"], *coverage);
    let planning = &intent["postureCoverage"];
    assert_eq!(planning["phase"], "planning", "{intent}");
    assert_eq!(planning["verdict"], "direct");
    assert_eq!(planning["planDigest"], coverage["planDigest"]);
    assert_eq!(planning["allowanceDigest"], coverage["allowanceDigest"]);

    let shown = f.cli(&["run", "show", &f.root(), RUN, "--json"]);
    assert_eq!(code(&shown), 0, "{}", text(&shown));
    assert_eq!(
        json_of(&shown)["value"]["posture"]["value"]["coverage"],
        *coverage
    );
    let human = f.cli(&["run", "show", &f.root(), RUN]);
    let h = text(&human);
    for line in [
        "command coverage: direct (checked, not enforced)",
        "cargo test --workspace --locked -> cargo",
        "allowance: claude (adapter), git (adapter), cargo (declared)",
        "limit: transitive programs are not seen",
        "limit: the allowance is checked, not enforced",
    ] {
        assert!(h.contains(line), "missing {line:?} in:\n{h}");
    }
}

/// A command with a pipe is named `unparsed` and the verdict is `partial`.
#[test]
fn a_piped_command_is_unparsed_and_the_verdict_is_partial() {
    let f = Fixture::new(&["git log --oneline | head -1", "git status"], None);
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let coverage = json_of(&out)["value"]["posture"]["value"]["coverage"].clone();
    assert_eq!(coverage["verdict"], "partial", "{coverage}");
    assert_eq!(
        coverage["commands"][0]["reading"],
        json!({"kind": "unparsed"})
    );
    let human = f.cli(&["run", "show", &f.root(), RUN]);
    assert!(
        text(&human).contains("git log --oneline | head -1 -> unparsed, not checked"),
        "{}",
        text(&human)
    );
}

/// A command led by a wrapper is `unparsed`; a path-qualified one is `path`
/// and requires nothing; an assignment prefix and a quoted first word are
/// `unparsed`. None of them makes `cargo` required, so an undeclared `cargo`
/// behind them is not seen, which is exactly what `partial` says.
#[test]
fn wrapped_path_qualified_and_unparsable_commands_are_read_as_the_rules_say() {
    let f = Fixture::new(
        &[
            "env cargo test",
            "sh -c 'cargo test'",
            "./scripts/check.sh --all",
            "FOO=1 cargo test",
            "'cargo' test",
        ],
        None,
    );
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let coverage = json_of(&out)["value"]["posture"]["value"]["coverage"].clone();
    let readings: Vec<Value> = coverage["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["reading"]["kind"].clone())
        .collect();
    assert_eq!(
        readings,
        [
            json!("unparsed"),
            json!("unparsed"),
            json!("path"),
            json!("unparsed"),
            json!("unparsed")
        ]
    );
    assert_eq!(coverage["verdict"], "partial");
    assert_eq!(coverage["missing"], json!([]));

    // A path alone requires nothing and is fully parsed: `direct`.
    let g = Fixture::new(&["./scripts/check.sh"], None);
    let out = g.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert_eq!(
        json_of(&out)["value"]["posture"]["value"]["coverage"]["verdict"],
        "direct"
    );
}

/// A declared entry holding a `/`, whitespace, or repeating another, refuses
/// the run and names it.
#[test]
fn a_malformed_declared_entry_refuses_and_is_named() {
    for (declared, named) in [
        (json!(["/usr/bin/cargo"]), "\"/usr/bin/cargo\""),
        (json!(["cargo make"]), "\"cargo make\""),
        (json!(["make", "make"]), "declared twice"),
        (json!("cargo"), "not a list"),
    ] {
        let f = Fixture::new(&["git status"], Some(declared.clone()));
        let out = f.run();
        let v = assert_refused_at_planning(&f, &out);
        let reason = v["value"]["reason"].as_str().unwrap();
        assert!(reason.contains(named), "{declared}: {reason}");
        assert!(reason.contains("project.commands"), "{reason}");
    }
}

/// An allowance changed between planning and launch (declared in the working
/// tree, absent at the base) is refused at launch naming the allowance. The
/// base is authoritative: the uncommitted declaration covered the suite at
/// planning and the base's does not.
#[test]
fn an_allowance_declared_only_in_the_working_tree_is_refused_at_launch() {
    let f = Fixture::new(&["cargo test"], None);
    f.declare(json!(["cargo"]));
    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let v = json_of(&out);
    assert_eq!(v["value"]["outcome"], "refused", "{v}");
    assert_eq!(f.launches(), 0, "nothing is spawned after a launch refusal");
    let coverage = &v["value"]["posture"]["value"]["coverage"];
    assert_eq!(coverage["drift"], json!(["allowance"]), "{coverage}");
    assert_eq!(coverage["declaration"]["state"], "absent");
    assert_eq!(coverage["verdict"], "refused");
    let (outcome, intent) = attempt_details(&f);
    assert_eq!(intent["postureCoverage"]["verdict"], "direct");
    let why = outcome["postureCoverageRefusal"].as_str().unwrap();
    assert!(why.contains("allowance"), "{why}");
    assert!(why.contains("`cargo`"), "{why}");
    // The refusal is counted under its guard, in the durable record.
    let counted: Vec<String> = f
        .entries()
        .iter()
        .filter(|e| e.subject != "attempt")
        .map(|e| e.detail.to_string())
        .filter(|d| d.contains("posture-coverage"))
        .collect();
    assert!(!counted.is_empty(), "no record names the guard");
    // `run show` renders the refused coverage from the record.
    let human = text(&f.cli(&["run", "show", &f.root(), RUN]));
    assert!(human.contains("command coverage: refused"), "{human}");
    assert!(
        human.contains("changed since planning: allowance"),
        "{human}"
    );
}

/// A suite plan changed in the working tree differs from the base's by digest
/// and is refused at launch, naming the suite plan; the base's plan is the one
/// recorded, and a program only the base's suite names is missing.
#[test]
fn a_suite_plan_that_differs_at_the_base_is_refused_and_the_base_is_recorded() {
    // Committed suite needs `cargo`; the working tree's does not.
    let f = Fixture::new(&["cargo test", "git status"], None);
    f.write_suite(&["git status"]);
    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let v = json_of(&out);
    assert_eq!(v["value"]["outcome"], "refused");
    let coverage = &v["value"]["posture"]["value"]["coverage"];
    assert_eq!(coverage["drift"], json!(["suite plan"]), "{coverage}");
    assert_eq!(coverage["verdict"], "refused");
    assert_eq!(coverage["missing"][0]["program"], "cargo");
    assert_eq!(
        coverage["commands"][0]["command"], "cargo test",
        "the recorded suite is the base's"
    );
    let (outcome, intent) = attempt_details(&f);
    assert_ne!(
        intent["postureCoverage"]["planDigest"],
        coverage["planDigest"]
    );
    assert!(
        outcome["postureCoverageRefusal"]
            .as_str()
            .unwrap()
            .contains("suite plan")
    );
    assert_eq!(f.launches(), 0);

    // An uncommitted change to anything else does not refuse.
    let g = Fixture::new(&["git status"], None);
    std::fs::write(g.project().join("README.md"), "edited\n").unwrap();
    let out = g.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
}

/// A plan that cannot be read refuses the run, naming why: the reader exits
/// non-zero, answers something that is not a plan, answers a plan with no
/// `commands`, or does not answer `verify --plan` at all.
#[test]
fn an_unreadable_malformed_or_absent_plan_refuses() {
    let f = Fixture::new(&["git status"], None);
    std::fs::write(f.bin().join("plan-exit"), "1").unwrap();
    let v = assert_refused_at_planning(&f, &f.run());
    let reason = v["value"]["reason"].as_str().unwrap();
    assert!(reason.contains("could not be read"), "{reason}");
    assert!(reason.contains("exit 1"), "{reason}");
    assert!(v["value"]["coverage"].is_null());
    std::fs::remove_file(f.bin().join("plan-exit")).unwrap();

    for (body, named) in [
        ("this is not json", "does not parse"),
        (
            r#"{"exitCode":0,"ok":true,"report":{"skipped":[],"specId":"fixture"},"verb":"verify"}"#,
            "commands",
        ),
        (
            r#"{"exitCode":1,"ok":false,"error":{"kind":"not-found"},"verb":"verify"}"#,
            "does not parse",
        ),
    ] {
        std::fs::write(f.project().join("suite-plan.json"), body).unwrap();
        let v = assert_refused_at_planning(&f, &f.run());
        let reason = v["value"]["reason"].as_str().unwrap();
        assert!(reason.contains(named), "{body}: {reason}");
    }

    std::fs::remove_file(f.project().join("suite-plan.json")).unwrap();
    let v = assert_refused_at_planning(&f, &f.run());
    assert!(
        v["value"]["reason"].as_str().unwrap().contains("exit 3"),
        "{v}"
    );
}

/// An empty plan is one that says it is empty: `direct`, nothing required.
#[test]
fn a_plan_that_says_it_is_empty_is_direct() {
    let f = Fixture::new(&[], None);
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let coverage = json_of(&out)["value"]["posture"]["value"]["coverage"].clone();
    assert_eq!(coverage["verdict"], "direct", "{coverage}");
    assert_eq!(coverage["commands"], json!([]));
}

/// A second attempt passed the coverage check and was launched. Its outcome
/// is not asserted `completed`: spec 003's base-movement observation compares
/// the target's base with the reused workspace's `HEAD`, which a second
/// attempt after any new commit reports as moved (`interrupted`). That is the
/// existing reading of spec 003 section 3.8 and is not this section's.
fn assert_launched_past_coverage(f: &Fixture, out: &Output, launches: usize) -> Value {
    let v = json_of(out);
    assert_ne!(v["value"]["outcome"], "refused", "{v}");
    assert_eq!(f.launches(), launches, "{v}");
    let (outcome, _) = attempt_details(f);
    assert!(outcome.get("postureCoverageRefusal").is_none(), "{outcome}");
    v["value"]["posture"]["value"]["coverage"].clone()
}

/// A second attempt after a committed change to the suite and to the
/// declaration reads the new base, not the first attempt's workspace, and
/// is launched: the base is the one the intent records.
#[test]
fn a_second_attempt_after_a_committed_change_reads_the_new_base_and_runs() {
    let f = Fixture::new(&["git status"], None);
    let first = f.run();
    assert_eq!(code(&first), 0, "{}", text(&first));
    let first_base = f.head();

    f.write_suite(&["git status", "cargo test"]);
    f.declare(json!(["cargo"]));
    f.git(&["add", "-A"]);
    f.git(&["commit", "--quiet", "-m", "the suite needs cargo, declared"]);
    let second = f.run();
    let coverage = assert_launched_past_coverage(&f, &second, 2);
    assert_eq!(coverage["verdict"], "direct", "{coverage}");
    assert_eq!(coverage["baseCommit"], f.head());
    assert_ne!(coverage["baseCommit"], first_base.as_str());
    assert!(coverage.get("drift").is_none(), "{coverage}");
    let (_, intent) = attempt_details(&f);
    assert_eq!(intent["baseCommit"], coverage["baseCommit"]);
}

/// A commit a session made inside the reused workspace is not the base: the
/// launch reading takes the commit the intent records, so a suite the session
/// changed there is not what is compared.
#[test]
fn a_commit_made_inside_the_workspace_is_not_read_as_the_base() {
    let f = Fixture::new(&["git status"], None);
    let first = f.run();
    assert_eq!(code(&first), 0, "{}", text(&first));

    let ws = f.workspace();
    std::fs::write(ws.join("suite-plan.json"), plan(&["cargo test"])).unwrap();
    let out = Command::new("git")
        .args([
            "-c",
            "user.email=s@example.invalid",
            "-c",
            "user.name=s",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-am",
            "a session's commit",
        ])
        .current_dir(&ws)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", text(&out));
    let session_commit = head_of(&ws);
    assert_ne!(session_commit, f.head());

    let second = f.run();
    let coverage = assert_launched_past_coverage(&f, &second, 2);
    assert_eq!(coverage["baseCommit"], f.head(), "{coverage}");
    assert_ne!(coverage["baseCommit"], session_commit.as_str());
    assert_eq!(coverage["commands"][0]["command"], "git status");
    assert_eq!(coverage["verdict"], "direct");
}

/// `export-ignore` does not change what the launch reading sees: the base's
/// tree is exported whole, so an attribute cannot hide the suite.
#[test]
fn an_export_ignore_attribute_does_not_change_the_plan_read_at_the_base() {
    let f = Fixture::new(&["git status"], None);
    std::fs::write(
        f.project().join(".gitattributes"),
        "suite-plan.json export-ignore\nspecs/ export-ignore\n",
    )
    .unwrap();
    f.git(&["add", "-A"]);
    f.git(&["commit", "--quiet", "-m", "hide the suite from an archive"]);
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let coverage = json_of(&out)["value"]["posture"]["value"]["coverage"].clone();
    assert_eq!(coverage["verdict"], "direct", "{coverage}");
    assert_eq!(coverage["commands"][0]["command"], "git status");
    assert!(coverage.get("drift").is_none(), "{coverage}");
}

/// A launch reading that cannot be made refuses the attempt under
/// `posture-coverage`, and `run show` renders the reason beside the coverage
/// line rather than a bare "not checked".
#[test]
fn a_launch_reading_that_fails_is_refused_and_run_show_says_why() {
    let f = Fixture::new(&["git status"], None);
    std::fs::write(f.bin().join("fail-outside-a-checkout"), "").unwrap();
    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let v = json_of(&out);
    assert_eq!(v["value"]["outcome"], "refused", "{v}");
    let unread = v["value"]["posture"]["value"]["coverage"]["unread"]
        .as_str()
        .unwrap_or_else(|| panic!("{v}"))
        .to_string();
    assert!(unread.contains("exit 4"), "{unread}");
    assert_eq!(f.launches(), 0);
    let human = text(&f.cli(&["run", "show", &f.root(), RUN]));
    assert!(
        human.contains(
            "command coverage: not checked, and the attempt was refused under posture-coverage"
        ),
        "{human}"
    );
    assert!(human.contains("no plan in the export"), "{human}");
}
