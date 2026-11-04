//! What section 3.1 spawns, as a value.
//!
//! `codex exec --json --color never --sandbox workspace-write
//! --skip-git-repo-check -C <workspace> -c approval_policy="never" -`, the
//! prompt on standard input. Codex's own workspace sandbox and its native
//! `PreToolUse` governance stay on; nothing here bypasses either. The prompt is
//! never an argument: the trailing `-` tells Codex to read it from standard
//! input, which is where the seam's supervisor writes it.

use std::path::Path;

/// The executable name the constructed environment resolves.
pub const PROGRAM: &str = "codex";

/// A spawn this adapter would make: the program and its argv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The provider executable.
    pub program: String,
    /// Every argument, in order. Never the prompt.
    pub args: Vec<String>,
}

impl Invocation {
    /// Section 3.1's invocation for one workspace.
    pub fn new(program: &str, workspace: &Path) -> Self {
        let workspace = workspace.to_string_lossy().into_owned();
        let args = [
            "exec",
            "--json",
            "--color",
            "never",
            "--sandbox",
            "workspace-write",
            "--skip-git-repo-check",
            "-C",
            &workspace,
            "-c",
            "approval_policy=\"never\"",
            "-",
        ];
        Self {
            program: program.to_string(),
            args: args.iter().map(|a| (*a).to_string()).collect(),
        }
    }

    /// The arguments as the supervisor takes them.
    pub fn args(&self) -> Vec<&str> {
        self.args.iter().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_argv_is_section_3_1s_and_carries_no_prompt() {
        let i = Invocation::new(PROGRAM, Path::new("/w"));
        assert_eq!(
            i.args().join(" "),
            "exec --json --color never --sandbox workspace-write --skip-git-repo-check \
             -C /w -c approval_policy=\"never\" -"
        );
        assert_eq!(i.args.last().map(String::as_str), Some("-"));
        assert!(!i.args.iter().any(|a| a.contains("dangerously")));
    }
}
