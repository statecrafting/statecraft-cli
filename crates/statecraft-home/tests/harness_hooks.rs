//! Spec 002 section 3.23's hook contracts, enforced against the hook this
//! build actually ships.
//!
//! Section 3.23 says whoever owns the files owns the assertions. This product
//! owns `hooks/statecraft-gate.sh`, so the assertions live here. Each test
//! extracts the shipped body, writes it out as a program, and runs it against
//! a stub `spec-spine`: a contract asserted by reading the source for a phrase
//! would pass on a script that happens to mention the phrase in a comment.
//!
//! The stubs are fixtures and are labelled as one. What matters is which
//! binary the script chose and what it did with the code it got back, and a
//! stub can answer both without a real corpus.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The shipped hook body, read from the harness rather than duplicated here.
fn hook_body() -> String {
    statecraft_home::harness::shipped()
        .into_iter()
        .find(|f| f.rel_path == "hooks/statecraft-gate.sh")
        .expect("the harness ships a gate hook")
        .contents
}

/// The real `git`, resolved once from the environment the test inherited.
///
/// The fixture's PATH deliberately holds nothing else, so the shim it writes
/// has to name this absolutely.
fn real_git() -> PathBuf {
    let out = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("a shell");
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(!path.is_empty(), "these tests need git on PATH");
    PathBuf::from(path)
}

fn executable(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// A git repository that is a Statecraft project, plus the hook written out.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    hook: PathBuf,
    /// A directory placed on PATH ahead of everything else.
    path_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join(".statecraft")).unwrap();
        std::fs::write(root.join(".statecraft/environment.json"), "{}").unwrap();
        let out = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&root)
            .output()
            .expect("git is available");
        assert!(out.status.success(), "git init failed");

        let hook = dir.path().join("statecraft-gate.sh");
        executable(&hook, &hook_body());

        // PATH holds exactly this directory, so nothing the developer happens
        // to have installed can answer for a stub. `git` still has to work, so
        // it arrives as a shim that execs the real one by absolute path.
        let path_dir = dir.path().join("path");
        std::fs::create_dir_all(&path_dir).unwrap();
        executable(
            &path_dir.join("git"),
            &format!("#!/bin/sh\nexec {} \"$@\"\n", real_git().display()),
        );

        Self {
            _dir: dir,
            root,
            hook,
            path_dir,
        }
    }

    /// Place a stub `spec-spine` somewhere, recording which one ran.
    ///
    /// The stub appends its own label to `witness`, so a test can ask which
    /// binary the resolution order actually chose rather than inferring it.
    fn stub(&self, at: &Path, label: &str, check_code: i32, carries_verbs: bool) {
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let witness = self.root.join("witness");
        let body = if carries_verbs {
            format!(
                "#!/bin/sh\nprintf '{label}\\n' >> '{}'\n\
                 case \"$1 $2\" in\n  'check --help'|'lint --help') exit 0 ;;\nesac\n\
                 [ \"$1\" = check ] && exit {check_code}\n\
                 exit 0\n",
                witness.display()
            )
        } else {
            // A binary older than the verb: clap spends 2 on an unknown
            // subcommand, which is also the stale code.
            format!(
                "#!/bin/sh\nprintf '{label}\\n' >> '{}'\nexit 2\n",
                witness.display()
            )
        };
        executable(at, &body);
    }

    fn witness(&self) -> String {
        std::fs::read_to_string(self.root.join("witness")).unwrap_or_default()
    }

    /// Assert which stub ran, and that one actually did.
    ///
    /// An empty witness satisfies "every line is X" vacuously, which is how a
    /// first draft of these tests passed while the hook ran nothing at all.
    fn assert_only_ran(&self, label: &str) {
        let w = self.witness();
        assert!(!w.trim().is_empty(), "no stub ran at all");
        assert!(
            w.lines().all(|l| l == label),
            "expected only {label} to run, got {w:?}"
        );
    }

    /// Run the hook against this repository, naming it on the command line.
    fn run(&self, env: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(&self.hook);
        cmd.arg(&self.root)
            // A deliberately unrelated working directory: contract 3 says the
            // target comes from the command, so the session's own cwd must not
            // be what decides which tree is governed.
            .current_dir(std::env::temp_dir())
            .env("PATH", &self.path_dir)
            .env_remove("SPEC_SPINE_BIN");
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Contract 1: read, never repair.
///
/// Asserted over the body rather than by running it, because the failure this
/// contract prevents is a writing verb *reachable* on some branch, not one on
/// the branch a test happened to take.
#[test]
fn contract_1_the_hook_invokes_no_writing_subcommand() {
    let body = hook_body();
    // A shell script gives a scanner no clean way to tell an executed word
    // from one inside a message, and a scan that cannot tell them apart gets
    // reworded around rather than fixed. So this asserts over invocations of
    // the binary specifically, which is what the contract is actually about.
    let code: Vec<&str> = body
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect();

    // An invocation is the binary in command position. `[ -x "$bin" ]` names
    // it without running it, and a filter that cannot tell those apart is the
    // same blunt scan in a different shape.
    let verb_of = |line: &str| -> Option<String> {
        let mut l = line.trim();
        l = l.strip_prefix("if ").unwrap_or(l);
        l = l.strip_prefix("! ").unwrap_or(l);
        let rest = l.strip_prefix("\"$bin\" ")?;
        rest.split_whitespace().next().map(str::to_string)
    };
    let verbs: Vec<String> = code.iter().filter_map(|l| verb_of(l)).collect();
    assert!(!verbs.is_empty(), "the hook invokes nothing at all");
    for verb in &verbs {
        assert!(
            matches!(verb.as_str(), "check" | "lint" | "\"$verb\""),
            "the gate hook invokes the binary with something other than a read: {verb}"
        );
    }

    // The probe loop is the one invocation whose verb is a variable, so the
    // values it takes are part of the same assertion.
    assert!(
        code.iter()
            .any(|l| l.trim() == "for verb in check lint; do"),
        "the probe loop no longer enumerates exactly the two reads"
    );

    // And nothing reaches a second binary by bare name, which would be both a
    // writing risk and contract 2's failure.
    for line in &code {
        let mut l = line.trim();
        l = l.strip_prefix("if ").unwrap_or(l);
        l = l.strip_prefix("! ").unwrap_or(l);
        assert!(
            !l.starts_with("spec-spine"),
            "the gate hook runs spec-spine by bare name, outside the resolution order: {line}"
        );
    }
}

/// Contract 2, first position: `$SPEC_SPINE_BIN` wins over everything.
#[test]
fn contract_2_spec_spine_bin_is_preferred() {
    let f = Fixture::new();
    f.stub(&f.path_dir.join("spec-spine"), "path", 0, true);
    f.stub(&f.root.join("target/release/spec-spine"), "local", 0, true);
    let explicit = f.root.join("explicit-spec-spine");
    f.stub(&explicit, "explicit", 0, true);

    let out = f.run(&[("SPEC_SPINE_BIN", explicit.to_str().unwrap())]);
    assert!(out.status.success(), "{}", stderr(&out));
    f.assert_only_ran("explicit");
}

/// Contract 2, second position: the target repository's own build beats PATH.
///
/// This is the half the shipped hook used to get wrong, and it is the one that
/// bites: a bare name is whichever copy the last unrelated project installed.
#[test]
fn contract_2_the_repositorys_own_build_beats_path() {
    let f = Fixture::new();
    f.stub(&f.path_dir.join("spec-spine"), "path", 0, true);
    f.stub(&f.root.join("target/release/spec-spine"), "local", 0, true);

    let out = f.run(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    f.assert_only_ran("local");
}

/// Contract 2, third position: PATH still keeps an adopter working.
#[test]
fn contract_2_path_is_the_fallback_and_still_works() {
    let f = Fixture::new();
    f.stub(&f.path_dir.join("spec-spine"), "path", 0, true);

    let out = f.run(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    f.assert_only_ran("path");
}

/// Contract 3: the target comes from the command, not from the session.
#[test]
fn contract_3_the_target_repository_comes_from_the_command() {
    let f = Fixture::new();
    f.stub(&f.root.join("target/release/spec-spine"), "local", 0, true);

    // `run` deliberately sets cwd to an unrelated directory and names the
    // repository as the argument. If the script resolved from the session it
    // would find no Statecraft project and exit 0 having run nothing.
    let out = f.run(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        !f.witness().is_empty(),
        "the hook resolved from the session and governed nothing"
    );
}

/// Contract 4: each of `check`'s four answers is read as itself.
#[test]
fn contract_4_each_verdict_is_read_as_itself() {
    // (answer from check, exit code the gate must produce, phrase it must name)
    let cases = [
        (0, 0, ""),
        (1, 1, "does not validate"),
        (2, 2, "stale"),
        (3, 3, "was not performed"),
    ];
    for (answer, expected, phrase) in cases {
        let f = Fixture::new();
        f.stub(&f.path_dir.join("spec-spine"), "path", answer, true);
        let out = f.run(&[]);
        assert_eq!(
            out.status.code(),
            Some(expected),
            "check answered {answer}: {}",
            stderr(&out)
        );
        if !phrase.is_empty() {
            assert!(
                stderr(&out).contains(phrase),
                "check answered {answer} and the gate did not name it: {}",
                stderr(&out)
            );
        }
    }
}

/// Contract 5: a binary older than the verb is not a stale tree.
///
/// The stub answers 2 to everything, which is what `clap` spends on an unknown
/// subcommand and also what a stale tree answers. Without the `--help` probe
/// the two are indistinguishable, and the session is sent to regenerate shards
/// that were already correct.
#[test]
fn contract_5_a_missing_verb_is_not_reported_as_stale() {
    let f = Fixture::new();
    f.stub(&f.path_dir.join("spec-spine"), "path", 2, false);

    let out = f.run(&[]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("does not carry the verb"),
        "a missing verb was not named: {err}"
    );
    assert!(
        !err.contains("stale"),
        "a missing verb was reported as a stale tree: {err}"
    );
}

/// Contract 6, and its boundary: a gate that did not run refuses, but a script
/// that is not a gate here at all is inert.
#[test]
fn contract_6_a_check_that_did_not_run_is_not_green() {
    let f = Fixture::new();
    // No stub anywhere: nothing on PATH, no local build, no SPEC_SPINE_BIN.
    let out = f.run(&[]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("no spec-spine binary"),
        "{}",
        stderr(&out)
    );
}

/// Section 3.14 rule 3: inert outside a Statecraft project, which is what makes
/// it safe on a path shared with other work. Not the same as contract 6.
#[test]
fn outside_a_statecraft_project_the_hook_is_inert() {
    let f = Fixture::new();
    f.stub(&f.path_dir.join("spec-spine"), "path", 1, true);
    std::fs::remove_file(f.root.join(".statecraft/environment.json")).unwrap();

    let out = f.run(&[]);
    assert!(
        out.status.success(),
        "a non-Statecraft repository was refused: {}",
        stderr(&out)
    );
    assert!(
        f.witness().is_empty(),
        "the hook ran a verb outside a Statecraft project"
    );
}

/// Section 3.23's read-only skill assertion: the shipped skill invokes no
/// writing verb.
#[test]
fn the_shipped_skill_is_read_only() {
    let skill = statecraft_home::harness::shipped()
        .into_iter()
        .find(|f| f.rel_path.starts_with("skills/"))
        .expect("the harness ships a skill");
    for writing in [
        "env apply",
        "home apply",
        "project arm",
        "run start",
        "init",
    ] {
        assert!(
            !skill.contents.contains(writing),
            "{} invokes a writing verb ({writing})",
            skill.rel_path
        );
    }
}
