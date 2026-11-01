//! Spec 013 sections 3.2 to 3.5 and section 4, against a fake sidecar, and
//! spec 043's two deadlines and start observation (its sections 3 and 4,
//! measurement M-3) against a fake that blocks before reading its input.
//!
//! The fake is a shell script run as `/bin/sh <script> sidecar`, so the test
//! never executes a file it has just written (the Linux ETXTBSY race). It
//! echoes the binding the test minted, which is the protocol's whole rule.

use sha2::{Digest, Sha256};
use statecraft_adapter::witness::ledger::VerifiedProducer;
use statecraft_adapter::witness::posture::Observed;
use statecraft_adapter::witness::protocol::{
    AttemptBinding, Authority, Integrity, Limits, Start, Termination,
};
use statecraft_adapter::witness::sidecar::{Plan, StartObservation, start};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const RESERVED: &str = "127.0.0.1:41999";

struct Fixture {
    _dir: tempfile::TempDir,
    script: PathBuf,
    area: PathBuf,
}

fn binding() -> AttemptBinding {
    AttemptBinding::new("013-run", 1, "013-run/1/witness-sidecar").unwrap()
}

fn binding_json() -> String {
    serde_json::to_string(&serde_json::json!({
        "run_id": "013-run", "attempt": 1, "effect_id": "013-run/1/witness-sidecar"
    }))
    .unwrap()
}

/// `body` runs after the start line is consumed and the area exists;
/// `$OUT` is the area, `$B` the binding JSON.
fn fixture(body: &str) -> Fixture {
    fixture_blocking("", body)
}

/// A producer whose code runs but reads nothing before `before` has run:
/// spec 043's M-3 fixture. The start line is read only after it.
fn fixture_blocking(before: &str, body: &str) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let area = dir.path().join("witness");
    let script = dir.path().join("fake-sidecar.sh");
    let text = format!(
        "OUT='{out}'\nB='{b}'\nP='\"protocol\":\"wire-witness.sidecar/1\"'\n{before}\nread -r start\nmkdir -m 700 \"$OUT\"\nprintf 'cert' > \"$OUT/ca.pem\"\n{body}\n",
        out = area.display(),
        b = binding_json(),
    );
    std::fs::write(&script, text).unwrap();
    Fixture {
        _dir: dir,
        script,
        area,
    }
}

const READY: &str = r#"printf '{%s,"type":"ready","binding":%s,"endpoint":"127.0.0.1:41999","certificate_path":"%s/ca.pem"}\n' "$P" "$B" "$OUT""#;

fn manifest_digest() -> String {
    let hex: String = Sha256::digest(b"manifest")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("sha256:{hex}")
}

fn finished() -> String {
    format!(
        r#"printf 'manifest' > "$OUT/wire-exchange.json"
printf '{{%s,"type":"finished","binding":%s,"completeness":"complete","capture_digest":"{d}","manifest_path":"%s/wire-exchange.json","exchange_count":1,"finding_count":0,"gaps":[]}}\n' "$P" "$B" "$OUT""#,
        d = manifest_digest()
    )
}

fn plan(fixture: &Fixture) -> Plan {
    Plan {
        producer: VerifiedProducer {
            name: "wire-witness".into(),
            version: "0.1.0".into(),
            interface: "wire-witness.sidecar/1".into(),
            target: "test-target".into(),
            executable_digest: "sha256:00".into(),
            qualification: "qualification/test.md".into(),
            path: fixture.script.clone(),
        },
        start: Start {
            binding: binding(),
            capture_authorities: vec![Authority {
                authority: "api.example.test:443".into(),
                provider: "unknown".into(),
                protocols: vec!["http/1.1".into()],
            }],
            authentication_tunnels: Vec::new(),
            requested_retention: "metadata-only".into(),
            limits: Limits::default(),
            output_directory: fixture.area.clone(),
            certificate_path: fixture.area.join("ca.pem"),
            host_policy_id: "statecraft/013-test".into(),
            listen_endpoint: RESERVED.parse().unwrap(),
        },
        wrapper: vec!["/bin/sh".into()],
        environment: BTreeMap::from([("PATH".into(), "/bin:/usr/bin".into())]),
        start_timeout: Duration::from_secs(60),
        ready_timeout: Duration::from_secs(20),
        grace: Duration::from_secs(5),
    }
}

/// `plan` with spec 043's two deadlines set for one test.
fn deadlines(fixture: &Fixture, start_timeout: u64, ready_timeout: u64) -> Plan {
    let mut p = plan(fixture);
    p.start_timeout = Duration::from_secs(start_timeout);
    p.ready_timeout = Duration::from_secs(ready_timeout);
    p
}

#[test]
fn a_complete_session_is_ready_then_finished_with_an_intact_manifest() {
    let f = fixture(&format!("{READY}\ncat >/dev/null\n{}", finished()));
    let running = start(&plan(&f)).expect("ready");
    assert_eq!(
        running.ready().endpoint,
        RESERVED.parse::<SocketAddr>().unwrap()
    );
    assert_eq!(running.ready().certificate_path, f.area.join("ca.pem"));
    let ended = running.close();
    assert_eq!(ended.termination, Termination::Finished);
    assert_eq!(ended.integrity, Integrity::Intact);
    assert!(
        ended.reader.findings().is_empty(),
        "{:?}",
        ended.reader.findings()
    );
    assert_eq!(
        ended.observed(true),
        Observed::Complete {
            intact: true,
            qualified: true
        }
    );
}

#[test]
fn a_sidecar_that_exits_before_ready_failed_and_the_child_never_spawns() {
    let f = fixture("exit 3");
    let ended = *start(&plan(&f)).unwrap_err().unwrap_err();
    assert_eq!(
        ended.refusal.as_deref(),
        Some("sidecar-exited-before-ready")
    );
    assert_eq!(ended.observed(true), Observed::Failed);
    assert!(ended.reader.terminal().is_none());
}

#[test]
fn a_ready_naming_another_endpoint_refuses_spawn_and_stops_the_sidecar() {
    let f = fixture(
        r#"printf '{%s,"type":"ready","binding":%s,"endpoint":"0.0.0.0:41999","certificate_path":"%s/ca.pem"}\n' "$P" "$B" "$OUT"
sleep 30"#,
    );
    let began = Instant::now();
    let ended = *start(&plan(&f)).unwrap_err().unwrap_err();
    assert_eq!(ended.refusal.as_deref(), Some("endpoint-wildcard"));
    assert_eq!(ended.termination, Termination::Killed);
    assert!(began.elapsed() < Duration::from_secs(20));
}

#[test]
fn a_changed_binding_never_reaches_ready() {
    let f = fixture(
        r#"printf '{%s,"type":"ready","binding":{"run_id":"013-run","attempt":1,"effect_id":"other"},"endpoint":"127.0.0.1:41999","certificate_path":"%s/ca.pem"}\n' "$P" "$OUT""#,
    );
    let ended = *start(&plan(&f)).unwrap_err().unwrap_err();
    assert_eq!(
        ended.refusal.as_deref(),
        Some("sidecar-exited-before-ready")
    );
    assert_eq!(ended.reader.findings().len(), 1);
}

#[test]
fn eof_after_ready_is_stopped_never_a_successful_finish() {
    let f = fixture(&format!("{READY}\ncat >/dev/null\nexit 0"));
    let ended = start(&plan(&f)).expect("ready").close();
    assert_eq!(ended.termination, Termination::Stopped);
    assert!(ended.reader.terminal().is_none());
    assert_eq!(ended.observed(true), Observed::Failed);
}

#[test]
fn a_sidecar_that_outlives_its_grace_is_killed() {
    let f = fixture(&format!("{READY}\nexec sleep 60"));
    let mut p = plan(&f);
    p.grace = Duration::from_millis(300);
    let began = Instant::now();
    let ended = start(&p).expect("ready").close();
    assert_eq!(ended.termination, Termination::Killed);
    assert!(began.elapsed() < Duration::from_secs(20));
}

#[test]
fn key_material_after_ready_is_reported_for_the_caller_to_stop_everything() {
    let f = fixture(&format!(
        "{READY}\nprintf '{{%s,\"type\":\"finding\",\"binding\":%s,\"code\":\"-----BEGIN PRIVATE KEY-----\"}}\\n' \"$P\" \"$B\"\nexec sleep 60"
    ));
    let mut running = start(&plan(&f)).expect("ready");
    let deadline = Instant::now() + Duration::from_secs(20);
    let finding = loop {
        if let Some(finding) = running.poll() {
            break finding;
        }
        assert!(Instant::now() < deadline, "no custody violation seen");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(finding.is_custody_violation());
    let ended = running.abort("custody-violation");
    assert_eq!(ended.termination, Termination::Killed);
    assert_eq!(ended.observed(true), Observed::Failed);
}

#[test]
fn an_incomplete_terminal_is_incomplete_and_a_wrong_digest_is_not_intact() {
    let f = fixture(&format!(
        "{READY}\ncat >/dev/null\nprintf 'tampered' > \"$OUT/wire-exchange.json\"\nprintf '{{%s,\"type\":\"finished\",\"binding\":%s,\"completeness\":\"complete\",\"capture_digest\":\"{}\",\"manifest_path\":\"%s/wire-exchange.json\",\"exchange_count\":1,\"finding_count\":0}}\\n' \"$P\" \"$B\" \"$OUT\"",
        manifest_digest()
    ));
    let ended = start(&plan(&f)).expect("ready").close();
    assert_eq!(ended.integrity, Integrity::Mismatch);
    assert_eq!(
        ended.observed(true),
        Observed::Complete {
            intact: false,
            qualified: true
        }
    );

    let g = fixture(&format!(
        "{READY}\ncat >/dev/null\nprintf '{{%s,\"type\":\"finished\",\"binding\":%s,\"completeness\":\"incomplete\",\"reason\":\"deadline-reached\",\"exchange_count\":0,\"finding_count\":0,\"gaps\":[\"deadline-reached\"]}}\\n' \"$P\" \"$B\""
    ));
    let ended = start(&plan(&g)).expect("ready").close();
    assert_eq!(ended.termination, Termination::Finished);
    assert_eq!(ended.observed(true), Observed::Incomplete);
    assert_eq!(ended.reader.terminal().unwrap().gaps, ["deadline-reached"]);
}

// Spec 043 (M-3, R-3, section 4 row 4): the start observation is not
// satisfied before the producer's code reads its input. The shell is running
// throughout the two seconds, so a wrapper or loader activity alone, or the
// kernel having created the process, would have been observed at once.
#[test]
fn a_producer_blocked_before_reading_its_input_is_not_observed_started() {
    let f = fixture_blocking(
        "sleep 2",
        &format!("{READY}\ncat >/dev/null\n{}", finished()),
    );
    let running = start(&deadlines(&f, 60, 20)).expect("ready");
    let timing = running.timing();
    assert_eq!(timing.observation, StartObservation::StartLineConsumed);
    let started_after = timing.started_after.expect("start observed");
    assert!(started_after >= Duration::from_secs(2), "{started_after:?}");
    let ready_after = timing.ready_after.expect("ready observed");
    assert!(ready_after < started_after, "{ready_after:?}");
    assert_eq!(running.close().termination, Termination::Finished);
}

// Spec 043 (R-1, section 4 row 1): blocked past the ready deadline but inside
// the start deadline still reaches ready, because the ready deadline runs
// from the start observation, not from spawn.
#[test]
fn a_producer_blocked_past_the_ready_deadline_still_reaches_ready() {
    let f = fixture_blocking(
        "sleep 2",
        &format!("{READY}\ncat >/dev/null\n{}", finished()),
    );
    let began = Instant::now();
    let running = start(&deadlines(&f, 20, 1)).expect("ready");
    assert!(began.elapsed() >= Duration::from_secs(2));
    assert_eq!(
        running.ready().endpoint,
        RESERVED.parse::<SocketAddr>().unwrap()
    );
    let ended = running.close();
    assert_eq!(ended.termination, Termination::Finished);
    assert_eq!(
        ended.observed(true),
        Observed::Complete {
            intact: true,
            qualified: true
        }
    );
}

// Spec 043 (R-2, section 4 row 2): blocked past the start deadline is
// `start-timeout`, `witness-failed`, and the child never spawns.
#[test]
fn a_producer_blocked_past_the_start_deadline_is_start_timeout() {
    let f = fixture_blocking("sleep 30", READY);
    let began = Instant::now();
    let ended = *start(&deadlines(&f, 1, 1)).unwrap_err().unwrap_err();
    assert_eq!(ended.refusal.as_deref(), Some("start-timeout"));
    assert_eq!(ended.termination, Termination::Killed);
    assert_eq!(ended.observed(true), Observed::Failed);
    assert_eq!(ended.timing.started_after, None);
    assert!(ended.reader.terminal().is_none());
    assert!(began.elapsed() < Duration::from_secs(20));
}

// Spec 043 (R-2, section 4 row 3): started but silent is `ready-timeout`
// after the ready deadline from the start observation.
#[test]
fn a_started_producer_that_never_reports_ready_is_ready_timeout() {
    let f = fixture("sleep 30");
    let began = Instant::now();
    let ended = *start(&deadlines(&f, 20, 1)).unwrap_err().unwrap_err();
    assert_eq!(ended.refusal.as_deref(), Some("ready-timeout"));
    assert_eq!(ended.termination, Termination::Killed);
    assert_eq!(ended.observed(true), Observed::Failed);
    assert_eq!(
        ended.timing.observation,
        StartObservation::StartLineConsumed
    );
    assert!(ended.timing.started_after.is_some());
    assert_eq!(ended.timing.ready_after, None);
    assert!(began.elapsed() >= Duration::from_secs(1));
    assert!(began.elapsed() < Duration::from_secs(20));
}

// Spec 043 (R-2, section 3.2): an exit before ready in the start phase keeps
// 013's word, as the ready-phase exit above does.
#[test]
fn an_exit_before_reading_the_start_line_is_exited_before_ready() {
    let f = fixture_blocking("exit 3", READY);
    let ended = *start(&deadlines(&f, 60, 20)).unwrap_err().unwrap_err();
    assert_eq!(
        ended.refusal.as_deref(),
        Some("sidecar-exited-before-ready")
    );
    assert_eq!(ended.timing.started_after, None);
    assert_eq!(ended.observed(true), Observed::Failed);
}

// Spec 043 (R-3, section 4 row 5): a producer message claiming a start time
// is ignored for timing. The claim arrives before the producer reads its
// input and the ready deadline would have run out had the claim started it;
// the message itself is a protocol finding, as 013 already requires.
#[test]
fn a_producer_claim_about_its_start_is_ignored_for_timing() {
    let claim = r#"printf '{%s,"type":"started","binding":%s,"started_at":"1970-01-01T00:00:00Z"}\n' "$P" "$B""#;
    let f = fixture_blocking(
        &format!("{claim}\nsleep 2"),
        &format!("{READY}\ncat >/dev/null\n{}", finished()),
    );
    let running = start(&deadlines(&f, 20, 1)).expect("ready");
    let started_after = running.timing().started_after.expect("start observed");
    assert!(started_after >= Duration::from_secs(2), "{started_after:?}");
    let ended = running.close();
    assert_eq!(ended.termination, Termination::Finished);
    assert_eq!(ended.reader.findings().len(), 1);
}

#[test]
fn no_process_at_all_is_a_spawn_failure_not_a_failed_sidecar() {
    let f = fixture("");
    let mut p = plan(&f);
    p.wrapper = vec![Path::new("/nonexistent/statecraft-wrapper").into()];
    assert!(start(&p).unwrap_err().is_ok());
}
