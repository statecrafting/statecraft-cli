//! The provider token (spec 042 section 3.2).
//!
//! The operator creates a long-lived token with the provider's own command
//! (`claude setup-token`) and stores it here once. The supervisor reads it at
//! launch and places it in the constructed environment, and nowhere else: not
//! in an argument, a record, a log, an answer or a file the child can read.
//! It lives under the product home, which the confinement keeps from the
//! child, so the child holds only its environment copy.
//!
//! A confined provider launch without a token refuses with exit 2. The
//! product never falls back to the operator's own login, which a per-run
//! configuration directory cannot reach and which a refresh by the child
//! could rotate.

use crate::home::Layout;
use std::path::Path;

/// The environment name the provider reads its token from.
pub const VARIABLE: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// The command an operator runs when the token is absent.
pub const SET_COMMAND: &str = "statecraft-cli provider token set";

/// The largest token accepted, in bytes.
const MAX_TOKEN: usize = 4096;

/// Why a token could not be stored or used. Never carries the token.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TokenError {
    /// No token is stored.
    #[error(
        "no provider token is stored; create one with `claude setup-token` and store it with `{SET_COMMAND}` (it reads the token from standard input)"
    )]
    Absent,
    /// A token is already stored; `set` does not replace one.
    #[error("a provider token is already stored; run `statecraft-cli provider token clear` first")]
    Exists,
    /// The input is not a token.
    #[error("the input is not a provider token: {0}")]
    Malformed(&'static str),
    /// The stored file is not owner-only or not a regular file.
    #[error(
        "the stored provider token is not an owner-only regular file; clear it and set it again"
    )]
    Unsafe,
    /// An I/O failure, without the path's contents.
    #[error("the provider token store failed: {0}")]
    Io(String),
}

/// The token's text, trimmed of surrounding whitespace, validated.
fn validate(input: &[u8]) -> Result<String, TokenError> {
    let text = std::str::from_utf8(input).map_err(|_| TokenError::Malformed("not UTF-8"))?;
    let token = text.trim();
    if token.is_empty() {
        return Err(TokenError::Malformed("empty"));
    }
    if token.len() > MAX_TOKEN {
        return Err(TokenError::Malformed("too long"));
    }
    if !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(TokenError::Malformed(
            "contains whitespace or a non-printable byte",
        ));
    }
    Ok(token.to_string())
}

/// Store `input` as the provider token: owner-only, in an owner-only
/// directory, by a no-replace rename. An existing token is not replaced.
pub fn set(home: &Path, input: &[u8]) -> Result<(), TokenError> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    let token = validate(input)?;
    let target = Layout::new(home).provider_token_file();
    let dir = target.parent().expect("the token file has a parent");
    let io = |e: std::io::Error| TokenError::Io(e.kind().to_string());
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(io)?;
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(TokenError::Exists);
    }
    let staged = tempfile::Builder::new()
        .prefix(".claude-code-token.")
        .permissions(std::fs::Permissions::from_mode(0o600))
        .tempfile_in(dir)
        .map_err(io)?;
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(staged.path())
            .map_err(io)?;
        f.write_all(token.as_bytes()).map_err(io)?;
        f.sync_all().map_err(io)?;
    }
    staged.persist_noclobber(&target).map(|_| ()).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            TokenError::Exists
        } else {
            io(e.error)
        }
    })
}

/// Remove the stored token. Whether there was one.
pub fn clear(home: &Path) -> Result<bool, TokenError> {
    match std::fs::remove_file(Layout::new(home).provider_token_file()) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(TokenError::Io(e.kind().to_string())),
    }
}

/// Whether a token is stored, without reading it.
pub fn present(home: &Path) -> bool {
    std::fs::symlink_metadata(Layout::new(home).provider_token_file()).is_ok_and(|m| m.is_file())
}

/// The stored token. Absent, not owner-only, or malformed refuses.
pub fn read(home: &Path) -> Result<String, TokenError> {
    use std::io::Read;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let path = Layout::new(home).provider_token_file();
    let mut f = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(TokenError::Absent),
        Err(_) => return Err(TokenError::Unsafe),
    };
    let meta = f
        .metadata()
        .map_err(|e| TokenError::Io(e.kind().to_string()))?;
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
        return Err(TokenError::Unsafe);
    }
    let mut bytes = Vec::new();
    f.by_ref()
        .take(MAX_TOKEN as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| TokenError::Io(e.kind().to_string()))?;
    validate(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_token_is_stored_owner_only_once_and_read_back() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(read(home.path()), Err(TokenError::Absent));
        assert!(!present(home.path()));
        set(home.path(), b"  sk-ant-oat01-example\n").unwrap();
        assert!(present(home.path()));
        let path = Layout::new(home.path()).provider_token_file();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(read(home.path()).unwrap(), "sk-ant-oat01-example");
        assert_eq!(set(home.path(), b"another"), Err(TokenError::Exists));
        assert_eq!(read(home.path()).unwrap(), "sk-ant-oat01-example");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(read(home.path()), Err(TokenError::Unsafe));
        assert!(clear(home.path()).unwrap());
        assert!(!clear(home.path()).unwrap());
        assert_eq!(read(home.path()), Err(TokenError::Absent));
    }

    #[test]
    fn malformed_input_is_refused_and_no_error_carries_the_input() {
        let home = tempfile::tempdir().unwrap();
        for bad in [&b""[..], b"   \n", b"two words", b"tab\there", b"\xff\xfe"] {
            let e = set(home.path(), bad).unwrap_err();
            assert!(matches!(e, TokenError::Malformed(_)), "{bad:?}");
        }
        set(home.path(), b"secret-token-value").unwrap();
        for e in [
            TokenError::Absent,
            TokenError::Exists,
            TokenError::Unsafe,
            set(home.path(), b"secret-token-value").unwrap_err(),
        ] {
            assert!(!e.to_string().contains("secret-token-value"));
        }
        // A link in place of the token is not followed.
        let path = Layout::new(home.path()).provider_token_file();
        std::fs::remove_file(&path).unwrap();
        let elsewhere = home.path().join("elsewhere");
        std::fs::write(&elsewhere, b"linked-token").unwrap();
        std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
        assert_eq!(read(home.path()), Err(TokenError::Unsafe));
    }
}
