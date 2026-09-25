//! The setup profile through the built binary: spec 002 section 5,
//! 2026-09-24, the setup-profile entry, acceptance obligations 1, 2, 3, 4,
//! 6 and 9, and `doctor --remote`'s six results.
//!
//! Every test runs `statecraft-cli` on a real directory with an isolated
//! product home. Obligation 1 runs the rendered scripts with the **real**
//! pinned spec-spine (the repository-local `.tooling/bin` copy, the same
//! binary this repository's gate uses) and the real cargo. `doctor --remote`
//! reads from a stub `gh` on `PATH`; nothing here reaches a host, a provider
//! or the network.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PROFILE: &str = "github-actions-rust";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

/// The pinned spec-spine this repository is governed by. Absent, the suite
/// fails and says what to run: it asserts against the real tool.
fn spec_spine() -> PathBuf {
    let local = repo_root().join(".tooling/bin/spec-spine");
    assert!(
        local.is_file(),
        "no .tooling/bin/spec-spine; run `make tools`. This suite runs the rendered gate with the real pinned tool."
    );
    local
}

/// The exact version this repository pins, read from its single stated
/// source rather than restated here.
fn pinned_version() -> String {
    let text = std::fs::read_to_string(repo_root().join("spec-spine.toml")).unwrap();
    statecraft_home::setup::exact_pin(&text).unwrap()
}

/// The producer's own `spec-spine.toml`, with `[meta]` and an exact pin
/// uncommented: what a project's operator writes before selecting the
/// profile (decision S-4), until the producer scaffolds an exact pin itself.
fn pinned_toml() -> String {
    let starter = statecraft_home::producer::produce(&statecraft_home::producer::Library).unwrap();
    let text = &starter
        .governance
        .iter()
        .find(|f| f.rel_path == "spec-spine.toml")
        .unwrap()
        .contents;
    let version = pinned_version();
    let out: Vec<String> = text
        .lines()
        .map(|l| {
            if l.trim() == "# [meta]" {
                "[meta]".to_string()
            } else if l.trim_start().starts_with("# required_version") {
                format!("required_version = \"={version}\"")
            } else {
                l.to_string()
            }
        })
        .collect();
    let toml = out.join("\n") + "\n";
    assert_eq!(statecraft_home::setup::exact_pin(&toml).unwrap(), version);
    toml
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    /// A minimal Rust repository with every local prerequisite: an exact pin,
    /// `rust-toolchain.toml`, `Cargo.lock` and a git work tree.
    fn new() -> Fixture {
        let f = Fixture {
            dir: tempfile::tempdir().unwrap(),
        };
        let p = f.project();
        std::fs::create_dir_all(p.join("src")).unwrap();
        std::fs::write(
            p.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n",
        )
        .unwrap();
        std::fs::write(
            p.join("src/lib.rs"),
            "/// One.\npub fn one() -> u32 {\n    1\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn one() {\n        assert_eq!(super::one(), 1);\n    }\n}\n",
        )
        .unwrap();
        std::fs::copy(
            repo_root().join("rust-toolchain.toml"),
            p.join("rust-toolchain.toml"),
        )
        .unwrap();
        std::fs::write(p.join("spec-spine.toml"), pinned_toml()).unwrap();
        std::fs::write(p.join(".gitignore"), "/target\n").unwrap();
        std::fs::write(p.join("README.md"), "# demo\n").unwrap();
        let lock = Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(&p)
            .output()
            .unwrap();
        assert!(
            lock.status.success(),
            "{}",
            String::from_utf8_lossy(&lock.stderr)
        );
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["add", "-A"],
            vec!["commit", "--quiet", "-m", "base"],
        ] {
            git(&p, &args);
        }
        // The pinned tool, where the rendered install script looks for it,
        // so no network is needed to satisfy the pin.
        std::fs::create_dir_all(p.join(".tooling/bin")).unwrap();
        std::fs::copy(spec_spine(), p.join(".tooling/bin/spec-spine")).unwrap();
        f
    }

    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn at(&self, rel: &str) -> PathBuf {
        self.project().join(rel)
    }

    /// Run the binary with an isolated product home and native root. Cargo
    /// and rustup keep their own homes, so `--verify-local` can build.
    fn cli(&self, args: &[&str], extra_env: &[(&str, &str)]) -> Output {
        let real_home = std::env::var("HOME").unwrap_or_default();
        let cargo_home =
            std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{real_home}/.cargo"));
        let rustup_home =
            std::env::var("RUSTUP_HOME").unwrap_or_else(|_| format!("{real_home}/.rustup"));
        let path = format!(
            "{}:{}:{}",
            self.dir.path().join("bin").display(),
            spec_spine().parent().unwrap().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"));
        cmd.args(args)
            .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
            .env("STATECRAFT_HOME", self.dir.path().join("home"))
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .env("CARGO_HOME", cargo_home)
            .env("RUSTUP_HOME", rustup_home)
            .env("PATH", path)
            .env("USER", "fixture-operator");
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }

    fn init(&self, verb: &str, flags: &[&str]) -> (i32, serde_json::Value, String) {
        let project = self.project().display().to_string();
        let mut args = vec!["init", verb, project.as_str()];
        args.extend_from_slice(flags);
        args.push("--json");
        let out = self.cli(&args, &[]);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let answer: serde_json::Value =
            serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {text}"));
        let report = answer["value"]["value"].clone();
        (out.status.code().unwrap(), report, text)
    }

    /// Every file under the project outside `.git`, `target` and the runtime
    /// state, with its digest.
    fn walk(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let base = self.project();
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                let rel = path.strip_prefix(&base).unwrap().display().to_string();
                // build-meta.json is the corpus tool's wall-clock record,
                // gitignored and rewritten by every compile.
                if rel == ".git"
                    || rel == "target"
                    || rel.starts_with(".statecraft/state")
                    || rel.ends_with("build-meta.json")
                {
                    continue;
                }
                if entry.file_type().unwrap().is_dir() {
                    stack.push(path);
                } else {
                    let bytes = std::fs::read(&path).unwrap();
                    out.insert(rel, statecraft_environment::digest::digest_bytes(&bytes));
                }
            }
        }
        out
    }
}

fn setup_file<'a>(report: &'a serde_json::Value, path: &str) -> &'a serde_json::Value {
    report["setup"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == path)
        .unwrap_or_else(|| panic!("{path} is not in the setup plan: {report}"))
}

fn result<'a>(report: &'a serde_json::Value, name: &str) -> &'a str {
    report["setup"]["results"][name]["state"].as_str().unwrap()
}

fn sh(dir: &Path, args: &[&str]) -> Output {
    Command::new("sh")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

#[test]
fn a_fresh_rust_repository_passes_its_rendered_gate() {
    let f = Fixture::new();
    let (exit, report, text) = f.init("apply", &["--profile", PROFILE, "--verify-local"]);
    assert_eq!(exit, 0, "{text}");
    assert_eq!(report["outcome"], "complete", "{text}");
    assert_eq!(result(&report, "files-installed"), "satisfied", "{text}");
    assert_eq!(result(&report, "local-checks"), "satisfied", "{text}");
    for remote in [
        "remote-prerequisites",
        "required-checks",
        "ci-executed",
        "ai-review-produced",
    ] {
        assert_eq!(result(&report, remote), "unverified", "{remote}");
    }
    // The rendered scripts, run by hand with the real pinned tool and cargo.
    let p = f.project();
    for args in [
        vec!["scripts/statecraft/install-spec-spine.sh"],
        vec!["scripts/statecraft/gate.sh", "governance"],
        vec!["scripts/statecraft/gate.sh", "code"],
    ] {
        let out = sh(&p, &args);
        assert!(
            out.status.success(),
            "{args:?}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // The ignore fragment was merged, the declaration records the selection,
    // and a Makefile was rendered because none existed.
    let ignore = std::fs::read_to_string(f.at(".gitignore")).unwrap();
    assert!(ignore.starts_with("/target\n"), "{ignore}");
    assert!(ignore.contains("\n.tooling/\n"), "{ignore}");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.at(".statecraft/environment.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["project"]["setup"]["profile"], PROFILE);
    assert_eq!(setup_file(&report, "Makefile")["action"], "write");
    assert!(
        !f.at(".statecraft/state/setup/apply.json").exists(),
        "the resume record is removed"
    );
}

#[test]
fn an_unpinned_project_withholds_the_profile_and_names_the_prerequisite() {
    let f = Fixture::new();
    let text = std::fs::read_to_string(f.at("spec-spine.toml")).unwrap();
    std::fs::write(
        f.at("spec-spine.toml"),
        text.replace("required_version = \"=", "required_version = \"^"),
    )
    .unwrap();
    let (exit, report, out) = f.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 1, "{out}");
    assert_eq!(report["outcome"], "partial");
    let withheld = report["setup"]["withheld"].as_str().unwrap();
    assert!(withheld.contains("not an exact pin"), "{withheld}");
    assert!(!f.at(".github/workflows/statecraft-ci.yml").exists());
    assert_eq!(result(&report, "files-installed"), "withheld");
}

#[test]
fn authored_files_are_preserved_and_named() {
    let f = Fixture::new();
    let authored = [
        (
            ".github/workflows/ci.yml",
            "name: mine\non: push\njobs: {}\n",
        ),
        (
            ".github/workflows/statecraft-ci.yml",
            "name: already mine\n",
        ),
        ("Makefile", "all:\n\techo mine\n"),
        ("CODEOWNERS", "* @someone\n"),
    ];
    for (rel, text) in authored {
        std::fs::create_dir_all(f.at(rel).parent().unwrap()).unwrap();
        std::fs::write(f.at(rel), text).unwrap();
    }
    std::fs::write(f.at(".gitignore"), "/target\n*.log\n").unwrap();
    git(&f.project(), &["add", "-A"]);
    git(&f.project(), &["commit", "--quiet", "-m", "authored"]);
    let before = f.walk();
    let (exit, report, text) = f.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 1, "{text}");
    assert_eq!(report["outcome"], "partial", "{text}");
    let after = f.walk();
    // Every authored file is byte-identical, and so is every unrelated one.
    for (rel, digest) in &before {
        if rel == ".gitignore" {
            continue;
        }
        assert_eq!(after.get(rel), Some(digest), "{rel} changed");
    }
    let ignore = std::fs::read_to_string(f.at(".gitignore")).unwrap();
    assert!(ignore.starts_with("/target\n*.log\n"), "{ignore}");
    let conflict = setup_file(&report, ".github/workflows/statecraft-ci.yml");
    assert_eq!(conflict["action"], "conflict");
    assert_eq!(conflict["kind"], "existing-authored");
    assert_eq!(setup_file(&report, "Makefile")["action"], "left-alone");
    assert_eq!(setup_file(&report, "CODEOWNERS")["action"], "left-alone");
    let governance = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["step"] == "governance")
        .unwrap();
    assert_eq!(governance["state"]["state"], "withheld");
    assert!(
        governance["state"]["reason"]
            .as_str()
            .unwrap()
            .contains("statecraft-ci.yml"),
        "{governance}"
    );
    // The profile's other files were written.
    assert!(f.at("scripts/statecraft/gate.sh").exists());
}

#[test]
fn a_repeat_apply_changes_no_project_file() {
    let f = Fixture::new();
    let (exit, _, text) = f.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 0, "{text}");
    let before = f.walk();
    let profile_record = |f: &Fixture| -> serde_json::Value {
        let m: serde_json::Value =
            serde_json::from_slice(&std::fs::read(f.at(".statecraft/environment.json")).unwrap())
                .unwrap();
        let entries: Vec<serde_json::Value> = m["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| {
                e["source"]["identity"]
                    .as_str()
                    .unwrap()
                    .starts_with("statecraft-setup:")
            })
            .cloned()
            .collect();
        serde_json::json!({"entries": entries, "setup": m["project"]["setup"]})
    };
    let recorded = profile_record(&f);
    // Without naming the profile: the declaration's selection re-plans it.
    let (exit, report, text) = f.init("apply", &[]);
    assert_eq!(exit, 0, "{text}");
    // No project file changes. The declaration is compared by its profile
    // records: on this base the governance step re-dates some of its own
    // entries on a repeat run, which is that step's behavior, not the
    // profile's, and is reported separately.
    let after = f.walk();
    for (rel, digest) in &before {
        if rel == ".statecraft/environment.json" {
            continue;
        }
        assert_eq!(after.get(rel), Some(digest), "a repeat apply changed {rel}");
    }
    assert_eq!(before.len(), after.len(), "a repeat apply added a file");
    assert_eq!(
        recorded,
        profile_record(&f),
        "a repeat apply changed the profile's record"
    );
    for file in report["setup"]["files"].as_array().unwrap() {
        assert!(
            file["action"] == "unchanged" || file["action"] == "left-alone",
            "{file}"
        );
    }
    let project_mutations: Vec<&serde_json::Value> = report["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| {
            let path = m["path"].as_str().unwrap();
            m["category"] == "project"
                && !path.ends_with("build-meta.json")
                && path != ".statecraft/environment.json"
        })
        .collect();
    assert!(project_mutations.is_empty(), "{project_mutations:?}");
}

/// Whether a directory's permission bits stop this process writing in it
/// (they do not for root).
fn permissions_refuse(dir: &Path) -> bool {
    std::fs::write(dir.join(".probe"), "x").is_err()
}

#[test]
fn an_interrupted_apply_resumes_to_the_same_tree() {
    // The uninterrupted reference.
    let reference = Fixture::new();
    let (exit, _, text) = reference.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 0, "{text}");

    // Interrupted after its second write: the scripts directory refuses.
    let f = Fixture::new();
    let scripts = f.at("scripts/statecraft");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::set_permissions(&scripts, std::fs::Permissions::from_mode(0o555)).unwrap();
    if !permissions_refuse(&scripts) {
        eprintln!("skipped: permission bits do not refuse this user (root)");
        return;
    }
    let (exit, report, text) = f.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 4, "{text}");
    assert_eq!(report["outcome"], "failed", "{text}");
    assert!(
        f.at(".statecraft/state/setup/apply.json").exists(),
        "the resume record stays"
    );
    assert!(f.at(".github/workflows/statecraft-ci.yml").exists());
    assert!(
        !f.at(".statecraft/environment.json").exists(),
        "the manifest is written last"
    );
    std::fs::set_permissions(&scripts, std::fs::Permissions::from_mode(0o755)).unwrap();

    let (exit, report, text) = f.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 0, "{text}");
    assert_eq!(
        setup_file(&report, ".github/workflows/statecraft-ci.yml")["action"],
        "resumed"
    );
    assert!(!f.at(".statecraft/state/setup/apply.json").exists());
    let profile_paths: Vec<String> = report["setup"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["action"] != "left-alone")
        .map(|f| f["path"].as_str().unwrap().to_string())
        .collect();
    for rel in &profile_paths {
        assert_eq!(
            std::fs::read(f.at(rel)).unwrap(),
            std::fs::read(reference.at(rel)).unwrap(),
            "{rel} differs from the uninterrupted run"
        );
    }
    let entries = |dir: &Fixture| -> BTreeMap<String, String> {
        let m: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.at(".statecraft/environment.json")).unwrap())
                .unwrap();
        m["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| {
                e["source"]["identity"]
                    .as_str()
                    .unwrap()
                    .starts_with("statecraft-setup:")
            })
            .map(|e| {
                (
                    e["path"].as_str().unwrap().to_string(),
                    format!("{} {}", e["class"], e["digest"]),
                )
            })
            .collect()
    };
    assert_eq!(entries(&f), entries(&reference));
}

#[test]
fn a_file_edited_after_an_interruption_is_a_conflict() {
    let f = Fixture::new();
    let scripts = f.at("scripts/statecraft");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::set_permissions(&scripts, std::fs::Permissions::from_mode(0o555)).unwrap();
    if !permissions_refuse(&scripts) {
        eprintln!("skipped: permission bits do not refuse this user (root)");
        return;
    }
    let (exit, _, text) = f.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 4, "{text}");
    std::fs::set_permissions(&scripts, std::fs::Permissions::from_mode(0o755)).unwrap();
    let edited = f.at(".github/workflows/statecraft-ci.yml");
    std::fs::write(&edited, "name: edited in between\n").unwrap();
    let (exit, report, text) = f.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 1, "{text}");
    let file = setup_file(&report, ".github/workflows/statecraft-ci.yml");
    assert_eq!(file["kind"], "edited-after-interruption", "{file}");
    assert_eq!(
        std::fs::read_to_string(&edited).unwrap(),
        "name: edited in between\n"
    );
    assert!(
        f.at(".statecraft/state/setup/.github/workflows/statecraft-ci.yml.intended")
            .exists()
    );
}

#[test]
fn an_approved_plan_refuses_when_an_input_changed() {
    let f = Fixture::new();
    // A plan of a fresh project cannot register it (there is no corpus yet),
    // which is the flow's own business; the setup plan is what is approved.
    let (_, plan, _) = f.init("plan", &["--profile", PROFILE]);
    let identity = plan["setup"]["planIdentity"].as_str().unwrap().to_string();
    assert!(plan["mutations"].as_array().unwrap().is_empty());
    // An input changes after the plan was approved.
    std::fs::write(f.at("README.md"), "# demo\n").unwrap();
    let lock = std::fs::read_to_string(f.at("Cargo.lock")).unwrap();
    std::fs::write(f.at("Cargo.lock"), format!("{lock}\n")).unwrap();
    let before = f.walk();
    let (exit, report, text) = f.init("apply", &["--profile", PROFILE, "--plan", &identity]);
    assert_eq!(exit, 2, "{text}");
    assert_eq!(report["outcome"], "refused");
    assert_eq!(before, f.walk(), "a refused apply wrote a project file");
    let written: Vec<&serde_json::Value> = report["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| {
            !m["path"].as_str().unwrap().ends_with("manifest.lock")
                && m["path"] != ".statecraft/state"
                && m["path"] != ".statecraft"
        })
        .collect();
    assert!(written.is_empty(), "{written:?}");
    // The plan as it now stands, approved, is performed.
    let (_, fresh, _) = f.init("plan", &["--profile", PROFILE]);
    let identity = fresh["setup"]["planIdentity"].as_str().unwrap().to_string();
    let (exit, report, text) = f.init("apply", &["--profile", PROFILE, "--plan", &identity]);
    assert_eq!(exit, 0, "{text}");
    assert_eq!(report["setup"]["planIdentity"], identity);
}

#[test]
fn the_plan_names_the_authority_set_and_the_credential_by_name_only() {
    let f = Fixture::new();
    let (_, plan, _) = f.init("plan", &["--profile", PROFILE]);
    for file in plan["setup"]["files"].as_array().unwrap() {
        let role = file["role"].as_str().unwrap();
        if matches!(role, "workflow" | "script" | "policy") {
            assert_eq!(file["authoritySet"], true, "{file}");
        }
    }
    assert_eq!(plan["setup"]["credential"], "CLAUDE_CODE_OAUTH_TOKEN");
    assert_eq!(
        plan["setup"]["credentialCommand"],
        "gh secret set CLAUDE_CODE_OAUTH_TOKEN"
    );
    // A value in the environment never reaches any output or any file.
    let sentinel = "sentinel-credential-value-5f1c";
    let project = f.project().display().to_string();
    let out = f.cli(
        &["init", "apply", &project, "--profile", PROFILE, "--json"],
        &[("CLAUDE_CODE_OAUTH_TOKEN", sentinel)],
    );
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!text.contains(sentinel));
    let human = f.cli(
        &["init", "plan", &project, "--profile", PROFILE],
        &[("CLAUDE_CODE_OAUTH_TOKEN", sentinel)],
    );
    assert!(!String::from_utf8_lossy(&human.stdout).contains(sentinel));
    for (rel, _) in f.walk() {
        let bytes = std::fs::read(f.at(&rel)).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(sentinel),
            "{rel} carries the credential value"
        );
    }
}

#[test]
fn unknown_parameters_and_profiles_refuse_before_any_write() {
    let f = Fixture::new();
    let before = f.walk();
    let (exit, report, text) = f.init("apply", &["--profile", "gitlab-python"]);
    assert_eq!(exit, 2, "{text}");
    assert_eq!(report["outcome"], "refused");
    assert_eq!(before, f.walk());
    // A usage error, not an operation: a flag the verb does not take.
    let project = f.project().display().to_string();
    let out = f.cli(&["init", "apply", &project, "--profiles", PROFILE], &[]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ------------------------------------------------------------ doctor --remote

const GH: &str = r#"#!/usr/bin/env bash
here="$(dirname "$0")"
printf '%s\n' "$*" >> "$here/gh-calls"
[ "$1 $2 $3" = "api -X GET" ] || { echo "gh stub: not a read: $*" >&2; exit 97; }
mode="$(cat "$here/gh-mode")"
if [ "$mode" = down ]; then echo "error connecting to api.github.com" >&2; exit 1; fi
path="$4"
case "$path" in
  */actions/secrets/CLAUDE_CODE_OAUTH_TOKEN)
    if [ "$mode" = partial ]; then echo "gh: Not Found (HTTP 404)" >&2; exit 1; fi
    echo '{"name":"CLAUDE_CODE_OAUTH_TOKEN","created_at":"2026-09-24T00:00:00Z"}' ;;
  */actions/permissions) echo '{"enabled":true,"allowed_actions":"all"}' ;;
  */actions/permissions/workflow) echo '{"default_workflow_permissions":"read"}' ;;
  */environments/statecraft-review-exception)
    echo '{"name":"statecraft-review-exception","protection_rules":[{"type":"required_reviewers"}]}' ;;
  */branches/main/protection)
    if [ "$mode" = partial ]; then echo "gh: Branch not protected (HTTP 404)" >&2; exit 1; fi
    if [ "$mode" = unbound ]; then
      echo '{"required_status_checks":{"strict":true,"contexts":["ci-gate"],"checks":[{"context":"ci-gate","app_id":null}]},"required_pull_request_reviews":{"require_code_owner_reviews":true,"required_approving_review_count":0}}'
      exit 0
    fi
    echo '{"required_status_checks":{"strict":true,"contexts":["ci-gate"],"checks":[{"context":"ci-gate","app_id":15368}]},"required_pull_request_reviews":{"require_code_owner_reviews":true,"required_approving_review_count":0}}' ;;
  */check-runs?check_name=ci-gate) echo '{"total_count":1,"check_runs":[{"name":"ci-gate","status":"completed","conclusion":"success"}]}' ;;
  */actions/artifacts?name=statecraft-ai-review-*) echo '{"total_count":1,"artifacts":[{"id":42,"name":"x"}]}' ;;
  *) echo "gh stub: unexpected $path" >&2; exit 96 ;;
esac
"#;

/// `doctor --remote` against the stub `gh`, which is always first on `PATH`
/// so no real host is ever asked. `down` is a host that cannot be reached.
fn doctor_remote(f: &Fixture, mode: &str) -> (serde_json::Value, String, String) {
    let bin = f.dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    if !bin.join("gh").exists() {
        statecraft_adapter::fixture::install_script(&bin.join("gh"), GH, 0o755).unwrap();
    }
    std::fs::write(bin.join("gh-mode"), mode).unwrap();
    let project = f.project().display().to_string();
    let out = f.cli(&["doctor", &project, "--remote", "--json"], &[]);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let answer: serde_json::Value =
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {text}"));
    let calls = std::fs::read_to_string(bin.join("gh-calls")).unwrap_or_default();
    (answer["value"]["setup"].clone(), calls, text)
}

fn applied_with_remote() -> Fixture {
    let f = Fixture::new();
    git(
        &f.project(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/fixture.git",
        ],
    );
    let (exit, _, text) = f.init("apply", &["--profile", PROFILE]);
    assert_eq!(exit, 0, "{text}");
    f
}

#[test]
fn doctor_remote_reads_the_six_results_and_writes_nothing() {
    let f = applied_with_remote();
    let before = f.walk();
    let (setup, calls, text) = doctor_remote(&f, "full");
    assert_eq!(setup["files-installed"]["state"], "satisfied", "{text}");
    assert_eq!(setup["local-checks"]["state"], "not-run", "{text}");
    for name in [
        "remote-prerequisites",
        "required-checks",
        "ci-executed",
        "ai-review-produced",
    ] {
        assert_eq!(setup[name]["state"], "satisfied", "{name}: {text}");
    }
    // Every host call was a read.
    for call in calls.lines() {
        assert!(
            call.starts_with("api -X GET repos/owner/fixture/"),
            "{call}"
        );
    }
    assert_eq!(before, f.walk(), "doctor --remote changed a project file");

    let (setup, _, text) = doctor_remote(&f, "partial");
    let prereq = &setup["remote-prerequisites"];
    assert_eq!(prereq["state"], "not-satisfied", "{text}");
    assert!(
        prereq["detail"]
            .as_str()
            .unwrap()
            .contains("CLAUDE_CODE_OAUTH_TOKEN is not set")
    );
    assert_eq!(setup["required-checks"]["state"], "not-satisfied", "{text}");

    // Revision 2, item 3: a ci-gate any token could report is not the
    // required check the profile asks for.
    let (setup, _, text) = doctor_remote(&f, "unbound");
    let required = &setup["required-checks"];
    assert_eq!(required["state"], "not-satisfied", "{text}");
    assert!(
        required["detail"]
            .as_str()
            .unwrap()
            .contains("ci-gate bound to GitHub Actions (app id 15368): false"),
        "{text}"
    );

    // No host to ask: unverified, never success.
    let (setup, _, text) = doctor_remote(&f, "down");
    for name in [
        "remote-prerequisites",
        "required-checks",
        "ci-executed",
        "ai-review-produced",
    ] {
        assert_eq!(setup[name]["state"], "unverified", "{name}: {text}");
    }
}
