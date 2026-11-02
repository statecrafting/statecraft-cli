//! Spec 055 sections 3.3 and 3.4: the per-run `CODEX_HOME` leaves the user's
//! own Codex directory byte-identical (obligation I-2), and a gate hook Codex
//! does not report as managed, synchronous, enabled and unrestricted refuses
//! before the turn (obligation R-2). The `hooks/list` answers are synthetic;
//! see `testdata/hooks/README.md`.

use statecraft_adapter::capability::Requested;
use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, construct};
use statecraft_adapter::protocol::{AttemptIdentity, Request};
use statecraft_adapter_codex::home::{CREDENTIAL_FILE, HOOKS_FILE, Refused, user_directory};
use statecraft_adapter_codex::hooks::{HookRefusal, judge, query};
use statecraft_adapter_codex::{CODEX_HOME, Invocation, MEASURED_PROVIDER_VERSION, RunHome};
use statecraft_run::attempt::Outcome;
use statecraft_secret::SecretString;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const GATE: &str = "/statecraft/home/harness/hooks/gate";

fn answer(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/hooks")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn request(workspace: &Path) -> Request {
    Request {
        context: None,
        documentation: None,
        workspace: workspace.to_path_buf(),
        base_commit: "0".repeat(40),
        prompt: b"prompt".to_vec(),
        capabilities: Requested::none(),
        deadline_seconds: 10,
        attempt: AttemptIdentity {
            run_id: "codex-configuration".into(),
            number: 1,
        },
    }
}

fn snapshot(dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            snapshot(&path, into);
        } else {
            into.insert(path.clone(), std::fs::read(&path).unwrap());
        }
    }
}

fn credential() -> SecretString {
    SecretString::from(r#"{"synthetic":"credential"}"#)
}

#[test]
fn a_launch_with_no_credential_refuses_and_creates_nothing() {
    let scratch = tempfile::tempdir().unwrap();
    let refused = RunHome::create(
        scratch.path(),
        Path::new("/nowhere/.codex"),
        &serde_json::json!({}),
        None,
    );
    assert_eq!(refused.unwrap_err().code(), "codex-credential-absent");
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}

#[test]
fn a_scratch_root_inside_the_users_codex_directory_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let user = user_directory(home.path());
    std::fs::create_dir_all(user.join("scratch")).unwrap();
    let refused = RunHome::create(
        &user.join("scratch"),
        &user,
        &serde_json::json!({}),
        Some(&credential()),
    );
    assert!(matches!(refused, Err(Refused::InsideUserDirectory { .. })));
    assert!(!user.join("scratch").join("codex-home").exists());
}

#[test]
fn a_run_leaves_the_users_codex_directory_byte_identical() {
    let home = tempfile::tempdir().unwrap();
    let user = user_directory(home.path());
    std::fs::create_dir_all(&user).unwrap();
    for (name, bytes) in [
        ("config.toml", "model = \"x\"\n"),
        ("hooks.json", "{}"),
        ("auth.json", "{\"user\":1}"),
    ] {
        std::fs::write(user.join(name), bytes).unwrap();
    }
    let mut before = BTreeMap::new();
    snapshot(&user, &mut before);

    let scratch = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let hooks = serde_json::json!({"hooks": {"PreToolUse": [{"command": GATE}]}});
    let run_home = RunHome::create(scratch.path(), &user, &hooks, Some(&credential())).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(run_home.path()), 0o700);
        assert_eq!(mode(&run_home.path().join(CREDENTIAL_FILE)), 0o600);
    }
    let blueprint =
        run_home.naming_in(Blueprint::empty().allowing("HOME", &home.path().to_string_lossy()));
    let environment = construct(&blueprint, &CheckSuiteCommands::default());
    std::fs::write(
        workspace.path().join("stream.jsonl"),
        "{\"type\":\"thread.started\",\"thread_id\":\"t\"}\n{\"type\":\"turn.completed\"}\n",
    )
    .unwrap();
    let invocation = Invocation {
        program: "/bin/sh".into(),
        args: vec![
            "-c".into(),
            "/bin/cat > /dev/null; /bin/ls \"$CODEX_HOME\" > seen.txt; /bin/cat stream.jsonl"
                .into(),
        ],
    };
    let execution = statecraft_adapter_codex::supervise(
        &invocation,
        &request(workspace.path()),
        &environment,
        MEASURED_PROVIDER_VERSION,
    )
    .unwrap();
    assert_eq!(execution.supervised.outcome, Outcome::Completed);
    let seen = std::fs::read_to_string(workspace.path().join("seen.txt")).unwrap();
    assert!(seen.contains(HOOKS_FILE) && seen.contains(CREDENTIAL_FILE));
    assert!(!environment.variables[CODEX_HOME].starts_with(&*user.to_string_lossy()));

    let per_run = run_home.path().to_path_buf();
    drop(run_home);
    assert!(!per_run.exists(), "the per-run directory is discarded");
    let mut after = BTreeMap::new();
    snapshot(&user, &mut after);
    assert_eq!(before, after);
}

#[test]
fn only_a_managed_synchronous_enabled_unrestricted_gate_is_admitted() {
    assert!(judge(&answer("managed.json"), GATE).is_ok());
    assert_eq!(
        judge(&answer("untrusted.json"), GATE),
        Err(HookRefusal::NotManaged {
            trust: "untrusted".into()
        })
    );
    let with = |field: &str, value: serde_json::Value| {
        let mut a = answer("managed.json");
        a["result"]["hooks"][0][field] = value;
        judge(&a, GATE).unwrap_err().to_string()
    };
    assert!(with("enabled", false.into()).starts_with("codex-gate-hook-disabled"));
    assert!(with("async", true.into()).starts_with("codex-gate-hook-asynchronous"));
    assert!(with("matcher", "Bash".into()).starts_with("codex-gate-hook-matcher-restricted"));
    assert!(with("eventName", "PostToolUse".into()).starts_with("codex-gate-hook-absent"));
    assert!(with("command", "/elsewhere".into()).starts_with("codex-gate-hook-absent"));
    assert!(with("trustStatus", serde_json::Value::Null).starts_with("codex-hook-list-unreadable"));
    let failed = judge(
        &serde_json::json!({"id": 1, "error": {"code": -32601}}),
        GATE,
    );
    assert!(matches!(failed, Err(HookRefusal::QueryFailed { .. })));
}

#[test]
fn the_query_refuses_an_environment_that_is_not_the_attempts_before_spawning() {
    let scratch = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let run_home = RunHome::create(
        scratch.path(),
        Path::new("/nowhere/.codex"),
        &serde_json::json!({}),
        Some(&credential()),
    )
    .unwrap();
    // An inherited CODEX_HOME, not the attempt's. A program that does not
    // exist proves nothing was spawned: a spawn would fail differently.
    let environment = construct(
        &Blueprint::empty().allowing(CODEX_HOME, "/operator/.codex"),
        &CheckSuiteCommands::default(),
    );
    let refused = query(
        Path::new("/nonexistent/codex"),
        &run_home,
        &environment,
        &request(workspace.path()),
        GATE,
    );
    assert!(matches!(refused, Err(HookRefusal::QueryEnvironment { .. })));
}

#[test]
fn the_query_runs_app_server_in_the_attempts_environment_and_sends_no_turn() {
    let scratch = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let run_home = RunHome::create(
        scratch.path(),
        Path::new("/nowhere/.codex"),
        &serde_json::json!({}),
        Some(&credential()),
    )
    .unwrap();
    let environment = construct(
        &run_home.naming_in(Blueprint::empty()),
        &CheckSuiteCommands::default(),
    );
    // `/bin/sh app-server` reads the script `app-server` from the working
    // directory, so no freshly written file is executed.
    std::fs::write(
        workspace.path().join("app-server"),
        "/bin/cat > query.txt\nprintf '%s\\n' \"$CODEX_HOME\" > home.txt\n/bin/cat answer.json\n",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("answer.json"),
        answer("managed.json").to_string(),
    )
    .unwrap();
    let admitted = query(
        Path::new("/bin/sh"),
        &run_home,
        &environment,
        &request(workspace.path()),
        GATE,
    );
    assert!(admitted.is_ok(), "{admitted:?}");
    let sent = std::fs::read_to_string(workspace.path().join("query.txt")).unwrap();
    let methods: Vec<String> = sent
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["method"].to_string())
        .collect();
    assert_eq!(
        methods,
        ["\"initialize\"", "\"initialized\"", "\"hooks/list\""]
    );
    let home = std::fs::read_to_string(workspace.path().join("home.txt")).unwrap();
    assert_eq!(home.trim(), run_home.path().to_string_lossy());
}
