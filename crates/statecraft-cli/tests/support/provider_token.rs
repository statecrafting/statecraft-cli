//! A fixture provider token for suites that launch a fake provider (spec
//! 042 section 3.2): a confined launch refuses without one, so a fixture
//! product home is given one, owner-only, exactly where `provider token set`
//! would put it. The value is a fixture's and authenticates nothing.

#![allow(dead_code)]

use std::path::Path;

/// The fixture token's bytes, which a suite can scan its outputs for.
pub const FIXTURE_TOKEN: &str = "fixture-provider-token-0042";

/// Store the fixture token under the product home `home`, once, and return
/// `home` so the call can wrap the argument that names it.
pub fn with_token<P: AsRef<Path>>(home: P) -> P {
    let file = home.as_ref().join("credentials").join("claude-code-token");
    if !file.exists() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(file.parent().unwrap())
            .unwrap();
        std::fs::write(&file, FIXTURE_TOKEN).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    home
}
