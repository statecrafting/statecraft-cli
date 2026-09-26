//! Per-path replacement of a drifted managed file, through the built binary.
//!
//! Spec 002 section 3.4: "An upgrade never resolves a conflict by choosing.
//! Replacing a drifted managed file requires the operator to say so per path."
//! The operator names a path to `env plan --replace <file>`, which reports a
//! plan identity binding the drift it saw and the replacement, and consents with
//! `env apply --replace <file>=<plan-id>` (or `env upgrade`). Exit codes are spec
//! 006 section 3.3's.
//!
//! The configured adapter claims its paths only when its three prerequisites
//! hold, and one of them, the credential path, is measured on macOS only (spec
//! 004). So the tests that need a claiming adapter are compiled on macOS, with a
//! synthetic `claude` and a synthetic qualification record in a temporary home;
//! the library half of every case is `statecraft-environment`'s
//! `tests/replace_per_path.rs`, which runs everywhere. The refusal for an
//! adapter that does not claim runs everywhere here too.
//!
//! Every run clears the environment and gets a temporary `STATECRAFT_HOME`,
//! `STATECRAFT_NATIVE_ROOT` and `HOME`.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[allow(dead_code)]
const INSTRUCTIONS: &str = ".claude/statecraft/instructions.md";
#[allow(dead_code)]
const POINTER: &str = "CLAUDE.md";

struct Sandbox {
    dir: tempfile::TempDir,
    path: String,
}

impl Sandbox {
    /// A registered git repository. `qualified` installs a synthetic `claude`
    /// and a matching synthetic qualification record, so the adapter claims.
    fn new(qualified: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(dir.path().join("home")).unwrap();
        for args in [
            vec!["init", "--quiet"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "base"],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(&project)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(project.join("README.md"), b"the user's readme").unwrap();
        if qualified {
            statecraft_adapter::fixture::install_script(
                &bin.join("claude"),
                "#!/bin/sh\n[ \"$1\" = --version ] && { echo '2.1.267 (Claude Code)'; exit 0; }\nexit 3\n",
                0o755,
            )
            .unwrap();
            let record = statecraft_adapter_claude_code::qualification::record(
                "2.1.267",
                "synthetic-fixture-only",
                "2026-09-17T00:00:00Z",
            );
            std::fs::write(
                dir.path().join("home").join("qualifications.json"),
                serde_json::to_vec(&vec![record]).unwrap(),
            )
            .unwrap();
        }
        let path = format!("{}:/usr/bin:/bin", bin.display());
        let sandbox = Self { dir, path };
        let registered = sandbox.run(&["project", "register", &sandbox.root()]);
        assert!(code(&registered) <= 1, "{registered:?}");
        sandbox
    }

    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }

    fn root(&self) -> String {
        self.project().to_str().unwrap().to_string()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("PATH", &self.path)
            .env("USER", "fixture-operator")
            .env("STATECRAFT_HOME", self.dir.path().join("home"))
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .output()
            .expect("the binary runs")
    }

    #[allow(dead_code)]
    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.project().join(rel)).unwrap()
    }

    #[allow(dead_code)]
    fn write(&self, rel: &str, bytes: &[u8]) {
        std::fs::write(self.project().join(rel), bytes).unwrap();
    }

    #[allow(dead_code)]
    fn plan(&self, named: &[&str]) -> Output {
        let root = self.root();
        let mut args = vec!["env", "plan", root.as_str(), "--json"];
        for n in named {
            args.push("--replace");
            args.push(n);
        }
        self.run(&args)
    }

    #[allow(dead_code)]
    fn apply(&self, verb: &str, consents: &[(&str, &str)]) -> Output {
        let root = self.root();
        let joined: Vec<String> = consents.iter().map(|(p, id)| format!("{p}={id}")).collect();
        let mut args = vec!["env", verb, root.as_str(), "--json"];
        for c in &joined {
            args.push("--replace");
            args.push(c);
        }
        self.run(&args)
    }

    #[allow(dead_code)]
    fn plan_id(&self, path: &str) -> String {
        let out = self.plan(&[path]);
        let value = json(&out);
        json_naming::payload(&value)["named"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["path"] == path && n["state"] == "replace")
            .and_then(|n| n["planId"].as_str())
            .unwrap_or_else(|| panic!("{path} is replaceable: {value}"))
            .to_string()
    }

    #[allow(dead_code)]
    fn snapshot(&self) -> std::collections::BTreeMap<String, Vec<u8>> {
        snapshot(&self.project())
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

fn json(o: &Output) -> serde_json::Value {
    json_naming::from_output(&o.stdout).unwrap_or_else(|e| panic!("{e}: {o:?}"))
}

fn snapshot(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap().display().to_string();
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

#[test]
fn a_replacement_for_an_adapter_that_does_not_claim_is_refused_and_writes_nothing() {
    let sandbox = Sandbox::new(false);
    let before = sandbox.snapshot();

    let planned = sandbox.plan(&[".claude/statecraft/instructions.md"]);
    assert_eq!(code(&planned), 2, "{planned:?}");
    assert!(
        json_naming::payload(&json(&planned))["refusals"][0]
            .as_str()
            .unwrap()
            .contains("does not claim its paths"),
        "{planned:?}"
    );
    let applied = sandbox.apply(
        "apply",
        &[(".claude/statecraft/instructions.md", &"0".repeat(64))],
    );
    assert_eq!(code(&applied), 2, "{applied:?}");
    assert_eq!(sandbox.snapshot(), before);
}

#[test]
fn a_consent_without_a_plan_identity_is_a_usage_error() {
    let sandbox = Sandbox::new(false);
    let root = sandbox.root();
    let id = "a".repeat(64);
    // A consent given to `env plan`, a consent whose identity is too short, and
    // a consent with no path before the identity.
    let consent_like = format!("CLAUDE.md={id}");
    let no_path = format!("={id}");
    let consent_like_short = "CLAUDE.md=abcd".to_string();
    for args in [
        vec!["env", "apply", root.as_str(), "--replace", "CLAUDE.md"],
        vec![
            "env",
            "upgrade",
            root.as_str(),
            "--replace",
            "CLAUDE.md=abc",
        ],
        vec!["env", "apply", root.as_str(), "--replace"],
        vec!["doctor", root.as_str(), "--replace", "CLAUDE.md"],
        vec!["env", "apply", root.as_str(), "CLAUDE.md"],
        vec!["env", "plan", root.as_str(), "--replace", "--json"],
        vec!["env", "plan", root.as_str(), "--replac", "CLAUDE.md"],
        vec![
            "env",
            "apply",
            root.as_str(),
            "--replace",
            &consent_like_short,
        ],
        vec!["env", "plan", root.as_str(), "--replace", &consent_like],
        vec!["env", "apply", root.as_str(), "--replace", &no_path],
    ] {
        let out = sandbox.run(&args);
        assert_eq!(code(&out), 3, "{args:?}: {out:?}");
    }
}

#[cfg(target_os = "macos")]
mod claiming {
    use super::*;

    /// Installed, then both managed files edited by the operator.
    fn drifted() -> Sandbox {
        let sandbox = Sandbox::new(true);
        let first = sandbox.run(&["env", "apply", &sandbox.root(), "--json"]);
        assert_eq!(code(&first), 0, "the adapter claims and writes: {first:?}");
        sandbox.write(INSTRUCTIONS, b"the operator edited the instructions");
        sandbox.write(POINTER, b"the operator edited the pointer");
        sandbox
    }

    fn declared(path: &str) -> Vec<u8> {
        statecraft_adapter_claude_code::environment::declaration()
            .files
            .into_iter()
            .find(|f| f.path == path)
            .unwrap()
            .contents
    }

    fn recorded_digest(sandbox: &Sandbox, path: &str) -> String {
        statecraft_environment::manifest::Manifest::read(&sandbox.project())
            .unwrap()
            .unwrap()
            .entry(path)
            .unwrap()
            .digest
            .clone()
    }

    #[test]
    fn without_a_named_path_an_upgrade_withholds_every_drifted_file() {
        let sandbox = drifted();
        let before = sandbox.snapshot();

        let out = sandbox.run(&["env", "upgrade", &sandbox.root(), "--json"]);

        assert_eq!(code(&out), 1, "{out:?}");
        assert_eq!(
            json_naming::payload(&json(&out))["withheld"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(sandbox.snapshot(), before, "nothing replaced implicitly");
    }

    #[test]
    fn the_plan_names_the_drift_and_the_replacement_and_writes_nothing() {
        let sandbox = drifted();
        let before = sandbox.snapshot();

        let out = sandbox.plan(&[INSTRUCTIONS]);

        let answer = json(&out);
        let named = &json_naming::payload(&answer)["named"][0];
        assert_eq!(named["state"], "replace", "{out:?}");
        assert_eq!(
            named["found"],
            statecraft_environment::digest::digest_bytes(b"the operator edited the instructions")
        );
        assert_eq!(
            named["replacement"],
            statecraft_environment::digest::digest_bytes(&declared(INSTRUCTIONS))
        );
        assert_eq!(named["planId"].as_str().unwrap().len(), 64);
        assert_eq!(code(&out), 1, "the unnamed drift is still a finding");
        assert_eq!(sandbox.snapshot(), before);
    }

    #[test]
    fn apply_replaces_only_the_named_path_and_keeps_every_other_byte() {
        let sandbox = drifted();
        let id = sandbox.plan_id(INSTRUCTIONS);

        let out = sandbox.apply("apply", &[(INSTRUCTIONS, &id)]);

        assert_eq!(
            code(&out),
            1,
            "the unnamed pointer drift is withheld: {out:?}"
        );
        assert_eq!(sandbox.read(INSTRUCTIONS), declared(INSTRUCTIONS));
        assert_eq!(sandbox.read(POINTER), b"the operator edited the pointer");
        assert_eq!(sandbox.read("README.md"), b"the user's readme");
        assert_eq!(
            recorded_digest(&sandbox, INSTRUCTIONS),
            statecraft_environment::digest::digest_bytes(&declared(INSTRUCTIONS))
        );
        assert_eq!(
            json_naming::payload(&json(&out))["named"][0]["state"],
            "replace"
        );
    }

    #[test]
    fn upgrade_takes_the_same_consent() {
        let sandbox = drifted();
        let a = sandbox.plan_id(INSTRUCTIONS);
        let b = sandbox.plan_id(POINTER);

        let out = sandbox.apply("upgrade", &[(INSTRUCTIONS, &a), (POINTER, &b)]);

        assert_eq!(code(&out), 0, "{out:?}");
        assert_eq!(sandbox.read(INSTRUCTIONS), declared(INSTRUCTIONS));
        assert_eq!(sandbox.read(POINTER), declared(POINTER));
    }

    #[test]
    fn a_stale_plan_is_refused_and_writes_nothing() {
        let sandbox = drifted();
        let id = sandbox.plan_id(INSTRUCTIONS);
        sandbox.write(INSTRUCTIONS, b"edited again after planning");
        let before = sandbox.snapshot();

        let out = sandbox.apply("apply", &[(INSTRUCTIONS, &id)]);

        assert_eq!(code(&out), 2, "{out:?}");
        assert!(
            json_naming::payload(&json(&out))["reasons"][0]
                .as_str()
                .unwrap()
                .contains("stale plan"),
            "{out:?}"
        );
        assert_eq!(
            sandbox.snapshot(),
            before,
            "not one byte, manifest included"
        );
    }

    #[test]
    fn an_identity_for_another_drift_is_refused() {
        let sandbox = drifted();
        let other = sandbox.plan_id(POINTER);
        let before = sandbox.snapshot();

        let out = sandbox.apply("apply", &[(INSTRUCTIONS, &other)]);

        assert_eq!(code(&out), 2, "{out:?}");
        assert_eq!(sandbox.snapshot(), before);
    }

    #[test]
    fn user_adopted_occupied_and_protected_paths_are_never_replaced() {
        let sandbox = Sandbox::new(true);
        // The pointer path holds the user's own file before install, so it is
        // occupied and withheld; it never becomes this product's.
        sandbox.write(POINTER, b"the user's CLAUDE.md");
        let first = sandbox.run(&["env", "apply", &sandbox.root()]);
        assert_eq!(code(&first), 1, "{first:?}");
        sandbox.write(INSTRUCTIONS, b"edited");
        // Mark the instructions adopted in the committed manifest.
        let mut manifest = statecraft_environment::manifest::Manifest::read(&sandbox.project())
            .unwrap()
            .unwrap();
        let mut entry = manifest.entry(INSTRUCTIONS).unwrap().clone();
        entry.class = statecraft_environment::manifest::Class::Adopted;
        manifest.upsert(entry);
        manifest.write(&sandbox.project()).unwrap();
        let before = sandbox.snapshot();

        for path in [
            POINTER,
            INSTRUCTIONS,
            "README.md",
            "AGENTS.md",
            ".statecraft/environment.json",
            "../outside.md",
        ] {
            let planned = sandbox.plan(&[path]);
            assert_eq!(code(&planned), 2, "{path}: {planned:?}");
            let applied = sandbox.apply("apply", &[(path, &"0".repeat(64))]);
            assert_eq!(code(&applied), 2, "{path}: {applied:?}");
        }
        assert_eq!(sandbox.snapshot(), before);
    }

    #[test]
    fn repeating_a_successful_replacement_reports_already_satisfied() {
        let sandbox = drifted();
        let a = sandbox.plan_id(INSTRUCTIONS);
        let b = sandbox.plan_id(POINTER);
        assert_eq!(
            code(&sandbox.apply("apply", &[(INSTRUCTIONS, &a), (POINTER, &b)])),
            0
        );
        let files = sandbox.read(INSTRUCTIONS);

        let again = sandbox.apply("apply", &[(INSTRUCTIONS, &a), (POINTER, &b)]);

        assert_eq!(code(&again), 0, "{again:?}");
        let named = json_naming::payload(&json(&again))["named"].clone();
        assert_eq!(named[0]["state"], "already-satisfied", "{again:?}");
        assert_eq!(named[1]["state"], "already-satisfied", "{again:?}");
        assert_eq!(sandbox.read(INSTRUCTIONS), files);
    }

    #[test]
    fn an_interrupted_replacement_leaves_the_old_bytes_and_the_same_request_then_succeeds() {
        let sandbox = drifted();
        let id = sandbox.plan_id(INSTRUCTIONS);
        let manifest_before = sandbox.read(".statecraft/environment.json");
        // Staging cannot happen: its directory is a file.
        std::fs::create_dir_all(sandbox.project().join(".statecraft/state")).unwrap();
        sandbox.write(".statecraft/state/replace", b"in the way");

        let failed = sandbox.apply("apply", &[(INSTRUCTIONS, &id)]);

        assert_eq!(code(&failed), 4, "an i/o failure is a failure: {failed:?}");
        assert_eq!(
            sandbox.read(INSTRUCTIONS),
            b"the operator edited the instructions"
        );
        assert_eq!(
            sandbox.read(".statecraft/environment.json"),
            manifest_before
        );

        std::fs::remove_file(sandbox.project().join(".statecraft/state/replace")).unwrap();
        let retried = sandbox.apply("apply", &[(INSTRUCTIONS, &id)]);
        assert_eq!(code(&retried), 1, "{retried:?}");
        assert_eq!(sandbox.read(INSTRUCTIONS), declared(INSTRUCTIONS));
    }

    #[test]
    fn a_replacement_that_landed_before_the_manifest_is_recovered_by_repeating_it() {
        let sandbox = drifted();
        let id = sandbox.plan_id(INSTRUCTIONS);
        // The state a crash between the rename and the manifest write leaves.
        sandbox.write(INSTRUCTIONS, &declared(INSTRUCTIONS));

        let out = sandbox.apply("apply", &[(INSTRUCTIONS, &id)]);

        assert!(code(&out) <= 1, "{out:?}");
        assert_eq!(
            json_naming::payload(&json(&out))["named"][0]["state"],
            "already-satisfied"
        );
        assert_eq!(
            recorded_digest(&sandbox, INSTRUCTIONS),
            statecraft_environment::digest::digest_bytes(&declared(INSTRUCTIONS))
        );
    }

    #[test]
    fn a_managed_path_replaced_by_a_symbolic_link_is_refused() {
        let sandbox = drifted();
        sandbox.write("elsewhere.md", b"linked");
        std::fs::remove_file(sandbox.project().join(INSTRUCTIONS)).unwrap();
        std::os::unix::fs::symlink(
            sandbox.project().join("elsewhere.md"),
            sandbox.project().join(INSTRUCTIONS),
        )
        .unwrap();

        let out = sandbox.plan(&[INSTRUCTIONS]);

        assert_eq!(code(&out), 2, "{out:?}");
        assert_eq!(sandbox.read("elsewhere.md"), b"linked");
    }

    #[test]
    fn a_replacement_sweeps_leftovers_and_keeps_the_files_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let sandbox = drifted();
        std::fs::set_permissions(
            sandbox.project().join(INSTRUCTIONS),
            std::fs::Permissions::from_mode(0o640),
        )
        .unwrap();
        let id = sandbox.plan_id(INSTRUCTIONS);
        std::fs::create_dir_all(sandbox.project().join(".statecraft/state/replace")).unwrap();
        sandbox.write(".statecraft/state/replace/12345-0.tmp", b"left behind");

        let out = sandbox.apply("apply", &[(INSTRUCTIONS, &id)]);

        assert_eq!(code(&out), 1, "{out:?}");
        assert_eq!(
            json_naming::payload(&json(&out))["swept"],
            serde_json::json!([".statecraft/state/replace/12345-0.tmp"])
        );
        assert!(
            !sandbox
                .project()
                .join(".statecraft/state/replace/12345-0.tmp")
                .exists()
        );
        let mode = std::fs::metadata(sandbox.project().join(INSTRUCTIONS))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o640);
        assert_eq!(sandbox.read(INSTRUCTIONS), declared(INSTRUCTIONS));
    }
}
