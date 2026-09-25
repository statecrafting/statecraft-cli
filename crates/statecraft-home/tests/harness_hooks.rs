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
//! | 2 binary resolution order, and the pin (amended 2026-09-24) | all four | `contract_2_*`, `pin_*` |
//! | 3 target from the command | post-edit, pre-bash | `contract_3_*` |
//! | 4 read the verdict, never guess it; the gate's unresolved flag (amended 2026-09-24) | all four | `contract_4_*`, `h4_*` |
//! | 5 establish the verb first | session-start, stop | `contract_5_*` |
//! | 6 a gate whose check did not run is not green | pre-bash | `contract_6_*`, and the seven derived-tree cases below |
//! | 7 a branch gate resolves the protected branch | pre-bash | `contract_7_*` |
//! | the Stop policy: advisory, and accurate | stop | `stop_policy_*` |
//! | section 3.14 rule 3, the project gate | all four | `outside_a_statecraft_project_*` |
//!
//! Each run is isolated from the operator's own configuration as well as from
//! their `spec-spine`: `HOME` is the fixture's, and git is told to read neither
//! the global nor the system configuration, so an operator's `core.hooksPath`,
//! their `includeIf` blocks and their own global hooks cannot reach into a
//! measurement of this product's. Where running the hook is part of the claim,
//! the stub's witness is asserted nonempty: "every line is X" is satisfied
//! vacuously by a hook that ran nothing.

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

/// The derived directory the fixture's stub reports and the fixture commits.
///
/// The real path this repository uses, so the test exercises the shape the
/// hook meets rather than a convenient one.
const DERIVED_DIR: &str = ".statecraft/derived";

/// The version every stub that carries the verbs reports, and the pin a
/// default fixture declares, so the stubs are compatible by construction.
const STUB_PIN: &str = "=9.9.9";

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

/// The real `git`, by absolute path.
///
/// A stub that intercepts one subcommand has to hand every other one to the
/// real binary, and it may not do that through `PATH`, because it is itself
/// first on `PATH`.
fn real_git() -> PathBuf {
    for dir in ["/usr/bin", "/bin"] {
        let candidate = Path::new(dir).join("git");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("no git on the system path, which these fixtures need");
}

fn executable(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    // Staged and copied, never written in place: a script this process
    // wrote and then exec'd is the `ETXTBSY` race spec 004 records.
    statecraft_adapter::fixture::install_script(path, body, 0o755).unwrap();
}

/// What a stub `spec-spine` answers when asked for the configuration.
///
/// The derived-tree check reads `config show --json` and takes
/// `layout.derived_dir` from it, so what that call answers is the
/// difference between a check that ran and one that did not.
#[derive(Clone, Copy)]
enum ConfigAnswer {
    /// A well-formed answer naming the derived directory.
    Reports(&'static str),
    /// A well-formed answer that does not carry the key.
    WithoutTheKey,
    /// Output the reader cannot parse.
    NotJson,
    /// The verb fails.
    Fails,
}

impl ConfigAnswer {
    /// The shell that answers a `config` invocation.
    fn shell(self) -> String {
        match self {
            ConfigAnswer::Reports(dir) => {
                format!("printf '%s\\n' '{{\"layout\":{{\"derived_dir\":\"{dir}\"}}}}'; exit 0")
            }
            ConfigAnswer::WithoutTheKey => "printf '%s\\n' '{\"layout\":{}}'; exit 0".to_string(),
            ConfigAnswer::NotJson => "printf '%s\\n' 'not json at all'; exit 0".to_string(),
            ConfigAnswer::Fails => "exit 4".to_string(),
        }
    }
}

/// A git repository that is a Statecraft project, plus a hook written out.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    dir: PathBuf,
    path_dir: PathBuf,
    home: PathBuf,
}

impl Fixture {
    /// A fixture whose repository carries the manifest that makes it managed,
    /// pinned to the version every carrying stub reports, so contract 2's
    /// compatibility test admits the stubs and each older test keeps its
    /// meaning.
    fn new() -> Self {
        Self::build(true, Some(STUB_PIN))
    }

    /// A managed fixture with a chosen pin, or none at all.
    fn pinned(pin: Option<&str>) -> Self {
        Self::build(true, pin)
    }

    /// A fixture whose repository is an ordinary one, with no manifest.
    fn unmanaged() -> Self {
        Self::build(false, Some(STUB_PIN))
    }

    fn build(managed: bool, pin: Option<&str>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join("specs/000-x")).unwrap();
        std::fs::write(root.join("specs/000-x/spec.md"), "# x\n").unwrap();
        if managed {
            std::fs::create_dir_all(root.join(".statecraft")).unwrap();
            std::fs::write(root.join(".statecraft/environment.json"), "{}").unwrap();
        }
        if let Some(pin) = pin {
            std::fs::write(
                root.join("spec-spine.toml"),
                format!("[meta]\nrequired_version = \"{pin}\"\n"),
            )
            .unwrap();
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
        let home = dir.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        Self {
            _dir: dir,
            root,
            dir: scripts,
            path_dir,
            home,
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
        self.stub_answering(
            at,
            label,
            check_code,
            carries_verbs,
            says,
            ConfigAnswer::Reports(DERIVED_DIR),
        );
    }

    /// A stub that also decides what `config show --json` answers, which is
    /// what the derived-tree check reads the derived directory from.
    fn stub_answering(
        &self,
        at: &Path,
        label: &str,
        check_code: i32,
        carries_verbs: bool,
        says: &str,
        config: ConfigAnswer,
    ) {
        self.stub_full(at, label, check_code, carries_verbs, says, config, false);
    }

    /// A binary that carries `check` but not `--fail-on-unresolved`: its help
    /// does not name the flag, and `clap` spends exit 2 on the argument.
    fn stub_flagless(&self, at: &Path, label: &str, check_code: i32) {
        self.stub_full(
            at,
            label,
            check_code,
            true,
            "",
            ConfigAnswer::Reports(DERIVED_DIR),
            true,
        );
    }

    /// Every argument vector any carrying stub was invoked with, one per line.
    fn argv(&self) -> String {
        std::fs::read_to_string(self.root.join("argv")).unwrap_or_default()
    }

    #[allow(clippy::too_many_arguments)]
    fn stub_full(
        &self,
        at: &Path,
        label: &str,
        check_code: i32,
        carries_verbs: bool,
        says: &str,
        config: ConfigAnswer,
        flagless: bool,
    ) {
        let witness = self.root.join("witness");
        let body = if carries_verbs {
            format!(
                "#!/bin/sh\nprintf '{label}\\n' >> '{}'\nprintf '%s\\n' \"$*\" >> '{}'\n\
                 case \"$1\" in --version) echo 'spec-spine 9.9.9'; exit 0 ;; esac\n\
                 case \"$1 $2\" in\n  'check --help') {help}; exit 0 ;;\n  'lint --help'|'couple --help') exit 0 ;;\nesac\n\
                 for a in \"$@\"; do\n  if [ \"$a\" = config ]; then {}; fi\ndone\n\
                 for a in \"$@\"; do\n  if [ \"$a\" = --fail-on-unresolved ] && [ {flagless} = 1 ]; then exit 2; fi\ndone\n\
                 for a in \"$@\"; do\n  if [ \"$a\" = check ]; then printf '%s\\n' '{says}'; exit {check_code}; fi\ndone\n\
                 exit 0\n",
                witness.display(),
                self.root.join("argv").display(),
                config.shell(),
                help = if flagless {
                    "true"
                } else {
                    "echo '      --fail-on-unresolved'"
                },
                flagless = u8::from(flagless),
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

    /// The environment that keeps a fixture run out of the operator's own
    /// configuration.
    ///
    /// `HOME` is the fixture's own directory, so the hook's `~` expansion and
    /// anything that reads a home find the fixture's. Git is told to read
    /// neither the global nor the system configuration, so the operator's
    /// `core.hooksPath`, their `includeIf` blocks and their own global hooks
    /// cannot reach into a measurement of this product's.
    fn isolated(&self) -> Vec<(String, String)> {
        vec![
            ("HOME".to_string(), self.home.display().to_string()),
            ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ]
    }

    /// Run git inside the fixture repository, isolated from the operator's own
    /// configuration, and assert it succeeded.
    fn git(&self, args: &[&str]) -> String {
        let out = Command::new(real_git())
            .args(["-C", &self.root.display().to_string()])
            .args(args)
            .env("PATH", system_path())
            .envs(self.isolated())
            .env("GIT_AUTHOR_NAME", "fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    /// Write one file under the derived directory.
    fn write_derived(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.root.join(DERIVED_DIR).join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    /// Commit everything currently in the tree, so the repository has a HEAD
    /// and a committed derived tree to diverge from.
    fn commit_everything(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
    }

    /// Put a `git` on the fixture's PATH that refuses the derived-tree reads.
    ///
    /// Everything else is handed to the real binary, so the hook still
    /// resolves the repository and the branch; only the three reads the
    /// derived-tree check performs fail, which is the state being measured.
    fn break_the_derived_reads(&self) {
        executable(
            &self.path_dir.join("git"),
            &format!(
                "#!/bin/sh\nfor a in \"$@\"; do\n  case \"$a\" in diff|ls-files) exit 9 ;; esac\ndone\nexec '{}' \"$@\"\n",
                real_git().display()
            ),
        );
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
            .env_remove("SPEC_SPINE_BIN")
            .env_remove("STATECRAFT_SPEC_SPINE")
            .env_remove("STATECRAFT_RUN_ID")
            .envs(self.isolated());
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
            .env_remove("STATECRAFT_SPEC_SPINE")
            .env_remove("STATECRAFT_RUN_ID")
            .envs(self.isolated())
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

/// Outside a managed session, `$STATECRAFT_SPEC_SPINE` is the operator's
/// override and wins over everything (section 5, 2026-09-25: one variable
/// selects the binary).
#[test]
fn contract_2_the_override_is_preferred() {
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

        fixture.run_project(
            file,
            &[("STATECRAFT_SPEC_SPINE", &named.display().to_string())],
        );
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
// Contract 4 as amended on 2026-09-24 (owner decision H-4): the gate's
// unresolved-claim flag. Spec 002 section 5, obligations 1 to 5.
// ---------------------------------------------------------------------------

/// What spec-spine 0.25.0 prints for a draft's unresolved claim under the
/// flag, measured (`session10/H4/measure-draft-claim-ahead.txt`).
const REFUSED_UNRESOLVED: &str = "spec-registry: fresh\ncodebase-index: fresh, but REFUSED: 1 unresolved unit diagnostic(s) (--fail-on-unresolved)";

/// Obligation 1: every hook that runs `check` passes the flag, and passing it
/// brings no writing verb with it.
#[test]
fn h4_every_freshness_read_passes_the_gates_unresolved_flag() {
    let spec_edit = |f: &Fixture| edit_payload(&f.root.join("specs/000-x/spec.md"));
    for file in ALL {
        let fixture = Fixture::new();
        fixture.stub_saying(
            &fixture.path_dir.join("spec-spine"),
            "path",
            0,
            true,
            "spec-registry: fresh\ncodebase-index: fresh",
        );
        let _ = match file {
            PRE_BASH => fixture.run_payload(
                file,
                &bash_payload("gh pr create --title x --body y", &fixture.root),
                &[],
            ),
            POST_EDIT => fixture.run_payload(file, &spec_edit(&fixture), &[]),
            _ => fixture.run_project(file, &[]),
        };
        let argv = fixture.argv();
        let checks: Vec<&str> = argv
            .lines()
            .filter(|l| l.split(' ').any(|w| w == "check") && !l.contains("--help"))
            .collect();
        assert!(!checks.is_empty(), "{file} never ran check: {argv:?}");
        for line in &checks {
            assert!(
                line.contains("--fail-on-unresolved"),
                "{file} ran check without the gate's unresolved flag: {line:?}"
            );
        }
    }
}

/// Obligation 2: a draft's unresolved claim is refused by the gate, reported
/// by session start as refused (never fresh), and reported by Stop without a
/// block.
#[test]
fn h4_a_drafts_unresolved_claim_is_what_the_gate_refuses() {
    let fixture = Fixture::new();
    let sc = fixture.path_dir.join("spec-spine");
    fixture.stub_saying(&sc, "path", 1, true, REFUSED_UNRESOLVED);
    let out = fixture.run_payload(
        PRE_BASH,
        &bash_payload("gh pr create --title x --body y", &fixture.root),
        &[],
    );
    let seen = text(&out);
    assert!(!out.status.success(), "the gate passed: {seen}");
    assert!(seen.contains("unresolved claim"), "{seen}");
    assert!(
        !seen.contains("does not validate"),
        "the gate called an unresolved claim an invalid corpus: {seen}"
    );

    let fixture = Fixture::new();
    fixture.stub_saying(
        &fixture.path_dir.join("spec-spine"),
        "path",
        1,
        true,
        REFUSED_UNRESOLVED,
    );
    let out = fixture.run_project(SESSION_START, &[]);
    let seen = text(&out);
    assert!(
        seen.contains("codebase index: REFUSED by the gate"),
        "{seen}"
    );
    assert!(!seen.contains("codebase index: fresh"), "{seen}");

    let fixture = Fixture::new();
    fixture.stub_saying(
        &fixture.path_dir.join("spec-spine"),
        "path",
        1,
        true,
        REFUSED_UNRESOLVED,
    );
    let out = fixture.run_project(STOP, &[]);
    let seen = text(&out);
    assert!(out.status.success(), "Stop blocked: {seen}");
    assert!(seen.contains("UNRESOLVED CLAIM"), "{seen}");
    assert!(!seen.contains("INVALID"), "{seen}");
}

/// Obligation 3: an unresolved claim riding with stale shards is exit 1, and
/// each hook names both and says regenerating clears only the stale part.
#[test]
fn h4_an_unresolved_claim_with_stale_shards_names_both() {
    let says = "spec-registry: fresh\ncodebase-index: STALE (run `spec-spine index`)\ncodebase-index: UNRESOLVED CLAIM: 1 unresolved claim(s) over 1 spec(s), which is not staleness";
    let fixture = Fixture::new();
    fixture.stub_saying(&fixture.path_dir.join("spec-spine"), "path", 1, true, says);
    let out = fixture.run_payload(
        PRE_BASH,
        &bash_payload("gh pr create --title x --body y", &fixture.root),
        &[],
    );
    let seen = text(&out);
    assert!(!out.status.success(), "{seen}");
    assert!(
        seen.contains("unresolved claim") && seen.contains("stale"),
        "{seen}"
    );
    assert!(seen.contains("clear that part only"), "{seen}");

    let fixture = Fixture::new();
    fixture.stub_saying(&fixture.path_dir.join("spec-spine"), "path", 1, true, says);
    let seen = text(&fixture.run_project(SESSION_START, &[]));
    assert!(seen.contains("STALE plus UNRESOLVED CLAIM"), "{seen}");

    let fixture = Fixture::new();
    fixture.stub_saying(&fixture.path_dir.join("spec-spine"), "path", 1, true, says);
    let seen = text(&fixture.run_project(STOP, &[]));
    assert!(
        seen.contains("UNRESOLVED CLAIM") && seen.contains("STALE as well"),
        "{seen}"
    );
}

/// Obligation 4: an invalid corpus is reported as today.
#[test]
fn h4_an_invalid_corpus_is_still_reported_as_invalid() {
    let says = "spec-registry: INVALID\ncodebase-index: fresh";
    let fixture = Fixture::new();
    fixture.stub_saying(&fixture.path_dir.join("spec-spine"), "path", 1, true, says);
    let seen = text(&fixture.run_payload(
        PRE_BASH,
        &bash_payload("gh pr create --title x --body y", &fixture.root),
        &[],
    ));
    assert!(seen.contains("does not validate"), "{seen}");
    assert!(!seen.contains("unresolved claim"), "{seen}");
    let fixture = Fixture::new();
    fixture.stub_saying(&fixture.path_dir.join("spec-spine"), "path", 1, true, says);
    let seen = text(&fixture.run_project(STOP, &[]));
    assert!(
        seen.contains("INVALID") && !seen.contains("UNRESOLVED"),
        "{seen}"
    );
}

/// Obligation 5: a binary whose `check --help` does not name the flag is
/// treated as lacking the verb, never read as stale.
#[test]
fn h4_a_binary_without_the_flag_is_not_read_as_stale() {
    for file in ALL {
        let fixture = Fixture::new();
        fixture.stub_flagless(&fixture.path_dir.join("spec-spine"), "path", 0);
        let out = match file {
            PRE_BASH => fixture.run_payload(
                file,
                &bash_payload("gh pr create --title x --body y", &fixture.root),
                &[],
            ),
            POST_EDIT => fixture.run_payload(
                file,
                &edit_payload(&fixture.root.join("specs/000-x/spec.md")),
                &[],
            ),
            _ => fixture.run_project(file, &[]),
        };
        let seen = text(&out);
        assert!(
            !seen.contains("STALE") && !seen.contains("is stale"),
            "{file}: {seen}"
        );
        assert!(
            seen.contains("--fail-on-unresolved"),
            "{file} did not name the missing flag: {seen}"
        );
        if file == PRE_BASH {
            assert!(
                !out.status.success(),
                "the gate passed an unjudged tree: {seen}"
            );
        } else {
            assert!(out.status.success(), "{file} blocked: {seen}");
        }
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
        // Pinned to the version the old binary reports, so contract 2 admits
        // it and contract 5 is what is measured.
        let fixture = Fixture::pinned(Some("=0.0.1"));
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
// Contract 6, on the derived-tree check the pull-request gate performs.
// ---------------------------------------------------------------------------
//
// The earlier round recorded this as a coverage gap and gave a reason that is
// wrong: it said the shipped hooks never inspect the index, so the three states
// would be indistinguishable and a test would be asserting a property of `git`.
// The shipped `statecraft-pre-bash.sh` reads `git diff` for the unstaged state,
// `git diff --cached` for the staged one and `git ls-files --others` for the
// untracked one, on three separate lines. What follows is not a test of `git`:
// it is a test of what THIS hook does with the three answers, which of them it
// names in the message, and whether it refuses the operation it stands in front
// of.

/// A fixture with a committed derived tree and a stub that reports where it is.
///
/// The positive control below is what makes the six refusals meaningful: a
/// suite in which the gate refused unconditionally would pass every one of
/// them.
fn derived_fixture() -> (Fixture, PathBuf) {
    let fixture = Fixture::new();
    fixture.stub(
        &fixture.root.join("target/release/spec-spine"),
        "repo",
        0,
        true,
    );
    let shard = fixture.write_derived("spec-registry.json", "{\"committed\":true}\n");
    fixture.commit_everything("the committed derived tree");
    (fixture, shard)
}

/// Open a pull request from the fixture, which is the operation this gate
/// stands in front of.
fn pr_create(fixture: &Fixture) -> Output {
    let payload = bash_payload("gh pr create --title x --body y", &fixture.root);
    fixture.run_payload(PRE_BASH, &payload, &[("SPEC_SPINE_DEFAULT_BRANCH", "main")])
}

/// The positive control: a committed derived tree does not refuse.
#[test]
fn the_derived_tree_check_passes_a_tree_whose_shards_are_committed() {
    let (fixture, _shard) = derived_fixture();
    let out = pr_create(&fixture);
    assert!(
        out.status.success(),
        "the gate refused a clean tree, so every refusal below proves nothing: {}",
        text(&out)
    );
    // The check ran: the stub was asked, rather than the hook exiting early.
    fixture.assert_only_ran("repo");
}

/// An unstaged derived change is refused, and named as unstaged.
#[test]
fn an_unstaged_derived_change_refuses_the_pull_request() {
    let (fixture, shard) = derived_fixture();
    std::fs::write(&shard, "{\"committed\":false}\n").unwrap();

    let out = pr_create(&fixture);
    let seen = text(&out);
    assert!(!out.status.success(), "the gate allowed it: {seen}");
    assert!(seen.contains("unstaged changes"), "{seen}");
    assert!(seen.contains("spec-registry.json"), "{seen}");
    assert!(!fixture.witness().trim().is_empty(), "no stub ran at all");
}

/// A staged derived change is refused, and named as staged.
///
/// The state `git add` leaves behind, and the one a bare `git diff` misses:
/// the index and the working tree agree, so the unstaged read is empty and only
/// the cached read answers.
#[test]
fn a_staged_derived_change_refuses_the_pull_request() {
    let (fixture, shard) = derived_fixture();
    std::fs::write(&shard, "{\"committed\":false}\n").unwrap();
    fixture.git(&["add", DERIVED_DIR]);

    let out = pr_create(&fixture);
    let seen = text(&out);
    assert!(!out.status.success(), "the gate allowed it: {seen}");
    assert!(seen.contains("staged changes"), "{seen}");
    assert!(
        !seen.contains("unstaged changes"),
        "a staged-only change was reported as unstaged, and the two remedies \
         differ: {seen}"
    );
}

/// An untracked derived file is refused, and named as untracked.
///
/// The state a new spec's shards leave behind, and the other one a bare
/// `git diff` misses: git knows nothing about the file, so neither diff
/// mentions it.
#[test]
fn an_untracked_derived_file_refuses_the_pull_request() {
    let (fixture, _shard) = derived_fixture();
    fixture.write_derived("codebase-index.json", "{\"new\":true}\n");

    let out = pr_create(&fixture);
    let seen = text(&out);
    assert!(!out.status.success(), "the gate allowed it: {seen}");
    assert!(seen.contains("untracked files"), "{seen}");
    assert!(seen.contains("codebase-index.json"), "{seen}");
}

/// A staged change whose working-tree contents cancel it is still refused, and
/// both states are named.
///
/// HEAD and the working tree agree, so one HEAD-relative comparison finds
/// nothing at all; the index holds an edit that a commit would land. The two
/// per-state reads both answer, and the message names both because the
/// remedies differ: one is `git add`, the other is `git commit`.
#[test]
fn a_staged_change_cancelled_by_the_working_tree_is_still_refused() {
    let (fixture, shard) = derived_fixture();
    let committed = std::fs::read_to_string(&shard).unwrap();
    std::fs::write(&shard, "{\"staged\":true}\n").unwrap();
    fixture.git(&["add", DERIVED_DIR]);
    std::fs::write(&shard, &committed).unwrap();

    // The premise: against HEAD alone this tree looks clean.
    assert!(
        fixture
            .git(&["diff", "--name-only", "HEAD", "--", DERIVED_DIR])
            .trim()
            .is_empty(),
        "the fixture does not set up the state it is named for"
    );

    let out = pr_create(&fixture);
    let seen = text(&out);
    assert!(
        !out.status.success(),
        "a staged edit that a commit would land was allowed because the working \
         tree cancelled it against HEAD: {seen}"
    );
    assert!(seen.contains("staged changes"), "{seen}");
    assert!(seen.contains("unstaged changes"), "{seen}");
}

/// A derived-tree read that failed is refused, and is not reported as clean.
///
/// `git diff --name-only` prints nothing both when a tree is clean and when the
/// command failed. The hook reads each exit status, so the second is a check
/// that did not run rather than a clean answer.
#[test]
fn a_failed_derived_read_refuses_rather_than_reading_as_clean() {
    let (fixture, _shard) = derived_fixture();
    fixture.break_the_derived_reads();

    let out = pr_create(&fixture);
    let seen = text(&out);
    assert!(
        !out.status.success(),
        "a failed read was reported as a clean derived tree: {seen}"
    );
    assert!(seen.contains("NOT PERFORMED"), "{seen}");
    assert!(
        seen.contains("not a clean tree"),
        "the message does not distinguish an unperformed read from a clean one: {seen}"
    );
}

/// A derived directory the configuration does not report refuses.
///
/// Section 3.23 contract 6 and the Stop policy's second part: an enforcing
/// operation gate refuses a failed or unavailable check. The inherited script
/// announced `derived-tree check skipped` and continued, citing the
/// counterparty's own weaker wording. A gate in front of `gh pr create` that
/// never established the derived-tree state may not report it as clean, and the
/// weaker behavior is not preserved merely because it was copied.
#[test]
fn an_unanswered_derived_directory_refuses_rather_than_skipping() {
    for answer in [
        ConfigAnswer::WithoutTheKey,
        ConfigAnswer::NotJson,
        ConfigAnswer::Fails,
    ] {
        let fixture = Fixture::new();
        fixture.stub_answering(
            &fixture.root.join("target/release/spec-spine"),
            "repo",
            0,
            true,
            "",
            answer,
        );
        fixture.write_derived("spec-registry.json", "{}\n");
        fixture.commit_everything("the committed derived tree");

        let out = pr_create(&fixture);
        let seen = text(&out);
        assert!(
            !out.status.success(),
            "the gate continued past a derived-tree check it never performed: {seen}"
        );
        assert!(seen.contains("NOT PERFORMED"), "{seen}");
        assert!(
            !seen.contains("check skipped"),
            "the gate still announces a skip: {seen}"
        );
        assert!(!fixture.witness().trim().is_empty(), "no stub ran at all");
    }
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

// ---------------------------------------------------------------------------
// Spec 002 section 3.31: the startup acknowledgment.
// ---------------------------------------------------------------------------

fn acknowledgments(out: &Output) -> Vec<String> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("statecraft-startup"))
        .map(str::to_string)
        .collect()
}

const BINDING: [(&str, &str); 4] = [
    (
        "STATECRAFT_STARTUP_NONCE",
        "0123456789abcdef0123456789abcdef",
    ),
    ("STATECRAFT_RUN_ID", "003-x"),
    ("STATECRAFT_ATTEMPT", "2"),
    ("STATECRAFT_HARNESS_SELECTED", "none"),
];

/// Inside a project, with a run's binding in its environment, the hook prints
/// exactly one acknowledgment naming the binding it was given, the project it
/// ran in and the directory it is installed in, and still reports as before.
#[test]
fn the_session_start_hook_acknowledges_a_run_it_was_bound_to() {
    let fixture = Fixture::new();
    fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 0, true);
    let out = fixture.run_project(SESSION_START, &BINDING);
    assert!(out.status.success(), "{}", text(&out));
    let acks = acknowledgments(&out);
    assert_eq!(acks.len(), 1, "{}", text(&out));
    let project = fixture.root.canonicalize().unwrap();
    let installed = fixture.dir.parent().unwrap().canonicalize().unwrap();
    assert_eq!(
        acks[0],
        format!(
            "statecraft-startup\tv1\tnonce=0123456789abcdef0123456789abcdef\trun=003-x\t\
             attempt=2\tselected=none\tproject={}\troot={}",
            project.display(),
            installed.display()
        )
    );
    // The freshness report is unchanged beside it.
    assert!(text(&out).contains("[session-freshness]"), "{}", text(&out));
    fixture.assert_only_ran("path");
}

/// Without a binding the hook prints no acknowledgment: an ordinary session is
/// not a run, and nothing here pretends it is one.
#[test]
fn the_session_start_hook_acknowledges_nothing_without_a_binding() {
    let fixture = Fixture::new();
    fixture.stub(&fixture.path_dir.join("spec-spine"), "path", 0, true);
    let out = fixture.run_project(SESSION_START, &[]);
    assert!(out.status.success());
    assert!(acknowledgments(&out).is_empty(), "{}", text(&out));
}

/// Outside a project the gate holds first, binding or not: section 3.14 rule 3.
#[test]
fn the_session_start_hook_acknowledges_nothing_outside_a_project() {
    let fixture = Fixture::unmanaged();
    let out = fixture.run_project(SESSION_START, &BINDING);
    assert!(out.status.success());
    assert!(out.stdout.is_empty(), "{}", text(&out));
    fixture.assert_nothing_ran();
}

// ---------------------------------------------------------------------------
/// macOS spells a temporary directory both with and without `/private`, and
/// git answers with the resolved spelling, so paths are compared normalised.
fn norm(s: &str) -> String {
    s.replace("/private/var/", "/var/")
        .replace("/private/tmp/", "/tmp/")
}

// Contract 2 as amended on 2026-09-24 (section 5): the selected binary is
// checked against the repository's pin. One test per acceptance obligation.
// ---------------------------------------------------------------------------

/// What a versioned stub answers to a bare `config show`, the probe a
/// non-exact pin is put to.
#[derive(Clone, Copy)]
enum Probe {
    /// The configuration loads: the binary satisfies the pin.
    Admits,
    /// spec-spine's own refusal at configuration load.
    RefusesThePin,
    /// Exit 3 for another reason: not a compatibility verdict.
    OtherFailure,
}

impl Fixture {
    /// A stub reporting `version`, which logs every invocation with its
    /// arguments to `calls`, and whose `check` answers stale unless
    /// `STUB_CHECK_EXIT=0` asks for fresh, so a test can tell `--version` from `check` and
    /// `compile` and can assert a binary was never invoked at all.
    fn versioned(&self, at: &Path, label: &str, version: &str, probe: Probe) {
        let calls = self.root.join("calls");
        let probe = match probe {
            Probe::Admits => "exit 0".to_string(),
            Probe::RefusesThePin => "echo 'config error: this repository requires spec-spine >=0.23, <0.24 (spec-spine.toml [meta] required_version)' >&2; exit 3".to_string(),
            Probe::OtherFailure => "echo 'config error: spec-spine.toml: parse error' >&2; exit 3".to_string(),
        };
        let body = format!(
            "#!/bin/sh\nprintf '%s %s\\n' '{label}' \"$*\" >> '{calls}'\n\
             case \"$1\" in --version) echo 'spec-spine {version}'; exit 0 ;; esac\n\
             case \"$1 $2\" in 'check --help') exit 0 ;; esac\n\
             case \" $* \" in\n\
             *' config show --json '*) printf '%s\\n' '{{\"layout\":{{\"derived_dir\":\"{DERIVED_DIR}\"}}}}'; exit 0 ;;\n\
             *' config show '*) {probe} ;;\n\
             *' check '*) if [ \"${{STUB_CHECK_EXIT:-2}}\" = 0 ]; then printf 'spec-registry: fresh\\ncodebase-index: fresh\\n'; exit 0; fi; printf 'spec-registry: fresh\\ncodebase-index: STALE\\n'; exit 2 ;;\n\
             esac\nexit 0\n",
            calls = calls.display(),
        );
        executable(at, &body);
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.root.join("calls")).unwrap_or_default()
    }

    /// Whether `label` was invoked with an argument list containing `verb`.
    fn invoked(&self, label: &str, verb: &str) -> bool {
        self.calls().lines().any(|l| {
            l.strip_prefix(label)
                .and_then(|rest| rest.strip_prefix(' '))
                .is_some_and(|args| args.split_whitespace().any(|a| a == verb))
        })
    }

    fn never_invoked(&self, label: &str) -> bool {
        !self
            .calls()
            .lines()
            .any(|l| l.split(' ').next() == Some(label))
    }

    fn repo_build(&self) -> PathBuf {
        self.root.join("target/release/spec-spine")
    }

    fn on_path(&self) -> PathBuf {
        self.path_dir.join("spec-spine")
    }

    /// Run any of the four hooks the way its event runs it: a session hook
    /// against the project, post-edit on a spec edit, pre-bash on a
    /// pull-request create.
    fn run_any(&self, file: &str, env: &[(&str, &str)]) -> Output {
        match file {
            POST_EDIT => {
                let spec = self.root.join("specs/000-x/spec.md");
                self.run_payload(POST_EDIT, &edit_payload(&spec), env)
            }
            PRE_BASH => {
                // The gate's green path: `check` answers fresh, so the
                // verdict measured is the resolver's. The advisory hooks get
                // a stale answer instead, because the Stop hook is silent on
                // a fresh tree and every verdict line is asserted below.
                let mut env = env.to_vec();
                env.push(("SPEC_SPINE_DEFAULT_BRANCH", "main"));
                env.push(("STUB_CHECK_EXIT", "0"));
                let payload = bash_payload("gh pr create --title x --body y", &self.root);
                self.run_payload(PRE_BASH, &payload, &env)
            }
            _ => self.run_project(file, env),
        }
    }
}

/// A pinned fixture whose tree is committed, so the pull-request gate's
/// derived-tree reads find a clean tree and the verdict is the resolver's.
fn pinned_fixture(pin: Option<&str>) -> Fixture {
    let fixture = Fixture::pinned(pin);
    fixture.write_derived("spec-registry.json", "{}\n");
    fixture.commit_everything("the committed tree");
    fixture
}

/// Obligations 1 and 11: an incompatible repository build is passed over and
/// named, the compatible binary on `PATH` judges, and the verdict names it.
#[test]
fn pin_an_incompatible_repository_build_is_passed_over_and_named() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        fixture.versioned(&fixture.repo_build(), "repo", "0.24.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let out = fixture.run_any(file, &[]);
        let seen = norm(&text(&out));
        assert!(
            out.status.success(),
            "{file} refused though a compatible binary exists: {seen}"
        );
        assert!(
            fixture.invoked("path", "check"),
            "{file}: the compatible binary did not judge: {}",
            fixture.calls()
        );
        assert!(
            !fixture.invoked("repo", "check") && !fixture.invoked("repo", "compile"),
            "{file}: the incompatible build judged: {}",
            fixture.calls()
        );
        let repo = norm(&fixture.repo_build().display().to_string());
        assert!(
            seen.contains(&format!("passed over {repo}"))
                && seen.contains("0.24.0")
                && seen.contains("=0.23.0"),
            "{file} did not name the build it passed over: {seen}"
        );
        // Obligation 11: the verdict names path, version, rule and pin.
        let path = norm(&fixture.on_path().display().to_string());
        assert!(
            seen.contains(&format!("judged by {path} (0.23.0, PATH; pin =0.23.0 from")),
            "{file}'s verdict does not name its judge: {seen}"
        );
    }
}

fn assert_not_performed(file: &str, out: &Output) {
    let seen = text(out);
    if file == PRE_BASH {
        assert!(
            !out.status.success(),
            "the gate allowed the operation with no admitted binary: {seen}"
        );
    } else {
        assert!(out.status.success(), "{file} blocked: {seen}");
    }
    assert!(
        seen.contains("NOT PERFORMED"),
        "{file} did not report the check as not performed: {seen}"
    );
}

/// Obligations 2 and 11: an incompatible override refuses without falling
/// back, and says which override, what it reports, the pin and both remedies.
#[test]
fn pin_an_incompatible_override_refuses_without_fallback() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        let named = fixture.root.join("named-spec-spine");
        fixture.versioned(&named, "named", "0.24.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let named_s = named.display().to_string();
        let out = fixture.run_any(file, &[("STATECRAFT_SPEC_SPINE", &named_s)]);
        assert_not_performed(file, &out);
        assert!(
            fixture.never_invoked("path"),
            "{file} fell back past the override: {}",
            fixture.calls()
        );
        assert!(!fixture.invoked("named", "check"));
        let seen = norm(&text(&out));
        for needle in [
            &format!("STATECRAFT_SPEC_SPINE={}", norm(&named_s)) as &str,
            "reports 0.24.0",
            "=0.23.0",
            "Unset STATECRAFT_SPEC_SPINE",
            "point it at a binary that satisfies the pin",
        ] {
            assert!(seen.contains(needle), "{file} omits {needle:?}: {seen}");
        }
    }
}

/// Obligation 3: an override naming no executable refuses; never a silent
/// skip to the next rule.
#[test]
fn pin_an_override_naming_no_executable_refuses() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let missing = fixture
            .root
            .join("no-such-spec-spine")
            .display()
            .to_string();
        let out = fixture.run_any(file, &[("STATECRAFT_SPEC_SPINE", &missing)]);
        assert_not_performed(file, &out);
        assert!(fixture.never_invoked("path"), "{file}: {}", fixture.calls());
        assert!(
            norm(&text(&out)).contains("names no executable")
                && norm(&text(&out)).contains(&norm(&missing)),
            "{file}: {}",
            text(&out)
        );
    }
}

/// Obligation 4: candidates present, none compatible: the gate refuses, the
/// advisory hooks say not performed, and every candidate is listed.
#[test]
fn pin_no_compatible_candidate_is_not_performed_and_lists_them() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        fixture.versioned(&fixture.repo_build(), "repo", "0.24.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.22.0", Probe::Admits);
        let out = fixture.run_any(file, &[]);
        assert_not_performed(file, &out);
        let seen = norm(&text(&out));
        for (p, v) in [
            (fixture.repo_build(), "0.24.0"),
            (fixture.on_path(), "0.22.0"),
        ] {
            assert!(
                seen.contains(&format!("passed over {}", norm(&p.display().to_string())))
                    && seen.contains(v),
                "{file} did not list {}: {seen}",
                p.display()
            );
        }
        assert!(!fixture.invoked("repo", "check") && !fixture.invoked("path", "check"));
    }
}

/// Obligations 5 and 11: an unpinned repository is judged by the first
/// candidate, says so on every verdict line, and the post-edit hook withholds
/// its compile while the check still runs.
#[test]
fn pin_an_unpinned_repository_says_so_and_withholds_the_compile() {
    for file in ALL {
        let fixture = pinned_fixture(None);
        fixture.versioned(&fixture.repo_build(), "repo", "0.24.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let out = fixture.run_any(file, &[]);
        let seen = norm(&text(&out));
        assert!(
            fixture.invoked("repo", "check"),
            "{file}: {}",
            fixture.calls()
        );
        assert!(fixture.never_invoked("path"), "{file}: {}", fixture.calls());
        let repo = norm(&fixture.repo_build().display().to_string());
        assert!(
            seen.contains(&format!(
                "judged by {repo} (0.24.0, repository build; unpinned)"
            )),
            "{file}'s verdict does not say unpinned: {seen}"
        );
        if file == POST_EDIT {
            assert!(
                !fixture.invoked("repo", "compile"),
                "an unpinned repository's shards were rewritten: {}",
                fixture.calls()
            );
            assert!(seen.contains("compile WITHHELD"), "{seen}");
        }
    }
}

/// Obligation 6: a pinned repository with a compatible binary keeps contract
/// 1's sanctioned compile.
#[test]
fn pin_a_compatible_binary_keeps_the_sanctioned_compile() {
    let fixture = pinned_fixture(Some("=0.23.0"));
    fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
    let out = fixture.run_any(POST_EDIT, &[]);
    assert!(fixture.invoked("path", "compile"), "{}", text(&out));
    assert!(
        norm(&text(&out)).contains("recompiled after spec edit"),
        "{}",
        text(&out)
    );
}

/// Obligation 7: a non-exact requirement is put to the binary, and one the
/// probe refuses is passed over for the next.
#[test]
fn pin_a_non_exact_requirement_is_decided_by_the_probe() {
    for file in ALL {
        let fixture = pinned_fixture(Some(">=0.23, <0.24"));
        fixture.versioned(
            &fixture.repo_build(),
            "repo",
            "0.24.0",
            Probe::RefusesThePin,
        );
        fixture.versioned(&fixture.on_path(), "path", "0.23.5", Probe::Admits);
        let out = fixture.run_any(file, &[]);
        assert!(out.status.success(), "{file}: {}", text(&out));
        assert!(fixture.invoked("repo", "config"), "{file}: no probe ran");
        assert!(
            fixture.invoked("path", "check"),
            "{file}: {}",
            fixture.calls()
        );
        assert!(
            !fixture.invoked("repo", "check"),
            "{file}: {}",
            fixture.calls()
        );
        assert!(
            norm(&text(&out)).contains("passed over"),
            "{file}: {}",
            text(&out)
        );
    }
}

/// Obligation 8: a probe that fails for another reason is not a verdict, and
/// no later candidate is tried.
#[test]
fn pin_a_probe_that_fails_otherwise_is_not_performed() {
    for file in ALL {
        let fixture = pinned_fixture(Some(">=0.23, <0.24"));
        fixture.versioned(&fixture.repo_build(), "repo", "0.23.1", Probe::OtherFailure);
        fixture.versioned(&fixture.on_path(), "path", "0.23.5", Probe::Admits);
        let out = fixture.run_any(file, &[]);
        assert_not_performed(file, &out);
        assert!(fixture.never_invoked("path"), "{file}: {}", fixture.calls());
        assert!(
            norm(&text(&out)).contains("probe failed"),
            "{file}: {}",
            text(&out)
        );
    }
}

/// Obligations 9 and 11: in a managed session the supervisor's binary is the
/// only candidate; one that is not executable refuses as rule 2 does.
#[test]
fn pin_the_supervisors_binary_is_the_only_candidate() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        let supervised = fixture.root.join("supervised-spec-spine");
        let named = fixture.root.join("named-spec-spine");
        fixture.versioned(&supervised, "supervisor", "0.23.0", Probe::Admits);
        fixture.versioned(&named, "named", "0.23.0", Probe::Admits);
        fixture.versioned(&fixture.repo_build(), "repo", "0.23.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let sup = supervised.display().to_string();
        let sup_n = norm(&sup);
        let named_s = named.display().to_string();
        let env = [
            ("STATECRAFT_SPEC_SPINE", sup.as_str()),
            ("STATECRAFT_RUN_ID", "003-x"),
            ("SPEC_SPINE_BIN", named_s.as_str()),
        ];
        let out = fixture.run_any(file, &env);
        assert!(out.status.success(), "{file}: {}", text(&out));
        assert!(
            fixture.invoked("supervisor", "check"),
            "{file}: {}",
            fixture.calls()
        );
        for other in ["named", "repo", "path"] {
            assert!(
                fixture.never_invoked(other),
                "{file} invoked {other}: {}",
                fixture.calls()
            );
        }
        assert!(
            norm(&text(&out)).contains(&format!(
                "judged by {sup_n} (0.23.0, supervisor; pin =0.23.0"
            )),
            "{file}: {}",
            text(&out)
        );
        assert!(
            !norm(&text(&out)).contains("identity not verified"),
            "{}",
            text(&out)
        );

        // Not executable: refused, nothing else consulted.
        let fixture = pinned_fixture(Some("=0.23.0"));
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let gone = fixture.root.join("gone").display().to_string();
        let out = fixture.run_any(
            file,
            &[
                ("STATECRAFT_SPEC_SPINE", gone.as_str()),
                ("STATECRAFT_RUN_ID", "003-x"),
            ],
        );
        assert_not_performed(file, &out);
        assert!(fixture.never_invoked("path"), "{file}: {}", fixture.calls());
        assert!(
            norm(&text(&out)).contains("STATECRAFT_SPEC_SPINE"),
            "{}",
            text(&out)
        );
    }
}

/// Obligation 10: a managed session without the supervisor's path is resolved
/// by rules 1 to 4 and says its identity was not verified.
#[test]
fn pin_a_managed_session_without_the_supervisors_path_is_only_version_checked() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let out = fixture.run_any(file, &[("STATECRAFT_RUN_ID", "003-x")]);
        assert!(out.status.success(), "{file}: {}", text(&out));
        assert!(fixture.invoked("path", "check"));
        assert!(
            norm(&text(&out)).contains("version-checked, identity not verified"),
            "{file}: {}",
            text(&out)
        );
    }
}

// ---------------------------------------------------------------------------
// One variable selects the binary (spec 002 section 5, 2026-09-25).
// ---------------------------------------------------------------------------

/// The retired name selects nothing: with only `SPEC_SPINE_BIN` set, no hook
/// invokes the binary it names, the convention candidate judges, and each hook
/// says the old name was ignored and names the new one.
#[test]
fn one_variable_the_retired_name_is_reported_and_never_selects() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        let named = fixture.root.join("named-spec-spine");
        fixture.versioned(&named, "named", "0.23.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let named_s = named.display().to_string();
        let out = fixture.run_any(file, &[("SPEC_SPINE_BIN", &named_s)]);
        assert!(out.status.success(), "{file}: {}", text(&out));
        assert!(
            fixture.never_invoked("named"),
            "{file} selected by the retired name: {}",
            fixture.calls()
        );
        assert!(
            fixture.invoked("path", "check"),
            "{file}: {}",
            fixture.calls()
        );
        let seen = norm(&text(&out));
        assert!(
            seen.contains(&format!("ignored SPEC_SPINE_BIN={}", norm(&named_s)))
                && seen.contains("set STATECRAFT_SPEC_SPINE"),
            "{file} did not report the retired name as ignored: {seen}"
        );
        // One complete line, once, and no empty prefixed line after it.
        let notices: Vec<&str> = seen
            .lines()
            .filter(|l| l.contains("ignored SPEC_SPINE_BIN="))
            .collect();
        assert_eq!(notices.len(), 1, "{file}: {seen}");
        assert!(
            notices[0].ends_with("set STATECRAFT_SPEC_SPINE to choose the binary"),
            "{file}: {seen}"
        );
        assert!(
            !seen
                .lines()
                .any(|l| l.starts_with('[') && l.trim_end().ends_with(']')),
            "{file} printed an empty prefixed line: {seen}"
        );
    }
}

/// With both names set outside a managed session, the new one is the override
/// and the old one is neither selected nor reported.
#[test]
fn one_variable_the_new_name_overrides_and_the_old_one_is_silent() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        let chosen = fixture.root.join("chosen-spec-spine");
        let named = fixture.root.join("named-spec-spine");
        fixture.versioned(&chosen, "chosen", "0.23.0", Probe::Admits);
        fixture.versioned(&named, "named", "0.23.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let chosen_s = chosen.display().to_string();
        let named_s = named.display().to_string();
        let out = fixture.run_any(
            file,
            &[
                ("STATECRAFT_SPEC_SPINE", chosen_s.as_str()),
                ("SPEC_SPINE_BIN", named_s.as_str()),
            ],
        );
        assert!(out.status.success(), "{file}: {}", text(&out));
        assert!(
            fixture.invoked("chosen", "check"),
            "{file}: {}",
            fixture.calls()
        );
        for other in ["named", "path"] {
            assert!(
                fixture.never_invoked(other),
                "{file} invoked {other}: {}",
                fixture.calls()
            );
        }
        let seen = norm(&text(&out));
        assert!(
            seen.contains(&format!(
                "judged by {} (0.23.0, override; pin =0.23.0",
                norm(&chosen_s)
            )),
            "{file}: {seen}"
        );
        assert!(!seen.contains("ignored SPEC_SPINE_BIN"), "{file}: {seen}");
    }
}

/// Outside a managed session the override is put to the pin, which the
/// supervisor's path is not: the same incompatible binary that a managed
/// session uses as the supervisor's is refused as an operator's override.
#[test]
fn one_variable_outside_a_managed_session_the_value_is_an_override() {
    for file in ALL {
        let fixture = pinned_fixture(Some("=0.23.0"));
        let named = fixture.root.join("named-spec-spine");
        fixture.versioned(&named, "named", "0.24.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let named_s = named.display().to_string();

        let out = fixture.run_any(file, &[("STATECRAFT_SPEC_SPINE", &named_s)]);
        assert_not_performed(file, &out);
        assert!(fixture.never_invoked("path"), "{file}: {}", fixture.calls());
        assert!(
            norm(&text(&out)).contains("the override STATECRAFT_SPEC_SPINE="),
            "{file}: {}",
            text(&out)
        );

        let fixture = pinned_fixture(Some("=0.23.0"));
        let named = fixture.root.join("named-spec-spine");
        fixture.versioned(&named, "named", "0.24.0", Probe::Admits);
        fixture.versioned(&fixture.on_path(), "path", "0.23.0", Probe::Admits);
        let named_s = named.display().to_string();
        let out = fixture.run_any(
            file,
            &[
                ("STATECRAFT_SPEC_SPINE", named_s.as_str()),
                ("STATECRAFT_RUN_ID", "003-x"),
            ],
        );
        assert!(out.status.success(), "{file}: {}", text(&out));
        assert!(
            fixture.invoked("named", "check"),
            "{file}: {}",
            fixture.calls()
        );
        assert!(fixture.never_invoked("path"), "{file}: {}", fixture.calls());
        assert!(
            norm(&text(&out)).contains("supervisor;"),
            "{file}: {}",
            text(&out)
        );
    }
}
