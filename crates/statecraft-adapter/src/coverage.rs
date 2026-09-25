//! The command allowance, the suite's programs, and the comparison between them.
//!
//! Spec 004 section 3.17. Section 3.5 row 8 requires that a posture declare
//! every command the run will need. That needs **two independent inputs**:
//!
//! - the **allowance**, which is what a posture allows: the adapter's own
//!   `requires_commands` plus the programs the repository declares in the
//!   project block of its committed `.statecraft/environment.json`, under
//!   `project.commands` (rule 1, read by [`declared_commands`]);
//! - the **requirement**, which is what the suite the attempt's spec declares
//!   actually names: `spec-spine verify <spec> --plan --json`, read by
//!   [`read_plan`] and parsed by [`parse_plan`], each command read by
//!   [`classify`] (rule 2).
//!
//! Neither is built from the other, which is rule 6: the product binding once
//! supplied the adapter manifest's commands as both, and the refusal could not
//! fire. [`Coverage::compare`] takes the two as separate arguments and nothing
//! in this module derives one from the other.
//!
//! # What the comparison can and cannot claim
//!
//! Rule 3, stated in [`LIMITS`] and carried on every recorded coverage: a
//! program one of the suite's commands runs in turn is not seen, and the
//! allowance is compared, not enforced. No verdict says `complete`.
//!
//! This module names no provider (section 3.8) and spawns only the suite
//! reader it is given: it never runs a suite command.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// The wrappers of rule 2: programs that run another program named in their
/// arguments. A command led by one is `unparsed`, because the program it runs
/// is not its first word. A closed list: adding one is an amendment to spec
/// 004 section 3.17.
pub const WRAPPERS: [&str; 19] = [
    "env", "exec", "command", "builtin", "xargs", "timeout", "nice", "nohup", "time", "sudo",
    "doas", "eval", "source", ".", "sh", "bash", "dash", "zsh", "ksh",
];

/// The two limits of rule 3, as words, carried on every recorded coverage and
/// in every rendering of a verdict.
pub const LIMITS: [&str; 2] = [
    "transitive programs are not seen: a program one of the suite's commands runs in turn \
     (make running cargo, cargo running rustc) is not read by this comparison",
    "the allowance is checked, not enforced: the child's PATH still resolves through the \
     operator's, which section 3.6's residuals name",
];

/// What the record says about enforcement, in one phrase (rule 3).
pub const ENFORCEMENT: &str = "checked, not enforced";

/// Where the declared allowance lives inside a target (rule 1).
pub const DECLARATION_PATH: &str = ".statecraft/environment.json";

/// The guard a launch refusal is concluded under (rule 4).
pub const GUARD: &str = "posture-coverage";

/// How one suite command was read (rule 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Reading {
    /// A simple command; its program is its first word.
    Program {
        /// The program `PATH` resolves.
        program: String,
    },
    /// A simple command whose first word contains `/`: it names a file, not a
    /// program `PATH` resolves, and requires no program directly.
    Path,
    /// Anything else. Not parsed, so not checked, and named as such.
    Unparsed,
}

/// Is `c` one of rule 2's metacharacters, outside a single-quoted span?
fn metacharacter(c: char) -> bool {
    matches!(
        c,
        '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' | '\\' | '\n' | '\r'
    )
}

/// Is `word` a bare program name: letters, digits and `_ . + -`, non-empty?
fn bare_program_name(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '-'))
}

/// Read one suite command (spec 004 section 3.17 rule 2).
///
/// A command is **simple** when, outside single-quoted spans (POSIX: every
/// character between two `'` is literal), it contains none of `|`, `&`, `;`,
/// `<`, `>`, `(`, `)`, `$`, a backquote, a backslash or a line break, every
/// single and double quote is closed, and its first word is a bare program name
/// that is not a wrapper. Its program is that first word. A simple command
/// whose first word contains `/` is a [`Reading::Path`]. Anything else is
/// [`Reading::Unparsed`]: a `NAME=value` assignment prefix, a wrapper, a
/// quoted or globbed first word, or any metacharacter.
///
/// Inside a double-quoted span a `'` is literal, as POSIX has it, so it opens
/// no single-quoted span; the metacharacters are still refused there, because
/// the rule exempts single-quoted spans only.
pub fn classify(command: &str) -> Reading {
    let mut single = false;
    let mut double = false;
    for c in command.chars() {
        if single {
            if c == '\'' {
                single = false;
            }
            continue;
        }
        match c {
            '\'' if !double => single = true,
            '"' => double = !double,
            c if metacharacter(c) => return Reading::Unparsed,
            _ => {}
        }
    }
    if single || double {
        return Reading::Unparsed;
    }
    let first = command
        .trim_start_matches([' ', '\t'])
        .split([' ', '\t'])
        .next()
        .unwrap_or_default();
    if bare_program_name(first) {
        if WRAPPERS.contains(&first) {
            return Reading::Unparsed;
        }
        return Reading::Program {
            program: first.to_string(),
        };
    }
    // A first word naming a file. An assignment (`A=/x cmd`) is not one, and a
    // quoted first word is not read at all.
    if first.contains('/') && !first.contains(['=', '\'', '"']) {
        return Reading::Path;
    }
    Reading::Unparsed
}

/// A fence tag the plan reports as declined, and how many blocks carried it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedBlocks {
    /// The fence tag as written, for example `verify:browser`.
    pub tag: String,
    /// How many blocks carried it.
    pub count: u64,
}

/// The suite a spec declares, as `spec-spine verify <spec> --plan --json`
/// reports it: the commands, without running them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuitePlan {
    /// The spec id the plan resolved.
    pub spec_id: String,
    /// The `## Verification` commands, in document order.
    pub commands: Vec<String>,
    /// Blocks the plan declines to run. They require nothing.
    pub skipped: Vec<SkippedBlocks>,
    /// The spec whose block these commands came from, when it is not this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_from: Option<String>,
}

impl SuitePlan {
    /// The bytes a plan digest is taken over: this plan's own serialization,
    /// so two readings that say the same thing digest the same whatever
    /// whitespace or member order the producer printed.
    pub fn canonical(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// Why a suite plan could not be read. Every case refuses the run (rule 2):
/// nothing is assumed about a suite that could not be read.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PlanError {
    /// The reader could not be started.
    #[error(
        "the suite plan could not be read: `{program} verify {spec} --plan --json` did not start: {detail}"
    )]
    NotRunnable {
        /// The reader program.
        program: String,
        /// The spec asked about.
        spec: String,
        /// The operating system's answer.
        detail: String,
    },
    /// The reader exited non-zero.
    #[error(
        "the suite plan could not be read: `{program} verify {spec} --plan --json` exited {status}: {detail}"
    )]
    Exited {
        /// The reader program.
        program: String,
        /// The spec asked about.
        spec: String,
        /// The exit, or `signal`.
        status: String,
        /// The first line it said.
        detail: String,
    },
    /// The reader did not answer within its deadline, and was killed.
    #[error(
        "the suite plan could not be read: `{program} verify {spec} --plan --json` did not answer within {seconds} seconds and was stopped"
    )]
    TimedOut {
        /// The reader program.
        program: String,
        /// The spec asked about.
        spec: String,
        /// The deadline.
        seconds: f64,
    },
    /// The reader answered something that does not parse as the plan.
    #[error("the suite plan could not be read: the answer does not parse as a verify plan: {0}")]
    Unreadable(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanEnvelope {
    verb: String,
    ok: bool,
    exit_code: i64,
    report: SuitePlan,
}

/// Parse `verify --plan --json`'s answer.
///
/// Strict where it matters: the envelope must name the `verify` verb, say
/// `ok`, carry exit code 0, and carry a report with `specId`, `commands` and
/// `skipped`. A missing `commands` is not an empty plan; an empty plan is only
/// one that says it is empty.
pub fn parse_plan(bytes: &[u8]) -> Result<SuitePlan, PlanError> {
    let envelope: PlanEnvelope =
        serde_json::from_slice(bytes).map_err(|e| PlanError::Unreadable(e.to_string()))?;
    if envelope.verb != "verify" {
        return Err(PlanError::Unreadable(format!(
            "the envelope names verb `{}`, not `verify`",
            envelope.verb
        )));
    }
    if !envelope.ok || envelope.exit_code != 0 {
        return Err(PlanError::Unreadable(format!(
            "the envelope says ok {} with exit code {}",
            envelope.ok, envelope.exit_code
        )));
    }
    Ok(envelope.report)
}

/// How long the suite reader may take before the read is refused.
///
/// Reading a plan runs nothing the plan names, so this is generous for a
/// read and short for a hang: a reader that does not answer is a plan that
/// could not be read (rule 2), never a plan assumed empty.
pub const PLAN_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

/// Ask `program` for the plan of `spec`, in `dir`, within [`PLAN_DEADLINE`].
///
/// `program` is the `spec-spine` this product invokes for work selection, and
/// `dir` is the tree being read: the target's working tree at planning, an
/// export of the base commit at launch (rule 4).
pub fn read_plan(program: &str, dir: &Path, spec: &str) -> Result<SuitePlan, PlanError> {
    read_plan_within(program, dir, spec, PLAN_DEADLINE)
}

/// [`read_plan`] under a named deadline. The reader runs under the
/// supervisor's own [`crate::supervisor::capture`]: its own process group,
/// killed with its descendants at the deadline. It is given this process's
/// environment, because it is the operator's tool reading the operator's tree,
/// not a child session.
pub fn read_plan_within(
    program: &str,
    dir: &Path,
    spec: &str,
    deadline: std::time::Duration,
) -> Result<SuitePlan, PlanError> {
    let environment: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let captured = crate::supervisor::capture(
        Path::new(program),
        &["verify", spec, "--plan", "--json"],
        dir,
        &environment,
        b"",
        deadline,
    )
    .map_err(|e| PlanError::NotRunnable {
        program: program.to_string(),
        spec: spec.to_string(),
        detail: e.to_string(),
    })?;
    if captured.timed_out {
        return Err(PlanError::TimedOut {
            program: program.to_string(),
            spec: spec.to_string(),
            seconds: deadline.as_secs_f64(),
        });
    }
    if captured.code != Some(0) {
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&captured.stderr),
            String::from_utf8_lossy(&captured.stdout)
        );
        return Err(PlanError::Exited {
            program: program.to_string(),
            spec: spec.to_string(),
            status: captured
                .code
                .map_or_else(|| "signal".to_string(), |c| format!("exit {c}")),
            detail: said
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("no detail")
                .to_string(),
        });
    }
    parse_plan(&captured.stdout)
}

/// What the committed project block declares (rule 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declared {
    /// No declaration file, or no `project.commands` member: nothing declared.
    Absent,
    /// The declared program names, as written.
    Commands(Vec<String>),
}

impl Declared {
    /// The bytes the declaration digest is taken over: the list's own
    /// serialization, or `None` when nothing is declared.
    pub fn canonical(&self) -> Option<Vec<u8>> {
        match self {
            Declared::Absent => None,
            Declared::Commands(list) => serde_json::to_vec(list).ok(),
        }
    }
}

/// Read `project.commands` from the bytes of a declaration file.
///
/// `None` is a file that does not exist, which declares nothing. A file that
/// does not parse, a `project` that is not an object, a `commands` that is not
/// a list of strings, and an entry that is empty, holds a `/` or whitespace,
/// or repeats another, each refuse and are named: rule 1 says a malformed
/// entry refuses the run.
pub fn declared_commands(file: Option<&[u8]>) -> Result<Declared, String> {
    let Some(bytes) = file else {
        return Ok(Declared::Absent);
    };
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("{DECLARATION_PATH} does not parse as JSON: {e}"))?;
    let Some(project) = value.get("project") else {
        return Ok(Declared::Absent);
    };
    if !project.is_object() {
        return Err(format!("{DECLARATION_PATH}: `project` is not an object"));
    }
    let Some(commands) = project.get("commands") else {
        return Ok(Declared::Absent);
    };
    let Some(list) = commands.as_array() else {
        return Err(format!(
            "{DECLARATION_PATH}: `project.commands` is not a list of program names"
        ));
    };
    let mut out: Vec<String> = Vec::with_capacity(list.len());
    for (i, entry) in list.iter().enumerate() {
        let Some(name) = entry.as_str() else {
            return Err(format!(
                "{DECLARATION_PATH}: `project.commands[{i}]` is {entry}, not a program name"
            ));
        };
        let why = if name.is_empty() {
            Some("is empty")
        } else if name.contains('/') {
            Some("holds a `/`; the allowance names programs, not files")
        } else if name.chars().any(char::is_whitespace) {
            Some("holds whitespace")
        } else if out.iter().any(|n| n == name) {
            Some("is declared twice")
        } else {
            None
        };
        if let Some(why) = why {
            return Err(format!(
                "{DECLARATION_PATH}: `project.commands[{i}]` {name:?} {why}"
            ));
        }
        out.push(name.to_string());
    }
    Ok(Declared::Commands(out))
}

/// Where an allowance entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// The adapter manifest's own `requires_commands`.
    Adapter,
    /// The committed project block's `commands`.
    Declared,
}

/// One program a posture allows, and where the allowance came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowanceEntry {
    /// The program name.
    pub program: String,
    /// Adapter or declared.
    pub source: Source,
}

/// What the declaration was, when it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum DeclarationRecord {
    /// Nothing was declared.
    Absent,
    /// A list was declared; this is its digest.
    Declared {
        /// The digest of the declared list.
        digest: String,
    },
}

/// The allowance: the adapter's own commands plus the declared ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allowance {
    /// Every allowed program with its source. The product adds nothing here on
    /// its own account.
    pub entries: Vec<AllowanceEntry>,
    /// What the declaration was.
    pub declaration: DeclarationRecord,
}

impl Allowance {
    /// Build the allowance from its two sources. `declaration_digest` is the
    /// digest of [`Declared::canonical`], taken by the caller.
    pub fn new(
        adapter_commands: &[String],
        declared: &Declared,
        declaration_digest: Option<String>,
    ) -> Self {
        let mut entries: Vec<AllowanceEntry> = adapter_commands
            .iter()
            .map(|p| AllowanceEntry {
                program: p.clone(),
                source: Source::Adapter,
            })
            .collect();
        if let Declared::Commands(list) = declared {
            entries.extend(list.iter().map(|p| AllowanceEntry {
                program: p.clone(),
                source: Source::Declared,
            }));
        }
        let declaration = match (declared, declaration_digest) {
            (Declared::Commands(_), Some(digest)) => DeclarationRecord::Declared { digest },
            (Declared::Commands(_), None) => DeclarationRecord::Declared {
                digest: String::new(),
            },
            (Declared::Absent, _) => DeclarationRecord::Absent,
        };
        Self {
            entries,
            declaration,
        }
    }

    /// Whether `program` is allowed.
    pub fn allows(&self, program: &str) -> bool {
        self.entries.iter().any(|e| e.program == program)
    }

    /// The bytes the allowance digest is taken over.
    pub fn canonical(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// One suite command and how it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandReading {
    /// The command as the plan reported it.
    pub command: String,
    /// Its program, `path` or `unparsed`.
    pub reading: Reading,
}

/// A required program the allowance lacks, and the commands naming it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Missing {
    /// The program.
    pub program: String,
    /// Every suite command whose program it is.
    pub commands: Vec<String>,
}

/// The coverage verdict (rule 3). No verdict says `complete`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// A required program is missing. The run is refused.
    Refused,
    /// Nothing is missing, and at least one command was not parsed.
    Partial,
    /// Nothing is missing, and every command was parsed.
    Direct,
    /// The attempt has no spec, so nothing was compared.
    NotApplicable,
}

impl Verdict {
    /// The word it is recorded and rendered as.
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Refused => "refused",
            Verdict::Partial => "partial",
            Verdict::Direct => "direct",
            Verdict::NotApplicable => "not-applicable",
        }
    }
}

/// The recorded comparison (rule 5): what was read, what was allowed, what was
/// missing, the verdict, and the two limits of rule 3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    /// The attempt's spec, where it has one.
    pub spec: Option<String>,
    /// The base commit the reading was taken at, where it was.
    pub base_commit: Option<String>,
    /// The digest of the suite plan read.
    pub plan_digest: Option<String>,
    /// Each suite command with its program, `path` or `unparsed`.
    pub commands: Vec<CommandReading>,
    /// Blocks the plan declined. They require nothing.
    pub skipped: Vec<SkippedBlocks>,
    /// The allowance, each entry with its source.
    pub allowance: Vec<AllowanceEntry>,
    /// The declaration as read at the base: its digest, or absent.
    pub declaration: DeclarationRecord,
    /// The digest of the allowance read.
    pub allowance_digest: Option<String>,
    /// The required programs the allowance lacks.
    pub missing: Vec<Missing>,
    /// The verdict.
    pub verdict: Verdict,
    /// Which planning digests the launch reading differs from (`allowance`,
    /// `suite plan`). Non-empty refuses the run whatever the verdict.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drift: Vec<String>,
    /// `checked, not enforced`.
    pub enforcement: String,
    /// The two limits of rule 3, as words.
    pub limits: Vec<String>,
}

impl Coverage {
    /// Compare a suite plan against an allowance.
    ///
    /// The two inputs are separate arguments and neither is derived from the
    /// other here; that is rule 6.
    pub fn compare(
        spec: &str,
        base_commit: Option<&str>,
        plan: &SuitePlan,
        plan_digest: String,
        allowance: &Allowance,
        allowance_digest: String,
    ) -> Self {
        let commands: Vec<CommandReading> = plan
            .commands
            .iter()
            .map(|c| CommandReading {
                command: c.clone(),
                reading: classify(c),
            })
            .collect();
        let mut missing: Vec<Missing> = Vec::new();
        for c in &commands {
            if let Reading::Program { program } = &c.reading {
                if allowance.allows(program) {
                    continue;
                }
                match missing.iter_mut().find(|m| &m.program == program) {
                    Some(m) => m.commands.push(c.command.clone()),
                    None => missing.push(Missing {
                        program: program.clone(),
                        commands: vec![c.command.clone()],
                    }),
                }
            }
        }
        let unparsed = commands.iter().any(|c| c.reading == Reading::Unparsed);
        let verdict = if !missing.is_empty() {
            Verdict::Refused
        } else if unparsed {
            Verdict::Partial
        } else {
            Verdict::Direct
        };
        Self {
            spec: Some(spec.to_string()),
            base_commit: base_commit.map(str::to_string),
            plan_digest: Some(plan_digest),
            commands,
            skipped: plan.skipped.clone(),
            allowance: allowance.entries.clone(),
            declaration: allowance.declaration.clone(),
            allowance_digest: Some(allowance_digest),
            missing,
            verdict,
            drift: Vec::new(),
            enforcement: ENFORCEMENT.to_string(),
            limits: LIMITS.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// The attempt has no spec: nothing was compared (the managed-startup
    /// trial of spec 002 section 3.33).
    pub fn not_applicable(allowance: &Allowance) -> Self {
        Self {
            spec: None,
            base_commit: None,
            plan_digest: None,
            commands: Vec::new(),
            skipped: Vec::new(),
            allowance: allowance.entries.clone(),
            declaration: allowance.declaration.clone(),
            allowance_digest: None,
            missing: Vec::new(),
            verdict: Verdict::NotApplicable,
            drift: Vec::new(),
            enforcement: ENFORCEMENT.to_string(),
            limits: LIMITS.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// The programs the parsed commands require, in first-seen order.
    pub fn required_programs(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in &self.commands {
            if let Reading::Program { program } = &c.reading
                && !out.contains(program)
            {
                out.push(program.clone());
            }
        }
        out
    }

    /// The commands named as not checked.
    pub fn unparsed(&self) -> Vec<&str> {
        self.commands
            .iter()
            .filter(|c| c.reading == Reading::Unparsed)
            .map(|c| c.command.as_str())
            .collect()
    }

    /// Why this coverage refuses the run, or `None`.
    pub fn refusal(&self) -> Option<String> {
        let mut parts = Vec::new();
        if self.verdict == Verdict::Refused {
            let named: Vec<String> = self
                .missing
                .iter()
                .map(|m| {
                    format!(
                        "`{}` (named by {})",
                        m.program,
                        m.commands
                            .iter()
                            .map(|c| format!("`{c}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect();
            parts.push(format!(
                "the suite of {} requires programs the posture does not allow: {}; declare each \
                 in `project.commands` of the committed {DECLARATION_PATH}",
                self.spec.as_deref().unwrap_or("this attempt"),
                named.join("; ")
            ));
        }
        if !self.drift.is_empty() {
            parts.push(format!(
                "the {} read at the base commit {} differs by digest from what planning read in \
                 the working tree; commit the change, or discard it, and run again",
                self.drift.join(" and the "),
                self.base_commit.as_deref().unwrap_or("unknown")
            ));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        }
    }

    /// A rendering an operator can read.
    pub fn render(&self) -> String {
        let mut out = format!(
            "command coverage: {} ({})\n",
            self.verdict.word(),
            self.enforcement
        );
        if let Some(spec) = &self.spec {
            out.push_str(&format!(
                "  suite of {spec} at {} (plan {})\n",
                self.base_commit.as_deref().unwrap_or("the working tree"),
                self.plan_digest.as_deref().unwrap_or("not read")
            ));
        } else {
            out.push_str("  no spec: nothing was compared\n");
        }
        for c in &self.commands {
            let reading = match &c.reading {
                Reading::Program { program } => program.clone(),
                Reading::Path => "path".to_string(),
                Reading::Unparsed => "unparsed, not checked".to_string(),
            };
            out.push_str(&format!("  {} -> {reading}\n", c.command));
        }
        for s in &self.skipped {
            out.push_str(&format!("  skipped: {} x{}\n", s.tag, s.count));
        }
        let allowed: Vec<String> = self
            .allowance
            .iter()
            .map(|e| {
                format!(
                    "{} ({})",
                    e.program,
                    match e.source {
                        Source::Adapter => "adapter",
                        Source::Declared => "declared",
                    }
                )
            })
            .collect();
        out.push_str(&format!("  allowance: {}\n", allowed.join(", ")));
        out.push_str(&match &self.declaration {
            DeclarationRecord::Absent => "  declaration: absent\n".to_string(),
            DeclarationRecord::Declared { digest } => format!("  declaration: {digest}\n"),
        });
        for m in &self.missing {
            out.push_str(&format!(
                "  missing: {} (named by {})\n",
                m.program,
                m.commands.join(", ")
            ));
        }
        for d in &self.drift {
            out.push_str(&format!("  changed since planning: {d}\n"));
        }
        for l in &self.limits {
            out.push_str(&format!("  limit: {l}\n"));
        }
        out
    }
}

/// What an attempt's posture records about coverage.
///
/// An attempt written before spec 004 section 3.17 has no `coverage` and reads
/// as [`CoverageRecord::NotChecked`], never as covered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CoverageRecord {
    /// The comparison that was made.
    Checked(Box<Coverage>),
    /// No comparison was made because an input could not be read at launch;
    /// the attempt was refused for it, and this says why.
    Unread(Unread),
    /// No comparison was recorded.
    NotChecked(NotChecked),
}

/// Why the launch reading could not be made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unread {
    /// The refusal, as the attempt's refusal account names it.
    pub unread: String,
}

/// The one word for an attempt with no recorded comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotChecked {
    /// `not-checked`.
    NotChecked,
}

impl Default for CoverageRecord {
    fn default() -> Self {
        CoverageRecord::NotChecked(NotChecked::NotChecked)
    }
}

impl CoverageRecord {
    /// A rendering an operator can read.
    pub fn render(&self) -> String {
        match self {
            CoverageRecord::Checked(c) => c.render(),
            CoverageRecord::Unread(u) => format!(
                "command coverage: not checked, and the attempt was refused under {GUARD}: {}\n",
                u.unread
            ),
            CoverageRecord::NotChecked(_) => {
                "command coverage: not checked (no comparison is recorded for this attempt)\n"
                    .to_string()
            }
        }
    }

    /// The recorded coverage, if any.
    pub fn checked(&self) -> Option<&Coverage> {
        match self {
            CoverageRecord::Checked(c) => Some(c),
            CoverageRecord::Unread(_) | CoverageRecord::NotChecked(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(p: &str) -> Reading {
        Reading::Program {
            program: p.to_string(),
        }
    }

    #[test]
    fn a_plain_command_yields_its_first_word() {
        assert_eq!(
            classify("cargo test --workspace --locked"),
            program("cargo")
        );
        assert_eq!(classify("  make gate"), program("make"));
        assert_eq!(classify("test -f a/b.rs"), program("test"));
        assert_eq!(classify("python3.12 x.py"), program("python3.12"));
        assert_eq!(classify("g++ -v"), program("g++"));
    }

    #[test]
    fn each_metacharacter_outside_single_quotes_is_unparsed() {
        for c in [
            "a | b", "a & b", "a && b", "a; b", "a < f", "a > f", "(a)", "a $(b)", "a $X", "a `b`",
            "a \\n", "a\nb", "a\rb",
        ] {
            assert_eq!(classify(c), Reading::Unparsed, "{c:?}");
        }
    }

    #[test]
    fn metacharacters_inside_single_quotes_are_literal_and_inside_double_quotes_are_not() {
        assert_eq!(
            classify("grep -qE '^## 5\\. (a|b)$' specs/x.md"),
            program("grep")
        );
        assert_eq!(classify("grep -q \"a|b\" f"), Reading::Unparsed);
        assert_eq!(classify("grep -q \"it's\" f"), program("grep"));
    }

    #[test]
    fn an_unclosed_quote_is_unparsed() {
        assert_eq!(classify("grep 'abc f"), Reading::Unparsed);
        assert_eq!(classify("grep \"abc f"), Reading::Unparsed);
    }

    #[test]
    fn an_assignment_prefix_is_unparsed() {
        assert_eq!(classify("FOO=1 cargo test"), Reading::Unparsed);
        assert_eq!(classify("PATH=/x/bin cargo test"), Reading::Unparsed);
    }

    #[test]
    fn every_wrapper_is_unparsed() {
        for w in WRAPPERS {
            assert_eq!(
                classify(&format!("{w} cargo test")),
                Reading::Unparsed,
                "{w}"
            );
        }
        assert_eq!(WRAPPERS.len(), 19);
    }

    #[test]
    fn a_first_word_with_a_slash_is_a_path() {
        assert_eq!(classify("./scripts/check.sh --all"), Reading::Path);
        assert_eq!(classify("/usr/bin/env cargo"), Reading::Path);
        assert_eq!(classify(".tooling/bin/spec-spine check"), Reading::Path);
    }

    #[test]
    fn a_quoted_or_odd_first_word_is_unparsed() {
        assert_eq!(classify("'cargo' test"), Reading::Unparsed);
        assert_eq!(classify("! cargo test"), Reading::Unparsed);
        assert_eq!(classify("{ cargo; }"), Reading::Unparsed);
        assert_eq!(classify(""), Reading::Unparsed);
    }

    fn plan(commands: &[&str]) -> SuitePlan {
        SuitePlan {
            spec_id: "007-x".into(),
            commands: commands.iter().map(|c| (*c).to_string()).collect(),
            skipped: vec![],
            acceptance_from: None,
        }
    }

    fn adapter() -> Vec<String> {
        vec!["agent".into(), "git".into()]
    }

    #[test]
    fn a_requirement_and_an_allowance_from_different_inputs_can_disagree() {
        // The requirement is read from a plan; the allowance from the adapter
        // and a declaration. Neither is built from the other, so a program the
        // suite names and the posture lacks is missing: the refusal can fire.
        let allowance = Allowance::new(&adapter(), &Declared::Absent, None);
        let c = Coverage::compare(
            "007-x",
            Some("abc"),
            &plan(&["cargo test", "git status", "cargo build"]),
            "p".into(),
            &allowance,
            "a".into(),
        );
        assert_eq!(c.verdict, Verdict::Refused);
        assert_eq!(
            c.missing,
            [Missing {
                program: "cargo".into(),
                commands: vec!["cargo test".into(), "cargo build".into()],
            }]
        );
        let why = c.refusal().unwrap();
        assert!(
            why.contains("`cargo`") && why.contains("project.commands"),
            "{why}"
        );
    }

    #[test]
    fn a_declared_program_covers_the_requirement() {
        let declared = Declared::Commands(vec!["cargo".into()]);
        let allowance = Allowance::new(&adapter(), &declared, Some("d".into()));
        let c = Coverage::compare(
            "007-x",
            None,
            &plan(&["cargo test"]),
            "p".into(),
            &allowance,
            "a".into(),
        );
        assert_eq!(c.verdict, Verdict::Direct);
        assert!(c.refusal().is_none());
        assert_eq!(c.allowance[2].source, Source::Declared);
    }

    #[test]
    fn an_unparsed_command_makes_a_covered_suite_partial_and_a_path_requires_nothing() {
        let allowance = Allowance::new(&adapter(), &Declared::Absent, None);
        let c = Coverage::compare(
            "007-x",
            None,
            &plan(&["git log | head", "./check.sh"]),
            "p".into(),
            &allowance,
            "a".into(),
        );
        assert_eq!(c.verdict, Verdict::Partial);
        assert_eq!(c.unparsed(), ["git log | head"]);
        let d = Coverage::compare(
            "007-x",
            None,
            &plan(&["./check.sh"]),
            "p".into(),
            &allowance,
            "a".into(),
        );
        assert_eq!(d.verdict, Verdict::Direct);
    }

    #[test]
    fn an_empty_plan_is_direct_and_no_verdict_says_complete() {
        let allowance = Allowance::new(&adapter(), &Declared::Absent, None);
        let c = Coverage::compare(
            "007-x",
            None,
            &plan(&[]),
            "p".into(),
            &allowance,
            "a".into(),
        );
        assert_eq!(c.verdict, Verdict::Direct);
        for v in [
            Verdict::Refused,
            Verdict::Partial,
            Verdict::Direct,
            Verdict::NotApplicable,
        ] {
            assert_ne!(v.word(), "complete");
        }
        assert!(c.render().contains("checked, not enforced"));
        assert!(c.render().contains("transitive programs are not seen"));
    }

    #[test]
    fn drift_refuses_whatever_the_verdict() {
        let allowance = Allowance::new(&adapter(), &Declared::Absent, None);
        let mut c = Coverage::compare(
            "007-x",
            None,
            &plan(&[]),
            "p".into(),
            &allowance,
            "a".into(),
        );
        c.drift.push("allowance".into());
        assert!(c.refusal().unwrap().contains("allowance"));
    }

    #[test]
    fn the_plan_parses_strictly() {
        let good = br#"{"exitCode":0,"ok":true,"report":{"commands":["cargo test"],"skipped":[{"tag":"verify:browser","count":2}],"specId":"007-x"},"schemaVersion":"0.6.0","verb":"verify"}"#;
        let p = parse_plan(good).unwrap();
        assert_eq!(p.commands, ["cargo test"]);
        assert_eq!(p.skipped[0].count, 2);
        for bad in [
            &b"not json"[..],
            br#"{"exitCode":0,"ok":true,"report":{"skipped":[],"specId":"x"},"verb":"verify"}"#,
            br#"{"exitCode":1,"ok":false,"report":{"commands":[],"skipped":[],"specId":"x"},"verb":"verify"}"#,
            br#"{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"x"},"verb":"check"}"#,
            br#"{"exitCode":0,"ok":true,"report":{"commands":"cargo","skipped":[],"specId":"x"},"verb":"verify"}"#,
        ] {
            assert!(parse_plan(bad).is_err(), "{}", String::from_utf8_lossy(bad));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_reader_that_hangs_is_stopped_at_its_deadline_and_the_plan_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let reader = dir.path().join("hanging-spec-spine");
        crate::fixture::install_script(&reader, "#!/bin/sh\nsleep 300 &\nexec sleep 300\n", 0o755)
            .unwrap();
        let started = std::time::Instant::now();
        let err = read_plan_within(
            reader.to_str().unwrap(),
            dir.path(),
            "007-x",
            std::time::Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(matches!(err, PlanError::TimedOut { .. }), "{err}");
        assert!(
            err.to_string().contains("did not answer within 1 seconds"),
            "{err}"
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(30));
    }

    #[cfg(unix)]
    #[test]
    fn a_reader_that_answers_in_time_is_parsed_and_one_that_fails_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let reader = dir.path().join("spec-spine");
        crate::fixture::install_script(
            &reader,
            "#!/bin/sh\n[ \"$*\" = 'verify 007-x --plan --json' ] || { echo nope >&2; exit 2; }\necho '{\"exitCode\":0,\"ok\":true,\"report\":{\"commands\":[\"make gate\"],\"skipped\":[],\"specId\":\"007-x\"},\"verb\":\"verify\"}'\n",
            0o755,
        )
        .unwrap();
        let program = reader.to_str().unwrap();
        let plan = read_plan(program, dir.path(), "007-x").unwrap();
        assert_eq!(plan.commands, ["make gate"]);
        let err = read_plan(program, dir.path(), "008-y").unwrap_err();
        assert!(err.to_string().contains("exit 2: nope"), "{err}");
    }

    #[test]
    fn the_declaration_is_read_and_malformed_entries_are_named() {
        assert_eq!(declared_commands(None).unwrap(), Declared::Absent);
        assert_eq!(
            declared_commands(Some(br#"{"version":2}"#)).unwrap(),
            Declared::Absent
        );
        assert_eq!(
            declared_commands(Some(br#"{"project":{}}"#)).unwrap(),
            Declared::Absent
        );
        assert_eq!(
            declared_commands(Some(br#"{"project":{"commands":["cargo","make"]}}"#)).unwrap(),
            Declared::Commands(vec!["cargo".into(), "make".into()])
        );
        for (bad, named) in [
            (
                &br#"{"project":{"commands":["/usr/bin/cargo"]}}"#[..],
                "/usr/bin/cargo",
            ),
            (br#"{"project":{"commands":["cargo make"]}}"#, "cargo make"),
            (br#"{"project":{"commands":[""]}}"#, "empty"),
            (br#"{"project":{"commands":["make","make"]}}"#, "twice"),
            (br#"{"project":{"commands":[3]}}"#, "commands[0]"),
            (br#"{"project":{"commands":"cargo"}}"#, "not a list"),
            (br#"{"project":[]}"#, "not an object"),
            (b"{", "does not parse"),
        ] {
            let err = declared_commands(Some(bad)).unwrap_err();
            assert!(err.contains(named), "{err}");
        }
    }

    #[test]
    fn an_attempt_recorded_before_this_section_reads_as_not_checked() {
        let record: CoverageRecord =
            serde_json::from_value(serde_json::json!("not-checked")).unwrap();
        assert_eq!(record, CoverageRecord::default());
        assert!(record.render().contains("not checked"));
        let allowance = Allowance::new(&adapter(), &Declared::Absent, None);
        let checked = CoverageRecord::Checked(Box::new(Coverage::not_applicable(&allowance)));
        let back: CoverageRecord =
            serde_json::from_value(serde_json::to_value(&checked).unwrap()).unwrap();
        assert_eq!(back, checked);
        assert!(back.render().contains("not-applicable"));
        let unread = CoverageRecord::Unread(Unread {
            unread: "the suite plan could not be read".into(),
        });
        let back: CoverageRecord =
            serde_json::from_value(serde_json::to_value(&unread).unwrap()).unwrap();
        assert_eq!(back, unread);
        let line = back.render();
        assert!(
            line.contains("not checked") && line.contains("posture-coverage"),
            "{line}"
        );
        assert!(line.contains("could not be read"), "{line}");
    }
}
