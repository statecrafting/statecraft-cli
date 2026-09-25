//! The consented settings modification: the one write into a harness's own
//! settings file that spec 002 section 3.14 rule 1 admits.
//!
//! Spec 002 section 3.24. It carries two kinds of line and no others: a hook
//! registration whose command resolves inside the canonical harness under the
//! product home, and a deny entry, which is a refusal. It may add a refusal and
//! it may never add or widen a permission. It is refused by default, performed
//! only against the operator's consent to that exact content, recorded as a
//! **modification** the way section 3.13 records the root instruction bridge,
//! removed only while intact, and idempotent.
//!
//! # Why this module splices text rather than reserializing
//!
//! Section 3.24 requires that outside the managed region "nothing is rewritten,
//! reordered or reformatted, and the file's own shape is preserved". Parsing a
//! settings file into a value and writing it back out again reformats every
//! byte of it: indentation, spacing, the author's own key order and any
//! trailing shape they chose. So this module parses only to **judge**, and
//! edits by splicing bytes into located spans. What a caller did not consent to
//! is not merely logically unchanged; it is the same bytes.
//!
//! # Three properties identify an insertion, and one of them is not enough
//!
//! Section 3.24, as the owner revised it on 2026-09-21: all managed insertions
//! are tracked by **one recorded modification**, they may occupy **multiple
//! syntactically valid locations**, and each is identified by its **exact
//! content**, its **structural location** and its **recorded provenance**.
//! Unrelated bytes are preserved, and removal happens only where this product
//! can establish that the content is an intact insertion it owns.
//!
//! The revision replaced a requirement that every managed line occupy one
//! physically contiguous marked region. That was unsatisfiable here and the
//! reason is structural rather than awkward: a settings file is strict JSON,
//! which has no comment syntax, and the two insertion points are not adjacent
//! in any document, because a hook registration belongs in `hooks.<Event>` and
//! a deny entry in `permissions.deny`. What the region was carrying was
//! attributability, and the three properties carry it directly.
//!
//! This module holds all three, and holds them **conjunctively**:
//!
//! 1. **Exact content.** A hook registration matches byte for byte, command
//!    and all. A deny entry matches as a string, in order, as a contiguous run
//!    inside the array.
//! 2. **Structural location.** Each recorded kind names where it lives: a
//!    [`Modification::hooks`] entry lives at `hooks.<event>` and a
//!    [`Modification::deny`] entry in `permissions.deny`. A document whose
//!    structure is ambiguous, meaning any object naming a key twice, is
//!    refused by [`unambiguous`] rather than guessed at.
//! 3. **Recorded provenance.** The home's record says this product placed it.
//!    Content this product cannot prove it placed is reported as unclaimed and
//!    is never removed.
//!
//! [`MARKER`] is **content**, which makes it the first property and not the
//! third. It is genuinely useful: a managed hook is recognizable from the file
//! alone, so a user reading their own settings can see what is there. It is
//! also copyable, so a registration carrying it is not thereby this product's,
//! and the unclaimed list in [`SettingsOutcome::Applied`] exists for exactly
//! that case. The error is always in the direction of leaving content alone.
//!
//! Strict JSON is the floor the owner fixed alongside the contract: no JSON5,
//! no comment outside a string value, no deny entry that refuses nothing and
//! exists only to mark a boundary, and no metadata key the harness does not
//! support. A marker travels inside content the harness already reads as
//! content, or it does not travel. [`MARKER`] rides inside a command string,
//! where it is a shell comment to the interpreter that runs it.
//!
//! Every guarantee section 3.24 states around the contract is met unchanged:
//! an exact reviewable plan, consent specific to the content, no widened
//! permission, `settings.local.json` never written, the digest either side,
//! idempotent application, conservative removal, and user edits and conflicts
//! preserved rather than resolved.

use crate::harness;
use crate::home::HomeError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use statecraft_environment::digest::digest_bytes;
use std::path::Path;

/// The settings file this modification may touch.
pub const SETTINGS: &str = "settings.json";

/// The settings file this product never writes at all.
///
/// Section 3.24: `settings.local.json` is the user's own override layer. It is
/// named here so a test can hold the rule, not so a code path can reach it.
pub const LOCAL_SETTINGS: &str = "settings.local.json";

/// The first line of every managed hook command.
///
/// A shell comment to the interpreter and a marker to this module. Followed by
/// one space and the harness revision, so the file itself says which revision
/// the registration belongs to.
///
/// It is the **content** property of section 3.24's three, and no more than
/// that. Anyone can type it, so a registration carrying it is not this
/// product's until the record says so as well. Section 3.28 states the
/// general rule: an unmarked registration is the user's, and resemblance is
/// never ownership.
pub const MARKER: &str = "# statecraft-managed";

/// The refusals that travel with any delivery that carries permissions at all.
///
/// Spec 002 section 3.23: no publish verb, no release verb, no force push, no
/// recursive removal of a corpus or a derived tree. A floor, so these are only
/// ever added; nothing here removes, weakens or reorders what a user already
/// refused.
///
/// **Where this lands is section 3.27, and it is not here by default.** A deny
/// entry is evaluated by the harness before anything of this product's runs,
/// so it has nowhere to test for `.statecraft/environment.json` and exit. It
/// carries no project gate and acquires none from the hook scripts registered
/// beside it. Written into a user's global settings it would refuse these
/// commands in every repository that user opens, managed or not, which is
/// exactly what section 3.14 rule 3 forbids. So it is delivered to a **managed
/// session** instead: see [`crate::session`].
pub const DENY_FLOOR: [&str; 8] = [
    "Bash(cargo publish*)",
    "Bash(npm publish*)",
    "Bash(gh release create *)",
    "Bash(gh release delete *)",
    "Bash(git push --force*)",
    "Bash(git push -f *)",
    "Bash(rm -rf specs*)",
    "Bash(rm -rf .statecraft/derived*)",
];

/// The event the shipped gate registers on.
///
/// `SessionStart` reports; it does not stand between the session and a tool
/// call. Whether a harness's end-of-turn event should advise or refuse is an
/// open adoption question this module deliberately does not answer, so the one
/// registration this build ships is on the event where the answer is the same
/// either way.
pub const SHIPPED_EVENT: &str = "SessionStart";

/// The matcher the shipped registration carries.
pub const SHIPPED_MATCHER: &str = "startup|resume|clear|compact";

/// One hook registration, exactly as it is written into the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookRegistration {
    /// The harness event, for example `SessionStart`.
    pub event: String,
    /// The matcher the registration carries.
    pub matcher: String,
    /// The command, whose first line is the marker and whose executable path is
    /// inside the canonical harness.
    pub command: String,
}

impl HookRegistration {
    /// The revision this registration's marker names.
    pub fn marked_revision(&self) -> Option<&str> {
        self.command
            .lines()
            .next()?
            .strip_prefix(MARKER)
            .map(str::trim)
    }

    /// The JSON a settings file holds for one registration's matcher group.
    fn group(&self) -> Value {
        serde_json::json!({
            "matcher": self.matcher,
            "hooks": [ { "type": "command", "command": self.command } ],
        })
    }
}

/// The exact content one revision would place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Managed {
    /// The harness revision this content belongs to.
    pub revision: String,
    /// Every hook registration, in the order they are written.
    pub hooks: Vec<HookRegistration>,
    /// Every deny entry, in the order they are appended.
    pub deny: Vec<String>,
}

impl Managed {
    /// The content identity: a digest over the exact content and nothing else.
    ///
    /// Section 3.24's "consent to one revision is not consent to the next" is
    /// this. Content whose lines have changed has a different identity, so the
    /// modification is presented again rather than performed on a consent
    /// given to something else.
    ///
    /// This is half of what an operator consents to. The other half is the
    /// file the content would go into: see [`consent_token`].
    pub fn token(&self) -> String {
        let mut material = String::new();
        for hook in &self.hooks {
            material.push_str(&hook.event);
            material.push('\u{0}');
            material.push_str(&hook.matcher);
            material.push('\u{0}');
            material.push_str(&hook.command);
            material.push('\n');
        }
        material.push('\u{1}');
        for entry in &self.deny {
            material.push_str(entry);
            material.push('\n');
        }
        digest_bytes(material.as_bytes())
    }

    /// The exact lines the plan shows before anything is written.
    ///
    /// Section 3.24 requires the modification to be named "in the exact lines
    /// it would add". This is those lines, and the plan renders nothing else.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for hook in &self.hooks {
            out.push(format!("hook {} matcher {}", hook.event, hook.matcher));
            for line in hook.command.lines() {
                out.push(format!("  {line}"));
            }
        }
        for entry in &self.deny {
            out.push(format!("deny {entry}"));
        }
        out
    }

    /// True when this would place nothing.
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty() && self.deny.is_empty()
    }
}

/// The token an operator consents to: the content, and the file it goes into.
///
/// Section 3.24 requires consent to identify **the exact planned
/// modification**, and a modification is not only what would be inserted. The
/// same content spliced into a different file is a different plan: different
/// insertion points, a different set of entries already refused, a different
/// set of conflicts, and different resulting bytes. An operator who reviewed
/// one of those did not review the other.
///
/// So the token covers the content identity and the digest of the target as it
/// was when the plan was computed. A settings file that changed after the plan
/// was shown produces a different token, the consent does not match, and the
/// current plan is presented instead of a plan nobody read. That is the
/// mechanism behind "do not overwrite a file changed since it was inspected":
/// an operator loses a retry and never loses a line they wrote.
pub fn consent_token(content: &str, target: &str) -> String {
    digest_bytes(format!("{content}\u{0}{target}").as_bytes())
}

/// The content this build would place, for one installed harness revision.
///
/// The hook command's executable path is inside that revision's directory under
/// the product home, which is section 3.24's rule 1: never a command assembled
/// from anything else.
pub fn managed(revision: &str, revision_root: &Path) -> Managed {
    let registration = |event: &str, matcher: &str, file: &str| {
        let script = revision_root.join("hooks").join(file);
        HookRegistration {
            event: event.to_string(),
            matcher: matcher.to_string(),
            command: format!(
                "{MARKER} {revision}\n\"{}\" \"${{CLAUDE_PROJECT_DIR:-.}}\"",
                script.display()
            ),
        }
    };

    // The four event behaviors the owner adopted on 2026-09-21, and nothing
    // else. Every command resolves inside the canonical harness under the
    // product home, which is section 3.24's rule 1.
    //
    // The hand-rolled `statecraft-gate.sh` this build shipped before the
    // adoption is gone rather than registered beside them: it was a
    // `SessionStart` freshness report written when no inventory had been
    // adopted, and the adopted `SessionStart` behavior does the same job
    // against the same contracts. Two registrations on one event would run
    // two freshness reports per session and make "one canonical source" false
    // in the only place a user would see it.
    let hooks: Vec<HookRegistration> = harness::ADOPTED_HOOKS
        .iter()
        .map(|hook| registration(hook.event, hook.matcher, hook.file))
        .collect();

    Managed {
        revision: revision.to_string(),
        hooks,
        // Empty, and section 3.27 is why. The consented modification into a
        // user's global settings carries hook registrations, which are
        // project-gated because a script can test for the manifest and exit.
        // It does not carry the deny floor, which cannot be. The floor is
        // delivered per managed session by [`crate::session`].
        deny: Vec::new(),
    }
}

/// The content this build would place, resolved from a home layout.
pub fn managed_for(layout: &crate::home::Layout) -> Managed {
    let revision = harness::revision_of(&harness::shipped());
    let root = layout.harness_revision_dir(&revision.id);
    managed(&revision.id, &root)
}

/// Why nothing will be written.
///
/// Every variant is a refusal: a precondition was not met and the file is
/// untouched. None of them is a partial write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Refusal {
    /// The file is not JSON this product can read.
    Malformed {
        /// What the parser said, including where.
        reason: String,
    },
    /// The file is JSON but not an object, so it is not a settings file.
    NotAnObject {
        /// What it is instead.
        found: String,
    },
    /// A key this modification must write into holds the wrong kind of value.
    WrongShape {
        /// The dotted path.
        at: String,
        /// What is there.
        found: String,
    },
    /// The computed result would have changed something outside the region.
    ///
    /// A bug in this module, caught before the write rather than after it. The
    /// never-widen rules are checked against the bytes that would be written,
    /// not against the intention of the code that produced them.
    WouldChangeMoreThanTheRegion {
        /// What moved.
        detail: String,
    },
    /// An object in the document names the same key twice.
    ///
    /// Section 3.24 identifies an insertion partly by its **structural
    /// location**, and a duplicate key means the document has two locations
    /// with one name. This module would not merely be imprecise about which it
    /// meant: `serde_json`, which judges, keeps the last occurrence, and
    /// [`locate`], which edits, finds the first. So a judgement made about one
    /// value would be applied to another. Refused rather than guessed.
    AmbiguousStructure {
        /// The dotted path of the object holding the duplicate.
        at: String,
        /// The key named more than once.
        key: String,
    },
    /// The file changed between being inspected and being written.
    ///
    /// The plan is computed from bytes that were read. Writing bytes derived
    /// from a stale read would discard whatever the user did in between, which
    /// is the one thing "unrelated bytes are preserved" cannot survive.
    ChangedSinceInspected {
        /// The digest the plan was computed against.
        inspected: String,
        /// The digest the file has now.
        found: String,
    },
}

impl Refusal {
    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Refusal::Malformed { reason } => {
                format!("{SETTINGS} is not readable JSON, so nothing is written: {reason}")
            }
            Refusal::NotAnObject { found } => {
                format!("{SETTINGS} holds {found}, not an object; this is not a settings file")
            }
            Refusal::WrongShape { at, found } => {
                format!("{at} holds {found}, which this modification cannot write into")
            }
            Refusal::WouldChangeMoreThanTheRegion { detail } => format!(
                "refusing to write: the result would change something outside the managed \
                 region ({detail})"
            ),
            Refusal::AmbiguousStructure { at, key } => format!(
                "{at} names `{key}` more than once, so there is no one structural location to \
                 identify an insertion by; nothing is written"
            ),
            Refusal::ChangedSinceInspected { inspected, found } => format!(
                "{SETTINGS} changed since it was inspected ({inspected} -> {found}), so the \
                 planned write is against bytes that are no longer there; nothing is written"
            ),
        }
    }
}

/// A user's own registration on an event this modification also writes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// The event both registrations are on.
    pub event: String,
    /// The matcher the existing registration carries.
    pub matcher: String,
    /// The first line of the existing command, enough to recognize it by.
    pub command_excerpt: String,
}

impl Conflict {
    /// A one-line rendering.
    pub fn describe(&self) -> String {
        format!(
            "{}: a hook you registered (matcher `{}`, `{}`) stays, and this one is added \
             beside it; this product does not decide which of two hooks you want",
            self.event, self.matcher, self.command_excerpt
        )
    }
}

/// What applying would do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// There is no settings file; one is created holding only the region.
    Create,
    /// The file exists and the region is added to it.
    Add,
    /// The region is present and byte-identical. Nothing to do.
    AlreadyCurrent,
    /// A region from a different revision is present. It is replaced.
    RevisionChanged,
    /// A managed registration is present whose command a user has edited.
    ///
    /// Reported and left: an edited region is a user's file again.
    Edited,
}

impl Action {
    /// Whether performing this changes the file.
    pub fn changes_the_file(self) -> bool {
        matches!(self, Action::Create | Action::Add | Action::RevisionChanged)
    }

    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            Action::Create => "create",
            Action::Add => "add",
            Action::AlreadyCurrent => "already-current",
            Action::RevisionChanged => "revision-changed",
            Action::Edited => "edited",
        }
    }
}

/// The modification, computed without writing anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// The file this is about, absolute.
    pub path: String,
    /// The harness revision the content belongs to.
    pub revision: String,
    /// What applying would do.
    pub action: Action,
    /// The exact content, and nothing else.
    pub managed: Managed,
    /// The identity of the content alone.
    pub content_token: String,
    /// The token an operator consents to: the content, and the target as it
    /// was when this plan was computed. See [`consent_token`].
    pub consent_token: String,
    /// The hook registrations this would actually add, which is empty when the
    /// region is already current.
    pub adding_hooks: Vec<HookRegistration>,
    /// The deny entries this would actually append. A floor entry the user
    /// already refused is not appended twice.
    pub adding_deny: Vec<String>,
    /// Deny entries the file already carries, left exactly where they are.
    pub already_denied: Vec<String>,
    /// A user's own registrations on the same events.
    pub conflicts: Vec<Conflict>,
    /// Managed content a user has edited. Reported, and left.
    pub edited: Vec<String>,
    /// The digest of the file before, when it exists.
    pub digest_before: Option<String>,
    /// The digest of the file as this product would leave it.
    pub digest_after: String,
    /// The bytes this product would write. Computed here so an apply writes
    /// exactly what the plan showed.
    #[serde(skip)]
    pub contents_after: String,
}

impl Plan {
    /// A rendering that shows the exact lines before anything is written.
    pub fn render(&self) -> String {
        let mut out = format!(
            "settings {} [{}] harness {}\n",
            self.path,
            self.action.word(),
            self.revision
        );
        if self.action.changes_the_file() {
            for hook in &self.adding_hooks {
                out.push_str(&format!("  + hook {} `{}`\n", hook.event, hook.matcher));
                for line in hook.command.lines() {
                    out.push_str(&format!("      {line}\n"));
                }
            }
            for entry in &self.adding_deny {
                out.push_str(&format!("  + deny {entry}\n"));
            }
            for entry in &self.already_denied {
                out.push_str(&format!("  = deny {entry} (already refused, left alone)\n"));
            }
        }
        for conflict in &self.conflicts {
            out.push_str(&format!("  ! {}\n", conflict.describe()));
        }
        for note in &self.edited {
            out.push_str(&format!("  ! {note}\n"));
        }
        out.push_str("  no allow entry, no ask setting and no existing deny entry is touched\n");
        out.push_str(&format!(
            "  consent token {}\n  it covers the content above and this file as it stands; edit \
             either and the plan is shown again\n  refused by default: re-run \
             `home apply --consent-settings {}`\n",
            self.consent_token, self.consent_token
        ));
        out
    }
}

/// A recorded modification of a file this product does not own.
///
/// Section 3.24, on section 3.13's model: path, the exact lines, the digest
/// before and the digest after. Never a managed entry, and never ownership of
/// the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Modification {
    /// The file, absolute.
    pub path: String,
    /// What kind of modification. One value today, present so a second kind is
    /// a new value rather than a reinterpretation of this record.
    pub kind: String,
    /// The harness this file belongs to.
    pub harness: String,
    /// The harness revision the content belongs to.
    pub revision: String,
    /// The exact content placed.
    pub hooks: Vec<HookRegistration>,
    /// The exact deny entries appended, in order.
    pub deny: Vec<String>,
    /// The digest of the file before this product wrote it.
    pub digest_before: Option<String>,
    /// The digest of the file after.
    pub digest_after: String,
    /// When, RFC 3339 UTC.
    pub recorded_at: String,
}

/// The kind every modification this module records carries.
pub const KIND: &str = "settings";

/// Every modification recorded in a home.
///
/// A plain list rather than a map: the path is the identity and a list keeps
/// the file readable by a person who is deciding whether to trust it.
pub fn read_modifications(layout: &crate::home::Layout) -> Result<Vec<Modification>, HomeError> {
    let path = layout.modifications_file();
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(HomeError::Io { path, source }),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&text).map_err(|source| HomeError::Malformed { path, source })
}

/// Write them back.
pub fn write_modifications(
    layout: &crate::home::Layout,
    modifications: &[Modification],
) -> Result<(), HomeError> {
    let path = layout.modifications_file();
    let mut json =
        serde_json::to_string_pretty(modifications).map_err(|source| HomeError::Malformed {
            path: path.clone(),
            source,
        })?;
    json.push('\n');
    std::fs::write(&path, json).map_err(|source| HomeError::Io { path, source })
}

/// The recorded modification for one path, if there is one.
pub fn recorded_for<'a>(modifications: &'a [Modification], path: &str) -> Option<&'a Modification> {
    modifications.iter().find(|m| m.path == path)
}

/// Replace or insert the record for one path.
pub fn record(modifications: &mut Vec<Modification>, entry: Modification) {
    match modifications.iter().position(|m| m.path == entry.path) {
        Some(i) => modifications[i] = entry,
        None => modifications.push(entry),
    }
}

/// Drop the record for one path.
pub fn forget(modifications: &mut Vec<Modification>, path: &str) {
    modifications.retain(|m| m.path != path);
}

/// Write a file so that an interruption leaves either the old bytes or the new
/// ones, and never half of each.
///
/// Section 3.24 has no half-applied state to describe, so there must not be
/// one: the bytes land under a neighbouring name and are renamed over the
/// target, which is atomic within a directory on every filesystem this product
/// runs on.
pub fn write_atomically(path: &Path, contents: &str) -> std::io::Result<()> {
    let temporary = path.with_extension("statecraft-partial");
    std::fs::write(&temporary, contents)?;
    std::fs::rename(&temporary, path)
}

/// What removal did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    tag = "outcome"
)]
pub enum Removal {
    /// The region was present and intact, and is taken back out.
    Removed {
        /// What was removed, one line each.
        removed: Vec<String>,
        /// Containers this product had filled and that are now empty, removed
        /// with the region rather than left as residue.
        emptied: Vec<String>,
        /// The digest before.
        digest_before: String,
        /// The digest after.
        digest_after: String,
        /// The bytes to write.
        #[serde(skip)]
        contents_after: String,
    },
    /// Nothing recorded is present. Removal is a no-op, not a failure.
    NotPresent {
        /// Why this is the answer.
        reason: String,
    },
    /// A user has edited the region. It is reported, and left.
    Edited {
        /// What does not match the record.
        detail: Vec<String>,
    },
}

impl Removal {
    /// A one-line rendering.
    pub fn render(&self) -> String {
        match self {
            Removal::Removed {
                removed, emptied, ..
            } => {
                let mut out = String::new();
                for line in removed {
                    out.push_str(&format!("  - {line}\n"));
                }
                for line in emptied {
                    out.push_str(&format!("  - {line} (emptied by the removal)\n"));
                }
                out
            }
            Removal::NotPresent { reason } => format!("  nothing to remove: {reason}\n"),
            Removal::Edited { detail } => {
                let mut out = String::from(
                    "  the managed content has been edited, so it is yours now and is left:\n",
                );
                for line in detail {
                    out.push_str(&format!("    {line}\n"));
                }
                out
            }
        }
    }

    /// The bytes to write, when there are any.
    pub fn contents_after(&self) -> Option<&str> {
        match self {
            Removal::Removed { contents_after, .. } => Some(contents_after),
            _ => None,
        }
    }
}

/// What the operator asked for, about this one modification.
///
/// Section 3.24: the modification is refused by default. [`Intent::Withheld`]
/// is what every read, every test, every project operation and a `home apply`
/// that says nothing about settings carries, and it writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Intent {
    /// Show the plan; write nothing.
    #[default]
    Withheld,
    /// Perform exactly the content whose token this is.
    Consented {
        /// The token the operator saw in the plan and repeated back.
        token: String,
    },
    /// Take the recorded region back out.
    Remove,
}

/// What the settings modification did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    tag = "state"
)]
pub enum SettingsOutcome {
    /// This harness has no settings file to modify here.
    NotApplicable {
        /// The harness.
        harness: String,
        /// Why.
        reason: String,
    },
    /// The plan, and the fact that nothing was written.
    Withheld {
        /// The plan, in the exact lines it would add.
        plan: Box<Plan>,
    },
    /// The consent given does not name the modification this apply would make.
    ///
    /// Either the content changed, or the file it would go into did. Both are
    /// "a modification the operator has not reviewed", and the answer to both
    /// is to present the current one.
    ConsentStale {
        /// The token the current content, against the current file, has.
        expected: String,
        /// The token the operator gave.
        given: String,
        /// The identity of the content alone, so an operator can see which
        /// half of the pair moved.
        content_token: String,
        /// The digest of the target the plan below was computed against.
        target_digest: String,
        /// The plan for the current content.
        plan: Box<Plan>,
    },
    /// The modification was performed and recorded.
    Applied {
        /// What was written.
        plan: Box<Plan>,
        /// A superseded region taken out first, when there was one.
        superseded: Option<Removal>,
        /// What the record deliberately does not claim, each with its reason.
        ///
        /// A refusal this product cannot prove it placed is not recorded as
        /// placed, because removal would then take out a refusal that was the
        /// user's. The error is always in the direction of keeping a refusal.
        unclaimed: Vec<String>,
    },
    /// The recorded region was taken back out, or was not there to take.
    Withdrawn {
        /// The file.
        path: String,
        /// What happened.
        removal: Removal,
    },
    /// A precondition stopped it and the file is untouched.
    Refused {
        /// The file.
        path: String,
        /// Why.
        refusal: Refusal,
    },
    /// Something nobody asked for went wrong.
    Failed {
        /// The file.
        path: String,
        /// Why.
        reason: String,
    },
}

impl SettingsOutcome {
    /// True when an operator was shown a modification and it was not performed.
    ///
    /// Spec 006 section 3.3 spends exit 1 on "a withheld write", and this is
    /// one: named, and not done.
    pub fn is_withheld_write(&self) -> bool {
        match self {
            SettingsOutcome::Withheld { plan } => plan.action.changes_the_file(),
            SettingsOutcome::ConsentStale { .. } => true,
            _ => false,
        }
    }

    /// True when a precondition stopped it.
    pub fn is_refusal(&self) -> bool {
        matches!(self, SettingsOutcome::Refused { .. })
    }

    /// True when something nobody asked for went wrong.
    pub fn is_failure(&self) -> bool {
        matches!(self, SettingsOutcome::Failed { .. })
    }

    /// A human-readable rendering.
    pub fn render(&self) -> String {
        match self {
            SettingsOutcome::NotApplicable { harness, reason } => {
                format!("settings {harness}: {reason}\n")
            }
            SettingsOutcome::Withheld { plan } => plan.render(),
            SettingsOutcome::ConsentStale {
                expected,
                given,
                content_token,
                target_digest,
                plan,
            } => {
                format!(
                    "settings {}: the consent given ({given}) does not name the modification this \
                 would make; nothing was written\n{}",
                    plan.path,
                    plan.render()
                ) + &format!(
                    "  the token covers the content ({content_token}) and the file it goes into \
                     ({target_digest}); one of the two has changed since you were shown a plan\n  \
                     consent to the modification above with {expected}\n"
                )
            }
            SettingsOutcome::Applied {
                plan,
                superseded,
                unclaimed,
            } => {
                let mut out = format!(
                    "settings {} [{}] harness {}\n",
                    plan.path,
                    plan.action.word(),
                    plan.revision
                );
                if let Some(removal) = superseded {
                    out.push_str("  superseded region:\n");
                    out.push_str(&removal.render());
                }
                for hook in &plan.adding_hooks {
                    out.push_str(&format!("  + hook {} `{}`\n", hook.event, hook.matcher));
                }
                for entry in &plan.adding_deny {
                    out.push_str(&format!("  + deny {entry}\n"));
                }
                for conflict in &plan.conflicts {
                    out.push_str(&format!("  ! {}\n", conflict.describe()));
                }
                for note in &plan.edited {
                    out.push_str(&format!("  ! {note}\n"));
                }
                for note in unclaimed {
                    out.push_str(&format!("  = {note}\n"));
                }
                out.push_str(&format!(
                    "  recorded as a modification: before {} after {}\n",
                    plan.digest_before.as_deref().unwrap_or("(absent)"),
                    plan.digest_after
                ));
                out
            }
            SettingsOutcome::Withdrawn { path, removal } => {
                format!("settings {path}\n{}", removal.render())
            }
            SettingsOutcome::Refused { path, refusal } => {
                format!("settings {path}: refused: {}\n", refusal.describe())
            }
            SettingsOutcome::Failed { path, reason } => {
                format!("settings {path}: failed: {reason}\n")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Planning
// ---------------------------------------------------------------------------

/// Compute the modification. Pure: no filesystem, no clock.
///
/// `existing` is the current file's text, or `None` when there is none.
pub fn plan(path: &str, existing: Option<&str>, managed: &Managed) -> Result<Plan, Refusal> {
    let Some(text) = existing else {
        let contents_after = fresh(managed);
        return Ok(Plan {
            path: path.to_string(),
            revision: managed.revision.clone(),
            action: Action::Create,
            managed: managed.clone(),
            content_token: managed.token(),
            consent_token: consent_token(&managed.token(), &digest_of(None)),
            adding_hooks: managed.hooks.clone(),
            adding_deny: managed.deny.clone(),
            already_denied: Vec::new(),
            conflicts: Vec::new(),
            edited: Vec::new(),
            digest_before: None,
            digest_after: digest_bytes(contents_after.as_bytes()),
            contents_after,
        });
    };

    let before: Value = serde_json::from_str(text).map_err(|e| Refusal::Malformed {
        reason: e.to_string(),
    })?;
    let Some(root) = before.as_object() else {
        return Err(Refusal::NotAnObject {
            found: kind_of(&before).to_string(),
        });
    };
    unambiguous(text)?;

    // What is already there, by its own marker.
    let present = marked_registrations(&before);
    let mut edited = Vec::new();
    let mut stale_revision = false;
    for (event, command) in &present {
        let matching = managed
            .hooks
            .iter()
            .find(|h| &h.event == event && &h.command == command);
        if matching.is_some() {
            continue;
        }
        let revision = command
            .lines()
            .next()
            .and_then(|l| l.strip_prefix(MARKER))
            .map(str::trim)
            .unwrap_or("");
        if revision == managed.revision {
            // Same revision, different bytes: a user edited the command.
            edited.push(format!(
                "{event}: a managed hook command for revision {revision} does not match what \
                 this build places; it is yours now and is left where it is"
            ));
        } else {
            stale_revision = true;
        }
    }

    // Deny: a floor entry the file already refuses is not appended twice.
    let existing_deny = deny_entries(root)?;
    let mut adding_deny = Vec::new();
    let mut already_denied = Vec::new();
    for entry in &managed.deny {
        if existing_deny.iter().any(|e| e == entry) {
            already_denied.push(entry.clone());
        } else {
            adding_deny.push(entry.clone());
        }
    }

    let adding_hooks: Vec<HookRegistration> = managed
        .hooks
        .iter()
        .filter(|h| {
            !present
                .iter()
                .any(|(e, c)| e == &h.event && c == &h.command)
        })
        .cloned()
        .collect();

    let conflicts = conflicts_on(&before, &managed.hooks);

    let action = if !edited.is_empty() && adding_hooks.is_empty() && adding_deny.is_empty() {
        Action::Edited
    } else if adding_hooks.is_empty() && adding_deny.is_empty() {
        Action::AlreadyCurrent
    } else if stale_revision {
        Action::RevisionChanged
    } else {
        Action::Add
    };

    let contents_after = if action.changes_the_file() {
        splice(text, &adding_hooks, &adding_deny)?
    } else {
        text.to_string()
    };

    // The never-widen rules, checked against the bytes that would be written.
    if action.changes_the_file() {
        let after: Value =
            serde_json::from_str(&contents_after).map_err(|e| Refusal::Malformed {
                reason: format!("this product produced unreadable JSON, which is a defect: {e}"),
            })?;
        preserved(&before, &after, &adding_deny, &adding_hooks)?;
    }

    Ok(Plan {
        path: path.to_string(),
        revision: managed.revision.clone(),
        action,
        managed: managed.clone(),
        content_token: managed.token(),
        consent_token: consent_token(&managed.token(), &digest_of(Some(text))),
        adding_hooks,
        adding_deny,
        already_denied,
        conflicts,
        edited,
        digest_before: Some(digest_bytes(text.as_bytes())),
        digest_after: digest_bytes(contents_after.as_bytes()),
        contents_after,
    })
}

/// A settings file holding only the managed region.
fn fresh(managed: &Managed) -> String {
    let mut root = serde_json::Map::new();
    if !managed.deny.is_empty() {
        let mut permissions = serde_json::Map::new();
        permissions.insert("deny".into(), Value::from(managed.deny.clone()));
        root.insert("permissions".into(), Value::Object(permissions));
    }
    if !managed.hooks.is_empty() {
        let mut hooks = serde_json::Map::new();
        for registration in &managed.hooks {
            hooks
                .entry(registration.event.clone())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("just inserted an array")
                .push(registration.group());
        }
        root.insert("hooks".into(), Value::Object(hooks));
    }
    format!("{}\n", pretty(&Value::Object(root), "  "))
}

/// Every marked registration in a settings value, as `(event, command)`.
fn marked_registrations(value: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(events) = value.get("hooks").and_then(Value::as_object) else {
        return out;
    };
    for (event, groups) in events {
        let Some(groups) = groups.as_array() else {
            continue;
        };
        for group in groups {
            for command in commands_of(group) {
                if command.starts_with(MARKER) {
                    out.push((event.clone(), command));
                }
            }
        }
    }
    out.sort();
    out
}

/// Every command string one matcher group registers.
fn commands_of(group: &Value) -> Vec<String> {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .map(|hooks| {
            hooks
                .iter()
                .filter_map(|h| h.get("command").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// A user's own registrations on the events this modification writes to.
fn conflicts_on(value: &Value, hooks: &[HookRegistration]) -> Vec<Conflict> {
    let mut out = Vec::new();
    let Some(events) = value.get("hooks").and_then(Value::as_object) else {
        return out;
    };
    for registration in hooks {
        let Some(groups) = events.get(&registration.event).and_then(Value::as_array) else {
            continue;
        };
        for group in groups {
            let matcher = group
                .get("matcher")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            for command in commands_of(group) {
                if command.starts_with(MARKER) {
                    continue;
                }
                let excerpt: String = command
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(60)
                    .collect();
                out.push(Conflict {
                    event: registration.event.clone(),
                    matcher: matcher.clone(),
                    command_excerpt: excerpt,
                });
            }
        }
    }
    out
}

/// The deny entries a settings object carries.
fn deny_entries(root: &serde_json::Map<String, Value>) -> Result<Vec<String>, Refusal> {
    let Some(permissions) = root.get("permissions") else {
        return Ok(Vec::new());
    };
    let Some(permissions) = permissions.as_object() else {
        return Err(Refusal::WrongShape {
            at: "permissions".into(),
            found: kind_of(permissions).into(),
        });
    };
    let Some(deny) = permissions.get("deny") else {
        return Ok(Vec::new());
    };
    let Some(deny) = deny.as_array() else {
        return Err(Refusal::WrongShape {
            at: "permissions.deny".into(),
            found: kind_of(deny).into(),
        });
    };
    Ok(deny
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect())
}

/// Refuse a document in which any object names a key twice.
///
/// Section 3.24 identifies an insertion by exact content, **structural
/// location** and recorded provenance. A duplicate key breaks the second: the
/// document holds two locations with one name, and this module's two views of
/// it disagree about which one is meant. `serde_json` keeps the last
/// occurrence of a duplicate and is what every judgement here is made against;
/// [`locate`] walks the text and takes the first, and is what every edit is
/// applied to. On a document with no duplicate key the two always agree, which
/// is why the rest of this module may treat them as one view.
///
/// Scanned over the whole document rather than only the paths this
/// modification writes into, because the never-widen checks compare the parsed
/// documents whole.
fn unambiguous(text: &str) -> Result<(), Refusal> {
    fn walk(text: &str, object: Span, path: &str) -> Result<(), Refusal> {
        let members = object_members(text, object);
        let mut seen: Vec<&str> = Vec::with_capacity(members.len());
        for member in &members {
            if seen.contains(&member.key.as_str()) {
                return Err(Refusal::AmbiguousStructure {
                    at: path.to_string(),
                    key: member.key.clone(),
                });
            }
            seen.push(&member.key);
        }
        for member in &members {
            let child = if path == "(root)" {
                member.key.clone()
            } else {
                format!("{path}.{}", member.key)
            };
            descend(text, member.value, &child)?;
        }
        Ok(())
    }

    fn descend(text: &str, value: Span, path: &str) -> Result<(), Refusal> {
        match text.as_bytes().get(value.start) {
            Some(b'{') => walk(text, value, path),
            Some(b'[') => {
                for (i, element) in array_elements(text, value).into_iter().enumerate() {
                    descend(text, element, &format!("{path}[{i}]"))?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    descend(text, root_span(text)?, "(root)")
}

/// The digest of a file's text, or the reserved word for a file that is absent.
///
/// An absent file and an empty one are different preconditions, and a digest
/// over zero bytes would make them the same.
fn digest_of(text: Option<&str>) -> String {
    match text {
        Some(text) => digest_bytes(text.as_bytes()),
        None => "(absent)".to_string(),
    }
}

/// What a value is, in one word, for a report.
fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The never-widen rules, checked against the two parsed documents.
///
/// Asserted over the bytes that would be written rather than over the intent of
/// the code that produced them: a splice that went wrong is caught here, before
/// the write, instead of in an operator's settings file afterwards.
fn preserved(
    before: &Value,
    after: &Value,
    adding_deny: &[String],
    adding_hooks: &[HookRegistration],
) -> Result<(), Refusal> {
    let refuse = |detail: String| Refusal::WouldChangeMoreThanTheRegion { detail };

    // No allow entry is added or widened, and no ask setting is downgraded.
    for key in ["allow", "ask", "defaultMode", "additionalDirectories"] {
        let b = before.pointer(&format!("/permissions/{key}"));
        let a = after.pointer(&format!("/permissions/{key}"));
        if b != a {
            return Err(refuse(format!("permissions.{key} changed")));
        }
    }

    // Existing deny entries are not removed, weakened or reordered: the entries
    // that were there are still there, in the same order, as a prefix.
    let deny_before = before
        .pointer("/permissions/deny")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let deny_after = after
        .pointer("/permissions/deny")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if deny_after.len() != deny_before.len() + adding_deny.len() {
        return Err(refuse(format!(
            "the deny list went from {} to {} entries and {} were added",
            deny_before.len(),
            deny_after.len(),
            adding_deny.len()
        )));
    }
    if deny_after[..deny_before.len()] != deny_before[..] {
        return Err(refuse(
            "an existing deny entry moved, changed or was removed".into(),
        ));
    }

    // Every key outside permissions and hooks is byte-for-byte the value it was.
    let (Some(bo), Some(ao)) = (before.as_object(), after.as_object()) else {
        return Err(refuse("the document stopped being an object".into()));
    };
    for (key, value) in bo {
        if key == "permissions" || key == "hooks" {
            continue;
        }
        if ao.get(key) != Some(value) {
            return Err(refuse(format!("the top-level key `{key}` changed")));
        }
    }
    for key in ao.keys() {
        if !bo.contains_key(key) && key != "permissions" && key != "hooks" {
            return Err(refuse(format!("a top-level key `{key}` was added")));
        }
    }

    // Every hook registration that was there is still there.
    let existing_groups = |v: &Value| -> Vec<(String, Value)> {
        v.get("hooks")
            .and_then(Value::as_object)
            .map(|events| {
                events
                    .iter()
                    .flat_map(|(event, groups)| {
                        groups
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(move |g| (event.clone(), g))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let groups_before = existing_groups(before);
    let groups_after = existing_groups(after);
    for entry in &groups_before {
        if !groups_after.contains(entry) {
            return Err(refuse(format!(
                "a hook registration on {} was removed or rewritten",
                entry.0
            )));
        }
    }
    if groups_after.len() != groups_before.len() + adding_hooks.len() {
        return Err(refuse(format!(
            "the hook registrations went from {} to {} and {} were added",
            groups_before.len(),
            groups_after.len(),
            adding_hooks.len()
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Removal
// ---------------------------------------------------------------------------

/// Compute the removal of a recorded modification. Pure.
pub fn removal(existing: Option<&str>, record: &Modification) -> Result<Removal, Refusal> {
    let Some(text) = existing else {
        return Ok(Removal::NotPresent {
            reason: format!("{SETTINGS} does not exist"),
        });
    };
    let before: Value = serde_json::from_str(text).map_err(|e| Refusal::Malformed {
        reason: e.to_string(),
    })?;
    let Some(root) = before.as_object() else {
        return Err(Refusal::NotAnObject {
            found: kind_of(&before).into(),
        });
    };
    unambiguous(text)?;

    let present = marked_registrations(&before);
    let mut detail = Vec::new();

    // Every recorded hook is present and byte-identical, or the region is not
    // intact and nothing is taken back.
    for hook in &record.hooks {
        if !present
            .iter()
            .any(|(e, c)| e == &hook.event && c == &hook.command)
        {
            let marked_here = present.iter().any(|(e, _)| e == &hook.event);
            detail.push(if marked_here {
                format!(
                    "{}: a managed registration is there but its command is not the one recorded",
                    hook.event
                )
            } else {
                format!("{}: the recorded registration is not there", hook.event)
            });
        }
    }
    // A marked registration this record does not know about is not this
    // record's to remove, and saying so is more useful than removing it.
    for (event, command) in &present {
        if !record
            .hooks
            .iter()
            .any(|h| &h.event == event && &h.command == command)
        {
            detail.push(format!(
                "{event}: a managed registration is present that this record does not describe"
            ));
        }
    }

    // The recorded deny entries are a contiguous run, in order, at the end.
    let existing_deny = deny_entries(root)?;
    let run = contiguous_run(&existing_deny, &record.deny);
    if run.is_none() && !record.deny.is_empty() {
        detail.push(
            "permissions.deny: the recorded entries are not present as one contiguous run in the \
             order they were written"
                .to_string(),
        );
    }

    if !detail.is_empty() {
        return Ok(Removal::Edited { detail });
    }
    if record.hooks.is_empty() && record.deny.is_empty() {
        return Ok(Removal::NotPresent {
            reason: "the record describes no content".into(),
        });
    }

    let (contents_after, emptied) = unsplice(text, &record.hooks, run)?;
    let after: Value = serde_json::from_str(&contents_after).map_err(|e| Refusal::Malformed {
        reason: format!("this product produced unreadable JSON, which is a defect: {e}"),
    })?;
    removal_preserved(&before, &after, record)?;

    let mut removed: Vec<String> = record
        .hooks
        .iter()
        .map(|h| format!("hook {} `{}`", h.event, h.matcher))
        .collect();
    removed.extend(record.deny.iter().map(|d| format!("deny {d}")));

    Ok(Removal::Removed {
        removed,
        emptied,
        digest_before: digest_bytes(text.as_bytes()),
        digest_after: digest_bytes(contents_after.as_bytes()),
        contents_after,
    })
}

/// Where a slice occurs contiguously inside another, if it does.
fn contiguous_run(haystack: &[String], needle: &[String]) -> Option<std::ops::Range<usize>> {
    if needle.is_empty() {
        return Some(0..0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len())
        .find(|&i| &haystack[i..i + needle.len()] == needle)
        .map(|i| i..i + needle.len())
}

/// Removal's own never-widen check: nothing but the region came out.
fn removal_preserved(before: &Value, after: &Value, record: &Modification) -> Result<(), Refusal> {
    let refuse = |detail: String| Refusal::WouldChangeMoreThanTheRegion { detail };
    for key in ["allow", "ask", "defaultMode", "additionalDirectories"] {
        if before.pointer(&format!("/permissions/{key}"))
            != after.pointer(&format!("/permissions/{key}"))
        {
            return Err(refuse(format!("permissions.{key} changed")));
        }
    }
    let deny_before: Vec<Value> = before
        .pointer("/permissions/deny")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let deny_after: Vec<Value> = after
        .pointer("/permissions/deny")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if deny_after.len() + record.deny.len() != deny_before.len() {
        return Err(refuse(format!(
            "the deny list went from {} to {} entries and {} were removed",
            deny_before.len(),
            deny_after.len(),
            record.deny.len()
        )));
    }
    let survivors: Vec<Value> = deny_before
        .iter()
        .filter(|v| !record.deny.iter().any(|d| v.as_str() == Some(d.as_str())))
        .cloned()
        .collect();
    // An entry a user wrote that happens to equal a floor entry is indistinguishable
    // from the managed copy, so the comparison is on multiplicity rather than identity.
    if deny_after.len() == survivors.len() && deny_after != survivors {
        return Err(refuse("a surviving deny entry moved".into()));
    }
    let (Some(bo), Some(ao)) = (before.as_object(), after.as_object()) else {
        return Err(refuse("the document stopped being an object".into()));
    };
    for (key, value) in bo {
        if key == "permissions" || key == "hooks" {
            continue;
        }
        if ao.get(key) != Some(value) {
            return Err(refuse(format!("the top-level key `{key}` changed")));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Performing it
// ---------------------------------------------------------------------------

/// The harness whose settings file this modification knows how to write.
///
/// One, deliberately. A second harness is a second file format and a second set
/// of load rules, and guessing at either is how a product ends up rewriting a
/// configuration it does not understand.
pub const SUPPORTED_HARNESS: &str = "claude-code";

/// Plan or perform the modification for one native home.
///
/// The one function in this module that touches a filesystem. Everything it
/// decides is decided by [`plan`] and [`removal`], which are pure, so an
/// operator reading a plan is reading the same computation that will run.
pub fn perform(
    layout: &crate::home::Layout,
    native: &crate::delivery::NativeHome,
    intent: &Intent,
    now: &str,
) -> SettingsOutcome {
    if native.harness != SUPPORTED_HARNESS {
        return SettingsOutcome::NotApplicable {
            harness: native.harness.clone(),
            reason: format!(
                "this product writes no settings file for {}; section 3.24 admits one file \
                 format and inventing a second is not a use of it",
                native.harness
            ),
        };
    }
    if !native.root.is_dir() {
        return SettingsOutcome::NotApplicable {
            harness: native.harness.clone(),
            reason: format!(
                "no {} directory, so there is no settings file to modify",
                native.root.display()
            ),
        };
    }

    let path = native.root.join(SETTINGS);
    let display = path.display().to_string();
    let read = || -> Result<Option<String>, std::io::Error> {
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    };
    let existing = match read() {
        Ok(existing) => existing,
        Err(e) => {
            return SettingsOutcome::Failed {
                path: display,
                reason: e.to_string(),
            };
        }
    };

    // The digest of what was inspected. Every write below is checked against
    // it immediately before it happens: consent names the exact content to
    // place, and the file it is placed into must still be the file the plan
    // was computed from. A user who edits their settings while an apply is in
    // flight gets a refusal, not a silent overwrite of what they wrote.
    let inspected = digest_of(existing.as_deref());
    let unchanged = || -> Result<(), Box<SettingsOutcome>> {
        let now = match read() {
            Ok(text) => digest_of(text.as_deref()),
            Err(e) => {
                return Err(Box::new(SettingsOutcome::Failed {
                    path: display.clone(),
                    reason: e.to_string(),
                }));
            }
        };
        if now == inspected {
            return Ok(());
        }
        Err(Box::new(SettingsOutcome::Refused {
            path: display.clone(),
            refusal: Refusal::ChangedSinceInspected {
                inspected: inspected.clone(),
                found: now,
            },
        }))
    };

    let mut modifications = match read_modifications(layout) {
        Ok(m) => m,
        Err(e) => {
            return SettingsOutcome::Failed {
                path: display,
                reason: e.to_string(),
            };
        }
    };

    if matches!(intent, Intent::Remove) {
        let Some(recorded) = recorded_for(&modifications, &display).cloned() else {
            return SettingsOutcome::Withdrawn {
                path: display,
                removal: Removal::NotPresent {
                    reason: "no modification of this file is recorded".into(),
                },
            };
        };
        let outcome = match removal(existing.as_deref(), &recorded) {
            Ok(outcome) => outcome,
            Err(refusal) => {
                return SettingsOutcome::Refused {
                    path: display,
                    refusal,
                };
            }
        };
        if let Some(contents) = outcome.contents_after() {
            if let Err(refused) = unchanged() {
                return *refused;
            }
            if let Err(e) = write_atomically(&path, contents) {
                return SettingsOutcome::Failed {
                    path: display,
                    reason: e.to_string(),
                };
            }
        }
        // The record is dropped only for an outcome that actually took the
        // region back. An edited region stays recorded, because the record is
        // the only thing that still says what this product put there.
        if matches!(
            outcome,
            Removal::Removed { .. } | Removal::NotPresent { .. }
        ) {
            forget(&mut modifications, &display);
            if let Err(e) = write_modifications(layout, &modifications) {
                return SettingsOutcome::Failed {
                    path: display,
                    reason: e.to_string(),
                };
            }
        }
        return SettingsOutcome::Withdrawn {
            path: display,
            removal: outcome,
        };
    }

    let content = managed_for(layout);
    // What a consent for this content, against this file as it stands, must
    // be. Computed from the on-disk digest rather than from the plan's,
    // because a plan that first takes a superseded region out is computed
    // against text that was never on disk, and an operator consents to the
    // file they inspected.
    let token = consent_token(&content.token(), &inspected);

    // A recorded region from a different revision comes out before the new one
    // goes in, and only while it is intact. That is what keeps a revision
    // change from being a rewrite of a file this product does not own.
    let superseded_record = recorded_for(&modifications, &display)
        .filter(|m| m.revision != content.revision)
        .cloned();

    let (base, superseded) = match (&superseded_record, intent) {
        (Some(record), Intent::Consented { .. }) => match removal(existing.as_deref(), record) {
            Ok(outcome) => {
                let base = outcome
                    .contents_after()
                    .map(str::to_string)
                    .or_else(|| existing.clone());
                (base, Some(outcome))
            }
            Err(refusal) => {
                return SettingsOutcome::Refused {
                    path: display,
                    refusal,
                };
            }
        },
        _ => (existing.clone(), None),
    };

    let plan = match plan(&display, base.as_deref(), &content) {
        Ok(plan) => plan,
        Err(refusal) => {
            return SettingsOutcome::Refused {
                path: display,
                refusal,
            };
        }
    };

    let Intent::Consented { token: given } = intent else {
        return SettingsOutcome::Withheld {
            plan: Box::new(plan),
        };
    };

    let would_write = plan.action.changes_the_file() || superseded.is_some();

    // Consent is checked here, at the write boundary, and only when there is a
    // write to consent to. A modification already in place is section 3.24's
    // "applying the modification twice changes nothing", and asking an
    // operator to re-consent to a no-op would turn idempotence into a
    // conversation. Where there **is** a write, the token must name this exact
    // planned modification: this content, into this file as it stands.
    if would_write && given != &token {
        let current = match self::plan(&display, existing.as_deref(), &content) {
            Ok(current) => current,
            Err(refusal) => {
                return SettingsOutcome::Refused {
                    path: display,
                    refusal,
                };
            }
        };
        return SettingsOutcome::ConsentStale {
            expected: token,
            given: given.clone(),
            content_token: content.token(),
            target_digest: inspected.clone(),
            plan: Box::new(current),
        };
    }

    if would_write {
        if let Err(refused) = unchanged() {
            return *refused;
        }
        if let Err(e) = write_atomically(&path, &plan.contents_after) {
            return SettingsOutcome::Failed {
                path: display,
                reason: e.to_string(),
            };
        }
    }

    // What the record says this product placed. A re-apply that adds nothing
    // keeps what the previous record for this revision already said, so a
    // second run does not shrink the region it can later take back out.
    let previous =
        recorded_for(&modifications, &display).filter(|m| m.revision == content.revision);
    let mut placed_deny: Vec<String> = previous.map(|m| m.deny.clone()).unwrap_or_default();
    for entry in &plan.adding_deny {
        if !placed_deny.contains(entry) {
            placed_deny.push(entry.clone());
        }
    }
    let mut placed_hooks: Vec<HookRegistration> =
        previous.map(|m| m.hooks.clone()).unwrap_or_default();
    for hook in &plan.adding_hooks {
        if !placed_hooks.contains(hook) {
            placed_hooks.push(hook.clone());
        }
    }

    // Content that is in the file but that no record attributes to this
    // product is left unclaimed. Three situations produce one, and the safe
    // answer is the same for all three: the user wrote it themselves, an apply
    // was interrupted before its record was written and the attribution is
    // gone, or someone copied a marked registration out of a shipped harness.
    // Claiming it would mean a later removal takes out content this product
    // cannot prove it placed, which is the one direction section 3.24 never
    // allows. A marker resembling this product's satisfies one of the three
    // identifying properties and is not, on its own, proof of ownership.
    let mut unclaimed: Vec<String> = content
        .deny
        .iter()
        .filter(|entry| !placed_deny.contains(entry))
        .map(|entry| {
            format!(
                "deny {entry} is refused already and is not recorded as placed here, so removal \
                 will leave it"
            )
        })
        .collect();
    unclaimed.extend(
        content
            .hooks
            .iter()
            .filter(|hook| !placed_hooks.contains(hook))
            .map(|hook| {
                format!(
                    "hook {} `{}` is registered already, with the bytes this build places, and is \
                     not recorded as placed here; a marker is content and content can be copied, \
                     so removal will leave it",
                    hook.event, hook.matcher
                )
            }),
    );

    // Recorded after the write, and unconditionally: an apply interrupted
    // between the two leaves a region on disk with no record, and running it
    // again restores the record rather than writing the region twice.
    record(
        &mut modifications,
        Modification {
            path: display.clone(),
            kind: KIND.to_string(),
            harness: native.harness.clone(),
            revision: content.revision.clone(),
            hooks: placed_hooks,
            deny: placed_deny,
            digest_before: plan.digest_before.clone(),
            digest_after: plan.digest_after.clone(),
            recorded_at: now.to_string(),
        },
    );
    if let Err(e) = write_modifications(layout, &modifications) {
        return SettingsOutcome::Failed {
            path: display,
            reason: e.to_string(),
        };
    }

    SettingsOutcome::Applied {
        plan: Box::new(plan),
        superseded,
        unclaimed,
    }
}

// ---------------------------------------------------------------------------
// The splice
// ---------------------------------------------------------------------------

/// A byte span of the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: usize,
    end: usize,
}

/// Insert the region into the text, touching nothing else.
fn splice(text: &str, hooks: &[HookRegistration], deny: &[String]) -> Result<String, Refusal> {
    let unit = indent_unit(text);
    let mut out = text.to_string();

    // Deny first, then hooks: each edit is computed against the text it is
    // applied to, so they are applied one at a time rather than both against
    // the original offsets.
    if !deny.is_empty() {
        out = splice_deny(&out, deny, &unit)?;
    }
    for hook in hooks {
        out = splice_hook(&out, hook, &unit)?;
    }
    Ok(out)
}

fn splice_deny(text: &str, deny: &[String], unit: &str) -> Result<String, Refusal> {
    let root = root_span(text)?;
    let elements: Vec<String> = deny
        .iter()
        .map(|d| Value::String(d.clone()).to_string())
        .collect();

    if let Some(deny_span) = locate(text, root, &["permissions", "deny"]) {
        if text.as_bytes()[deny_span.start] != b'[' {
            return Err(Refusal::WrongShape {
                at: "permissions.deny".into(),
                found: "a value that is not an array".into(),
            });
        }
        return Ok(append_to_array(text, deny_span, &elements, unit));
    }
    if let Some(permissions_span) = locate(text, root, &["permissions"]) {
        if text.as_bytes()[permissions_span.start] != b'{' {
            return Err(Refusal::WrongShape {
                at: "permissions".into(),
                found: "a value that is not an object".into(),
            });
        }
        let value = pretty(&Value::from(deny.to_vec()), unit);
        return Ok(insert_member(text, permissions_span, "deny", &value, unit));
    }
    let mut permissions = serde_json::Map::new();
    permissions.insert("deny".into(), Value::from(deny.to_vec()));
    let value = pretty(&Value::Object(permissions), unit);
    Ok(insert_member(text, root, "permissions", &value, unit))
}

fn splice_hook(text: &str, hook: &HookRegistration, unit: &str) -> Result<String, Refusal> {
    let root = root_span(text)?;
    let group = pretty(&hook.group(), unit);

    if let Some(event_span) = locate(text, root, &["hooks", &hook.event]) {
        if text.as_bytes()[event_span.start] != b'[' {
            return Err(Refusal::WrongShape {
                at: format!("hooks.{}", hook.event),
                found: "a value that is not an array".into(),
            });
        }
        return Ok(append_to_array(text, event_span, &[group], unit));
    }
    if let Some(hooks_span) = locate(text, root, &["hooks"]) {
        if text.as_bytes()[hooks_span.start] != b'{' {
            return Err(Refusal::WrongShape {
                at: "hooks".into(),
                found: "a value that is not an object".into(),
            });
        }
        let value = pretty(&Value::Array(vec![hook.group()]), unit);
        return Ok(insert_member(text, hooks_span, &hook.event, &value, unit));
    }
    let mut events = serde_json::Map::new();
    events.insert(hook.event.clone(), Value::Array(vec![hook.group()]));
    let value = pretty(&Value::Object(events), unit);
    Ok(insert_member(text, root, "hooks", &value, unit))
}

/// Take the region back out, touching nothing else.
///
/// Returns the text and the containers that the removal emptied.
fn unsplice(
    text: &str,
    hooks: &[HookRegistration],
    deny_run: Option<std::ops::Range<usize>>,
) -> Result<(String, Vec<String>), Refusal> {
    let mut out = text.to_string();
    let mut emptied = Vec::new();

    for hook in hooks {
        let root = root_span(&out)?;
        let Some(event_span) = locate(&out, root, &["hooks", &hook.event]) else {
            continue;
        };
        let elements = array_elements(&out, event_span);
        let Some(index) = elements.iter().position(|e| {
            serde_json::from_str::<Value>(&out[e.start..e.end])
                .map(|v| commands_of(&v).iter().any(|c| c == &hook.command))
                .unwrap_or(false)
        }) else {
            continue;
        };
        out = remove_from_array(&out, event_span, index);

        // A container this product filled and that the removal emptied comes
        // out with the region rather than being left as residue.
        let root = root_span(&out)?;
        if let Some(span) = locate(&out, root, &["hooks", &hook.event])
            && array_elements(&out, span).is_empty()
        {
            out = remove_member(&out, root_span(&out)?, &["hooks", &hook.event]);
            emptied.push(format!("hooks.{}", hook.event));
        }
        let root = root_span(&out)?;
        if let Some(span) = locate(&out, root, &["hooks"])
            && object_members(&out, span).is_empty()
        {
            out = remove_member(&out, root_span(&out)?, &["hooks"]);
            emptied.push("hooks".into());
        }
    }

    if let Some(run) = deny_run
        && !run.is_empty()
    {
        let root = root_span(&out)?;
        if let Some(span) = locate(&out, root, &["permissions", "deny"]) {
            for _ in run.clone() {
                let span = locate(&out, root_span(&out)?, &["permissions", "deny"]).unwrap_or(span);
                out = remove_from_array(&out, span, run.start);
            }
            let root = root_span(&out)?;
            if let Some(span) = locate(&out, root, &["permissions", "deny"])
                && array_elements(&out, span).is_empty()
            {
                out = remove_member(&out, root_span(&out)?, &["permissions", "deny"]);
                emptied.push("permissions.deny".into());
                let root = root_span(&out)?;
                if let Some(span) = locate(&out, root, &["permissions"])
                    && object_members(&out, span).is_empty()
                {
                    out = remove_member(&out, root_span(&out)?, &["permissions"]);
                    emptied.push("permissions".into());
                }
            }
        }
    }

    Ok((out, emptied))
}

/// Pretty-print a value with the file's own indentation unit.
fn pretty(value: &Value, unit: &str) -> String {
    let mut buffer = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(unit.as_bytes());
    let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, formatter);
    serde::Serialize::serialize(value, &mut serializer).expect("a Value serializes");
    String::from_utf8(buffer).expect("serde_json emits UTF-8")
}

/// The file's own indentation unit, read from its first indented line.
fn indent_unit(text: &str) -> String {
    for line in text.lines().skip(1) {
        let indent: String = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        if !indent.is_empty() && indent.len() < line.len() {
            return indent;
        }
    }
    "  ".to_string()
}

/// The whitespace at the start of the line holding a byte offset.
fn line_indent(text: &str, at: usize) -> String {
    let start = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    text[start..at]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

/// The span of the whole document's root value.
fn root_span(text: &str) -> Result<Span, Refusal> {
    let mut cursor = Cursor {
        b: text.as_bytes(),
        i: 0,
    };
    cursor.value().ok_or(Refusal::Malformed {
        reason: "the document has no value".into(),
    })
}

/// A minimal JSON walker that yields byte spans.
///
/// Deliberately not a parser: `serde_json` has already established that the
/// text is valid JSON before anything here runs, and what this needs is the
/// offsets that `serde_json` does not keep.
struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Option<Span> {
        self.ws();
        let start = self.i;
        match *self.b.get(self.i)? {
            b'{' => self.balanced(b'{', b'}')?,
            b'[' => self.balanced(b'[', b']')?,
            b'"' => self.string()?,
            _ => {
                while self.i < self.b.len()
                    && !matches!(self.b[self.i], b',' | b'}' | b']')
                    && !(self.b[self.i] as char).is_ascii_whitespace()
                {
                    self.i += 1;
                }
            }
        }
        Some(Span { start, end: self.i })
    }

    fn string(&mut self) -> Option<()> {
        debug_assert_eq!(self.b[self.i], b'"');
        self.i += 1;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'\\' => self.i += 2,
                b'"' => {
                    self.i += 1;
                    return Some(());
                }
                _ => self.i += 1,
            }
        }
        None
    }

    fn balanced(&mut self, open: u8, close: u8) -> Option<()> {
        let mut depth = 0usize;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b'"' {
                self.string()?;
                continue;
            }
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                self.i += 1;
                if depth == 0 {
                    return Some(());
                }
                continue;
            }
            self.i += 1;
        }
        None
    }
}

/// One member of an object: the key, its value's span, and the member's own
/// span from the opening quote of the key to the end of the value.
#[derive(Debug, Clone)]
struct Member {
    key: String,
    value: Span,
    span: Span,
}

fn object_members(text: &str, object: Span) -> Vec<Member> {
    let b = text.as_bytes();
    if b.get(object.start) != Some(&b'{') {
        return Vec::new();
    }
    let mut cursor = Cursor {
        b: &b[..object.end - 1],
        i: object.start + 1,
    };
    let mut out = Vec::new();
    loop {
        cursor.ws();
        if cursor.i >= cursor.b.len() {
            break;
        }
        if cursor.b[cursor.i] == b',' {
            cursor.i += 1;
            continue;
        }
        let key_start = cursor.i;
        let Some(key_span) = cursor.value() else {
            break;
        };
        let key: String = match serde_json::from_str::<String>(&text[key_span.start..key_span.end])
        {
            Ok(k) => k,
            Err(_) => break,
        };
        cursor.ws();
        if cursor.b.get(cursor.i) != Some(&b':') {
            break;
        }
        cursor.i += 1;
        let Some(value) = cursor.value() else { break };
        out.push(Member {
            key,
            value,
            span: Span {
                start: key_start,
                end: value.end,
            },
        });
    }
    out
}

fn array_elements(text: &str, array: Span) -> Vec<Span> {
    let b = text.as_bytes();
    if b.get(array.start) != Some(&b'[') {
        return Vec::new();
    }
    let mut cursor = Cursor {
        b: &b[..array.end - 1],
        i: array.start + 1,
    };
    let mut out = Vec::new();
    loop {
        cursor.ws();
        if cursor.i >= cursor.b.len() {
            break;
        }
        if cursor.b[cursor.i] == b',' {
            cursor.i += 1;
            continue;
        }
        match cursor.value() {
            Some(span) => out.push(span),
            None => break,
        }
    }
    out
}

/// The span of the value at a dotted path under an object.
fn locate(text: &str, object: Span, path: &[&str]) -> Option<Span> {
    let mut current = object;
    for segment in path {
        let member = object_members(text, current)
            .into_iter()
            .find(|m| m.key == *segment)?;
        current = member.value;
    }
    Some(current)
}

/// Append elements to an array, preserving everything already in it.
fn append_to_array(text: &str, array: Span, elements: &[String], unit: &str) -> String {
    let existing = array_elements(text, array);
    let close_indent = line_indent(text, array.end - 1);
    let element_indent = match existing.first() {
        Some(first) => line_indent(text, first.start),
        None => format!("{close_indent}{unit}"),
    };

    let mut insertion = String::new();
    for element in elements {
        if !insertion.is_empty() || !existing.is_empty() {
            insertion.push(',');
        }
        insertion.push('\n');
        insertion.push_str(&element_indent);
        insertion.push_str(&reindent(element, &element_indent));
    }

    let at = match existing.last() {
        Some(last) => last.end,
        None => array.start + 1,
    };
    let mut out = String::with_capacity(text.len() + insertion.len() + 8);
    out.push_str(&text[..at]);
    out.push_str(&insertion);
    if existing.is_empty() {
        out.push('\n');
        out.push_str(&close_indent);
        out.push_str(&text[array.end - 1..]);
    } else {
        out.push_str(&text[at..]);
    }
    out
}

/// Remove one element from an array, preserving the rest exactly.
fn remove_from_array(text: &str, array: Span, index: usize) -> String {
    let existing = array_elements(text, array);
    let Some(element) = existing.get(index) else {
        return text.to_string();
    };
    // Take the element and the separator that attaches it: the comma before it
    // when it is not the first, otherwise the comma after it.
    let (start, end) = if index > 0 {
        let previous = existing[index - 1].end;
        (previous, element.end)
    } else if existing.len() > 1 {
        (element.start, existing[1].start)
    } else {
        // The only element: the array becomes empty, and the whitespace the
        // author put inside it goes with it.
        (array.start + 1, array.end - 1)
    };
    format!("{}{}", &text[..start], &text[end..])
}

/// Insert a member as the last one of an object, preserving the rest exactly.
fn insert_member(text: &str, object: Span, key: &str, value: &str, unit: &str) -> String {
    let existing = object_members(text, object);
    let close_indent = line_indent(text, object.end - 1);
    let member_indent = match existing.first() {
        Some(first) => line_indent(text, first.span.start),
        None => format!("{close_indent}{unit}"),
    };
    let rendered = format!(
        "{}: {}",
        Value::String(key.to_string()),
        reindent(value, &member_indent)
    );

    let at = match existing.last() {
        Some(last) => last.span.end,
        None => object.start + 1,
    };
    let mut out = String::with_capacity(text.len() + rendered.len() + 8);
    out.push_str(&text[..at]);
    if !existing.is_empty() {
        out.push(',');
    }
    out.push('\n');
    out.push_str(&member_indent);
    out.push_str(&rendered);
    if existing.is_empty() {
        out.push('\n');
        out.push_str(&close_indent);
        out.push_str(&text[object.end - 1..]);
    } else {
        out.push_str(&text[at..]);
    }
    out
}

/// Remove a member at a path, preserving the rest exactly.
fn remove_member(text: &str, root: Span, path: &[&str]) -> String {
    let (parent, key) = match path.split_last() {
        Some((key, parent)) => (
            match locate(text, root, parent) {
                Some(span) => span,
                None => return text.to_string(),
            },
            *key,
        ),
        None => return text.to_string(),
    };
    let members = object_members(text, parent);
    let Some(index) = members.iter().position(|m| m.key == key) else {
        return text.to_string();
    };
    let member = &members[index];
    let (start, end) = if index > 0 {
        (members[index - 1].span.end, member.span.end)
    } else if members.len() > 1 {
        (member.span.start, members[1].span.start)
    } else {
        (parent.start + 1, parent.end - 1)
    };
    format!("{}{}", &text[..start], &text[end..])
}

/// Re-indent a pretty-printed fragment so it sits at a given depth.
fn reindent(fragment: &str, indent: &str) -> String {
    let mut out = String::with_capacity(fragment.len());
    for (i, line) in fragment.lines().enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(indent);
        }
        out.push_str(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Managed {
        managed(
            "h-abcdef012345",
            Path::new("/home/.statecraft/harness/h-abcdef012345"),
        )
    }

    fn planned(existing: Option<&str>) -> Plan {
        plan("/n/.claude/settings.json", existing, &content()).expect("a plan")
    }

    #[test]
    fn every_hook_command_resolves_inside_the_canonical_harness_and_carries_its_marker() {
        let managed = content();
        assert_eq!(managed.hooks.len(), harness::ADOPTED_HOOKS.len());
        for (hook, adopted) in managed.hooks.iter().zip(harness::ADOPTED_HOOKS) {
            assert_eq!(hook.event, adopted.event);
            assert_eq!(hook.marked_revision(), Some("h-abcdef012345"));
            let command = hook.command.lines().nth(1).unwrap();
            // Section 3.24 rule 1: never a command assembled from anything
            // else. The executable path is inside this revision's directory
            // under the product home, and the revision is in the path.
            assert!(
                command.contains(&format!(
                    "/home/.statecraft/harness/h-abcdef012345/hooks/{}",
                    adopted.file
                )),
                "{command}"
            );
        }
    }

    #[test]
    fn the_consent_token_changes_when_the_content_does() {
        let a = content();
        let b = managed(
            "h-000000000000",
            Path::new("/home/.statecraft/harness/h-000000000000"),
        );
        assert_ne!(a.token(), b.token());
        assert_eq!(a.token(), content().token());
    }

    #[test]
    fn an_absent_file_is_created_holding_only_the_region() {
        let plan = planned(None);
        assert_eq!(plan.action, Action::Create);
        let value: Value = serde_json::from_str(&plan.contents_after).unwrap();
        assert!(value.get("hooks").is_some());
        // Section 3.27: the global modification carries hook registrations and
        // not the deny floor. A deny entry has no project gate, so one written
        // here would refuse in every repository the user opens.
        assert!(
            value.get("permissions").is_none(),
            "the floor reached a user's global settings: {value}"
        );
        assert_eq!(value.as_object().unwrap().len(), 1);
    }

    #[test]
    fn reapplication_changes_nothing() {
        let first = planned(None);
        let second = planned(Some(&first.contents_after));
        assert_eq!(second.action, Action::AlreadyCurrent);
        assert_eq!(second.contents_after, first.contents_after);
        assert!(!second.action.changes_the_file());
    }

    #[test]
    fn everything_outside_the_region_keeps_its_own_bytes() {
        let before = "{\n    \"$schema\": \"https://example/s.json\",\n    \"model\": \"opus\",\n    \"permissions\": {\n        \"allow\": [\n            \"Bash(ls *)\"\n        ],\n        \"deny\": [\n            \"Bash(rm -rf /*)\"\n        ]\n    }\n}\n";
        let plan = planned(Some(before));
        assert_eq!(plan.action, Action::Add);
        let after = &plan.contents_after;
        assert!(after.contains("\"$schema\": \"https://example/s.json\""));
        assert!(after.contains("\"model\": \"opus\""));
        assert!(after.contains("            \"Bash(ls *)\""));
        assert!(after.contains("\n    \"hooks\": {"), "{after}");
        // The user's own permissions block is untouched, entry for entry:
        // section 3.27 stops this modification writing into it at all.
        let value: Value = serde_json::from_str(after).unwrap();
        let deny = value
            .pointer("/permissions/deny")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(deny, &[Value::from("Bash(rm -rf /*)")]);
        let allow = value
            .pointer("/permissions/allow")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(allow, &[Value::from("Bash(ls *)")]);
    }

    #[test]
    fn the_global_modification_adds_no_deny_entry_at_all() {
        // Section 3.27. The floor is real and it is delivered per managed
        // session by `crate::session`; what it may not do is arrive in a
        // user's global settings, where it would apply to every repository.
        let before = "{\n  \"permissions\": {\n    \"deny\": [\n      \"Bash(cargo publish*)\"\n    ]\n  }\n}\n";
        let plan = planned(Some(before));
        assert!(plan.managed.deny.is_empty(), "{:?}", plan.managed.deny);
        assert!(plan.adding_deny.is_empty(), "{:?}", plan.adding_deny);
        let value: Value = serde_json::from_str(&plan.contents_after).unwrap();
        let deny = value
            .pointer("/permissions/deny")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(
            deny,
            &[Value::from("Bash(cargo publish*)")],
            "the user's own deny list was written to"
        );
    }

    #[test]
    fn a_users_own_hook_on_the_same_event_is_preserved_and_reported() {
        let before = "{\n  \"hooks\": {\n    \"SessionStart\": [\n      {\n        \"matcher\": \"startup\",\n        \"hooks\": [\n          { \"type\": \"command\", \"command\": \"echo mine\" }\n        ]\n      }\n    ]\n  }\n}\n";
        let plan = planned(Some(before));
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].event, "SessionStart");
        assert!(plan.contents_after.contains("echo mine"));
        let value: Value = serde_json::from_str(&plan.contents_after).unwrap();
        let groups = value
            .pointer("/hooks/SessionStart")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn malformed_settings_are_refused_and_nothing_is_computed() {
        let e = plan("/p", Some("{ not json"), &content()).unwrap_err();
        assert!(matches!(e, Refusal::Malformed { .. }));
        assert!(e.describe().contains("nothing is written"));
    }

    #[test]
    fn a_settings_file_that_is_not_an_object_is_refused() {
        let e = plan("/p", Some("[1, 2]"), &content()).unwrap_err();
        assert!(matches!(e, Refusal::NotAnObject { .. }));
    }

    #[test]
    fn a_changed_revision_replaces_the_old_registration_and_asks_again() {
        let old = managed("h-000000000000", Path::new("/h/harness/h-000000000000"));
        let first = plan("/p", None, &old).unwrap();
        let second = plan("/p", Some(&first.contents_after), &content()).unwrap();
        assert_eq!(second.action, Action::RevisionChanged);
        assert_ne!(second.consent_token, first.consent_token);
        let value: Value = serde_json::from_str(&second.contents_after).unwrap();
        let groups = value
            .pointer("/hooks/SessionStart")
            .unwrap()
            .as_array()
            .unwrap();
        // Both are present after the splice; the stale one comes out with its
        // own recorded removal, which is what keeps this from being a rewrite.
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn an_edited_managed_command_is_reported_and_left() {
        let first = planned(None);
        let tampered = first
            .contents_after
            .replace("statecraft-session-start.sh", "something-else.sh");
        let second = plan("/p", Some(&tampered), &content()).unwrap();
        assert!(!second.edited.is_empty());
        // The edited command is still there: this product does not take it back.
        assert!(second.contents_after.contains("something-else.sh"));
    }

    #[test]
    fn removal_takes_the_region_and_nothing_else() {
        let before = "{\n  \"model\": \"opus\",\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(ls *)\"\n    ],\n    \"deny\": [\n      \"Bash(rm -rf /*)\"\n    ]\n  }\n}\n";
        let managed = content();
        let plan = plan("/p", Some(before), &managed).unwrap();
        let record = Modification {
            path: "/p".into(),
            kind: KIND.into(),
            harness: "claude-code".into(),
            revision: managed.revision.clone(),
            hooks: plan.adding_hooks.clone(),
            deny: plan.adding_deny.clone(),
            digest_before: plan.digest_before.clone(),
            digest_after: plan.digest_after.clone(),
            recorded_at: "2026-09-21T00:00:00Z".into(),
        };
        let removal = removal(Some(&plan.contents_after), &record).unwrap();
        let Removal::Removed { contents_after, .. } = &removal else {
            panic!("expected a removal, got {removal:?}");
        };
        assert_eq!(contents_after, before);
    }

    #[test]
    fn removal_of_an_edited_region_reports_and_leaves_it() {
        let managed = content();
        let plan = planned(None);
        let record = Modification {
            path: "/p".into(),
            kind: KIND.into(),
            harness: "claude-code".into(),
            revision: managed.revision.clone(),
            hooks: plan.adding_hooks.clone(),
            deny: plan.adding_deny.clone(),
            digest_before: None,
            digest_after: plan.digest_after.clone(),
            recorded_at: "2026-09-21T00:00:00Z".into(),
        };
        let tampered = plan
            .contents_after
            .replace("statecraft-session-start.sh", "mine.sh");
        let removal = removal(Some(&tampered), &record).unwrap();
        assert!(matches!(removal, Removal::Edited { .. }));
    }

    #[test]
    fn removing_what_was_never_there_is_not_a_failure() {
        let managed = content();
        let record = Modification {
            path: "/p".into(),
            kind: KIND.into(),
            harness: "claude-code".into(),
            revision: managed.revision.clone(),
            hooks: managed.hooks.clone(),
            deny: managed.deny.clone(),
            digest_before: None,
            digest_after: String::new(),
            recorded_at: "2026-09-21T00:00:00Z".into(),
        };
        assert!(matches!(
            removal(None, &record).unwrap(),
            Removal::NotPresent { .. }
        ));
    }

    #[test]
    fn the_local_override_layer_is_named_and_never_reachable() {
        // Nothing here takes a path: the file this module writes is always
        // SETTINGS, and LOCAL_SETTINGS exists so a test can hold the rule.
        assert_ne!(SETTINGS, LOCAL_SETTINGS);
        let plan = planned(None);
        assert!(!plan.contents_after.contains(LOCAL_SETTINGS));
    }
}
