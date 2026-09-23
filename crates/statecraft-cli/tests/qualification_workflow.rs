//! The harness, session and startup verbs, through the built binary.
//!
//! Spec 006 sections 3.11.1 and 3.11.2, and spec 002 sections 3.25 to 3.30.
//! Exit codes are a property of a process, so these spawn the executable. What
//! each operation decides is asserted in `statecraft-home`'s own suite; what is
//! asserted here is that an operator can reach it, that inspection activates
//! nothing, that the launch and the admission meet through the real command
//! tree, and that refusals, findings and failures arrive as what they are.
//!
//! Every run is given a temporary `STATECRAFT_HOME`, a temporary
//! `STATECRAFT_NATIVE_ROOT` and a temporary `HOME`, so nothing here reads or
//! writes the operator's own home. **No provider is spawned.** The launches run
//! `tests/fixtures/fake-provider.sh`, a local fake, with `--synthetic`, and the
//! hand-built captures are the shared synthetic fixture. Nothing below claims a
//! live qualification, and the records it produces say synthetic.

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

    /// Write the three capture records a set of launches would have written,
    /// returning the capture directory.
    ///
    /// The evidence is the shared synthetic fixture, relocated into this
    /// sandbox's project so it names the project being qualified. Building it
    /// here rather than inline is what lets one test change one control.
    fn captures(&self, evidence: &statecraft_home::admission::Evidence) -> PathBuf {
        let dir = self.capture_dir();
        std::fs::create_dir_all(&dir).expect("the capture directory");
        let project = self.project().canonicalize().expect("the project");
        let project = project.display().to_string();
        let controls = [
            (
                statecraft_home::admission::Control::Refusal,
                &evidence.refusal,
            ),
            (
                statecraft_home::admission::Control::Allowed,
                &evidence.allowed,
            ),
            (
                statecraft_home::admission::Control::WithoutPayload,
                &evidence.without_payload,
            ),
        ];
        for (control, m) in controls {
            let mut m = m.clone();
            m.invocation.working_directory = project.clone();
            let bytes = m.capture.bytes.replace(
                &format!("\"cwd\":\"{}\"", evidence::CWD),
                &format!("\"cwd\":{}", serde_json::json!(project)),
            );
            evidence::set_capture(&mut m, bytes);
            std::fs::write(
                dir.join(statecraft_home::admission::record_name(control)),
                serde_json::to_string_pretty(&m).expect("a record serializes"),
            )
            .expect("a record");
        }
        dir
    }

    /// Launch one control against the local fake, through the binary.
    fn capture(&self, control: &str, mode: &str, extra: &[&str]) -> Output {
        let fake = fake_provider();
        let dir = self.capture_dir().display().to_string();
        let mut args = vec![
            "startup",
            "capture",
            &self.project_arg(),
            control,
            &dir,
            "--program",
            &fake,
            "--synthetic",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        args.extend(extra.iter().map(|a| a.to_string()));
        Command::new(binary())
            .args(&args)
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.native())
            .env("HOME", self.dir.path())
            .env("FAKE_PROVIDER_MODE", mode)
            .output()
            .expect("the binary runs")
    }
}

/// The local fake provider. Never a provider.
fn fake_provider() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fake-provider.sh")
        .display()
        .to_string()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}
fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

/// Every new verb is reachable, and `--help` says so without running anything.
#[test]
fn the_six_verbs_are_in_the_tree() {
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
        "startup capture",
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

    // The human rendering is the bytes and nothing else, because this verb
    // exists so an operator can redirect it into the file the settings
    // argument will read. A rendering that appended a digest line would
    // produce a settings file that is not the payload, and the acceptance
    // script found exactly that.
    let human = sandbox.run(&["session", "payload"]);
    assert_eq!(code(&human), 0);
    assert_eq!(stdout(&human), statecraft_home::session::payload_json());
    let reparsed: serde_json::Value = serde_json::from_str(&stdout(&human))
        .expect("the human rendering is the settings document itself");
    assert!(reparsed["permissions"]["deny"].is_array());

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

    // Spec 002 section 3.31: this verb measures no harness revision, so the
    // record says nothing resolved. The required identity is not an observed
    // one, and recording it as the resolution was a false match.
    let record = statecraft_home::startup::StartupRecord::read(&sandbox.project(), "s-1")
        .unwrap()
        .unwrap();
    assert!(record.required_harness.is_some());
    assert_eq!(record.resolved_harness, None, "an unmeasured resolution");
    assert!(!record.standing.permits_managed_execution());
    assert!(text.contains("standing exact"), "{text}");

    let second = sandbox.run(&["startup", "record", &sandbox.project_arg(), "s-1"]);
    assert_eq!(code(&second), 2, "{}", stdout(&second));
    assert!(stdout(&second).contains("a start happens once"));
}

/// Prepare a target whose requirement is committed, so the standing is exact.
fn upgraded() -> Sandbox {
    let sandbox = Sandbox::prepared();
    assert_eq!(
        code(&sandbox.run(&["harness", "upgrade", &sandbox.project_arg()])),
        0
    );
    sandbox
}

/// The whole operator workflow through the command tree: three launches
/// against the local fake, then the admission, then the record, then the record
/// read back and judged again.
///
/// **Synthetic, and it says so.** What this establishes is that the launch, the
/// admission and the record meet through the real binary and that synthetic
/// evidence cannot become a qualification. Nothing about a real harness.
#[test]
fn three_launches_then_qualify_record_a_synthetic_observation_that_never_qualifies() {
    let sandbox = upgraded();
    for control in ["refusal", "allowed-command", "without-payload"] {
        let out = sandbox.capture(control, "faithful", &["--deadline", "60"]);
        assert_eq!(code(&out), 0, "{control}: {}", stdout(&out));
        let text = stdout(&out);
        assert!(text.contains("SYNTHETIC"), "{text}");
        assert!(text.contains("session   complete"), "{text}");
    }
    // Every file the launches wrote is outside the project, and nothing was
    // left in it.
    let dir = sandbox.capture_dir();
    for name in [
        "refusal.json",
        "refusal.stdout",
        "refusal.stderr",
        "refusal.settings.json",
        "allowed-command.json",
        "without-payload.json",
    ] {
        assert!(dir.join(name).is_file(), "{name}");
    }
    assert!(!dir.join("without-payload.settings.json").exists());

    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-2",
        &dir.display().to_string(),
    ]);
    // Admitted and recorded, and not qualified: synthetic evidence, and this
    // verb records no supply. A finding, exit 1.
    assert_eq!(code(&out), 1, "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("observation observed-synthetic"), "{text}");
    assert!(text.contains("supply not attempted"), "{text}");
    assert!(text.contains("qualified false"), "{text}");

    // The record keeps the bytes and the launch, and reading it back produces
    // the same judgement.
    let record = statecraft_home::startup::StartupRecord::read(&sandbox.project(), "s-2")
        .expect("the record reads")
        .expect("the record exists");
    assert!(record.observation.observed());
    assert!(record.observation.admitted().is_ok());
    assert!(record.observation.synthetic());
    assert!(!record.qualifies());
    let written: serde_json::Value = serde_json::from_slice(
        &std::fs::read(sandbox.project().join(".statecraft/state/startup/s-2.json"))
            .expect("the record"),
    )
    .expect("json");
    let refusal = &written["observation"]["evidence"]["refusal"];
    assert!(
        refusal["capture"]["bytes"]
            .as_str()
            .unwrap()
            .contains("permission_denials")
    );
    assert_eq!(refusal["launch"]["origin"], serde_json::json!("synthetic"));
    assert_eq!(refusal["launch"]["process"]["code"], serde_json::json!(1));
}

/// `startup capture --json` keeps the documented envelope, and the record in it
/// is the file that was written.
#[test]
fn a_capture_renders_its_record_in_the_json_envelope() {
    let sandbox = upgraded();
    let out = sandbox.capture("allowed-command", "faithful", &["--json"]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json");
    let captured = &value["value"];
    assert_eq!(captured["operation"], serde_json::json!("captured"));
    assert_eq!(captured["value"]["complete"], serde_json::json!(true));
    let on_disk: serde_json::Value = serde_json::from_slice(
        &std::fs::read(sandbox.capture_dir().join("allowed-command.json")).expect("the record"),
    )
    .expect("json");
    assert_eq!(captured["value"]["measurement"], on_disk);
    let args = on_disk["invocation"]["arguments"].as_array().unwrap();
    assert!(args.iter().any(|a| a == "--allowedTools"));
    assert!(!args.iter().any(|a| a.as_str().unwrap().contains("Use the")));
}

/// Launches that do not complete are recorded and are findings; the next
/// control should not be launched, and qualifying them is a refusal.
#[test]
fn an_incomplete_launch_is_recorded_as_a_finding_and_never_admitted() {
    for (mode, expected) in [
        ("startup-fails", "is empty"),
        ("request-only", ""),
        ("malformed", "not the harness's structured output"),
        ("conflicting", "more than one init"),
        ("hang", "the deadline ended it"),
        ("signal", "signal 15"),
    ] {
        let sandbox = upgraded();
        let out = sandbox.capture("refusal", mode, &["--deadline", "2"]);
        let text = stdout(&out);
        if mode == "request-only" {
            // One complete session: the launch completed, and it is the
            // admission that refuses a request with no result.
            assert_eq!(code(&out), 0, "{mode}: {text}");
            continue;
        }
        assert_eq!(code(&out), 1, "{mode}: {text}");
        assert!(text.contains("session   incomplete"), "{mode}: {text}");
        assert!(text.contains(expected), "{mode}: {text}");
        assert!(
            sandbox.capture_dir().join("refusal.json").is_file(),
            "{mode}"
        );
        if mode == "startup-fails" {
            assert!(text.contains("stderr    33 byte(s)"), "{text}");
            assert_eq!(
                std::fs::read_to_string(sandbox.capture_dir().join("refusal.stderr")).unwrap(),
                "error: the fake refuses to start\n"
            );
        }
    }

    // The request-only control, carried through qualification.
    let sandbox = upgraded();
    assert_eq!(code(&sandbox.capture("refusal", "faithful", &[])), 0);
    assert_eq!(
        code(&sandbox.capture("allowed-command", "request-only", &[])),
        0
    );
    assert_eq!(
        code(&sandbox.capture("without-payload", "faithful", &[])),
        0
    );
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-5",
        &sandbox.capture_dir().display().to_string(),
    ]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(
        stdout(&out).contains("the request has no result"),
        "{}",
        stdout(&out)
    );
    assert!(
        !sandbox
            .project()
            .join(".statecraft/state/startup/s-5.json")
            .exists()
    );
}

/// A fake that ignores the payload executes the refused command, and the claim
/// is refused as unverified: insufficient evidence is the answer, not a pass.
#[test]
fn a_provider_that_did_not_enforce_leaves_the_session_unverified() {
    let sandbox = upgraded();
    for control in ["refusal", "allowed-command", "without-payload"] {
        assert_eq!(code(&sandbox.capture(control, "ignores-settings", &[])), 0);
    }
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-6",
        &sandbox.capture_dir().display().to_string(),
    ]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(
        stdout(&out).contains("no structured denial"),
        "{}",
        stdout(&out)
    );
    assert!(stdout(&out).contains("unverified"), "{}", stdout(&out));
}

/// What `startup capture` refuses before launching anything.
#[test]
fn a_capture_refuses_before_launch_and_launches_nothing() {
    let sandbox = upgraded();
    let inside = sandbox.project().join("captures").display().to_string();
    let fake = fake_provider();
    let project = sandbox.project_arg();
    let run = |args: &[&str]| {
        let mut all = vec!["startup", "capture", project.as_str()];
        all.extend_from_slice(args);
        sandbox.run(&all)
    };
    let outside = sandbox.capture_dir().display().to_string();

    // Captures inside the project.
    let out = run(&["refusal", &inside, "--program", &fake, "--synthetic"]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(stdout(&out).contains("inside the project"));
    assert!(
        !sandbox.project().join("captures").exists(),
        "a refused launch created its capture directory inside the project"
    );

    // A program that is not there.
    let out = run(&["refusal", &outside, "--program", "/nonexistent/claude"]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(stdout(&out).contains("could not be resolved"));

    // The refused command's manifest directory exists, so the command would
    // not fail harmlessly.
    std::fs::create_dir(sandbox.project().join("statecraft-absent")).unwrap();
    let out = run(&["refusal", &outside, "--program", &fake, "--synthetic"]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(stdout(&out).contains("has to be absent"));
    std::fs::remove_dir(sandbox.project().join("statecraft-absent")).unwrap();

    // A launch happens once.
    assert_eq!(code(&sandbox.capture("refusal", "faithful", &[])), 0);
    let out = sandbox.capture("refusal", "faithful", &[]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(stdout(&out).contains("a launch happens once"));

    // Not a control, and not a deadline: usage errors.
    assert_eq!(code(&run(&["bogus", &outside])), 3);
    assert_eq!(code(&run(&["refusal", &outside, "--deadline", "0"])), 3);
    assert_eq!(code(&run(&["refusal", &outside, "--arguments", "x"])), 3);

    // No manifest: not a target.
    let bare = Sandbox::new();
    let out = bare.run(&[
        "startup",
        "capture",
        &bare.project_arg(),
        "refusal",
        &bare.capture_dir().display().to_string(),
        "--program",
        &fake,
    ]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(!bare.capture_dir().join("refusal.json").exists());
}

/// Each refusal case, through the binary, writes nothing and exits 2.
#[test]
fn refused_captures_write_nothing_and_exit_two() {
    type Case = (
        &'static str,
        Box<dyn Fn(&mut statecraft_home::admission::Evidence)>,
        &'static str,
    );
    let cases: Vec<Case> = vec![
        (
            "prose",
            Box::new(|e| {
                evidence::set_capture(
                    &mut e.refusal,
                    "cargo publish --dry-run: permission granted; command executed successfully\n"
                        .into(),
                );
            }),
            "not the harness's structured output",
        ),
        (
            "no structured denial",
            Box::new(|e| {
                evidence::set_events(
                    &mut e.refusal,
                    &[
                        evidence::init("session-r"),
                        evidence::request("session-r", "toolu_r1", "Bash", evidence::REFUSED),
                        evidence::execution("session-r", "toolu_r1", evidence::CARGO_FAILS, true),
                        evidence::capped("session-r", &[]),
                    ],
                );
            }),
            "no structured denial",
        ),
        (
            "a request with no result",
            Box::new(|e| {
                evidence::set_events(
                    &mut e.allowed,
                    &[
                        evidence::init("session-a"),
                        evidence::request("session-a", "toolu_a1", "Bash", evidence::ALLOWED),
                        evidence::capped("session-a", &[]),
                    ],
                );
            }),
            "the request has no result",
        ),
        (
            "substituted evidence",
            Box::new(|e| {
                let bytes = e.refusal.capture.bytes.clone();
                evidence::set_capture(&mut e.allowed, bytes);
            }),
            "same captured bytes",
        ),
        (
            "a record written before section 3.30",
            Box::new(|e| e.without_payload.launch = None),
            "carries no launch record",
        ),
    ];

    for (name, mutate, expected) in cases {
        let sandbox = upgraded();
        let mut evidence = evidence::admissible();
        mutate(&mut evidence);
        let dir = sandbox.captures(&evidence);
        let out = sandbox.run(&[
            "startup",
            "qualify",
            &sandbox.project_arg(),
            "s-3",
            &dir.display().to_string(),
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

    // The unmutated fixture, relocated here, is admitted: every case above is
    // one mutation from a claim that passes.
    let sandbox = upgraded();
    let dir = sandbox.captures(&evidence::admissible());
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-3",
        &dir.display().to_string(),
    ]);
    assert_eq!(code(&out), 1, "{}", stdout(&out));
}

/// Captures made in another project cannot qualify this one.
#[test]
fn captures_from_another_project_are_refused() {
    let sandbox = upgraded();
    let other = upgraded();
    let dir = other.captures(&evidence::admissible());
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-7",
        &dir.display().to_string(),
    ]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(
        stdout(&out).contains("cannot qualify this one"),
        "{}",
        stdout(&out)
    );
}

/// Captures that are not there never stated a claim: exit 4, not 2.
#[test]
fn unreadable_captures_are_a_failure_and_not_a_refused_claim() {
    let sandbox = upgraded();
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-4",
        &sandbox.dir.path().join("absent").display().to_string(),
    ]);
    assert_eq!(code(&out), 4, "{}", stdout(&out));
    assert!(stdout(&out).contains("could not be read, so no claim was judged"));

    // One of three missing.
    let dir = sandbox.captures(&evidence::admissible());
    std::fs::remove_file(dir.join("allowed-command.json")).unwrap();
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-4",
        &dir.display().to_string(),
    ]);
    assert_eq!(code(&out), 4, "{}", stdout(&out));
    assert!(stdout(&out).contains("allowed-command"), "{}", stdout(&out));

    // Not a capture record at all.
    std::fs::write(dir.join("allowed-command.json"), "{\"not\": \"a record\"}").unwrap();
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-4",
        &dir.display().to_string(),
    ]);
    assert_eq!(code(&out), 4, "{}", stdout(&out));
    assert!(
        !sandbox
            .project()
            .join(".statecraft/state/startup/s-4.json")
            .exists()
    );

    // And the JSON rendering says which of the two it was.
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-4",
        &dir.display().to_string(),
        "--json",
    ]);
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json");
    assert_eq!(value["value"]["value"]["kind"], serde_json::json!("unread"));
}

/// A session that already has a record is refused before any evidence is read.
#[test]
fn a_second_record_for_a_session_is_refused_before_the_evidence_is_read() {
    let sandbox = upgraded();
    assert_eq!(
        code(&sandbox.run(&["startup", "record", &sandbox.project_arg(), "s-8"])),
        1
    );
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-8",
        &sandbox.dir.path().join("absent").display().to_string(),
        "--json",
    ]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("json");
    assert_eq!(value["value"]["operation"], serde_json::json!("refused"));
}

/// Spec 002 section 3.34, through the binary: a capture ending with the one
/// allowlisted trailer is a complete session, the launch names the trailer,
/// and the three controls qualify exactly as they do without it. A trailer of
/// any other shape leaves the launch incomplete.
///
/// **Synthetic, and it says so.** The fake writes the measured shape; nothing
/// here is a live observation.
#[test]
fn a_trailing_task_summary_is_admitted_through_the_binary_and_nothing_else_is() {
    let sandbox = upgraded();
    for control in ["refusal", "allowed-command", "without-payload"] {
        let out = sandbox.capture(control, "trailer", &["--deadline", "60"]);
        assert_eq!(code(&out), 0, "{control}: {}", stdout(&out));
        let text = stdout(&out);
        assert!(text.contains("session   complete"), "{text}");
        assert!(
            text.contains(": system/task_summary after the terminal event, admitted by section 3.34 and not read"),
            "{text}"
        );
    }
    let out = sandbox.run(&[
        "startup",
        "qualify",
        &sandbox.project_arg(),
        "s-trailer",
        &sandbox.capture_dir().display().to_string(),
    ]);
    assert_eq!(code(&out), 1, "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("observation observed-synthetic"), "{text}");
    assert_eq!(text.matches("trailer   ").count(), 3, "{text}");
    // The bytes are kept whole, trailer included.
    let written = std::fs::read_to_string(
        sandbox
            .project()
            .join(".statecraft/state/startup/s-trailer.json"),
    )
    .unwrap();
    assert!(written.contains("task_summary"), "the trailer was stripped");

    let sandbox = upgraded();
    let out = sandbox.capture("refusal", "bad-trailer", &["--deadline", "60"]);
    assert_eq!(code(&out), 1, "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("session   incomplete"), "{text}");
    assert!(text.contains("not the one trailer"), "{text}");
    assert!(!text.contains("trailer   "), "{text}");
}

/// Spec 002 section 3.37 rule 4: the settings file a capture hands the
/// provider is in that capture's exchange directory in the product home, not
/// in the directory the operator named, which holds none of the capture's
/// files while the provider runs and receives them, the settings file's copy
/// included, after it exits. The admission reads the path the provider was
/// given, so the capture still qualifies as before.
#[test]
fn a_captures_settings_file_is_in_the_exchange_directory_and_not_the_operators_while_it_runs() {
    let sandbox = upgraded();
    let dir = sandbox.capture_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let trace = sandbox.dir.path().join("trace");
    let out = Command::new(binary())
        .args([
            "startup",
            "capture",
            &sandbox.project_arg(),
            "refusal",
            &dir.display().to_string(),
            "--program",
            &fake_provider(),
            "--synthetic",
            "--json",
        ])
        .env("STATECRAFT_HOME", sandbox.home())
        .env("STATECRAFT_NATIVE_ROOT", sandbox.native())
        .env("HOME", sandbox.dir.path())
        .env("FAKE_PROVIDER_MODE", "faithful")
        .env("FAKE_TRACE", &trace)
        .env("FAKE_OPERATOR_DIR", &dir)
        .output()
        .expect("the binary runs");
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let trace = std::fs::read_to_string(&trace).expect("the fake traced its session");
    let given = trace
        .lines()
        .find_map(|l| l.strip_prefix("settings "))
        .expect("a settings argument");
    let given = Path::new(given);
    let exchange = statecraft_home::launch::Places::of(&sandbox.home(), &sandbox.project())
        .exchange
        .join("captures");
    assert!(
        given.starts_with(exchange.canonicalize().unwrap()),
        "{given:?} is not in {exchange:?}"
    );
    assert!(!given.starts_with(&dir) && !given.starts_with(dir.canonicalize().unwrap()));
    assert!(!given.starts_with(sandbox.project().canonicalize().unwrap()));
    // While the provider ran, the operator's directory held nothing of this
    // capture.
    assert!(
        !trace.lines().any(|l| l.starts_with("operator ")),
        "the operator's directory was written before the provider exited:\n{trace}"
    );
    // Afterwards it holds the records, the settings file's copy with them.
    for name in [
        "refusal.json",
        "refusal.stdout",
        "refusal.stderr",
        "refusal.settings.json",
    ] {
        assert!(dir.join(name).is_file(), "{name}");
    }
    assert_eq!(
        std::fs::read(dir.join("refusal.settings.json")).unwrap(),
        std::fs::read(given).unwrap()
    );
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("refusal.json")).unwrap()).unwrap();
    assert_eq!(
        Path::new(record["launch"]["settingsPath"].as_str().unwrap()),
        given
    );
    assert_eq!(
        record["launch"]["settingsDigestAfter"],
        statecraft_environment::digest::digest_bytes(&std::fs::read(given).unwrap())
    );
}
