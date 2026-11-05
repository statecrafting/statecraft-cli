//! What a capture of each scenario asks the provider, and how its stream is
//! redacted before it is written.
//!
//! Spec 052 section 3.5. The capture verb runs each of the five scenarios
//! against the installed provider; this module holds everything about that
//! run that is a value rather than an act, so the owner reviews one table and
//! one pure function before any provider usage is spent:
//!
//! - [`SCENARIOS`]: per scenario, the prompt, the flags and settings, and the
//!   invocation string its manifest entry carries, which is the one
//!   `corpus.json` already records for that scenario;
//! - [`SCRATCH_FILES`]: what the scratch git work tree holds;
//! - [`redact`]: the stream README's "What was dropped, and what was
//!   redacted", applied to raw stream lines, and [`REDACTIONS`], the list
//!   each entry carries.
//!
//! Nothing here spawns. The binding in `statecraft-cli` creates the scratch
//! tree, spawns the provider, delivers the prompt on standard input (never on
//! the command line, spec 004 section 3.1) and writes what [`redact`] returns.

use crate::golden::{Capture, VERBATIM_TOOLS};
use crate::spawn::BASE_ARGS;
use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use std::fmt;

/// One scenario, as the capture verb runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scenario {
    /// The scenario's name, as `corpus.json` names it.
    pub name: &'static str,
    /// The prompt, delivered on standard input.
    pub prompt: &'static str,
    /// `--max-turns`, when the scenario caps turns.
    pub max_turns: Option<u32>,
    /// `--disallowedTools`, when the scenario removes a tool.
    pub disallowed_tools: Option<&'static str>,
    /// `permissions.deny` rules, written to a `--settings` file when present.
    pub deny: &'static [&'static str],
    /// Run in a constructed environment carrying only `PATH`.
    pub only_path: bool,
    /// The manifest entry's invocation, exactly as `corpus.json` records it.
    pub invocation: &'static str,
}

/// The five scenarios, in `corpus.json` order. **The prompts are the review
/// surface**: each is short and asks for one deterministic behaviour.
pub const SCENARIOS: [Scenario; 5] = [
    Scenario {
        name: "success",
        prompt: "Reply with the single word OK and nothing else. Do not use any tool.",
        max_turns: None,
        disallowed_tools: None,
        deny: &[],
        only_path: false,
        invocation: "claude --print --output-format stream-json --verbose, in a scratch git work tree; plain",
    },
    Scenario {
        name: "denied",
        prompt: "Use the Bash tool to run the command `echo hello`, then reply with what it printed.",
        max_turns: None,
        disallowed_tools: None,
        deny: &["Bash(echo:*)"],
        only_path: false,
        invocation: "claude --print --output-format stream-json --verbose, in a scratch git work tree; settings permissions.deny [\"Bash(echo:*)\"]",
    },
    Scenario {
        name: "max-turns",
        prompt: "Use the Read tool three times, one call per file, to read one.txt, two.txt and three.txt in the current directory, then reply with their contents.",
        max_turns: Some(1),
        disallowed_tools: None,
        deny: &[],
        only_path: false,
        invocation: "claude --print --output-format stream-json --verbose, in a scratch git work tree; --max-turns 1, on a prompt needing three tool calls",
    },
    Scenario {
        name: "tool-removed",
        prompt: "Use the Bash tool to run the command `echo hello`, then reply with what it printed.",
        max_turns: None,
        disallowed_tools: Some("Bash"),
        deny: &[],
        only_path: false,
        invocation: "claude --print --output-format stream-json --verbose, in a scratch git work tree; --disallowedTools Bash",
    },
    Scenario {
        name: "api-error",
        prompt: "Reply with the single word OK and nothing else. Do not use any tool.",
        max_turns: None,
        disallowed_tools: None,
        deny: &[],
        only_path: true,
        invocation: "claude --print --output-format stream-json --verbose, in a scratch git work tree; plain, in a constructed environment carrying only PATH",
    },
];

/// The files the scratch git work tree holds before every scenario, so the
/// `max-turns` prompt has three files to read.
pub const SCRATCH_FILES: [(&str, &str); 3] = [
    ("one.txt", "one\n"),
    ("two.txt", "two\n"),
    ("three.txt", "three\n"),
];

/// What every capture's manifest entry lists as redacted: `corpus.json`'s
/// list for the five goldens, then the rule Claude Code 2.1.318's `init`
/// fields made necessary (the stream README).
pub const REDACTIONS: [&str; 6] = [
    "dropped: system events of subtype thinking_tokens, and rate_limit_event events",
    "dropped: assistant events whose only content block is thinking",
    "fixed values: session_id, uuid, hook_id, timestamp",
    "placeholder path: cwd, memory_paths, messaging_socket_path; plugins emptied",
    "placeholder path: original_cwd, and the scenario's work tree wherever a string names it; additional_directories emptied",
    "count-preserving placeholders: names in tools, skills, agents, slash_commands, terminal_slash_commands, mcp_servers, except Bash, Read, Edit, Write, Glob, Grep, Task",
];

/// The fixed `session_id`, as the goldens spell it.
pub const FIXED_SESSION_ID: &str = "11111111-1111-1111-1111-111111111111";
/// The fixed `uuid` and `hook_id`, as the goldens spell them.
pub const FIXED_ID: &str = "00000000-0000-0000-0000-000000000000";
/// The placeholder `cwd`, as the goldens spell it.
pub const PLACEHOLDER_CWD: &str = "/tmp/statecraft-probe";
/// The placeholder `messaging_socket_path`, as the goldens spell it.
pub const PLACEHOLDER_SOCKET: &str = "/tmp/statecraft-probe/socket";
/// The placeholder for each path `memory_paths` carries. The goldens' value
/// is `{}`, so they fix no spelling for a path inside it.
pub const PLACEHOLDER_MEMORY: &str = "/tmp/statecraft-probe/memory";

/// The name lists whose entries become count-preserving placeholders, each
/// with its placeholder stem as the goldens spell it (`tool-1`, `server-1`).
pub const PLACEHOLDER_LISTS: [(&str, &str); 6] = [
    ("tools", "tool"),
    ("skills", "skill"),
    ("agents", "agent"),
    ("slash_commands", "command"),
    ("terminal_slash_commands", "terminal-command"),
    ("mcp_servers", "server"),
];

impl Scenario {
    /// The arguments after the program: spec 004 section 3.9's fixed flags,
    /// then this scenario's. `settings` is the path of the file holding
    /// [`Scenario::settings`], given only when that is `Some`.
    pub fn args(&self, settings: Option<&str>) -> Vec<String> {
        let mut args: Vec<String> = BASE_ARGS.iter().map(|a| (*a).to_string()).collect();
        if let Some(turns) = self.max_turns {
            args.extend(["--max-turns".to_string(), turns.to_string()]);
        }
        if let Some(tools) = self.disallowed_tools {
            args.extend(["--disallowedTools".to_string(), tools.to_string()]);
        }
        if let Some(path) = settings {
            args.extend(["--settings".to_string(), path.to_string()]);
        }
        args
    }

    /// The settings document, when the scenario carries deny rules.
    pub fn settings(&self) -> Option<serde_json::Value> {
        (!self.deny.is_empty()).then(|| serde_json::json!({"permissions": {"deny": self.deny}}))
    }

    /// The manifest entry for this scenario's capture.
    pub fn entry(&self, provider_version: &str, platform: &str, captured: &str) -> Capture {
        Capture {
            scenario: self.name.to_string(),
            provider_version: provider_version.to_string(),
            platform: platform.to_string(),
            invocation: self.invocation.to_string(),
            captured: captured.to_string(),
            file: format!("{}.jsonl", self.name),
            redactions: REDACTIONS.iter().map(|r| (*r).to_string()).collect(),
        }
    }
}

/// The platform word a manifest entry carries for an operating system as
/// `std::env::consts::OS` names it: the goldens say `darwin`, not `macos`.
pub fn platform(os: &str) -> &str {
    match os {
        "macos" => "darwin",
        other => other,
    }
}

/// Why a raw stream could not be redacted. Nothing is written for it: a line
/// that cannot be read cannot be shown to be redacted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("stream line {line} is not a JSON object: {detail}")]
pub struct RedactError {
    /// The 1-based line number in the raw stream.
    pub line: usize,
    /// What the reader said.
    pub detail: String,
}

/// Redact a raw stream, one JSON event per line, by the stream README's
/// rules, keeping each object's key order. `captured` is the capture date,
/// `YYYY-MM-DD`; every `timestamp` becomes its midnight, as the goldens'
/// `2026-09-17T00:00:00.000Z` is theirs. Blank lines are dropped; every other
/// field is verbatim.
pub fn redact(raw: &str, captured: &str) -> Result<String, RedactError> {
    let timestamp = format!("{captured}T00:00:00.000Z");
    let roots = work_trees(raw);
    let mut out = String::new();
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let error = |detail: String| RedactError {
            line: index + 1,
            detail,
        };
        let value: Ordered = serde_json::from_str(line).map_err(|e| error(e.to_string()))?;
        let Ordered::Object(mut event) = value else {
            return Err(error("not an object".into()));
        };
        if dropped(&event) {
            continue;
        }
        for (key, value) in &mut event {
            if let Some((_, stem)) = PLACEHOLDER_LISTS.iter().find(|(list, _)| list == key) {
                placeholders(value, stem, *key == "tools");
            }
        }
        let mut event = Ordered::Object(event);
        fixed(&mut event, &timestamp, &roots);
        out.push_str(&serde_json::to_string(&event).map_err(|e| error(e.to_string()))?);
        out.push('\n');
    }
    Ok(out)
}

fn field<'a>(event: &'a [(String, Ordered)], key: &str) -> Option<&'a Ordered> {
    event.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn is_str(value: Option<&Ordered>, word: &str) -> bool {
    matches!(value, Some(Ordered::String(s)) if s == word)
}

/// The two dropped kinds of line: `system`/`thinking_tokens` and
/// `rate_limit_event` events, and an `assistant` event whose content is
/// thinking and nothing else.
fn dropped(event: &[(String, Ordered)]) -> bool {
    let kind = field(event, "type");
    if is_str(kind, "rate_limit_event") {
        return true;
    }
    if is_str(kind, "system") && is_str(field(event, "subtype"), "thinking_tokens") {
        return true;
    }
    if !is_str(kind, "assistant") {
        return false;
    }
    let Some(Ordered::Object(message)) = field(event, "message") else {
        return false;
    };
    match field(message, "content") {
        Some(Ordered::Array(blocks)) if !blocks.is_empty() => blocks.iter().all(
            |b| matches!(b, Ordered::Object(block) if is_str(field(block, "type"), "thinking")),
        ),
        _ => false,
    }
}

/// The scenario's work tree as the stream names it: every `cwd` and
/// `original_cwd` of an `init` event, each also in its other spelling on
/// macOS (`/private/var/...` and `/var/...` name the same directory), longest
/// first so a longer spelling is replaced before a shorter one inside it. A
/// root that is not absolute, or is `/` itself, is not taken, because
/// replacing it would rewrite text that names no path.
fn work_trees(raw: &str) -> Vec<String> {
    let mut roots: Vec<String> = Vec::new();
    for line in raw.lines() {
        let Ok(Ordered::Object(event)) = serde_json::from_str::<Ordered>(line) else {
            continue;
        };
        if !is_str(field(&event, "subtype"), "init") {
            continue;
        }
        for key in ["cwd", "original_cwd"] {
            let Some(Ordered::String(path)) = field(&event, key) else {
                continue;
            };
            let path = path.trim_end_matches('/');
            if !path.starts_with('/') || path.len() < 2 {
                continue;
            }
            let other = match path.strip_prefix("/private") {
                Some(rest) if rest.starts_with("/var/") || rest.starts_with("/tmp/") => {
                    rest.to_string()
                }
                _ if path.starts_with("/var/") || path.starts_with("/tmp/") => {
                    format!("/private{path}")
                }
                _ => String::new(),
            };
            for root in [path.to_string(), other] {
                if !root.is_empty() && root != PLACEHOLDER_CWD && !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
    }
    roots.sort_by_key(|r| std::cmp::Reverse(r.len()));
    roots
}

/// The fixed values and placeholder paths, at every depth of an event.
fn fixed(value: &mut Ordered, timestamp: &str, roots: &[String]) {
    match value {
        Ordered::Object(fields) => {
            for (key, value) in fields.iter_mut() {
                match key.as_str() {
                    "session_id" => *value = Ordered::String(FIXED_SESSION_ID.into()),
                    "uuid" | "hook_id" => *value = Ordered::String(FIXED_ID.into()),
                    "timestamp" => *value = Ordered::String(timestamp.into()),
                    "cwd" | "original_cwd" => *value = Ordered::String(PLACEHOLDER_CWD.into()),
                    "messaging_socket_path" => {
                        *value = Ordered::String(PLACEHOLDER_SOCKET.into());
                    }
                    "memory_paths" => memory(value),
                    "plugins" | "additional_directories" => *value = Ordered::Array(Vec::new()),
                    _ => fixed(value, timestamp, roots),
                }
            }
        }
        Ordered::Array(items) => items.iter_mut().for_each(|v| fixed(v, timestamp, roots)),
        Ordered::String(text) => {
            for root in roots {
                if text.contains(root.as_str()) {
                    *text = text.replace(root.as_str(), PLACEHOLDER_CWD);
                }
            }
        }
        _ => {}
    }
}

/// Every path `memory_paths` carries becomes the placeholder; its shape and
/// its keys stay.
fn memory(value: &mut Ordered) {
    match value {
        Ordered::String(s) => *s = PLACEHOLDER_MEMORY.into(),
        Ordered::Array(items) => items.iter_mut().for_each(memory),
        Ordered::Object(fields) => fields.iter_mut().for_each(|(_, v)| memory(v)),
        _ => {}
    }
}

/// Replace a name list's names with `stem-1`, `stem-2`, ..., one per entry.
/// In `tools`, the verbatim names stay, first and in their order, and the
/// placeholders follow, which is how the goldens lay the list out. An entry
/// that is an object keeps its other fields and has its `name` replaced.
fn placeholders(value: &mut Ordered, stem: &str, tools: bool) {
    let Ordered::Array(items) = value else {
        return;
    };
    let verbatim = |item: &Ordered| {
        tools && matches!(item, Ordered::String(s) if VERBATIM_TOOLS.contains(&s.as_str()))
    };
    let (mut kept, rest): (Vec<Ordered>, Vec<Ordered>) =
        std::mem::take(items).into_iter().partition(|i| verbatim(i));
    for (n, mut item) in rest.into_iter().enumerate() {
        let name = format!("{stem}-{}", n + 1);
        match &mut item {
            Ordered::String(s) => *s = name,
            Ordered::Object(fields) => {
                if let Some((_, v)) = fields.iter_mut().find(|(k, _)| k == "name") {
                    *v = Ordered::String(name);
                }
            }
            _ => {}
        }
        kept.push(item);
    }
    *items = kept;
}

/// A JSON value that keeps its objects' key order, so a redacted line reads
/// like the line the provider wrote. `serde_json::Value` sorts keys in this
/// workspace, and enabling its order-preserving feature would change every
/// other crate's serialization.
#[derive(Debug, Clone, PartialEq)]
enum Ordered {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Ordered>),
    Object(Vec<(String, Ordered)>),
}

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Ordered::Null => s.serialize_unit(),
            Ordered::Bool(b) => s.serialize_bool(*b),
            Ordered::Number(n) => n.serialize(s),
            Ordered::String(v) => s.serialize_str(v),
            Ordered::Array(items) => {
                let mut seq = s.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Ordered::Object(fields) => {
                let mut map = s.serialize_map(Some(fields.len()))?;
                for (k, v) in fields {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

struct OrderedVisitor;

impl<'de> Visitor<'de> for OrderedVisitor {
    type Value = Ordered;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> Result<Ordered, E> {
        Ok(Ordered::Null)
    }
    fn visit_none<E>(self) -> Result<Ordered, E> {
        Ok(Ordered::Null)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Ordered, D::Error> {
        Ordered::deserialize(d)
    }
    fn visit_bool<E>(self, b: bool) -> Result<Ordered, E> {
        Ok(Ordered::Bool(b))
    }
    fn visit_i64<E>(self, n: i64) -> Result<Ordered, E> {
        Ok(Ordered::Number(n.into()))
    }
    fn visit_u64<E>(self, n: u64) -> Result<Ordered, E> {
        Ok(Ordered::Number(n.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, n: f64) -> Result<Ordered, E> {
        serde_json::Number::from_f64(n)
            .map(Ordered::Number)
            .ok_or_else(|| E::custom("a number JSON cannot carry"))
    }
    fn visit_str<E>(self, s: &str) -> Result<Ordered, E> {
        Ok(Ordered::String(s.to_string()))
    }
    fn visit_string<E>(self, s: String) -> Result<Ordered, E> {
        Ok(Ordered::String(s))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Ordered, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Ordered::Array(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
        let mut fields = Vec::new();
        while let Some((k, v)) = map.next_entry::<String, Ordered>()? {
            fields.push((k, v));
        }
        Ok(Ordered::Object(fields))
    }
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(OrderedVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const DATE: &str = "2026-11-02";

    fn lines(text: &str) -> Vec<Value> {
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn the_dropped_kinds_of_line_are_dropped_and_nothing_else_is() {
        let raw = [
            r#"{"type":"system","subtype":"thinking_tokens","tokens":12}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"","signature":"abc"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","signature":"a"},{"type":"text","text":"hi"}]}}"#,
            r#"{"type":"assistant","message":{"content":[]}}"#,
            r#"{"type":"system","subtype":"hook_started"}"#,
            "",
            r#"{"type":"result","subtype":"success","is_error":false}"#,
        ]
        .join("\n");
        let out = lines(&redact(&raw, DATE).unwrap());
        assert_eq!(out.len(), 4);
        assert_eq!(out[0]["message"]["content"][1]["text"], "hi");
        assert_eq!(out[1]["message"]["content"], json!([]));
        assert_eq!(out[2]["subtype"], "hook_started");
        assert_eq!(out[3]["type"], "result");
    }

    #[test]
    fn ids_and_timestamps_are_fixed_at_every_depth() {
        let raw = r#"{"type":"user","session_id":"abc","uuid":"u-1","timestamp":"2026-11-02T13:14:15.123Z","message":{"content":[{"uuid":"inner","hook_id":"h"}]},"tool_use_result":{"session_id":"deep"}}"#;
        let v = &lines(&redact(raw, DATE).unwrap())[0];
        assert_eq!(v["session_id"], FIXED_SESSION_ID);
        assert_eq!(v["uuid"], FIXED_ID);
        assert_eq!(v["timestamp"], "2026-11-02T00:00:00.000Z");
        assert_eq!(v["message"]["content"][0]["uuid"], FIXED_ID);
        assert_eq!(v["message"]["content"][0]["hook_id"], FIXED_ID);
        assert_eq!(v["tool_use_result"]["session_id"], FIXED_SESSION_ID);
    }

    #[test]
    fn paths_become_placeholders_and_plugins_are_emptied() {
        let raw = r#"{"type":"system","subtype":"init","cwd":"/Users/someone/work/tree","memory_paths":{"user":"/Users/someone/.claude/CLAUDE.md","project":["/Users/someone/work/tree/CLAUDE.md"]},"messaging_socket_path":"/var/folders/xy/T/claude.sock","plugins":[{"name":"private","path":"/Users/someone/p"}]}"#;
        let out = redact(raw, DATE).unwrap();
        assert!(
            !out.contains("someone") && !out.contains("folders"),
            "{out}"
        );
        let v = &lines(&out)[0];
        assert_eq!(v["cwd"], PLACEHOLDER_CWD);
        assert_eq!(v["messaging_socket_path"], PLACEHOLDER_SOCKET);
        assert_eq!(
            v["memory_paths"],
            json!({"user": PLACEHOLDER_MEMORY, "project": [PLACEHOLDER_MEMORY]})
        );
        assert_eq!(v["plugins"], json!([]));
        // An empty memory_paths stays empty, as the goldens carry it.
        let empty = redact(r#"{"type":"system","memory_paths":{}}"#, DATE).unwrap();
        assert_eq!(lines(&empty)[0]["memory_paths"], json!({}));
    }

    #[test]
    fn the_work_tree_is_a_placeholder_wherever_a_string_names_it() {
        let raw = [
            r#"{"type":"system","subtype":"init","cwd":"/private/var/folders/xy/T/statecraft-capture-AbC","original_cwd":"/private/var/folders/xy/T/statecraft-capture-AbC","additional_directories":["/Users/someone/work","/Users/someone/dev"]}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/private/var/folders/xy/T/statecraft-capture-AbC/one.txt"}}]}}"#,
            r#"{"type":"user","tool_use_result":{"filePath":"/var/folders/xy/T/statecraft-capture-AbC/two.txt","content":"read /private/var/folders/xy/T/statecraft-capture-AbC/three.txt"}}"#,
            r#"{"type":"result","result":"/varied text stays","is_error":false}"#,
        ]
        .join("\n");
        let out = redact(&raw, DATE).unwrap();
        assert!(
            !out.contains("someone") && !out.contains("folders") && !out.contains("AbC"),
            "{out}"
        );
        let v = lines(&out);
        assert_eq!(v[0]["cwd"], PLACEHOLDER_CWD);
        assert_eq!(v[0]["original_cwd"], PLACEHOLDER_CWD);
        assert_eq!(v[0]["additional_directories"], json!([]));
        assert_eq!(
            v[1]["message"]["content"][0]["input"]["file_path"],
            format!("{PLACEHOLDER_CWD}/one.txt")
        );
        assert_eq!(
            v[2]["tool_use_result"]["filePath"],
            format!("{PLACEHOLDER_CWD}/two.txt")
        );
        assert_eq!(
            v[2]["tool_use_result"]["content"],
            format!("read {PLACEHOLDER_CWD}/three.txt")
        );
        assert_eq!(v[3]["result"], "/varied text stays");
    }

    #[test]
    fn names_become_count_preserving_placeholders_except_the_verbatim_tools() {
        let raw = r#"{"type":"system","subtype":"init","tools":["mcp__private__x","Bash","Agent","Read","Glob","WebFetch","Task"],"skills":["mine","yours"],"agents":["reviewer"],"slash_commands":["a","b","c"],"terminal_slash_commands":["t"],"mcp_servers":[{"name":"private","status":"connected"},{"name":"other","status":"failed"}],"model":"claude-x"}"#;
        let v = &lines(&redact(raw, DATE).unwrap())[0];
        assert_eq!(
            v["tools"],
            json!(["Bash", "Read", "Glob", "Task", "tool-1", "tool-2", "tool-3"])
        );
        assert_eq!(v["skills"], json!(["skill-1", "skill-2"]));
        assert_eq!(v["agents"], json!(["agent-1"]));
        assert_eq!(
            v["slash_commands"],
            json!(["command-1", "command-2", "command-3"])
        );
        assert_eq!(v["terminal_slash_commands"], json!(["terminal-command-1"]));
        assert_eq!(
            v["mcp_servers"],
            json!([{"name": "server-1", "status": "connected"}, {"name": "server-2", "status": "failed"}])
        );
        assert_eq!(v["model"], "claude-x");
    }

    #[test]
    fn everything_else_is_verbatim_and_key_order_is_kept() {
        let raw = r#"{"type":"result","subtype":"success","is_error":true,"total_cost_usd":0.044009,"num_turns":1,"permission_denials":[{"tool_name":"Bash","tool_input":{"command":"echo hello"}}],"terminal_reason":"api_error","result":"Not logged in · Please run /login"}"#;
        let out = redact(raw, DATE).unwrap();
        assert_eq!(out, format!("{raw}\n"));
    }

    #[test]
    fn a_line_that_is_not_an_object_is_refused_naming_it() {
        let e = redact("{\"type\":\"system\"}\nnot json\n", DATE).unwrap_err();
        assert_eq!(e.line, 2);
        assert_eq!(redact("[1]", DATE).unwrap_err().line, 1);
    }

    /// The rules reproduce the goldens' own spellings: redacting a committed
    /// golden again, on its capture date, changes nothing it means.
    #[test]
    fn redacting_a_golden_again_leaves_it_as_it_is() {
        for (name, text) in [
            ("success", include_str!("../testdata/stream/success.jsonl")),
            ("denied", include_str!("../testdata/stream/denied.jsonl")),
            (
                "max-turns",
                include_str!("../testdata/stream/max-turns.jsonl"),
            ),
            (
                "tool-removed",
                include_str!("../testdata/stream/tool-removed.jsonl"),
            ),
            (
                "api-error",
                include_str!("../testdata/stream/api-error.jsonl"),
            ),
        ] {
            let again = redact(text, "2026-09-17").unwrap();
            assert_eq!(lines(&again), lines(text), "{name}");
        }
    }

    /// Each entry the verb writes carries the invocation and redactions the
    /// committed corpus records for that scenario.
    #[test]
    fn the_scenario_table_matches_the_committed_corpus() {
        let corpus: Value =
            serde_json::from_str(include_str!("../testdata/stream/corpus.json")).unwrap();
        let entries = corpus["captures"].as_array().unwrap();
        assert_eq!(entries.len(), SCENARIOS.len());
        for (scenario, golden) in SCENARIOS.iter().zip(entries) {
            let entry = scenario.entry("2.1.267", "darwin", "2026-09-17");
            let entry = serde_json::to_value(&entry).unwrap();
            assert_eq!(entry, *golden, "{}", scenario.name);
            assert!(!scenario.prompt.trim().is_empty());
        }
    }

    #[test]
    fn each_scenario_asks_for_exactly_its_recorded_flags() {
        let by = |name: &str| *SCENARIOS.iter().find(|s| s.name == name).unwrap();
        assert_eq!(by("success").args(None), BASE_ARGS);
        assert!(by("success").settings().is_none());
        assert_eq!(by("max-turns").args(None)[4..], ["--max-turns", "1"]);
        assert_eq!(
            by("tool-removed").args(None)[4..],
            ["--disallowedTools", "Bash"]
        );
        let denied = by("denied");
        assert_eq!(
            denied.settings(),
            Some(json!({"permissions": {"deny": ["Bash(echo:*)"]}}))
        );
        assert_eq!(denied.args(Some("/s.json"))[4..], ["--settings", "/s.json"]);
        assert!(by("api-error").only_path);
        assert_eq!(SCENARIOS.iter().filter(|s| s.only_path).count(), 1);
        // Spec 004 section 3.1: no prompt reaches the argument vector.
        for s in SCENARIOS {
            assert!(!s.args(Some("/s.json")).iter().any(|a| a == s.prompt));
        }
        assert_eq!(platform("macos"), "darwin");
        assert_eq!(platform("linux"), "linux");
    }
}
