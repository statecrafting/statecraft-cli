//! What initialization reports, and in what order it decides: spec 002
//! section 5, the 2026-09-24 entry that amends section 3.17, through the built
//! binary on a real directory.
//!
//! Every test compares the report's mutation list with a walk of the project
//! root (outside `.git`) and the product home taken before and after, so the
//! list is checked against the disk and never against the plan. Failures are
//! arranged on disk: permission bits, a directory where a file belongs, a held
//! lock, and a stub `spec-spine` whose answers are files beside it. A test
//! that needs permission bits to refuse skips, saying so, when it runs as
//! root.

#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A `spec-spine` whose behavior is set by files beside it: `check-exit`,
/// `compile-mode` (`ok`, or `signal`), and `vanish`, which removes the stub
/// after its first `check --help`.
const STUB: &str = r#"#!/bin/sh
here="$(dirname "$0")"
case "$*" in
  --version) echo 'spec-spine 0.23.0' ;;
  'check --help')
    if [ -f "$here/vanish" ]; then /bin/rm -f "$0"; fi
    exit 0 ;;
  check) exit "$(/bin/cat "$here/check-exit")" ;;
  compile)
    if [ "$(/bin/cat "$here/compile-mode")" = signal ]; then kill -9 $$; fi
    exit 0 ;;
  index) exit 0 ;;
  *) exit 3 ;;
esac
"#;

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(f.bin()).unwrap();
        std::fs::create_dir_all(f.project()).unwrap();
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "base"],
        ] {
            let out = Command::new("git")
                .args(&args)
                .current_dir(f.project())
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        }
        statecraft_adapter::fixture::install_script(&f.bin().join("spec-spine"), STUB, 0o755)
            .unwrap();
        f.set("check-exit", "0");
        f.set("compile-mode", "ok");
        f
    }

    fn set(&self, name: &str, value: &str) {
        std::fs::write(self.bin().join(name), value).unwrap();
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }
    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn at(&self, rel: &str) -> PathBuf {
        self.project().join(rel)
    }

    fn cli(&self, verb: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args([
                "init",
                verb,
                &self.project().display().to_string(),
                "--json",
            ])
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    /// Every path under the project (outside `.git`) and the home, with its
    /// state: a digest, or `directory`.
    fn walk(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let project = std::fs::canonicalize(self.project()).unwrap();
        walk_into(&project, &project, true, &mut out);
        let home_parent = std::fs::canonicalize(self.dir.path()).unwrap();
        let home = home_parent.join("home");
        if home.exists() {
            out.insert(home.display().to_string(), "directory".to_string());
            walk_into(&home, &home, false, &mut out);
        }
        out
    }

    /// Run `init <verb>` between two walks, and hand back the exit, the report
    /// and the change the walks saw.
    fn run(&self, verb: &str) -> Run {
        let before = self.walk();
        let out = self.cli(verb);
        let after = self.walk();
        let answer: serde_json::Value =
            serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out)));
        let report = answer["value"]["value"].clone();
        assert!(report.is_object(), "{answer}");
        let mut seen = BTreeSet::new();
        for path in before.keys().chain(after.keys()) {
            let was = before.get(path).cloned().unwrap_or_else(|| "absent".into());
            let now = after.get(path).cloned().unwrap_or_else(|| "removed".into());
            if was != now {
                seen.insert((path.clone(), was, now));
            }
        }
        Run {
            exit: out.status.code().expect("exited by itself"),
            report,
            seen,
            text: text(&out),
        }
    }
}

fn walk_into(base: &Path, dir: &Path, relative: bool, out: &mut BTreeMap<String, String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if relative && path == base.join(".git") {
            continue;
        }
        let name = if relative {
            path.strip_prefix(base).unwrap().display().to_string()
        } else {
            path.display().to_string()
        };
        if entry.file_type().unwrap().is_dir() {
            out.insert(name, "directory".to_string());
            walk_into(base, &path, relative, out);
        } else {
            let state = std::fs::read(&path)
                .map(|b| statecraft_environment::digest::digest_bytes(&b))
                .unwrap_or_else(|_| "unreadable".to_string());
            out.insert(name, state);
        }
    }
}

struct Run {
    exit: i32,
    report: serde_json::Value,
    seen: BTreeSet<(String, String, String)>,
    text: String,
}

impl Run {
    fn outcome(&self) -> &str {
        self.report["outcome"].as_str().unwrap()
    }

    fn step(&self, name: &str) -> serde_json::Value {
        self.report["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["step"] == name)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }

    /// The list composed per path: an entry is one operation, and a path
    /// written twice (the progress record, step by step) shows its first
    /// state before and its last state after, which is what a walk sees.
    fn listed(&self) -> BTreeSet<(String, String, String)> {
        let mut net: BTreeMap<String, (String, String)> = BTreeMap::new();
        for m in self.report["mutations"].as_array().unwrap() {
            let path = m["path"].as_str().unwrap().to_string();
            let before = m["before"].as_str().unwrap().to_string();
            let after = m["after"].as_str().unwrap().to_string();
            net.entry(path)
                .and_modify(|e| e.1 = after.clone())
                .or_insert((before, after));
        }
        net.into_iter()
            .filter(|(_, (b, a))| b != a)
            .map(|(p, (b, a))| (p, b, a))
            .collect()
    }

    fn listed_paths(&self) -> BTreeSet<String> {
        self.listed().into_iter().map(|(p, _, _)| p).collect()
    }

    /// Obligation 5: the report's list equals what the walks saw, path for
    /// path and state for state.
    fn list_is_the_disk(&self) {
        assert_eq!(
            self.listed(),
            self.seen,
            "the mutation list is not what the disk shows\n{}",
            self.text
        );
    }

    fn only_the_lock(&self) {
        let lock: BTreeSet<String> = [
            ".statecraft",
            ".statecraft/state",
            ".statecraft/state/manifest.lock",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert!(
            self.listed_paths().is_subset(&lock),
            "more than the lock was written: {:?}",
            self.listed_paths()
        );
        for m in self.report["mutations"].as_array().unwrap() {
            assert_eq!(m["step"], "lock", "{m}");
            let expected = if m["path"] == ".statecraft" {
                "project"
            } else {
                "project-state"
            };
            assert_eq!(m["category"], expected, "{m}");
        }
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}

fn chmod(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

// A fresh project with a working corpus tool: complete, 0, and every change
// on the list.
#[test]
fn a_fresh_initialization_is_complete_and_lists_every_change() {
    let f = Fixture::new();
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (0, "complete"), "{}", r.text);
    r.list_is_the_disk();
    let paths = r.listed_paths();
    for expected in [
        ".statecraft/state/manifest.lock",
        ".statecraft/state/init.json",
        ".statecraft/environment.json",
        ".statecraft/AGENTS.md",
        ".gitignore",
        "AGENTS.md",
    ] {
        assert!(paths.contains(expected), "{expected} not listed: {paths:?}");
    }
    let home = std::fs::canonicalize(f.dir.path()).unwrap().join("home");
    assert!(
        paths.contains(&home.join("projects.json").display().to_string()),
        "{paths:?}"
    );
    for m in r.report["mutations"].as_array().unwrap() {
        let path = m["path"].as_str().unwrap();
        let expected = if path.starts_with('/') {
            "home"
        } else if path.starts_with(".statecraft/state") {
            "project-state"
        } else {
            "project"
        };
        assert_eq!(m["category"], expected, "{m}");
    }
}

// `init plan` runs the preflight alone: no lock, no directory, no file, in the
// project or in the home.
#[test]
fn init_plan_changes_nothing_on_disk() {
    let f = Fixture::new();
    let r = f.run("plan");
    assert_eq!(r.exit, 0, "{}", r.text);
    assert!(r.seen.is_empty(), "plan changed the disk: {:?}", r.seen);
    assert!(r.listed().is_empty());
    assert!(!f.at(".statecraft").exists());
    assert!(!f.home().exists());
}

// The same for a plan that would refuse.
#[test]
fn a_refusing_plan_changes_nothing_either() {
    let f = Fixture::new();
    std::fs::write(f.at(".gitignore"), "target\n.statecraft/\n").unwrap();
    let r = f.run("plan");
    assert_eq!((r.exit, r.outcome()), (2, "refused"), "{}", r.text);
    assert!(r.seen.is_empty(), "plan changed the disk: {:?}", r.seen);
}

// T1: the ignore rules would take the area out of version control. Refused in
// the preflight: the lock is all that was written, and nothing in the home.
#[test]
fn t1_an_ignored_area_refuses_before_any_write_but_the_lock() {
    let f = Fixture::new();
    std::fs::write(f.at(".gitignore"), "target\n.statecraft/\n").unwrap();
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (2, "refused"), "{}", r.text);
    r.list_is_the_disk();
    r.only_the_lock();
    assert!(r.listed_paths().contains(".statecraft/state/manifest.lock"));
    assert!(
        !f.home().exists(),
        "the home was written before the refusal"
    );
    assert_eq!(r.step("governance")["phase"], "preflight");
}

// T2: another writer holds the manifest lock past the wait.
#[test]
fn t2_a_held_lock_refuses_and_writes_nothing_more() {
    let f = Fixture::new();
    // Taken in this process, which is another process to the binary.
    let _held =
        statecraft_environment::manifest::lock(&f.project(), std::time::Duration::from_secs(1))
            .unwrap();
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (2, "refused"), "{}", r.text);
    r.list_is_the_disk();
    assert!(r.listed().is_empty(), "{:?}", r.listed());
    assert!(!f.home().exists());
}

// T3: an unreadable declaration is an execution error: failed, 4, and the
// lock is all that was written.
#[test]
fn t3_an_unreadable_declaration_fails_with_only_the_lock_written() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.at(".statecraft/environment.json")).unwrap();
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (4, "failed"), "{}", r.text);
    r.list_is_the_disk();
    r.only_the_lock();
    assert!(!f.home().exists());
}

// T4: a governance file cannot be written. Failed, 4, and what was written
// before the error is on the list.
#[test]
fn t4_a_governance_write_error_fails_and_lists_what_was_written() {
    if root() {
        eprintln!("skipped: running as root, where permission bits do not refuse");
        return;
    }
    let f = Fixture::new();
    std::fs::create_dir_all(f.at("standards")).unwrap();
    chmod(&f.at("standards"), 0o555);
    let r = f.run("apply");
    chmod(&f.at("standards"), 0o755);
    assert_eq!((r.exit, r.outcome()), (4, "failed"), "{}", r.text);
    assert_eq!(
        r.step("governance")["state"]["state"],
        "failed",
        "{}",
        r.text
    );
    r.list_is_the_disk();
    let home = std::fs::canonicalize(f.dir.path()).unwrap().join("home");
    assert!(
        r.listed_paths()
            .contains(&home.join("tools.json").display().to_string()),
        "the home writes are listed"
    );
    assert!(!f.at(".statecraft/environment.json").exists());
}

// T5: the product home cannot be read. Failed in the preflight, before any
// project write or home write.
#[test]
fn t5_an_unreadable_home_fails_before_any_write() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.home().join("tools.json")).unwrap();
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (4, "failed"), "{}", r.text);
    r.list_is_the_disk();
    r.only_the_lock();
    assert_eq!(r.step("home")["state"]["state"], "failed");
}

// T6: the root instructions cannot be written. Failed, 4; the governance
// files and the ignore rules written before it are listed, and the root file
// is unchanged.
#[test]
fn t6_an_unwritable_root_instruction_file_fails_and_is_unchanged() {
    if root() {
        eprintln!("skipped: running as root, where permission bits do not refuse");
        return;
    }
    let f = Fixture::new();
    std::fs::write(f.at("AGENTS.md"), "# mine\n").unwrap();
    chmod(&f.at("AGENTS.md"), 0o444);
    let r = f.run("apply");
    chmod(&f.at("AGENTS.md"), 0o644);
    assert_eq!((r.exit, r.outcome()), (4, "failed"), "{}", r.text);
    r.list_is_the_disk();
    assert_eq!(
        std::fs::read_to_string(f.at("AGENTS.md")).unwrap(),
        "# mine\n"
    );
    let paths = r.listed_paths();
    assert!(paths.contains(".gitignore"), "{paths:?}");
    assert!(paths.contains(".statecraft/AGENTS.md"), "{paths:?}");
    assert!(!paths.contains("AGENTS.md"), "{paths:?}");
}

// T7: no corpus tool. Steps 6 and 7 are degradable: refused in the preflight,
// the rest performed, partial, 1.
#[test]
fn t7_no_corpus_tool_is_partial_with_both_refusals_from_the_preflight() {
    let f = Fixture::new();
    std::fs::remove_file(f.bin().join("spec-spine")).unwrap();
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (1, "partial"), "{}", r.text);
    r.list_is_the_disk();
    assert_eq!(r.step("corpus")["state"]["state"], "refused");
    assert_eq!(r.step("corpus")["phase"], "preflight");
    assert_eq!(r.step("register")["state"]["state"], "refused");
    assert_eq!(r.step("register")["phase"], "preflight");
    assert!(r.listed_paths().contains(".statecraft/environment.json"));
}

// T8: the tool was there in the preflight and is gone by step 6: a late
// refusal, and still partial.
#[test]
fn t8_a_tool_gone_after_the_preflight_is_a_late_refusal() {
    let f = Fixture::new();
    f.set("vanish", "");
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (1, "partial"), "{}", r.text);
    r.list_is_the_disk();
    assert_eq!(r.step("corpus")["state"]["state"], "refused");
    assert_eq!(r.step("corpus")["phase"], "late");
}

// T9: `check` did not perform its read (exit 3): failed, 4, with everything
// written before it listed.
#[test]
fn t9_a_check_that_did_not_perform_fails() {
    let f = Fixture::new();
    f.set("check-exit", "3");
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (4, "failed"), "{}", r.text);
    r.list_is_the_disk();
    assert_eq!(r.step("corpus")["state"]["state"], "failed");
    assert!(r.listed_paths().contains(".statecraft/environment.json"));
}

// T10: `compile` is killed by a signal: the verb did not perform, failed, 4.
#[test]
fn t10_a_compile_killed_by_a_signal_fails() {
    let f = Fixture::new();
    f.set("compile-mode", "signal");
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (4, "failed"), "{}", r.text);
    r.list_is_the_disk();
    assert_eq!(r.step("corpus")["state"]["state"], "failed");
}

// T11: the progress record cannot be written: failed, 4.
#[test]
fn t11_an_unwritable_progress_record_fails() {
    if root() {
        eprintln!("skipped: running as root, where permission bits do not refuse");
        return;
    }
    let f = Fixture::new();
    let state = f.at(".statecraft/state");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("manifest.lock"), "").unwrap();
    chmod(&state, 0o555);
    let r = f.run("apply");
    chmod(&state, 0o755);
    assert_eq!((r.exit, r.outcome()), (4, "failed"), "{}", r.text);
    r.list_is_the_disk();
    assert!(r.text.contains("init.json"), "{}", r.text);
}

// T12: a re-run of an initialized project is complete, and lists only what it
// actually changed: no project file outside the runtime state.
#[test]
fn t12_a_re_run_lists_only_what_changed() {
    let f = Fixture::new();
    let first = f.run("apply");
    assert_eq!(first.exit, 0, "{}", first.text);
    let r = f.run("apply");
    assert_eq!((r.exit, r.outcome()), (0, "complete"), "{}", r.text);
    r.list_is_the_disk();
    // The declaration is this product's own record and is rewritten under
    // the lock; it is reported because it changed. No other project path is.
    for m in r.report["mutations"].as_array().unwrap() {
        if m["category"] == "project" {
            assert_eq!(m["path"], ".statecraft/environment.json", "{m}");
        }
    }
}
