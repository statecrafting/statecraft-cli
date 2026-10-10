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

#[path = "support/json_naming.rs"]
mod json_naming;

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
    json_naming::from_output(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(out)))
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
        self.cli_with(args, &[])
    }

    /// [`Fixture::cli`] with more names in the operator's environment.
    fn cli_with(&self, args: &[&str], extra: &[(&str, &str)]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .envs(extra.iter().copied())
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
        std::fs::write(f.dir.path().join(".claude.json"), b"{}").unwrap();
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
            &as_linked(
                r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check|'check --help') exit 0 ;;
  compile|index) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"replay","title":"recorded stream"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"replay","status":"approved","implementation":"pending"}]}' ;;
  'verify '*' --plan --json') printf '{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"%s"},"schemaVersion":"0.6.0","verb":"verify"}' $2 ;;
  *) exit 3 ;;
esac
"#,
            ),
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
        let display = json_naming::payload(&v)["value"]["inspection"]["requiredDisplay"]
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
        std::fs::read(self.workspace().join(".fixture-output").join(name)).unwrap_or_default()
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
        std::fs::read_to_string(self.workspace().join(".fixture-output/launches"))
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
/// The stub answers as the linked producer: a fresh scaffold pins that
/// release exactly (spec 018 section 3.3), so initialization admits it.
fn as_linked(script: &str) -> String {
    let linked = format!("spec-spine {}", statecraft_home::producer::PRODUCER_VERSION);
    let stubbed = script.replace("spec-spine 0.20.0", &linked);
    assert!(stubbed.contains(&linked), "the stub reports a version");
    stubbed
}

const FAKE_PROVIDER: &str = r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267 (Claude Code)'; exit 0; fi
here="$(dirname "$0")"
output="$PWD/.fixture-output"
/bin/mkdir -p "$output"
settings=""
while [ "$#" -gt 0 ]; do
  case "$1" in --settings) settings="$2"; shift ;; esac
  shift
done
mode="$(/bin/cat "$here/mode" 2>/dev/null || echo faithful)"
/bin/cp "$settings" "$output/received-settings"
echo "$settings" > "$output/received-settings-path"
/bin/ls -A "$(dirname "$settings")" > "$output/exchange-listing"
/usr/bin/env > "$output/received-env"
/bin/cat > "$output/received-prompt"
echo launched >> "$output/launches"
log() { echo "$1" >> "$output/order"; }
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
    printf '{"tool_name":"Bash","tool_input":{"command":"touch %s"}}' "$1" | /bin/sh -c "$gate_cmd" 2>> "$output/gate-stderr"
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
# 3.37 rule 2), refused by spec 004 section 3.18: the gate's copy is beside
# the settings file.
if [ "$mode" = plant-decision ]; then
  if printf '{"decision":"admitted"}\n' > "$(dirname "$settings")/admission.json"; then
    log planted
  else
    log plant-refused
  fi
fi
if [ "$mode" = ignores-hooks ]; then
  : > "$PWD/sentinel-before-decision"
  log "effect sentinel-before-decision"
fi
if [ -f "$here/hostile" ]; then
  /bin/sh "$here/hostile" "$settings" > "$output/hostile-report" 2> "$output/hostile-errors"
  log "hostile exit=$?"
fi
if [ "$mode" = early-tool ]; then
  tool sentinel-early &
  early=$!
  /bin/sleep 0.5
  if kill -0 "$early" 2>/dev/null; then log "gate waiting before init"; fi
fi
/usr/bin/sed -n '1,3p' "$here/native.jsonl"
log "init emitted"
# Spec 003 section 3.5.1: structured write requests the session reports.
if [ -f "$here/write-request" ]; then /bin/cat "$here/write-request"; fi
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
    json_naming::payload(&json(out))["value"].clone()
}

fn position(order: &[String], line: &str) -> usize {
    order
        .iter()
        .position(|l| l == line)
        .unwrap_or_else(|| panic!("{line:?} is not in {order:?}"))
}

#[test]
fn a_hostile_provider_cannot_change_authority_or_redirect_supervisor_git() {
    let f = Fixture::new();
    let home = f.home().join("protected-fixture");
    let records = f.places().records.join("protected-fixture");
    let protected_chain =
        statecraft_run::record::chain_path(&f.home(), &f.project().join("another-project"));
    let protected_override =
        statecraft_run::overrides::journal_path(&f.home(), &f.project().join("another-project"));
    let protected_authority =
        statecraft_run::overrides::authority_path(&f.home(), &f.project().join("another-project"));
    let active_chain = statecraft_run::record::chain_path(&f.home(), &f.project());
    let target = f.project().join("protected-fixture");
    let other = f
        .project()
        .join(".statecraft/state/workspaces/other/protected-fixture");
    for path in [
        &home,
        &records,
        &protected_chain,
        &protected_override,
        &protected_authority,
        &target,
        &other,
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"authority").unwrap();
    }
    // The checkout sentinel is committed so its presence is ordinary input.
    f.git(&["add", "protected-fixture"]);
    f.git(&["commit", "--quiet", "-m", "protected input"]);
    let quote = |path: &Path| format!("'{}'", path.display().to_string().replace('\'', "'\\''"));
    let script = format!(
        r#"#!/bin/sh
home={home}
records={records}
target={target}
other={other}
chain={chain}
override={override_journal}
authority={authority}
active_chain={active_chain}
deny_mutations() {{
  path="$1"
  (printf changed > "$path") && exit 31
  (printf changed >> "$path") && exit 32
  rm "$path" && exit 33
  mv "$path" "$PWD/stolen" && exit 34
  chmod 777 "$path" && exit 35
  ln "$path" "$PWD/hard-alias" && exit 36
}}
for path in "$home" "$records" "$chain" "$override" "$authority" "$active_chain"; do
  cat "$path" && exit 30
  deny_mutations "$path"
  relative="$(/usr/bin/perl -MFile::Spec -e 'print File::Spec->abs2rel($ARGV[0], $ARGV[1])' "$path" "$PWD")"
  cat "$relative" && exit 52
  deny_mutations "$relative"
  variant="$(printf '%s' "$path" | tr '[:lower:]' '[:upper:]')"
  if [ "$(uname -s)" = Darwin ]; then
    cat "$variant" && exit 53
    deny_mutations "$variant"
  fi
  ln -s "$path" "$PWD/alias"
  cat "$PWD/alias" && exit 37
  (printf changed > "$PWD/alias") && exit 49
  chmod 777 "$PWD/alias" && exit 50
  rm "$PWD/alias"
done
for path in "$target" "$other"; do
  [ "$(cat "$path")" = authority ] || exit 38
  deny_mutations "$path"
  ln -s "$path" "$PWD/alias"
  [ "$(cat "$PWD/alias")" = authority ] || exit 39
  (printf changed > "$PWD/alias") && exit 49
  chmod 777 "$PWD/alias" && exit 50
  rm "$PWD/alias"
  relative="$(/usr/bin/perl -MFile::Spec -e 'print File::Spec->abs2rel($ARGV[0], $ARGV[1])' "$path" "$PWD")"
  [ "$(cat "$relative")" = authority ] || exit 56
  deny_mutations "$relative"
  if [ "$(uname -s)" = Darwin ]; then
    variant="$(printf '%s' "$path" | tr '[:lower:]' '[:upper:]')"
    [ "$(cat "$variant")" = authority ] || exit 57
    deny_mutations "$variant"
  fi
done
if [ "$(uname -s)" = Darwin ]; then
  cat {home_firmlink} && exit 58
  deny_mutations {home_firmlink}
  [ "$(cat {target_firmlink})" = authority ] || exit 59
  deny_mutations {target_firmlink}
fi
mv "$(dirname "$home")" "$PWD/stolen-home" && exit 54
mv "$(dirname "$target")" "$PWD/stolen-target" && exit 55
# A descendant that leaves the original session retains the same restrictions.
/usr/bin/perl -MPOSIX=setsid -MFcntl=O_WRONLY -e 'setsid() >= 0 or exit 1; open(my $read, "<", $ARGV[0]) and exit 2; sysopen(my $write, $ARGV[1], O_WRONLY) and exit 3; exit 0' "$home" "$target" || exit 51
gate="$(dirname "$1")/gate.log"
printf hostile-trace >> "$gate" || exit 40
chmod 777 "$gate" && exit 41
rm "$gate" && exit 42
printf forged > "$(dirname "$1")/admission.json" && exit 43
printf own > "$PWD/provider-commit"
git -c core.hooksPath=/dev/null add provider-commit || exit 44
git -c user.name=fixture -c user.email=fixture@example.invalid -c core.hooksPath=/dev/null commit -qm provider || exit 45
git update-ref refs/heads/statecraft/replay/extra HEAD || exit 46
# These paths are writable data. No later supervisor invocation follows them.
printf '%s\n' 'gitdir: /untrusted' > "$PWD/.git" || exit 47
printf '%s\n' /untrusted > "$GIT_DIR/commondir" || exit 48
printf '%s\n' passed
"#,
        home = quote(&home),
        records = quote(&records),
        target = quote(&target),
        other = quote(&other),
        chain = quote(&protected_chain),
        override_journal = quote(&protected_override),
        authority = quote(&protected_authority),
        active_chain = quote(&active_chain),
        home_firmlink = quote(
            &Path::new("/System/Volumes/Data")
                .join(home.canonicalize().unwrap().strip_prefix("/").unwrap(),)
        ),
        target_firmlink = quote(
            &Path::new("/System/Volumes/Data")
                .join(target.canonicalize().unwrap().strip_prefix("/").unwrap(),)
        ),
    );
    executable(&f.bin().join("hostile"), &script);
    let out = f.run();
    let chain_path = statecraft_run::record::chain_path(&f.home(), &f.project());
    assert_eq!(
        code(&out),
        0,
        "{}\nchain: {}",
        text(&out),
        std::fs::read_to_string(chain_path).unwrap()
    );
    assert!(
        f.order().contains(&"hostile exit=0".into()),
        "{:?}: {}",
        f.order(),
        String::from_utf8_lossy(&f.received("hostile-errors"))
    );
    assert_eq!(f.received("hostile-report"), b"passed\n");
    for path in [
        &home,
        &records,
        &protected_chain,
        &protected_override,
        &protected_authority,
        &target,
        &other,
    ] {
        assert_eq!(std::fs::read(path).unwrap(), b"authority");
    }
    let head = statecraft_run::trusted_git::output(
        &f.project(),
        &["show", "refs/heads/statecraft/replay/work:provider-commit"],
    )
    .unwrap();
    assert_eq!(head.stdout, b"own");
    let references = statecraft_run::trusted_git::output(
        &f.project(),
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads/statecraft/replay/",
        ],
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(references.stdout).unwrap().trim(),
        "refs/heads/statecraft/replay/work"
    );
    assert_eq!(
        json_naming::payload(&json(&out))["posture"]["value"]["confinement"]["platform"],
        std::env::consts::OS
    );
}

/// Object import failure still concludes with durable reference cleanup evidence.
#[test]
fn failed_object_import_still_records_removed_private_references() {
    let f = Fixture::new();
    executable(
        &f.bin().join("hostile"),
        r#"#!/bin/sh
printf own > "$PWD/provider-commit"
git -c core.hooksPath=/dev/null add provider-commit || exit 44
git -c user.name=fixture -c user.email=fixture@example.invalid -c core.hooksPath=/dev/null commit -qm provider || exit 45
git update-ref refs/heads/statecraft/replay/extra HEAD || exit 46
# Remove the private objects, so the confined producer cannot export this head.
rm -rf "$GIT_OBJECT_DIRECTORY"/* || exit 47
printf passed
"#,
    );
    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    assert!(
        f.order().contains(&"hostile exit=0".into()),
        "{:?}",
        f.order()
    );
    let chain = String::from_utf8(chain_bytes(&f)).unwrap();
    assert!(chain.contains("object-transfer"), "{chain}");
    assert!(chain.contains("privateReferenceCleanup"), "{chain}");
    assert!(
        chain.contains("refs/heads/statecraft/replay/extra"),
        "{chain}"
    );
    assert!(
        !f.project()
            .join(".git/refs/heads/statecraft/replay/extra")
            .exists()
    );
    assert!(f.exchange_dir(1).join("objects").is_dir());
}

/// Managed startup keeps each supplied identity bound to its launch evidence.
#[test]
fn a_managed_run_supplies_its_startup_hook_and_gate_and_releases_work_on_admission() {
    let f = Fixture::new();
    let revision = f.required_revision();

    let out = f.run();
    // Completed. Says nothing about acceptance and nothing about qualification.
    assert_eq!(code(&out), 0, "{}", text(&out));
    let answer = json(&out);
    let startup = &json_naming::payload(&answer)["startup"];
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
    assert_eq!(listing.len(), 5, "{listing:?}");
    assert_eq!(&listing[..2], ["admission-gate", "gate.log"]);
    assert_eq!(listing[2], "objects");
    assert!(listing[4].starts_with("temporary-"));
    assert!(
        listing[3].starts_with("statecraft-settings-"),
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
    let floor = json_naming::payload(&floor)["value"]["digest"]
        .as_str()
        .unwrap();
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
        (
            "STATECRAFT_SPEC_SPINE",
            f.bin().join("spec-spine").display().to_string(),
        ),
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
    let _ = std::fs::remove_file(f.workspace().join(".fixture-output/order"));
    let _ = std::fs::remove_file(f.workspace().join("sentinel-after"));

    let out = f.run();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let answer = json(&out);
    assert_eq!(json_naming::payload(&answer)["attempt"], 2);
    assert_eq!(
        json_naming::payload(&answer)["outcome"],
        "refused",
        "{answer}"
    );
    assert_eq!(
        json_naming::payload(&answer)["startup"]["verdict"],
        "mismatched"
    );

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

/// Spec 002 section 3.16's last rule and section 3.10's row "a global upgrade
/// after a run resolved": the home gains a newer harness revision and `home
/// apply` runs, and neither the attempt already resolved nor the run's next
/// attempt changes. Section 3.25 records the resolved identity per managed
/// session, which is where the freeze lives: each attempt's write-once intent
/// and record carry the requested (required) and the resolved identity, and a
/// later attempt selects the committed requirement, never the newest revision
/// the home holds (section 3.31 rule 17).
#[test]
fn a_global_upgrade_after_a_run_resolved_changes_nothing_about_that_run() {
    let f = Fixture::new();
    let required_dir = f.required_revision();
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    let first = value(&f.show(Some(1)));
    let required = first["intent"]["requiredHarness"]
        .as_str()
        .unwrap()
        .to_string();
    // Requested and resolved, both recorded, and they agree.
    assert_eq!(first["intent"]["selected"]["digest"], required.as_str());
    assert_eq!(first["record"]["resolvedHarness"], required.as_str());
    let program = first["intent"]["program"].clone();
    assert!(program.as_str().is_some_and(|p| p.ends_with("/claude")));
    let intent_before = std::fs::read(f.attempt_dir(1).join("intent.json")).unwrap();
    let record_before = std::fs::read(f.attempt_dir(1).join("record.json")).unwrap();

    // The global upgrade: a newer harness revision lands in the home, and the
    // operator's `home apply` runs. The project's committed requirement is not
    // touched; changing it is `harness upgrade`, a reviewed project change.
    let mut newer = statecraft_home::harness::shipped();
    newer[0].contents.push_str("\na newer harness revision\n");
    let installed =
        statecraft_home::harness::install(&statecraft_home::home::Layout::new(f.home()), &newer)
            .unwrap();
    assert_ne!(installed.revision.digest, required, "a different revision");
    let applied = f.cli(&["home", "apply"]);
    assert!(code(&applied) <= 1, "{}", text(&applied));
    assert!(installed.root.is_dir());
    let revisions = std::fs::read_dir(f.home().join("harness")).unwrap().count();
    assert!(revisions >= 2, "the home now holds {revisions} revision(s)");

    // The run's next attempt.
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert_eq!(json_naming::payload(&json(&out))["attempt"], 2);
    let second = value(&f.show(Some(2)));
    assert_eq!(second["intent"]["requiredHarness"], required.as_str());
    assert_eq!(second["intent"]["selected"]["digest"], required.as_str());
    assert_eq!(second["record"]["resolvedHarness"], required.as_str());
    assert_eq!(
        second["record"]["launch"]["harness"]["digest"],
        required.as_str()
    );
    assert_eq!(second["intent"]["program"], program, "the same tool");
    assert_eq!(
        Path::new(
            second["intent"]["registrations"][0]["script"]
                .as_str()
                .unwrap()
        ),
        required_dir.join("hooks/statecraft-session-start.sh"),
        "the startup hook is the resolved revision's, not the newer one's"
    );
    let env = String::from_utf8(f.received("received-env")).unwrap();
    assert!(
        env.lines()
            .any(|l| l == format!("STATECRAFT_HARNESS_SELECTED={required}")),
        "{env}"
    );

    // The attempt already resolved is unchanged, byte for byte.
    assert_eq!(
        std::fs::read(f.attempt_dir(1).join("intent.json")).unwrap(),
        intent_before
    );
    assert_eq!(
        std::fs::read(f.attempt_dir(1).join("record.json")).unwrap(),
        record_before
    );
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
    assert_eq!(json_naming::payload(&json(&out))["outcome"], "refused");
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
    let refusal = json_naming::payload(&json(&again)).clone();
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
    assert_eq!(code(&out), 2, "{}", text(&out));
    let answer = json(&out);
    assert_eq!(json_naming::payload(&answer)["outcome"], "refused");
    assert_eq!(json_naming::payload(&answer)["reason"], "startup-record");
    assert_eq!(json_naming::payload(&answer)["startup"]["launched"], false);
    assert!(
        json_naming::payload(&answer)["startup"]["error"]
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
    // Inject the storage failure from the trusted test parent. A confined
    // provider cannot mutate launch records to simulate a disk failure.
    f.mode("block");
    let out = std::thread::scope(|scope| {
        let child = scope.spawn(|| f.run());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !f.order().iter().any(|line| line == "blocked") {
            assert!(
                std::time::Instant::now() < deadline,
                "provider did not block"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::fs::create_dir(f.attempt_dir(1).join("record.json")).unwrap();
        std::fs::write(f.bin().join("release"), "").unwrap();
        child.join().unwrap()
    });
    assert_eq!(code(&out), 4, "{}", text(&out));
    let answer = json(&out);
    assert_eq!(json_naming::payload(&answer)["startup"]["launched"], true);
    assert_eq!(
        json_naming::payload(&answer)["startup"]["record"],
        serde_json::Value::Null
    );
    assert!(
        json_naming::payload(&answer)["startup"]["error"]
            .as_str()
            .is_some()
    );
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
    let v = json_naming::payload(&json(&refused)).clone();
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
    let row = json_naming::payload(&json(&listed))["runs"][0]["attempts"][0].clone();
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
    let r = json_naming::payload(&json(&out)).clone();
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
    let recs = json_naming::payload(&json(&shown))["reconciliations"].clone();
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
        json_naming::payload(&json(&out))["observed"]["gateReleasedToolCall"],
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
    assert_eq!(json_naming::payload(&json(&out))["corroborated"], true);
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
    let row = json_naming::payload(&json(&listed))["runs"][0]["attempts"][0].clone();
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
    let r = json_naming::payload(&json(&out)).clone();
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
    let row = json_naming::payload(&json(&listed))["runs"][0]["attempts"][0].clone();
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
    assert_eq!(json_naming::payload(&json(&out))["retryAllowed"], true);
    assert!(text(&out).contains("confirmed"), "{}", text(&out));

    let shown = f.cli(&["run", "show", &f.root(), RUN, "--json"]);
    let recs = json_naming::payload(&json(&shown))["reconciliations"].clone();
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
    // The launch records are keyed as the chain is: the same attempt's
    // records in the product home answer under that spelling too.
    let shown = f.cli(&["startup", "show", &spelled, RUN, "--attempt", "1", "--json"]);
    assert_eq!(value(&shown)["placement"], "home", "{}", text(&shown));
    assert_eq!(value(&shown), value(&f.show(Some(1))));

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
    let answer: serde_json::Value = json_naming::from_output(&out.stdout).unwrap();
    assert!(
        json_naming::payload(&answer)["refused"].is_string(),
        "{answer}"
    );

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
    let observed = json_naming::payload(&json(&out))["observed"].clone();
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

/// Rule 2 and spec 004 section 3.18: the child's attempted decision mutation
/// is refused, and only the supervisor's admitted decision reaches the gate.
#[test]
fn a_decision_the_child_planted_before_the_supervisors_is_not_the_decision() {
    let f = Fixture::new();
    f.mode("plant-decision");
    let out = f.run();
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(
        f.order().iter().any(|l| l == "plant-refused"),
        "{:?}",
        f.order()
    );
    assert!(!f.order().iter().any(|l| l == "planted"));
    assert_eq!(
        std::fs::read(f.exchange_dir(1).join("admission.json")).unwrap(),
        std::fs::read(f.attempt_dir(1).join("admission.json")).unwrap()
    );
    // The OS denies the write inside the child, so the supervisor observes no
    // event to record: the refusal is proven by the child's own report and the
    // unchanged admission bytes above, and the verdict judges the run alone.
    let v = value(&f.show(None));
    assert_eq!(v["verdict"], "unverified", "{v}");
    assert!(f.workspace().join("sentinel-after").is_file());
    assert!(v["record"]["launch"]["admissionError"].is_null());
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
            if entry.file_type().unwrap().is_dir() {
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
    let observed = json_naming::payload(&json(&out))["observed"].clone();
    assert_eq!(observed["gateReleasedToolCall"], true, "{observed}");
    assert_eq!(observed["confirmedPid"].to_string(), pid);
    for file in observed["files"].as_array().unwrap() {
        assert!(
            Path::new(file.as_str().unwrap()).starts_with(f.project()),
            "{observed}"
        );
    }
}

/// Spec 002 section 5, 2026-09-25: one variable selects the spec-spine binary.
/// The supervisor sets `STATECRAFT_SPEC_SPINE` in the constructed environment
/// from its own selection, and a value in the operator's environment, under
/// either name, neither reaches the session nor changes what is selected.
///
/// Spec 029: outside a session the operator's value is the override the verb's
/// own reads honor, so the inherited binary is a working engine and records
/// only calls made from inside the managed session.
#[test]
fn a_managed_run_replaces_an_inherited_spec_spine_selection_with_the_supervisors() {
    let f = Fixture::new();
    let inherited = f.dir.path().join("inherited-spec-spine");
    executable(
        &inherited,
        &format!(
            "#!/bin/sh\nif [ -n \"$STATECRAFT_RUN_ID\" ]; then echo inherited >> \"$0.calls\"; fi\n\
             exec {} \"$@\"\n",
            f.bin().join("spec-spine").display()
        ),
    );
    let inherited_s = inherited.display().to_string();
    let out = f.cli_with(
        &["run", &f.root(), RUN, "--json"],
        &[
            ("STATECRAFT_SPEC_SPINE", inherited_s.as_str()),
            ("SPEC_SPINE_BIN", inherited_s.as_str()),
        ],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    let env = String::from_utf8(f.received("received-env")).unwrap();
    let values: Vec<&str> = env
        .lines()
        .filter_map(|l| l.strip_prefix("STATECRAFT_SPEC_SPINE="))
        .collect();
    assert_eq!(
        values,
        [f.bin().join("spec-spine").display().to_string()],
        "{env}"
    );
    assert!(
        !env.lines().any(|l| l.starts_with("SPEC_SPINE_BIN=")),
        "the retired name reached the session: {env}"
    );
    assert!(
        !f.dir.path().join("inherited-spec-spine.calls").exists(),
        "the operator's binary was invoked inside the session"
    );
}

/// Spec 003 section 3.5.1 rule 1, first kind: a `Write` into the product home
/// is one finding with its tool-use id and classification, beside a refusal
/// count it does not change; an `Edit` inside the workspace is none.
#[test]
fn a_write_request_into_the_home_is_a_finding_and_one_into_the_workspace_is_not() {
    let f = Fixture::new();
    let forged = f.home().join("records").join("forged.jsonl");
    let line = serde_json::json!({
        "type": "assistant",
        "message": {"content": [
            {"type": "tool_use", "id": "toolu_home", "name": "Write",
             "input": {"file_path": forged.display().to_string(), "content": "x"}},
            {"type": "tool_use", "id": "toolu_work", "name": "Edit",
             "input": {"file_path": "src/lib.rs"}}
        ]}
    });
    std::fs::write(f.bin().join("write-request"), format!("{line}\n")).unwrap();
    let out = f.run();
    assert!(code(&out) <= 1, "{}", text(&out));

    let (chain, _) = statecraft_run::record::Chain::open(&f.home(), &f.project()).unwrap();
    let accounting = chain
        .entries()
        .into_iter()
        .find(|e| e.kind == statecraft_run::record::Kind::Accounting && e.run_id == RUN)
        .expect("the accounting record");
    assert_eq!(accounting.detail["count"], 0, "{}", accounting.detail);
    let findings = accounting.detail["tamperFindings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{}", accounting.detail);
    assert_eq!(findings[0]["toolUseId"], "toolu_home");
    assert_eq!(findings[0]["tool"], "Write");
    assert_eq!(findings[0]["classification"], "unresolved");
    assert!(accounting.detail.get("tamper_attempts").is_none());
    assert!(!forged.exists(), "the request was only a request");

    let shown = f.cli(&["run", "show", &f.root(), RUN, "--json"]);
    let tamper = json_naming::payload(&json(&shown))["tamper"][0].clone();
    assert_eq!(tamper["recorded"], "checked", "{tamper}");
    assert_eq!(tamper["writeRequest"], "observed", "{tamper}");
    assert_eq!(tamper["recordChange"], "none-observed", "{tamper}");
    let human = f.cli(&["run", "show", &f.root(), RUN]);
    assert!(
        text(&human).contains("write requests observed; record changes none observed"),
        "{}",
        text(&human)
    );
    assert!(text(&human).contains("toolu_home"), "{}", text(&human));
    assert!(!text(&human).contains("toolu_work"), "{}", text(&human));
}

/// Start `run` in `mode` with its answer captured, and wait until the fake
/// blocks after the startup decision.
fn run_captured_until_blocked(f: &Fixture, mode: &str) -> std::process::Child {
    f.mode(mode);
    let launcher = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
        .args(["run", &f.root(), RUN, "--json"])
        .env_clear()
        .env("STATECRAFT_HOME", f.home())
        .env("STATECRAFT_NATIVE_ROOT", f.dir.path().join("native"))
        .env("HOME", f.dir.path())
        .env("PATH", format!("{}:/usr/bin:/bin", f.bin().display()))
        .env("USER", "fixture-operator")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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
    launcher
}

/// Append a well-formed entry to the run record from outside the supervisor,
/// as a session reaching the home would: the record still verifies.
fn forge_an_entry(f: &Fixture) {
    let (mut chain, _) = statecraft_run::record::Chain::open(&f.home(), &f.project()).unwrap();
    let mut entry = chain
        .entries()
        .first()
        .cloned()
        .expect("the attempt's intent is already recorded");
    entry.run_id = "forged-by-the-session".to_string();
    chain
        .append("forged", "2026-10-08T00:00:00Z", &entry)
        .unwrap();
}

/// Spec 003 section 3.5.1 rule 1, second kind, and rule 2: a change to the
/// record while the process ran is written to the audit file, nothing more is
/// appended, the attempt stays live, and `run show` and a later `run` name it.
#[test]
fn a_change_to_the_record_while_the_process_ran_is_audited_and_the_attempt_stays_live() {
    let f = Fixture::new();
    let launcher = run_captured_until_blocked(&f, "block");
    forge_an_entry(&f);
    let forged = chain_bytes(&f);
    std::fs::write(f.bin().join("release"), "").unwrap();
    let out = launcher.wait_with_output().unwrap();
    assert_eq!(code(&out), 1, "{}", text(&out));
    let v = json_naming::payload(&json(&out)).clone();
    assert_eq!(v["recorded"], true, "{v}");
    assert_eq!(v["attemptLive"], true, "{v}");
    assert!(!v["next"].as_str().unwrap().contains("  "), "{v}");
    let changes = v["finding"]["changes"].as_array().unwrap();
    assert!(
        changes
            .iter()
            .any(|c| c["path"].as_str().unwrap().ends_with(".jsonl")
                && c["before"]["length"].as_u64() < c["after"]["length"].as_u64()),
        "{v}"
    );
    assert_eq!(
        chain_bytes(&f),
        forged,
        "nothing was appended after the change"
    );
    let (chain, _) = statecraft_run::record::Chain::open(&f.home(), &f.project()).unwrap();
    assert!(
        !chain
            .entries()
            .iter()
            .any(|e| e.run_id == RUN && e.kind == statecraft_run::record::Kind::Accounting),
        "no outcome or accounting for the attempt"
    );
    let audit = statecraft_run::tamper::read_audit(&f.home(), &f.project()).unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!((audit[0].run_id.as_str(), audit[0].attempt), (RUN, 1));

    let shown = f.cli(&["run", "show", &f.root(), RUN]);
    assert!(
        text(&shown).contains("a change to the record this product did not make"),
        "{}",
        text(&shown)
    );
    let shown = f.cli(&["run", "show", &f.root(), RUN, "--json"]);
    let tamper = json_naming::payload(&json(&shown))["tamper"][0].clone();
    assert_eq!(tamper["recordChange"], "observed", "{tamper}");

    let later = f.run();
    assert_eq!(code(&later), 2, "{}", text(&later));
    let refusal = json_naming::payload(&json(&later))["audit"].clone();
    assert_eq!(refusal[0]["runId"], RUN, "{}", text(&later));
    assert_eq!(f.launches(), 1, "nothing was launched for the later run");

    // The record still verifies, so the attempt goes to reconciliation, whose
    // answer names the finding.
    let state = json_naming::payload(&json(&later))["launchState"]
        .as_str()
        .unwrap_or_else(|| panic!("{}", text(&later)))
        .to_string();
    let reconciled = reconcile(&f, "unknown", &state, &["--json"]);
    assert_eq!(code(&reconciled), 0, "{}", text(&reconciled));
    let named = json_naming::payload(&json(&reconciled))["audit"].clone();
    assert_eq!(named[0]["attempt"], 1, "{}", text(&reconciled));
}

/// Spec 003 section 3.5.1 rule 3: an audit file that cannot be written is a
/// failure that prints the finding, never a silence, and the attempt stays
/// live.
#[test]
fn an_audit_file_that_cannot_be_written_fails_and_prints_the_finding() {
    let f = Fixture::new();
    let launcher = run_captured_until_blocked(&f, "block");
    std::fs::create_dir_all(statecraft_run::tamper::audit_path(&f.home(), &f.project())).unwrap();
    forge_an_entry(&f);
    std::fs::write(f.bin().join("release"), "").unwrap();
    let out = launcher.wait_with_output().unwrap();
    assert_eq!(code(&out), 4, "{}", text(&out));
    let v = json_naming::payload(&json(&out)).clone();
    assert_eq!(v["recorded"], false, "{v}");
    assert!(v["auditError"].is_string(), "{v}");
    assert_eq!(v["finding"]["runId"], RUN, "{v}");
    assert_eq!(v["attemptLive"], true, "{v}");
}
