//! Product launch admission using the provider-neutral boundary (004 section 3.18).

use statecraft_adapter::boundary::{DataRoot, Grant, Policy, Prepared, Probe};
use std::collections::BTreeMap;
use std::fs::File;
use std::net::TcpListener;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Existing supervisor-owned paths for one launch.
pub struct Roots<'a> {
    /// Protected product home.
    pub home: &'a Path,
    /// Read-only operator checkout.
    pub target: &'a Path,
    /// The attempt's workspace.
    pub workspace: &'a Path,
    /// Readable exchange area, whose entries remain protected.
    pub exchange: &'a Path,
    /// Protected launch records, possibly outside the home for a capture.
    pub records: &'a Path,
    /// Existing gate log, if this is a gated launch.
    pub gate: Option<&'a Path>,
    /// Exact private Git administrative/reference roots, for a provider only.
    pub git_writes: Vec<PathBuf>,
    /// Grant the provider's required configuration paths.
    pub provider: bool,
}

/// A probe and its live sentinels. Held until the corresponding launch finishes.
pub struct Admission {
    /// Exact admitted boundary, attachable to a constructed environment.
    pub boundary: Arc<Prepared>,
    /// Environment additions for the private temporary directory.
    pub variables: BTreeMap<String, String>,
    /// Exchange handle captured before any child exists.
    pub exchange: DataRoot,
    common: Option<DataRoot>,
    scratch: tempfile::TempDir,
    home_sentinel: tempfile::NamedTempFile,
    records_sentinel: tempfile::NamedTempFile,
    target_sentinel: tempfile::NamedTempFile,
    socket: UnixListener,
    socket_directory: tempfile::TempDir,
    gate: File,
    listener: TcpListener,
    protected: File,
    probe: Probe,
}

impl Admission {
    /// Apply the fixed probe again before another process uses this policy.
    pub fn retest(&self, executable: &Path) -> Result<(), String> {
        // These owned resources keep every negative probe meaningful.
        let _held = (
            &self.scratch,
            &self.home_sentinel,
            &self.records_sentinel,
            &self.target_sentinel,
            &self.socket,
            &self.socket_directory,
            &self.listener,
            &self.protected,
        );
        let length = self.gate.metadata().map_err(|e| e.to_string())?.len();
        let result = self
            .boundary
            .test(executable, &self.probe)
            .map(|_| ())
            .map_err(|e| e.to_string());
        self.gate.set_len(length).map_err(|e| e.to_string())?;
        result
    }
}

/// Admit a launch. No provider process or attempt record is created here.
pub fn admit(roots: Roots<'_>, executable: &Path, path: &str) -> Result<Admission, String> {
    fn prepare(
        roots: Roots<'_>,
        executable: &Path,
        path: &str,
    ) -> Result<Admission, Box<dyn std::error::Error>> {
        for directory in [roots.home, roots.records, roots.exchange] {
            std::fs::create_dir_all(directory)?;
        }
        let scratch = tempfile::Builder::new()
            .prefix("temporary-")
            .tempdir_in(roots.exchange)?;
        let home_sentinel = tempfile::NamedTempFile::new_in(roots.home)?;
        let records_sentinel = tempfile::NamedTempFile::new_in(roots.records)?;
        let target_sentinel = tempfile::NamedTempFile::new_in(roots.target)?;
        let protected = File::open(home_sentinel.path())?;
        let socket_directory = tempfile::Builder::new()
            .prefix("probe-")
            .tempdir_in(roots.home)?;
        let socket_path = socket_directory.path().join("socket");
        let socket = UnixListener::bind(&socket_path)?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let gate = roots
            .gate
            .map(Path::to_path_buf)
            .unwrap_or_else(|| scratch.path().join("gate-probe"));
        if roots.gate.is_none() {
            std::fs::write(&gate, b"")?;
        }
        let mut writable = vec![
            Grant {
                path: roots.workspace.into(),
                directory: true,
            },
            Grant {
                path: scratch.path().into(),
                directory: true,
            },
        ];
        if roots.gate.is_some() {
            writable.push(Grant {
                path: gate.clone(),
                directory: false,
            });
        }
        for root in roots.git_writes {
            writable.push(Grant {
                path: root,
                directory: true,
            });
        }
        #[cfg(target_os = "macos")]
        let mut adjacent_prefixes = Vec::new();
        #[cfg(not(target_os = "macos"))]
        let adjacent_prefixes = Vec::new();
        if roots.provider {
            let operator_home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or("operator home unavailable")?;
            if !operator_home.is_absolute() {
                return Err("operator home is relative".into());
            }
            let directory = operator_home.join(".claude");
            std::fs::create_dir_all(&directory)?;
            writable.push(Grant {
                path: directory,
                directory: true,
            });
            let configuration = operator_home.join(".claude.json");
            #[cfg(target_os = "macos")]
            adjacent_prefixes.push(configuration);
            #[cfg(target_os = "linux")]
            writable.push(Grant {
                path: configuration,
                directory: false,
            });
        }
        let harness = crate::home::Layout::new(roots.home).harness_dir();
        let mut readable = vec![roots.exchange.into()];
        if harness.is_dir() {
            readable.push(harness);
        }
        let common_path = if roots.target.join(".git").exists() {
            Some(statecraft_run::trusted_git::common(roots.target)?)
        } else {
            None
        };
        let mut readonly = vec![roots.target.to_path_buf()];
        readonly.extend(common_path.iter().cloned());
        let mut boundary = Prepared::prepare(Policy {
            inaccessible: vec![roots.home.into(), roots.records.into()],
            readonly,
            readable,
            writable,
            adjacent_prefixes,
        })?;
        for program in [executable, Path::new("/bin/sh"), Path::new("/usr/bin/git")] {
            boundary.resolve(program, path)?;
        }
        #[cfg(target_os = "macos")]
        boundary.resolve(Path::new("/usr/bin/sandbox-exec"), path)?;
        let probe = Probe {
            home: home_sentinel.path().into(),
            records: records_sentinel.path().into(),
            target: target_sentinel.path().into(),
            workspace: roots.workspace.join(".statecraft-boundary-probe"),
            gate,
            unix_socket: socket_path,
            loopback_port: listener.local_addr()?.port(),
            supervisor: std::process::id(),
            protected_fd: protected.as_raw_fd(),
        };
        use std::os::unix::fs::OpenOptionsExt;
        let gate = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&probe.gate)?;
        let length = gate.metadata()?.len();
        let tested = boundary.self_test(executable, &probe);
        gate.set_len(length)?;
        tested?;
        let variables = ["TMPDIR", "TMP", "TEMP"]
            .into_iter()
            .map(|name| (name.into(), scratch.path().display().to_string()))
            .collect();
        let exchange = DataRoot::open(roots.exchange)?;
        let common = common_path.as_deref().map(DataRoot::open).transpose()?;
        Ok(Admission {
            boundary: Arc::new(boundary),
            variables,
            exchange,
            common,
            scratch,
            home_sentinel,
            records_sentinel,
            target_sentinel,
            socket,
            socket_directory,
            gate,
            listener,
            protected,
            probe,
        })
    }
    prepare(roots, executable, path).map_err(|e| e.to_string())
}

/// Receive and verify the child's object stream before reading any commit.
/// The producer alone reads private objects, under the admitted OS policy.
pub fn import_objects(
    admission: &Admission,
    target: &Path,
    workspace: &Path,
    run_id: &str,
    variables: &BTreeMap<String, String>,
) -> Result<serde_json::Value, String> {
    admission.retest(&std::env::current_exe().map_err(|e| e.to_string())?)?;
    let git = statecraft_run::trusted_git::program().map_err(|e| e.to_string())?;
    let path = variables.get("PATH").map(String::as_str).unwrap_or("");
    admission
        .boundary
        .resolve(&git, path)
        .map_err(|e| e.to_string())?;
    let branch = format!("refs/heads/statecraft/{run_id}/work");
    let reference = admission
        .common
        .as_ref()
        .ok_or("object import requires a held common Git directory")?
        .read(Path::new(&branch), 128)
        .map_err(|e| e.to_string())?;
    let head = String::from_utf8(reference).map_err(|e| e.to_string())?;
    let head = head.trim();
    if !matches!(head.len(), 40 | 64) || !head.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("private branch does not contain an object identity".into());
    }
    let common = statecraft_run::trusted_git::common(target).map_err(|e| e.to_string())?;
    let common_arg = format!("--git-dir={}", common.display());
    let arguments = [
        &*common_arg,
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.fsmonitor=false",
        "pack-objects",
        "--stdout",
        "--revs",
    ];
    let mut producer_environment = variables.clone();
    producer_environment.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
    producer_environment.insert("GIT_CONFIG_SYSTEM".into(), "/dev/null".into());
    producer_environment.insert("GIT_CONFIG_GLOBAL".into(), "/dev/null".into());
    let produced = statecraft_adapter::supervisor::capture_confined(
        &git,
        &arguments,
        workspace,
        &producer_environment,
        format!("{head}\n").as_bytes(),
        std::time::Duration::from_secs(120),
        &admission.boundary,
    )
    .map_err(|e| e.to_string())?;
    if produced.code != Some(0) || produced.timed_out || produced.surviving_processes.is_some() {
        return Err(format!(
            "confined object producer refused: {}",
            String::from_utf8_lossy(&produced.stderr)
        ));
    }
    // Only bounded stream bytes cross the boundary. The receiver gets the
    // trusted common directory and none of the producer's environment.
    let receiver = statecraft_run::trusted_git::command(target).map_err(|e| e.to_string())?;
    let mut arguments: Vec<String> = receiver
        .get_args()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    arguments.extend(["index-pack".into(), "--stdin".into(), "--strict".into()]);
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let environment: BTreeMap<String, String> = receiver
        .get_envs()
        .filter_map(|(key, value)| {
            value.map(|value| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
        })
        .collect();
    let received = statecraft_adapter::supervisor::capture(
        &git,
        &arguments,
        target,
        &environment,
        &produced.stdout,
        std::time::Duration::from_secs(120),
    )
    .map_err(|e| e.to_string())?;
    if received.code != Some(0) || received.timed_out || received.surviving_processes.is_some() {
        return Err(format!(
            "object receiver refused: {}",
            String::from_utf8_lossy(&received.stderr)
        ));
    }
    statecraft_run::trusted_git::output(
        target,
        &[
            "fsck",
            "--strict",
            "--connectivity-only",
            "--no-reflogs",
            head,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(
        serde_json::json!({ "head": head, "import": "rehashed-and-connectivity-checked", "objectsRetained": true }),
    )
}

/// Remove references outside the run's sole retained work branch at conclusion.
/// Names come only from the trusted common directory.
pub fn remove_extra_refs(admission: &Admission, run_id: &str) -> Result<Vec<String>, String> {
    let common = admission
        .common
        .as_ref()
        .ok_or("private reference cleanup requires held Git directory")?;
    let prefix = format!("refs/heads/statecraft/{run_id}");
    let references = common
        .directory(Path::new(&prefix))
        .map_err(|e| e.to_string())?;
    let mut removed = Vec::new();
    for (relative, _) in references.snapshot().map_err(|e| e.to_string())? {
        if relative == Path::new("work") {
            continue;
        }
        references.unlink(&relative).map_err(|e| e.to_string())?;
        removed.push(format!("{prefix}/{}", relative.display()));
    }
    Ok(removed)
}
