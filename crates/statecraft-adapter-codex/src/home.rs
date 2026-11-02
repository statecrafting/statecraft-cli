//! Section 3.3: each attempt's own `CODEX_HOME`.
//!
//! Codex reads its configuration, hooks and credentials from `CODEX_HOME`
//! (default `~/.codex`). The supervisor creates a fresh directory, mode `0700`,
//! inside the attempt's scratch root (spec 042 section 3.1 rule 1), writes the
//! hooks document and the credential the supervisor holds into it, and adds
//! exactly one name to the constructed environment. The user's own directory
//! is named only to refuse a scratch root inside it: it is never read for
//! configuration and never written (obligation I-2). The directory is removed
//! when the value is dropped (spec 042 section 3.1 rule 3).
//!
//! A launch with no credential refuses before anything is created, as spec
//! 042 section 3.2 rule 3 does for the first adapter; there is no fallback to
//! the user's own login.

use statecraft_adapter::environment::Blueprint;
use statecraft_secret::SecretString;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The one name added to the constructed environment.
pub const CODEX_HOME: &str = "CODEX_HOME";

/// The hooks file Codex reads from `CODEX_HOME`.
pub const HOOKS_FILE: &str = "hooks.json";

/// The credential file Codex reads from `CODEX_HOME`.
pub const CREDENTIAL_FILE: &str = "auth.json";

/// The directory name created inside the scratch root.
pub const DIRECTORY: &str = "codex-home";

/// Why a per-run `CODEX_HOME` was not created. Each carries a stable code.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    /// No credential was supplied.
    #[error(
        "codex-credential-absent: no Codex credential is provisioned; the launch starts nothing"
    )]
    NoCredential,
    /// The scratch root lies inside the user's own Codex directory.
    #[error(
        "codex-home-inside-user-directory: {scratch} is inside the user's Codex directory {user}, \
         which this adapter never writes"
    )]
    InsideUserDirectory {
        /// The scratch root.
        scratch: PathBuf,
        /// The user's directory.
        user: PathBuf,
    },
    /// Creating or writing the directory failed.
    #[error("codex-home-unwritable: {0}")]
    Io(#[from] std::io::Error),
}

impl Refused {
    /// The stable code, the message's first word.
    pub fn code(&self) -> &'static str {
        match self {
            Refused::NoCredential => "codex-credential-absent",
            Refused::InsideUserDirectory { .. } => "codex-home-inside-user-directory",
            Refused::Io(_) => "codex-home-unwritable",
        }
    }
}

/// The user's own Codex directory under a home directory. Named, never read.
pub fn user_directory(home: &Path) -> PathBuf {
    home.join(".codex")
}

/// One attempt's `CODEX_HOME`. Removed on drop.
#[derive(Debug)]
pub struct RunHome {
    path: PathBuf,
}

impl RunHome {
    /// Create the directory and write the hooks document and the credential.
    ///
    /// `credential` is the provisioned Codex credential document, written
    /// verbatim to [`CREDENTIAL_FILE`] with mode `0600`; it appears in no
    /// argument, record or other file.
    ///
    /// # Errors
    ///
    /// [`Refused`], before anything is created when the credential is absent
    /// or the scratch root is inside `user_directory`.
    pub fn create(
        scratch_root: &Path,
        user_directory: &Path,
        hooks: &serde_json::Value,
        credential: Option<&SecretString>,
    ) -> Result<Self, Refused> {
        let credential = credential.ok_or(Refused::NoCredential)?;
        let scratch = scratch_root.canonicalize()?;
        let user = user_directory
            .canonicalize()
            .unwrap_or_else(|_| user_directory.to_path_buf());
        if scratch.starts_with(&user) {
            return Err(Refused::InsideUserDirectory { scratch, user });
        }
        let path = scratch.join(DIRECTORY);
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        // Fresh: an existing directory is refused, never reused.
        builder.create(&path)?;
        let home = RunHome { path };
        home.write_new(
            HOOKS_FILE,
            &serde_json::to_vec_pretty(hooks).map_err(std::io::Error::other)?,
        )?;
        home.write_new(CREDENTIAL_FILE, credential.expose().as_bytes())?;
        Ok(home)
    }

    fn write_new(&self, name: &str, bytes: &[u8]) -> std::io::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(self.path.join(name))?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The blueprint with [`CODEX_HOME`] added, and nothing else.
    #[must_use]
    pub fn naming_in(&self, blueprint: Blueprint) -> Blueprint {
        blueprint.allowing(CODEX_HOME, &self.path.to_string_lossy())
    }
}

impl Drop for RunHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
