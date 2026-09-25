//! The setup profile's rendered workflows, executed: spec 002 section 5,
//! 2026-09-24, the setup-profile entry, acceptance obligations 7 and 8.
//!
//! spec-spine 091's method: the real `run:` scalars of the rendered
//! workflows, not a copy of their logic, run by `bash -e` as a runner runs
//! them, with stubbed `claude`, `gh` and `npm` on `PATH`. Every `${{ }}`
//! expression is resolved from a table this file states, and an expression
//! the table does not carry **panics**, so a workflow edit that introduces a
//! new expression cannot pass by being silently substituted with nothing.
//!
//! Revision 4's governance steps run the same way, with a stubbed
//! `spec-spine` (at the fixture's `.tooling/bin`) and a stubbed `cargo`, and
//! the real `scripts/check-authored-content.sh` as the declared script.
//!
//! The mutation tests edit one blocking branch at a time in the rendered
//! `ci-gate.sh` and `ai-review.sh` and require the case suite to notice: a
//! suite that still passes with a branch inverted does not test that branch.
//!
//! Nothing here reaches a provider, a host or the network.

#![cfg(unix)]

use statecraft_environment::manifest::{Manifest, Pins};
use statecraft_home::setup::{self, Inputs, Profile};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const TOML: &str = "[meta]\nrequired_version = \"=0.25.0\"\n";

fn git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Render the registered profile into `root` through the library's own plan
/// and apply, so every test below runs the bytes a project receives.
fn render(root: &Path) {
    render_with(root, &[]);
}

/// As [`render`], with parameters added to the declared block.
fn render_with(root: &Path, extra: &[(&str, serde_json::Value)]) {
    // The local prerequisites, so the profile is not withheld.
    for (rel, text) in [
        ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.96.0\"\n"),
        ("Cargo.lock", "version = 4\n"),
    ] {
        if !root.join(rel).exists() {
            write(root, rel, text);
        }
    }
    std::fs::create_dir_all(root.join(".git")).unwrap();
    let profile = Profile::registered();
    let manifest = Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "unpinned".into(),
        adapters: Default::default(),
        producer: None,
    });
    let mut block = BTreeMap::new();
    block.insert("default_branch".to_string(), serde_json::json!("main"));
    for (k, v) in extra {
        block.insert(k.to_string(), v.clone());
    }
    let plan = setup::plan(&Inputs {
        root,
        profile: &profile,
        block: &block,
        manifest: &manifest,
        spec_spine_toml: Some(TOML),
        derived_dir: ".statecraft/derived",
    })
    .unwrap();
    assert!(plan.withheld.is_none(), "{:?}", plan.withheld);
    let mut manifest = manifest;
    setup::apply(
        root,
        &plan,
        &mut manifest,
        "2026-09-24T00:00:00Z",
        &mut |p, b| {
            std::fs::create_dir_all(p.parent().unwrap())?;
            std::fs::write(p, b)
        },
    )
    .unwrap();
    setup::finish(root).unwrap();
}

/// A reusable workflow a declared extra required job calls (revision 7).
const REUSABLE: &str = "on:\n  workflow_call:\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - run: true\n";

/// A repository with a base commit and a head commit. `at_base` names the
/// profile paths committed at the base (the rest of the rendered profile is
/// in the work tree only, which is the adoption case the workflows fall back
/// to the candidate for).
struct Repo {
    dir: tempfile::TempDir,
    base: String,
    head: String,
}

impl Repo {
    fn new(at_base: &[&str], head_edits: &[(&str, &str)]) -> Repo {
        Repo::new_with(&[], at_base, head_edits)
    }

    /// As [`Repo::new`], rendered with `params`; a declared authored-content
    /// script is written (executable) before the render.
    fn new_with(
        params: &[(&str, serde_json::Value)],
        at_base: &[&str],
        head_edits: &[(&str, &str)],
    ) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            git(root, &args);
        }
        write(root, "src/lib.rs", "pub fn one() -> u32 {\n    1\n}\n");
        write(root, "README.md", "# fixture\n");
        write(root, "spec-spine.toml", TOML);
        for (k, v) in params {
            if *k == "governance.authored_content" {
                let rel = v.as_str().unwrap();
                std::fs::create_dir_all(root.join(rel).parent().unwrap()).unwrap();
                statecraft_adapter::fixture::install_script(&root.join(rel), CHECK_AUTHORED, 0o755)
                    .unwrap();
            }
            if *k == "ci.extra_required_jobs" {
                for e in v.as_array().unwrap() {
                    write(root, e["workflow"].as_str().unwrap(), REUSABLE);
                }
            }
        }
        render_with(root, params);
        git(root, &["add", "src", "README.md", "spec-spine.toml"]);
        for rel in at_base {
            git(root, &["add", rel]);
        }
        git(root, &["commit", "--quiet", "-m", "base"]);
        let base = git(root, &["rev-parse", "HEAD"]);
        git(root, &["checkout", "--quiet", "-b", "topic"]);
        write(
            root,
            "src/lib.rs",
            "pub fn one() -> u32 {\n    1\n}\n\npub fn two() -> u32 {\n    2\n}\n",
        );
        git(root, &["add", "src/lib.rs"]);
        for (rel, text) in head_edits {
            write(root, rel, text);
            git(root, &["add", rel]);
        }
        git(root, &["commit", "--quiet", "-m", "head"]);
        let head = git(root, &["rev-parse", "HEAD"]);
        Repo { dir, base, head }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Commit `text` at `rel` on the topic branch; it becomes the head.
    fn commit(&mut self, rel: &str, text: &str) {
        write(self.root(), rel, text);
        git(self.root(), &["add", rel]);
        git(self.root(), &["commit", "--quiet", "-m", "edit"]);
        self.head = git(self.root(), &["rev-parse", "HEAD"]);
    }
}

fn workflow(root: &Path, name: &str) -> serde_yaml::Value {
    let text = std::fs::read_to_string(root.join(".github/workflows").join(name)).unwrap();
    serde_yaml::from_str(&text).unwrap()
}

/// A named step of a job: its `run:` scalar and its `env:` map, raw.
fn step(wf: &serde_yaml::Value, job: &str, name: &str) -> (String, Vec<(String, String)>) {
    let steps = wf["jobs"][job]["steps"].as_sequence().unwrap();
    let s = steps
        .iter()
        .find(|s| s["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("no step {name} in {job}"));
    let run = s["run"].as_str().unwrap().to_string();
    let env = s["env"]
        .as_mapping()
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    let v = match v {
                        serde_yaml::Value::String(s) => s.clone(),
                        serde_yaml::Value::Number(n) => n.to_string(),
                        serde_yaml::Value::Bool(b) => b.to_string(),
                        other => panic!("unsupported env value {other:?}"),
                    };
                    (k.as_str().unwrap().to_string(), v)
                })
                .collect()
        })
        .unwrap_or_default();
    (run, env)
}

/// Resolve every `${{ expr }}`. An expression the table does not carry is a
/// panic: the harness supports what it states and nothing else.
fn resolve(text: &str, ctx: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 3..];
        let end = after.find("}}").expect("an unterminated expression");
        let expr = after[..end].trim();
        let value = ctx
            .get(expr)
            .unwrap_or_else(|| panic!("the harness does not support the expression `{expr}`"));
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

struct Ran {
    exit: i32,
    outputs: BTreeMap<String, String>,
    text: String,
    stubs: PathBuf,
    runner: tempfile::TempDir,
}

impl Ran {
    fn output(&self, key: &str) -> &str {
        self.outputs.get(key).map_or("", String::as_str)
    }
    fn stub_file(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.stubs.join(name)).ok()
    }
}

const CLAUDE: &str = r#"#!/usr/bin/env bash
here="$STUB_STATE"
mode="$(cat "$here/claude-mode")"
input="$(cat)"
head="$(printf '%s\n' "$input" | sed -n 's/^head: //p' | head -n 1)"
printf '%s\n' "$PWD" > "$here/claude-cwd"
printf '%s\n' "$HOME" > "$here/claude-home"
ls -A "$PWD" > "$here/claude-cwd-listing"
touch "$here/claude-called"
case "$mode" in
  findings)
    printf 'One finding.\n\n```json\n{"head": "%s", "verdict": "findings", "findings": [{"path": "src/lib.rs", "line": 5, "summary": "two is untested"}]}\n```\n' "$head" ;;
  no-findings)
    printf 'Nothing to report.\n\n```json\n{"head": "%s", "verdict": "no-findings", "findings": []}\n```\n' "$head" ;;
  empty) : ;;
  unrelated) echo "I have finished the refactor you asked for earlier." ;;
  wronghead)
    printf '```json\n{"head": "%s", "verdict": "no-findings", "findings": []}\n```\n' "1111111111111111111111111111111111111111" ;;
  findings-empty)
    printf '```json\n{"head": "%s", "verdict": "findings", "findings": []}\n```\n' "$head" ;;
  no-findings-listed)
    printf '```json\n{"head": "%s", "verdict": "no-findings", "findings": [{"path": "src/lib.rs", "line": 1, "summary": "x"}]}\n```\n' "$head" ;;
  badverdict)
    printf '```json\n{"head": "%s", "verdict": "approve", "findings": []}\n```\n' "$head" ;;
  outside)
    printf '```json\n{"head": "%s", "verdict": "findings", "findings": [{"path": "README.md", "line": 1, "summary": "x"}]}\n```\n' "$head" ;;
  transient)
    echo 'API Error: 529 {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}' >&2
    exit 1 ;;
  refusal)
    echo 'API Error: 403 {"type":"error","error":{"type":"permission_error","message":"Your organization does not have access to Claude."}}' >&2
    exit 1 ;;
  refusal-and-transient)
    echo 'API Error: 529 overloaded_error; also: your organization does not have access to Claude' >&2
    exit 1 ;;
  unclassified)
    echo 'something went wrong' >&2
    exit 1 ;;
  *) echo "unknown stub mode $mode" >&2; exit 99 ;;
esac
"#;

const GH: &str = r#"#!/usr/bin/env bash
here="$STUB_STATE"
printf '%s\n' "$*" >> "$here/gh-calls"
case "$1 $2" in
  "pr comment")
    cat > /dev/null
    code="$(cat "$here/gh-comment-exit")"
    if [ "$code" = 0 ]; then
      # The thread the next read sees: each posted body, in order.
      prev=""
      for a in "$@"; do
        if [ "$prev" = "--body-file" ]; then cat "$a" >> "$here/gh-comments"; fi
        prev="$a"
      done
    fi
    exit "$code" ;;
  "api --paginate")
    case "$3" in
      repos/*/issues/*/comments) cat "$here/gh-comments" 2> /dev/null || true ;;
      *) echo "gh stub: unsupported $*" >&2; exit 98 ;;
    esac ;;
  api\ repos/*/actions/runs/*/jobs*)
    cat "$here/gh-jobs" 2> /dev/null || echo '{"jobs": []}' ;;
  api\ repos/*/actions/runs*)
    cat "$here/gh-runs" 2> /dev/null || echo '{"workflow_runs": []}' ;;
  api\ repos/*/commits/*)
    # Revision 4: GitHub's verification of one commit, as `--jq` reads it.
    sha="${2##*/}"
    if grep -qx "$sha" "$here/gh-unsigned" 2> /dev/null; then echo false; else echo true; fi ;;
  api\ repos/*/pulls/*)
    if [ -f "$here/gh-pull-fails" ]; then echo "HTTP 404: Not Found" >&2; exit 1; fi
    cat "$here/gh-head" ;;
  "run download")
    dir=""; prev=""
    for a in "$@"; do
      if [ "$prev" = "--dir" ]; then dir="$a"; fi
      prev="$a"
    done
    [ -f "$here/gh-record" ] || { echo "no artifact" >&2; exit 1; }
    mkdir -p "$dir"
    cp "$here/gh-record" "$dir/ai-review-evidence.json" ;;
  *) echo "gh stub: unsupported $*" >&2; exit 98 ;;
esac
"#;

const NPM: &str = "#!/bin/sh\nhere=\"$STUB_STATE\"\necho \"$*\" >> \"$here/npm-calls\"\nexit \"$(cat \"$here/npm-exit\" 2>/dev/null || echo 0)\"\n";

/// The stub programs, written once per test process: a freshly written
/// executable can stall on its first exec on some hosts, so each case
/// reuses these and keeps its own state in a directory named by
/// `STUB_STATE`.
fn stub_bin() -> &'static Path {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap().keep();
        for (name, body) in [("claude", CLAUDE), ("gh", GH), ("npm", NPM)] {
            statecraft_adapter::fixture::install_script(&dir.join(name), body, 0o755).unwrap();
        }
        dir
    })
}

/// Run one step's `run:` scalar as a runner would: `bash -e`, in the
/// checkout, with the step's `env:` resolved from `ctx` plus `extra`.
fn run_step(
    root: &Path,
    run: &str,
    env: &[(String, String)],
    ctx: &BTreeMap<String, String>,
    extra: &[(&str, &str)],
    setup_stubs: impl Fn(&Path),
) -> Ran {
    run_steps(
        root,
        &[(run.to_string(), env.to_vec())],
        ctx,
        extra,
        setup_stubs,
    )
}

/// Run several steps of one job in order, as a runner would: one
/// `RUNNER_TEMP`, and what a step appends to `GITHUB_ENV` is in the
/// environment of every later step. The first step that fails ends the job;
/// the text is every step's, in order, and the outputs are the last step's.
fn run_steps(
    root: &Path,
    steps: &[(String, Vec<(String, String)>)],
    ctx: &BTreeMap<String, String>,
    extra: &[(&str, &str)],
    setup_stubs: impl Fn(&Path),
) -> Ran {
    let runner = tempfile::tempdir().unwrap();
    let stubs_dir = runner.path().join("stub-state");
    std::fs::create_dir_all(&stubs_dir).unwrap();
    setup_stubs(&stubs_dir);
    let temp = runner.path().join("temp");
    std::fs::create_dir_all(&temp).unwrap();
    let github_env = runner.path().join("env");
    std::fs::write(&github_env, "").unwrap();
    let summary = runner.path().join("summary");
    let path = format!(
        "{}:{}",
        stub_bin().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut text = String::new();
    let mut exit = 0;
    let mut outputs = BTreeMap::new();
    for (run, env) in steps {
        let output = runner.path().join("output");
        std::fs::write(&output, "").unwrap();
        let mut cmd = Command::new("bash");
        cmd.args(["-e", "-c", &resolve(run, ctx)])
            .current_dir(root)
            .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
            .env_remove("GH_TOKEN")
            .env_remove("AI_REVIEW_TMP")
            .env_remove("STATECRAFT_GATE")
            .env_remove("STATECRAFT_INSTALL")
            .env_remove("BASE_SHA")
            .env("PATH", &path)
            .env("STUB_STATE", &stubs_dir)
            .env("RUNNER_TEMP", &temp)
            .env("GITHUB_ENV", &github_env)
            .env("GITHUB_OUTPUT", &output)
            .env("GITHUB_STEP_SUMMARY", &summary);
        for (k, v) in std::fs::read_to_string(&github_env)
            .unwrap()
            .lines()
            .filter_map(|l| l.split_once('='))
        {
            cmd.env(k, v);
        }
        for (k, v) in env {
            cmd.env(k, resolve(v, ctx));
        }
        for (k, v) in extra {
            cmd.env(k, v);
        }
        let out = cmd.output().unwrap();
        outputs = std::fs::read_to_string(&output)
            .unwrap()
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        text.push_str(&String::from_utf8_lossy(&out.stdout));
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        exit = out.status.code().unwrap_or(-1);
        if exit != 0 {
            break;
        }
    }
    Ran {
        exit,
        outputs,
        text,
        stubs: stubs_dir,
        runner,
    }
}

/// The step that reads the gate at the base (revision 5, rule 1).
const READ_GATE: &str = "Read the gate at the base";

/// Run a named step of a job as the job runs it: after the job's step that
/// reads the gate at the base, when the job has one, so the named step runs
/// the copy that step chose.
fn run_job_step(
    root: &Path,
    job: &str,
    name: &str,
    ctx: &BTreeMap<String, String>,
    extra: &[(&str, &str)],
    setup_stubs: impl Fn(&Path),
) -> Ran {
    let wf = workflow(root, "statecraft-ci.yml");
    let has_read = wf["jobs"][job]["steps"]
        .as_sequence()
        .unwrap()
        .iter()
        .any(|s| s["name"].as_str() == Some(READ_GATE));
    let mut steps = Vec::new();
    if has_read && name != READ_GATE {
        steps.push(step(&wf, job, READ_GATE));
    }
    steps.push(step(&wf, job, name));
    run_steps(root, &steps, ctx, extra, setup_stubs)
}

// ---------------------------------------------------------------- ci-gate

fn needs(entries: &[(&str, &str)], review: Option<&str>, rc: bool) -> String {
    let mut map = serde_json::Map::new();
    for (job, result) in entries {
        let mut v = serde_json::json!({"result": result, "outputs": {}});
        if *job == "ai-review" {
            if let Some(r) = review {
                v["outputs"]["result"] = serde_json::json!(r);
            }
            v["outputs"]["release_candidate"] = serde_json::json!(rc.to_string());
        }
        map.insert(job.to_string(), v);
    }
    serde_json::Value::Object(map).to_string()
}

fn gate_ctx(
    repo: &Repo,
    event: &str,
    needs_json: &str,
    head_ref: &str,
) -> BTreeMap<String, String> {
    let mut ctx = BTreeMap::new();
    ctx.insert("toJSON(needs)".into(), needs_json.to_string());
    ctx.insert("github.event_name".into(), event.to_string());
    ctx.insert(
        "github.event.pull_request.base.sha || github.event.merge_group.base_sha || github.event.before".into(),
        repo.base.clone(),
    );
    ctx.insert(
        "github.event.pull_request.head.sha || github.event.merge_group.head_sha || github.sha"
            .into(),
        repo.head.clone(),
    );
    // On pull_request and push the queue ref is empty; a merge-queue case
    // replaces it (`run_queue`).
    ctx.insert("github.head_ref".into(), head_ref.to_string());
    ctx.insert("github.event.merge_group.head_ref".into(), String::new());
    ctx.insert("github.repository".into(), "owner/fixture".into());
    ctx.insert("github.token".into(), "fixture-token".into());
    ctx
}

fn run_gate(repo: &Repo, event: &str, needs_json: &str, head_ref: &str) -> Ran {
    let wf = workflow(repo.root(), "statecraft-ci.yml");
    let (run, env) = step(&wf, "ci-gate", "Aggregate");
    run_step(
        repo.root(),
        &run,
        &env,
        &gate_ctx(repo, event, needs_json, head_ref),
        &[],
        |_| {},
    )
}

const ALL_OK: [(&str, &str); 4] = [
    ("governance", "success"),
    ("code", "success"),
    ("ai-review", "success"),
    ("review-exception", "skipped"),
];

fn with(result: (&str, &str)) -> Vec<(&'static str, String)> {
    ALL_OK
        .iter()
        .map(|(j, r)| {
            let r = if *j == result.0 { result.1 } else { r };
            (*j, r.to_string())
        })
        .collect()
}

fn as_refs<'a>(v: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    v.iter().map(|(j, r)| (*j, r.as_str())).collect()
}

/// Every ci-gate case, with the exit it must have. Shared by the plain test
/// and the mutation test.
fn gate_cases() -> Vec<(&'static str, &'static str, String, &'static str, i32)> {
    let mut cases = vec![
        (
            "pr, all required succeed, reviewed",
            "pull_request",
            needs(&ALL_OK, Some("no-findings"), false),
            "topic",
            0,
        ),
        // Revision 2 (R2-1): the AI review is the final approver, so a
        // findings verdict blocks unless the owner's exception was approved.
        // Revision 1 asserted "pr, findings do not block" (exit 0) here.
        (
            "pr, findings, no owner exception (R2-1)",
            "pull_request",
            needs(&ALL_OK, Some("findings"), false),
            "topic",
            1,
        ),
        (
            "pr, findings, owner exception rejected (R2-1)",
            "pull_request",
            needs(
                &as_refs(&with(("review-exception", "failure"))),
                Some("findings"),
                false,
            ),
            "topic",
            1,
        ),
        (
            "pr, findings, owner exception approved (R2-1)",
            "pull_request",
            needs(
                &as_refs(&with(("review-exception", "success"))),
                Some("findings"),
                false,
            ),
            "topic",
            0,
        ),
        (
            "pr, a visible skip does not block (S-2)",
            "pull_request",
            needs(&ALL_OK, Some("skipped:oversized"), false),
            "topic",
            0,
        ),
    ];
    for (job, result) in [
        ("governance", "failure"),
        ("code", "cancelled"),
        ("ai-review", "failure"),
        ("governance", "skipped"),
        ("code", "skipped"),
        ("ai-review", "skipped"),
    ] {
        let label: &'static str = Box::leak(format!("pr, {job} {result}").into_boxed_str());
        cases.push((
            label,
            "pull_request",
            needs(&as_refs(&with((job, result))), Some("no-findings"), false),
            "topic",
            1,
        ));
    }
    for missing in ["governance", "code", "ai-review", "review-exception"] {
        let label: &'static str = Box::leak(format!("pr, {missing} vanished").into_boxed_str());
        let entries: Vec<(&str, &str)> = ALL_OK
            .iter()
            .copied()
            .filter(|(j, _)| *j != missing)
            .collect();
        cases.push((
            label,
            "pull_request",
            needs(&entries, Some("no-findings"), false),
            "topic",
            1,
        ));
    }
    cases.extend([
        (
            "pr, success without a review result",
            "pull_request",
            needs(&ALL_OK, None, false),
            "topic",
            1,
        ),
        (
            "pr, an unknown review result",
            "pull_request",
            needs(&ALL_OK, Some("approved"), false),
            "topic",
            1,
        ),
        (
            "push, review and exception inapplicable",
            "push",
            needs(
                &[
                    ("governance", "success"),
                    ("code", "success"),
                    ("ai-review", "skipped"),
                    ("review-exception", "skipped"),
                ],
                None,
                false,
            ),
            "",
            0,
        ),
        (
            "push, an inapplicable review that ran",
            "push",
            needs(
                &[
                    ("governance", "success"),
                    ("code", "success"),
                    ("ai-review", "success"),
                    ("review-exception", "skipped"),
                ],
                Some("no-findings"),
                false,
            ),
            "",
            1,
        ),
        (
            "push, code failed",
            "push",
            needs(
                &[
                    ("governance", "success"),
                    ("code", "failure"),
                    ("ai-review", "skipped"),
                    ("review-exception", "skipped"),
                ],
                None,
                false,
            ),
            "",
            1,
        ),
        (
            "release candidate, review skipped, no exception (S-1)",
            "pull_request",
            needs(&ALL_OK, Some("skipped:oversized"), true),
            "release/1.0",
            1,
        ),
        (
            "release candidate, review skipped, owner exception approved (S-1)",
            "pull_request",
            needs(
                &as_refs(&with(("review-exception", "success"))),
                Some("skipped:oversized"),
                true,
            ),
            "release/1.0",
            0,
        ),
        (
            "release candidate, reviewed",
            "pull_request",
            needs(&ALL_OK, Some("no-findings"), true),
            "release/1.0",
            0,
        ),
        // Revision 7: a precondition the gate cannot judge past refuses (2).
        (
            "an event the policy states no rule for",
            "workflow_dispatch",
            needs(&ALL_OK, Some("no-findings"), false),
            "",
            2,
        ),
    ]);
    cases
}

const POLICY: &str = ".statecraft/setup/github-actions-rust.json";

/// What a blocking case's report must say: a gate that blocks without
/// saying why has lost the branch that was meant to block it.
fn reason(label: &str) -> &'static str {
    if let Some(why) = queue_reason(label) {
        return why;
    }
    if label.contains("findings, no owner exception") || label.contains("exception rejected") {
        "returned findings"
    } else if label.ends_with("vanished") {
        "vanished"
    } else if label.ends_with("skipped") || label.contains("no exception") {
        "an unexpected skip"
    } else if label.contains("review result") {
        "without a review result"
    } else if label.contains("inapplicable review that ran") {
        "must be skipped"
    } else if label.contains("no rule") {
        "states no rule"
    } else {
        "ended"
    }
}

#[test]
fn ci_gate_blocks_exactly_what_the_policy_says() {
    let repo = Repo::new(&[POLICY, "scripts/statecraft/ci-gate.sh"], &[]);
    for (label, event, needs_json, head_ref, want) in gate_cases() {
        let ran = run_gate(&repo, event, &needs_json, head_ref);
        assert_eq!(ran.exit, want, "{label}:\n{}", ran.text);
        if want != 0 {
            assert!(ran.text.contains(reason(label)), "{label}: {}", ran.text);
        }
        if want == 0 {
            assert!(
                ran.text.contains("policy: read at the base"),
                "{label}: {}",
                ran.text
            );
        }
    }
    // The rule that admitted an inapplicable skip is printed.
    let ran = run_gate(
        &repo,
        "push",
        &needs(
            &[
                ("governance", "success"),
                ("code", "success"),
                ("ai-review", "skipped"),
                ("review-exception", "skipped"),
            ],
            None,
            false,
        ),
        "",
    );
    assert!(
        ran.text
            .contains("ai-review skipped, admitted because it is inapplicable on push"),
        "{}",
        ran.text
    );
}

#[test]
fn a_candidate_cannot_drop_a_job_from_the_set_that_judges_it() {
    // The candidate's policy no longer requires `code`, and its gate script
    // no longer blocks anything. The base's copies judge it.
    let base_policy = {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "spec-spine.toml", TOML);
        render(tmp.path());
        std::fs::read_to_string(tmp.path().join(POLICY)).unwrap()
    };
    let mut weakened: serde_json::Value = serde_json::from_str(&base_policy).unwrap();
    weakened["jobs"].as_object_mut().unwrap().remove("code");
    let repo = Repo::new(
        &[POLICY, "scripts/statecraft/ci-gate.sh"],
        &[
            (POLICY, &serde_json::to_string_pretty(&weakened).unwrap()),
            (
                "scripts/statecraft/ci-gate.sh",
                "#!/usr/bin/env bash\necho weakened\nexit 0\n",
            ),
        ],
    );
    let entries: Vec<(&str, &str)> = ALL_OK
        .iter()
        .copied()
        .filter(|(j, _)| *j != "code")
        .collect();
    let ran = run_gate(
        &repo,
        "pull_request",
        &needs(&entries, Some("no-findings"), false),
        "topic",
    );
    assert_eq!(ran.exit, 1, "{}", ran.text);
    assert!(
        ran.text.contains("ci-gate.sh: read at the base"),
        "{}",
        ran.text
    );
    assert!(
        ran.text.contains("'code' is not in the needs record"),
        "{}",
        ran.text
    );
    // And the change to the gate is reported as an authority change.
    assert!(ran.text.contains("authority change"), "{}", ran.text);
    assert!(ran.text.contains(POLICY), "{}", ran.text);
}

/// The authority-change report compares the candidate with its fork point,
/// not with the base's tip: a base that moved on after the branch was cut,
/// changing a file of the profile, is not reported as this candidate's change
/// (the third live AI review of #138, finding 1).
#[test]
fn an_advanced_base_is_not_reported_as_the_candidates_authority_change() {
    let mut repo = Repo::new(&[POLICY, "scripts/statecraft/ci-gate.sh"], &[]);
    let root = repo.root().to_path_buf();
    git(&root, &["checkout", "--quiet", "main"]);
    let gate = std::fs::read_to_string(root.join("scripts/statecraft/ci-gate.sh")).unwrap();
    write(
        &root,
        "scripts/statecraft/ci-gate.sh",
        &format!("{gate}# a later change on the base\n"),
    );
    git(&root, &["add", "scripts/statecraft/ci-gate.sh"]);
    git(&root, &["commit", "--quiet", "-m", "the base moves on"]);
    repo.base = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "topic"]);
    let ran = run_gate(
        &repo,
        "pull_request",
        &needs(&ALL_OK, Some("no-findings"), false),
        "topic",
    );
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(
        ran.text.contains("ci-gate.sh: read at the base"),
        "{}",
        ran.text
    );
    assert!(!ran.text.contains("authority change"), "{}", ran.text);
}

#[test]
fn the_adoption_reads_the_candidate_and_says_so() {
    let repo = Repo::new(&[], &[]);
    let ran = run_gate(
        &repo,
        "pull_request",
        &needs(&ALL_OK, Some("no-findings"), false),
        "topic",
    );
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(ran.text.contains("the base carries none"), "{}", ran.text);
    assert!(ran.text.contains("authority change"), "{}", ran.text);
}

#[test]
fn ci_gate_needs_every_job_the_policy_requires() {
    // Mutation obligation 8, first half: removing a required job from
    // ci-gate's `needs` makes this fail.
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "spec-spine.toml", TOML);
    render(tmp.path());
    let wf = workflow(tmp.path(), "statecraft-ci.yml");
    let mut needs: Vec<String> = wf["jobs"]["ci-gate"]["needs"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    needs.sort();
    let policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.path().join(POLICY)).unwrap()).unwrap();
    let mut required: Vec<String> = policy["jobs"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, v)| v["required"] == true)
        .map(|(k, _)| k.clone())
        .collect();
    required.sort();
    assert_eq!(needs, required);
    // Every other job is a job of the workflow, and ci-gate always runs.
    for job in &required {
        assert!(
            !wf["jobs"][job.as_str()].is_null(),
            "{job} is required and not defined"
        );
    }
    assert_eq!(wf["jobs"]["ci-gate"]["if"].as_str(), Some("always()"));
    // Revision 2 (R2-1): the owner exception also runs for a findings verdict,
    // and keeps revision 1's release-candidate case.
    let exception_if = wf["jobs"]["review-exception"]["if"].as_str().unwrap();
    assert!(
        exception_if.contains("needs.ai-review.outputs.result == 'findings'"),
        "{exception_if}"
    );
    assert!(
        exception_if.contains("needs.ai-review.outputs.release_candidate == 'true'"),
        "{exception_if}"
    );
    assert_eq!(
        wf["jobs"]["review-exception"]["environment"].as_str(),
        Some("statecraft-review-exception")
    );
    // Revision 3: the queue is judged, and the gate job only reads.
    // `merge_group:` has no value, so the key's presence is the trigger.
    assert!(
        wf["on"]
            .as_mapping()
            .unwrap()
            .contains_key(serde_yaml::Value::from("merge_group")),
        "a merge_group trigger"
    );
    let perms = &wf["jobs"]["ci-gate"]["permissions"];
    for (k, v) in perms.as_mapping().unwrap() {
        assert_eq!(v.as_str(), Some("read"), "ci-gate may only read: {k:?}");
    }
    assert!(
        wf["jobs"]["ai-review"]["if"]
            .as_str()
            .unwrap()
            .contains("github.event_name == 'pull_request'"),
        "the review is never re-run in the queue"
    );
    assert!(
        wf["on"]["pull_request_target"].is_null(),
        "never pull_request_target"
    );
}

#[test]
fn inverting_a_blocking_branch_of_ci_gate_is_noticed() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "spec-spine.toml", TOML);
    render(tmp.path());
    let script = std::fs::read_to_string(tmp.path().join("scripts/statecraft/ci-gate.sh")).unwrap();
    let lines: Vec<&str> = script.lines().collect();
    let sites: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            t.starts_with("block \"") || t.starts_with("stop \"")
        })
        .map(|(i, _)| i)
        .collect();
    assert!(
        sites.len() >= 8,
        "found only {} blocking branches",
        sites.len()
    );
    let cases = gate_cases();
    for site in sites {
        let mutated: String = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i == site {
                    // The branch reports and does not block.
                    format!(
                        "{}\n",
                        l.replacen("block \"", ": \"", 1)
                            .replacen("stop \"", ": \"", 1)
                    )
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        // The base carries the policy but not the script, so the mutated
        // candidate copy is the one that runs.
        let repo = Repo::new(&[POLICY], &[]);
        std::fs::write(repo.root().join("scripts/statecraft/ci-gate.sh"), &mutated).unwrap();
        // Noticed: a case ends differently, or a block no longer says why.
        let differs = |ran: &Ran, want: i32, why: &str| {
            ran.exit != want || (want != 0 && !ran.text.contains(why))
        };
        let noticed = cases
            .iter()
            .any(|(label, event, needs_json, head_ref, want)| {
                differs(
                    &run_gate(&repo, event, needs_json, head_ref),
                    *want,
                    reason(label),
                )
            })
            || queue_cases()
                .iter()
                .any(|q| differs(&run_queue(&repo, q), q.want, reason(q.label)));
        // Two branches need a policy of their own: none at all, and one that
        // names a rule the gate does not know.
        let noticed = noticed || {
            let bare = Repo::new(&[], &[]);
            std::fs::remove_file(bare.root().join(POLICY)).unwrap();
            std::fs::write(bare.root().join("scripts/statecraft/ci-gate.sh"), &mutated).unwrap();
            differs(
                &run_gate(
                    &bare,
                    "pull_request",
                    &needs(&ALL_OK, Some("no-findings"), false),
                    "topic",
                ),
                2,
                "no policy",
            )
        };
        // Revision 5, rule 2: two branches need an authority change, on a
        // pull request and in the queue. The base carries the policy but not
        // the script, so the mutated copy runs.
        let noticed = noticed || {
            let mut auth = Repo::new(&[POLICY], &[]);
            auth.commit("scripts/statecraft/helper.sh", "#!/bin/sh\n");
            std::fs::write(auth.root().join("scripts/statecraft/ci-gate.sh"), &mutated).unwrap();
            differs(
                &run_gate(
                    &auth,
                    "pull_request",
                    &needs(&ALL_OK, Some("no-findings"), false),
                    "topic",
                ),
                1,
                "changes the authority set and the owner exception",
            ) || differs(
                &run_queue(&auth, &queue("queue, authority change", 1)),
                1,
                "the queued group changes the authority set",
            )
        };
        let noticed = noticed || {
            let odd = Repo::new(&[], &[]);
            let mut policy: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(odd.root().join(POLICY)).unwrap())
                    .unwrap();
            policy["jobs"]["code"]["pull_request"] = serde_json::json!("sometimes");
            std::fs::write(odd.root().join(POLICY), policy.to_string()).unwrap();
            std::fs::write(odd.root().join("scripts/statecraft/ci-gate.sh"), &mutated).unwrap();
            differs(
                &run_gate(
                    &odd,
                    "pull_request",
                    &needs(&ALL_OK, Some("no-findings"), false),
                    "topic",
                ),
                1,
                "unknown rule",
            )
        };
        assert!(
            noticed,
            "inverting line {} went unnoticed: {}",
            site + 1,
            lines[site]
        );
    }
}

// ------------------------------------------------------------ merge queue

/// The pull request a queue entry was built from, in the fixtures.
const PR_HEAD: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";
const QUEUE_REF: &str = "gh-readonly-queue/main/pr-7-0123456789abcdef0123456789abcdef01234567";

/// One merge-queue case: what the API answers about the entry's pull
/// request, its runs, their jobs and the evidence record.
struct Queue {
    label: &'static str,
    needs_json: String,
    group_ref: &'static str,
    pull_fails: bool,
    pr_ref: &'static str,
    run: bool,
    record: Option<serde_json::Value>,
    exception: &'static str,
    want: i32,
}

fn record(result: &str, pr: u64, head: &str) -> serde_json::Value {
    serde_json::json!({
        "subject": {"repository": "owner/fixture", "pullRequest": pr, "head": head},
        "result": result,
        "findings": [],
    })
}

const QUEUE_OK: [(&str, &str); 4] = [
    ("governance", "success"),
    ("code", "success"),
    ("ai-review", "skipped"),
    ("review-exception", "skipped"),
];

fn queue_needs(over: Option<(&str, &str)>) -> String {
    let entries: Vec<(&str, &str)> = QUEUE_OK
        .iter()
        .map(|(j, r)| match over {
            Some((oj, or)) if oj == *j => (*j, or),
            _ => (*j, *r),
        })
        .collect();
    needs(&entries, None, false)
}

fn queue(label: &'static str, want: i32) -> Queue {
    Queue {
        label,
        needs_json: queue_needs(None),
        group_ref: QUEUE_REF,
        pull_fails: false,
        pr_ref: "topic",
        run: true,
        record: Some(record("no-findings", 7, PR_HEAD)),
        exception: "skipped",
        want,
    }
}

/// Every merge-queue case (revision 3), shared by the plain test and the
/// mutation test.
fn queue_cases() -> Vec<Queue> {
    vec![
        queue("queue, recorded no-findings", 0),
        Queue {
            record: Some(record("findings", 7, PR_HEAD)),
            exception: "success",
            ..queue("queue, recorded findings, owner exception approved", 0)
        },
        Queue {
            record: Some(record("skipped:oversized", 7, PR_HEAD)),
            ..queue("queue, recorded skip, not a release candidate", 0)
        },
        Queue {
            group_ref: "gh-readonly-queue/main/not-a-queue-entry",
            ..queue("queue, unreadable queue ref", 1)
        },
        Queue {
            pull_fails: true,
            ..queue("queue, pull request unreadable", 1)
        },
        Queue {
            run: false,
            ..queue("queue, no recorded run", 1)
        },
        Queue {
            record: None,
            ..queue("queue, no evidence record", 1)
        },
        Queue {
            record: Some(record(
                "no-findings",
                7,
                "1111111111111111111111111111111111111111",
            )),
            ..queue("queue, record for another head", 1)
        },
        Queue {
            record: Some(record("no-findings", 8, PR_HEAD)),
            ..queue("queue, record for another pull request", 1)
        },
        Queue {
            needs_json: queue_needs(Some(("ai-review", "success"))),
            ..queue("queue, the review ran in the queue", 1)
        },
        Queue {
            record: Some(record("findings", 7, PR_HEAD)),
            ..queue("queue, recorded findings, no owner exception", 1)
        },
        Queue {
            record: Some(record("findings", 7, PR_HEAD)),
            exception: "failure",
            ..queue("queue, recorded findings, owner exception rejected", 1)
        },
        Queue {
            record: Some(record("skipped:oversized", 7, PR_HEAD)),
            pr_ref: "release/1.0",
            ..queue("queue, recorded skip, release candidate, no exception", 1)
        },
        Queue {
            record: Some(record("", 7, PR_HEAD)),
            ..queue("queue, record without a result", 1)
        },
        Queue {
            needs_json: queue_needs(Some(("code", "failure"))),
            ..queue("queue, code failed", 1)
        },
        Queue {
            needs_json: queue_needs(Some(("governance", "skipped"))),
            ..queue("queue, governance skipped", 1)
        },
        Queue {
            needs_json: queue_needs(Some(("review-exception", "success"))),
            ..queue("queue, an exception job that ran", 1)
        },
    ]
}

/// What a blocking queue case's report must say.
fn queue_reason(label: &str) -> Option<&'static str> {
    if !label.starts_with("queue, ") {
        return None;
    }
    Some(if label.contains("unreadable queue ref") {
        "cannot read the queued pull request's number"
    } else if label.contains("pull request unreadable") {
        "cannot read pull request #7"
    } else if label.contains("no recorded run") {
        "no completed statecraft-ci run"
    } else if label.contains("no evidence record") {
        "carries no evidence record"
    } else if label.contains("another") {
        "names another pull request or head"
    } else if label.contains("ran in the queue") {
        "not re-run in the merge queue"
    } else if label.contains("release candidate") {
        "release candidate #7"
    } else if label.contains("recorded findings") {
        "returned findings and its owner exception was not approved"
    } else if label.contains("without a result") {
        "carries no review result"
    } else if label.contains("skipped") {
        "an unexpected skip"
    } else if label.contains("exception job that ran") {
        "must be skipped"
    } else {
        "ended"
    })
}

fn run_queue(repo: &Repo, q: &Queue) -> Ran {
    let wf = workflow(repo.root(), "statecraft-ci.yml");
    let (run, env) = step(&wf, "ci-gate", "Aggregate");
    let mut ctx = gate_ctx(repo, "merge_group", &q.needs_json, "");
    ctx.insert(
        "github.event.merge_group.head_ref".into(),
        q.group_ref.to_string(),
    );
    run_step(repo.root(), &run, &env, &ctx, &[], |state| {
        let pull = serde_json::json!({"number": 7, "head": {"sha": PR_HEAD, "ref": q.pr_ref}});
        std::fs::write(state.join("gh-head"), pull.to_string()).unwrap();
        if q.pull_fails {
            std::fs::write(state.join("gh-pull-fails"), "").unwrap();
        }
        let runs = if q.run {
            serde_json::json!({"workflow_runs": [
                {"id": 41, "name": "statecraft-ci", "event": "pull_request", "head_sha": PR_HEAD, "status": "completed"},
                {"id": 42, "name": "statecraft-ci", "event": "pull_request", "head_sha": PR_HEAD, "status": "completed"},
                {"id": 43, "name": "something-else", "event": "pull_request", "head_sha": PR_HEAD, "status": "completed"}
            ]})
        } else {
            serde_json::json!({"workflow_runs": []})
        };
        std::fs::write(state.join("gh-runs"), runs.to_string()).unwrap();
        let jobs = serde_json::json!({"jobs": [
            {"name": "ai-review / review", "conclusion": "success"},
            {"name": "review-exception", "conclusion": q.exception}
        ]});
        std::fs::write(state.join("gh-jobs"), jobs.to_string()).unwrap();
        if let Some(r) = &q.record {
            std::fs::write(state.join("gh-record"), r.to_string()).unwrap();
        }
    })
}

#[test]
fn a_queue_entry_is_judged_by_the_review_recorded_for_its_pull_request() {
    let repo = Repo::new(&[POLICY, "scripts/statecraft/ci-gate.sh"], &[]);
    for q in queue_cases() {
        let ran = run_queue(&repo, &q);
        assert_eq!(ran.exit, q.want, "{}:\n{}", q.label, ran.text);
        if q.want == 1 {
            assert!(
                ran.text.contains(reason(q.label)),
                "{}: {}",
                q.label,
                ran.text
            );
        } else {
            assert!(
                ran.text.contains("admitted by the review recorded for #7"),
                "{}: {}",
                q.label,
                ran.text
            );
            // The latest completed statecraft-ci run is the one read.
            assert!(ran.text.contains("run 42"), "{}: {}", q.label, ran.text);
        }
        // Never a second review: the gate only reads.
        assert!(ran.stub_file("claude-called").is_none(), "{}", q.label);
        let calls = ran.stub_file("gh-calls").unwrap_or_default();
        assert!(!calls.contains("pr comment"), "{}: {calls}", q.label);
    }
}

// -------------------------------------------------------------- ai-review

struct Case {
    label: &'static str,
    mode: &'static str,
    extra: Vec<(&'static str, String)>,
    comment_exit: &'static str,
    moved_head: bool,
    unreadable_head: bool,
    npm_exit: &'static str,
    want_exit: i32,
    want_result: &'static str,
}

fn review_cases() -> Vec<Case> {
    let c = |label, mode, want_exit, want_result| Case {
        label,
        mode,
        extra: vec![],
        comment_exit: "0",
        moved_head: false,
        unreadable_head: false,
        npm_exit: "0",
        want_exit,
        want_result,
    };
    let mut cases = vec![
        c("a review with findings", "findings", 0, "findings"),
        c("a review with none", "no-findings", 0, "no-findings"),
        c("empty output", "empty", 4, ""),
        c("unrelated output", "unrelated", 4, ""),
        c("a verdict for another head", "wronghead", 4, ""),
        c("a finding outside the diff", "outside", 4, ""),
        c("a findings verdict listing none", "findings-empty", 4, ""),
        c(
            "a no-findings verdict listing some",
            "no-findings-listed",
            4,
            "",
        ),
        c("a verdict that is neither", "badverdict", 4, ""),
        c(
            "a recognized transient failure",
            "transient",
            0,
            "skipped:transient",
        ),
        c("an explicit refusal", "refusal", 2, ""),
        c(
            "a refusal outranks a transient signal",
            "refusal-and-transient",
            2,
            "",
        ),
        c("an unclassified failure", "unclassified", 4, ""),
    ];
    let mut missing = c("a missing credential", "no-findings", 2, "");
    missing
        .extra
        .push(("CLAUDE_CODE_OAUTH_TOKEN", String::new()));
    cases.push(missing);
    let mut fork = c("a fork", "no-findings", 0, "skipped:fork");
    fork.extra.push(("HEAD_REPO", "someone/fork".into()));
    fork.extra.push(("CLAUDE_CODE_OAUTH_TOKEN", String::new()));
    cases.push(fork);
    let mut draft = c("a draft", "no-findings", 0, "skipped:draft");
    draft.extra.push(("IS_DRAFT", "true".into()));
    cases.push(draft);
    let mut bot = c("dependabot", "no-findings", 0, "skipped:dependabot");
    bot.extra.push(("ACTOR", "dependabot[bot]".into()));
    cases.push(bot);
    let mut big = c("an oversized diff", "no-findings", 0, "skipped:oversized");
    big.extra.push(("DIFF_CAP", "1".into()));
    cases.push(big);
    let mut stale = c("a stale subject", "no-findings", 2, "");
    stale.moved_head = true;
    cases.push(stale);
    let mut unreadable = c("the current head cannot be read", "no-findings", 4, "");
    unreadable.unreadable_head = true;
    cases.push(unreadable);
    let mut uninstalled = c("the reviewer cannot be installed", "no-findings", 4, "");
    uninstalled.npm_exit = "1";
    cases.push(uninstalled);
    let mut unposted = c("a failed publication", "findings", 4, "");
    unposted.comment_exit = "1";
    cases.push(unposted);
    let mut unposted_skip = c(
        "a skip notice that could not be posted",
        "no-findings",
        4,
        "",
    );
    unposted_skip.extra.push(("IS_DRAFT", "true".into()));
    unposted_skip.comment_exit = "1";
    cases.push(unposted_skip);
    cases
}

/// What a blocking review case's report must say.
fn review_reason(label: &str) -> &'static str {
    match label {
        "empty output" => "wrote nothing",
        "unrelated output" => "carries no verdict block",
        "a verdict for another head" => "not the subject",
        "a finding outside the diff" => "which this diff does not change",
        "a findings verdict listing none" => "a findings verdict with no findings",
        "a no-findings verdict listing some" => "a no-findings verdict that lists findings",
        "a verdict that is neither" => "is neither findings nor no-findings",
        "an explicit refusal" | "a refusal outranks a transient signal" => "the provider refused",
        "an unclassified failure" => "no recognized transient signal",
        "a missing credential" => "is not set for this repository",
        "a stale subject" => "stale subject",
        "the current head cannot be read" => "the current head could not be read",
        "the reviewer cannot be installed" => "could not be installed",
        "a failed publication" => "the review could not be posted",
        "a skip notice that could not be posted" => "the skip notice could not be posted",
        other => panic!("no stated reason for `{other}`"),
    }
}

fn review_ctx(repo: &Repo) -> BTreeMap<String, String> {
    let mut ctx = BTreeMap::new();
    for (k, v) in [
        ("secrets.CLAUDE_CODE_OAUTH_TOKEN", "stub-credential-value"),
        ("github.token", "stub-github-token"),
        ("github.event.pull_request.number", "7"),
        ("github.repository", "owner/fixture"),
        (
            "github.event.pull_request.head.repo.full_name",
            "owner/fixture",
        ),
        ("github.head_ref", "topic"),
        ("github.actor", "someone"),
        ("github.event.pull_request.draft", "false"),
    ] {
        ctx.insert(k.to_string(), v.to_string());
    }
    ctx.insert(
        "github.event.pull_request.base.sha".into(),
        repo.base.clone(),
    );
    ctx.insert(
        "github.event.pull_request.head.sha".into(),
        repo.head.clone(),
    );
    ctx
}

fn run_review(repo: &Repo, case: &Case) -> Ran {
    let wf = workflow(repo.root(), "statecraft-ai-review.yml");
    let (run, env) = step(&wf, "review", "Review");
    let extra: Vec<(&str, &str)> = case.extra.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let head = if case.moved_head {
        "2222222222222222222222222222222222222222".to_string()
    } else {
        repo.head.clone()
    };
    run_step(
        repo.root(),
        &run,
        &env,
        &review_ctx(repo),
        &extra,
        |stubs| {
            std::fs::write(stubs.join("claude-mode"), case.mode).unwrap();
            std::fs::write(stubs.join("gh-comment-exit"), case.comment_exit).unwrap();
            if !case.unreadable_head {
                std::fs::write(stubs.join("gh-head"), format!("{head}\n")).unwrap();
            }
            std::fs::write(stubs.join("npm-exit"), case.npm_exit).unwrap();
        },
    )
}

#[test]
fn ai_review_classifies_every_case() {
    let repo = Repo::new(&["scripts/statecraft/ai-review.sh"], &[]);
    for case in review_cases() {
        let ran = run_review(&repo, &case);
        assert_eq!(ran.exit, case.want_exit, "{}:\n{}", case.label, ran.text);
        if case.want_exit != 0 {
            assert!(
                ran.text.contains(review_reason(case.label)),
                "{}:\n{}",
                case.label,
                ran.text
            );
        }
        assert_eq!(
            ran.output("result"),
            case.want_result,
            "{}:\n{}",
            case.label,
            ran.text
        );
        assert_eq!(ran.output("release_candidate"), "false", "{}", case.label);
        if case.want_exit == 0 {
            // Evidence exists for a review and for a visible skip, so the
            // upload step's `if:` holds and its failure fails the job.
            let dir = ran.output("evidence");
            assert!(!dir.is_empty(), "{}: no evidence output", case.label);
            let record: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(Path::new(dir).join("ai-review-evidence.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(record["subject"]["head"], repo.head, "{}", case.label);
            assert_eq!(record["subject"]["base"], repo.base, "{}", case.label);
            assert_eq!(record["subject"]["pullRequest"], 7, "{}", case.label);
            assert_eq!(record["result"], case.want_result, "{}", case.label);
            assert_eq!(record["tool"]["version"], setup::REVIEW_TOOL_VERSION);
        }
        let called = ran.stub_file("claude-called").is_some();
        let skipped_early = matches!(
            case.want_result,
            "skipped:fork" | "skipped:draft" | "skipped:dependabot" | "skipped:oversized"
        ) || matches!(
            case.label,
            "a missing credential"
                | "a skip notice that could not be posted"
                | "the reviewer cannot be installed"
        );
        assert_eq!(
            called, !skipped_early,
            "{}: reviewer invoked: {called}",
            case.label
        );
        if called {
            // The reviewer ran from an empty directory with a temporary HOME.
            let listing = ran.stub_file("claude-cwd-listing").unwrap();
            assert!(
                listing.trim().is_empty(),
                "{}: cwd not empty: {listing}",
                case.label
            );
            let home = ran.stub_file("claude-home").unwrap();
            assert!(
                home.trim().starts_with(ran.runner.path().to_str().unwrap())
                    || home.contains("ai-review-home"),
                "{}: {home}",
                case.label
            );
        }
        let comments = ran
            .stub_file("gh-calls")
            .unwrap_or_default()
            .lines()
            .filter(|l| l.starts_with("pr comment"))
            .count();
        let posted = matches!(
            case.want_result,
            "findings"
                | "no-findings"
                | "skipped:draft"
                | "skipped:oversized"
                | "skipped:transient"
        );
        if posted {
            assert_eq!(comments, 1, "{}: comments {comments}", case.label);
        }
        if matches!(case.want_result, "skipped:fork" | "skipped:dependabot") {
            assert_eq!(comments, 0, "{}: a read-only token cannot post", case.label);
        }
        // The credential value never appears in anything the step printed.
        assert!(
            !ran.text.contains("stub-credential-value"),
            "{}",
            case.label
        );
    }
}

#[test]
fn contributor_text_is_never_spliced_into_a_run_scalar() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "spec-spine.toml", TOML);
    render(tmp.path());
    for name in ["statecraft-ci.yml", "statecraft-ai-review.yml"] {
        let wf = workflow(tmp.path(), name);
        for (job, def) in wf["jobs"].as_mapping().unwrap() {
            for s in def["steps"].as_sequence().into_iter().flatten() {
                if let Some(run) = s["run"].as_str() {
                    assert!(
                        !run.contains("${{"),
                        "{name} {job:?}: an expression in a run scalar: {run}"
                    );
                }
                if let Some(uses) = s["uses"].as_str() {
                    let pinned = uses.split_once('@').is_some_and(|(_, r)| {
                        r.len() == 40 && r.bytes().all(|b| b.is_ascii_hexdigit())
                    });
                    assert!(pinned, "{name}: `{uses}` is not pinned by commit");
                }
            }
        }
        assert!(wf["on"]["pull_request_target"].is_null(), "{name}");
    }
    // The caller forwards exactly one secret, by name; never `inherit`.
    let caller = workflow(tmp.path(), "statecraft-ci.yml");
    let secrets = caller["jobs"]["ai-review"]["secrets"].as_mapping().unwrap();
    assert_eq!(secrets.len(), 1);
    assert!(secrets.contains_key(setup::CREDENTIAL));
    // The workflow default is read-only; the review job adds only its comment.
    let ci = workflow(tmp.path(), "statecraft-ci.yml");
    assert_eq!(ci["permissions"]["contents"].as_str(), Some("read"));
    let review = workflow(tmp.path(), "statecraft-ai-review.yml");
    let perms = review["jobs"]["review"]["permissions"]
        .as_mapping()
        .unwrap();
    assert_eq!(perms.len(), 2);
    assert_eq!(
        review["jobs"]["review"]["permissions"]["pull-requests"].as_str(),
        Some("write")
    );
    // The upload step runs whenever evidence was made, and a missing file fails.
    let steps = review["jobs"]["review"]["steps"].as_sequence().unwrap();
    let upload = steps
        .iter()
        .find(|s| s["name"].as_str() == Some("Upload the evidence record"))
        .unwrap();
    assert_eq!(
        upload["if"].as_str(),
        Some("steps.review.outputs.evidence != ''")
    );
    assert_eq!(upload["with"]["if-no-files-found"].as_str(), Some("error"));
    assert_eq!(
        upload["with"]["name"].as_str(),
        Some("statecraft-ai-review-${{ github.event.pull_request.head.sha }}")
    );
}

#[test]
fn inverting_a_blocking_branch_of_ai_review_is_noticed() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "spec-spine.toml", TOML);
    render(tmp.path());
    let script =
        std::fs::read_to_string(tmp.path().join("scripts/statecraft/ai-review.sh")).unwrap();
    let lines: Vec<&str> = script.lines().collect();
    let sites: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            (t.contains("refuse \"") || t.contains("|| refuse")) && !t.starts_with("refuse()")
        })
        .map(|(i, _)| i)
        .collect();
    assert!(
        sites.len() >= 12,
        "found only {} blocking branches",
        sites.len()
    );
    let cases = review_cases();
    // One repository; the base does not carry the script, so the mutated
    // candidate copy is the one the step runs.
    let repo = Repo::new(&[], &[]);
    let target = repo.root().join("scripts/statecraft/ai-review.sh");
    for site in sites {
        let mutated: String = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i == site {
                    format!(
                        "{}\n",
                        l.replacen("refuse \"", "true \"", 1)
                            .replacen("|| refuse", "|| true", 1)
                    )
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        std::fs::write(&target, &mutated).unwrap();
        // Noticed: a case ends differently, or a block no longer says why.
        let noticed = cases.iter().any(|case| {
            let ran = run_review(&repo, case);
            ran.exit != case.want_exit
                || ran.output("result") != case.want_result
                || (case.want_exit != 0 && !ran.text.contains(review_reason(case.label)))
        });
        assert!(
            noticed,
            "inverting line {} went unnoticed: {}",
            site + 1,
            lines[site]
        );
    }
}

// ------------------------------------------------- governance (revision 4)

/// The authored-content script a project declares: this repository's own,
/// whose `--text` mode is spec 001 section 3.6's contract.
const CHECK_AUTHORED: &str = include_str!("../../../scripts/check-authored-content.sh");
const DECLARED: &str = "scripts/check-authored-content.sh";
/// U+2014, spelled as an escape so this file stays clean.
const EM: &str = "\u{2014}";

/// A stubbed spec-spine: every verb passes except `index coverage`, which
/// finds `src/untraced.rs` untraced and refuses it only under
/// `--fail-on-untraced`, as the real flag does.
const SPEC_SPINE: &str = r#"#!/bin/sh
printf '%s | %s\n' "$PWD" "$*" >> "$STUB_STATE/spec-spine-calls"
case "$*" in
  --version) echo "spec-spine 0.25.0" ;;
  "index coverage"*)
    if [ -f src/untraced.rs ]; then
      echo "untraced: src/untraced.rs"
      case "$*" in *--fail-on-untraced*) exit 1 ;; esac
    else
      echo "every file is traced"
    fi ;;
esac
exit 0
"#;

/// A stubbed cargo: `fmt --all --check` fails on a tree carrying
/// `src/unformatted.rs`; every call is recorded with its directory.
const CARGO: &str = r#"#!/bin/sh
printf '%s | %s\n' "$PWD" "$*" >> "$STUB_STATE/cargo-calls"
if [ "$1" = metadata ]; then
  if [ -f "$STUB_STATE/no-members" ]; then
    echo '{"packages": [], "workspace_members": [ ], "version": 1}'
  else
    echo '{"packages":[],"workspace_members":["fixture 0.1.0 (path+file:///fixture)"],"version":1}'
  fi
  exit 0
fi
if [ "$1" = fmt ] && [ -f src/unformatted.rs ]; then
  echo "Diff in src/unformatted.rs"
  exit 1
fi
exit 0
"#;

fn gov_bin() -> &'static Path {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap().keep();
        for (name, body) in [("spec-spine", SPEC_SPINE), ("cargo", CARGO)] {
            statecraft_adapter::fixture::install_script(&dir.join(name), body, 0o755).unwrap();
        }
        dir
    })
}

/// A rendered project with the declared script committed at its base, on a
/// topic branch.
struct Gov {
    dir: tempfile::TempDir,
    base: String,
}

impl Gov {
    fn new(params: &[(&str, serde_json::Value)]) -> Gov {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            git(root, &args);
        }
        write(root, "src/lib.rs", "pub fn one() -> u32 {\n    1\n}\n");
        write(root, "README.md", "# fixture\n");
        write(root, "spec-spine.toml", TOML);
        write(root, ".gitignore", ".tooling/\n");
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        statecraft_adapter::fixture::install_script(&root.join(DECLARED), CHECK_AUTHORED, 0o755)
            .unwrap();
        render_with(root, params);
        std::fs::create_dir_all(root.join(".tooling/bin")).unwrap();
        std::os::unix::fs::symlink(
            gov_bin().join("spec-spine"),
            root.join(".tooling/bin/spec-spine"),
        )
        .unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "--quiet", "-m", "base"]);
        let base = git(root, &["rev-parse", "HEAD"]);
        git(root, &["checkout", "--quiet", "-b", "topic"]);
        Gov { dir, base }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// The adoption (revision 5, rule 1): a base that carries none of the
    /// profile, and a topic commit that adds it and the declared script.
    fn adoption(params: &[(&str, serde_json::Value)]) -> Gov {
        let gov = Gov::new(params);
        let root = gov.root();
        git(root, &["checkout", "--quiet", "--orphan", "bare"]);
        git(root, &["rm", "-r", "--cached", "--quiet", "."]);
        git(
            root,
            &["add", "src", "README.md", "spec-spine.toml", ".gitignore"],
        );
        git(root, &["commit", "--quiet", "-m", "a base with no gate"]);
        let base = git(root, &["rev-parse", "HEAD"]);
        git(root, &["checkout", "--quiet", "-B", "topic"]);
        git(root, &["add", "-A"]);
        git(root, &["commit", "--quiet", "-m", "adopt the gate"]);
        Gov { base, ..gov }
    }

    /// Commit `edits` (`None` deletes) with `message`; the new head.
    fn commit(&self, edits: &[(&str, Option<&str>)], message: &str) -> String {
        for (rel, text) in edits {
            match text {
                Some(t) => write(self.root(), rel, t),
                None => std::fs::remove_file(self.root().join(rel)).unwrap(),
            }
            git(self.root(), &["add", "-A", rel]);
        }
        git(
            self.root(),
            &["commit", "--quiet", "--allow-empty", "-m", message],
        );
        git(self.root(), &["rev-parse", "HEAD"])
    }

    fn short(&self, sha: &str) -> String {
        git(self.root(), &["rev-parse", "--short", sha])
    }
}

struct Event<'a> {
    name: &'a str,
    base_ref: &'a str,
    title: &'a str,
    body: &'a str,
    group_ref: &'a str,
    head: &'a str,
}

fn event<'a>(name: &'a str, head: &'a str) -> Event<'a> {
    Event {
        name,
        base_ref: "main",
        title: "a clean title",
        body: "a clean body",
        group_ref: if name == "merge_group" { QUEUE_REF } else { "" },
        head,
    }
}

/// Run one step of the rendered governance job, as a runner would.
fn run_gov(gov: &Gov, name: &str, ev: &Event<'_>, stubs: impl Fn(&Path)) -> Ran {
    run_gov_job(gov, "governance", name, ev, stubs)
}

/// The expressions the governance and code jobs use, for one event.
fn gov_ctx(gov: &Gov, ev: &Event<'_>) -> BTreeMap<String, String> {
    let mut ctx = BTreeMap::new();
    for (k, v) in [
        ("github.event_name", ev.name),
        ("github.event.pull_request.base.ref", ev.base_ref),
        ("github.event.pull_request.title", ev.title),
        ("github.event.pull_request.body", ev.body),
        ("github.event.merge_group.head_ref", ev.group_ref),
        (
            "github.event.pull_request.base.sha || github.event.merge_group.base_sha",
            gov.base.as_str(),
        ),
        (
            "github.event.pull_request.head.sha || github.event.merge_group.head_sha",
            ev.head,
        ),
        // Revision 5: the base the gate is read at, on every event (on push
        // it is the event's `before`), and the pull request's endpoints the
        // authority change is computed from.
        (
            "github.event.pull_request.base.sha || github.event.merge_group.base_sha || github.event.before",
            gov.base.as_str(),
        ),
        ("github.event.pull_request.base.sha", gov.base.as_str()),
        ("github.event.pull_request.head.sha", ev.head),
        ("github.repository", "owner/fixture"),
        ("github.token", "fixture-token"),
    ] {
        ctx.insert(k.to_string(), v.to_string());
    }
    ctx
}

/// Run one step of a rendered job that runs the gate, after the job's step
/// that reads the gate at the base.
fn run_gov_job(gov: &Gov, job: &str, name: &str, ev: &Event<'_>, stubs: impl Fn(&Path)) -> Ran {
    let path = format!(
        "{}:{}:{}",
        gov_bin().display(),
        stub_bin().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    run_job_step(
        gov.root(),
        job,
        name,
        &gov_ctx(gov, ev),
        &[("PATH", &path)],
        stubs,
    )
}

/// Rule 1: `governance.enforce_coverage` makes coverage a refusal; its
/// default reports.
#[test]
fn coverage_enforced_refuses_an_untraced_file_and_reported_does_not() {
    for enforce in [true, false] {
        let gov = Gov::new(&[("governance.enforce_coverage", serde_json::json!(enforce))]);
        write(gov.root(), "src/untraced.rs", "pub fn three() {}\n");
        let head = git(gov.root(), &["rev-parse", "HEAD"]);
        let ran = run_gov(&gov, "Governance", &event("push", &head), |_| {});
        let calls = ran.stub_file("spec-spine-calls").unwrap_or_default();
        assert!(
            ran.text.contains("untraced: src/untraced.rs"),
            "{}",
            ran.text
        );
        if enforce {
            assert_eq!(ran.exit, 1, "{}", ran.text);
            assert!(
                calls.contains("index coverage --fail-on-untraced"),
                "{calls}"
            );
        } else {
            assert_eq!(ran.exit, 0, "{}", ran.text);
            assert!(!calls.contains("--fail-on-untraced"), "{calls}");
            assert!(calls.contains("index coverage"), "{calls}");
        }
    }
}

/// Rule 2: a declared authored-content script is required, and runs; an
/// undeclared one runs nothing, and the gate says so.
#[test]
fn a_declared_authored_content_script_is_required_and_an_undeclared_one_runs_nothing() {
    let declared = [("governance.authored_content", serde_json::json!(DECLARED))];
    let head = |gov: &Gov| git(gov.root(), &["rev-parse", "HEAD"]);

    // Declared and clean: it runs and passes.
    let gov = Gov::new(&declared);
    let ran = run_gov(&gov, "Governance", &event("push", &head(&gov)), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(ran.text.contains("authored file(s) clean"), "{}", ran.text);

    // Declared, and the tree breaks its rule: it runs and refuses.
    write(gov.root(), "README.md", &format!("# fixture {EM} x\n"));
    let ran = run_gov(&gov, "Governance", &event("push", &head(&gov)), |_| {});
    assert_eq!(ran.exit, 1, "{}", ran.text);
    assert!(ran.text.contains("U+2014"), "{}", ran.text);

    // Declared and absent: refused, never a silent pass. Revision 5 reads
    // the script at the base, so "absent" is absent there and in the
    // candidate: the base carries none, the candidate's copy is looked for,
    // and there is none.
    let mut gov = Gov::new(&declared);
    gov.base = gov.commit(&[(DECLARED, None)], "remove the script");
    let ran = run_gov(&gov, "Governance", &event("push", &head(&gov)), |_| {});
    // Revision 7: refused (2), a precondition the operator supplies.
    assert_eq!(ran.exit, 2, "{}", ran.text);
    assert!(ran.text.contains("which is absent"), "{}", ran.text);
    assert!(
        ran.text.contains(&format!(
            "the base carries no {DECLARED}; the candidate's copy runs (adoption)"
        )),
        "{}",
        ran.text
    );

    // Declared and not executable at the base: refused.
    let mut gov = Gov::new(&declared);
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        gov.root().join(DECLARED),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    gov.base = gov.commit(
        &[(DECLARED, Some(CHECK_AUTHORED))],
        "drop the executable bit",
    );
    let ran = run_gov(&gov, "Governance", &event("push", &head(&gov)), |_| {});
    assert_eq!(ran.exit, 2, "{}", ran.text);
    assert!(ran.text.contains("which is not executable"), "{}", ran.text);

    // Revision 5: absent or not executable only in the candidate, the base's
    // copy still runs. The candidate's change is an authority change, which
    // ci-gate blocks without the owner's exception (tested below).
    let gov = Gov::new(&declared);
    std::fs::remove_file(gov.root().join(DECLARED)).unwrap();
    let ran = run_gov(&gov, "Governance", &event("push", &head(&gov)), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(
        ran.text
            .contains(&format!("{DECLARED} read at the base {}", gov.base)),
        "{}",
        ran.text
    );

    // Undeclared: the script is present and would refuse, and nothing runs it.
    let gov = Gov::new(&[]);
    write(gov.root(), "README.md", &format!("# fixture {EM} x\n"));
    let ran = run_gov(&gov, "Governance", &event("push", &head(&gov)), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(
        ran.text
            .contains("no authored-content script is declared (governance.authored_content)"),
        "{}",
        ran.text
    );
    assert!(!ran.text.contains("U+2014"), "{}", ran.text);
}

/// Rule 3: the declared script's `--text` mode judges the pull request's
/// title and body (from the event, and through the API in the queue) and
/// every commit message in the change.
#[test]
fn the_text_mode_refuses_a_u2014_in_a_title_a_body_and_a_commit_message() {
    let gov = Gov::new(&[
        ("governance.authored_content", serde_json::json!(DECLARED)),
        ("governance.authored_content_text", serde_json::json!(true)),
    ]);
    let clean = gov.commit(&[("src/two.rs", Some("pub fn two() {}\n"))], "add two");
    let step = "Authored-content rules (title and body)";
    let dirty = format!("a title {EM} with a dash");
    let dirty_body = format!("a body {EM} with a dash");

    let ran = run_gov(&gov, step, &event("pull_request", &clean), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);
    let ran = run_gov(
        &gov,
        step,
        &Event {
            title: &dirty,
            ..event("pull_request", &clean)
        },
        |_| {},
    );
    assert_eq!(ran.exit, 1, "title: {}", ran.text);
    assert!(ran.text.contains("U+2014"), "{}", ran.text);
    let ran = run_gov(
        &gov,
        step,
        &Event {
            body: &dirty_body,
            ..event("pull_request", &clean)
        },
        |_| {},
    );
    assert_eq!(ran.exit, 1, "body: {}", ran.text);

    // In the queue the event carries neither; the entry's pull request is
    // read through the API.
    for (answer, want) in [
        ("a clean title\na clean body\n".to_string(), 0),
        (format!("a clean title\n{dirty_body}\n"), 1),
    ] {
        let ran = run_gov(&gov, step, &event("merge_group", &clean), |state| {
            std::fs::write(state.join("gh-head"), &answer).unwrap();
        });
        assert_eq!(ran.exit, want, "{answer}: {}", ran.text);
        let calls = ran.stub_file("gh-calls").unwrap_or_default();
        assert!(calls.contains("api repos/owner/fixture/pulls/7"), "{calls}");
    }

    // Every commit message in base..head, not only the head's.
    let walk = "Every commit in the change";
    let ran = run_gov(&gov, walk, &event("pull_request", &clean), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);
    let bad = gov.commit(&[], &format!("a message {EM} with a dash"));
    let head = gov.commit(
        &[("src/three.rs", Some("pub fn three() {}\n"))],
        "add three",
    );
    let ran = run_gov(&gov, walk, &event("pull_request", &head), |_| {});
    assert_eq!(ran.exit, 1, "{}", ran.text);
    assert!(
        ran.text.contains(&format!(
            "{}'s message breaks the authored-content rules",
            gov.short(&bad)
        )),
        "{}",
        ran.text
    );

    // Not enabled: the title is not judged.
    let off = Gov::new(&[("governance.authored_content", serde_json::json!(DECLARED))]);
    let head = off.commit(&[], "empty");
    let ran = run_gov(
        &off,
        step,
        &Event {
            title: &dirty,
            ..event("pull_request", &head)
        },
        |_| {},
    );
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(ran.text.contains("not judged"), "{}", ran.text);
}

/// Rule 4: the walk refuses an unsigned commit, and a commit whose own tree
/// fails the gate or the format check while the head passes.
#[test]
fn the_commit_walk_refuses_an_unsigned_commit_and_a_red_intermediate_tree() {
    let gov = Gov::new(&[
        ("governance.authored_content", serde_json::json!(DECLARED)),
        ("governance.gate_each_commit", serde_json::json!(true)),
        ("governance.require_signed_commits", serde_json::json!(true)),
    ]);
    let dashed = gov.commit(
        &[("README.md", Some(&format!("# fixture {EM} x\n")))],
        "a dash",
    );
    let fixed = gov.commit(&[("README.md", Some("# fixture\n"))], "no dash");
    let unformatted = gov.commit(
        &[("src/unformatted.rs", Some("fn  x(){}\n"))],
        "unformatted",
    );
    let head = gov.commit(&[("src/unformatted.rs", None)], "formatted");
    let walk = "Every commit in the change";

    // The head passes the gate on its own.
    let ran = run_gov(&gov, "Governance", &event("pull_request", &head), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);

    let ran = run_gov(&gov, walk, &event("pull_request", &head), |_| {});
    assert_eq!(ran.exit, 1, "{}", ran.text);
    for red in [&dashed, &unformatted] {
        assert!(
            ran.text.contains(&format!(
                "{} fails the gate or the format check at its own tree",
                gov.short(red)
            )),
            "{}",
            ran.text
        );
    }
    for green in [&fixed, &head] {
        assert!(
            ran.text.contains(&format!(
                "{}: the gate and the format check pass at its own tree",
                gov.short(green)
            )),
            "{}",
            ran.text
        );
    }
    // Each commit whose gate passed ran the format check at its own tree (the
    // dashed one stopped at the gate), and GitHub was asked about each
    // commit's signature.
    let cargo = ran.stub_file("cargo-calls").unwrap_or_default();
    assert_eq!(cargo.matches("fmt --all --check").count(), 3, "{cargo}");
    for c in [&fixed, &unformatted, &head] {
        assert!(
            cargo.contains(&format!(
                "statecraft-commit-{} | fmt --all --check",
                gov.short(c)
            )),
            "{cargo}"
        );
    }
    let calls = ran.stub_file("gh-calls").unwrap_or_default();
    for c in [&dashed, &fixed, &unformatted, &head] {
        assert!(
            calls.contains(&format!("api repos/owner/fixture/commits/{c}")),
            "{calls}"
        );
    }
    assert!(!ran.text.contains("is not signed"), "{}", ran.text);
    // The worktrees are removed.
    assert_eq!(
        git(gov.root(), &["worktree", "list", "--porcelain"])
            .matches("worktree ")
            .count(),
        1
    );

    // Signatures alone: every commit green, one unverified.
    let signed = Gov::new(&[("governance.require_signed_commits", serde_json::json!(true))]);
    let one = signed.commit(&[("src/two.rs", Some("pub fn two() {}\n"))], "two");
    let two = signed.commit(&[("src/three.rs", Some("pub fn three() {}\n"))], "three");
    let ran = run_gov(&signed, walk, &event("merge_group", &two), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);
    let ran = run_gov(&signed, walk, &event("merge_group", &two), |state| {
        std::fs::write(state.join("gh-unsigned"), format!("{one}\n")).unwrap();
    });
    assert_eq!(ran.exit, 1, "{}", ran.text);
    assert!(
        ran.text.contains(&format!(
            "{} is not signed with a key GitHub verifies",
            signed.short(&one)
        )),
        "{}",
        ran.text
    );
    assert!(
        !ran.text
            .contains(&format!("{} is not signed", signed.short(&two)))
    );

    // Nothing enabled: the walk judges nothing and says so.
    let off = Gov::new(&[]);
    let head = off.commit(&[], "empty");
    let ran = run_gov(&off, walk, &event("pull_request", &head), |state| {
        std::fs::write(state.join("gh-unsigned"), format!("{head}\n")).unwrap();
    });
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(
        ran.text.contains("no per-commit check is enabled"),
        "{}",
        ran.text
    );
}

/// The code job's gate judges nothing in a workspace with no member crates
/// yet, and says so, instead of failing on the virtual manifest; with a member
/// it runs all four verbs (the review of #138, finding 1).
#[test]
fn the_code_gate_judges_nothing_without_member_crates_and_says_so() {
    let gov = Gov::new(&[]);
    let head = git(gov.root(), &["rev-parse", "HEAD"]);
    for empty in [true, false] {
        // Revision 5: the step runs after the job reads the gate at the base.
        let ran = run_gov_job(
            &gov,
            "code",
            "Build, test, clippy, fmt",
            &event("push", &head),
            |state| {
                if empty {
                    std::fs::write(state.join("no-members"), "").unwrap();
                }
            },
        );
        assert_eq!(ran.exit, 0, "{}", ran.text);
        let calls = ran.stub_file("cargo-calls").unwrap_or_default();
        let verbs = [
            "build --workspace",
            "test --workspace",
            "clippy --workspace",
            "fmt --all",
        ];
        if empty {
            assert!(
                ran.text.contains("the workspace has no member crates yet"),
                "{}",
                ran.text
            );
            for verb in verbs {
                assert!(!calls.contains(verb), "{calls}");
            }
        } else {
            for verb in verbs {
                assert!(calls.contains(verb), "{calls}");
            }
        }
    }
}

/// A skip notice is posted once per class and head: a re-run of the review
/// job finds the notice it already posted and does not repeat it, and still
/// records the skip (the second review of #138, finding 1).
#[test]
fn a_rerun_does_not_repeat_a_skip_notice() {
    let repo = Repo::new(&["scripts/statecraft/ai-review.sh"], &[]);
    let mut draft = review_cases()
        .into_iter()
        .find(|c| c.label == "a draft")
        .expect("the draft case");
    draft.comment_exit = "0";
    let first = run_review(&repo, &draft);
    assert_eq!(first.exit, 0, "{}", first.text);
    assert_eq!(first.output("result"), "skipped:draft");
    let posted = first.stub_file("gh-comments").unwrap_or_default();
    assert!(
        posted.contains(&format!(
            "<!-- statecraft-ai-review-skip draft {} -->",
            repo.head
        )),
        "{posted}"
    );

    // The re-run sees the thread the first run left.
    let wf = workflow(repo.root(), "statecraft-ai-review.yml");
    let (run, env) = step(&wf, "review", "Review");
    let extra: Vec<(&str, &str)> = draft.extra.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let again = run_step(
        repo.root(),
        &run,
        &env,
        &review_ctx(&repo),
        &extra,
        |stubs| {
            std::fs::write(stubs.join("claude-mode"), draft.mode).unwrap();
            std::fs::write(stubs.join("gh-comment-exit"), "0").unwrap();
            std::fs::write(stubs.join("gh-head"), format!("{}\n", repo.head)).unwrap();
            std::fs::write(stubs.join("npm-exit"), "0").unwrap();
            std::fs::write(stubs.join("gh-comments"), &posted).unwrap();
        },
    );
    assert_eq!(again.exit, 0, "{}", again.text);
    assert_eq!(again.output("result"), "skipped:draft");
    let calls = again.stub_file("gh-calls").unwrap_or_default();
    assert!(
        !calls.lines().any(|l| l.starts_with("pr comment")),
        "{calls}"
    );
    assert!(again.text.contains("already posted"), "{}", again.text);
}

/// A commit whose `spec-spine.toml` states no exact pin is refused, and the
/// refusal prints why: the reason is in the log the refusal shows, not only
/// on another stream (the review of #138, finding 2).
#[test]
fn a_commit_without_an_exact_pin_is_refused_and_says_why() {
    let gov = Gov::new(&[("governance.gate_each_commit", serde_json::json!(true))]);
    let unpinned = gov.commit(&[("spec-spine.toml", Some("[meta]\n"))], "unpinned");
    let head = gov.commit(&[("spec-spine.toml", Some(TOML))], "pinned again");
    let ran = run_gov(
        &gov,
        "Every commit in the change",
        &event("pull_request", &head),
        |_| {},
    );
    assert_eq!(ran.exit, 1, "{}", ran.text);
    let short = gov.short(&unpinned);
    assert!(
        ran.text.contains(&format!(
            "{short} fails the gate or the format check at its own tree"
        )),
        "{}",
        ran.text
    );
    // Once on stderr and once from the log the refusal prints.
    let reason = format!("{short}'s spec-spine.toml states no exact pin, so no gate can judge it");
    assert_eq!(ran.text.matches(&reason).count(), 2, "{}", ran.text);
}

/// Rule 5: a pull request whose base is not the default branch fails
/// governance, by default; the default branch passes.
#[test]
fn a_base_other_than_the_default_branch_refuses() {
    let step = "The base is the default branch";
    let gov = Gov::new(&[]);
    let head = gov.commit(&[], "empty");
    let ran = run_gov(&gov, step, &event("pull_request", &head), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);
    let stacked = Event {
        base_ref: "feature/below",
        ..event("pull_request", &head)
    };
    let ran = run_gov(&gov, step, &stacked, |_| {});
    assert_eq!(ran.exit, 1, "{}", ran.text);
    assert!(
        ran.text
            .contains("base is 'feature/below', not the default branch main"),
        "{}",
        ran.text
    );
    let off = Gov::new(&[("governance.require_default_base", serde_json::json!(false))]);
    let head = off.commit(&[], "empty");
    let ran = run_gov(
        &off,
        step,
        &Event {
            base_ref: "feature/below",
            ..event("pull_request", &head)
        },
        |_| {},
    );
    assert_eq!(ran.exit, 0, "{}", ran.text);
}

/// Rules 4, 6 and 7 in the rendered workflow: the new checks are steps of the
/// governance job (never jobs that can be skipped), every job that holds them
/// only reads, the gate job stays read-only, and the caches decide nothing.
#[test]
fn revision_four_adds_steps_not_jobs_and_keeps_the_gate_read_only() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "spec-spine.toml", TOML);
    render(tmp.path());
    let wf = workflow(tmp.path(), "statecraft-ci.yml");
    let jobs: Vec<&str> = wf["jobs"]
        .as_mapping()
        .unwrap()
        .keys()
        .map(|k| k.as_str().unwrap())
        .collect();
    assert_eq!(
        jobs,
        [
            "governance",
            "code",
            "ai-review",
            "review-exception",
            "ci-gate"
        ]
    );
    for (job, def) in [("governance", "governance"), ("ci-gate", "ci-gate")] {
        for (k, v) in wf["jobs"][def]["permissions"].as_mapping().unwrap() {
            assert_eq!(v.as_str(), Some("read"), "{job} may only read: {k:?}");
        }
    }
    let steps = wf["jobs"]["governance"]["steps"].as_sequence().unwrap();
    let named = |name: &str| {
        steps
            .iter()
            .find(|s| s["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("no step {name}"))
    };
    let queue_or_pr = "github.event_name == 'pull_request' || github.event_name == 'merge_group'";
    assert_eq!(
        named("Every commit in the change")["if"].as_str(),
        Some(queue_or_pr)
    );
    assert_eq!(
        named("Authored-content rules (title and body)")["if"].as_str(),
        Some(queue_or_pr)
    );
    assert_eq!(
        named("The base is the default branch")["if"].as_str(),
        Some("github.event_name == 'pull_request'")
    );
    assert!(named("Governance")["if"].is_null());
    // The caches: the binary keyed on the pin, cargo on the lockfile and the
    // toolchain. The install step always runs, so a restored binary is still
    // checked against the pin.
    for job in ["governance", "code"] {
        let steps = wf["jobs"][job]["steps"].as_sequence().unwrap();
        let find = |name: &str| steps.iter().find(|s| s["name"].as_str() == Some(name));
        let cache = find("Cache spec-spine").unwrap_or_else(|| panic!("{job}: no cache"));
        assert!(
            cache["uses"]
                .as_str()
                .unwrap()
                .starts_with("actions/cache@")
        );
        assert_eq!(
            cache["with"]["path"].as_str(),
            Some(".tooling/bin/spec-spine")
        );
        assert!(
            cache["with"]["key"]
                .as_str()
                .unwrap()
                .contains("steps.pin.outputs.version"),
            "{job}"
        );
        assert!(find("Install the pinned spec-spine").unwrap()["if"].is_null());
    }
    let code = wf["jobs"]["code"]["steps"].as_sequence().unwrap();
    let cargo = code
        .iter()
        .find(|s| s["name"].as_str() == Some("Cache cargo registry, git and target"))
        .unwrap();
    assert!(
        cargo["with"]["key"]
            .as_str()
            .unwrap()
            .contains("hashFiles('Cargo.lock', 'rust-toolchain.toml')")
    );
    for path in ["~/.cargo/registry", "~/.cargo/git", "target"] {
        assert!(cargo["with"]["path"].as_str().unwrap().contains(path));
    }
    // The pin step answers the key from the pin itself. Revision 5: it runs
    // the gate the job read; with no base named, the candidate's copy.
    let mut ctx = BTreeMap::new();
    ctx.insert(
        "github.event.pull_request.base.sha || github.event.merge_group.base_sha || github.event.before".to_string(),
        String::new(),
    );
    let ran = run_job_step(
        tmp.path(),
        "governance",
        "Read the spec-spine pin",
        &ctx,
        &[],
        |_| {},
    );
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert_eq!(ran.output("version"), "0.25.0");
    // The defaults keep a revision-3 project's behaviour, except the base.
    let gate = std::fs::read_to_string(tmp.path().join("scripts/statecraft/gate.sh")).unwrap();
    for line in [
        "ENFORCE_COVERAGE=false",
        "AUTHORED_CONTENT=''",
        "AUTHORED_CONTENT_TEXT=false",
        "GATE_EACH_COMMIT=false",
        "REQUIRE_SIGNED_COMMITS=false",
        "REQUIRE_DEFAULT_BASE=true",
        "DEFAULT_BRANCH='main'",
    ] {
        assert!(gate.contains(line), "{line}");
    }
    assert!(!gate.contains("if [ -x scripts/check-authored-content.sh ]"));
}

// ------------------------------------------------------------- revision 5

/// Rule 1: a candidate that weakens `gate.sh`, and the declared
/// authored-content script, is judged by the base's copies and fails: in the
/// governance step, the title and body, the commit walk (every commit judged
/// by the base's gate, never its own) and the code job.
#[test]
fn a_candidate_that_weakens_the_gate_is_judged_by_the_base_copy_and_fails() {
    let gov = Gov::new(&[
        ("governance.authored_content", serde_json::json!(DECLARED)),
        ("governance.authored_content_text", serde_json::json!(true)),
        ("governance.gate_each_commit", serde_json::json!(true)),
    ]);
    let weakened = gov.commit(
        &[
            (
                "scripts/statecraft/gate.sh",
                Some("#!/bin/sh\necho weakened gate\nexit 0\n"),
            ),
            (
                DECLARED,
                Some("#!/usr/bin/env bash\necho weakened check\nexit 0\n"),
            ),
            ("README.md", Some(&format!("# fixture {EM} x\n"))),
            ("src/unformatted.rs", Some("fn  x(){}\n")),
        ],
        "weaken the gate",
    );
    let at_base = |ran: &Ran| {
        assert!(
            ran.text
                .contains(&format!("gate.sh: read at the base {}", gov.base)),
            "{}",
            ran.text
        );
        assert!(!ran.text.contains("weakened gate"), "{}", ran.text);
        assert!(!ran.text.contains("weakened check"), "{}", ran.text);
    };

    // The governance step: the base's script finds the dash.
    let ran = run_gov(
        &gov,
        "Governance",
        &event("pull_request", &weakened),
        |_| {},
    );
    assert_eq!(ran.exit, 1, "{}", ran.text);
    at_base(&ran);
    assert!(
        ran.text
            .contains(&format!("{DECLARED} read at the base {}", gov.base)),
        "{}",
        ran.text
    );
    assert!(ran.text.contains("U+2014"), "{}", ran.text);

    // The title: the base's script judges it.
    let title = format!("a title {EM} with a dash");
    let ran = run_gov(
        &gov,
        "Authored-content rules (title and body)",
        &Event {
            title: &title,
            ..event("pull_request", &weakened)
        },
        |_| {},
    );
    assert_eq!(ran.exit, 1, "{}", ran.text);
    at_base(&ran);

    // The walk: the commit that weakens the gate is judged by the base's.
    let ran = run_gov(
        &gov,
        "Every commit in the change",
        &event("pull_request", &weakened),
        |_| {},
    );
    assert_eq!(ran.exit, 1, "{}", ran.text);
    at_base(&ran);
    assert!(
        ran.text.contains("never by the commit's own copy"),
        "{}",
        ran.text
    );
    assert!(
        ran.text.contains(&format!(
            "{} fails the gate or the format check at its own tree",
            gov.short(&weakened)
        )),
        "{}",
        ran.text
    );

    // The code job: the base's gate runs the format check, which fails.
    let ran = run_gov_job(
        &gov,
        "code",
        "Build, test, clippy, fmt",
        &event("pull_request", &weakened),
        |_| {},
    );
    assert_eq!(ran.exit, 1, "{}", ran.text);
    at_base(&ran);
    let cargo = ran.stub_file("cargo-calls").unwrap_or_default();
    assert!(cargo.contains("fmt --all --check"), "{cargo}");
}

/// Rule 1, the adoption: a base that carries no gate runs the candidate's
/// copies and says so; a base commit that cannot be read stops the job
/// rather than falling back to the candidate.
#[test]
fn the_adoption_runs_the_candidates_gate_and_says_so() {
    let mut gov = Gov::adoption(&[("governance.authored_content", serde_json::json!(DECLARED))]);
    let head = git(gov.root(), &["rev-parse", "HEAD"]);
    let ran = run_gov(&gov, "Governance", &event("pull_request", &head), |_| {});
    assert_eq!(ran.exit, 0, "{}", ran.text);
    for said in [
        "gate.sh: the base carries none; the candidate's copy runs (adoption)",
        "install-spec-spine.sh: the base carries none; the candidate's copy runs (adoption)",
        "the base carries no scripts/check-authored-content.sh; the candidate's copy runs (adoption)",
        "authored file(s) clean",
    ] {
        assert!(ran.text.contains(said), "{said}: {}", ran.text);
    }
    let ran = run_gov_job(
        &gov,
        "code",
        "Build, test, clippy, fmt",
        &event("pull_request", &head),
        |_| {},
    );
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(
        ran.text
            .contains("gate.sh: the base carries none; the candidate's copy runs (adoption)"),
        "{}",
        ran.text
    );
    // The exception job is not asked for: the base carries no policy, and
    // ci-gate reports the adoption (the_adoption_reads_the_candidate_and_says_so).
    let ran = run_gov(
        &gov,
        "Authority change",
        &event("pull_request", &head),
        |_| {},
    );
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert_eq!(ran.output("authority_change"), "false");
    assert!(
        ran.text.contains("the base carries no policy"),
        "{}",
        ran.text
    );

    // An unreadable base is never the adoption. Revision 7: refused (2), the
    // same code gate.sh gives the same condition.
    gov.base = "1111111111111111111111111111111111111111".to_string();
    let ran = run_gov(&gov, "Governance", &event("pull_request", &head), |_| {});
    assert_eq!(ran.exit, 2, "{}", ran.text);
    assert!(
        ran.text.contains("cannot read the base commit"),
        "{}",
        ran.text
    );
    assert!(!ran.text.contains("(adoption)"), "{}", ran.text);
}

/// Every file of the authority set, as a candidate changes it: the rendered
/// workflows, `scripts/statecraft/*` (a new file too), the policy and the
/// declared authored-content script.
const AUTHORITY_SET: [&str; 8] = [
    ".github/workflows/statecraft-ci.yml",
    // Revision 6: a workflow the profile does not render.
    ".github/workflows/impostor.yml",
    ".github/workflows/statecraft-ai-review.yml",
    "scripts/statecraft/gate.sh",
    "scripts/statecraft/install-spec-spine.sh",
    "scripts/statecraft/helper.sh",
    POLICY,
    DECLARED,
];

/// Every rendered file at the base, with the declared script; the candidate
/// changes `rel` (or `README.md` and a script outside the set, for `None`).
fn authority_repo(rel: Option<&str>) -> Repo {
    let mut repo = Repo::new_with(
        &[("governance.authored_content", serde_json::json!(DECLARED))],
        &[
            ".github/workflows/statecraft-ci.yml",
            ".github/workflows/statecraft-ai-review.yml",
            "scripts/statecraft",
            POLICY,
            DECLARED,
        ],
        &[],
    );
    match rel {
        Some(rel) if rel == POLICY => {
            let mut policy: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(repo.root().join(rel)).unwrap())
                    .unwrap();
            policy["jobs"]["review-exception"]["pull_request"] = serde_json::json!("inapplicable");
            repo.commit(rel, &serde_json::to_string_pretty(&policy).unwrap());
        }
        Some(rel) => {
            let before = std::fs::read_to_string(repo.root().join(rel)).unwrap_or_default();
            repo.commit(rel, &format!("{before}# a candidate's edit\n"));
        }
        None => {
            repo.commit("README.md", "# fixture, edited\n");
            repo.commit("scripts/other.sh", "#!/bin/sh\necho outside the set\n");
        }
    }
    repo
}

/// The workflow's own computation, in the governance job, of whether the
/// exception job must run.
fn authority_output(repo: &Repo) -> Ran {
    let wf = workflow(repo.root(), "statecraft-ci.yml");
    let (run, env) = step(&wf, "governance", "Authority change");
    let mut ctx = BTreeMap::new();
    ctx.insert(
        "github.event.pull_request.base.sha".to_string(),
        repo.base.clone(),
    );
    ctx.insert(
        "github.event.pull_request.head.sha".to_string(),
        repo.head.clone(),
    );
    run_step(repo.root(), &run, &env, &ctx, &[], |_| {})
}

/// Rule 2: a candidate that changes any file of the authority set blocks
/// `ci-gate` without the owner's exception for its run and passes with it; a
/// candidate workflow that skips the exception job fails closed, because the
/// gate recomputes the change at the base. In the merge queue the exception
/// recorded for the entry's pull request counts. On push it is reported.
#[test]
fn an_authority_change_blocks_without_the_owner_exception_and_passes_with_it() {
    for rel in AUTHORITY_SET {
        let repo = authority_repo(Some(rel));
        let ok = |exception: &str, review: &str| {
            needs(
                &as_refs(&with(("review-exception", exception))),
                Some(review),
                false,
            )
        };
        for (exception, review, want) in [
            ("skipped", "no-findings", 1),
            ("failure", "no-findings", 1),
            ("cancelled", "no-findings", 1),
            ("success", "no-findings", 0),
            ("success", "findings", 0),
            ("skipped", "findings", 1),
        ] {
            let ran = run_gate(&repo, "pull_request", &ok(exception, review), "topic");
            assert_eq!(ran.exit, want, "{rel}, {exception}, {review}: {}", ran.text);
            assert!(ran.text.contains("authority change"), "{rel}: {}", ran.text);
            assert!(ran.text.contains(rel), "{rel}: {}", ran.text);
            if want == 1 && review == "no-findings" {
                assert!(
                    ran.text
                        .contains("changes the authority set and the owner exception"),
                    "{rel}: {}",
                    ran.text
                );
            }
        }
        // The workflow asks for the exception job.
        let ran = authority_output(&repo);
        assert_eq!(ran.exit, 0, "{rel}: {}", ran.text);
        assert_eq!(
            ran.output("authority_change"),
            "true",
            "{rel}: {}",
            ran.text
        );

        // The queue: the exception recorded for the entry's pull request.
        let ran = run_queue(
            &repo,
            &Queue {
                exception: "skipped",
                ..queue("queue, authority change", 1)
            },
        );
        assert_eq!(ran.exit, 1, "{rel}: {}", ran.text);
        assert!(
            ran.text
                .contains("the queued group changes the authority set"),
            "{rel}: {}",
            ran.text
        );
        // A record that cannot be read never admits an authority change:
        // recorded_review blocks with its reason (the review of #143,
        // finding 1).
        let ran = run_queue(
            &repo,
            &Queue {
                pull_fails: true,
                exception: "success",
                ..queue("queue, authority change, record unreadable", 1)
            },
        );
        assert_eq!(ran.exit, 1, "{rel}: {}", ran.text);
        assert!(
            ran.text.contains("cannot read pull request #7"),
            "{rel}: {}",
            ran.text
        );
        let ran = run_queue(
            &repo,
            &Queue {
                exception: "success",
                ..queue("queue, authority change approved", 0)
            },
        );
        assert_eq!(ran.exit, 0, "{rel}: {}", ran.text);
        assert!(
            ran.text
                .contains("admitted by the owner exception recorded for #7"),
            "{rel}: {}",
            ran.text
        );

        // Push: the change was approved on its pull request; it is named.
        let ran = run_gate(
            &repo,
            "push",
            &needs(
                &[
                    ("governance", "success"),
                    ("code", "success"),
                    ("ai-review", "skipped"),
                    ("review-exception", "skipped"),
                ],
                None,
                false,
            ),
            "",
        );
        assert_eq!(ran.exit, 0, "{rel}: {}", ran.text);
        assert!(ran.text.contains("reported on push"), "{rel}: {}", ran.text);
    }
}

/// Rule 2's other side: a candidate that changes no file of the authority
/// set is judged as before, needs no exception, and the workflow does not
/// ask for one.
#[test]
fn a_candidate_that_changes_no_authority_file_is_unaffected() {
    let repo = authority_repo(None);
    let ran = run_gate(
        &repo,
        "pull_request",
        &needs(&ALL_OK, Some("no-findings"), false),
        "topic",
    );
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(!ran.text.contains("authority change"), "{}", ran.text);
    let ran = run_queue(&repo, &queue("queue, recorded no-findings", 0));
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert!(!ran.text.contains("authority change"), "{}", ran.text);
    let ran = authority_output(&repo);
    assert_eq!(ran.exit, 0, "{}", ran.text);
    assert_eq!(ran.output("authority_change"), "false", "{}", ran.text);
    assert!(ran.text.contains("no authority change"), "{}", ran.text);
}

/// Rules 1 and 2 in the rendered workflow: every job that runs the gate reads
/// it at the base first, with the whole history; no later step runs a
/// candidate's script; and the exception job runs for an authority change.
#[test]
fn revision_five_reads_the_gate_at_the_base_in_every_job_that_runs_it() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "spec-spine.toml", TOML);
    render(tmp.path());
    let wf = workflow(tmp.path(), "statecraft-ci.yml");
    for job in ["governance", "code"] {
        let steps = wf["jobs"][job]["steps"].as_sequence().unwrap();
        assert!(
            steps[0]["uses"]
                .as_str()
                .unwrap()
                .starts_with("actions/checkout@"),
            "{job}"
        );
        assert_eq!(steps[0]["with"]["fetch-depth"].as_u64(), Some(0), "{job}");
        assert_eq!(steps[1]["name"].as_str(), Some(READ_GATE), "{job}");
        assert!(steps[1]["if"].is_null(), "{job}");
        for s in &steps[2..] {
            if let Some(run) = s["run"].as_str() {
                for runs in ["sh scripts/statecraft/", "bash scripts/statecraft/"] {
                    assert!(
                        !run.contains(runs),
                        "{job}: a step runs the candidate's script: {run}"
                    );
                }
            }
        }
        let (_, env) = step(&wf, job, READ_GATE);
        assert!(
            env.iter().any(|(k, v)| k == "BASE_SHA"
                && v.contains("github.event.before")
                && v.contains("github.event.merge_group.base_sha")),
            "{job}: {env:?}"
        );
    }
    assert_eq!(
        wf["jobs"]["governance"]["outputs"]["authority_change"].as_str(),
        Some("${{ steps.authority.outputs.authority_change }}")
    );
    let exception = &wf["jobs"]["review-exception"];
    let needs: Vec<&str> = exception["needs"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(needs.contains(&"governance"), "{needs:?}");
    assert!(
        exception["if"]
            .as_str()
            .unwrap()
            .contains("needs.governance.outputs.authority_change == 'true'")
    );
    // The policy states the rule the gate enforces and the workflow reads.
    let policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.path().join(POLICY)).unwrap()).unwrap();
    assert_eq!(policy["revision"], setup::REVISION);
    assert_eq!(
        policy["authority_rule"]["exception_environment"],
        "statecraft-review-exception"
    );
}

// ------------------------------------------- the family exit contract (revision 7)

#[path = "support/exits.rs"]
mod exits;

/// The family exit contract: 0 ok, 1 finding, 2 refused, 3 usage, 4 failed.
const CONTRACT: [&str; 5] = ["0", "1", "2", "3", "4"];

/// Revision 7: every exit statement a rendered script or a rendered workflow
/// step states is in the family contract, and every implicit exit path is
/// closed. A deliberate non-zero exit is a literal code (or the one
/// translated variable whose every assignment is a literal); a command that
/// stops a script under `set -e` is reported as 4 by its trap; an unset
/// input is never the shell's own `${X:?}` code.
#[test]
fn every_exit_a_rendered_script_states_is_in_the_family_contract() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "spec-spine.toml", TOML);
    render(tmp.path());
    let mut bodies: Vec<(String, String)> = Vec::new();
    for name in [
        "gate.sh",
        "install-spec-spine.sh",
        "ci-gate.sh",
        "ai-review.sh",
    ] {
        let rel = format!("scripts/statecraft/{name}");
        let text = std::fs::read_to_string(tmp.path().join(&rel)).unwrap();
        // The implicit paths: each script carries the net that turns a
        // command that broke into 4, and no input check of the shell's own.
        if text.starts_with("#!/bin/sh") {
            assert!(text.contains("\nset -eu\n"), "{rel}");
            let trap = text
                .lines()
                .find(|l| l.starts_with("trap ") && l.ends_with(" EXIT"))
                .unwrap_or_else(|| panic!("{rel}: no EXIT trap"));
            assert!(trap.contains("exit 4"), "{rel}: {trap}");
        } else {
            assert!(text.contains("\nset -eEuo pipefail\n"), "{rel}");
            let trap = text
                .lines()
                .find(|l| l.starts_with("trap ") && l.ends_with(" ERR"))
                .unwrap_or_else(|| panic!("{rel}: no ERR trap"));
            assert!(trap.contains("exit 4"), "{rel}: {trap}");
        }
        assert!(
            !text.contains(":?"),
            "{rel}: an input checked by `${{X:?}}`"
        );
        bodies.push((rel, text));
    }
    for name in ["statecraft-ci.yml", "statecraft-ai-review.yml"] {
        let wf = workflow(tmp.path(), name);
        for (job, def) in wf["jobs"].as_mapping().unwrap() {
            for s in def["steps"].as_sequence().into_iter().flatten() {
                if let Some(run) = s["run"].as_str() {
                    bodies.push((format!("{name} {job:?}"), run.to_string()));
                }
            }
        }
    }
    let mut seen = 0;
    for (rel, text) in &bodies {
        let lines: Vec<&str> = text.lines().collect();
        for e in exits::exit_statements(text) {
            seen += 1;
            let at = format!("{rel}:{}: {}", e.line, lines[e.line - 1].trim());
            if CONTRACT.contains(&e.code.as_str()) {
                continue;
            }
            match (e.verb, e.code.as_str()) {
                // `leave`'s own body, which clears the trap and exits with
                // the literal its caller named.
                ("exit", "\"$1\"") => assert_eq!(lines[e.line - 2].trim(), "trap - EXIT", "{at}"),
                // The spec-spine translation table's one variable.
                ("leave", "\"$ss_to\"") => {
                    for l in lines.iter().filter(|l| l.contains("ss_to=")) {
                        let v = l.split("ss_to=").nth(1).unwrap();
                        let v = &v[..1];
                        assert!(CONTRACT.contains(&v), "{rel}: {l}");
                    }
                }
                _ => panic!("an exit outside the family contract: {at}"),
            }
        }
    }
    assert!(seen >= 40, "found only {seen} exit statements");
}

/// Stubs whose exit codes a case sets through `STUB_STATE`: `spec-spine`
/// answers `ss-<verb>` (`ss-index-<sub>` for `index`), and `cargo` answers
/// `cargo-<verb>`; either is 0 when the file is absent.
fn exit_bin() -> &'static Path {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap().keep();
        let spec_spine = "#!/bin/sh\nverb=\"$1\"\n[ \"$verb\" = index ] && verb=\"index-$2\"\nf=\"$STUB_STATE/ss-$verb\"\nif [ -f \"$f\" ]; then exit \"$(cat \"$f\")\"; fi\nexit 0\n";
        let cargo = "#!/bin/sh\nf=\"$STUB_STATE/cargo-$1\"\nif [ -f \"$f\" ]; then exit \"$(cat \"$f\")\"; fi\nif [ \"$1\" = metadata ]; then echo '{\"workspace_members\":[\"fixture\"]}'; fi\nexit 0\n";
        for (name, body) in [("spec-spine", spec_spine), ("cargo", cargo)] {
            statecraft_adapter::fixture::install_script(&dir.join(name), body, 0o755).unwrap();
        }
        // A PATH with no cargo: only what gate.sh needs before it asks.
        let bare = dir.join("bare");
        std::fs::create_dir_all(&bare).unwrap();
        for tool in ["dirname", "basename"] {
            let found = ["/usr/bin", "/bin"]
                .iter()
                .map(|d| Path::new(d).join(tool))
                .find(|p| p.exists())
                .unwrap();
            std::os::unix::fs::symlink(found, bare.join(tool)).unwrap();
        }
        dir
    })
}

/// Run the rendered `gate.sh` with `args`, as `make gate` does: `sh`, from
/// the checkout, with the case's stub codes in `state`.
fn gate_sh(
    root: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    state: &[(&str, &str)],
) -> (i32, String) {
    let stub_state = tempfile::tempdir().unwrap();
    for (name, code) in state {
        std::fs::write(stub_state.path().join(name), code).unwrap();
    }
    let path = format!(
        "{}:{}",
        exit_bin().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("scripts/statecraft/gate.sh")
        .args(args)
        .current_dir(root)
        .env_remove("BASE_SHA")
        .env_remove("HEAD_SHA")
        .env_remove("BASE_REF")
        .env("PATH", &path)
        .env("STUB_STATE", stub_state.path());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

/// Revision 7, the three known defects and the translations, observed by
/// running the rendered `gate.sh`: a usage error is 3 (was 64), a missing
/// spec-spine refuses with 2 (was 3), and an absent or non-executable
/// declared authored-content script refuses with 2 (was 1). spec-spine's
/// own codes and cargo's are translated, and a command that broke is 4.
#[test]
fn the_rendered_gate_exits_in_the_family_contract() {
    let declared = [("governance.authored_content", serde_json::json!(DECLARED))];
    let gov = Gov::new(&declared);
    let root = gov.root();
    let ss = root.join(".tooling/bin/spec-spine");
    std::fs::remove_file(&ss).unwrap();
    std::os::unix::fs::symlink(exit_bin().join("spec-spine"), &ss).unwrap();

    // Usage: no mode, an unknown mode, two modes, a mode missing its input.
    let usage: [&[&str]; 5] = [
        &[],
        &["nonsense"],
        &["governance", "code"],
        &["base"],
        &["couple"],
    ];
    for args in usage {
        let (code, text) = gate_sh(root, args, &[], &[]);
        assert_eq!(code, 3, "{args:?}: {text}");
    }

    // The translation table, spec-spine 0.25.0's codes.
    for (file, ss_code, want, said) in [
        ("ss-check", "1", 1, "does not pass"),
        ("ss-check", "2", 1, "stale"),
        ("ss-check", "3", 4, "did not perform the read"),
        ("ss-lint", "7", 4, "does not know"),
        ("ss-index-coverage", "1", 1, "does not pass"),
        ("ss-index-check", "2", 1, "stale"),
    ] {
        let (code, text) = gate_sh(root, &["governance"], &[], &[(file, ss_code)]);
        assert_eq!(code, want, "{file}={ss_code}: {text}");
        assert!(text.contains(said), "{file}={ss_code}: {text}");
        assert!(
            text.contains(&format!("spec-spine exit {ss_code}, gate exit {want}")),
            "{text}"
        );
    }
    // The same rendering under a 0.26.0 pin reads spec-spine's 132 table:
    // stale is already 1, a pin not met refuses with 2, a usage error from
    // the gate's own invocation is the gate failing, and 4 is failed.
    let toml = root.join("spec-spine.toml");
    let pinned_25 = std::fs::read_to_string(&toml).unwrap();
    let pinned_26 = pinned_25
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("required_version") {
                "required_version = \"=0.26.0\"".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(pinned_25, pinned_26, "the fixture states a pin");
    // Restores the 0.25.0 pin even when an assertion below panics.
    struct Restore<'a>(&'a Path, String);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            let _ = std::fs::write(self.0, &self.1);
        }
    }
    let restore = Restore(&toml, pinned_25);
    std::fs::write(&toml, pinned_26).unwrap();
    for (file, ss_code, want, said) in [
        ("ss-check", "1", 1, "or a stale committed tree"),
        ("ss-check", "2", 2, "a pin not met"),
        ("ss-check", "3", 4, "usage error"),
        ("ss-check", "4", 4, "could not do its work"),
        ("ss-lint", "7", 4, "does not know"),
        ("ss-index-check", "1", 1, "or a stale committed tree"),
    ] {
        let (code, text) = gate_sh(root, &["governance"], &[], &[(file, ss_code)]);
        assert_eq!(code, want, "0.26.0 {file}={ss_code}: {text}");
        assert!(text.contains(said), "0.26.0 {file}={ss_code}: {text}");
        assert!(
            text.contains(&format!("spec-spine exit {ss_code}, gate exit {want}")),
            "{text}"
        );
    }
    drop(restore);

    let head = git(root, &["rev-parse", "HEAD"]);
    let ends = [("BASE_SHA", gov.base.as_str()), ("HEAD_SHA", head.as_str())];
    let (code, text) = gate_sh(root, &["couple"], &ends, &[("ss-couple", "1")]);
    assert_eq!(code, 1, "drift is a finding: {text}");
    let (code, text) = gate_sh(root, &["couple"], &ends, &[]);
    assert_eq!(code, 0, "{text}");

    // cargo: a verb that ran and failed is a finding, a metadata read that
    // failed is a failure, and no cargo at all refuses.
    let (code, text) = gate_sh(root, &["code"], &[], &[("cargo-test", "101")]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("cargo test did not pass (cargo exit 101, gate exit 1)"),
        "{text}"
    );
    let (code, text) = gate_sh(root, &["code"], &[], &[("cargo-metadata", "101")]);
    assert_eq!(code, 4, "{text}");
    let (code, text) = gate_sh(root, &["code"], &[], &[]);
    assert_eq!(code, 0, "{text}");
    let bare = exit_bin().join("bare");
    let (code, text) = gate_sh(root, &["code"], &[("PATH", bare.to_str().unwrap())], &[]);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("cargo is not on PATH"), "{text}");

    // The declared authored-content script: a violation is a finding; absent
    // or not executable refuses.
    let (code, text) = gate_sh(root, &["governance"], &[], &[]);
    assert_eq!(code, 0, "{text}");
    write(root, "README.md", &format!("# fixture {EM} x\n"));
    let (code, text) = gate_sh(root, &["governance"], &[], &[]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("found a violation"), "{text}");
    write(root, "README.md", "# fixture\n");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(root.join(DECLARED), std::fs::Permissions::from_mode(0o644)).unwrap();
    let (code, text) = gate_sh(root, &["governance"], &[], &[]);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("which is not executable"), "{text}");
    std::fs::remove_file(root.join(DECLARED)).unwrap();
    let (code, text) = gate_sh(root, &["governance"], &[], &[]);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("which is absent"), "{text}");

    // A missing spec-spine refuses, in every mode that needs it.
    std::fs::remove_file(&ss).unwrap();
    for (mode, env) in [("governance", &[][..]), ("couple", &ends[..])] {
        let (code, text) = gate_sh(root, &[mode], env, &[]);
        assert_eq!(code, 2, "{mode}: {text}");
        assert!(text.contains("is not installed"), "{mode}: {text}");
    }

    // A command that broke is a failure, whatever its own code: the commit
    // walk over a base git cannot read.
    let walk = Gov::new(&[("governance.gate_each_commit", serde_json::json!(true))]);
    let head = git(walk.root(), &["rev-parse", "HEAD"]);
    let (code, text) = gate_sh(
        walk.root(),
        &["commits"],
        &[
            ("BASE_SHA", "1111111111111111111111111111111111111111"),
            ("HEAD_SHA", head.as_str()),
        ],
        &[],
    );
    assert_eq!(code, 4, "{text}");
    assert!(text.contains("reported as failed (4)"), "{text}");
}

/// Revision 7: the install script refuses an absent pin with 2 and reports a
/// failed install as 4, never as cargo's own code.
#[test]
fn the_rendered_installer_exits_in_the_family_contract() {
    let gov = Gov::new(&[]);
    let root = gov.root();
    std::fs::remove_file(root.join(".tooling/bin/spec-spine")).unwrap();
    let run = |state: &[(&str, &str)]| {
        let stub_state = tempfile::tempdir().unwrap();
        for (name, code) in state {
            std::fs::write(stub_state.path().join(name), code).unwrap();
        }
        let out = Command::new("sh")
            .arg("scripts/statecraft/install-spec-spine.sh")
            .current_dir(root)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    exit_bin().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("STUB_STATE", stub_state.path())
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        )
    };
    let (code, text) = run(&[("cargo-install", "101")]);
    assert_eq!(code, 4, "{text}");
    assert!(
        text.contains("cargo install of spec-spine 0.25.0 failed"),
        "{text}"
    );
    write(
        root,
        "spec-spine.toml",
        "[meta]\nrequired_version = \"0.25\"\n",
    );
    let (code, text) = run(&[]);
    assert_eq!(code, 2, "{text}");
    write(root, "spec-spine.toml", "[meta]\n");
    let (code, text) = run(&[]);
    assert_eq!(code, 2, "{text}");
}

// ------------------------------------- declared extra required jobs (revision 7)

const EXTRA: &str = "deny";

fn extra_params() -> Vec<(&'static str, serde_json::Value)> {
    vec![(
        "ci.extra_required_jobs",
        serde_json::json!([{"job": EXTRA, "workflow": ".github/workflows/deny.yml"}]),
    )]
}

/// The ci-gate cases for a declared extra job: each ends as the profile's
/// own required jobs do.
fn extra_cases() -> Vec<(&'static str, &'static str, String, i32)> {
    let mut all_ok: Vec<(&str, &str)> = ALL_OK.to_vec();
    all_ok.push((EXTRA, "success"));
    let over = |result: &'static str| -> Vec<(&'static str, &'static str)> {
        all_ok
            .iter()
            .map(|(j, r)| (*j, if *j == EXTRA { result } else { *r }))
            .collect()
    };
    let push = |entries: &[(&str, &str)]| -> String {
        let e: Vec<(&str, &str)> = entries
            .iter()
            .map(|(j, r)| match *j {
                "ai-review" | "review-exception" => (*j, "skipped"),
                _ => (*j, *r),
            })
            .collect();
        needs(&e, None, false)
    };
    let pr = |entries: &[(&str, &str)]| needs(entries, Some("no-findings"), false);
    let vanished: Vec<(&str, &str)> = ALL_OK.to_vec();
    vec![
        ("extra job succeeds", "pull_request", pr(&all_ok), 0),
        ("extra job succeeds on push", "push", push(&all_ok), 0),
        ("extra job failed", "pull_request", pr(&over("failure")), 1),
        (
            "extra job cancelled",
            "pull_request",
            pr(&over("cancelled")),
            1,
        ),
        ("extra job skipped", "pull_request", pr(&over("skipped")), 1),
        (
            "extra job skipped on push",
            "push",
            push(&over("skipped")),
            1,
        ),
        ("extra job vanished", "pull_request", pr(&vanished), 1),
    ]
}

fn extra_reason(label: &str) -> &'static str {
    if label.contains("skipped") {
        "'deny' was skipped where it applies"
    } else if label.contains("vanished") {
        "'deny' is not in the needs record"
    } else {
        "'deny' ended"
    }
}

fn run_extra_cases(repo: &Repo) -> Vec<(&'static str, i32, String)> {
    extra_cases()
        .into_iter()
        .map(|(label, event, needs_json, _)| {
            let head_ref = if event == "push" { "" } else { "topic" };
            let ran = run_gate(repo, event, &needs_json, head_ref);
            (label, ran.exit, ran.text)
        })
        .collect()
}

/// Revision 7: a job declared in `ci.extra_required_jobs` is rendered as a
/// call of the project's reusable workflow, ci-gate needs it, the policy
/// requires it on every event, and failed, cancelled, skipped and vanished
/// all block, exactly as for the profile's own jobs.
#[test]
fn a_declared_extra_job_is_required_like_the_profiles_own() {
    let repo = Repo::new_with(
        &extra_params(),
        &[POLICY, "scripts/statecraft/ci-gate.sh"],
        &[],
    );
    let wf = workflow(repo.root(), "statecraft-ci.yml");
    assert_eq!(
        wf["jobs"][EXTRA]["uses"].as_str(),
        Some("./.github/workflows/deny.yml")
    );
    assert!(
        wf["jobs"][EXTRA]["secrets"].is_null(),
        "no secret is passed"
    );
    let needs_list: Vec<&str> = wf["jobs"]["ci-gate"]["needs"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        needs_list,
        ["governance", "code", "ai-review", "review-exception", EXTRA]
    );
    let policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repo.root().join(POLICY)).unwrap()).unwrap();
    let mut required: Vec<&str> = policy["jobs"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, v)| v["required"] == true)
        .map(|(k, _)| k.as_str())
        .collect();
    required.sort_unstable();
    let mut sorted = needs_list.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, required);
    for event in ["pull_request", "push", "merge_group"] {
        assert_eq!(policy["jobs"][EXTRA][event], "required", "{event}");
    }
    for ((label, exit, text), (_, _, _, want)) in
        run_extra_cases(&repo).into_iter().zip(extra_cases())
    {
        assert_eq!(exit, want, "{label}: {text}");
        if want != 0 {
            assert!(text.contains(extra_reason(label)), "{label}: {text}");
        }
    }

    // Without a declaration, the rendered workflow is the profile's alone.
    let plain = tempfile::tempdir().unwrap();
    write(plain.path(), "spec-spine.toml", TOML);
    render(plain.path());
    let text =
        std::fs::read_to_string(plain.path().join(".github/workflows/statecraft-ci.yml")).unwrap();
    assert!(
        text.contains("needs: [governance, code, ai-review, review-exception]\n"),
        "{text}"
    );
}

/// Revision 7, the mutation obligation for declared jobs: each blocking
/// branch of ci-gate a required job reaches (failed or cancelled, skipped,
/// vanished), inverted, is noticed by the declared job's cases alone.
#[test]
fn inverting_a_required_job_branch_is_noticed_by_the_declared_job_cases() {
    let repo = Repo::new_with(&extra_params(), &[POLICY], &[]);
    let target = repo.root().join("scripts/statecraft/ci-gate.sh");
    let script = std::fs::read_to_string(&target).unwrap();
    let lines: Vec<&str> = script.lines().collect();
    let sites: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            t.starts_with("block \"required job '${job}'")
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(sites.len(), 3, "the three required-job branches");
    for site in sites {
        let mutated: String = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i == site {
                    format!("{}\n", l.replacen("block \"", ": \"", 1))
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        std::fs::write(&target, &mutated).unwrap();
        let noticed = run_extra_cases(&repo).into_iter().zip(extra_cases()).any(
            |((label, exit, text), (_, _, _, want))| {
                exit != want || (want != 0 && !text.contains(extra_reason(label)))
            },
        );
        assert!(
            noticed,
            "inverting line {} went unnoticed by the declared job's cases: {}",
            site + 1,
            lines[site]
        );
    }
}
