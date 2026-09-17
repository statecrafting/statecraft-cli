//! Spec 004 section 3.5, the negative suite, plus section 3.8's rows.
//!
//! The suite runs against the fixture adapter, which ships with it so the table
//! can be run with no real provider installed. The fixture is a real child
//! process: the stream, the deadline and the kill are not properties a function
//! call could exercise.

use statecraft_adapter::capability::{Capability, Requested, negotiate};
use statecraft_adapter::environment::{
    Blueprint, CheckSuiteCommands, EnvironmentState, RESIDUALS, construct,
};
use statecraft_adapter::fixture::{self, Behavior};
use statecraft_adapter::manifest::{
    Qualification, QualificationRecord, discrepancy, qualification,
};
use statecraft_adapter::posture::Posture;
use statecraft_adapter::protocol::{
    AttemptIdentity, Classification, Event, ReportedCost, Request, StreamError, read_stream,
    refusals,
};
use statecraft_adapter::supervisor::{SpawnRefusal, preflight, supervise};
use statecraft_run::attempt::Outcome;
use statecraft_run::refusal::{Accounting, decide};
use std::path::Path;

fn env_for(commands: &[&str]) -> statecraft_adapter::ChildEnvironment {
    let mut b = Blueprint::empty().allowing("PATH", "/usr/bin:/bin");
    for c in commands {
        b = b.needing_command(c);
    }
    construct(
        &b,
        &CheckSuiteCommands(commands.iter().map(|c| c.to_string()).collect()),
    )
}

fn request(workspace: &Path, deadline: u64, capabilities: Requested) -> Request {
    Request {
        workspace: workspace.to_path_buf(),
        base_commit: "0".repeat(40),
        prompt: b"do the work".to_vec(),
        capabilities,
        deadline_seconds: deadline,
        attempt: AttemptIdentity {
            run_id: "run-1".into(),
            number: 1,
        },
    }
}

// Suite row 1: a refusal is retained beside a completed result.
#[test]
fn suite_1_a_refusal_is_retained_beside_a_completed_result() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = fixture::write(dir.path(), Behavior::RefusalThenCompleted).unwrap();
    let env = env_for(&["sh"]);

    let run = supervise(
        &adapter,
        &[],
        &request(dir.path(), 30, Requested::none()),
        &env,
    )
    .unwrap();

    let observed = refusals(&run.events);
    assert_eq!(observed.len(), 1, "the refusal is retained");
    assert!(
        run.events.iter().any(|e| matches!(
            e,
            Event::Result {
                classification: Classification::Completed,
                ..
            }
        )),
        "beside a completed result"
    );

    // And the supervisor's own accounting is what decides the attempt.
    let mut accounting = Accounting::default();
    for r in observed {
        accounting.observe(r);
    }
    assert_eq!(decide(Outcome::Completed, &accounting), Outcome::Refused);
}

// Suite row 2: a required capability the manifest lacks is refused before spawn.
#[test]
fn suite_2_a_required_capability_the_manifest_lacks_is_refused_with_no_process_created() {
    let env = env_for(&[]);
    let result = preflight(
        &fixture::manifest(),
        &Requested::none().requiring(Capability::HookEnforcement),
        &env,
    );
    match result {
        Err(SpawnRefusal::MissingRequired {
            adapter,
            version,
            token,
        }) => {
            assert_eq!(adapter, "fixture");
            assert_eq!(version, "1.0.0");
            assert_eq!(token, "hook-enforcement");
        }
        other => panic!("expected a pre-spawn refusal, got {other:?}"),
    }
}

#[test]
fn a_preferred_capability_the_manifest_lacks_runs_and_records_the_degradation() {
    let env = env_for(&[]);
    let requested = Requested::none().preferring(Capability::CostReport);
    let n = preflight(&fixture::manifest(), &requested, &env).expect("preferred does not refuse");
    assert_eq!(n.degraded, [Capability::CostReport]);
}

// Suite row 3: a hung child is killed at the deadline with its descendants.
#[test]
fn suite_3_a_hung_child_is_killed_at_the_deadline_and_the_attempt_is_interrupted() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = fixture::write(dir.path(), Behavior::HangsForever).unwrap();
    let env = env_for(&["sh"]);

    let started = std::time::Instant::now();
    let run = supervise(
        &adapter,
        &[],
        &request(dir.path(), 1, Requested::none()),
        &env,
    )
    .unwrap();
    let elapsed = started.elapsed();

    assert_eq!(
        run.outcome,
        Outcome::Interrupted,
        "not failed: nothing was judged"
    );
    assert!(
        !Outcome::Interrupted.was_judged(),
        "interrupted is the outcome where nothing was judged"
    );

    // The fixture also backgrounds a second `sleep 300`, so this covers the row
    // about a child that spawns a survivor. The supervisor must return at its
    // own deadline whether or not the group kill reached that survivor: being
    // held past the deadline by the thing being supervised is the failure this
    // bound exists to catch, and it is the one CI caught at 300 seconds, because
    // the reader thread was joined while a survivor still held the pipe open.
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "the deadline was enforced, not waited out: {elapsed:?}"
    );

    // If a descendant did outlive the kill it is reported as a residual, and
    // never as a clean termination.
    if let Some(residual) = &run.surviving_processes {
        assert!(
            residual.contains("outlived") || residual.contains("could not"),
            "a survivor is described, not merely flagged: {residual}"
        );
    }
}

// Suite row 4: a malformed event stream is reported as malformed.
#[test]
fn suite_4_a_malformed_stream_is_reported_and_never_read_as_success() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = fixture::write(dir.path(), Behavior::MalformedStream).unwrap();
    let env = env_for(&["sh"]);

    let run = supervise(
        &adapter,
        &[],
        &request(dir.path(), 30, Requested::none()),
        &env,
    )
    .unwrap();

    match run.stream_error {
        Some(StreamError::Malformed { line, .. }) => assert!(line >= 1),
        other => panic!("expected a malformed report, got {other:?}"),
    }
    assert_ne!(run.outcome, Outcome::Completed, "never read as success");
}

// Suite row 5: an absent cost is reported as unknown, never zero.
#[test]
fn suite_5_an_absent_cost_is_unknown_and_never_zero() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = fixture::write(dir.path(), Behavior::CompletedWithNoCost).unwrap();
    let env = env_for(&["sh"]);

    let run = supervise(
        &adapter,
        &[],
        &request(dir.path(), 30, Requested::none()),
        &env,
    )
    .unwrap();
    let result = read_stream(&run.events, &[]).unwrap();

    assert_eq!(result.cost, ReportedCost::Unknown("unknown".into()));
    assert!(!result.cost.is_known());
    assert_eq!(serde_json::to_string(&result.cost).unwrap(), "\"unknown\"");
}

// Suite row 6: a manifest declaring a token the adapter does not honor fails
// qualification.
#[test]
fn suite_6_declaring_a_token_the_init_event_does_not_apply_fails_qualification() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = fixture::write(dir.path(), Behavior::AppliesLessThanDeclared).unwrap();
    let env = env_for(&["sh"]);
    let requested = Requested::none().requiring(Capability::TurnLimit);
    let n = preflight(&fixture::manifest(), &requested, &env).unwrap();

    let run = supervise(
        &adapter,
        &[],
        &request(dir.path(), 30, requested.clone()),
        &env,
    )
    .unwrap();
    let result = read_stream(&run.events, &n.degraded).unwrap();

    let d = discrepancy(&n.granted, &result.applied);
    assert!(d.any(), "the discrepancy is detected");
    assert_eq!(d.declared_not_applied, [Capability::TurnLimit]);
    assert!(
        d.describe(&fixture::manifest())
            .contains("fails qualification")
    );
    assert!(
        result.applied.is_empty(),
        "the applied set is recorded as observed, not as declared"
    );
}

// Suite row 7: records through the seam are identical whichever implementation
// produced them, for the same request and the same provider behavior.
#[test]
fn suite_7_two_implementations_with_the_same_behavior_produce_identical_records() {
    let dir = tempfile::tempdir().unwrap();
    let env = env_for(&["sh"]);

    // Two distinct adapter binaries, written to different paths, behaving the
    // same. The seam's output must not be able to tell them apart.
    let first_dir = dir.path().join("first");
    let second_dir = dir.path().join("second");
    std::fs::create_dir_all(&first_dir).unwrap();
    std::fs::create_dir_all(&second_dir).unwrap();
    let a = fixture::write(&first_dir, Behavior::CompletedWithNoCost).unwrap();
    let b = fixture::write(&second_dir, Behavior::CompletedWithNoCost).unwrap();
    assert_ne!(a, b, "two different binaries");

    let ra = read_stream(
        &supervise(&a, &[], &request(dir.path(), 30, Requested::none()), &env)
            .unwrap()
            .events,
        &[],
    )
    .unwrap();
    let rb = read_stream(
        &supervise(&b, &[], &request(dir.path(), 30, Requested::none()), &env)
            .unwrap()
            .events,
        &[],
    )
    .unwrap();

    assert_eq!(
        serde_json::to_string(&ra).unwrap(),
        serde_json::to_string(&rb).unwrap()
    );
}

// Suite row 8: the posture declares every command the run will need.
#[test]
fn suite_8_a_posture_omitting_a_command_the_check_suite_invokes_is_refused_at_plan_time() {
    // The observed instance design note 06 records: a profile allowing git, the
    // GitHub CLI, Bun and spec-spine, against a gate that also needs cargo and
    // make.
    let blueprint = Blueprint::empty()
        .needing_command("git")
        .needing_command("gh")
        .needing_command("bun")
        .needing_command("spec-spine");
    let suite = CheckSuiteCommands(
        ["git", "gh", "bun", "spec-spine", "cargo", "make"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );

    let env = construct(&blueprint, &suite);
    match &env.state {
        EnvironmentState::Refused { reasons } => {
            assert!(reasons.iter().any(|r| r.contains("`cargo`")));
            assert!(reasons.iter().any(|r| r.contains("`make`")));
            assert!(reasons.iter().any(|r| r.contains("plan time")));
        }
        other => panic!("expected a plan-time refusal, got {other:?}"),
    }

    // And nothing spawns.
    assert!(matches!(
        preflight(&fixture::manifest(), &Requested::none(), &env),
        Err(SpawnRefusal::Environment { .. })
    ));
}

// Section 3.8: an adapter binary with no qualification record runs, labelled.
#[test]
fn an_unqualified_binary_runs_and_is_labelled_in_posture_attempt_and_outcome() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = fixture::write(dir.path(), Behavior::CompletedWithNoCost).unwrap();
    let env = env_for(&["sh"]);
    let manifest = fixture::manifest();
    let q = qualification(&manifest, &[]);
    assert_eq!(q, Qualification::Unqualified);

    let requested = Requested::none();
    let n = preflight(&manifest, &requested, &env).expect("not refused for being unqualified");
    let run = supervise(
        &adapter,
        &[],
        &request(dir.path(), 30, requested.clone()),
        &env,
    )
    .unwrap();
    let result = read_stream(&run.events, &n.degraded).unwrap();

    let posture = Posture::new(&manifest, q, &requested, &n, &result.applied, &env);
    assert!(posture.render().contains("unqualified"));
    assert_eq!(run.outcome, Outcome::Completed, "it runs");
}

// Section 3.8: a qualification record for a different version is not honored.
#[test]
fn a_record_for_another_version_of_the_same_adapter_leaves_the_binary_unqualified() {
    let manifest = fixture::manifest();
    let stale = QualificationRecord {
        adapter: "fixture".into(),
        binary_version: "0.9.0".into(),
        suite_version: "1".into(),
        date: "2026-09-16T00:00:00Z".into(),
    };
    assert_eq!(
        qualification(&manifest, &[stale]),
        Qualification::Unqualified
    );
}

// Section 3.8: an event stream that ends without a result event.
#[test]
fn a_stream_that_ends_with_no_result_event_is_interrupted_not_completed() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = fixture::write(dir.path(), Behavior::NoResultEvent).unwrap();
    let env = env_for(&["sh"]);

    let run = supervise(
        &adapter,
        &[],
        &request(dir.path(), 30, Requested::none()),
        &env,
    )
    .unwrap();

    assert_eq!(run.outcome, Outcome::Interrupted);
    assert!(matches!(
        run.stream_error,
        Some(StreamError::NoResult { .. })
    ));
}

// Section 3.8: an adapter requesting a credential the environment omits fails
// visibly, and the supervisor does not widen the environment.
#[test]
fn a_withheld_credential_is_absent_from_the_child_and_the_environment_is_not_widened() {
    let env = construct(
        &Blueprint::empty()
            .allowing("PATH", "/usr/bin")
            .allowing("GITHUB_TOKEN", "would-let-the-child-publish"),
        &CheckSuiteCommands::default(),
    );
    assert!(!env.variables.contains_key("GITHUB_TOKEN"));
    match &env.state {
        EnvironmentState::Degraded { reasons } => {
            assert!(reasons[0].contains("never placed in the child"));
        }
        other => panic!("expected the withholding to be recorded, got {other:?}"),
    }
}

// Section 3.6 and 3.8: the residuals are named, and no claim of containment is
// made anywhere.
#[test]
fn every_residual_is_named_on_the_attempt_and_nothing_claims_the_child_cannot_publish() {
    let env = env_for(&[]);
    let manifest = fixture::manifest();
    let requested = Requested::none();
    let n = negotiate(&requested, &manifest.supports);
    let posture = Posture::new(
        &manifest,
        Qualification::Qualified,
        &requested,
        &n,
        &[],
        &env,
    );

    assert_eq!(posture.residuals.len(), RESIDUALS.len());
    assert_eq!(posture.residuals.len(), 4);
    let rendered = posture.render();
    assert!(rendered.contains("keychain"));
    assert!(rendered.contains("leaves no record"));
    assert!(
        !rendered.contains("cannot publish"),
        "no document may claim the child cannot publish"
    );
}

// Section 3.1: the prompt is delivered on a stream, never on a command line.
#[test]
fn the_prompt_reaches_the_child_on_a_stream_and_no_argument_carries_it() {
    let dir = tempfile::tempdir().unwrap();
    // A fixture that records whatever it reads from stdin, so "it arrived on the
    // stream" is observed rather than assumed.
    let adapter = dir.path().join("echoing-adapter.sh");
    let seen = dir.path().join("stdin-seen.txt");
    std::fs::write(
        &adapter,
        format!(
            "#!/bin/sh\ncat > {}\n\
             echo '{{\"event\":\"init\",\"applied\":[],\"adapterVersion\":\"1\",\"providerVersion\":\"f\"}}'\n\
             echo '{{\"event\":\"result\",\"classification\":\"completed\",\"cost\":null}}'\n",
            seen.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&adapter, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let env = env_for(&["sh"]);
    let mut req = request(dir.path(), 30, Requested::none());
    req.prompt = b"a prompt with 'quotes' and $(a subshell)".to_vec();

    // No arguments at all: there is nowhere for a prompt to be interpolated.
    let run = supervise(&adapter, &[], &req, &env).unwrap();
    assert_eq!(run.outcome, Outcome::Completed);
    assert_eq!(std::fs::read(&seen).unwrap(), req.prompt);
}
