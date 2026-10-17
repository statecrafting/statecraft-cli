//! Spec 013 section 3.6.1: the child's one route to the witness sidecar,
//! measured under the real operating-system policy and the fixed probe.

use statecraft_adapter::boundary::{Grant, Policy, Prepared, Probe};
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    _unix: std::os::unix::net::UnixListener,
    other: TcpListener,
    protected: fs::File,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    for directory in [
        "home",
        "home/records",
        "home/exchange",
        "workspace",
        "target",
    ] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    for file in [
        "home/sentinel",
        "home/records/sentinel",
        "target/sentinel",
        "home/exchange/gate.log",
    ] {
        fs::write(root.join(file), b"unchanged").unwrap();
    }
    let unix = std::os::unix::net::UnixListener::bind(root.join("service.sock")).unwrap();
    Fixture {
        protected: fs::File::open(root.join("home/sentinel")).unwrap(),
        other: TcpListener::bind("127.0.0.1:0").unwrap(),
        _unix: unix,
        root,
        _dir: dir,
    }
}

impl Fixture {
    fn policy(&self) -> Policy {
        Policy {
            inaccessible: vec![self.root.join("home")],
            readonly: vec![self.root.join("target")],
            readable: vec![self.root.join("home/exchange")],
            writable: vec![
                Grant {
                    path: self.root.join("workspace"),
                    directory: true,
                },
                Grant {
                    path: self.root.join("home/exchange/gate.log"),
                    directory: false,
                },
            ],
            adjacent_prefixes: vec![],
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            home: self.root.join("home/sentinel"),
            records: self.root.join("home/records/sentinel"),
            target: self.root.join("target/sentinel"),
            workspace: self.root.join("workspace/probe"),
            gate: self.root.join("home/exchange/gate.log"),
            unix_socket: self.root.join("service.sock"),
            loopback_port: self.other.local_addr().unwrap().port(),
            supervisor: std::process::id(),
            protected_fd: self.protected.as_raw_fd(),
        }
    }
}

fn probe_executable() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_statecraft-cli"))
}

#[test]
fn the_child_reaches_the_endpoint_and_no_other_loopback_port() {
    let f = fixture();
    let sidecar = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = sidecar.local_addr().unwrap();
    let mut prepared = Prepared::prepare_witnessed(f.policy(), endpoint).unwrap();
    if let Err(refusal) = prepared.self_test(probe_executable(), &f.probe()) {
        panic!("{refusal}");
    }
    let record = prepared.record();
    assert_eq!(record.witness_endpoint, Some(endpoint));
    assert_eq!(record.self_test.get("witnessEndpointConnect"), Some(&true));
    assert_eq!(record.self_test.get("loopbackDenied"), Some(&true));
    assert_eq!(record.self_test.get("witnessListenerDenied"), Some(&true));
    #[cfg(target_os = "macos")]
    assert_eq!(
        record.self_test.get("witnessNonLoopbackDenied"),
        Some(&true)
    );
    let residual = format!("loopback-address-unenforced: TCP port {}", endpoint.port());
    assert_eq!(
        record.open_items.iter().any(|i| i.starts_with(&residual)),
        cfg!(target_os = "linux"),
        "{:?}",
        record.open_items
    );
}

#[test]
fn an_endpoint_nobody_listens_on_fails_the_self_test() {
    let f = fixture();
    let vacant = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = vacant.local_addr().unwrap();
    drop(vacant);
    let mut prepared = Prepared::prepare_witnessed(f.policy(), endpoint).unwrap();
    let refusal = prepared
        .self_test(probe_executable(), &f.probe())
        .unwrap_err();
    assert_eq!(refusal.step, "self-test");
    assert!(prepared.record().self_test.is_empty());
}

#[test]
fn without_an_endpoint_the_record_and_probe_are_unchanged() {
    let f = fixture();
    let mut prepared = Prepared::prepare(f.policy()).unwrap();
    prepared.self_test(probe_executable(), &f.probe()).unwrap();
    let record = prepared.record();
    assert_eq!(record.witness_endpoint, None);
    assert!(!record.self_test.contains_key("witnessEndpointConnect"));
    let json = serde_json::to_value(record).unwrap();
    assert!(json.get("witnessEndpoint").is_none());
}

#[test]
fn only_a_127_0_0_1_endpoint_on_an_ordinary_port_is_admitted() {
    let f = fixture();
    for endpoint in [
        "0.0.0.0:41999",
        "[::1]:41999",
        "192.0.2.1:41999",
        "127.0.0.1:443",
        "127.0.0.1:0",
    ] {
        let endpoint: SocketAddr = endpoint.parse().unwrap();
        let refusal = Prepared::prepare_witnessed(f.policy(), endpoint).unwrap_err();
        assert_eq!(refusal.step, "witness-endpoint", "{endpoint}");
    }
}

#[test]
fn the_denied_loopback_port_may_not_be_the_endpoint() {
    let f = fixture();
    let endpoint = f.other.local_addr().unwrap();
    let mut prepared = Prepared::prepare_witnessed(f.policy(), endpoint).unwrap();
    let refusal = prepared
        .self_test(probe_executable(), &f.probe())
        .unwrap_err();
    assert_eq!(refusal.step, "probe-input");
}
