//! The readiness override, through the built binary (spec 003 section 3.1.4,
//! spec 006 section 3.11.5).
//!
//! Every invocation gets a temporary `STATECRAFT_HOME` and `HOME`, and a `PATH`
//! holding only a fake `spec-spine`, a fake `claude` and the system
//! directories. The fake `spec-spine` offers one spec as ready whose status is
//! `draft`, which is what section 3.1.1's default refuses. The fake provider
//! replays a recorded Claude Code stream and runs nothing. **No provider is
//! spawned, and nothing here is live evidence.**

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DRAFT: &str = "009-draft";

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

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }
    fn project(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(f.bin()).unwrap();
        executable(
            &f.bin().join("spec-spine"),
            &format!(
                r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.23.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{{"ready":[{{"id":"{DRAFT}","title":"a draft","status":"draft"}}]}}' ;;
  'registry list --json') echo '{{"items":[{{"id":"{DRAFT}","status":"draft","implementation":"pending"}},{{"id":"010-unready","status":"draft","implementation":"pending"}}]}}' ;;
  'verify '*' --plan --json') printf '{{"exitCode":0,"ok":true,"report":{{"commands":[],"skipped":[],"specId":"%s"}},"schemaVersion":"0.6.0","verb":"verify"}}' $2 ;;
  *) exit 3 ;;
esac
"#
            ),
        );
        let recorded = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl");
        std::fs::copy(recorded, f.bin().join("native.jsonl")).unwrap();
        executable(
            &f.bin().join("claude"),
            r#"#!/bin/sh
here="$(dirname "$0")"
if [ "${1:-}" = "--version" ]; then echo '2.1.267 (Claude Code)'; exit 0; fi
cat > /dev/null
cat "$here/native.jsonl"
"#,
        );
        f
    }

    /// A registered, armed git repository.
    fn repository(&self, name: &str) -> String {
        let p = self.project(name);
        std::fs::create_dir_all(&p).unwrap();
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            let out = Command::new("git")
                .args(&args)
                .current_dir(&p)
                .output()
                .unwrap();
            assert!(out.status.success());
        }
        std::fs::write(p.join("README.md"), "x\n").unwrap();
        for args in [vec!["add", "."], vec!["commit", "--quiet", "-m", "base"]] {
            let out = Command::new("git")
                .args(&args)
                .current_dir(&p)
                .output()
                .unwrap();
            assert!(out.status.success());
        }
        let root = p.display().to_string();
        for verb in ["register", "arm"] {
            let out = self.cli(&["project", verb, &root]);
            assert!(code(&out) <= 1, "{verb}: {}", text(&out));
        }
        root
    }
}

#[test]
fn a_draft_is_refused_by_default_admitted_by_an_override_and_refused_again_after_revocation() {
    let f = Fixture::new();
    let root = f.repository("a");

    // Default refusal: listed as excluded, and `run` does not schedule it.
    let out = f.cli(&["work", "show", &root, DRAFT]);
    assert!(text(&out).contains("excluded"), "{}", text(&out));
    let out = f.cli(&["run", &root, DRAFT]);
    assert_ne!(code(&out), 0, "{}", text(&out));
    assert!(text(&out).contains("excluded"), "{}", text(&out));
    let listed = f.cli(&["run", "list", &root]);
    assert!(
        text(&listed).contains("no runs recorded"),
        "{}",
        text(&listed)
    );

    // The grant, and what it shows.
    let out = f.cli(&[
        "override",
        "grant",
        &root,
        DRAFT,
        "alice",
        "reviewing",
        "the",
        "draft",
    ]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(text(&out).contains("operator-supplied"), "{}", text(&out));
    assert!(text(&out).contains("does not ratify"), "{}", text(&out));
    let shown = f.cli(&["override", "show", &root, "--json"]);
    assert_eq!(code(&shown), 0);
    let v: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    let in_force = &v["value"]["inForce"];
    assert_eq!(in_force.as_array().unwrap().len(), 1, "{v}");
    assert_eq!(in_force[0]["specId"], DRAFT);
    assert_eq!(in_force[0]["reason"], "reviewing the draft");

    // `work show` now names the override, and `run` schedules it; the intent
    // records how it was admitted.
    let out = f.cli(&["work", "show", &root, DRAFT]);
    assert!(text(&out).contains("eligible"), "{}", text(&out));
    let out = f.cli(&["run", &root, DRAFT]);
    assert!(code(&out) <= 1, "{}", text(&out));
    let listed = f.cli(&["run", "list", &root, "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    let admission = &v["value"]["runs"][0]["attempts"][0]["admission"];
    assert_eq!(admission["by"], "override", "{v}");
    assert_eq!(admission["operator"], "alice");
    assert_eq!(admission["operator_provenance"], "operator-supplied");
    assert!(admission["grant_line"].as_str().unwrap().len() == 64);
    let shown = f.cli(&["run", "show", &root, DRAFT]);
    assert!(
        text(&shown).contains("attempt 1 admitted by override: alice"),
        "{}",
        text(&shown)
    );
    // The spec's status is what the corpus says; the override ratified
    // nothing.
    assert_eq!(v["value"]["runs"][0]["id"], DRAFT);

    // Revocation restores the default refusal, and the earlier attempt keeps
    // the override it recorded.
    let out = f.cli(&["override", "revoke", &root, DRAFT, "alice", "done"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let out = f.cli(&["run", &root, DRAFT]);
    assert!(text(&out).contains("excluded"), "{}", text(&out));
    let listed = f.cli(&["run", "list", &root, "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(
        v["value"]["runs"][0]["attempts"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        v["value"]["runs"][0]["attempts"][0]["admission"]["by"],
        "override"
    );
}

#[test]
fn refused_overrides_write_nothing() {
    let f = Fixture::new();
    let root = f.repository("a");
    let journal = |f: &Fixture| {
        std::fs::read_dir(f.home().join("records"))
            .map(|d| {
                d.filter_map(Result::ok)
                    .filter(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .ends_with(".overrides.jsonl")
                    })
                    .count()
            })
            .unwrap_or(0)
    };
    for (args, why) in [
        (
            vec!["override", "grant", &root, DRAFT, " ", "why"],
            "operator",
        ),
        (vec!["override", "grant", &root, DRAFT, "alice"], "reason"),
        (
            vec!["override", "grant", &root, "404-absent", "alice", "why"],
            "not a spec",
        ),
        (
            vec!["override", "revoke", &root, DRAFT, "alice", "why"],
            "no override",
        ),
    ] {
        let out = f.cli(&args);
        assert_eq!(code(&out), 2, "{args:?}: {}", text(&out));
        assert!(text(&out).contains(why), "{args:?}: {}", text(&out));
    }
    assert_eq!(journal(&f), 0);

    // An unregistered repository is refused before anything is read.
    let unregistered = f.project("elsewhere").display().to_string();
    let out = f.cli(&["override", "grant", &unregistered, DRAFT, "alice", "why"]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("not registered"), "{}", text(&out));

    // A duplicate grant is refused, and the first stands.
    assert_eq!(
        code(&f.cli(&["override", "grant", &root, DRAFT, "alice", "why"])),
        0
    );
    let out = f.cli(&["override", "grant", &root, DRAFT, "bob", "again"]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("already in force"), "{}", text(&out));
}

#[test]
fn an_override_is_scoped_to_its_repository_and_to_the_ready_set() {
    let f = Fixture::new();
    let a = f.repository("a");
    let b = f.repository("b");
    assert_eq!(
        code(&f.cli(&["override", "grant", &a, DRAFT, "alice", "why"])),
        0
    );
    // Repository B has the same spec id and no override.
    let out = f.cli(&["run", &b, DRAFT]);
    assert!(text(&out).contains("excluded"), "{}", text(&out));
    // A spec the report does not offer as ready is not scheduled by an
    // override: the corpus names it, so the grant is recorded, and `run`
    // still finds it not offered.
    assert_eq!(
        code(&f.cli(&["override", "grant", &a, "010-unready", "alice", "why"])),
        0
    );
    let out = f.cli(&["run", &a, "010-unready"]);
    assert_ne!(code(&out), 0, "{}", text(&out));
    assert!(text(&out).contains("not in"), "{}", text(&out));
}

#[test]
fn an_edited_journal_refuses_work_and_run_rather_than_reading_as_no_override() {
    let f = Fixture::new();
    let root = f.repository("a");
    assert_eq!(
        code(&f.cli(&["override", "grant", &root, DRAFT, "alice", "why"])),
        0
    );
    // A second line, so the first has a successor whose link checks it. The
    // last line alone has none: section 3.1.4 rule 3 names that residual.
    assert_eq!(
        code(&f.cli(&["override", "grant", &root, "010-unready", "bob", "why"])),
        0
    );
    let path = statecraft_run::overrides::journal_path(&f.home(), Path::new(&root));
    let bytes = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, bytes.replacen("alice", "mallory", 1)).unwrap();
    for args in [
        vec!["work", "list", &root],
        vec!["run", &root, DRAFT],
        vec!["override", "show", &root],
    ] {
        let out = f.cli(&args);
        assert_eq!(code(&out), 4, "{args:?}: {}", text(&out));
        assert!(text(&out).contains("override journal"), "{}", text(&out));
    }
}

#[test]
fn an_intent_written_before_the_section_reads_as_not_recorded() {
    let f = Fixture::new();
    let root = f.repository("a");
    let (mut chain, _) = statecraft_run::record::Chain::open(&f.home(), Path::new(&root)).unwrap();
    statecraft_run::session::begin(
        &mut chain,
        Path::new(&root),
        "old",
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();
    let out = f.cli(&["run", "show", &root, "old", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["value"]["admissions"][0]["admission"].is_null(), "{v}");
    let out = f.cli(&["run", "show", &root, "old"]);
    assert!(
        text(&out).contains("admission not recorded"),
        "{}",
        text(&out)
    );
}

#[test]
fn while_another_process_holds_the_repository_lock_grant_and_run_refuse() {
    let f = Fixture::new();
    let root = f.repository("a");
    let held = statecraft_run::lock::try_acquire(&f.home(), Path::new(&root)).unwrap();
    let out = f.cli(&["override", "grant", &root, DRAFT, "alice", "why"]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("lock"), "{}", text(&out));
    drop(held);
    assert_eq!(
        code(&f.cli(&["override", "grant", &root, DRAFT, "alice", "why"])),
        0
    );
    let held = statecraft_run::lock::try_acquire(&f.home(), Path::new(&root)).unwrap();
    let out = f.cli(&["run", &root, DRAFT]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("lock"), "{}", text(&out));
    drop(held);
    let listed = f.cli(&["run", "list", &root]);
    assert!(
        text(&listed).contains("no runs recorded"),
        "{}",
        text(&listed)
    );
}

/// How many files in the home's records directory end with `suffix`.
fn records_ending(f: &Fixture, suffix: &str) -> usize {
    std::fs::read_dir(f.home().join("records"))
        .map(|d| {
            d.filter_map(Result::ok)
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.ends_with(suffix)
                        && (suffix != ".jsonl" || !name.ends_with(".overrides.jsonl"))
                })
                .count()
        })
        .unwrap_or(0)
}

/// Spec 003 section 3.1.4 rules 3, 4 and 7, section 5 (2026-09-23): the
/// register equates `<root>`, `<root>/` and `<root>/.`, so all three name one
/// repository, with one lock, one run record and one journal, filed under the
/// root as registered.
#[test]
fn every_spelling_of_a_registered_root_keys_one_lock_one_chain_and_one_journal() {
    let f = Fixture::new();
    let root = f.repository("a");
    let slash = format!("{root}/");
    let dot = format!("{root}/.");

    // Granted through one spelling, seen and refused as a duplicate through
    // the others.
    let out = f.cli(&["override", "grant", &slash, DRAFT, "alice", "why"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(statecraft_run::overrides::journal_path(&f.home(), Path::new(&root)).exists());
    for spelling in [&root, &dot] {
        let out = f.cli(&["override", "grant", spelling, DRAFT, "bob", "again"]);
        assert_eq!(code(&out), 2, "{spelling}: {}", text(&out));
        assert!(text(&out).contains("already in force"), "{}", text(&out));
    }
    for spelling in [&root, &slash, &dot] {
        let out = f.cli(&["work", "list", spelling]);
        assert_eq!(code(&out), 0, "{spelling}: {}", text(&out));
        assert!(
            text(&out).contains("override"),
            "{spelling}: {}",
            text(&out)
        );
    }

    // Run through a third spelling; every spelling reads the one run.
    let out = f.cli(&["run", &dot, DRAFT]);
    assert!(code(&out) <= 1, "{}", text(&out));
    assert!(statecraft_run::record::chain_path(&f.home(), Path::new(&root)).exists());
    for spelling in [&root, &slash, &dot] {
        let listed = f.cli(&["run", "list", spelling, "--json"]);
        let v: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
        assert_eq!(v["value"]["runs"][0]["id"], DRAFT, "{spelling}: {v}");
        assert_eq!(
            v["value"]["runs"][0]["attempts"].as_array().unwrap().len(),
            1,
            "{spelling}: {v}"
        );
    }
    let out = f.cli(&["run", &slash, DRAFT]);
    assert!(code(&out) <= 1, "{}", text(&out));
    let listed = f.cli(&["run", "list", &root, "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(
        v["value"]["runs"][0]["attempts"].as_array().unwrap().len(),
        2,
        "a retry through another spelling appends to the same run: {v}"
    );

    assert_eq!(records_ending(&f, ".jsonl"), 1, "one run record");
    assert_eq!(records_ending(&f, ".overrides.jsonl"), 1, "one journal");
    assert_eq!(records_ending(&f, ".lock"), 1, "one lock");

    // A symbolic link to the root is not a registered path: refused, and
    // nothing is keyed by it.
    let link = f.project("link");
    std::os::unix::fs::symlink(&root, &link).unwrap();
    let link = link.display().to_string();
    for args in [
        vec!["run", &link, DRAFT],
        vec!["override", "grant", &link, "010-unready", "alice", "why"],
        vec!["work", "list", &link],
    ] {
        let out = f.cli(&args);
        assert_eq!(code(&out), 2, "{args:?}: {}", text(&out));
        assert!(text(&out).contains("not registered"), "{}", text(&out));
    }
    assert_eq!(records_ending(&f, ".jsonl"), 1);
    assert_eq!(records_ending(&f, ".overrides.jsonl"), 1);
    assert_eq!(records_ending(&f, ".lock"), 1);

    // Registered as well, the link is a second root for one directory: both
    // are refused rather than given a second lock.
    assert!(code(&f.cli(&["project", "register", &link])) <= 1);
    for spelling in [&root, &link] {
        let out = f.cli(&["run", spelling, DRAFT]);
        assert_eq!(code(&out), 2, "{spelling}: {}", text(&out));
        assert!(
            text(&out).contains("registered under more than one path"),
            "{}",
            text(&out)
        );
    }
    assert_eq!(records_ending(&f, ".lock"), 1);
}

/// The reproduced race: while a run holds the repository, a second `run`
/// through another spelling is refused, never started in a second chain.
#[test]
fn a_second_run_through_another_spelling_while_one_is_live_is_refused() {
    let f = Fixture::new();
    let root = f.repository("a");
    assert_eq!(
        code(&f.cli(&["override", "grant", &root, DRAFT, "alice", "why"])),
        0
    );
    let spellings = [format!("{root}/"), format!("{root}/.")];

    // The supervising process's lock, as `run` takes it.
    let held = statecraft_run::lock::try_acquire(&f.home(), Path::new(&root)).unwrap();
    for spelling in &spellings {
        let out = f.cli(&["run", spelling, DRAFT]);
        assert_eq!(code(&out), 2, "{spelling}: {}", text(&out));
        assert!(text(&out).contains("lock"), "{}", text(&out));
        let out = f.cli(&["override", "revoke", spelling, DRAFT, "alice", "why"]);
        assert_eq!(code(&out), 2, "{spelling}: {}", text(&out));
        assert!(text(&out).contains("lock"), "{}", text(&out));
    }
    drop(held);

    // An attempt with an intent and no outcome is live in the one chain, and
    // a run through another spelling sees it.
    let (mut chain, _) = statecraft_run::record::Chain::open(&f.home(), Path::new(&root)).unwrap();
    statecraft_run::session::begin(
        &mut chain,
        Path::new(&root),
        DRAFT,
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();
    drop(chain);
    for spelling in &spellings {
        let out = f.cli(&["run", spelling, DRAFT]);
        assert_eq!(code(&out), 2, "{spelling}: {}", text(&out));
        assert!(text(&out).contains("live"), "{}", text(&out));
    }
    assert_eq!(records_ending(&f, ".jsonl"), 1, "no second chain");
    let listed = f.cli(&["run", "list", &format!("{root}/"), "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(
        v["value"]["runs"][0]["attempts"].as_array().unwrap().len(),
        1,
        "{v}"
    );
}

/// Section 5 (2026-09-23): a home written before the key was the stored root
/// may hold records filed under the spelling that was typed. They are never
/// read as "no history" and never merged: every verb that would read them
/// fails, naming the file.
#[test]
fn records_filed_under_another_spelling_are_neither_orphaned_nor_merged() {
    let f = Fixture::new();
    let root = f.repository("a");
    let typed = format!("{root}/");
    assert_eq!(
        code(&f.cli(&["override", "grant", &root, DRAFT, "alice", "why"])),
        0
    );
    // What the previous build wrote for `run <root>/`: a chain keyed by the
    // typed spelling.
    let (mut chain, _) = statecraft_run::record::Chain::open(&f.home(), Path::new(&typed)).unwrap();
    statecraft_run::session::begin(
        &mut chain,
        Path::new(&typed),
        "old",
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();
    drop(chain);
    let stray = statecraft_run::record::chain_path(&f.home(), Path::new(&typed));
    let before = std::fs::read(&stray).unwrap();
    for spelling in [&root, &typed] {
        for args in [
            vec!["run", "list", spelling],
            vec!["run", "show", spelling, "old"],
            vec!["run", spelling, DRAFT],
        ] {
            let out = f.cli(&args);
            assert_eq!(code(&out), 4, "{args:?}: {}", text(&out));
            assert!(
                text(&out).contains("another spelling"),
                "{args:?}: {}",
                text(&out)
            );
        }
    }
    assert_eq!(std::fs::read(&stray).unwrap(), before, "never rewritten");
    assert!(!statecraft_run::record::chain_path(&f.home(), Path::new(&root)).exists());
}
