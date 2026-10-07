//! Spec 004 section 3.18: real OS policy, fixed probe and inert reads.

use statecraft_adapter::boundary::{DataRoot, Grant, Policy, Prepared, Probe};
use std::fs;
use std::os::fd::AsRawFd;
use std::path::Path;

#[test]
fn exact_policy_preserves_readonly_access_and_denies_evidence_access() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    for directory in [
        "home",
        "home/records",
        "home/exchange",
        "workspace",
        "target",
        "other-workspace",
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
    let policy = Policy {
        inaccessible: vec![root.join("home")],
        readonly: vec![root.join("target")],
        readable: vec![root.join("home/exchange")],
        writable: vec![
            Grant {
                path: root.join("workspace"),
                directory: true,
            },
            Grant {
                path: root.join("home/exchange/gate.log"),
                directory: false,
            },
        ],
        adjacent_prefixes: vec![],
    };
    let mut prepared = Prepared::prepare(policy).unwrap();
    let protected = fs::File::open(root.join("home/sentinel")).unwrap();
    let socket_path = root.join("service.sock");
    let _unix = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let probe = Probe {
        home: root.join("home/sentinel"),
        records: root.join("home/records/sentinel"),
        target: root.join("target/sentinel"),
        workspace: root.join("workspace/probe"),
        gate: root.join("home/exchange/gate.log"),
        unix_socket: socket_path,
        loopback_port: tcp.local_addr().unwrap().port(),
        supervisor: std::process::id(),
        protected_fd: protected.as_raw_fd(),
    };
    if let Err(refusal) =
        prepared.self_test(Path::new(env!("CARGO_BIN_EXE_statecraft-cli")), &probe)
    {
        panic!("{refusal}");
    }
    assert!(prepared.record().self_test.values().all(|v| *v));
    assert!(!prepared.record().open_items.is_empty());
    // A failed probe cannot replace the durable evidence of an admitted test.
    // Naming inaccessible data as the positive read must refuse, not weaken
    // the policy until the probe passes.
    let admitted = prepared.record().self_test.clone();
    let mut failed_probe = probe.clone();
    failed_probe.target = root.join("home/sentinel");
    assert_eq!(
        prepared
            .self_test(
                Path::new(env!("CARGO_BIN_EXE_statecraft-cli")),
                &failed_probe
            )
            .unwrap_err()
            .step,
        "self-test"
    );
    assert_eq!(prepared.record().self_test, admitted);
    assert_eq!(fs::read(root.join("home/sentinel")).unwrap(), b"unchanged");
    assert_eq!(
        fs::read(root.join("target/sentinel")).unwrap(),
        b"unchanged"
    );
    fs::write(root.join("other-workspace/sentinel"), b"other").unwrap();
    std::os::unix::fs::symlink(root.join("target"), root.join("workspace/target-alias")).unwrap();
    let environment = std::collections::BTreeMap::from([("PATH".into(), "/usr/bin:/bin".into())]);
    let script = r#"
        cat "$1/sentinel" || exit 10
        cat "$2/sentinel" || exit 11
        for protected in "$1/sentinel" "$2/sentinel" "$3/target-alias/sentinel"; do
            (printf changed > "$protected") && exit 12
            (printf changed >> "$protected") && exit 13
            rm "$protected" && exit 14
            mv "$protected" "$3/stolen" && exit 15
            chmod 777 "$protected" && exit 16
            ln "$protected" "$3/hard-alias" && exit 17
        done
        printf trace >> "$4" || exit 21
        chmod 777 "$4" && exit 22
        rm "$4" && exit 23
        mv "$4" "$3/stolen-gate" && exit 24
        (printf changed > "$1/sentinel") &
        wait
        printf own > "$3/ordinary"
        git init -q "$3" || exit 18
        git -C "$3" add ordinary || exit 19
        git -C "$3" -c user.name=fixture -c user.email=fixture@example.invalid -c core.hooksPath=/dev/null commit -qm fixture || exit 20
    "#;
    let target = root.join("target");
    let other = root.join("other-workspace");
    let workspace = root.join("workspace");
    let gate = root.join("home/exchange/gate.log");
    let output = statecraft_adapter::supervisor::capture_confined(
        Path::new("/bin/sh"),
        &[
            "-c",
            script,
            "boundary-fixture",
            target.to_str().unwrap(),
            other.to_str().unwrap(),
            workspace.to_str().unwrap(),
            gate.to_str().unwrap(),
        ],
        &workspace,
        &environment,
        b"",
        std::time::Duration::from_secs(10),
        &prepared,
    )
    .unwrap();
    assert_eq!(
        output.code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(target.join("sentinel")).unwrap(), b"unchanged");
    assert_eq!(fs::read(other.join("sentinel")).unwrap(), b"other");
    assert_eq!(fs::read(workspace.join("ordinary")).unwrap(), b"own");
    let exchange = DataRoot::open(&root.join("home/exchange")).unwrap();
    std::os::unix::fs::symlink(
        root.join("home/sentinel"),
        root.join("home/exchange/substitution"),
    )
    .unwrap();
    assert!(exchange.read(Path::new("substitution"), 4096).is_err());
    assert!(exchange.read(Path::new("../sentinel"), 4096).is_err());
    let (prefix, truncated) = exchange.read_prefix(Path::new("gate.log"), 3).unwrap();
    assert_eq!(prefix.len(), 3);
    assert!(truncated);
    assert!(
        prepared
            .resolve(Path::new("sh"), ".:/usr/bin:/bin")
            .is_err()
    );
    assert!(
        prepared
            .resolve(
                Path::new("sh"),
                &root.join("workspace").display().to_string()
            )
            .is_err()
    );
}

#[test]
fn writable_roots_cannot_replace_or_cover_protected_roots() {
    let fixture = tempfile::tempdir().unwrap();
    let protected = fixture.path().join("protected");
    fs::create_dir(&protected).unwrap();
    let alias = fixture.path().join("alias");
    std::os::unix::fs::symlink(&protected, &alias).unwrap();
    for writable in [fixture.path().to_path_buf(), protected.clone(), alias] {
        let refusal = Prepared::prepare(Policy {
            inaccessible: vec![protected.clone()],
            readonly: vec![],
            readable: vec![],
            writable: vec![Grant {
                path: writable,
                directory: true,
            }],
            adjacent_prefixes: vec![],
        })
        .err()
        .unwrap();
        assert_eq!(refusal.step, "write-root");
    }
}

#[test]
fn existing_hardlink_to_protected_bytes_refuses_before_launch() {
    let fixture = tempfile::tempdir().unwrap();
    let protected = fixture.path().join("protected");
    let writable = fixture.path().join("workspace");
    fs::create_dir_all(&protected).unwrap();
    fs::create_dir_all(&writable).unwrap();
    fs::write(protected.join("record"), b"authority").unwrap();
    fs::hard_link(protected.join("record"), writable.join("alias")).unwrap();
    let refusal = Prepared::prepare(Policy {
        inaccessible: vec![protected],
        readonly: vec![],
        readable: vec![],
        writable: vec![Grant {
            path: writable,
            directory: true,
        }],
        adjacent_prefixes: vec![],
    })
    .err()
    .unwrap();
    assert_eq!(refusal.step, "hardlink");
}
