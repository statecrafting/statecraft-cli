//! Provider-neutral protected evidence confinement, spec 004 section 3.18.
//!
//! An admitted policy is immutable. There is no permissive fallback when a
//! mechanism, an exact root, or the fixed probe cannot be established.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(unsafe_code)]
mod native;

/// Open items remain open even after an operating-system probe passes.
pub const OPEN_ITEMS: &[&str] = &[
    "IX remains open through later unconfined consumers of writable provider configuration",
    "Other macOS Mach services are unmeasured",
    "The gate log is child-attested",
    "Real-provider confined activation requires separate owner authorization",
    "Linux listeners created after the listener check remain an open route",
    "A Linux connected same-user UDP socket is reachable from its peer's address and port",
    "This boundary makes no claim about credentials or publishing",
];

/// An exact writable grant. A file grant never includes its parent's entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// Existing file or directory, resolved before admission.
    pub path: PathBuf,
    /// Whether descendants and directory entries may be changed.
    pub directory: bool,
}

/// All paths are supplied by the supervisor, never by workspace configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    /// Roots with neither read nor write access.
    pub inaccessible: Vec<PathBuf>,
    /// Roots with read-only access, checked for writable aliases at admission.
    pub readonly: Vec<PathBuf>,
    /// Read and execute exceptions within inaccessible roots.
    pub readable: Vec<PathBuf>,
    /// The complete write allowance.
    pub writable: Vec<Grant>,
    /// Literal adjacent configuration-file prefixes, on macOS only.
    pub adjacent_prefixes: Vec<PathBuf>,
}

/// Durable description of the mechanism actually admitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    /// Host platform.
    pub platform: String,
    /// Applied OS mechanism.
    pub mechanism: String,
    /// Digest of the exact compiled profile or ruleset description.
    pub digest: String,
    /// Canonical protected and allowed roots.
    pub policy: Policy,
    /// The fixed probe's observed results.
    pub self_test: BTreeMap<String, bool>,
    /// Residual gaps, never inferred closed from the probe.
    pub open_items: Vec<String>,
}

/// A platform and failed step, suitable for an exit-2 preflight answer.
#[derive(Debug, thiserror::Error)]
#[error("{platform} boundary {step}: {detail}")]
pub struct Refused {
    /// Platform refusing admission.
    pub platform: String,
    /// Failed admission step.
    pub step: String,
    /// Concrete reason.
    pub detail: String,
}

impl Refused {
    pub(crate) fn at(step: &str, detail: impl ToString) -> Self {
        Self {
            platform: std::env::consts::OS.into(),
            step: step.into(),
            detail: detail.to_string(),
        }
    }
}

/// Inputs to the fixed probe. Sentinels are existing ordinary files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    /// Home sentinel: reading and mutation must fail.
    pub home: PathBuf,
    /// Launch-record sentinel: reading and mutation must fail.
    pub records: PathBuf,
    /// Operator checkout sentinel: reading succeeds, mutation fails.
    pub target: PathBuf,
    /// Ordinary workspace file that can be written.
    pub workspace: PathBuf,
    /// Precreated gate log that can be written in place.
    pub gate: PathBuf,
    /// Listening Unix socket outside the domain.
    pub unix_socket: PathBuf,
    /// Listening loopback port, never 443.
    pub loopback_port: u16,
    /// Supervisor PID, used for the signal denial.
    pub supervisor: u32,
    /// Held protected descriptor, checked on Linux.
    pub protected_fd: i32,
}

/// An admitted OS policy, holding its directory handles until all children end.
pub struct Prepared {
    native: native::Prepared,
    record: Record,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.record.fmt(f)
    }
}
impl PartialEq for Prepared {
    fn eq(&self, other: &Self) -> bool {
        self.record == other.record
    }
}
impl Eq for Prepared {}

impl Prepared {
    /// Resolve an exact policy and refuse ambiguous aliases or unsupported roots.
    /// This does not launch the provider or append any attempt.
    pub fn prepare(mut policy: Policy) -> Result<Self, Refused> {
        for roots in [
            &mut policy.inaccessible,
            &mut policy.readonly,
            &mut policy.readable,
        ] {
            for path in roots.iter_mut() {
                *path = fs::canonicalize(&*path).map_err(|e| Refused::at("resolve-root", e))?;
            }
            roots.sort();
            roots.dedup();
        }
        for grant in &mut policy.writable {
            if fs::symlink_metadata(&grant.path)
                .map_err(|e| Refused::at("write-root", e))?
                .file_type()
                .is_symlink()
            {
                return Err(Refused::at("write-root", "writable root is a symlink"));
            }
            grant.path =
                fs::canonicalize(&grant.path).map_err(|e| Refused::at("resolve-write-root", e))?;
            let meta =
                fs::symlink_metadata(&grant.path).map_err(|e| Refused::at("write-root", e))?;
            if grant.directory != meta.is_dir() || (!grant.directory && !meta.is_file()) {
                return Err(Refused::at(
                    "write-root",
                    "grant kind does not match an ordinary file or directory",
                ));
            }
            if policy
                .inaccessible
                .iter()
                .chain(&policy.readonly)
                .any(|p| p.starts_with(&grant.path))
            {
                return Err(Refused::at("write-root", "protected root is also writable"));
            }
        }
        for prefix in &mut policy.adjacent_prefixes {
            let parent = prefix
                .parent()
                .ok_or_else(|| Refused::at("configuration-prefix", "no parent"))?;
            let name = prefix
                .file_name()
                .ok_or_else(|| Refused::at("configuration-prefix", "no file name"))?;
            *prefix = fs::canonicalize(parent)
                .map_err(|e| Refused::at("configuration-prefix", e))?
                .join(name);
        }
        for root in policy.inaccessible.iter().chain(&policy.readonly) {
            check_links(root, &policy)?;
        }
        let native = native::Prepared::new(&policy)?;
        let material = native.identity();
        let record = Record {
            platform: std::env::consts::OS.into(),
            mechanism: native.mechanism().into(),
            digest: Sha256::digest(material)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            policy,
            self_test: BTreeMap::new(),
            open_items: OPEN_ITEMS.iter().map(|s| (*s).into()).collect(),
        };
        Ok(Self { native, record })
    }

    /// Resolve a program only through trusted absolute PATH entries.
    pub fn resolve(&self, program: &Path, path: &str) -> Result<PathBuf, Refused> {
        for entry in std::env::split_paths(path) {
            if !entry.is_absolute() || self.writable_path(&entry)? {
                return Err(Refused::at(
                    "PATH",
                    format!("untrusted entry {}", entry.display()),
                ));
            }
        }
        let resolved = if program.is_absolute() {
            fs::canonicalize(program).map_err(|e| Refused::at("program", e))?
        } else if program.components().count() == 1 {
            std::env::split_paths(path)
                .filter_map(|dir| fs::canonicalize(dir.join(program)).ok())
                .find(|p| p.is_file())
                .ok_or_else(|| {
                    Refused::at("program", format!("{} does not resolve", program.display()))
                })?
        } else {
            return Err(Refused::at("program", "relative program path"));
        };
        if self.writable_path(&resolved)? {
            return Err(Refused::at(
                "program",
                format!("writable program {}", resolved.display()),
            ));
        }
        Ok(resolved)
    }

    fn writable_path(&self, path: &Path) -> Result<bool, Refused> {
        let mut ancestor = path;
        let mut suffix = Vec::new();
        let real = loop {
            match fs::canonicalize(ancestor) {
                Ok(mut real) => {
                    for name in suffix.iter().rev() {
                        real.push(name);
                    }
                    break real;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    suffix.push(
                        ancestor
                            .file_name()
                            .ok_or_else(|| Refused::at("PATH", error))?
                            .to_os_string(),
                    );
                    ancestor = ancestor
                        .parent()
                        .ok_or_else(|| Refused::at("PATH", "no existing ancestor"))?;
                }
                Err(error) => return Err(Refused::at("PATH", error)),
            }
        };
        Ok(self
            .record
            .policy
            .writable
            .iter()
            .any(|g| real.starts_with(&g.path))
            || self.record.policy.adjacent_prefixes.iter().any(|p| {
                real.as_os_str()
                    .as_encoded_bytes()
                    .starts_with(p.as_os_str().as_encoded_bytes())
            }))
    }

    /// Build a command with the same program and arguments inside confinement.
    /// Callers must run `self_test` immediately before each launch.
    pub fn command(&self, program: &Path, args: &[&str]) -> Command {
        self.native.command(program, args)
    }

    /// Run the fixed probe under this exact policy and retain every observation.
    pub fn self_test(&mut self, executable: &Path, probe: &Probe) -> Result<(), Refused> {
        self.record.self_test = self.test(executable, probe)?;
        Ok(())
    }

    /// Repeat the exact fixed probe without changing the admitted record.
    pub fn test(
        &self,
        executable: &Path,
        probe: &Probe,
    ) -> Result<BTreeMap<String, bool>, Refused> {
        let encoded = serde_json::to_string(probe).map_err(|e| Refused::at("probe-input", e))?;
        let output = crate::supervisor::capture_confined(
            executable,
            &["__boundary_probe", &encoded],
            &self
                .record
                .policy
                .writable
                .iter()
                .find(|grant| grant.directory)
                .ok_or_else(|| Refused::at("self-test", "no workspace"))?
                .path,
            &BTreeMap::new(),
            b"",
            std::time::Duration::from_secs(10),
            self,
        )
        .map_err(|e| Refused::at("self-test-spawn", e))?;
        let observations: BTreeMap<String, bool> =
            serde_json::from_slice(&output.stdout).map_err(|e| {
                Refused::at(
                    "self-test-output",
                    format!("{e}; {}", String::from_utf8_lossy(&output.stderr)),
                )
            })?;
        #[allow(unused_mut)]
        let mut required = vec![
            "homeReadDenied",
            "homeWriteDenied",
            "recordsReadDenied",
            "recordsWriteDenied",
            "targetRead",
            "targetWriteDenied",
            "workspaceWrite",
            "workspaceCleanup",
            "workspaceMetadata",
            "targetMetadataDenied",
            "gateWrite",
            "unixDenied",
            "loopbackDenied",
            "supervisorSignalDenied",
            "noInheritedDescriptors",
        ];
        #[cfg(target_os = "linux")]
        required.extend([
            "otherSocketFamilyDenied",
            "ioUringDenied",
            "openByHandleDenied",
            "supervisorFdDenied",
        ]);
        required.sort_unstable();
        if output.code != Some(0)
            || output.timed_out
            || output.surviving_processes.is_some()
            || observations.keys().map(String::as_str).collect::<Vec<_>>() != required
            || observations.values().any(|passed| !passed)
        {
            return Err(Refused::at("self-test", format!("{observations:?}")));
        }
        Ok(observations)
    }

    /// The admitted posture, including all residuals.
    pub fn record(&self) -> &Record {
        &self.record
    }
}

fn check_links(path: &Path, policy: &Policy) -> Result<(), Refused> {
    if policy
        .writable
        .iter()
        .any(|g| g.directory && path.starts_with(&g.path))
    {
        return Ok(());
    }
    let meta = fs::symlink_metadata(path).map_err(|e| Refused::at("alias-scan", e))?;
    #[cfg(unix)]
    if meta.is_file() {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() != 1 {
            return Err(Refused::at(
                "hardlink",
                format!("multiply linked protected file {}", path.display()),
            ));
        }
    }
    if meta.is_dir() {
        for item in fs::read_dir(path).map_err(|e| Refused::at("alias-scan", e))? {
            check_links(
                &item.map_err(|e| Refused::at("alias-scan", e))?.path(),
                policy,
            )?;
        }
    }
    Ok(())
}

/// Execute only the fixed internal probe, never a provider or workspace command.
pub fn probe(encoded: &str) -> i32 {
    match serde_json::from_str::<Probe>(encoded) {
        Ok(probe) => {
            let result = native::probe(&probe);
            println!(
                "{}",
                serde_json::to_string(&result).expect("boolean observations")
            );
            i32::from(result.values().any(|p| !p))
        }
        Err(_) => 3,
    }
}

/// A directory handle held before launch. Reads never follow any component link.
#[derive(Debug)]
pub struct DataRoot(File);

impl DataRoot {
    /// Open an existing directory without following its last component.
    pub fn open(path: &Path) -> io::Result<Self> {
        native::open_directory(path).map(Self)
    }
    /// Open a child directory without following any path component link.
    pub fn directory(&self, relative: &Path) -> io::Result<Self> {
        native::directory_relative(&self.0, relative).map(Self)
    }
    /// Remove a leaf through held directories, without following links.
    pub fn unlink(&self, relative: &Path) -> io::Result<()> {
        native::unlink_relative(&self.0, relative)
    }
    /// Read an ordinary file, relative to this handle, with a strict byte bound.
    pub fn read(&self, relative: &Path, limit: usize) -> io::Result<Vec<u8>> {
        native::read_relative(&self.0, relative, limit)
    }
    /// Read a bounded prefix of an ordinary file and report truncation.
    pub fn read_prefix(&self, relative: &Path, limit: usize) -> io::Result<(Vec<u8>, bool)> {
        native::read_prefix(&self.0, relative, limit)
    }
    /// Snapshot inert file bytes and link text through the held directory.
    /// No workspace metadata is interpreted as configuration.
    pub fn snapshot(&self) -> io::Result<BTreeMap<PathBuf, (u32, Vec<u8>)>> {
        native::snapshot(&self.0)
    }
}
