//! Linux backend placeholder: every preparation refuses until the Landlock and
//! seccomp backend replaces this file. Nothing launches through the boundary yet.

use super::*;
// The real backend installs its rules through `pre_exec`.
#[allow(unused_imports)]
use super::CommandExt as _;

// Never constructed: preparation refuses before a rule set exists.
#[allow(dead_code)]
pub(super) struct Rules {
    pub(super) identity: Vec<u8>,
}

impl Rules {
    pub(super) fn new(_policy: &Policy) -> Result<Self, Refused> {
        Err(Refused::at(
            "linux-backend",
            "the Landlock backend is not in this build",
        ))
    }

    pub(super) fn apply(&self, _command: &mut Command) {}
}

pub(super) fn probe_extra(_probe: &Probe, _out: &mut BTreeMap<String, bool>) {}
