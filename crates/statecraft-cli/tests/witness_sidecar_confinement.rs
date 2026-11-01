//! Spec 013 section 3.6.2: the witness sidecar's own confinement, measured
//! under the real operating-system policy by the fixed probe, and a fake
//! sidecar completing the protocol inside it.

use statecraft_adapter::boundary::{Grant, Policy, Prepared, Probe};
use statecraft_adapter::witness::ledger::VerifiedProducer;
use statecraft_adapter::witness::protocol::{
    AttemptBinding, Authority, Limits, Start, Termination,
};
use statecraft_adapter::witness::sidecar::{Plan, start_confined};
use std::collections::BTreeMap;
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    area: PathBuf,
    _unix: std::os::unix::net::UnixListener,
    protected: fs::File,
}

/// `home` holds the evidence and authority roots and the sidecar's area;
/// `target` is the operator checkout. Both are inaccessible except the area.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let area = root.join("home/records/witness");
    for directory in ["home/records/witness", "target", "bin"] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    for file in ["home/sentinel", "home/records/sentinel", "target/sentinel"] {
        fs::write(root.join(file), b"unchanged").unwrap();
    }
    let unix = std::os::unix::net::UnixListener::bind(root.join("service.sock")).unwrap();
    Fixture {
        protected: fs::File::open(root.join("home/sentinel")).unwrap(),
        _unix: unix,
        area,
        root,
        _dir: dir,
    }
}

impl Fixture {
    fn policy(&self) -> Policy {
        Policy {
            inaccessible: vec![self.root.join("home"), self.root.join("target")],
            readonly: vec![],
            readable: vec![self.area.clone()],
            writable: vec![Grant {
                path: self.area.clone(),
                directory: true,
            }],
            adjacent_prefixes: vec![],
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            home: self.root.join("home/sentinel"),
            records: self.root.join("home/records/sentinel"),
            target: self.root.join("target/sentinel"),
            workspace: self.area.join("probe"),
            gate: self.area.join("unused-gate.log"),
            unix_socket: self.root.join("service.sock"),
            loopback_port: 9,
            supervisor: std::process::id(),
            protected_fd: self.protected.as_raw_fd(),
        }
    }
}

fn vacant_endpoint() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap()
}

fn probe_executable() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_statecraft-cli"))
}

#[test]
fn the_sidecar_binds_only_its_endpoint_and_reaches_no_protected_path() {
    let f = fixture();
    let endpoint = vacant_endpoint();
    let mut prepared = Prepared::prepare_sidecar(f.policy(), endpoint, &[443]).unwrap();
    if let Err(refusal) = prepared.self_test(probe_executable(), &f.probe()) {
        panic!("{refusal}");
    }
    let record = prepared.record();
    assert_eq!(record.witness_endpoint, Some(endpoint));
    assert_eq!(record.sidecar_outbound_ports.as_deref(), Some(&[443][..]));
    for key in [
        "sidecarEndpointBind",
        "sidecarOtherListenerDenied",
        "sidecarDisallowedOutboundDenied",
        "targetReadDenied",
        "recordsReadDenied",
    ] {
        assert_eq!(record.self_test.get(key), Some(&true), "{key}");
    }
    assert!(!record.self_test.contains_key("witnessEndpointConnect"));
    let linux = cfg!(target_os = "linux");
    for residual in [
        "sidecar-bind-address-unenforced",
        "sidecar-connect-address-unenforced",
        "sidecar-udp-unmediated",
    ] {
        assert_eq!(
            record.open_items.iter().any(|i| i.starts_with(residual)),
            linux,
            "{residual}: {:?}",
            record.open_items
        );
    }
}

#[test]
fn an_allowlist_without_a_port_or_naming_the_endpoint_is_refused() {
    let f = fixture();
    let endpoint = vacant_endpoint();
    for outbound in [&[][..], &[0][..], &[endpoint.port()][..]] {
        let refusal = Prepared::prepare_sidecar(f.policy(), endpoint, outbound).unwrap_err();
        assert_eq!(refusal.step, "sidecar-outbound", "{outbound:?}");
    }
    let refusal = Prepared::prepare_sidecar(f.policy(), "0.0.0.0:41999".parse().unwrap(), &[443])
        .unwrap_err();
    assert_eq!(refusal.step, "witness-endpoint");
}

#[test]
fn a_confined_sidecar_completes_the_protocol_and_cannot_read_the_checkout() {
    let f = fixture();
    let endpoint = vacant_endpoint();
    let mut prepared = Prepared::prepare_sidecar(f.policy(), endpoint, &[443]).unwrap();
    prepared.self_test(probe_executable(), &f.probe()).unwrap();
    let out = f.area.join("attempt-1");
    let script = f.root.join("bin/fake-sidecar.sh");
    let binding = r#"{"run_id":"013-run","attempt":1,"effect_id":"013-run/1/witness-sidecar"}"#;
    fs::write(
        &script,
        format!(
            r#"OUT='{out}'
B='{binding}'
P='"protocol":"wire-witness.sidecar/1"'
read -r start
mkdir -m 700 "$OUT" || exit 5
printf 'cert' > "$OUT/ca.pem"
if cat '{checkout}' >/dev/null 2>&1; then echo leaked > "$OUT/observed"; else echo denied > "$OUT/observed"; fi
printf '{{%s,"type":"ready","binding":%s,"endpoint":"{endpoint}","certificate_path":"%s/ca.pem"}}\n' "$P" "$B" "$OUT"
cat >/dev/null
printf '{{%s,"type":"finished","binding":%s,"completeness":"absent","reason":"no-exchange-observed","exchange_count":0,"finding_count":0}}\n' "$P" "$B"
"#,
            out = out.display(),
            checkout = f.root.join("target/sentinel").display(),
        ),
    )
    .unwrap();
    let plan = Plan {
        producer: VerifiedProducer {
            name: "wire-witness".into(),
            version: "0.1.0".into(),
            interface: "wire-witness.sidecar/1".into(),
            target: "test-target".into(),
            executable_digest: "sha256:00".into(),
            qualification: "qualification/test.md".into(),
            path: script,
        },
        start: Start {
            binding: AttemptBinding::new("013-run", 1, "013-run/1/witness-sidecar").unwrap(),
            capture_authorities: vec![Authority {
                authority: "api.example.test:443".into(),
                provider: "unknown".into(),
                protocols: vec!["http/1.1".into()],
            }],
            authentication_tunnels: Vec::new(),
            requested_retention: "metadata-only".into(),
            limits: Limits::default(),
            output_directory: out.clone(),
            certificate_path: out.join("ca.pem"),
            host_policy_id: "statecraft/013-test".into(),
            listen_endpoint: endpoint,
        },
        wrapper: vec!["/bin/sh".into()],
        environment: BTreeMap::from([("PATH".into(), "/bin:/usr/bin".into())]),
        start_timeout: Duration::from_secs(60),
        ready_timeout: Duration::from_secs(20),
        grace: Duration::from_secs(5),
    };
    let ended = match start_confined(&plan, &prepared) {
        Ok(running) => running.close(),
        Err(Ok(failed)) => panic!("{failed}"),
        Err(Err(ended)) => panic!("{:?} {:?}", ended.refusal, ended.reader.findings()),
    };
    assert_eq!(ended.termination, Termination::Finished);
    assert!(
        ended.reader.findings().is_empty(),
        "{:?}",
        ended.reader.findings()
    );
    assert_eq!(
        fs::read_to_string(out.join("observed")).unwrap(),
        "denied\n"
    );
    assert_eq!(
        fs::read(f.root.join("target/sentinel")).unwrap(),
        b"unchanged"
    );
}

#[test]
fn the_child_boundary_is_not_the_sidecar_boundary() {
    let f = fixture();
    let endpoint = vacant_endpoint();
    let child = Prepared::prepare_witnessed(f.policy(), endpoint).unwrap();
    let plan_error = start_confined(
        &Plan {
            producer: VerifiedProducer {
                name: "wire-witness".into(),
                version: "0.1.0".into(),
                interface: "wire-witness.sidecar/1".into(),
                target: "test-target".into(),
                executable_digest: "sha256:00".into(),
                qualification: "qualification/test.md".into(),
                path: "/bin/true".into(),
            },
            start: Start {
                binding: AttemptBinding::new("r", 1, "e").unwrap(),
                capture_authorities: Vec::new(),
                authentication_tunnels: Vec::new(),
                requested_retention: "metadata-only".into(),
                limits: Limits::default(),
                output_directory: f.area.join("x"),
                certificate_path: f.area.join("x/ca.pem"),
                host_policy_id: "p".into(),
                listen_endpoint: endpoint,
            },
            wrapper: Vec::new(),
            environment: BTreeMap::new(),
            start_timeout: Duration::from_secs(1),
            ready_timeout: Duration::from_secs(1),
            grace: Duration::from_secs(1),
        },
        &child,
    );
    assert!(matches!(plan_error, Err(Ok(_))));
}
