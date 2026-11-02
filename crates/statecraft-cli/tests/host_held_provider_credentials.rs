//! Spec 051's acceptance through the home's real admission and the platform
//! boundary, with a fixture token and no provider: the operator's token never
//! reaches the child's environment or any file the attempt leaves (I-1, V-1);
//! the child gets a per-attempt token in a 0600 file of its per-run directory
//! and the gateway's loopback address (R-1); the gateway refuses a missing,
//! wrong, revoked or other attempt's token and forwards nothing (R-2); and a
//! witnessed attempt keeps spec 042's delivery (section 5). Nothing here
//! reaches the provider: every request is one the gateway answers itself.

#![cfg(unix)]

#[path = "support/provider_token.rs"]
mod provider_token;

use provider_token::FIXTURE_TOKEN;
use statecraft_home::confinement::{Admission, CONFIG_DIR_VARIABLE, Roots, admit};
use statecraft_home::provider_token::gateway::TOKEN_FILE;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const PATH: &str = "/usr/bin:/bin";
const BASE_URL: &str = "ANTHROPIC_BASE_URL";
const TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";

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
    provider_token::with_token(&places.home);
    places
}

fn admitted(p: &Places, witness: bool) -> Admission {
    admit(
        Roots {
            home: &p.home,
            target: &p.target,
            workspace: &p.workspace,
            exchange: &p.exchange,
            records: &p.records,
            gate: None,
            git_writes: Vec::new(),
            provider: true,
            authenticated: true,
            witness,
        },
        Path::new(env!("CARGO_BIN_EXE_statecraft-cli")),
        PATH,
    )
    .unwrap()
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => files_under(&p, out),
            Ok(m) if m.is_file() => out.push(p),
            _ => {}
        }
    }
}

/// Every file under the attempt's places but the operator's own token store
/// carries no byte sequence of the operator's token.
fn scan_for_operator_token(p: &Places) {
    let held = p.home.join("credentials/claude-code-token");
    let mut files = Vec::new();
    for root in [&p.home, &p.exchange, &p.workspace, &p.target] {
        files_under(root, &mut files);
    }
    for file in files.iter().filter(|f| **f != held) {
        let bytes = fs::read(file).unwrap_or_default();
        assert!(
            !bytes
                .windows(FIXTURE_TOKEN.len())
                .any(|w| w == FIXTURE_TOKEN.as_bytes()),
            "{} carries the operator's token",
            file.display()
        );
    }
}

fn ask(admission: &Admission, request: &str) -> String {
    let endpoint = admission.gateway_endpoint().expect("a gateway");
    let mut client = TcpStream::connect(endpoint).unwrap();
    client.write_all(request.as_bytes()).unwrap();
    let mut answer = String::new();
    client.read_to_string(&mut answer).unwrap();
    answer
}

fn preflight(token: &str) -> String {
    format!("HEAD /api/hello HTTP/1.1\r\nAuthorization: Bearer {token}\r\n\r\n")
}

#[test]
fn the_child_gets_a_per_attempt_token_and_the_gateway_never_the_operators() {
    let p = places();
    let admission = admitted(&p, false);
    let endpoint = admission.gateway_endpoint().expect("a gateway is open");
    assert!(endpoint.ip().is_loopback());

    // R-1 and I-1: the environment names the gateway and an attempt token,
    // and no variable carries the operator's.
    let variables = &admission.variables;
    assert_eq!(variables[BASE_URL], format!("http://{endpoint}"));
    let attempt = variables[TOKEN].clone();
    assert_ne!(attempt, FIXTURE_TOKEN);
    for (name, value) in variables {
        assert!(!value.contains(FIXTURE_TOKEN), "{name} carries the token");
    }
    let file = Path::new(&variables[CONFIG_DIR_VARIABLE]).join(TOKEN_FILE);
    assert_eq!(fs::read_to_string(&file).unwrap(), attempt);
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );

    // Section 3.3: the record names the gateway, not a witness, and the
    // single-endpoint probes passed against it.
    let record = admission.boundary.record();
    assert_eq!(record.gateway_endpoint, Some(endpoint));
    assert_eq!(record.witness_endpoint, None);
    assert_eq!(record.self_test.get("witnessEndpointConnect"), Some(&true));
    assert_eq!(record.self_test.get("witnessListenerDenied"), Some(&true));
    let json = serde_json::to_string(record).unwrap();
    assert!(!json.contains(FIXTURE_TOKEN) && !json.contains(&attempt));

    // V-1: no file the attempt can read or leaves carries the token.
    scan_for_operator_token(&p);

    // R-2: the attempt's token is admitted, and nothing else is.
    assert!(ask(&admission, &preflight(&attempt)).starts_with("HTTP/1.1 200 "));
    for wrong in ["", "wrong", FIXTURE_TOKEN] {
        assert!(
            ask(&admission, &preflight(wrong)).starts_with("HTTP/1.1 401 "),
            "{wrong:?}"
        );
    }
    let other_places = places();
    let other = admitted(&other_places, false);
    let others = other.variables[TOKEN].clone();
    assert_ne!(others, attempt);
    assert!(ask(&admission, &preflight(&others)).starts_with("HTTP/1.1 401 "));

    // Section 3.2: once the attempt has an outcome, its token is refused.
    admission.revoke_provider_gateway();
    assert!(ask(&admission, &preflight(&attempt)).starts_with("HTTP/1.1 401 "));
    assert!(ask(&other, &preflight(&others)).starts_with("HTTP/1.1 200 "));

    // Dropped with the admission, the gateway and the token file are gone.
    drop(admission);
    assert!(TcpStream::connect(endpoint).is_err());
    assert!(!file.exists());
    scan_for_operator_token(&p);
}

#[test]
fn a_confined_child_reaches_the_gateway_through_its_one_loopback_grant() {
    let curl = Path::new("/usr/bin/curl");
    if !curl.is_file() {
        eprintln!("skipped: /usr/bin/curl is absent");
        return;
    }
    let p = places();
    let admission = admitted(&p, false);
    let url = format!("{}/api/hello", admission.variables[BASE_URL]);
    let program = admission.boundary.resolve(curl, PATH).unwrap();
    let run = |token: &str| {
        let header = format!("Authorization: Bearer {token}");
        let captured = statecraft_adapter::supervisor::capture_confined(
            &program,
            &[
                "-s",
                "-I",
                "-o",
                "/dev/null",
                "-w",
                "%{http_code}",
                "-H",
                &header,
                &url,
            ],
            &p.workspace,
            &admission.variables,
            b"",
            std::time::Duration::from_secs(20),
            &admission.boundary,
        )
        .unwrap();
        String::from_utf8_lossy(&captured.stdout).into_owned()
    };
    assert_eq!(run(&admission.variables[TOKEN]), "200");
    assert_eq!(run("a-copy-from-another-attempt"), "401");
}

#[test]
fn a_witnessed_attempt_keeps_the_environment_delivery_and_opens_no_gateway() {
    let p = places();
    let admission = admitted(&p, true);
    assert_eq!(admission.gateway_endpoint(), None);
    assert_eq!(admission.variables[TOKEN], FIXTURE_TOKEN);
    assert!(!admission.variables.contains_key(BASE_URL));
    let record = admission.boundary.record();
    assert_eq!(record.gateway_endpoint, None);
    assert!(record.witness_endpoint.is_some());
    let configuration = Path::new(&admission.variables[CONFIG_DIR_VARIABLE]);
    assert!(!configuration.join(TOKEN_FILE).exists());
}

#[test]
fn without_a_stored_token_an_unwitnessed_launch_refuses_before_any_gateway() {
    let p = places();
    fs::remove_file(p.home.join("credentials/claude-code-token")).unwrap();
    let refused = admit(
        Roots {
            home: &p.home,
            target: &p.target,
            workspace: &p.workspace,
            exchange: &p.exchange,
            records: &p.records,
            gate: None,
            git_writes: Vec::new(),
            provider: true,
            authenticated: true,
            witness: false,
        },
        Path::new(env!("CARGO_BIN_EXE_statecraft-cli")),
        PATH,
    );
    let Err(why) = refused else {
        panic!("admitted without a token");
    };
    assert!(why.contains("provider token set"), "{why}");
}
