//! Spec 043 section 5, M-1 and M-2: the released producer's two start
//! intervals (spawn to the start observation, the observation to `ready`),
//! read from the sidecar's own `Timing`, never from the wall clock.
//!
//! The run path's route is followed: the producer is placed at its one home
//! path and verified by digest (`statecraft_home::witness::locate`), the
//! sidecar's confinement is admitted and self-tested by `admit_sidecar`, and
//! the producer is started with no wrapper under `start_confined`, with the
//! environment and deadlines `crates/statecraft-cli/src/witness.rs` gives it.
//!
//! The harness is ignored, so `make code` never runs it. Run it with
//! `cargo test -p statecraft-cli --test witness_start_measurement --
//! --ignored --nocapture`. It reads:
//!
//! - `STATECRAFT_M_PRODUCER`: the released wire-witness for this target
//!   (required; the harness skips without it);
//! - `STATECRAFT_M_ITER`: iterations per case, default 20;
//! - `STATECRAFT_M_CASES`: `fresh`, `warm` or both, comma-separated, default
//!   both;
//! - `STATECRAFT_M_LOAD`: busy-loop processes kept running during the runs,
//!   default 0;
//! - `STATECRAFT_M_ASSESS`: processes that keep copying the producer to a new
//!   path and executing it, the assessment load of M-1, default 0.
//!
//! One JSON line per case is printed, prefixed `M043 `.

#![cfg(unix)]

use statecraft_adapter::witness::effect;
use statecraft_adapter::witness::ledger::current_target;
use statecraft_adapter::witness::protocol::{Limits, Start};
use statecraft_adapter::witness::sidecar::{Plan, StartObservation, Timing, start_confined};
use statecraft_home::witness::{SidecarAdmission, SidecarRoots, admit_sidecar};
use std::collections::BTreeMap;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// The run path's values (`crates/statecraft-cli/src/witness.rs`), which are
/// private to the binary.
// The run path's own deadlines, so a measurement never runs stale copies.
use statecraft_cli::witness::{GRACE, READY_TIMEOUT, START_TIMEOUT};

fn probe_executable() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_statecraft-cli"))
}

fn env_number(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(v) => v
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("{name}={v} is not a number")),
        Err(_) => default,
    }
}

/// A home laid out as the run path lays it out, with the producer at its one
/// path, verified against the adopted ledger.
struct Home {
    _dir: tempfile::TempDir,
    home: PathBuf,
    records: PathBuf,
    target: PathBuf,
    workspace: PathBuf,
    producer: statecraft_adapter::witness::ledger::VerifiedProducer,
    attempts: u32,
}

impl Home {
    /// A new home holding a new copy of `released`: a file the operating
    /// system has never assessed.
    fn place(released: &Path) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        let home = root.join("home");
        let records = home.join("records/k.startup");
        let target = root.join("target");
        let workspace = root.join("workspace");
        for directory in [&records, &target, &workspace] {
            fs::create_dir_all(directory).unwrap();
        }
        let triple = current_target().expect("a ledger target for this platform");
        let path = statecraft_home::witness::producer_path(&home, triple);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(released, &path).unwrap();
        let producer = statecraft_home::witness::locate(&home).unwrap_or_else(|refusal| {
            panic!("STATECRAFT_M_PRODUCER is not the adopted release for {triple}: {refusal}")
        });
        Self {
            _dir: dir,
            home,
            records,
            target,
            workspace,
            producer,
            attempts: 0,
        }
    }

    /// One attempt: admit the sidecar's confinement in a new area, start the
    /// producer confined, and close it after `ready`.
    fn attempt(&mut self) -> Outcome {
        self.attempts += 1;
        let attempt = self.attempts;
        let exchange = self.home.join(format!("exchange/k/runs/m043/{attempt}"));
        fs::create_dir_all(&exchange).unwrap();
        let area = exchange.join(statecraft_home::witness::AREA);
        let endpoint = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let admission = match admit_sidecar(
            &SidecarRoots {
                home: &self.home,
                records: &self.records,
                target: &self.target,
                workspace: &self.workspace,
                area: &area,
                producer: &self.producer,
            },
            probe_executable(),
            endpoint,
            statecraft_adapter_claude_code::witness::OUTBOUND_PORTS,
        ) {
            Ok(admission) => admission,
            Err(why) => return Outcome::Refused(format!("sidecar-confinement: {why}")),
        };
        let binding = effect::mint("m043", attempt).expect("an effect identity");
        let plan = Plan {
            start: Start {
                binding,
                capture_authorities: statecraft_adapter_claude_code::witness::capture_authorities(),
                authentication_tunnels: Vec::new(),
                requested_retention: "metadata-only".into(),
                limits: Limits::default(),
                output_directory: admission.output.clone(),
                certificate_path: admission.output.join("ca.pem"),
                host_policy_id: format!("statecraft/{}", admission.boundary.record().digest),
                listen_endpoint: endpoint,
            },
            producer: self.producer.clone(),
            wrapper: Vec::new(),
            environment: sidecar_environment(&admission),
            start_timeout: START_TIMEOUT,
            ready_timeout: READY_TIMEOUT,
            grace: GRACE,
        };
        match start_confined(&plan, &admission.boundary) {
            Ok(running) => {
                let timing = running.timing();
                let _ = running.close();
                Outcome::Ready(timing)
            }
            Err(Ok(failed)) => Outcome::Refused(format!("spawn-failed: {failed}")),
            Err(Err(ended)) => Outcome::Refused(
                ended
                    .refusal
                    .clone()
                    .unwrap_or_else(|| "sidecar-failed".into()),
            ),
        }
    }
}

/// The run path's sidecar environment (`sidecar_environment` there).
fn sidecar_environment(admission: &SidecarAdmission) -> BTreeMap<String, String> {
    #[allow(unused_mut)]
    let mut environment = BTreeMap::from([
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("TMPDIR".into(), admission.temporary.display().to_string()),
    ]);
    #[cfg(target_os = "macos")]
    environment.insert("SSL_CERT_FILE".into(), "/etc/ssl/cert.pem".into());
    environment
}

enum Outcome {
    Ready(Timing),
    Refused(String),
}

/// Background processes, killed when dropped. Processes rather than threads,
/// so no descriptor of this process is ever open across their execs.
struct Load(Vec<Child>);

impl Load {
    fn start(busy: usize, assess: usize, released: &Path, scratch: &Path) -> Self {
        let mut children = Vec::new();
        for _ in 0..busy {
            children.push(
                Command::new("/bin/sh")
                    .args(["-c", "while :; do :; done"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
        }
        for n in 0..assess {
            let dir = scratch.join(format!("assess-{n}"));
            fs::create_dir_all(&dir).unwrap();
            children.push(
                Command::new("/bin/sh")
                    .args([
                        "-c",
                        r#"i=0; while :; do cp "$1" "$2/w$i"; "$2/w$i" identity >/dev/null 2>&1; rm -f "$2/w$i"; i=$((i+1)); done"#,
                        "assess",
                    ])
                    .arg(released)
                    .arg(&dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
        }
        Self(children)
    }
}

impl Drop for Load {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn millis(d: Duration) -> f64 {
    (d.as_secs_f64() * 1_000_000.0).round() / 1000.0
}

/// min, median, p95 and max, nearest rank, in milliseconds.
fn summary(mut values: Vec<f64>) -> serde_json::Value {
    if values.is_empty() {
        return serde_json::Value::Null;
    }
    values.sort_by(f64::total_cmp);
    let rank = |p: f64| {
        let index = ((p * values.len() as f64).ceil() as usize).clamp(1, values.len()) - 1;
        values[index]
    };
    serde_json::json!({
        "min": values[0],
        "median": rank(0.5),
        "p95": rank(0.95),
        "max": values[values.len() - 1],
        "count": values.len(),
    })
}

struct Tally {
    started: Vec<f64>,
    ready: Vec<f64>,
    unavailable: usize,
    refusals: BTreeMap<String, usize>,
}

impl Tally {
    fn new() -> Self {
        Self {
            started: Vec::new(),
            ready: Vec::new(),
            unavailable: 0,
            refusals: BTreeMap::new(),
        }
    }

    fn add(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Ready(timing) => {
                if timing.observation == StartObservation::Unavailable {
                    self.unavailable += 1;
                }
                if let Some(started) = timing.started_after {
                    self.started.push(millis(started));
                }
                if let Some(ready) = timing.ready_after {
                    self.ready.push(millis(ready));
                }
            }
            Outcome::Refused(why) => *self.refusals.entry(why).or_default() += 1,
        }
    }
}

#[test]
#[ignore = "spec 043 M-1/M-2 measurement; needs STATECRAFT_M_PRODUCER"]
fn measure_the_released_producer_start_intervals() {
    let Ok(released) = std::env::var("STATECRAFT_M_PRODUCER") else {
        eprintln!("skipped: STATECRAFT_M_PRODUCER names no released wire-witness");
        return;
    };
    let released = fs::canonicalize(released).expect("STATECRAFT_M_PRODUCER exists");
    let iterations = env_number("STATECRAFT_M_ITER", 20);
    let busy = env_number("STATECRAFT_M_LOAD", 0);
    let assess = env_number("STATECRAFT_M_ASSESS", 0);
    let cases = std::env::var("STATECRAFT_M_CASES").unwrap_or_else(|_| "fresh,warm".into());
    let triple = current_target().unwrap_or("unknown");
    let load_name = if busy == 0 && assess == 0 {
        "idle".to_string()
    } else {
        format!("load-busy{busy}-assess{assess}")
    };

    let scratch = tempfile::tempdir().unwrap();
    let _load = Load::start(busy, assess, &released, scratch.path());
    if busy + assess > 0 {
        // Let the load reach a steady state before the first spawn.
        std::thread::sleep(Duration::from_secs(2));
    }

    for case in cases.split(',').map(str::trim).filter(|c| !c.is_empty()) {
        let mut tally = Tally::new();
        let mut warmup = serde_json::Value::Null;
        match case {
            "fresh" => {
                for _ in 0..iterations {
                    let mut home = Home::place(&released);
                    tally.add(home.attempt());
                }
            }
            "warm" => {
                let mut home = Home::place(&released);
                // The first, discarded attempt executes the copy once,
                // confined, so every measured attempt finds it assessed.
                warmup = match home.attempt() {
                    Outcome::Ready(t) => serde_json::json!({
                        "startedAfterMs": t.started_after.map(millis),
                        "readyAfterMs": t.ready_after.map(millis),
                    }),
                    Outcome::Refused(why) => serde_json::json!({ "refusal": why }),
                };
                for _ in 0..iterations {
                    tally.add(home.attempt());
                }
            }
            other => panic!("STATECRAFT_M_CASES names an unknown case: {other}"),
        }
        let line = serde_json::json!({
            "spec": "043",
            "case": format!("{case}/{load_name}"),
            "target": triple,
            "os": std::env::consts::OS,
            "confined": true,
            "iterations": iterations,
            "load": { "busy": busy, "assess": assess },
            "startTimeoutMs": millis(START_TIMEOUT),
            "readyTimeoutMs": millis(READY_TIMEOUT),
            "spawnToObservationMs": summary(tally.started),
            "observationToReadyMs": summary(tally.ready),
            "observationUnavailable": tally.unavailable,
            "refusals": tally.refusals,
            "warmup": warmup,
        });
        println!("M043 {line}");
    }
}
