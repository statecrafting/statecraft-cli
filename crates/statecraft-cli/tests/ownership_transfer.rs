//! Spec 006 section 3.11.7's transfer verbs, for spec 002 section 3.35, through
//! the built binary.
//!
//! Every run is given a temporary `STATECRAFT_HOME`, a temporary `HOME` and a
//! constructed `PATH`, so nothing here reads or writes the operator's own home
//! or finds a provider or a spec-spine this test did not place. Every refusal is
//! checked for exit 2 and for every byte of the fixture repository being as it
//! was. The library's own suite is `statecraft-environment`'s
//! `tests/transfer.rs`.
//!
//! The installed adapter is the claude-code one the binary configures. Whether
//! it *claims* its paths depends on its credential prerequisite, which is
//! satisfied only on macOS (spec 004 section 3.14), and rule 1 admits a move
//! to `managed` only where it does. So every test that needs a `managed` path
//! asserts the move on macOS, against a fake provider and a synthetic
//! qualification record, and elsewhere asserts the refusal
//! (`adapter-not-claiming`) and reports the rest skipped. The library suite
//! asserts the `managed` halves on every platform with a test probe.

#![cfg(unix)]

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OWNED: &str = ".claude/statecraft/instructions.md";

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_statecraft-cli"))
}

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    /// A registered git repository with a manifest, and a home with a fake
    /// provider and a synthetic qualification record for it.
    fn new() -> Self {
        Self::with(&[])
    }

    /// The same, with `files` already in the repository when the first `env
    /// apply` runs, so an adapter path among them is withheld as `foreign`
    /// rather than written.
    fn with(files: &[(&str, &[u8])]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let s = Self { dir };
        std::fs::create_dir_all(s.home()).unwrap();
        std::fs::create_dir_all(s.bin()).unwrap();
        std::fs::create_dir_all(s.project()).unwrap();
        statecraft_adapter::fixture::install_script(
            &s.bin().join("claude"),
            "#!/bin/sh\n[ \"$1\" = --version ] && { echo 2.1.267; exit 0; }\nexit 3\n",
            0o755,
        )
        .unwrap();
        let record = statecraft_adapter_claude_code::qualification::record(
            "2.1.267",
            "synthetic-fixture-only",
            "2026-09-23T00:00:00Z",
        );
        std::fs::write(
            s.home().join("qualifications.json"),
            serde_json::to_vec(&vec![record]).unwrap(),
        )
        .unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(s.project())
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "test"]);
        git(&["config", "commit.gpgsign", "false"]);
        s.write("README.md", b"x\n");
        for (path, bytes) in files {
            s.write(path, bytes);
        }
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "one"]);
        let root = s.root();
        assert!(code(&s.run(&["project", "register", &root])) <= 1);
        // `env apply` is what creates the manifest a transfer is recorded in.
        assert!(code(&s.run(&["env", "apply", &root])) <= 1);
        assert!(s.project().join(".statecraft/environment.json").exists());
        s
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

    fn root(&self) -> String {
        self.project().to_string_lossy().to_string()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(binary())
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .expect("the binary runs")
    }

    fn json(&self, args: &[&str]) -> (i32, Value) {
        let mut args = args.to_vec();
        args.push("--json");
        let out = self.run(&args);
        let value = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{args:?}: {e}: {}", stdout(&out)));
        (code(&out), value)
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let at = self.project().join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, bytes).unwrap();
    }

    fn read(&self, path: &str) -> Vec<u8> {
        std::fs::read(self.project().join(path)).unwrap()
    }

    fn manifest(&self) -> Value {
        serde_json::from_slice(&self.read(".statecraft/environment.json")).unwrap()
    }

    fn entry(&self, path: &str) -> Option<Value> {
        self.manifest()["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == path)
            .cloned()
    }

    fn plan(&self, file: &str, from: &str, to: &str) -> Value {
        let (c, v) = self.json(&["transfer", "plan", &self.root(), file, from, to]);
        assert_eq!(c, 0, "{v}");
        v
    }

    fn plan_id(&self, file: &str, from: &str, to: &str) -> String {
        self.plan(file, from, to)["value"]["plan_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn apply_with(&self, file: &str, from: &str, to: &str, id: &str) -> (i32, Value) {
        self.json(&[
            "transfer",
            "apply",
            &self.root(),
            file,
            from,
            to,
            id,
            "bart",
            "moving",
            "it",
        ])
    }

    /// Plan and apply; returns the record's identity.
    fn transfer(&self, file: &str, from: &str, to: &str) -> String {
        let id = self.plan_id(file, from, to);
        let (c, v) = self.apply_with(file, from, to, &id);
        assert_eq!(c, 0, "{v}");
        assert_eq!(v["value"]["result"], "applied", "{v}");
        v["value"]["record"]["id"].as_str().unwrap().to_string()
    }

    fn revert(&self, id: &str) -> (i32, Value) {
        self.json(&["transfer", "revert", &self.root(), id, "bart", "undo"])
    }

    /// Every byte of the fixture repository.
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        walk(&self.project(), &self.project(), &mut out);
        out
    }

    /// Whether the configured adapter claims its paths here.
    fn claiming(&self) -> bool {
        let (_, v) = self.json(&["env", "plan", &self.root()]);
        !v.to_string().contains("refuses to claim")
    }

    /// Assert exit 2, naming `kind`, with every byte unchanged.
    fn refused(&self, kind: &str, args: &[&str]) {
        let before = self.snapshot();
        let (c, v) = self.json(args);
        assert_eq!(c, 2, "{args:?}: {v}");
        assert_eq!(v["value"]["refused"]["kind"], kind, "{args:?}: {v}");
        assert_eq!(self.snapshot(), before, "{args:?} changed a byte");
        // The human rendering says the same, from the same value.
        let human = self.run(args);
        assert_eq!(code(&human), 2);
        assert!(stdout(&human).contains("refused"), "{}", stdout(&human));
        assert_eq!(self.snapshot(), before);
    }
}

fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(at).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        // Runtime state (the manifest lock file) is gitignored and not a byte
        // of the committed repository.
        if rel == ".statecraft/state" {
            continue;
        }
        let kind = entry.file_type().unwrap();
        if kind.is_symlink() {
            let target = std::fs::read_link(&path).unwrap();
            out.insert(rel, format!("-> {}", target.display()).into_bytes());
        } else if kind.is_dir() {
            out.insert(format!("{rel}/"), Vec::new());
            walk(root, &path, out);
        } else {
            out.insert(rel, std::fs::read(&path).unwrap());
        }
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

// Acceptance: `user` to `adopted` and back.
#[test]
fn user_to_adopted_and_back_through_the_binary() {
    let s = Sandbox::new();
    s.write("docs/policy.md", b"the user's policy\n");
    let before = s.snapshot();

    let plan = s.plan("docs/policy.md", "user", "adopted");
    assert_eq!(s.snapshot(), before, "plan writes nothing");
    let v = &plan["value"];
    assert_eq!(v["current"]["class"], "user");
    assert_eq!(v["resulting"]["class"], "adopted");
    assert_eq!(v["bytes"], 18);
    assert_eq!(v["producer"], "spec-spine-core@0.25.0");
    let human = s.run(&[
        "transfer",
        "plan",
        &s.root(),
        "docs/policy.md",
        "user",
        "adopted",
    ]);
    assert_eq!(code(&human), 0);
    assert!(stdout(&human).contains(v["plan_id"].as_str().unwrap()));

    let (c, applied) = s.apply_with(
        "docs/policy.md",
        "user",
        "adopted",
        v["plan_id"].as_str().unwrap(),
    );
    assert_eq!(c, 0, "{applied}");
    assert_eq!(applied["value"]["result"], "applied");
    let record = &applied["value"]["record"];
    assert_eq!(record["operator"], "bart");
    assert_eq!(record["reason"], "moving it");
    assert_eq!(record["manifest_before"], v["manifest_digest"]);
    assert_eq!(s.entry("docs/policy.md").unwrap()["class"], "adopted");
    assert_eq!(s.manifest()["transfers"].as_array().unwrap().len(), 1);
    assert_eq!(s.read("docs/policy.md"), b"the user's policy\n");

    let (c, reverted) = s.revert(record["id"].as_str().unwrap());
    assert_eq!(c, 0, "{reverted}");
    assert_eq!(reverted["value"]["result"], "reverted");
    assert_eq!(reverted["value"]["record"]["reverts"], record["id"]);
    assert!(s.entry("docs/policy.md").is_none());
    assert_eq!(s.manifest()["transfers"].as_array().unwrap().len(), 2);
    assert_eq!(s.read("docs/policy.md"), b"the user's policy\n");
}

// Acceptance: `user` to `managed` for an adapter path `env apply` withheld as
// `foreign`, after which `env apply` writes it, and the reversal, after which
// it is withheld again.
#[test]
fn a_withheld_adapter_path_moved_to_managed_is_written_then_withheld_after_reversal() {
    let s = Sandbox::with(&[(OWNED, b"the user's copy\n")]);
    let claiming = s.claiming();
    if cfg!(target_os = "macos") {
        assert!(claiming, "the fake provider and record satisfy the adapter");
    }
    let root = s.root();
    if !claiming {
        s.refused(
            "adapter-not-claiming",
            &["transfer", "plan", &root, OWNED, "user", "managed"],
        );
        eprintln!("skipped: the adapter does not claim its paths on this platform");
        return;
    }
    {
        let (c, v) = s.json(&["env", "apply", &root]);
        assert_eq!(c, 1, "partial: {v}");
        assert!(v.to_string().contains(OWNED), "withheld and named: {v}");
        assert_eq!(s.read(OWNED), b"the user's copy\n");
    }

    let plan = s.plan(OWNED, "user", "managed");
    assert_eq!(plan["value"]["current"]["declared_by"], "claude-code");
    let id = s.transfer(OWNED, "user", "managed");
    let entry = s.entry(OWNED).unwrap();
    assert_eq!(entry["class"], "managed");
    assert_eq!(entry["source"]["identity"], "claude-code");
    assert_eq!(
        entry["transfer"]["evaluated_against"],
        "spec-spine-core@0.25.0"
    );
    assert_eq!(
        s.read(OWNED),
        b"the user's copy\n",
        "the transfer wrote nothing"
    );

    {
        let (c, v) = s.json(&["env", "apply", &root]);
        assert_eq!(c, 0, "applied: {v}");
        assert_ne!(s.read(OWNED), b"the user's copy\n", "env apply wrote it");
    }
    let written = s.read(OWNED);

    let (c, v) = s.revert(&id);
    assert_eq!(c, 0, "{v}");
    assert!(s.entry(OWNED).is_none());
    assert_eq!(
        s.read(OWNED),
        written,
        "reversal changes ownership, never content"
    );

    {
        let (c, v) = s.json(&["env", "apply", &root]);
        assert_eq!(c, 1, "withheld again: {v}");
        assert!(v.to_string().contains(OWNED), "{v}");
        assert_eq!(s.read(OWNED), written);
    }
}

// Acceptance: `managed` to `user`, after which `env remove` leaves the file.
#[test]
fn a_managed_path_released_to_user_is_left_by_env_remove() {
    let s = Sandbox::new();
    if !s.claiming() {
        eprintln!("skipped: no path is managed where the adapter does not claim its paths");
        return;
    }
    assert_eq!(
        s.entry(OWNED).unwrap()["class"],
        "managed",
        "env apply wrote it"
    );
    let bytes = s.read(OWNED);

    s.transfer(OWNED, "managed", "user");
    let (c, v) = s.json(&["env", "remove", &s.root()]);
    assert!(c <= 1, "{v}");
    assert_eq!(s.read(OWNED), bytes, "the file stays, byte for byte");
}

// Negative: a stale plan after the file, the manifest or the class changed.
#[test]
fn a_stale_plan_is_refused_naming_what_changed() {
    let s = Sandbox::new();
    let root = s.root();
    s.write("notes.md", b"one\n");
    s.write("other.md", b"other\n");
    let stale = |id: &str, changed: &[&str]| {
        let args = [
            "transfer", "apply", &root, "notes.md", "user", "adopted", id, "bart", "why",
        ];
        s.refused("stale-plan", &args);
        let (_, v) = s.json(&args);
        assert_eq!(
            v["value"]["refused"]["changed"],
            serde_json::json!(changed),
            "{v}"
        );
    };

    let id = s.plan_id("notes.md", "user", "adopted");
    s.write("notes.md", b"two\n");
    stale(&id, &["file"]);

    let id = s.plan_id("notes.md", "user", "adopted");
    s.transfer("other.md", "user", "adopted");
    stale(&id, &["manifest"]);

    let other = s.plan_id("other.md", "adopted", "user");
    stale(&other, &["classes", "path", "file"]);

    stale("not-a-plan", &["identity"]);

    // A class that changed is named as the class, before the identity.
    let id = s.plan_id("notes.md", "user", "adopted");
    s.transfer("notes.md", "user", "adopted");
    s.write("notes.md", b"edited after adoption\n");
    s.refused(
        "class-mismatch",
        &[
            "transfer", "apply", &root, "notes.md", "user", "adopted", &id, "bart", "why",
        ],
    );
}

// Negative: a named class that is not the path's; a move to `managed` with no
// adapter source; a move the table does not admit.
#[test]
fn classes_and_moves_the_contract_does_not_admit_are_refused() {
    let s = Sandbox::new();
    let root = s.root();
    s.write("notes.md", b"one\n");
    s.refused(
        "class-mismatch",
        &["transfer", "plan", &root, "notes.md", "adopted", "user"],
    );
    s.refused(
        "no-source",
        &["transfer", "plan", &root, "notes.md", "user", "managed"],
    );
    s.refused(
        "move-not-admitted",
        &["transfer", "plan", &root, "notes.md", "adopted", "managed"],
    );
}

// Negative: a root `AGENTS.md`, a symbolic link, a `..` path, a directory,
// `.statecraft/environment.json` and a `.git` path.
#[test]
fn protected_escaping_linked_and_instruction_paths_are_refused() {
    let s = Sandbox::new();
    let root = s.root();
    s.write("AGENTS.md", b"the user's instructions\n");
    s.write("real.md", b"x\n");
    std::os::unix::fs::symlink(s.project().join("real.md"), s.project().join("link.md")).unwrap();
    std::fs::create_dir_all(s.project().join("a-directory")).unwrap();
    for (file, kind) in [
        ("AGENTS.md", "instruction-file"),
        ("link.md", "symbolic-link"),
        ("../outside.md", "escaping"),
        ("a-directory", "directory"),
        (".statecraft/environment.json", "protected"),
        (".git/HEAD", "protected"),
        ("absent.md", "missing"),
    ] {
        s.refused(kind, &["transfer", "plan", &root, file, "user", "adopted"]);
        s.refused(
            kind,
            &[
                "transfer", "apply", &root, file, "user", "adopted", "x", "bart", "why",
            ],
        );
    }
}

// Negative: a reversal after an intervening edit or a later transfer.
#[test]
fn a_reversal_after_an_unrecorded_edit_or_a_later_transfer_is_refused() {
    let s = Sandbox::new();
    let root = s.root();
    s.write("notes.md", b"one\n");
    let first = s.transfer("notes.md", "user", "adopted");
    s.write("notes.md", b"edited behind the product's back\n");
    s.refused(
        "unrecorded-change",
        &["transfer", "revert", &root, &first, "bart", "undo"],
    );

    s.write("other.md", b"two\n");
    let adopted = s.transfer("other.md", "user", "adopted");
    s.transfer("other.md", "adopted", "user");
    s.refused(
        "later-transfer",
        &["transfer", "revert", &root, &adopted, "bart", "undo"],
    );
    s.refused(
        "unknown-transfer",
        &[
            "transfer",
            "revert",
            &root,
            "no-such-transfer",
            "bart",
            "undo",
        ],
    );
}

// Rule 6 without an intervening change is the ordinary reversal; covered with
// one above too, stated here beside its negative.
#[test]
fn a_reversal_with_nothing_changed_since_is_applied() {
    let s = Sandbox::new();
    s.write("notes.md", b"one\n");
    let id = s.transfer("notes.md", "user", "adopted");
    let (c, v) = s.revert(&id);
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["value"]["result"], "reverted");
}

// Negative: a repeated request is `already-satisfied`, exit 0, nothing written.
#[test]
fn a_repeated_request_is_already_satisfied_and_writes_nothing() {
    let s = Sandbox::new();
    s.write("notes.md", b"one\n");
    let id = s.plan_id("notes.md", "user", "adopted");
    let (c, first) = s.apply_with("notes.md", "user", "adopted", &id);
    assert_eq!(c, 0);
    let before = s.snapshot();
    for given in [id.as_str(), "some-other-plan"] {
        let (c, v) = s.apply_with("notes.md", "user", "adopted", given);
        assert_eq!(c, 0, "{v}");
        assert_eq!(v["value"]["result"], "already-satisfied");
        assert_eq!(v["value"]["record"]["id"], first["value"]["record"]["id"]);
        assert_eq!(s.snapshot(), before, "nothing written");
    }
}

// Exit 3: usage. Exit 2: an unregistered target, or a blank operator.
#[test]
fn usage_errors_exit_3_and_an_unregistered_target_is_refused() {
    let s = Sandbox::new();
    let root = s.root();
    s.write("notes.md", b"one\n");
    let before = s.snapshot();
    for args in [
        vec!["transfer", "plan", root.as_str(), "notes.md", "user"],
        vec![
            "transfer",
            "plan",
            root.as_str(),
            "notes.md",
            "user",
            "foreign",
        ],
        vec![
            "transfer",
            "apply",
            root.as_str(),
            "notes.md",
            "user",
            "adopted",
            "id",
            "bart",
        ],
        vec!["transfer", "revert", root.as_str(), "id", "bart"],
        vec!["transfer", "publish", root.as_str()],
    ] {
        let out = s.run(&args);
        assert_eq!(code(&out), 3, "{args:?}");
        assert!(stdout(&out).is_empty(), "{args:?}");
    }
    assert_eq!(s.snapshot(), before);

    let id = s.plan_id("notes.md", "user", "adopted");
    s.refused(
        "missing-operator-or-reason",
        &[
            "transfer", "apply", &root, "notes.md", "user", "adopted", &id, " ", "why",
        ],
    );

    let elsewhere = tempfile::tempdir().unwrap();
    let out = s.run(&[
        "transfer",
        "plan",
        elsewhere.path().to_str().unwrap(),
        "notes.md",
        "user",
        "adopted",
    ]);
    assert_eq!(code(&out), 2);
    assert!(stdout(&out).contains("not registered"), "{}", stdout(&out));
}

// Exit 4: the manifest could not be read, or could not be written durably.
#[test]
fn an_unreadable_or_unwritable_manifest_is_a_failure_and_leaves_every_byte() {
    use std::os::unix::fs::PermissionsExt as _;
    let s = Sandbox::new();
    let root = s.root();
    s.write("notes.md", b"one\n");
    let id = s.plan_id("notes.md", "user", "adopted");

    // Unwritable: the rename cannot land, so the old manifest stays whole.
    let area = s.project().join(".statecraft");
    let before = s.snapshot();
    std::fs::set_permissions(&area, std::fs::Permissions::from_mode(0o555)).unwrap();
    let writable = std::fs::write(area.join(".probe"), b"").is_ok();
    let (c, v) = if writable {
        (4, Value::Null)
    } else {
        s.apply_with("notes.md", "user", "adopted", &id)
    };
    std::fs::set_permissions(&area, std::fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        // A superuser writes through the mode; there is nothing to test.
        std::fs::remove_file(area.join(".probe")).unwrap();
    } else {
        assert_eq!(c, 4, "{v}");
        assert!(v["value"]["failed"].is_string(), "{v}");
        assert_eq!(s.snapshot(), before, "the old manifest, and no temporary");
    }

    // Unreadable: not a manifest this build understands.
    s.write(".statecraft/environment.json", b"{ not json");
    let before = s.snapshot();
    let (c, v) = s.json(&["transfer", "plan", &root, "notes.md", "user", "adopted"]);
    assert_eq!(c, 4, "{v}");
    let (c, _) = s.apply_with("notes.md", "user", "adopted", &id);
    assert_eq!(c, 4);
    assert_eq!(s.snapshot(), before);
}

// Isolation: the binary never wrote to the real home.
#[test]
fn nothing_is_written_outside_the_sandbox_home() {
    let s = Sandbox::new();
    s.write("notes.md", b"one\n");
    s.transfer("notes.md", "user", "adopted");
    assert!(
        !s.dir.path().join(".statecraft").exists(),
        "the binary wrote to $HOME/.statecraft despite STATECRAFT_HOME"
    );
}

// Rule 3: on a case- or normalization-insensitive volume a second spelling of
// one file is refused, so it cannot be recorded twice.
#[test]
fn a_second_spelling_of_one_file_is_refused() {
    let s = Sandbox::new();
    let root = s.root();
    s.write("Notes.md", b"one\n");
    if !s.project().join("notes.md").exists() {
        eprintln!("skipped: this volume is case-sensitive, so `notes.md` is another file");
        return;
    }
    s.refused(
        "spelling",
        &["transfer", "plan", &root, "notes.md", "user", "adopted"],
    );
    s.transfer("Notes.md", "user", "adopted");
    s.refused(
        "spelling",
        &["transfer", "plan", &root, "notes.md", "user", "adopted"],
    );
}

// Rule 5 through the binary: plan lists the disagreement and exits 0; apply
// and revert refuse.
#[test]
fn a_journal_that_disagrees_is_listed_by_plan_and_refused_by_apply_and_revert() {
    let s = Sandbox::new();
    let root = s.root();
    s.write("notes.md", b"one\n");
    s.write("other.md", b"two\n");
    let id = s.transfer("notes.md", "user", "adopted");
    // Someone removes the entry by hand; the journal still says `adopted`.
    let mut m = s.manifest();
    m["entries"]
        .as_array_mut()
        .unwrap()
        .retain(|e| e["path"] != "notes.md");
    s.write(
        ".statecraft/environment.json",
        format!("{}\n", serde_json::to_string_pretty(&m).unwrap()).as_bytes(),
    );

    let plan = s.plan("other.md", "user", "adopted");
    let listed = plan["value"]["journal_disagreements"].as_array().unwrap();
    assert_eq!(listed.len(), 1, "{plan}");
    assert!(listed[0].as_str().unwrap().contains("notes.md"));
    let human = s.run(&["transfer", "plan", &root, "other.md", "user", "adopted"]);
    assert_eq!(code(&human), 0);
    assert!(
        stdout(&human).contains("journal disagrees"),
        "{}",
        stdout(&human)
    );

    let token = plan["value"]["plan_id"].as_str().unwrap().to_string();
    s.refused(
        "journal-disagrees",
        &[
            "transfer", "apply", &root, "other.md", "user", "adopted", &token, "bart", "why",
        ],
    );
    s.refused(
        "journal-disagrees",
        &["transfer", "revert", &root, &id, "bart", "undo"],
    );
}

// Another holder of the manifest lock: refused, `busy`, nothing written.
#[test]
fn a_transfer_while_another_process_holds_the_manifest_lock_is_refused_busy() {
    let s = Sandbox::new();
    let root = s.root();
    s.write("notes.md", b"one\n");
    let id = s.plan_id("notes.md", "user", "adopted");
    let held =
        statecraft_environment::manifest::lock(&s.project(), std::time::Duration::ZERO).unwrap();
    s.refused(
        "busy",
        &[
            "transfer", "apply", &root, "notes.md", "user", "adopted", &id, "bart", "why",
        ],
    );
    drop(held);
    let (c, v) = s.apply_with("notes.md", "user", "adopted", &id);
    assert_eq!(c, 0, "{v}");
}

// A file restored at a path `env remove` removed does not make the journal
// disagree.
#[test]
fn a_file_restored_after_env_remove_does_not_block_transfers() {
    let s = Sandbox::with(&[(OWNED, b"the user's copy\n")]);
    let root = s.root();
    if !s.claiming() {
        eprintln!("skipped: no path is managed where the adapter does not claim its paths");
        return;
    }
    s.transfer(OWNED, "user", "managed");
    let (c, v) = s.json(&["env", "apply", &root]);
    assert_eq!(c, 0, "{v}");
    let (c, v) = s.json(&["env", "remove", &root]);
    assert!(c <= 1, "{v}");
    assert!(!s.project().join(OWNED).exists());
    s.write(OWNED, b"restored by the user\n");
    s.write("notes.md", b"one\n");
    let plan = s.plan("notes.md", "user", "adopted");
    assert!(
        plan["value"]["journal_disagreements"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{plan}"
    );
    let token = plan["value"]["plan_id"].as_str().unwrap().to_string();
    let (c, v) = s.apply_with("notes.md", "user", "adopted", &token);
    assert_eq!(c, 0, "{v}");
}

// Rule 1 on every platform: with no qualification record the adapter claims
// nothing, so this product would not itself write its paths.
#[test]
fn user_to_managed_is_refused_where_the_adapter_does_not_claim_its_paths() {
    let s = Sandbox::with(&[(OWNED, b"the user's copy\n")]);
    std::fs::remove_file(s.home().join("qualifications.json")).unwrap();
    assert!(!s.claiming());
    let root = s.root();
    s.refused(
        "adapter-not-claiming",
        &["transfer", "plan", &root, OWNED, "user", "managed"],
    );
    let (c, v) = s.json(&["transfer", "plan", &root, OWNED, "user", "adopted"]);
    assert_eq!(c, 0, "adoption needs no source: {v}");
}

// Rule 4 as written: `transfer apply` takes the bare plan identity as well as
// the token.
#[test]
fn the_bare_plan_identity_is_accepted_by_the_binary() {
    let s = Sandbox::new();
    s.write("notes.md", b"one\n");
    let plan = s.plan("notes.md", "user", "adopted");
    let bare = plan["value"]["identity"].as_str().unwrap().to_string();
    assert_eq!(bare.len(), 64);
    let (c, v) = s.apply_with("notes.md", "user", "adopted", &bare);
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["value"]["result"], "applied");
}
