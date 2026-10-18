//! Spec 013 sections 3.3 and 3.6: the home admits a witnessed child with one
//! reserved endpoint, and admits the sidecar under its own confinement with
//! only its area writable and only the producer's directory readable.

use statecraft_adapter::witness::ledger::VerifiedProducer;
use statecraft_adapter::witness::protocol::{AttemptBinding, Limits, Start, Termination};
use statecraft_adapter::witness::sidecar::{Plan, start_confined};
use statecraft_home::confinement::{Roots, admit};
use statecraft_home::witness::{SidecarRoots, admit_sidecar};
use std::collections::BTreeMap;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

struct Places {
    _dir: tempfile::TempDir,
    home: PathBuf,
    records: PathBuf,
    exchange: PathBuf,
    target: PathBuf,
    workspace: PathBuf,
}

fn places() -> Places {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let home = root.join("home");
    let places = Places {
        records: home.join("records/k.startup"),
        exchange: home.join("exchange/k/runs/r/1"),
        target: root.join("target"),
        workspace: root.join("workspace"),
        home,
        _dir: dir,
    };
    for directory in [
        &places.records,
        &places.exchange,
        &places.target,
        &places.workspace,
    ] {
        fs::create_dir_all(directory).unwrap();
    }
    fs::write(places.target.join("checkout"), b"operator").unwrap();
    places
}

fn probe_executable() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_statecraft-cli"))
}

fn roots(p: &Places, witness: bool) -> Roots<'_> {
    Roots {
        home: &p.home,
        target: &p.target,
        workspace: &p.workspace,
        exchange: &p.exchange,
        records: &p.records,
        gate: None,
        git_writes: Vec::new(),
        provider: false,
        authenticated: false,
        witness,
    }
}

const PATH: &str = "/usr/bin:/bin";

#[test]
fn an_unwitnessed_child_reserves_no_endpoint() {
    let p = places();
    let admission = admit(roots(&p, false), probe_executable(), PATH).unwrap();
    assert_eq!(admission.witness_endpoint(), None);
    assert_eq!(admission.boundary.record().witness_endpoint, None);
}

#[test]
fn a_witnessed_child_reaches_only_the_reserved_endpoint_while_it_is_held() {
    let p = places();
    let mut admission = admit(roots(&p, true), probe_executable(), PATH).unwrap();
    let endpoint = admission
        .witness_endpoint()
        .expect("an endpoint is reserved");
    assert!(endpoint.ip().is_loopback());
    let record = admission.boundary.record();
    assert_eq!(record.witness_endpoint, Some(endpoint));
    assert_eq!(record.self_test.get("witnessEndpointConnect"), Some(&true));
    assert_eq!(record.self_test.get("witnessListenerDenied"), Some(&true));
    // Released, nothing listens and the connect probe fails the retest;
    // once something binds the endpoint again, as the sidecar will, it passes.
    admission.release_witness_endpoint();
    assert!(admission.retest(probe_executable()).is_err());
    let _sidecar = TcpListener::bind(endpoint).unwrap();
    admission.retest(probe_executable()).unwrap();
}

fn producer(p: &Places, script: &str) -> VerifiedProducer {
    let directory = p.home.join("producers/wire-witness/0.1.0/test-target");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("wire-witness");
    fs::write(&path, script).unwrap();
    VerifiedProducer {
        name: "wire-witness".into(),
        version: "0.1.0".into(),
        interface: "wire-witness.sidecar/1".into(),
        target: "test-target".into(),
        executable_digest: "sha256:00".into(),
        qualification: "qualification/test.md".into(),
        path,
    }
}

#[test]
fn the_sidecar_runs_its_producer_writes_only_its_area_and_reads_no_protected_root() {
    let p = places();
    let area = p.exchange.join(statecraft_home::witness::AREA);
    let output = area.join(statecraft_home::witness::OUTPUT);
    let binding = r#"{"run_id":"r","attempt":1,"effect_id":"r/1/witness-sidecar"}"#;
    let probes = [
        ("target", p.target.join("checkout")),
        ("records", p.records.clone()),
        ("home", p.home.join("records")),
        ("workspace", p.workspace.clone()),
    ];
    let mut checks = String::new();
    // Reads, not `stat`: Landlock does not mediate metadata, so a file is
    // read with `cat` and a directory listed with `ls`.
    for (name, path) in &probes {
        let read = if path.is_dir() { "ls" } else { "cat" };
        checks.push_str(&format!(
            "if {read} '{}' >/dev/null 2>&1; then echo {name} >> \"$OUT/leaked\"; fi\n",
            path.display()
        ));
    }
    let script = format!(
        r#"exec 2>"$TMPDIR/err"
OUT='{out}'
B='{binding}'
P='"protocol":"wire-witness.sidecar/1"'
read -r start
mkdir -m 700 "$OUT" || exit 5
printf 'cert' > "$OUT/ca.pem"
touch "$OUT/leaked"
{checks}if touch '{escape}' 2>/dev/null; then echo exchange >> "$OUT/leaked"; fi
printf '{{%s,"type":"ready","binding":%s,"endpoint":"%s","certificate_path":"%s/ca.pem"}}\n' "$P" "$B" "$EP" "$OUT"
cat >/dev/null
printf '{{%s,"type":"finished","binding":%s,"completeness":"absent","reason":"no-exchange-observed","exchange_count":0,"finding_count":0}}\n' "$P" "$B"
"#,
        out = output.display(),
        escape = p.exchange.join("escaped").display(),
    );
    let producer = producer(&p, &script);
    let endpoint = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let admission = admit_sidecar(
        &SidecarRoots {
            home: &p.home,
            records: &p.records,
            target: &p.target,
            workspace: &p.workspace,
            area: &area,
            producer: &producer,
        },
        probe_executable(),
        endpoint,
        &[443],
    )
    .unwrap();
    assert_eq!(admission.output, output);
    let record = admission.boundary.record();
    assert_eq!(record.witness_endpoint, Some(endpoint));
    for key in ["homeReadDenied", "recordsReadDenied", "targetReadDenied"] {
        assert_eq!(record.self_test.get(key), Some(&true), "{key}");
    }
    let plan = Plan {
        producer,
        start: Start {
            binding: AttemptBinding::new("r", 1, "r/1/witness-sidecar").unwrap(),
            capture_authorities: Vec::new(),
            authentication_tunnels: Vec::new(),
            requested_retention: "metadata-only".into(),
            limits: Limits::default(),
            output_directory: output.clone(),
            certificate_path: output.join("ca.pem"),
            host_policy_id: "statecraft/013-test".into(),
            listen_endpoint: endpoint,
        },
        wrapper: vec!["/bin/sh".into()],
        environment: BTreeMap::from([
            ("PATH".into(), PATH.into()),
            ("EP".into(), endpoint.to_string()),
            ("TMPDIR".into(), admission.temporary.display().to_string()),
        ]),
        ready_timeout: Duration::from_secs(20),
        grace: Duration::from_secs(5),
    };
    let ended = match start_confined(&plan, &admission.boundary) {
        Ok(running) => running.close(),
        Err(Ok(failed)) => panic!("{failed}"),
        Err(Err(ended)) => panic!(
            "{:?} {:?} {:?}",
            ended.refusal,
            ended.reader.findings(),
            fs::read_to_string(admission.temporary.join("err"))
        ),
    };
    assert_eq!(ended.termination, Termination::Finished);
    assert!(
        ended.reader.findings().is_empty(),
        "{:?}",
        ended.reader.findings()
    );
    assert_eq!(fs::read_to_string(output.join("leaked")).unwrap(), "");
    assert!(!p.exchange.join("escaped").exists());
}

#[test]
fn an_existing_area_is_refused() {
    let p = places();
    let area = p.exchange.join(statecraft_home::witness::AREA);
    fs::create_dir(&area).unwrap();
    let producer = producer(&p, "");
    let endpoint = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let refused = admit_sidecar(
        &SidecarRoots {
            home: &p.home,
            records: &p.records,
            target: &p.target,
            workspace: &p.workspace,
            area: &area,
            producer: &producer,
        },
        probe_executable(),
        endpoint,
        &[443],
    );
    assert!(refused.is_err());
}
