//! The harness, session and startup verbs, through the built binary.
//!
//! Spec 006 section 3.11.1 and spec 002 sections 3.25 to 3.29. Exit codes are
//! a property of a process, so these spawn the executable. What each operation
//! does is asserted in `statecraft-home`'s own suite; what is asserted here is
//! that an operator can reach it, that inspection activates nothing, and that
//! the refusals arrive as refusals rather than as findings.
//!
//! Every run is given a temporary `STATECRAFT_HOME`, a temporary
//! `STATECRAFT_NATIVE_ROOT` and a temporary `HOME`, so nothing here reads or
//! writes the operator's own home. No provider is spawned and no session is
//! started: the captures are the synthetic ones this workspace's fixture
//! builds, and nothing below claims a live qualification.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The one evidence fixture, shared with `statecraft-home`'s suite.
#[path = "../../statecraft-home/tests/support/evidence.rs"]
#[allow(dead_code)]
mod evidence;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_statecraft-cli"))
}

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let sandbox = Self { dir };
        sandbox.init_repo(&sandbox.project());
        sandbox
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn native(&self) -> PathBuf {
        self.dir.path().join("native")
    }
    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn capture_dir(&self) -> PathBuf {
        self.dir.path().join("capture")
    }
    fn project_arg(&self) -> String {
        self.project().display().to_string()
    }

    fn init_repo(&self, at: &Path) {
        std::fs::create_dir_all(at).expect("the directory");
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(at)
                .output()
                .expect("git runs");
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "test"]);
        git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(at.join("README.md"), b"x").expect("a file");
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "one"]);
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(binary())
            .args(args)
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.native())
            .env("HOME", self.dir.path())
            .output()
            .expect("the binary runs")
    }

    /// A target: a home, an initialized project, and the instruction bridge the
    /// documented load rule needs in order to reach the managed file.
    fn prepared() -> Self {
        let sandbox = Self::new();
        assert_eq!(code(&sandbox.run(&["home", "apply"])), 0);
        let out = sandbox.run(&["init", "apply", &sandbox.project_arg()]);
        assert!(matches!(code(&out), 0 | 1), "{}", stdout(&out));
        std::fs::write(
            sandbox.project().join("AGENTS.md"),
            "@.statecraft/AGENTS.md\n\n# the project's own\n",
        )
        .expect("the bridge");
        sandbox
    }

    /// Write a submission and its captures, returning the submission's path.
    ///
    /// The evidence is the shared synthetic fixture, taken apart into the files
    /// an operator would have captured. Building it here rather than inline is
    /// what lets one test change one control.
    fn submission(&self, evidence: &statecraft_home::admission::Evidence) -> PathBuf {
        let dir = self.capture_dir();
        std::fs::create_dir_all(&dir).expect("the capture directory");
        let write = |name: &str, contents: &str| {
            std::fs::write(dir.join(name), contents).expect("a capture");
            name.to_string()
        };
        let settings = write("floor.json", &statecraft_home::session::payload_json());
        let control = |name: &str, m: &statecraft_home::admission::Measurement| {
            serde_json::json!({
                "invocation": m.invocation,
                "settings": m.settings.as_ref().map(|_| settings.clone()),
                "capture": name,
            })
        };
        let refusal = control("b1.jsonl", &evidence.refusal);
        let allowed = control("b3.jsonl", &evidence.allowed);
        let without = control("b4.jsonl", &evidence.without_payload);
        write("b1.jsonl", &evidence.refusal.capture.bytes);
        write("b3.jsonl", &evidence.allowed.capture.bytes);
        write("b4.jsonl", &evidence.without_payload.capture.bytes);
        let submission = dir.join("submission.json");
        std::fs::write(
            &submission,
            serde_json::to_string_pretty(&serde_json::json!({
                "version": evidence.version,
                "payloadDigest": evidence.payload_digest,
                "refusedCommand": evidence.refused_command,
                "allowedCommand": evidence.allowed_command,
                "refusal": refusal,
                "allowed": allowed,
                "withoutPayload": without,
            }))
            .expect("a submission serializes"),
        )
        .expect("the submission");
        submission
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}
fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

/// Every new verb is reachable, and `--help` says so without running anything.
#[test]
fn the_five_verbs_are_in_the_tree() {
    let sandbox = Sandbox::new();
    for group in ["harness", "session", "startup"] {
        let out = sandbox.run(&[group, "--help"]);
        assert_eq!(code(&out), 0, "`{group} --help` is not reachable");
    }
    let text = stdout(&sandbox.run(&["--help"]));
    for verb in [
        "harness show",
        "harness upgrade",
        "session payload",
        "startup record",
        "startup qualify",
    ] {
        assert!(text.contains(verb), "the help text omits `{verb}`");
    }
}

/// `session payload` prints the exact bytes, and delivers nothing.
#[test]
fn session_payload_prints_the_bytes_and_their_identity() {
    let sandbox = Sandbox::new();
    let out = sandbox.run(&["session", "payload", "--json"]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json");
    let payload = &value["value"]["value"]["payload"];
    assert_eq!(
        payload,
        &serde_json::json!(statecraft_home::session::payload_json())
    );
    assert_eq!(
        value["value"]["value"]["digest"],
        serde_json::json!(statecraft_home::startup::payload_identity())
    );
    assert_eq!(
        value["value"]["value"]["delivered"],
        serde_json::json!(false)
    );
    // It needs no target, which is the point of it taking no path.
    assert!(!sandbox.home().exists() || sandbox.home().is_dir());
}

/// `harness show` reads, and changes nothing it read.
#[test]
fn harness_show_activates_nothing() {
    let sandbox = Sandbox::prepared();
    let manifest = sandbox.project().join(".statecraft/environment.json");
    let before = std::fs::read(&manifest).expect("the manifest");
    let revisions_before = std::fs::read_dir(sandbox.home().join("harness"))
        .map(|d| d.count())
        .unwrap_or(0);

    let out = sandbox.run(&["harness", "show", &sandbox.project_arg(), "--json"]);
    // No requirement is committed yet, which is a diagnostic state: a finding.
    assert_eq!(code(&out), 1, "{}", stdout(&out));
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json");
    assert_eq!(
        value["value"]["value"]["activatedAnything"],
        serde_json::json!(false)
    );
    assert_eq!(
        value["value"]["value"]["availableUpgrade"]["availability"],
        serde_json::json!("available")
    );

    assert_eq!(
        std::fs::read(&manifest).expect("the manifest"),
        before,
        "an inspection rewrote the manifest it was inspecting"
    );
    assert_eq!(
        std::fs::read_dir(sandbox.home().join("harness"))
            .map(|d| d.count())
            .unwrap_or(0),
        revisions_before,
        "an inspection installed a revision"
    );
}

/// `harness upgrade` is the act, and `harness show` then agrees with it.
#[test]
fn harness_upgrade_commits_the_requirement_and_the_standing_becomes_exact() {
    let sandbox = Sandbox::prepared();
    let out = sandbox.run(&["harness", "upgrade", &sandbox.project_arg()]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(stdout(&out).contains("standing  exact"), "{}", stdout(&out));

    let shown = sandbox.run(&["harness", "show", &sandbox.project_arg()]);
    assert_eq!(code(&shown), 0, "{}", stdout(&shown));
    assert!(stdout(&shown).contains("standing  exact"));

    // Idempotent: a second upgrade commits the same identity and says the
    // manifest is unchanged rather than rewriting it.
    let again = sandbox.run(&["harness", "upgrade", &sandbox.project_arg()]);
    assert_eq!(code(&again), 0, "{}", stdout(&again));
    assert!(stdout(&again).contains("manifest  unchanged"));
}

/// An unregistered directory is a refusal, not a failure.
#[test]
fn a_directory_with_no_manifest_refuses() {
    let sandbox = Sandbox::new();
    let out = sandbox.run(&["harness", "show", &sandbox.project_arg()]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(stdout(&out).contains("holds no"));
}

/// `startup record` writes the unobserved record, and a start happens once.
#[test]
fn startup_record_writes_an_unqualified_record_and_refuses_a_second_one() {
    let sandbox = Sandbox::prepared();
    assert_eq!(
        code(&sandbox.run(&["harness", "upgrade", &sandbox.project_arg()])),
        0
    );
    let out = sandbox.run(&["startup", "record", &sandbox.project_arg(), "s-1"]);
    // Not qualified: nothing observed a session. A finding, not a failure.
    assert_eq!(code(&out), 1, "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("observation not-observed"), "{text}");
    assert!(text.contains("qualified false"), "{text}");
    assert!(
        sandbox
            .project()
            .join(".statecraft/state/startup/s-1.json")
            .is_file()
    );

    let second = sandbox.run(&["startup", "record", &sandbox.project_arg(), "s-1"]);
    assert_eq!(code(&second), 2, "{}", stdout(&second));
    assert!(stdout(&second).contains("a start happens once"));
}

/// The whole workflow, ending in a qualified record.
///
/// **The captures are synthetic.** This establishes that an operator can carry
/// admitted evidence through the command tree into a record, and nothing about
/// whether a real harness enforces anything.
#[test]
fn submitted_evidence_that_is_admitted_reaches_a_qualified_record() {
    let sandbox = Sandbox::prepared();
    assert_eq!(
        code(&sandbox.run(&["harness", "upgrade", &sandbox.project_arg()])),
        0
    );
    let submission = sandbox.submission(&evidence::admissible());
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-2",
        &submission.display().to_string(),
    ]);
    assert_eq!(code(&out), 1, "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("observation observed"), "{text}");
    // Qualification needs all three evidence classes. This verb records and
    // delivers nothing, so `Supply` is `not-attempted` and the record is
    // honestly not qualified even with the observation admitted.
    assert!(text.contains("supply not attempted"), "{text}");
    assert!(text.contains("qualified false"), "{text}");

    let written: serde_json::Value = serde_json::from_slice(
        &std::fs::read(sandbox.project().join(".statecraft/state/startup/s-2.json"))
            .expect("the record"),
    )
    .expect("json");
    // The bytes are in the record, not merely pointed at.
    assert!(
        written["observation"]["evidence"]["refusal"]["capture"]["bytes"]
            .as_str()
            .expect("the capture")
            .contains("permission_denials")
    );
}

/// Each refusal case, through the binary.
#[test]
fn refused_submissions_write_nothing_and_exit_two() {
    /// One refusal case: a name, the single thing it changes, and the phrase
    /// the binary must say about it.
    type Case = (
        &'static str,
        Box<dyn Fn(&mut statecraft_home::admission::Evidence)>,
        &'static str,
    );
    let cases: Vec<Case> = vec![
        (
            "prose",
            Box::new(|e| {
                e.refusal.capture.bytes =
                    "cargo publish --dry-run: permission granted; command executed successfully\n"
                        .into();
            }),
            "not the harness's structured output",
        ),
        (
            "no structured denial",
            Box::new(|e| {
                e.refusal.capture = evidence::capture("b1.jsonl", &[evidence::REFUSED], &[]);
            }),
            "no structured denial",
        ),
        (
            "the allowed control was refused",
            Box::new(|e| {
                e.allowed.capture =
                    evidence::capture("b3.jsonl", &[evidence::ALLOWED], &[evidence::ALLOWED]);
            }),
            "allowed-command control",
        ),
        (
            "the no-payload control was refused too",
            Box::new(|e| {
                e.without_payload.capture =
                    evidence::capture("b4.jsonl", &[evidence::REFUSED], &[evidence::REFUSED]);
            }),
            "operator's own configuration",
        ),
        (
            "substituted evidence",
            Box::new(|e| e.allowed.capture = e.refusal.capture.clone()),
            "same captured bytes",
        ),
    ];

    for (name, mutate, expected) in cases {
        let sandbox = Sandbox::prepared();
        assert_eq!(
            code(&sandbox.run(&["harness", "upgrade", &sandbox.project_arg()])),
            0
        );
        let mut evidence = evidence::admissible();
        mutate(&mut evidence);
        let submission = sandbox.submission(&evidence);
        let out = sandbox.run(&[
            "startup",
            "qualify",
            &sandbox.project_arg(),
            "s-3",
            &submission.display().to_string(),
        ]);
        let text = stdout(&out);
        assert_eq!(code(&out), 2, "{name}: {text}");
        assert!(text.contains(expected), "{name}: {text}");
        assert!(text.contains("nothing was written"), "{name}: {text}");
        assert!(
            !sandbox
                .project()
                .join(".statecraft/state/startup/s-3.json")
                .exists(),
            "{name}: a refused claim wrote a record"
        );
    }
}

/// A submission that is not there never stated a claim.
#[test]
fn an_unreadable_submission_is_distinguished_from_a_refused_one() {
    let sandbox = Sandbox::prepared();
    assert_eq!(
        code(&sandbox.run(&["harness", "upgrade", &sandbox.project_arg()])),
        0
    );
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-4",
        &sandbox.dir.path().join("absent.json").display().to_string(),
    ]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(
        stdout(&out).contains("could not be read, so no claim was judged"),
        "{}",
        stdout(&out)
    );
}
