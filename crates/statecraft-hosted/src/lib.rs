//! The hosted client (spec 038): the half of the Statecraft platform's thin
//! path that obtains, holds and presents a token.
//!
//! This is the only crate in the workspace allowed to speak HTTP to a cell
//! (spec 038 section 2, O-5). It knows one cell at a time, named by its origin
//! (section 3.1), discovers that cell's issuer from the cell itself, logs in
//! with the OAuth device authorization grant (section 3.2), keeps the
//! credential owner-only under the product home (section 3.3), and makes
//! bearer requests by the rules of section 3.4. It submits evidence and
//! retrieves a decision bundle as bytes; verifying the bundle is the envelope
//! crate's, which is pure.
//!
//! **No token is ever disclosed.** No access token, refresh token or device
//! code appears in an error, a `Debug` rendering, a log line or anything this
//! crate returns for display. Errors name a request by its method and route
//! template, never by a URL.

pub mod api;
pub mod credential;
pub mod declaration;
pub mod discovery;
pub mod grant;
pub mod http;
pub mod origin;

pub use origin::Origin;

/// Why a hosted operation did not complete, by the family exit contract.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// A precondition the operator or the cell supplies is not met (exit 2).
    #[error("{0}")]
    Refused(String),
    /// No credential, or the cell asked for one (exit 2).
    #[error("log in to {0}")]
    LoginRequired(String),
    /// The cell refused for a scope the token lacks; final (exit 2).
    #[error("the cell refused: the token lacks the scope {0}")]
    InsufficientScope(String),
    /// A transport failure or a server error (exit 4). Named by method and
    /// route template, never by a URL that could carry a credential.
    #[error("{method} {route} failed: {why}")]
    Failed {
        /// The HTTP method.
        method: &'static str,
        /// The route template.
        route: &'static str,
        /// What went wrong, without the URL.
        why: String,
    },
}

impl Error {
    /// The family exit code (spec 007): 2 refused, 4 failed.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Failed { .. } => 4,
            _ => 2,
        }
    }
}

/// The time, in whole seconds since the Unix epoch, and the one way this
/// crate waits. Injected so a test can drive a device grant without waiting.
pub trait Clock {
    /// Now.
    fn now(&self) -> u64;
    /// Wait `seconds`.
    fn sleep(&self, seconds: u64);
}

/// The system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    fn sleep(&self, seconds: u64) {
        std::thread::sleep(std::time::Duration::from_secs(seconds));
    }
}

/// Lowercase hexadecimal SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
