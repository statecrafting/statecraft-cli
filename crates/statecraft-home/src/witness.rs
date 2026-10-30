//! The witness sidecar's place in the home (spec 013 sections 3.1 and 3.6.2).
//!
//! Spec 002 claims this crate; spec 013, which is approved, reaches it
//! through its additive `extends` edge. The producer is found at one fixed
//! path and verified by digest only; nothing here downloads or installs it.
//! The sidecar's own confinement is admitted here, with live sentinels, the
//! way [`crate::confinement::admit`] admits the child's.

use statecraft_adapter::boundary::{Grant, Policy, Prepared, Probe};
use statecraft_adapter::witness::ledger::{
    ADOPTED, ProducerRefusal, VerifiedProducer, current_target, verify,
};
use std::fs::File;
use std::net::{SocketAddr, TcpListener};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

/// The sidecar area's name inside the attempt's exchange directory.
pub const AREA: &str = "witness";

/// The directory the sidecar creates for its output; it must not exist.
pub const OUTPUT: &str = "capture";

/// The sidecar's own temporary directory inside its area.
pub const TEMPORARY: &str = "tmp";

/// Where the operator places the adopted producer for `target`:
/// `<home>/producers/<name>/<version>/<target>/<name>`.
#[must_use]
pub fn producer_path(home: &Path, target: &str) -> PathBuf {
    home.join("producers")
        .join(ADOPTED.name)
        .join(ADOPTED.version)
        .join(target)
        .join(ADOPTED.name)
}

/// The adopted producer for this platform, verified at its one path.
///
/// # Errors
///
/// A [`ProducerRefusal`]: the platform is not qualified, the file is absent
/// or not a regular file, or its digest is not the ledger's. There is no
/// fallback to another path or version.
pub fn locate(home: &Path) -> Result<VerifiedProducer, ProducerRefusal> {
    let Some(target) = current_target() else {
        return Err(ProducerRefusal::UnsupportedPlatform {
            version: ADOPTED.version,
            target: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        });
    };
    verify(&ADOPTED, target, &producer_path(home, target))
}

/// Supervisor-owned paths the sidecar must not reach, and its own area.
pub struct SidecarRoots<'a> {
    /// Protected product home.
    pub home: &'a Path,
    /// Protected launch records.
    pub records: &'a Path,
    /// The operator checkout.
    pub target: &'a Path,
    /// The child's workspace.
    pub workspace: &'a Path,
    /// The attempt's sidecar area: created here, private, and the only
    /// writable root.
    pub area: &'a Path,
    /// The verified producer; its directory alone is readable outside the area.
    pub producer: &'a VerifiedProducer,
}

/// The sidecar's admitted confinement and the live sentinels that make its
/// self-test meaningful. Held until the sidecar exits.
pub struct SidecarAdmission {
    /// The admitted boundary, given to the sidecar's spawn.
    pub boundary: Prepared,
    /// The directory the sidecar is told to create.
    pub output: PathBuf,
    /// The sidecar's temporary directory.
    pub temporary: PathBuf,
    _home_sentinel: tempfile::NamedTempFile,
    _records_sentinel: tempfile::NamedTempFile,
    _target_sentinel: tempfile::NamedTempFile,
    _socket: UnixListener,
    _socket_directory: tempfile::TempDir,
    _listener: TcpListener,
    _protected: File,
}

/// Admit the sidecar's confinement for `endpoint`, connecting out only to
/// `outbound`, and prove it with the fixed probe (section 3.6.2). A failure
/// refuses witness startup; nothing falls back to in-process capture.
///
/// # Errors
///
/// What could not be prepared, or the self-test's refusal.
pub fn admit_sidecar(
    roots: &SidecarRoots<'_>,
    executable: &Path,
    endpoint: SocketAddr,
    outbound: &[u16],
) -> Result<SidecarAdmission, String> {
    fn prepare(
        roots: &SidecarRoots<'_>,
        executable: &Path,
        endpoint: SocketAddr,
        outbound: &[u16],
    ) -> Result<SidecarAdmission, Box<dyn std::error::Error>> {
        use std::os::unix::fs::DirBuilderExt;
        let mut private = std::fs::DirBuilder::new();
        private.mode(0o700);
        private.create(roots.area)?;
        let temporary = roots.area.join(TEMPORARY);
        private.create(&temporary)?;
        let producer_dir = roots
            .producer
            .path
            .parent()
            .ok_or("the producer has no directory")?
            .to_path_buf();
        // Inside the home, under one fixed entry, so the protected roots
        // name no per-launch file.
        let boundary_dir = roots.home.join(crate::home::BOUNDARY_DIR);
        std::fs::create_dir_all(&boundary_dir)?;
        let home_sentinel = tempfile::NamedTempFile::new_in(&boundary_dir)?;
        let records_sentinel = tempfile::NamedTempFile::new_in(roots.records)?;
        let target_sentinel = tempfile::NamedTempFile::new_in(roots.target)?;
        let protected = File::open(home_sentinel.path())?;
        let socket_directory = tempfile::Builder::new()
            .prefix("probe-")
            .tempdir_in(&boundary_dir)?;
        let socket_path = socket_directory.path().join("socket");
        let socket = UnixListener::bind(&socket_path)?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let mut boundary = Prepared::prepare_sidecar(
            Policy {
                // Every entry of the home but the operator's (spec 064
                // section 3.1, amending spec 004 section 3.18 rule 2).
                inaccessible: {
                    let mut protected = crate::home::protected_entries(roots.home)?;
                    protected.extend([
                        roots.records.into(),
                        roots.target.into(),
                        roots.workspace.into(),
                    ]);
                    protected
                },
                // Readable, not read-only: only a readable root is carved out
                // of the inaccessible home.
                readonly: Vec::new(),
                readable: vec![roots.area.into(), producer_dir],
                writable: vec![Grant {
                    path: roots.area.into(),
                    directory: true,
                }],
                adjacent_prefixes: Vec::new(),
            },
            endpoint,
            outbound,
        )?;
        let probe = Probe {
            home: home_sentinel.path().into(),
            records: records_sentinel.path().into(),
            target: target_sentinel.path().into(),
            workspace: roots.area.join("probe"),
            gate: roots.area.join("unused-gate.log"),
            unix_socket: socket_path,
            loopback_port: listener.local_addr()?.port(),
            supervisor: std::process::id(),
            protected_fd: protected.as_raw_fd(),
        };
        boundary.self_test(executable, &probe)?;
        Ok(SidecarAdmission {
            boundary,
            output: roots.area.join(OUTPUT),
            temporary,
            _home_sentinel: home_sentinel,
            _records_sentinel: records_sentinel,
            _target_sentinel: target_sentinel,
            _socket: socket,
            _socket_directory: socket_directory,
            _listener: listener,
            _protected: protected,
        })
    }
    prepare(roots, executable, endpoint, outbound).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_producer_has_one_fixed_path_under_the_home() {
        let home = Path::new("/h");
        assert_eq!(
            producer_path(home, "aarch64-apple-darwin"),
            Path::new("/h/producers/wire-witness/0.1.0/aarch64-apple-darwin/wire-witness")
        );
    }

    #[test]
    fn an_absent_or_foreign_producer_is_refused_and_nothing_else_is_tried() {
        let home = tempfile::tempdir().unwrap();
        let Some(target) = current_target() else {
            assert!(matches!(
                locate(home.path()),
                Err(ProducerRefusal::UnsupportedPlatform { .. })
            ));
            return;
        };
        let refusal = locate(home.path()).unwrap_err();
        if ADOPTED.platform(target).is_none() {
            assert!(matches!(
                refusal,
                ProducerRefusal::UnsupportedPlatform { .. }
            ));
            return;
        }
        assert!(
            matches!(refusal, ProducerRefusal::Unavailable { .. }),
            "{refusal}"
        );
        let path = producer_path(home.path(), target);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not the artifact").unwrap();
        assert!(matches!(
            locate(home.path()).unwrap_err(),
            ProducerRefusal::DigestMismatch { .. }
        ));
    }
}
