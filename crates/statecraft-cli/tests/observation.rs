//! Spec 027 sections 3.1 to 3.5 and 3.11, through the binary: `observe` and
//! `observe show` against a real Git repository and a fake `spec-spine` that
//! answers from files committed in the observed tree, so what is observed is
//! the commit and never the working tree.

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

#[path = "support/json_naming.rs"]
mod json_naming;
#[path = "support/provider_token.rs"]
mod provider_token;

/// A provider that reads its prompt and replays a recorded successful
/// stream, so an admitted proposal's attempt runs to an outcome.
const PROVIDER: &str = r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267'; exit 0; fi
/bin/cat > /dev/null
/bin/cat "$(dirname "$0")/native.jsonl"
"#;

/// Answers from `.fake/` in the tree it runs in: `plan.json`, `list.json`,
/// `closure-<spec>` (a digest) and `stale` (present means a stale ledger).
const SPEC_SPINE: &str = r#"#!/bin/sh
f="$PWD/.fake"
case "$*" in
  --version) echo 'spec-spine 0.22.0' ;;
  'check --help') exit 0 ;;
  check) if [ -f "$f/stale" ]; then echo 'spec-registry: STALE'; echo 'spec-registry: STALE' >&2; exit 2; fi ;;
  'registry plan --json') /bin/cat "$f/plan.json" ;;
  'registry list --json') /bin/cat "$f/list.json" ;;
  'verify '*' --plan --json') printf '{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"%s"},"schemaVersion":"0.6.0","verb":"verify"}' $2 ;;
  'registry closure --help') exit 0 ;;
  'registry closure --request - --json')
    req="$(/bin/cat)"
    for s in $(/bin/ls "$f" | /usr/bin/sed -n 's/^closure-//p'); do
      case "$req" in *"\"$s\""*)
        printf '{"digest":"%s","members":[{"kind":"spec","spec":"%s","contentHash":"h"}],"schemaVersion":"0.6.0"}' "$(/bin/cat "$f/closure-$s")" "$s"
        exit 0 ;;
      esac
    done
    echo 'spec-spine: no closure' >&2; exit 1 ;;
  *) exit 3 ;;
esac
"#;

struct Fixture {
    target: tempfile::TempDir,
    home: tempfile::TempDir,
    bin: tempfile::TempDir,
    operator_home: tempfile::TempDir,
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

impl Fixture {
    fn new(register: bool) -> Self {
        let f = Self {
            target: tempfile::tempdir().unwrap(),
            home: tempfile::tempdir().unwrap(),
            bin: tempfile::tempdir().unwrap(),
            operator_home: tempfile::tempdir().unwrap(),
        };
        std::fs::write(f.operator_home.path().join(".claude.json"), "{}").unwrap();
        let t = f.target.path();
        git(t, &["init", "--quiet", "-b", "main"]);
        git(t, &["config", "user.name", "fixture"]);
        git(t, &["config", "user.email", "fixture@example.com"]);
        // As every governed target does: run state is never committed.
        std::fs::write(t.join(".gitignore"), ".statecraft/state/\n").unwrap();
        let program = f.bin.path().join("spec-spine");
        std::fs::write(&program, SPEC_SPINE).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let provider = f.bin.path().join("claude");
        std::fs::write(&provider, PROVIDER).unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl"),
            f.bin.path().join("native.jsonl"),
        )
        .unwrap();
        f.commit(
            &[("001-a", "approved", Some("c1")), ("002-b", "draft", None)],
            false,
        );
        if register {
            assert!(
                f.cli(&["project", "register", &f.root()])
                    .status
                    .code()
                    .unwrap()
                    <= 1
            );
        }
        f
    }

    fn root(&self) -> String {
        self.target.path().display().to_string()
    }

    /// Commit a corpus: each spec's status and, when eligible, its closure.
    fn commit(&self, specs: &[(&str, &str, Option<&str>)], stale: bool) -> String {
        let t = self.target.path();
        let fake = t.join(".fake");
        let _ = std::fs::remove_dir_all(&fake);
        std::fs::create_dir_all(&fake).unwrap();
        let ready: Vec<Value> = specs
            .iter()
            .map(|(id, status, _)| serde_json::json!({"id": id, "status": status, "title": id}))
            .collect();
        let items: Vec<Value> = specs
            .iter()
            .map(|(id, status, _)| {
                serde_json::json!({"id": id, "status": status, "implementation": "pending"})
            })
            .collect();
        std::fs::write(
            fake.join("plan.json"),
            serde_json::json!({ "ready": ready }).to_string(),
        )
        .unwrap();
        std::fs::write(
            fake.join("list.json"),
            serde_json::json!({ "items": items }).to_string(),
        )
        .unwrap();
        for (id, _, closure) in specs {
            if let Some(d) = closure {
                std::fs::write(fake.join(format!("closure-{id}")), d).unwrap();
            }
        }
        if stale {
            std::fs::write(fake.join("stale"), "").unwrap();
        }
        git(t, &["add", "-A"]);
        git(t, &["commit", "--quiet", "--allow-empty", "-m", "corpus"]);
        git(t, &["rev-parse", "HEAD"])
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env(
                "STATECRAFT_HOME",
                provider_token::with_token(self.home.path()),
            )
            .env("HOME", self.operator_home.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.bin.path().display()),
            )
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    fn observe(&self, extra: &[&str]) -> (i32, Value) {
        let root = self.root();
        let mut args = vec!["observe", root.as_str()];
        args.extend_from_slice(extra);
        args.push("--json");
        let out = self.cli(&args);
        (
            out.status.code().unwrap(),
            json_naming::from_output(&out.stdout).unwrap(),
        )
    }

    fn chain(&self) -> Vec<Value> {
        let dir = self.home.path().join("observations");
        let Some(file) = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "jsonl"))
        else {
            return Vec::new();
        };
        std::fs::read_to_string(file)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap()["payload"].clone())
            .collect()
    }
}

fn entries(chain: &[Value]) -> Vec<String> {
    chain
        .iter()
        .map(|e| e["entry"].as_str().unwrap().to_string())
        .collect()
}

fn kinds(answer: &Value) -> Vec<(String, String)> {
    answer["report"]["transitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["spec"].as_str().unwrap_or("").to_string(),
                t["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn the_first_observation_proposes_the_backlog_as_initial_and_an_unchanged_one_writes_nothing() {
    let f = Fixture::new(true);
    let (code, first) = f.observe(&[]);
    assert_eq!(code, 0, "{first}");
    assert_eq!(first["report"]["result"], "answered");
    assert_eq!(first["report"]["ref"], "main");
    assert_eq!(
        kinds(&first),
        [("001-a".to_string(), "entered".to_string())]
    );
    assert_eq!(first["report"]["transitions"][0]["initial"], true);
    assert_eq!(first["report"]["proposals"][0]["entry"], "proposed");
    assert_eq!(first["report"]["proposals"][0]["kind"], "run");
    assert_eq!(entries(&f.chain()), ["observed", "proposed"]);

    let (code, again) = f.observe(&[]);
    assert_eq!(code, 0);
    assert_eq!(again["report"]["result"], "unchanged");
    assert_eq!(entries(&f.chain()).len(), 2);
}

#[test]
fn uncommitted_edits_are_not_observed() {
    let f = Fixture::new(true);
    f.observe(&[]);
    // The working tree now says 002-b is approved; the commit does not.
    let fake = f.target.path().join(".fake");
    std::fs::write(
        fake.join("plan.json"),
        r#"{"ready":[{"id":"002-b","status":"approved","title":"b"}]}"#,
    )
    .unwrap();
    let (code, again) = f.observe(&[]);
    assert_eq!(code, 0);
    assert_eq!(again["report"]["result"], "unchanged");
}

#[test]
fn a_refusal_is_recorded_as_a_notice_and_spanned_by_the_next_answer() {
    let f = Fixture::new(true);
    f.observe(&[]);
    f.commit(&[("001-a", "approved", Some("c1"))], true);
    let (code, refused) = f.observe(&[]);
    assert_eq!(code, 1, "{refused}");
    assert_eq!(refused["report"]["result"], "refused");
    assert_eq!(refused["report"]["refusal"]["class"], "ledger-stale");
    assert_eq!(
        kinds(&refused),
        [(String::new(), "corpus-changed".to_string())]
    );
    assert_eq!(refused["report"]["proposals"][0]["kind"], "notice");

    // Fresh again, same eligibility: corpus-state changes back, and 001-a
    // neither exits nor re-enters.
    f.commit(&[("001-a", "approved", Some("c1"))], false);
    let (code, back) = f.observe(&[]);
    assert_eq!(code, 0);
    assert_eq!(
        kinds(&back),
        [(String::new(), "corpus-changed".to_string())]
    );
    assert!(back["report"]["proposals"].as_array().unwrap().is_empty());
}

#[test]
fn the_same_fact_from_a_second_commit_recurs_and_a_moved_contract_supersedes() {
    let f = Fixture::new(true);
    f.observe(&[]);
    // A commit that moves nothing the projections see: the input identity
    // differs, the transitions are empty, nothing is proposed.
    std::fs::write(f.target.path().join("readme"), "x").unwrap();
    git(f.target.path(), &["add", "-A"]);
    git(f.target.path(), &["commit", "--quiet", "-m", "docs"]);
    let (_, docs) = f.observe(&[]);
    assert_eq!(docs["report"]["result"], "answered");
    assert!(kinds(&docs).is_empty());

    // 001-a leaves and re-enters under the same contract: one proposal,
    // `recurred`.
    f.commit(&[("001-a", "draft", None)], false);
    let (_, exited) = f.observe(&[]);
    assert_eq!(
        kinds(&exited),
        [("001-a".to_string(), "exited".to_string())]
    );
    f.commit(&[("001-a", "approved", Some("c1"))], false);
    let (_, back) = f.observe(&[]);
    assert_eq!(back["report"]["proposals"][0]["entry"], "recurred");

    // The contract moves: a new run proposal supersedes the open one, and a
    // moved-contract notice is made.
    f.commit(&[("001-a", "approved", Some("c2"))], false);
    let (_, moved) = f.observe(&[]);
    assert_eq!(
        kinds(&moved),
        [("001-a".to_string(), "contract-moved".to_string())]
    );
    let made: Vec<_> = moved["report"]["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["kind"].as_str().unwrap(), p["entry"].as_str().unwrap()))
        .collect();
    assert_eq!(made, [("run", "proposed"), ("notice", "proposed")]);
    assert_eq!(moved["report"]["superseded"].as_array().unwrap().len(), 1);
}

#[test]
fn observe_show_reads_without_writing_and_names_an_unobserved_commit() {
    let f = Fixture::new(true);
    let root = f.root();
    let out = f.cli(&["observe", "show", &root, "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let none: Value = json_naming::from_output(&out.stdout).unwrap();
    assert!(none["report"]["lastAnswered"].is_null());
    assert_eq!(none["report"]["unobserved"], true);
    assert!(!f.home.path().join("observations").exists());

    f.observe(&[]);
    f.commit(&[("001-a", "approved", Some("c1"))], false);
    let out = f.cli(&["observe", "show", &root, "--json"]);
    let shown: Value = json_naming::from_output(&out.stdout).unwrap();
    assert_eq!(shown["report"]["unobserved"], true);
    assert!(shown["report"]["lastAnswered"]["observation"].is_string());
}

#[test]
fn an_unresolved_ref_an_unregistered_target_and_a_held_lock_are_refused() {
    let f = Fixture::new(true);
    let (code, bad) = f.observe(&["--ref", "no-such-branch"]);
    assert_eq!(code, 2, "{bad}");
    assert!(f.chain().is_empty());

    let held = statecraft_observe::store::lock(f.home.path(), f.target.path()).unwrap();
    let (code, busy) = f.observe(&[]);
    assert_eq!(code, 2, "{busy}");
    assert_eq!(busy["error"]["details"]["reason"], "observation-lock-held");
    drop(held);

    let g = Fixture::new(false);
    let (code, _) = g.observe(&[]);
    assert_eq!(code, 2);

    // A named ref is recorded as named.
    let first = git(f.target.path(), &["rev-parse", "HEAD"]);
    let (code, pinned) = f.observe(&["--ref", &first]);
    assert_eq!(code, 0);
    assert_eq!(pinned["report"]["ref"], first.as_str());
}

#[test]
fn a_crash_after_the_observed_entry_is_completed_by_the_next_unchanged_observation() {
    let f = Fixture::new(true);
    f.observe(&[]);
    // The process died after `observed` and before its `proposed`.
    let dir = f.home.path().join("observations");
    let file = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    let first = text.lines().next().unwrap();
    std::fs::write(&file, format!("{first}\n")).unwrap();
    assert_eq!(entries(&f.chain()), ["observed"]);

    let (code, again) = f.observe(&[]);
    assert_eq!(code, 0, "{again}");
    assert_eq!(again["report"]["result"], "unchanged");
    assert_eq!(again["report"]["proposals"][0]["entry"], "proposed");
    assert_eq!(entries(&f.chain()), ["observed", "proposed"]);
    // And only once.
    let (_, third) = f.observe(&[]);
    assert!(third["report"]["proposals"].as_array().unwrap().is_empty());
}

fn verb(f: &Fixture, args: &[&str]) -> (i32, Value) {
    let mut all = args.to_vec();
    all.push("--json");
    let out = f.cli(&all);
    (
        out.status.code().unwrap(),
        json_naming::from_output(&out.stdout).unwrap(),
    )
}

#[test]
fn the_proposal_verbs_list_trace_and_close_each_kind_by_its_own_verb() {
    let f = Fixture::new(true);
    let root = f.root();
    let (_, first) = f.observe(&[]);
    let run_id = first["report"]["proposals"][0]["proposal"]
        .as_str()
        .unwrap()
        .to_string();
    f.commit(&[("001-a", "draft", None)], false);
    let (_, exited) = f.observe(&[]);
    let notice_id = exited["report"]["proposals"][0]["proposal"]
        .as_str()
        .unwrap()
        .to_string();

    let (code, listed) = verb(&f, &["proposal", "list", &root]);
    assert_eq!(code, 0);
    let rows: Vec<_> = listed["report"]["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(rows, ["run", "notice"]);

    // The trace: the run's one cause is initial; the notice's cites
    // spec-spine's delta, which this producer does not answer, so it is
    // unavailable with spec-spine's reason rather than derived.
    let (code, shown) = verb(&f, &["proposal", "show", &root, &run_id]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["report"]["causes"][0]["delta"]["state"], "initial");
    let (_, notice) = verb(&f, &["proposal", "show", &root, &notice_id]);
    assert_eq!(
        notice["report"]["causes"][0]["delta"]["state"],
        "unavailable"
    );
    assert_eq!(
        notice["report"]["causes"][0]["transitions"][0]["kind"],
        "exited"
    );

    // Each kind is closed only by its own verb, and only once.
    let (code, wrong) = verb(&f, &["proposal", "acknowledge", &root, &run_id, "sam"]);
    assert_eq!(code, 2, "{wrong}");
    assert_eq!(wrong["error"]["details"]["reason"], "wrong-kind");
    let (code, _) = verb(
        &f,
        &["proposal", "dismiss", &root, &run_id, "sam", "not", "now"],
    );
    assert_eq!(code, 0);
    let (code, again) = verb(&f, &["proposal", "dismiss", &root, &run_id, "sam", "again"]);
    assert_eq!(code, 2);
    assert_eq!(again["error"]["details"]["reason"], "not-open");
    let (code, _) = verb(&f, &["proposal", "acknowledge", &root, &notice_id, "sam"]);
    assert_eq!(code, 0);
    let (_, empty) = verb(&f, &["proposal", "list", &root]);
    assert!(empty["report"]["proposals"].as_array().unwrap().is_empty());

    // A dismissal stands against the same work under the same contract.
    f.commit(&[("001-a", "approved", Some("c1"))], false);
    let (_, back) = f.observe(&[]);
    assert_eq!(back["report"]["proposals"][0]["entry"], "recurred");
    let (_, shown) = verb(&f, &["proposal", "show", &root, &run_id]);
    assert_eq!(shown["report"]["state"], "dismissed");

    let (code, unknown) = verb(&f, &["proposal", "show", &root, "nope"]);
    assert_eq!(code, 2, "{unknown}");
}

fn first_run_proposal(f: &Fixture) -> String {
    let (_, first) = f.observe(&[]);
    first["report"]["proposals"][0]["proposal"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn admission_is_stale_when_the_repository_moved_and_never_runs() {
    let f = Fixture::new(true);
    let root = f.root();
    let id = first_run_proposal(&f);
    // The contract moved on the observed ref since the proposal was made.
    f.commit(&[("001-a", "approved", Some("c2"))], false);
    let (code, stale) = verb(&f, &["proposal", "admit", &root, &id, "sam", "go"]);
    assert_eq!(code, 1, "{stale}");
    assert!(
        stale["report"]["differing"]
            .as_str()
            .unwrap()
            .contains("c1")
    );
    let (_, shown) = verb(&f, &["proposal", "show", &root, &id]);
    assert_eq!(shown["report"]["state"], "stale");
    assert!(
        !f.home.path().join("records").exists() || {
            let out = f.cli(&["run", "show", &root, "001-a", "--json"]);
            out.status.code() != Some(0)
        }
    );

    // A notice is never admitted.
    let g = Fixture::new(true);
    let root = g.root();
    g.observe(&[]);
    g.commit(&[("001-a", "draft", None)], false);
    let (_, exited) = g.observe(&[]);
    let notice = exited["report"]["proposals"][0]["proposal"]
        .as_str()
        .unwrap();
    let (code, wrong) = verb(&g, &["proposal", "admit", &root, notice, "sam", "go"]);
    assert_eq!(code, 2);
    assert_eq!(wrong["error"]["details"]["reason"], "wrong-kind");
}

#[test]
fn a_spec_003_refusal_releases_the_admission_and_the_proposal_is_open_again() {
    // Registered but not armed: spec 003's run path refuses before any
    // attempt, so the admission is released, not disposed.
    let f = Fixture::new(true);
    let root = f.root();
    let id = first_run_proposal(&f);
    let (code, refused) = verb(&f, &["proposal", "admit", &root, &id, "sam", "go"]);
    assert_eq!(code, 2, "{refused}");
    let tail: Vec<_> = entries(&f.chain()).into_iter().rev().take(2).collect();
    assert_eq!(tail, ["released", "admitting"]);
    let (_, listed) = verb(&f, &["proposal", "list", &root]);
    assert_eq!(listed["report"]["proposals"][0]["state"], "open");
}

#[test]
fn an_admitted_proposal_enters_the_run_path_and_names_itself_in_the_intent() {
    let f = Fixture::new(true);
    let root = f.root();
    assert_eq!(f.cli(&["project", "arm", &root]).status.code(), Some(0));
    let id = first_run_proposal(&f);
    let (code, ran) = verb(&f, &["proposal", "admit", &root, &id, "sam", "go"]);
    assert!(code <= 1, "{ran}");
    assert_eq!(ran["verb"], "proposal.admit");
    let (_, shown) = verb(&f, &["proposal", "show", &root, &id]);
    assert_eq!(shown["report"]["state"], "admitted", "{shown}");
    assert_eq!(shown["report"]["run"]["runId"], "001-a");
    assert_eq!(shown["report"]["run"]["attempt"], 1);
    // Admitted once: a second admission is refused; a retry is `run`.
    let (code, again) = verb(&f, &["proposal", "admit", &root, &id, "sam", "again"]);
    assert_eq!(code, 2);
    assert_eq!(again["error"]["details"]["reason"], "not-open");

    // The contract moves: the moved-contract notice names the attempt bound
    // to the earlier digest.
    f.commit(&[("001-a", "approved", Some("c2"))], false);
    let (code, moved) = f.observe(&[]);
    assert_eq!(code, 0, "{moved}");
    let notice = f
        .chain()
        .into_iter()
        .find(|e| e["subscription"] == "moved-contract")
        .unwrap();
    assert_eq!(
        notice["detail"]["boundAttempts"],
        serde_json::json!([{ "runId": "001-a", "attempt": 1, "live": false }])
    );
}
