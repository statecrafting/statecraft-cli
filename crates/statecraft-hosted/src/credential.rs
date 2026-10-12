//! Holding a credential (spec 038 section 3.3).
//!
//! One file per cell under the product home, keyed by the origin's digest,
//! mode `0600` in a `0700` directory, written by a rename so a reader never
//! sees half of one. Never inside a repository or a worktree, on any
//! platform (O-2). A file another user could read is refused, not used.

use crate::{Error, Origin};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A held credential.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Credential {
    /// The cell.
    pub origin: String,
    /// The access token.
    pub access_token: String,
    /// The refresh token, when the issuer gave one.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// `expires_in` as the token response carried it, in seconds.
    pub expires_in: u64,
    /// When the token response arrived, in seconds since the Unix epoch.
    pub issued_at: u64,
    /// The scope the issuer granted, when it said.
    #[serde(default)]
    pub scope: Option<String>,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("origin", &self.origin)
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_in", &self.expires_in)
            .field("issued_at", &self.issued_at)
            .field("scope", &self.scope)
            .finish()
    }
}

impl Credential {
    /// When the access token expires.
    pub fn expires_at(&self) -> u64 {
        self.issued_at.saturating_add(self.expires_in)
    }

    /// The first instant a refresh token may be presented: Rauthy sets its
    /// `nbf` to `issued_at + lifetime - 60`, and presenting it earlier
    /// invalidates it and every linked session (Rahi 038 D-13).
    pub fn refresh_not_before(&self) -> u64 {
        self.expires_at().saturating_sub(60)
    }
}

/// The credential directory under a product home.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// The store under `home` (the product home).
    pub fn new(home: &Path) -> Store {
        Store {
            dir: home.join("hosted").join("credentials"),
        }
    }

    /// The file `origin`'s credential is held in.
    pub fn path(&self, origin: &Origin) -> PathBuf {
        self.dir.join(format!("{}.json", origin.digest()))
    }

    /// Write `credential` for `origin`, owner-only, by a rename.
    pub fn save(&self, origin: &Origin, credential: &Credential) -> Result<(), Error> {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let io =
            |e: std::io::Error| Error::Refused(format!("the credential cannot be stored: {e}"));
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)
            .map_err(io)?;
        std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700)).map_err(io)?;
        let bytes = serde_json::to_vec(credential).expect("serializable");
        // A uniquely named staging file, created owner-only, so concurrent
        // saves never share one; the rename replaces the held credential.
        let mut staged = tempfile::Builder::new()
            .prefix(&format!(".{}.", origin.digest()))
            .suffix(".tmp")
            .permissions(std::fs::Permissions::from_mode(0o600))
            .tempfile_in(&self.dir)
            .map_err(io)?;
        staged.write_all(&bytes).map_err(io)?;
        staged.as_file().sync_all().map_err(io)?;
        staged
            .persist(self.path(origin))
            .map(|_| ())
            .map_err(|e| io(e.error))
    }

    /// The credential held for `origin`, if any. A file another user could
    /// read or write is refused.
    pub fn load(&self, origin: &Origin) -> Result<Option<Credential>, Error> {
        use std::os::unix::fs::PermissionsExt;
        let path = self.path(origin);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(Error::Refused(format!(
                    "the credential cannot be read: {e}"
                )));
            }
        };
        if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
            return Err(Error::Refused(format!(
                "the credential file for {origin} is not an owner-only regular file; log in again"
            )));
        }
        let bytes = std::fs::read(&path)
            .map_err(|e| Error::Refused(format!("the credential cannot be read: {e}")))?;
        let c: Credential = serde_json::from_slice(&bytes).map_err(|_| {
            Error::Refused(format!(
                "the credential for {origin} is malformed; log in again"
            ))
        })?;
        if c.origin != origin.as_str() {
            return Err(Error::Refused(format!(
                "the credential file for {origin} names another cell; log in again"
            )));
        }
        Ok(Some(c))
    }

    /// Remove `origin`'s credential. Whether there was one.
    pub fn delete(&self, origin: &Origin) -> Result<bool, Error> {
        match std::fs::remove_file(self.path(origin)) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(Error::Refused(format!(
                "the credential cannot be removed: {e}"
            ))),
        }
    }
}
