//! One secret type (spec 046).
//!
//! [`SecretString`] owns bearer material: the provider token spec 042 stores
//! and the hosted access and refresh tokens spec 038 holds. Its `Debug` and
//! `Display` render `[redacted]`, never the value, its length or a prefix
//! (section 3.1). Its bytes are overwritten with zeros when it is dropped,
//! including on an early return and an unwinding panic (section 3.2). The
//! value is read through [`SecretString::expose`] and nothing else.
//!
//! The type implements `Deserialize`, so a credential file can be read into
//! it, and does **not** implement `Serialize`: writing a secret is an explicit
//! expose-and-write at the one site that persists it.
//!
//! ```compile_fail
//! fn write<T: serde::Serialize>(_: &T) {}
//! write(&statecraft_secret::SecretString::from("a secret"));
//! ```
//!
//! Equality is not implemented on the value, so a comparison cannot leak it
//! through timing or a test assertion.
//!
//! ```compile_fail
//! let a = statecraft_secret::SecretString::from("a secret");
//! let _same = a == a.clone();
//! ```
//!
//! What this does not cover (section 3.2): copies the operating system or an
//! allocator made before the value reached the type, a swapped-out page, or
//! bytes a child process received.

#![forbid(unsafe_code)]

use serde::Deserialize;
use zeroize::Zeroize;

/// What `Debug` and `Display` render in place of a value.
pub const REDACTED: &str = "[redacted]";

/// A secret string: redacted when shown, zeroed when dropped, exposed only by
/// [`SecretString::expose`].
///
/// `Clone` is implemented so a holder may keep a copy; each copy zeroes its
/// own bytes on drop.
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    /// Take ownership of `value`. The caller's `String` moves in without a
    /// copy, so no second buffer holding the value is left behind here.
    pub fn new(value: String) -> SecretString {
        SecretString(value)
    }

    /// The value. The only accessor: every use of a secret is a call to this
    /// by name, so a search for `expose` finds each one.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> SecretString {
        SecretString::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> SecretString {
        SecretString::new(value.to_string())
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(REDACTED)
    }
}

impl std::fmt::Display for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(REDACTED)
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<SecretString, D::Error> {
        String::deserialize(deserializer).map(SecretString::new)
    }
}

impl Drop for SecretString {
    /// Section 3.2: the buffer is overwritten with zeros, through `zeroize`
    /// (which the compiler may not elide), before it is freed. The buffer is
    /// taken as bytes so its length survives the first pass; the test hook
    /// observes it there, and the second pass clears the spare capacity.
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.as_mut_slice().zeroize();
        #[cfg(test)]
        hook::observe(&bytes);
        bytes.zeroize();
    }
}

/// The test hook spec 046 V-1 names: the zeroed buffer each drop leaves,
/// recorded for the dropping thread so parallel tests do not see each other's.
#[cfg(test)]
mod hook {
    use std::cell::RefCell;

    thread_local! {
        static LAST: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    }

    pub fn observe(bytes: &[u8]) {
        LAST.with(|last| *last.borrow_mut() = Some(bytes.to_vec()));
    }

    /// The buffer the most recent drop on this thread left, taken.
    pub fn take() -> Option<Vec<u8>> {
        LAST.with(|last| last.borrow_mut().take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALUE: &str = "sk-ant-oat01-a-secret-value";

    #[test]
    fn debug_and_display_render_redacted_and_never_the_value() {
        let s = SecretString::from(VALUE);
        for shown in [format!("{s:?}"), format!("{s:#?}"), format!("{s}")] {
            assert_eq!(shown, REDACTED);
            assert!(!shown.contains(VALUE));
            assert!(!shown.contains(&VALUE.len().to_string()));
        }
        // Inside a derived Debug the type redacts by itself.
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Holder {
            token: SecretString,
            refresh: Option<SecretString>,
        }
        let shown = format!(
            "{:?}",
            Holder {
                token: SecretString::from(VALUE),
                refresh: Some(SecretString::from("other")),
            }
        );
        assert_eq!(
            shown,
            "Holder { token: [redacted], refresh: Some([redacted]) }"
        );
    }

    #[test]
    fn expose_is_the_accessor_and_deserialize_reads_a_file_value() {
        let s = SecretString::from(VALUE);
        assert_eq!(s.expose(), VALUE);
        let s: SecretString = serde_json::from_str(&format!("{VALUE:?}")).unwrap();
        assert_eq!(s.expose(), VALUE);
        #[derive(Deserialize)]
        struct Holder {
            token: SecretString,
            #[serde(default)]
            refresh: Option<SecretString>,
        }
        let h: Holder = serde_json::from_str(&format!(r#"{{"token": {VALUE:?}}}"#)).unwrap();
        assert_eq!(h.token.expose(), VALUE);
        assert!(h.refresh.is_none());
        assert!(serde_json::from_str::<SecretString>("17").is_err());
    }

    #[test]
    fn dropping_zeroes_the_bytes_on_a_normal_path_an_early_return_and_a_panic() {
        hook::take();
        // The normal path.
        {
            let _s = SecretString::from(VALUE);
            assert!(hook::take().is_none(), "observed before the drop");
        }
        assert_eq!(hook::take(), Some(vec![0u8; VALUE.len()]));
        // An early return.
        fn early(flag: bool) -> Option<usize> {
            let s = SecretString::from(VALUE);
            if flag {
                return None;
            }
            Some(s.expose().len())
        }
        assert_eq!(early(true), None);
        assert_eq!(hook::take(), Some(vec![0u8; VALUE.len()]));
        // A panic that unwinds.
        let unwound = std::panic::catch_unwind(|| {
            let _s = SecretString::from(VALUE);
            panic!("unwinding past a secret");
        });
        assert!(unwound.is_err());
        assert_eq!(hook::take(), Some(vec![0u8; VALUE.len()]));
    }

    #[test]
    fn each_clone_zeroes_its_own_bytes() {
        hook::take();
        let a = SecretString::from(VALUE);
        let b = a.clone();
        assert_eq!(b.expose(), VALUE);
        drop(b);
        assert_eq!(hook::take(), Some(vec![0u8; VALUE.len()]));
        // The original still holds its value after a copy was zeroed.
        assert_eq!(a.expose(), VALUE);
        drop(a);
        assert_eq!(hook::take(), Some(vec![0u8; VALUE.len()]));
    }
}
