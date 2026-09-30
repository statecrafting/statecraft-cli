//! Governed inputs for a complete first bootstrap: spec 018, through the built
//! binary.
//!
//! The acceptance anchor is section 2's measured gap: a fresh repository with
//! no toolchain files, planned and then applied with an explicit setup input.
//! Every test runs `statecraft-cli` on a real directory with an isolated
//! product home and the real pinned spec-spine from `.tooling/bin` (`make
//! tools`); nothing here reaches a host, a provider or the network.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PROFILE: &str = "github-actions-rust";
const CHECKER: &str = "scripts/check-authored-content.sh";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn spec_spine() -> PathBuf {
    let local = repo_root().join(".tooling/bin/spec-spine");
    assert!(
        local.is_file(),
        "no .tooling/bin/spec-spine; run `make tools`. This suite runs the real pinned tool."
    );
    local
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

fn digest(bytes: &[u8]) -> String {
    statecraft_environment::digest::digest_bytes(bytes)
}

/// The doc-manus shape of section 2: a git work tree holding a README and
/// nothing a Rust gate needs.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        let f = Fixture {
            dir: tempfile::tempdir().unwrap(),
        };
        let p = f.project();
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("README.md"), "# fresh\n").unwrap();
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
        f
    }

    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.project().join(rel)
    }

    /// Write a setup input document outside the target (section 3.1: an
    /// empty repository need not contain a temporary control file).
    fn input(&self, name: &str, document: &serde_json::Value) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::write(&path, serde_json::to_vec_pretty(document).unwrap()).unwrap();
        path
    }

    fn cli(&self, args: &[&str]) -> Output {
        let path = format!(
            "{}:{}",
            spec_spine().parent().unwrap().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env("STATECRAFT_HOME", self.dir.path().join("home"))
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .env("PATH", path)
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    /// `init <verb> <project> <flags> --json`: the exit, the envelope and the
    /// text for a failure message.
    fn init(&self, verb: &str, flags: &[&str]) -> (i32, serde_json::Value, String) {
        let project = self.project().display().to_string();
        let mut args = vec!["init", verb, project.as_str()];
        args.extend_from_slice(flags);
        args.push("--json");
        let out = self.cli(&args);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let answer: serde_json::Value =
            json_naming::from_output(&out.stdout).unwrap_or_else(|e| panic!("{e}: {text}"));
        (out.status.code().unwrap(), answer, text)
    }

    /// Every file under the project outside `.git` and the runtime state,
    /// with its digest.
    fn walk(&self) -> std::collections::BTreeMap<String, String> {
        let mut out = std::collections::BTreeMap::new();
        let base = self.project();
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                let rel = path.strip_prefix(&base).unwrap().display().to_string();
                if rel == ".git"
                    || rel.starts_with(".statecraft/state")
                    || rel.ends_with("build-meta.json")
                {
                    continue;
                }
                if entry.file_type().unwrap().is_dir() {
                    stack.push(path);
                } else {
                    out.insert(rel, digest(&std::fs::read(&path).unwrap()));
                }
            }
        }
        out
    }
}

/// The initialization report inside the envelope.
fn report(answer: &serde_json::Value) -> &serde_json::Value {
    &json_naming::payload(answer)["value"]
}

/// The operator's reviewed choices, section 3.1's example less the parameters
/// a repository without a checker would not declare.
fn choices() -> serde_json::Value {
    serde_json::json!({
        "schema": "statecraft/setup-input/1",
        "profile": PROFILE,
        "parameters": {
            "review.code_owners": ["@owner"],
            "governance.enforce_coverage": true,
            "governance.authored_content": CHECKER,
            "governance.authored_content_text": true,
            "governance.gate_each_commit": true,
            "governance.require_signed_commits": true
        }
    })
}

fn prerequisite<'a>(report: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    report["setup"]["prerequisites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"].as_str().is_some_and(|n| n.contains(name)))
        .unwrap_or_else(|| panic!("no prerequisite {name}: {report}"))
}

fn effective<'a>(report: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    report["setup"]["effective"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("no effective parameter {name}: {report}"))
}

fn plan_identity(report: &serde_json::Value) -> String {
    report["setup"]["planIdentity"]
        .as_str()
        .unwrap_or_else(|| panic!("no setup plan identity: {report}"))
        .to_string()
}

/// The acceptance anchor (section 2): a fresh repository with no toolchain
/// files, planned then applied with a governed input document. The plan shows
/// the operator's choices before anything is written; the governance scaffold
/// carries the producer's exact pin; the profile is withheld, naming every
/// unmet project-owned prerequisite; nothing the project owns is invented; and
/// planning again converges.
#[test]
fn a_fresh_repository_without_toolchain_files_is_planned_and_applied_honestly() {
    let f = Fixture::new();
    let input = f.input("setup-input.json", &choices());
    let input_arg = input.display().to_string();
    let bytes = std::fs::read(&input).unwrap();
    let before = f.walk();

    // 3.2: the plan reports the input and every effective parameter, and
    // writes nothing.
    let (exit, answer, text) = f.init("plan", &["--setup-input", &input_arg]);
    assert_eq!(exit, 1, "a withheld profile is partial: {text}");
    let plan = report(&answer);
    assert_eq!(plan["outcome"], "partial", "{text}");
    assert_eq!(f.walk(), before, "a plan writes nothing");
    let setup = &plan["setup"];
    assert_eq!(
        setup["profile"], PROFILE,
        "the document selects its profile"
    );
    assert_eq!(setup["input"]["supplied"], true, "{setup}");
    assert_eq!(setup["input"]["schema"], "statecraft/setup-input/1");
    assert_eq!(setup["input"]["digest"], digest(&bytes));
    assert!(
        !plan
            .to_string()
            .contains("\"parameters\":{\"review.code_owners\""),
        "the report does not reproduce the input bytes"
    );
    let owners = effective(plan, "review.code_owners");
    assert_eq!(owners["value"], serde_json::json!(["@owner"]));
    assert_eq!(owners["provenance"], "setup-input");
    let unresolved = effective(plan, "governance.fail_on_unresolved");
    assert_eq!(unresolved["value"], true, "the default is kept");
    assert_eq!(unresolved["provenance"], "default");
    assert_eq!(
        effective(plan, "governance.gate_each_commit")["value"],
        true
    );

    // 3.3: a new spec-spine.toml carries the producer's exact pin.
    let pin = prerequisite(plan, "spec-spine pin");
    assert_eq!(pin["met"], true, "{pin}");
    assert_eq!(pin["owner"], "governance producer", "{pin}");

    // 3.4: each project-owned prerequisite is reported with its owner, and
    // any unmet one withholds the whole profile.
    for name in ["rust-toolchain.toml", "Cargo.lock"] {
        let p = prerequisite(plan, name);
        assert_eq!(p["met"], false, "{p}");
        assert_eq!(p["owner"], "target project", "{p}");
        assert!(p["consequence"].as_str().is_some_and(|c| !c.is_empty()));
    }
    let checker = prerequisite(plan, "authored-content checker");
    assert_eq!(checker["met"], false, "{checker}");
    assert!(
        checker["observed"].as_str().unwrap().contains(CHECKER),
        "the exact path is named: {checker}"
    );
    assert_eq!(prerequisite(plan, "git work tree")["met"], true);
    assert!(setup["withheld"].is_string(), "{setup}");
    let identity = plan_identity(plan);

    // 3.5: apply with a setup input requires the reviewed plan.
    let (exit, _, text) = f.init("apply", &["--setup-input", &input_arg]);
    assert_eq!(
        exit, 3,
        "a setup input without --plan is a usage refusal: {text}"
    );
    assert_eq!(f.walk(), before, "nothing is written");

    let (exit, answer, text) = f.init("apply", &["--setup-input", &input_arg, "--plan", &identity]);
    assert_eq!(exit, 1, "partial: {text}");
    let applied = report(&answer);
    assert_eq!(applied["outcome"], "partial", "{text}");
    let governance = applied["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["step"] == "governance")
        .unwrap();
    assert_eq!(governance["state"]["state"], "withheld", "{governance}");

    // The exact pin, written from the producer's bytes.
    let toml = std::fs::read_to_string(f.at("spec-spine.toml")).unwrap();
    assert_eq!(
        statecraft_home::setup::exact_pin(&toml).unwrap(),
        statecraft_home::producer::PRODUCER_VERSION
    );
    // I-2: nothing the project owns is invented, and no resolver ran.
    for absent in ["rust-toolchain.toml", "Cargo.lock", "Cargo.toml", CHECKER] {
        assert!(!f.at(absent).exists(), "{absent} was created");
    }
    assert!(!f.at(".github/workflows/statecraft-ci.yml").exists());
    assert!(!f.at(".statecraft/setup").exists());
    // Withheld intent is reported, not persisted.
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.at(".statecraft/environment.json")).unwrap())
            .unwrap();
    assert!(
        manifest["project"].get("setup").is_none_or(|s| s.is_null()),
        "no setup selection is recorded while withheld: {manifest}"
    );

    // Convergence: planning again with the same input proposes no write, and
    // twice gives one identity.
    let after = f.walk();
    let (_, first, text) = f.init("plan", &["--setup-input", &input_arg]);
    let (_, second, _) = f.init("plan", &["--setup-input", &input_arg]);
    assert_eq!(f.walk(), after, "a plan writes nothing");
    let first = report(&first);
    assert_eq!(
        first["writes"],
        serde_json::json!([]),
        "a converged partial plan proposes no write: {text}"
    );
    assert_eq!(plan_identity(first), plan_identity(report(&second)));
    assert_eq!(
        effective(first, "review.code_owners")["provenance"],
        "setup-input",
        "the requested parameters stay visible in the report"
    );
}

/// The completion of the same bootstrap once the project supplies what it
/// owns: an exact-plan apply records the normalized parameters, and a second
/// plan proposes zero target writes with every managed file unchanged.
#[test]
fn once_the_project_supplies_its_prerequisites_the_profile_applies_and_converges() {
    let f = Fixture::new();
    let input = f.input("setup-input.json", &choices());
    let input_arg = input.display().to_string();
    let (exit, _, text) = f.init("apply", &[]);
    assert_eq!(exit, 0, "governance alone completes: {text}");

    // The operator supplies the project-owned files, tracked.
    std::fs::copy(
        repo_root().join("rust-toolchain.toml"),
        f.at("rust-toolchain.toml"),
    )
    .unwrap();
    std::fs::write(f.at("Cargo.lock"), "version = 4\n").unwrap();
    std::fs::create_dir_all(f.at("scripts")).unwrap();
    std::fs::write(f.at(CHECKER), "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(f.at(CHECKER), std::fs::Permissions::from_mode(0o755)).unwrap();
    git(&f.project(), &["add", "-A"]);

    let (exit, answer, text) = f.init("plan", &["--setup-input", &input_arg]);
    assert_eq!(exit, 0, "{text}");
    let identity = plan_identity(report(&answer));
    assert!(report(&answer)["setup"]["withheld"].is_null(), "{text}");

    let (exit, answer, text) = f.init("apply", &["--setup-input", &input_arg, "--plan", &identity]);
    assert_eq!(exit, 0, "{text}");
    assert_eq!(report(&answer)["outcome"], "complete");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.at(".statecraft/environment.json")).unwrap())
            .unwrap();
    let recorded = &manifest["project"]["setup"]["parameters"];
    assert_eq!(
        recorded["review.code_owners"],
        serde_json::json!(["@owner"])
    );
    assert_eq!(recorded["governance.enforce_coverage"], true);
    assert!(f.at("CODEOWNERS").exists() || f.at(".github/CODEOWNERS").exists());

    let after = f.walk();
    let (exit, first, text) = f.init("plan", &["--setup-input", &input_arg]);
    assert_eq!(exit, 0, "{text}");
    let (_, second, _) = f.init("plan", &["--setup-input", &input_arg]);
    assert_eq!(f.walk(), after);
    let first = report(&first);
    assert_eq!(first["writes"], serde_json::json!([]), "{text}");
    for file in first["setup"]["files"].as_array().unwrap() {
        let action = file["action"].as_str().unwrap_or_default();
        assert!(
            action == "unchanged" || action == "left-alone",
            "every managed file is unchanged: {file}"
        );
    }
    assert_eq!(plan_identity(first), plan_identity(report(&second)));
    // A plan without the input reads the recorded selection.
    let (exit, answer, text) = f.init("plan", &[]);
    assert_eq!(exit, 0, "{text}");
    assert_eq!(
        effective(report(&answer), "review.code_owners")["provenance"],
        "recorded"
    );
    assert_eq!(report(&answer)["setup"]["input"]["supplied"], false);
}

/// Section 3.6: the input changes after review. The apply refuses the stale
/// plan before every write and names the input's digest.
#[test]
fn an_input_changed_after_review_refuses_the_stale_plan_before_every_write() {
    let f = Fixture::new();
    let input = f.input("setup-input.json", &choices());
    let input_arg = input.display().to_string();
    let (_, answer, text) = f.init("plan", &["--setup-input", &input_arg]);
    let reviewed = plan_identity(report(&answer));

    let mut changed = choices();
    changed["parameters"]["review.code_owners"] = serde_json::json!(["@someone-else"]);
    std::fs::write(&input, serde_json::to_vec_pretty(&changed).unwrap()).unwrap();
    let now = digest(&std::fs::read(&input).unwrap());
    let before = f.walk();

    let (exit, answer, _) = f.init("apply", &["--setup-input", &input_arg, "--plan", &reviewed]);
    assert_eq!(exit, 2, "a stale plan is refused: {text}");
    let refused = report(&answer);
    assert_eq!(refused["outcome"], "refused");
    let reason = refused["steps"].to_string();
    assert!(reason.contains(&now), "the input digest is named: {reason}");
    assert!(reason.contains(&reviewed), "{reason}");
    assert_eq!(f.walk(), before, "nothing is written");
}

/// Section 3.6: `--profile` and the document disagree. A usage refusal, and
/// neither profile is selected.
#[test]
fn an_input_and_a_profile_that_disagree_are_a_usage_refusal() {
    let f = Fixture::new();
    let mut other = choices();
    other["profile"] = serde_json::json!("another-profile");
    let input = f.input("setup-input.json", &other);
    let input_arg = input.display().to_string();
    let before = f.walk();
    for verb in ["plan", "apply"] {
        let mut flags = vec!["--setup-input", input_arg.as_str(), "--profile", PROFILE];
        if verb == "apply" {
            flags.extend(["--plan", "0000"]);
        }
        let (exit, answer, text) = f.init(verb, &flags);
        assert_eq!(exit, 3, "{verb}: {text}");
        assert_eq!(answer["error"]["kind"], "usage", "{text}");
        assert!(
            text.contains("another-profile") && text.contains("disagree"),
            "{text}"
        );
    }
    assert_eq!(f.walk(), before, "nothing is selected or written");
}

/// Section 3.6: a fresh input naming a parameter the profile does not know
/// is refused by the profile's closed validator.
#[test]
fn a_fresh_input_with_an_unknown_parameter_is_refused_by_the_profile() {
    let f = Fixture::new();
    let mut unknown = choices();
    unknown["parameters"]["review.reviewers"] = serde_json::json!(["@x"]);
    let input = f.input("setup-input.json", &unknown);
    let (exit, answer, text) = f.init("plan", &["--setup-input", &input.display().to_string()]);
    assert_eq!(exit, 2, "{text}");
    assert_eq!(report(&answer)["outcome"], "refused");
    assert!(
        text.contains("unknown setup parameter `review.reviewers`"),
        "{text}"
    );
}

/// Section 3.6: a credential or any unknown top-level member refuses the
/// plan, and its value appears nowhere in the answer.
#[test]
fn an_input_carrying_a_credential_is_refused_without_echoing_it() {
    let f = Fixture::new();
    let secret = "sk-ant-not-a-real-credential-0123456789";
    for member in ["CLAUDE_CODE_OAUTH_TOKEN", "consent", "plan"] {
        let mut carrying = choices();
        carrying[member] = serde_json::json!(secret);
        let input = f.input("setup-input.json", &carrying);
        let (exit, answer, text) = f.init("plan", &["--setup-input", &input.display().to_string()]);
        assert_eq!(exit, 2, "{member}: {text}");
        assert_eq!(report(&answer)["outcome"], "refused");
        assert!(text.contains(&format!("`{member}`")), "{text}");
        assert!(!text.contains(secret), "the value is echoed: {text}");
    }
    let mut nested = choices();
    nested["parameters"]["github.token"] = serde_json::json!(secret);
    let input = f.input("setup-input.json", &nested);
    let (exit, _, text) = f.init("plan", &["--setup-input", &input.display().to_string()]);
    assert_eq!(exit, 2, "{text}");
    assert!(!text.contains(secret), "the value is echoed: {text}");
}

/// Section 3.6: an adopted configuration without an exact pin withholds the
/// profile, leaves the initialization partial, and is never rewritten.
#[test]
fn an_adopted_configuration_without_an_exact_pin_withholds_the_profile() {
    let f = Fixture::new();
    // An existing configuration: the project's own, pinned by a range.
    let (exit, _, text) = f.init("apply", &[]);
    assert_eq!(exit, 0, "{text}");
    let exact = std::fs::read_to_string(f.at("spec-spine.toml")).unwrap();
    let version = statecraft_home::producer::PRODUCER_VERSION;
    let adopted = exact.replace(
        &format!("required_version = \"={version}\""),
        &format!("required_version = \"{version}\""),
    );
    assert_ne!(adopted, exact);
    std::fs::write(f.at("spec-spine.toml"), &adopted).unwrap();
    let input = f.input("setup-input.json", &choices());
    let input_arg = input.display().to_string();
    let (exit, answer, text) = f.init("plan", &["--setup-input", &input_arg]);
    assert_eq!(exit, 1, "{text}");
    let pin = prerequisite(report(&answer), "spec-spine pin");
    assert_eq!(pin["met"], false, "{pin}");
    assert_eq!(pin["owner"], "target project");
    assert!(
        pin["observed"]
            .as_str()
            .unwrap()
            .contains(&format!("\"{version}\" is not an exact pin")),
        "the observed value is named: {pin}"
    );
    let identity = plan_identity(report(&answer));

    let (exit, answer, text) = f.init("apply", &["--setup-input", &input_arg, "--plan", &identity]);
    assert_eq!(exit, 1, "{text}");
    assert_eq!(report(&answer)["outcome"], "partial");
    assert_eq!(
        std::fs::read_to_string(f.at("spec-spine.toml")).unwrap(),
        adopted,
        "the adopted file is unchanged"
    );
    assert!(!f.at(".statecraft/setup").exists());
}

/// Section 3.1: without an input, a fresh plan says defaults are in effect
/// and that nobody chose them, and `--profile` keeps its behavior.
#[test]
fn a_fresh_plan_without_an_input_says_the_defaults_are_nobody_s_choice() {
    let f = Fixture::new();
    let (exit, answer, text) = f.init("plan", &["--profile", PROFILE]);
    assert_eq!(exit, 1, "prerequisites are unmet: {text}");
    let input = &report(&answer)["setup"]["input"];
    assert_eq!(input["supplied"], false, "{input}");
    assert!(
        input["detail"]
            .as_str()
            .unwrap()
            .contains("not an operator's choice"),
        "{input}"
    );
    assert_eq!(
        effective(report(&answer), "review.code_owners")["provenance"],
        "default"
    );
    let human = f.cli(&[
        "init",
        "plan",
        &f.project().display().to_string(),
        "--profile",
        PROFILE,
    ]);
    let text = String::from_utf8_lossy(&human.stdout);
    assert!(text.contains("setup      input none:"), "{text}");
    assert!(
        text.contains("setup      parameter review.code_owners = [] (default)"),
        "{text}"
    );
}
