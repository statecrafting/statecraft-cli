//! A-EQ-LAUNCH, and the halves of A-EQ-RUN, A-EQ-TRIAL, A-EQ-ACCEPT and
//! A-EQ-RECONCILE that need a launched attempt: a verb and its typed operation
//! each act on the same fixture, at the same paths, and must answer the same
//! envelope and leave the same bytes behind, attempt records and chain
//! linkage included.
//!
//! The verb runs as the built binary. The operation runs in a fresh process
//! too, this test binary re-entered at [`operation_in_child`], so both sides
//! start from the same process state: the fixed clock, and the launch nonces
//! and temporary names `STATECRAFT_TEST_NONCE_SEED` draws in sequence in a
//! debug build (spec 040 section 5, 2026-10-18). The re-entered process calls
//! the library operation and renders its answer through `render::Answer`;
//! nothing in it parses argv for the operation.
//!
//! Five values neither side chooses are compared by what they stand for, each
//! checked on its own side first (spec 040 section 5, 2026-10-18): the pid
//! the kernel gave the provider, which must be the pid the fake reported; the
//! wall-clock seconds git stamps a reflog line it adds with, which must fall
//! within that side's run; the stat cache of a git index, compared as the
//! entries it stages; the random name spec 004's adapter gives the settings
//! file, which only the fake's own report carries; and the random names of
//! the temporary files a killed launcher leaves behind. Everything else is
//! compared byte for byte, and nothing the product records is rewritten.

use super::AT;
use crate::fake_provider::{FAKE_PROVIDER, as_linked};
use crate::provider_token;
use statecraft_cli::application::{
    Context, ReconcileOperands, Response, TrialOperands, accept, run, run_reconcile, startup_trial,
};
use statecraft_cli::commands::Verb;
use statecraft_cli::render::Format;
use statecraft_environment::registry::Registry;
use statecraft_environment::time::FixedClock;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RUN: &str = "replay";
const SEED: &str = "040";
/// Set only in the re-entered process: the case it is to run.
const CHILD: &str = "STATECRAFT_EQUIVALENCE_CASE";

/// Held by each fixture from its creation to its removal: see [`Fixture`].
static LAUNCHING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One operation a case compares, as both sides are given it.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum Operation {
    Run,
    Trial,
    Accept,
    Reconcile {
        attempt: String,
        finding: String,
        inspected: String,
    },
}

impl Operation {
    fn verb(&self) -> Verb {
        match self {
            Operation::Run => Verb::Run,
            Operation::Trial => Verb::StartupTrial,
            Operation::Accept => Verb::Accept,
            Operation::Reconcile { .. } => Verb::RunReconcile,
        }
    }

    fn args(&self, target: &str) -> Vec<String> {
        let words: Vec<&str> = match self {
            Operation::Run => vec!["run", target, RUN],
            Operation::Trial => vec!["startup", "trial", target, "--synthetic"],
            Operation::Accept => vec!["accept", target, RUN],
            Operation::Reconcile {
                attempt,
                finding,
                inspected,
            } => vec![
                "run",
                "reconcile",
                target,
                RUN,
                attempt,
                finding,
                inspected,
                "sam",
                "a",
                "reason",
            ],
        };
        let mut args: Vec<String> = words.into_iter().map(str::to_string).collect();
        args.push("--json".into());
        args
    }

    fn call(&self, ctx: &Context<'_>, target: &Path) -> Result<Response, String> {
        let r = match self {
            Operation::Run => run(ctx, target, Some(RUN)),
            Operation::Trial => startup_trial(
                ctx,
                target,
                Some(TrialOperands {
                    stated: vec![statecraft_home::trial::Origin::Synthetic],
                    deadline: statecraft_home::trial::DEFAULT_DEADLINE_SECONDS,
                }),
            ),
            Operation::Accept => accept(ctx, target, Some(RUN)),
            Operation::Reconcile {
                attempt,
                finding,
                inspected,
            } => run_reconcile(
                ctx,
                &ReconcileOperands {
                    path: target,
                    run_id: RUN,
                    attempt,
                    finding,
                    inspected,
                    operator: "sam",
                    reason: "a reason",
                    evidence: &[],
                },
            ),
        };
        r.map_err(|u| u.0)
    }
}

/// What the re-entered process is told: the operation, the fixture, and where
/// to leave its answer.
#[derive(serde::Serialize, serde::Deserialize)]
struct Case {
    operation: Operation,
    home: PathBuf,
    target: PathBuf,
    answer: PathBuf,
}

/// The boundary self-test and every retest re-execute the current executable
/// as `__boundary_probe <json>` (spec 004), so the process an operation runs
/// in must answer that, as the binary's `main` does. libtest owns this
/// binary's `main` and prints before any test runs, so the answer is given
/// from a constructor, before `main`, and the process exits there.
#[used]
#[cfg_attr(target_os = "linux", unsafe(link_section = ".init_array"))]
#[cfg_attr(target_os = "macos", unsafe(link_section = "__DATA,__mod_init_func"))]
static BOUNDARY_PROBE: extern "C" fn() = boundary_probe;

extern "C" fn boundary_probe() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "__boundary_probe" {
        std::process::exit(statecraft_adapter::boundary::probe(&args[1]));
    }
}

/// The operation's side of a case, when this process was re-entered for one;
/// otherwise nothing.
#[test]
fn operation_in_child() {
    let Some(case) = std::env::var_os(CHILD) else {
        return;
    };
    let case: Case = serde_json::from_slice(&std::fs::read(case).unwrap()).unwrap();
    let registry = Registry::read(&case.home).unwrap();
    let clock = FixedClock(AT);
    let ctx = Context {
        home: &case.home,
        registry: &registry,
        clock: &clock,
    };
    let verb = case.operation.verb();
    let (code, envelope) = match case.operation.call(&ctx, &case.target) {
        Ok(answer) => (answer.exit.code(), answer.render(Format::Json(verb))),
        Err(usage) => (
            3,
            statecraft_cli::render::usage_envelope(&verb.dotted(), &usage),
        ),
    };
    std::fs::write(&case.answer, format!("{code}\n{envelope}")).unwrap();
}

fn executable(path: &Path, script: &str) {
    statecraft_adapter::fixture::install_script(path, script, 0o755).unwrap();
}

/// The producer stub: `run_startup`'s, answering `registry closure` from a
/// file as `contract_binding`'s does, so a closure can move between binding
/// and acceptance.
const PRODUCER: &str = r#"#!/bin/sh
here="$(dirname "$0")"
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check|'check --help') exit 0 ;;
  compile|index) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"replay","title":"recorded stream"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"replay","status":"approved","implementation":"pending","obligations":[{"id":"R-1","kind":"requirement","text":"t","anchor":"a"}]}]}' ;;
  'verify '*' --plan --json') printf '{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"%s"},"schemaVersion":"0.6.0","verb":"verify"}' $2 ;;
  'delta --base '*' --head '*' --json') printf '{"exitCode":0,"ok":true,"report":{"schemaVersion":"0.1.0","tool":{"name":"spec-spine","version":"0.20.0"},"classifiedUnder":"base","base":"%s","mergeBase":"%s","head":"%s","changes":[],"counts":{},"priorPolicy":{"required":false,"classes":[]}},"schemaVersion":"0.6.0","verb":"delta"}' "$3" "$3" "$5" ;;
  'registry closure --help') exit 0 ;;
  'registry closure --request - --json') /bin/cat > /dev/null; /bin/cat "$here/closure-answer" ;;
  *) exit 3 ;;
esac
"#;

/// A closure answer for the one spec, whose obligation reads `text`.
fn closure(text: &str) -> String {
    serde_json::json!({
        "digest": format!("digest-of-{text}"),
        "members": [
            {"kind": "spec", "spec": RUN, "contentHash": "hash"},
            {"kind": "obligation", "spec": RUN, "id": "R-1", "obligationKind": "requirement",
             "text": text, "anchor": "a", "sectionDigest": "s1"}
        ],
        "schemaVersion": "0.6.0"
    })
    .to_string()
}

/// A managed, required, registered and armed project with a fake producer
/// and a fake provider on `PATH`: `run_startup`'s operator route.
///
/// On Linux the confinement's ruleset, and so the digest an attempt's
/// posture records, covers every entry of every directory on the path to the
/// roots it denies. So one fixture exists at a time, holding [`LAUNCHING`]
/// from its creation to its removal, and it lives in a directory of the
/// build's temporary directory no other case uses: what is listed on that
/// path is the same while both sides run, rather than whatever else the
/// machine holds in `/tmp`. One launch at a time also keeps spec 004's
/// deadlines from being measured against a machine other launches load.
pub struct Fixture {
    dir: tempfile::TempDir,
    /// Outside the compared tree: the pristine copy and the child's case.
    aside: tempfile::TempDir,
    /// Released after `dir` and `aside` are removed: fields drop in order.
    _one: std::sync::MutexGuard<'static, ()>,
}

/// One side's result: its answer, if it gave one, and the tree it left.
struct Side {
    answer: Option<(i32, String)>,
    tree: BTreeMap<PathBuf, Option<Vec<u8>>>,
}

impl Fixture {
    pub fn new() -> Fixture {
        let one = LAUNCHING.lock().unwrap_or_else(|e| e.into_inner());
        // Short names: the confinement's probe binds a Unix socket at
        // `<root>/home/probe-XXXXXX/socket`, and a socket path has a limit.
        let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("eq");
        std::fs::create_dir_all(&base).unwrap();
        let dir = tempfile::Builder::new()
            .prefix("")
            .rand_bytes(4)
            .tempdir_in(&base)
            .unwrap();
        let socket = dir.path().join("home/probe-XXXXXX/socket");
        assert!(
            socket.as_os_str().len() < 100,
            "{} is too long for a Unix socket; build with a shorter CARGO_TARGET_DIR",
            socket.display()
        );
        let f = Fixture {
            dir,
            aside: tempfile::tempdir().unwrap(),
            _one: one,
        };
        std::fs::create_dir_all(f.project()).unwrap();
        std::fs::create_dir_all(f.bin()).unwrap();
        std::fs::write(f.root().join(".claude.json"), b"{}").unwrap();
        f.git(&["init", "--quiet", "--initial-branch=main"]);
        for (k, v) in [
            ("user.email", "fixture@example.invalid"),
            ("user.name", "fixture"),
            ("commit.gpgsign", "false"),
            ("maintenance.auto", "false"),
            ("gc.auto", "0"),
        ] {
            f.git(&["config", k, v]);
        }
        std::fs::write(f.project().join("README.md"), "x\n").unwrap();
        f.git(&["add", "."]);
        f.git(&["commit", "--quiet", "-m", "base"]);
        executable(&f.bin().join("spec-spine"), &as_linked(PRODUCER));
        f.closure("t");
        f.stream("success.jsonl");
        executable(&f.bin().join("claude"), FAKE_PROVIDER);
        provider_token::with_token(f.home());
        let target = f.target();
        for args in [
            &["home", "apply"][..],
            &["init", "apply", &target],
            &["harness", "upgrade", &target],
        ] {
            let (code, out) = f.verb(args);
            assert!(code <= 1, "{args:?}: {out}");
        }
        std::fs::write(
            f.project().join("AGENTS.md"),
            "@.statecraft/AGENTS.md\n\n# the fixture project\n",
        )
        .unwrap();
        f.git(&["add", "-A"]);
        f.git(&["commit", "--quiet", "-m", "managed"]);
        for args in [["project", "register"], ["project", "arm"]] {
            let (code, out) = f.verb(&[args[0], args[1], &target]);
            assert!(code <= 1, "{args:?}: {out}");
        }
        f
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }
    fn home(&self) -> PathBuf {
        self.root().join("home")
    }
    fn project(&self) -> PathBuf {
        self.root().join("project")
    }
    fn bin(&self) -> PathBuf {
        self.root().join("bin")
    }
    fn target(&self) -> String {
        self.project().display().to_string()
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(self.project())
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    }

    /// Select one of the fake provider's deviations.
    pub fn mode(&self, mode: &str) {
        std::fs::write(self.bin().join("mode"), mode).unwrap();
    }

    /// The recorded provider stream the fake replays.
    pub fn stream(&self, name: &str) {
        let recorded = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../statecraft-adapter-claude-code/testdata/stream")
            .join(name);
        std::fs::copy(recorded, self.bin().join("native.jsonl")).unwrap();
    }

    /// The closure the producer answers from now on.
    pub fn closure(&self, text: &str) {
        std::fs::write(self.bin().join("closure-answer"), closure(text)).unwrap();
    }

    fn attempt_dir(&self, n: u32) -> PathBuf {
        statecraft_home::launch::AttemptIdentity {
            run_id: RUN.to_string(),
            attempt: n,
        }
        .records_dir(&statecraft_home::launch::Places::of(
            &self.home(),
            &self.project(),
        ))
    }

    fn workspace(&self) -> PathBuf {
        self.project()
            .join(".statecraft/state/workspaces")
            .join(RUN)
    }

    /// The environment both sides of a case run in, and every setup verb.
    fn environment(&self, command: &mut Command) {
        command
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.root().join("native"))
            .env("HOME", self.root())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .env("STATECRAFT_TEST_CLOCK", AT.to_string())
            .env("STATECRAFT_TEST_NONCE_SEED", SEED)
            .current_dir(self.root());
    }

    fn verb(&self, args: &[&str]) -> (i32, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"));
        command.args(args);
        self.environment(&mut command);
        let out = command.output().unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap(),
        )
    }

    fn verb_command(&self, operation: &Operation) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"));
        command.args(operation.args(&self.target()));
        self.environment(&mut command);
        command
    }

    fn answer_path(&self) -> PathBuf {
        self.aside.path().join("answer")
    }

    /// This test binary, re-entered to run the operation in the environment
    /// the verb is given.
    fn operation_command(&self, operation: &Operation) -> Command {
        let case = self.aside.path().join("case.json");
        let _ = std::fs::remove_file(self.answer_path());
        std::fs::write(
            &case,
            serde_json::to_vec(&Case {
                operation: operation.clone(),
                home: self.home(),
                target: self.project(),
                answer: self.answer_path(),
            })
            .unwrap(),
        )
        .unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "launch::operation_in_child",
            "--nocapture",
            "--test-threads=1",
        ]);
        self.environment(&mut command);
        command.env(CHILD, &case);
        command
    }

    fn by_verb(&self, operation: &Operation) -> (i32, String) {
        let out = self.verb_command(operation).output().unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap(),
        )
    }

    fn by_operation(&self, operation: &Operation) -> (i32, String) {
        let out = self.operation_command(operation).output().unwrap();
        assert!(
            out.status.success(),
            "the operation's process: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        let text = std::fs::read_to_string(self.answer_path()).unwrap();
        let (code, envelope) = text.split_once('\n').unwrap();
        (code.parse().unwrap(), envelope.to_string())
    }

    /// Start `command`, wait until the fake blocks after the startup decision,
    /// then kill the launcher and the provider's group: a launcher that died
    /// mid-session, with nothing left running to write afterwards.
    fn killed(&self, mut command: Command, attempt: u32) {
        let mut launcher = command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let order = self.workspace().join(".fixture-output/order");
        let started = Instant::now();
        while !(std::fs::read_to_string(&order)
            .unwrap_or_default()
            .lines()
            .any(|l| l == "blocked")
            && self.settled(attempt))
        {
            assert!(
                started.elapsed() < Duration::from_secs(300),
                "the fake never blocked"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        launcher.kill().unwrap();
        launcher.wait().unwrap();
        let launched: serde_json::Value = serde_json::from_slice(
            &std::fs::read(self.attempt_dir(attempt).join("launched.json")).unwrap(),
        )
        .unwrap();
        let pid = launched["pid"].as_u64().unwrap();
        let _ = Command::new("kill")
            .args(["-s", "KILL", "--", &format!("-{pid}")])
            .output();
    }

    /// The startup decision is written, both where the records keep it and
    /// where the gate reads its copy, and no write to either is in flight: the
    /// state a launcher killed mid-session leaves, and not one caught between
    /// a partial file and its rename.
    fn settled(&self, attempt: u32) -> bool {
        let identity = statecraft_home::launch::AttemptIdentity {
            run_id: RUN.to_string(),
            attempt,
        };
        let places = statecraft_home::launch::Places::of(&self.home(), &self.project());
        let dirs = [
            identity.records_dir(&places),
            identity.exchange_dir(&places),
        ];
        dirs.iter().all(|d| d.join("admission.json").is_file())
            && dirs.iter().all(|d| {
                std::fs::read_dir(d).is_ok_and(|entries| {
                    entries.flatten().all(|e| {
                        !e.file_name()
                            .to_string_lossy()
                            .starts_with(".statecraft-partial-")
                    })
                })
            })
    }

    /// Every file and directory under the fixture, by path, as compared: see
    /// the module comment for the four values compared by what they stand
    /// for. `window` is the wall-clock span this side ran in.
    fn snapshot(&self, window: (u64, u64)) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![self.root().to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                let m = std::fs::symlink_metadata(&p).unwrap();
                let rel = p.strip_prefix(self.root()).unwrap().to_path_buf();
                if m.is_dir() {
                    out.insert(rel, None);
                    stack.push(p);
                } else if m.file_type().is_symlink() {
                    let link = std::fs::read_link(&p).unwrap();
                    out.insert(rel, Some(link.into_os_string().into_encoded_bytes()));
                } else {
                    let bytes = std::fs::read(&p).unwrap_or_default();
                    let bytes = if in_git(&rel, "index") {
                        self.staged(&p)
                    } else if in_git(&rel, "logs") {
                        let before =
                            std::fs::read_to_string(self.aside.path().join("pristine").join(&rel))
                                .unwrap_or_default();
                        reflog(&rel, &bytes, &before, window)
                    } else {
                        bytes
                    };
                    out.insert(rel, Some(bytes));
                }
            }
        }
        settings_names(&mut out);
        out
    }

    /// A git index as the entries it stages: mode, object and path, without
    /// the stat cache (inode, device, nanosecond times) the filesystem fills.
    fn staged(&self, index: &Path) -> Vec<u8> {
        let out = Command::new("git")
            .args(["ls-files", "--stage"])
            .env("GIT_DIR", self.project().join(".git"))
            .env("GIT_INDEX_FILE", index)
            .current_dir(self.root())
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", index.display());
        let mut bytes = b"staged:\n".to_vec();
        bytes.extend(out.stdout);
        bytes
    }

    fn keep(&self) -> PathBuf {
        let copy = self.aside.path().join("pristine");
        let _ = std::fs::remove_dir_all(&copy);
        std::fs::create_dir_all(&copy).unwrap();
        copy_tree(self.root(), &copy);
        copy
    }
    fn restore(&self, copy: &Path) {
        for e in std::fs::read_dir(self.root()).unwrap().flatten() {
            let p = e.path();
            if std::fs::symlink_metadata(&p).unwrap().is_dir() {
                std::fs::remove_dir_all(&p).unwrap();
            } else {
                std::fs::remove_file(&p).unwrap();
            }
        }
        copy_tree(copy, self.root());
    }

    /// One side, timed and snapshotted.
    fn side(&self, act: impl FnOnce() -> Option<(i32, String)>) -> Side {
        let start = now();
        let answer = act();
        let end = now();
        let mut tree = self.snapshot((start - 1, end + 1));
        let pids = provider_pids(&mut tree);
        random_names(&mut tree);
        Side {
            answer: answer.map(|(code, envelope)| (code, unpid(&envelope, &pids))),
            tree,
        }
    }

    /// Run the verb, restore, run the operation; both answers and both
    /// resulting trees must be equal. The operation's result is left in
    /// place for the next case, and the verb's envelope is returned.
    pub fn compare(&self, operation: Operation) -> serde_json::Value {
        let pristine = self.keep();
        let by_verb = self.side(|| Some(self.by_verb(&operation)));
        self.restore(&pristine);
        let by_operation = self.side(|| Some(self.by_operation(&operation)));
        agree(&operation.args(&self.target()), &by_verb, &by_operation);
        let (_, envelope) = by_verb.answer.unwrap();
        serde_json::from_str(&envelope).unwrap()
    }

    /// As [`Fixture::compare`], with each side's launcher killed once the
    /// fake blocks in `attempt`: neither answers, and what each left behind
    /// must agree.
    pub fn compare_killed(&self, operation: Operation, attempt: u32) {
        self.mode("block");
        let pristine = self.keep();
        let by_verb = self.side(|| {
            self.killed(self.verb_command(&operation), attempt);
            None
        });
        self.restore(&pristine);
        let by_operation = self.side(|| {
            self.killed(self.operation_command(&operation), attempt);
            None
        });
        agree(&operation.args(&self.target()), &by_verb, &by_operation);
        self.mode("faithful");
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Whether `rel` is under a `.git` directory and, below it, under or named
/// `part`.
fn in_git(rel: &Path, part: &str) -> bool {
    let names: Vec<&std::ffi::OsStr> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(n) => Some(n),
            _ => None,
        })
        .collect();
    names
        .iter()
        .position(|n| *n == ".git")
        .is_some_and(|at| names[at + 1..].iter().any(|n| *n == part))
}

/// A reflog as compared. A line the state before this side already held is
/// kept byte for byte. A line this side added has its seconds checked to fall
/// within `window` and then written as the word `git-time`; every other byte
/// of the line is kept.
fn reflog(rel: &Path, bytes: &[u8], before: &str, window: (u64, u64)) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let held: BTreeSet<&str> = before.lines().collect();
    let mut out = String::new();
    for line in text.lines() {
        if held.contains(line) {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let (head, message) = line.split_once('\t').unwrap_or((line, ""));
        let mut words: Vec<&str> = head.split(' ').collect();
        let at = words.len() - 2;
        let seconds: u64 = words[at].parse().unwrap();
        assert!(
            (window.0..=window.1).contains(&seconds),
            "{}: git stamped {seconds}, outside this side's run {window:?}",
            rel.display()
        );
        words[at] = "git-time";
        out.push_str(&format!("{}\t{message}\n", words.join(" ")));
    }
    out.into_bytes()
}

/// The pid each launch record names, checked against the pid the fake
/// reported where its report survives, and then written as `provider-pid`
/// in the records that carry it. Returns the pids, so the answer is read the
/// same way.
fn provider_pids(tree: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) -> BTreeSet<u64> {
    let mut pids = BTreeSet::new();
    for (path, bytes) in tree.iter() {
        if path.starts_with("home/records") && path.ends_with("launched.json") {
            let v: serde_json::Value = serde_json::from_slice(bytes.as_ref().unwrap()).unwrap();
            pids.insert(v["pid"].as_u64().unwrap());
        }
    }
    for (path, bytes) in tree.iter_mut() {
        let Some(bytes) = bytes else { continue };
        if path.ends_with(".fixture-output/pid") {
            let reported: u64 = String::from_utf8_lossy(bytes).trim().parse().unwrap();
            assert!(
                pids.contains(&reported),
                "the fake ran as {reported}, which no launch record names: {pids:?}"
            );
            *bytes = b"provider-pid\n".to_vec();
        } else if path.starts_with("home/records") {
            *bytes = unpid(&String::from_utf8_lossy(bytes), &pids).into_bytes();
        }
    }
    pids
}

/// `text` with each of `pids` written as `provider-pid` where a record or an
/// answer states it: a `"pid"` member and a summary's `process <pid> `. Any
/// other number, and any pid no launch record names, is left as it is.
fn unpid(text: &str, pids: &BTreeSet<u64>) -> String {
    let mut text = text.to_string();
    for pid in pids {
        for (from, to) in [
            (format!("\"pid\": {pid},"), "\"pid\": \"provider-pid\","),
            (format!("\"pid\": {pid}\n"), "\"pid\": \"provider-pid\"\n"),
            (format!("process {pid} "), "process provider-pid "),
        ] {
            text = text.replace(&from, to);
        }
    }
    text
}

/// The temporary files a killed process leaves behind carry tempfile's random
/// names, and a partial write the writer's pid and a random suffix. Each such
/// name is compared by its shape, its directory and its bytes: the random part
/// is written as `random`, and names that then coincide are told apart by
/// their order of contents.
fn random_names(tree: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
    fn random(part: &str) -> Option<String> {
        let alnum =
            |s: &str, n: usize| s.len() == n && s.chars().all(|c| c.is_ascii_alphanumeric());
        for prefix in [".tmp", "probe-"] {
            if part.strip_prefix(prefix).is_some_and(|rest| alnum(rest, 6)) {
                return Some(format!("{prefix}random"));
            }
        }
        if part
            .strip_prefix("statecraft-settings-")
            .and_then(|r| r.strip_suffix(".json"))
            .is_some_and(|rest| alnum(rest, 6))
        {
            return Some("statecraft-settings-random.json".into());
        }
        if part
            .strip_prefix(".statecraft-partial-")
            .and_then(|r| r.split_once('-'))
            .is_some_and(|(pid, suffix)| {
                pid.chars().all(|c| c.is_ascii_digit())
                    && suffix.len() == 32
                    && suffix.chars().all(|c| c.is_ascii_hexdigit())
            })
        {
            return Some(".statecraft-partial-random".into());
        }
        None
    }
    let mut grouped: BTreeMap<PathBuf, Vec<Option<Vec<u8>>>> = BTreeMap::new();
    for (path, bytes) in std::mem::take(tree) {
        let renamed: PathBuf = path
            .components()
            .map(|c| {
                let part = c.as_os_str().to_string_lossy();
                random(&part).unwrap_or_else(|| part.into_owned())
            })
            .collect();
        grouped.entry(renamed).or_default().push(bytes);
    }
    for (path, mut all) in grouped {
        if all.len() == 1 {
            tree.insert(path, all.pop().unwrap());
            continue;
        }
        all.sort();
        for (n, bytes) in all.into_iter().enumerate() {
            tree.insert(PathBuf::from(format!("{} #{n}", path.display())), bytes);
        }
    }
}

/// Spec 004's adapter names the settings file it hands the provider with
/// tempfile's random suffix. No record carries the name; the fake's report of
/// what it was handed does, and there the six random characters are written
/// as `random` once the shape is checked.
fn settings_names(tree: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
    const PREFIX: &str = "statecraft-settings-";
    for (path, bytes) in tree.iter_mut() {
        let Some(bytes) = bytes else { continue };
        let reported = path.ends_with(".fixture-output/received-settings-path");
        if !reported && !path.ends_with(".fixture-output/exchange-listing") {
            continue;
        }
        let text = String::from_utf8_lossy(bytes).into_owned();
        let mut out = String::new();
        let mut rest = text.as_str();
        let mut found = false;
        while let Some(at) = rest.find(PREFIX) {
            let (before, after) = rest.split_at(at + PREFIX.len());
            let random = &after[..6];
            assert!(
                random.chars().all(|c| c.is_ascii_alphanumeric())
                    && after[6..].starts_with(".json"),
                "{}: {text}",
                path.display()
            );
            out.push_str(before);
            out.push_str("random");
            rest = &after[6..];
            found = true;
        }
        out.push_str(rest);
        assert!(found || !reported, "{}: {text}", path.display());
        *bytes = out.into_bytes();
    }
}

/// Both sides' answers, paths and bytes are equal; every differing file is
/// named with both contents.
fn agree(args: &[String], by_verb: &Side, by_operation: &Side) {
    assert_eq!(
        by_verb.answer, by_operation.answer,
        "{args:?}: the answers differ"
    );
    let differing: Vec<String> = by_verb
        .tree
        .keys()
        .chain(by_operation.tree.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| by_verb.tree.get(*path) != by_operation.tree.get(*path))
        .map(|path| {
            let show = |side: &Side| match side.tree.get(path) {
                None => "(absent)".to_string(),
                Some(None) => "(a directory)".to_string(),
                Some(Some(bytes)) => String::from_utf8_lossy(bytes).into_owned(),
            };
            format!(
                "{}\n--- verb\n{}\n--- operation\n{}",
                path.display(),
                show(by_verb),
                show(by_operation)
            )
        })
        .collect();
    assert!(
        differing.is_empty(),
        "{args:?}: what each side left differs:\n{}",
        differing.join("\n\n")
    );
}

/// `from`'s tree at `to`. A killed launcher leaves its probe's socket
/// behind, and BSD `cp` copies no socket, so each socket `cp` left out is
/// bound again at the same path: what is compared is that the path exists,
/// as it did. GNU `cp` recreates sockets itself.
fn copy_tree(from: &Path, to: &Path) {
    use std::os::unix::fs::FileTypeExt;
    let copied = Command::new("cp")
        .args([
            "-Rp",
            &format!("{}/.", from.display()),
            to.to_str().unwrap(),
        ])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    let mut sockets = Vec::new();
    let mut stack = vec![from.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let kind = e.file_type().unwrap();
            if kind.is_dir() {
                stack.push(e.path());
            } else if kind.is_socket() {
                sockets.push(e.path().strip_prefix(from).unwrap().to_path_buf());
            }
        }
    }
    // BSD `cp` exits 1 for a socket it skipped and for nothing else here.
    assert!(copied.success() || !sockets.is_empty(), "cp failed");
    for socket in sockets {
        let at = to.join(socket);
        if std::fs::symlink_metadata(&at).is_err() {
            drop(std::os::unix::net::UnixListener::bind(at).unwrap());
        }
    }
}

fn text(v: &serde_json::Value, pointer: &str) -> String {
    v.pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("{pointer} in {v}"))
        .to_string()
}

fn exit(v: &serde_json::Value) -> i64 {
    v["exitCode"].as_i64().unwrap()
}

/// A-EQ-LAUNCH, A-EQ-RUN and A-EQ-ACCEPT: an admitted run launches the
/// provider, which completes; both sides append the same intent and attempt
/// records, finalize startup alike and keep the same workspace. Acceptance of
/// that attempt judges the same evidence alike, and once its contract closure
/// moves both answer no acceptance, naming the change.
#[test]
fn a_completed_launch_and_its_acceptance_record_as_the_verb_does() {
    let f = Fixture::new();
    let v = f.compare(Operation::Run);
    assert_eq!(exit(&v), 0, "{v}");
    assert_eq!(text(&v, "/report/outcome"), "completed");
    assert_eq!(text(&v, "/report/contract/digest"), "digest-of-t");
    let v = f.compare(Operation::Accept);
    assert_eq!(text(&v, "/report/contract/word"), "current");
    f.closure("t, amended");
    let v = f.compare(Operation::Accept);
    assert_eq!(text(&v, "/report/acceptance"), "none");
    assert_eq!(
        text(&v, "/report/contract/nowDigest"),
        "digest-of-t, amended"
    );
}

/// A-EQ-LAUNCH: a provider that reports failure and one stopped at its turn
/// limit are each concluded alike, and acceptance of the interrupted attempt
/// is not attempted on either side. A-EQ-RECONCILE: reconciling a concluded
/// attempt, as observed or as something else, invents nothing on either side.
#[test]
fn failed_and_interrupted_launches_conclude_as_the_verb_does() {
    let f = Fixture::new();
    f.stream("api-error.jsonl");
    let v = f.compare(Operation::Run);
    assert_eq!(text(&v, "/report/adapterClaimed"), "failed");
    assert_eq!(text(&v, "/report/outcome"), "interrupted");
    f.stream("max-turns.jsonl");
    let v = f.compare(Operation::Run);
    assert_eq!(v["report"]["attempt"], 2, "{v}");
    assert_eq!(text(&v, "/report/outcome"), "interrupted");
    let v = f.compare(Operation::Accept);
    assert_eq!(text(&v, "/report/acceptance"), "not-attempted");
    assert_eq!(text(&v, "/report/reason"), "attempt-interrupted");
    for (finding, inspected) in [("absent", "interrupted"), ("confirmed", "not-launched")] {
        let v = f.compare(Operation::Reconcile {
            attempt: "2".into(),
            finding: finding.into(),
            inspected: inspected.into(),
        });
        assert_eq!(exit(&v), 2, "{v}");
        assert!(text(&v, "/error/message").contains("is not live"), "{v}");
    }
}

/// A-EQ-LAUNCH and A-ADMISSION: refusals before any launch. A malformed
/// override journal, read under the lock, and a requirement whose content
/// cannot be established each refuse with no attempt; an intent that cannot
/// be written refuses with no process.
#[test]
fn launches_refused_before_the_provider_refuse_as_the_verb_does() {
    let f = Fixture::new();
    let journal = statecraft_run::overrides::journal_path(&f.home(), &f.project());
    std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
    std::fs::write(&journal, "{ not a journal line\n").unwrap();
    let v = f.compare(Operation::Run);
    assert!(
        text(&v, "/error/details/failed").contains("cannot be read as a set of overrides"),
        "{v}"
    );
    std::fs::remove_file(&journal).unwrap();
    let records = statecraft_home::launch::Places::of(&f.home(), &f.project()).records;
    std::fs::create_dir_all(records.parent().unwrap()).unwrap();
    std::fs::write(&records, "not a directory").unwrap();
    let v = f.compare(Operation::Run);
    assert_eq!(text(&v, "/error/details/reason"), "startup-record");
    assert_eq!(v["error"]["details"]["startup"]["launched"], false, "{v}");
    std::fs::remove_file(&records).unwrap();
    let (_, shown) = f.verb(&["harness", "show", &f.target(), "--json"]);
    let shown: serde_json::Value = serde_json::from_str(&shown).unwrap();
    let display = text(&shown, "/report/value/inspection/requiredDisplay");
    std::fs::write(
        f.home()
            .join("harness")
            .join(display)
            .join("rules/statecraft-governed-work.md"),
        "changed\n",
    )
    .unwrap();
    let v = f.compare(Operation::Run);
    assert_eq!(exit(&v), 2, "{v}");
    assert!(text(&v, "/error/message").contains("is not driven"), "{v}");
}

/// A-EQ-LAUNCH and A-EQ-RECONCILE: a launcher killed mid-session leaves the
/// same confirmed spawn and decision on either side and no outcome; the next
/// run is refused alike, naming what to inspect; and each reconcile choice
/// against the uncertain attempt answers and records alike.
#[test]
fn a_killed_launcher_and_its_reconciliation_record_as_the_verb_does() {
    let f = Fixture::new();
    f.compare_killed(Operation::Run, 1);
    let v = f.compare(Operation::Run);
    assert_eq!(text(&v, "/error/details/launchState"), "outcome-unknown");
    for (finding, inspected, code) in [
        ("absent", "not-launched", 2),
        ("unknown", "outcome-unknown", 0),
        ("absent", "outcome-unknown", 0),
        ("confirmed", "outcome-unknown", 2),
    ] {
        let v = f.compare(Operation::Reconcile {
            attempt: "1".into(),
            finding: finding.into(),
            inspected: inspected.into(),
        });
        assert_eq!(exit(&v), code, "{finding} {inspected}: {v}");
    }
}

/// A-EQ-TRIAL: a faithful synthetic trial concludes alike and records alike,
/// and the repeat is refused alike, spending nothing.
#[test]
fn a_trial_and_its_repeat_record_as_the_verb_does() {
    let f = Fixture::new();
    let v = f.compare(Operation::Trial);
    assert_eq!(text(&v, "/report/outcome"), "completed");
    let v = f.compare(Operation::Trial);
    assert!(
        text(&v, "/error/details/reason").contains("trial is spent"),
        "{v}"
    );
}

/// A-EQ-TRIAL: a provider that ignores the registration leaves the startup
/// evidence incomplete, and both sides conclude and record that alike.
#[test]
fn a_trial_with_incomplete_startup_evidence_records_as_the_verb_does() {
    let f = Fixture::new();
    f.mode("ignores-hooks");
    let v = f.compare(Operation::Trial);
    assert_eq!(text(&v, "/report/outcome"), "refused");
}
