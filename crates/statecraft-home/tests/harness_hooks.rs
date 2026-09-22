//! Spec 002 section 3.23's hook contracts, and the Stop policy the owner
//! adopted on 2026-09-21, enforced against the hooks this build actually
//! ships.
//!
//! Section 3.23 says whoever owns the files owns the assertions. The
//! counterparty enforced these in its own tree, in a file that could not move
//! because a hermetic test may not read `$HOME`. This product now owns the
//! four adopted event behaviors, so the assertions live here.
//!
//! **Every test runs a hook as a program.** Each one writes the shipped body
//! out, gives it the input its event actually delivers, and runs it against a
//! stub `spec-spine`. A contract asserted by searching the source for a phrase
//! would pass on a script that merely mentions the phrase in a comment, which
//! is the failure mode the owner named.
//!
//! The four events take their input differently and the fixture reflects that
//! rather than flattening it: `SessionStart` and `Stop` read
//! `$CLAUDE_PROJECT_DIR`, and `PostToolUse` and `PreToolUse` read a JSON
//! payload on stdin. A harness that fed all four the same way would be testing
//! something no session does.
//!
//! | Contract | Hooks it binds | Tests |
//! |---|---|---|
//! | 1 read, never repair | all four | `contract_1_*` |
//! | 2 binary resolution order | session-start, stop, post-edit | `contract_2_*` |
//! | 3 target from the command | post-edit, pre-bash | `contract_3_*` |
//! | 4 read the verdict, never guess it | session-start, stop | `contract_4_*` |
//! | 5 establish the verb first | session-start, stop | `contract_5_*` |
//! | 6 a gate whose check did not run is not green | pre-bash | `contract_6_*` |
//! | 7 a branch gate resolves the protected branch | pre-bash | `contract_7_*` |
//! | the Stop policy: advisory, and accurate | stop | `stop_policy_*` |
//! | section 3.14 rule 3, the project gate | all four | `outside_a_statecraft_project_*` |

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// One shipped hook body, read from the harness rather than duplicated here.
fn hook_body(file: &str) -> String {
    statecraft_home::harness::shipped()
        .into_iter()
        .find(|f| f.rel_path == format!("hooks/{file}"))
        .unwrap_or_else(|| panic!("the harness ships hooks/{file}"))
        .contents
}

const SESSION_START: &str = "statecraft-session-start.sh";
const POST_EDIT: &str = "statecraft-post-edit.sh";
const PRE_BASH: &str = "statecraft-pre-bash.sh";
const STOP: &str = "statecraft-stop.sh";

/// Every hook the build ships, so a contract that binds all four says so by
/// iterating rather than by naming three of them and forgetting the fourth.
const ALL: [&str; 4] = [SESSION_START, POST_EDIT, PRE_BASH, STOP];

/// The system directories, which hold the ordinary tools a shell script needs
/// and no `spec-spine`.
///
/// Asserted rather than assumed: if a `spec-spine` were installed here, every
/// resolution-order test would silently start measuring it instead of a stub,
/// and would still pass.
fn system_path() -> String {
    for dir in ["/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
        assert!(
            !Path::new(dir).join("spec-spine").exists(),
            "{dir} holds a spec-spine, so these fixtures cannot isolate one"
        );
    }
    "/usr/bin:/bin:/usr/sbin:/sbin".to_string()
}

/// The PATH a fixture run uses: its own directory first, then the system.
fn fixture_path(path_dir: &Path) -> String {
    format!("{}:{}", path_dir.display(), system_path())
}

fn executable(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// A git repository that is a Statecraft project, plus a hook written out.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    dir: PathBuf,
    path_dir: PathBuf,
}

impl Fixture {
    /// A fixture whose repository carries the manifest that makes it managed.
    fn new() -> Self {
        Self::build(true)
    }

    /// A fixture whose repository is an ordinary one, with no manifest.
    fn unmanaged() -> Self {
        Self::build(false)
    }

    fn build(managed: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join("specs/000-x")).unwrap();
        std::fs::write(root.join("specs/000-x/spec.md"), "# x\n").unwrap();
        if managed {
            std::fs::create_dir_all(root.join(".statecraft")).unwrap();
            std::fs::write(root.join(".statecraft/environment.json"), "{}").unwrap();
        }
        let out = Command::new("git")
            .args(["init", "--quiet", "--initial-branch=main"])
            .current_dir(&root)
            .output()
            .expect("git is available");
        assert!(out.status.success(), "git init failed");

        // PATH is this directory followed by the system ones. The fixture
        // directory comes first so a stub always wins, and the system
        // directories supply the ordinary tools a shell script needs
        // (`dirname`, `awk`, `jq`) without supplying a `spec-spine`: the
        // developer's own copy lives under `~/.cargo/bin`, which is not here,
        // and `system_path` asserts that rather than assuming it.
        let path_dir = dir.path().join("path");
        std::fs::create_dir_all(&path_dir).unwrap();

        let scripts = dir.path().join("scripts");
        Self {
            _dir: dir,
            root,
            dir: scripts,
            path_dir,
        }
    }

    fn hook(&self, file: &str) -> PathBuf {
        let path = self.dir.join(file);
        executable(&path, &hook_body(file));
        path
    }

    /// Place a stub `spec-spine` somewhere, recording which one ran.
    ///
    /// The stub appends its own label to `witness`, so a test can ask which
    /// binary the resolution order actually chose rather than inferring it.
    /// `carries_verbs: false` is a binary older than `check`: `clap` spends
    /// exit 2 on an unknown subcommand, which is also the staleness code.
    fn stub(&self, at: &Path, label: &str, check_code: i32, carries_verbs: bool) {
        self.stub_saying(at, label, check_code, carries_verbs, "");
    }

    /// A stub that also prints a report line, so contract 4 can assert that
    /// the hook reads the report rather than guessing from the exit code.
    fn stub_saying(
        &self,
        at: &Path,
        label: &str,
        check_code: i32,
        carries_verbs: bool,
        says: &str,
    ) {
        let witness = self.root.join("witness");
        let body = if carries_verbs {
            format!(
                "#!/bin/sh\nprintf '{label}\\n' >> '{}'\n\
                 case \"$1\" in --version) echo 'spec-spine 9.9.9'; exit 0 ;; esac\n\
                 case \"$1 $2\" in\n  'check --help'|'lint --help'|'couple --help') exit 0 ;;\nesac\n\
                 for a in \"$@\"; do\n  if [ \"$a\" = check ]; then printf '%s\\n' '{says}'; exit {check_code}; fi\ndone\n\
                 exit 0\n",
                witness.display()
            )
        } else {
            format!(
                "#!/bin/sh\nprintf '{label}\\n' >> '{}'\n\
                 case \"$1\" in --version) echo 'spec-spine 0.0.1'; exit 0 ;; esac\nexit 2\n",
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

    fn assert_nothing_ran(&self) {
        let w = self.witness();
        assert!(
            w.trim().is_empty(),
            "a stub ran when none should have: {w:?}"
        );
    }

    /// Run a `$CLAUDE_PROJECT_DIR` hook: `SessionStart` and `Stop`.
    fn run_project(&self, file: &str, env: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(self.hook(file));
        cmd.current_dir(std::env::temp_dir())
            .env("PATH", fixture_path(&self.path_dir))
            .env("CLAUDE_PROJECT_DIR", &self.root)
            .env_remove("SPEC_SPINE_BIN");
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }

    /// Run a stdin-payload hook: `PostToolUse` and `PreToolUse`.
    fn run_payload(&self, file: &str, payload: &str, env: &[(&str, &str)]) -> Output {
        use std::io::Write;
        let mut child = Command::new(self.hook(file))
            // A deliberately unrelated working directory: contract 3 says the
            // target comes from the command, so the session's own cwd must not
            // be what decides which tree is acted on.
            .current_dir(std::env::temp_dir())
            .env("PATH", fixture_path(&self.path_dir))
            .env("CLAUDE_PROJECT_DIR", &self.root)
            .env_remove("SPEC_SPINE_BIN")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A `PreToolUse` payload naming a Bash command in a repository.
fn bash_payload(command: &str, cwd: &Path) -> String {
    serde_json::json!({
        "tool_input": { "command": command },
        "cwd": cwd.display().to_string(),
    })
    .to_string()
}

/// A `PostToolUse` payload naming an edited file.
fn edit_payload(file: &Path) -> String {
    serde_json::json!({
        "tool_input": { "file_path": file.display().to_string() },
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// Contract 1: read, never repair.
// ---------------------------------------------------------------------------

/// Asserted over the body rather than by running it, because the failure this
/// contract prevents is a writing verb **reachable on some branch**, not one
/// on the branch a test happened to take. Running the hook can only show that
/// one path did not write.
#[test]
fn contract_1_no_hook_invokes_a_writing_subcommand() {
    // The verbs that write. `compile` and `index` are the two that regenerate
    // the derived tree; the rest change the corpus or the forge.
    const WRITING: [&str; 4] = ["compile", "index", "init", "ratify"];
    for file in ALL {
        let body = hook_body(file);
        for line in body.lines() {
            let code = line.split('#').next().unwrap_or("");
            for verb in WRITING {
                // An INVOCATION, not advice. These hooks legitimately tell a
                // session to `run spec-spine compile` in a report line; what
                // contract 1 forbids is the hook running it. Every invocation
                // in these scripts goes through the resolved binary `$sc`, so
                // that is what is matched, plus a bare name in command
                // position for a hook that skipped the resolution entirely.
                let through_sc = code.contains(&format!("\"$sc\" {verb}"))
                    || code.contains(&format!("\"$sc\" --repo \"$root\" {verb}"));
                let bare = code.trim_start().starts_with(&format!("spec-spine {verb}"))
                    || code.contains(&format!("$(spec-spine {verb}"))
                    || code.contains(&format!("; spec-spine {verb}"));
                if through_sc || bare {
                    // The single sanctioned exception: a `compile` after an
                    // edit to a spec.md, where the session is live and can
                    // commit the result.
                    let sanctioned = file == POST_EDIT && verb == "compile";
                    assert!(
                        sanctioned,
                        "{file} invokes the writing verb `{verb}`: {line}"
                    );
                }
            }
        }
    }
}

/// The sanctioned exception is exactly one verb, on exactly one trigger.
///
/// Not "post-edit may write": post-edit may recompile after a **spec edit**,
/// and this asserts the guard is the spec path rather than any edit at all.
#[test]
fn contract_1_the_sanctioned_compile_is_guarded_on_a_spec_edit() {
    let fixture = Fixture::new();
    let sc = fixture.root.join("target/release/spec-spine");
    fixture.stub(&sc, "repo-build", 0, true);

    // An ordinary source edit: nothing is regenerated.
    let src = fixture.root.join("src/lib.rs");
    std::fs::create_dir_all(src.parent().unwrap()).unwrap();
    std::fs::write(&src, "// x\n").unwrap();
    let out = fixture.run_payload(POST_EDIT, &edit_payload(&src), &[]);
    assert!(
        !text(&out).contains("recompiled"),
        "an ordinary edit triggered a regeneration: {}",
        text(&out)
    );

    // A spec edit: the one sanctioned write.
    let spec = fixture.root.join("specs/000-x/spec.md");
    let out = fixture.run_payload(POST_EDIT, &edit_payload(&spec), &[]);
    assert!(
        text(&out).contains("recompiled"),
        "the sanctioned compile did not run after a spec edit: {}",
        text(&out)
    );
}

// ---------------------------------------------------------------------------
// Contract 2: resolve the binary in order.
// ---------------------------------------------------------------------------

/// `$SPEC_SPINE_BIN` wins over everything.
#[test]
fn contract_2_spec_spine_bin_is_preferred() {
    for file in [SESSION_START, STOP] {
        let fixture = Fixture::new();
        let named = fixture.root.join("named-spec-spine");
        fixture.stub(&named, "named", 0, true);
        fixture.stub(
            &fixture.root.join("target/release/spec-spine"),
            "repo",
            0,
            true,
        );
        fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 0, true);

        fixture.run_project(file, &[("SPEC_SPINE_BIN", &named.display().to_string())]);
        fixture.assert_only_ran("named");
    }
}

/// The repository's own release build beats `PATH`.
///
/// This is the half a shipped hook is most likely to get wrong, and it is the
/// one that decides whether a repository is governed by the binary it builds
/// or by whichever copy an unrelated project installed last.
#[test]
fn contract_2_the_repositorys_own_build_beats_path() {
    for file in [SESSION_START, STOP] {
        let fixture = Fixture::new();
        fixture.stub(
            &fixture.root.join("target/release/spec-spine"),
            "repo",
            0,
            true,
        );
        fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 0, true);

        fixture.run_project(file, &[]);
        fixture.assert_only_ran("repo");
    }
}

/// `PATH` is the fallback and still works, which is what keeps an adopter on
/// the published CLI working.
#[test]
fn contract_2_path_is_the_fallback_and_still_works() {
    for file in [SESSION_START, STOP] {
        let fixture = Fixture::new();
        fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 0, true);

        fixture.run_project(file, &[]);
        fixture.assert_only_ran("path");
    }
}

/// The post-edit hook resolves against the repository the edited file is in.
#[test]
fn contract_2_post_edit_resolves_against_the_edited_repository() {
    let fixture = Fixture::new();
    fixture.stub(
        &fixture.root.join("target/release/spec-spine"),
        "repo",
        0,
        true,
    );
    fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 0, true);

    let spec = fixture.root.join("specs/000-x/spec.md");
    fixture.run_payload(POST_EDIT, &edit_payload(&spec), &[]);
    fixture.assert_only_ran("repo");
}

/// A binary that is simply absent is reported, not crashed on.
#[test]
fn contract_2_a_missing_binary_is_reported_and_is_not_a_crash() {
    for file in [SESSION_START, STOP] {
        let fixture = Fixture::new();
        let out = fixture.run_project(file, &[]);
        assert!(
            out.status.success(),
            "{file} failed when the binary was absent"
        );
        assert!(
            text(&out).contains("absent") || text(&out).contains("skipped"),
            "{file} did not say the binary was missing: {}",
            text(&out)
        );
    }
}

// ---------------------------------------------------------------------------
// Contract 3: resolve the target repository from the command, not the session.
// ---------------------------------------------------------------------------

/// A multi-repository session acts in whichever tree the command names.
#[test]
fn contract_3_the_target_repository_comes_from_the_command() {
    let fixture = Fixture::new();
    // A second managed repository, which is the one the command will name.
    let other = fixture.root.parent().unwrap().join("other");
    std::fs::create_dir_all(other.join(".statecraft")).unwrap();
    std::fs::write(other.join(".statecraft/environment.json"), "{}").unwrap();
    std::fs::create_dir_all(other.join("specs")).unwrap();
    let out = Command::new("git")
        .args(["init", "--quiet", "--initial-branch=main"])
        .current_dir(&other)
        .output()
        .unwrap();
    assert!(out.status.success());

    // The stub lives only in the OTHER repository, so if the hook ran it, the
    // hook resolved the target from the command rather than from the session.
    fixture.stub(&other.join("target/release/spec-spine"), "other", 0, true);
    fixture.stub(
        &fixture.root.join("target/release/spec-spine"),
        "session",
        0,
        true,
    );

    let payload = bash_payload(
        &format!("cd {} && git push origin main", other.display()),
        &fixture.root,
    );
    fixture.run_payload(PRE_BASH, &payload, &[]);

    let w = fixture.witness();
    assert!(
        !w.lines().any(|l| l == "session"),
        "the hook governed the session's own repository rather than the one the \
         command names: {w:?}"
    );
}

/// An edit in a sibling checkout does not recompile this one.
#[test]
fn contract_3_an_edit_in_a_sibling_checkout_is_judged_there() {
    let fixture = Fixture::new();
    let sibling = fixture.root.parent().unwrap().join("sibling");
    std::fs::create_dir_all(sibling.join("specs/000-y")).unwrap();
    std::fs::write(sibling.join("specs/000-y/spec.md"), "# y\n").unwrap();
    let out = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&sibling)
        .output()
        .unwrap();
    assert!(out.status.success());
    // The sibling is NOT a Statecraft project, so the hook is inert there even
    // though the session's own repository is one.
    fixture.stub(
        &fixture.root.join("target/release/spec-spine"),
        "session",
        0,
        true,
    );

    let out = fixture.run_payload(
        POST_EDIT,
        &edit_payload(&sibling.join("specs/000-y/spec.md")),
        &[],
    );
    assert!(out.status.success());
    fixture.assert_nothing_ran();
}

// ---------------------------------------------------------------------------
// Contract 4: read the verdict; never guess it.
// ---------------------------------------------------------------------------

/// `check` has four answers and they are not interchangeable.
///
/// Each row gives the stub an exit code and a report line, and asserts the
/// hook reports the condition that code actually means. Only `2` is the one
/// regenerating repairs, so only `2` may say so.
#[test]
fn contract_4_each_verdict_is_read_as_itself() {
    let rows: [(i32, &str, &str, bool); 4] = [
        (
            0,
            "spec-registry: fresh\ncodebase-index: fresh",
            "fresh",
            false,
        ),
        (
            1,
            "spec-registry: INVALID\ncodebase-index: INVALID",
            "INVALID",
            false,
        ),
        (
            2,
            "spec-registry: STALE\ncodebase-index: STALE",
            "STALE",
            true,
        ),
        (3, "", "NOT READ", false),
    ];
    for file in [SESSION_START, STOP] {
        for (code, says, expect, may_advise_regenerating) in rows {
            let fixture = Fixture::new();
            fixture.stub_saying(
                &fixture.path_dir.join("spec-spine"),
                "path",
                code,
                true,
                says,
            );
            let out = fixture.run_project(file, &[]);
            let seen = text(&out);
            if code == 0 {
                // A fresh tree says nothing alarming, and above all does not
                // send anyone to regenerate.
                assert!(
                    !seen.contains("compile") || !seen.contains("STALE"),
                    "{file} reported a fresh tree as stale: {seen}"
                );
                continue;
            }
            assert!(
                seen.contains(expect),
                "{file} did not report exit {code} as {expect}: {seen}"
            );
            if !may_advise_regenerating {
                assert!(
                    !seen.contains("run `spec-spine compile`")
                        && !seen.contains("run spec-spine compile"),
                    "{file} sent a session to regenerate for exit {code}, which \
                     regenerating does not repair: {seen}"
                );
            }
        }
    }
}

/// An unresolved claim is not staleness, and the hook says so.
#[test]
fn contract_4_an_unresolved_claim_is_distinguished_from_staleness() {
    for file in [SESSION_START, STOP] {
        let fixture = Fixture::new();
        fixture.stub_saying(
            &fixture.path_dir.join("spec-spine"),
            "path",
            2,
            true,
            "codebase-index: UNRESOLVED CLAIM",
        );
        let out = fixture.run_project(file, &[]);
        let seen = text(&out);
        assert!(
            seen.contains("UNRESOLVED CLAIM"),
            "{file} folded an unresolved claim into staleness: {seen}"
        );
        assert!(
            seen.contains("regenerating does not clear") || seen.contains("not staleness"),
            "{file} did not say regenerating will not clear it: {seen}"
        );
    }
}

// ---------------------------------------------------------------------------
// Contract 5: establish the verb before reading its exit code.
// ---------------------------------------------------------------------------

/// A binary older than the verb reports a fresh tree as stale unless the hook
/// asks first: `clap` spends `2` on an unknown subcommand and this tool spends
/// `2` on staleness.
#[test]
fn contract_5_a_missing_verb_is_not_reported_as_stale() {
    for file in [SESSION_START, STOP] {
        let fixture = Fixture::new();
        fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 2, false);
        let out = fixture.run_project(file, &[]);
        let seen = text(&out);
        assert!(
            !seen.contains("STALE"),
            "{file} reported a binary older than the verb as a stale tree: {seen}"
        );
        // The two hooks word it differently and both are correct. What is
        // asserted is that the report names the binary rather than the tree.
        assert!(
            seen.contains("predates")
                || seen.contains("does not carry the check verb")
                || seen.contains("rebuild")
                || seen.contains("reinstall"),
            "{file} did not name the real problem: {seen}"
        );
    }
}

// ---------------------------------------------------------------------------
// Contract 6: a gate whose check did not run is not green.
// ---------------------------------------------------------------------------

/// The enforcing gate refuses every non-zero code, including the ones that are
/// not about the corpus at all.
#[test]
fn contract_6_an_enforcing_gate_refuses_every_non_zero_code() {
    for code in [1, 2, 3, 7] {
        let fixture = Fixture::new();
        fixture.stub(
            &fixture.root.join("target/release/spec-spine"),
            "repo",
            code,
            true,
        );
        let payload = bash_payload("gh pr create --title x --body y", &fixture.root);
        let out = fixture.run_payload(PRE_BASH, &payload, &[]);
        assert!(
            !out.status.success(),
            "the pull-request gate allowed the operation on check exit {code}: {}",
            text(&out)
        );
    }
}

/// Only `enforcing` hooks refuse. The others report and hand back.
#[test]
fn contract_6_only_the_enforcing_event_is_declared_enforcing() {
    use statecraft_home::harness::ADOPTED_HOOKS;
    let enforcing: Vec<&str> = ADOPTED_HOOKS
        .iter()
        .filter(|h| h.enforcing)
        .map(|h| h.event)
        .collect();
    assert_eq!(
        enforcing,
        ["PreToolUse"],
        "the set of enforcing events changed; section 3.23's Stop policy says \
         an operation gate enforces and an end-of-turn event advises"
    );
}

// ---------------------------------------------------------------------------
// Contract 7: a branch gate resolves the protected branch.
// ---------------------------------------------------------------------------

/// The protected branch is resolved rather than assumed to be `main`.
#[test]
fn contract_7_the_protected_branch_is_resolved_not_assumed() {
    let fixture = Fixture::new();
    fixture.stub(
        &fixture.root.join("target/release/spec-spine"),
        "repo",
        0,
        true,
    );

    // A push to the branch the environment names is refused.
    let payload = bash_payload("git push origin release", &fixture.root);
    let out = fixture.run_payload(
        PRE_BASH,
        &payload,
        &[("SPEC_SPINE_DEFAULT_BRANCH", "release")],
    );
    assert!(
        !out.status.success(),
        "a push to the declared protected branch was allowed: {}",
        text(&out)
    );

    // And a push to `main` is not, because `main` is not this repository's
    // protected branch. A gate that assumed `main` would refuse this one and
    // allow the one above, which is both errors at once.
    let payload = bash_payload("git push origin main", &fixture.root);
    let out = fixture.run_payload(
        PRE_BASH,
        &payload,
        &[("SPEC_SPINE_DEFAULT_BRANCH", "release")],
    );
    assert!(
        out.status.success(),
        "a push to an unprotected branch was refused because the gate assumed \
         `main`: {}",
        text(&out)
    );
}

/// The gate is anchored on the command that invokes the verb, so text that
/// merely contains the words still runs.
#[test]
fn contract_7_command_text_that_merely_mentions_a_push_is_not_a_push() {
    let fixture = Fixture::new();
    fixture.stub(
        &fixture.root.join("target/release/spec-spine"),
        "repo",
        0,
        true,
    );
    for command in [
        "grep -rn 'git push origin main' docs/",
        "echo 'remember to git push origin main later'",
    ] {
        let payload = bash_payload(command, &fixture.root);
        let out = fixture.run_payload(PRE_BASH, &payload, &[("SPEC_SPINE_DEFAULT_BRANCH", "main")]);
        assert!(
            out.status.success(),
            "a command that only mentions a push was refused: {command}: {}",
            text(&out)
        );
    }
}

// ---------------------------------------------------------------------------
// The Stop policy, adopted 2026-09-21.
// ---------------------------------------------------------------------------

/// `Stop` is advisory: it reports, and it never blocks.
///
/// Every answer `check` can give, including the ones that are refusals for a
/// gate. A session that has something to hand back hands it back, and a stale
/// derived tree is a fact the handback carries rather than a reason to
/// withhold it.
#[test]
fn stop_policy_is_advisory_for_every_answer_check_can_give() {
    for (code, says) in [
        (0, "spec-registry: fresh"),
        (1, ""),
        (2, "spec-registry: STALE"),
        (2, "codebase-index: UNRESOLVED CLAIM"),
        (3, ""),
        (9, ""),
    ] {
        let fixture = Fixture::new();
        fixture.stub_saying(
            &fixture.path_dir.join("spec-spine"),
            "path",
            code,
            true,
            says,
        );
        let out = fixture.run_project(STOP, &[]);
        assert!(
            out.status.success(),
            "Stop blocked on check exit {code} ({says:?}), which would cost the \
             handback that says why the work failed: {}",
            text(&out)
        );
    }
}

/// And it is advisory when the read could not be performed at all.
#[test]
fn stop_policy_does_not_block_when_the_binary_is_absent() {
    let fixture = Fixture::new();
    let out = fixture.run_project(STOP, &[]);
    assert!(
        out.status.success(),
        "Stop blocked because it could not find a binary: {}",
        text(&out)
    );
}

/// Advisory is not silent. Every non-zero answer is reported accurately.
#[test]
fn stop_policy_reports_accurately_rather_than_passing_quietly() {
    for (code, says, expect) in [
        (1, "", "INVALID"),
        (2, "spec-registry: STALE", "STALE"),
        (3, "", "NOT READ"),
        (9, "", "UNRECOGNISED"),
    ] {
        let fixture = Fixture::new();
        fixture.stub_saying(
            &fixture.path_dir.join("spec-spine"),
            "path",
            code,
            true,
            says,
        );
        let out = fixture.run_project(STOP, &[]);
        let seen = text(&out);
        assert!(
            seen.contains(expect),
            "Stop exited 0 without reporting exit {code} as {expect}, which is \
             advisory collapsed into silent: {seen}"
        );
    }
}

// ---------------------------------------------------------------------------
// Section 3.14 rule 3: gated to a Statecraft project.
// ---------------------------------------------------------------------------

/// Outside a Statecraft project every delivered behavior is inert.
///
/// The manifest is the gate, not the presence of a `specs/` directory: the
/// fixture below is a git repository holding a spec corpus, and an unrelated
/// corpus is not this product's to report on.
#[test]
fn outside_a_statecraft_project_every_hook_is_inert() {
    for file in ALL {
        let fixture = Fixture::unmanaged();
        fixture.stub(
            &fixture.root.join("target/release/spec-spine"),
            "repo",
            2,
            true,
        );
        fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 2, true);

        let out = match file {
            POST_EDIT => fixture.run_payload(
                file,
                &edit_payload(&fixture.root.join("specs/000-x/spec.md")),
                &[],
            ),
            PRE_BASH => fixture.run_payload(
                file,
                &bash_payload("git push origin main", &fixture.root),
                &[("SPEC_SPINE_DEFAULT_BRANCH", "main")],
            ),
            _ => fixture.run_project(file, &[]),
        };
        assert!(
            out.status.success(),
            "{file} refused an operation in an unrelated repository: {}",
            text(&out)
        );
        fixture.assert_nothing_ran();
    }
}

/// And inside one, the same hooks are not inert, so the test above is not
/// passing because the fixture is broken.
#[test]
fn inside_a_statecraft_project_the_hooks_do_run() {
    for file in [SESSION_START, STOP] {
        let fixture = Fixture::new();
        fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 0, true);
        fixture.run_project(file, &[]);
        fixture.assert_only_ran("path");
    }
}

// ---------------------------------------------------------------------------
// Duplicate registrations.
// ---------------------------------------------------------------------------

/// A hook registered twice runs twice and is still correct.
///
/// A user who already registers a shipped script keeps their registration
/// (section 3.28), so a session can genuinely run the same body twice on one
/// event. It must be idempotent in the only sense that matters here: reading
/// twice reports twice and changes nothing.
#[test]
fn a_duplicate_registration_is_harmless_because_the_hook_only_reads() {
    let fixture = Fixture::new();
    fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 0, true);
    let first = fixture.run_project(SESSION_START, &[]);
    let second = fixture.run_project(SESSION_START, &[]);
    assert!(first.status.success() && second.status.success());
    assert_eq!(
        text(&first),
        text(&second),
        "a second run of the same hook answered differently"
    );
}
