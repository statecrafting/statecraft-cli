//! `provider capture <out-dir>` (spec 052 section 3.5).
//!
//! **This verb spends provider usage.** It runs only when an operator invokes
//! it: no other verb calls it, and no verify line runs it against a real
//! provider. Its tests put a fake `claude` on `PATH`.
//!
//! For each scenario of [`SCENARIOS`] it makes a fresh scratch git work tree
//! holding [`SCRATCH_FILES`], spawns the `claude` that `PATH` resolves with
//! that scenario's flags, delivers the prompt on standard input, and redacts
//! what the provider wrote through [`redact`] before anything reaches
//! `<out-dir>`. It then writes a `captures.json` of `corpus.json`'s shape, so
//! `provider compare <out-dir> --corpus <dir>` reads the result as it is.
//! The captures are reviewed and committed like any authored change; nothing
//! here commits, and nothing reads a product home or a project.

use serde_json::{Value, json};
use statecraft_adapter_claude_code::capture::{
    SCENARIOS, SCRATCH_FILES, Scenario, platform, redact,
};
use statecraft_adapter_claude_code::golden::{CAPTURE_MANIFEST, CaptureSet, meaning};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::exit::Exit;
use crate::render::{Answer, ErrorKind};

const USAGE: &str = "usage: provider capture <out-dir>   (spends provider usage)";

/// How long one scenario may run before its provider is killed.
pub const DEADLINE: Duration = Duration::from_secs(600);

fn arguments(rest: &[String]) -> Result<PathBuf, String> {
    match rest
        .iter()
        .filter(|a| a.as_str() != "--json")
        .collect::<Vec<_>>()
        .as_slice()
    {
        [dir] if !dir.starts_with('-') => Ok(PathBuf::from(dir)),
        _ => Err(USAGE.to_string()),
    }
}

fn refused(kind: ErrorKind, message: String) -> Answer<Value> {
    Answer::new(json!({}), Exit::Refused, message).with_kind(kind)
}

fn failed(message: String) -> Answer<Value> {
    Answer::new(json!({}), Exit::Failed, message).with_kind(ErrorKind::Io)
}

/// The first executable `claude` on `PATH`, so every scenario runs the same
/// binary, and the `PATH` the constructed environment carries.
fn provider() -> Option<(PathBuf, String)> {
    let path = std::env::var("PATH").ok()?;
    let found = std::env::split_paths(&path)
        .map(|dir| dir.join("claude"))
        .find(|candidate| {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(candidate)
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })?;
    Some((found, path))
}

fn today() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    statecraft_environment::time::rfc3339_utc(now)[..10].to_string()
}

/// A scratch git work tree holding [`SCRATCH_FILES`].
fn scratch_tree() -> Result<tempfile::TempDir, String> {
    let tree = tempfile::Builder::new()
        .prefix("statecraft-capture-")
        .tempdir()
        .map_err(|e| format!("the scratch work tree could not be made: {e}"))?;
    for (name, body) in SCRATCH_FILES {
        std::fs::write(tree.path().join(name), body)
            .map_err(|e| format!("{name} could not be written: {e}"))?;
    }
    let init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(tree.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .map_err(|e| format!("git init could not run: {e}"))?;
    if !init.success() {
        return Err("git init failed in the scratch work tree".into());
    }
    Ok(tree)
}

/// Run one scenario and return the provider's raw standard output.
fn run(scenario: &Scenario, program: &Path, path: &str) -> Result<String, String> {
    let tree = scratch_tree()?;
    // The settings file lives beside the tree, never inside it.
    let settings_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let settings = match scenario.settings() {
        Some(document) => {
            let file = settings_dir.path().join("settings.json");
            std::fs::write(&file, document.to_string()).map_err(|e| e.to_string())?;
            Some(file.display().to_string())
        }
        None => None,
    };
    let mut command = Command::new(program);
    command
        .args(scenario.args(settings.as_deref()))
        .current_dir(tree.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if scenario.only_path {
        command.env_clear().env("PATH", path);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("{} could not be spawned: {e}", program.display()))?;
    // Spec 004 section 3.1: the prompt is delivered on a stream.
    let mut stdin = child.stdin.take().ok_or("no standard input")?;
    let prompt = scenario.prompt.as_bytes().to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&prompt);
    });
    let mut stdout = child.stdout.take().ok_or("no standard output")?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let started = Instant::now();
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(_)) => break false,
            Ok(None) if started.elapsed() >= DEADLINE => {
                let _ = child.kill();
                let _ = child.wait();
                break true;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("the provider could not be waited for: {e}")),
        }
    };
    let _ = writer.join();
    let out = reader.join().unwrap_or_default();
    if timed_out {
        return Err(format!("killed at the {}s deadline", DEADLINE.as_secs()));
    }
    // The exit code is not read: spec 004 section 3.3 measured it unreliable
    // in both directions, and the stream is what is recorded.
    Ok(out)
}

/// `provider capture <out-dir>`: every scenario against the installed
/// provider, redacted, with a `captures.json`.
///
/// Exit 0 when every scenario was captured and the set loads as one
/// version's captures; 4 when a scenario produced no stream that carries a
/// version, naming it, with the rest still written; 2 when the directory is
/// not empty, no `claude` is on `PATH`, or the captures name more than one
/// version.
pub fn capture_verb(rest: &[String]) -> Result<Answer<Value>, String> {
    let out = arguments(rest)?;
    if std::fs::read_dir(&out).is_ok_and(|mut d| d.next().is_some()) {
        return Ok(refused(
            ErrorKind::Refused,
            format!(
                "{} is not empty; a capture is written into a fresh directory",
                out.display()
            ),
        ));
    }
    let Some((program, path)) = provider() else {
        return Ok(refused(
            ErrorKind::Config,
            "no executable `claude` is on PATH".into(),
        ));
    };
    if let Err(e) = std::fs::create_dir_all(&out) {
        return Ok(failed(format!("{} could not be made: {e}", out.display())));
    }
    let captured = today();
    let platform = platform(std::env::consts::OS);
    let mut entries = Vec::new();
    let mut rows = Vec::new();
    let mut lacking = Vec::new();
    for scenario in &SCENARIOS {
        let written = run(scenario, &program, &path).and_then(|raw| {
            let text = redact(&raw, &captured).map_err(|e| e.to_string())?;
            let (version, _) = meaning(&text)?;
            let entry = scenario.entry(&version, platform, &captured);
            std::fs::write(out.join(&entry.file), &text)
                .map_err(|e| format!("{} could not be written: {e}", entry.file))?;
            Ok(entry)
        });
        match written {
            Ok(entry) => {
                rows.push(json!({"scenario": scenario.name, "file": entry.file,
                    "providerVersion": entry.provider_version, "status": "captured"}));
                entries.push(entry);
            }
            Err(detail) => {
                rows.push(json!({"scenario": scenario.name, "status": "failed", "detail": detail}));
                lacking.push(scenario.name);
            }
        }
    }
    let manifest = json!({ "captures": entries });
    let manifest = serde_json::to_string_pretty(&manifest).unwrap_or_default() + "\n";
    if let Err(e) = std::fs::write(out.join(CAPTURE_MANIFEST), manifest) {
        return Ok(failed(format!(
            "{CAPTURE_MANIFEST} could not be written: {e}"
        )));
    }
    let value = json!({
        "outDir": out.display().to_string(),
        "captured": captured,
        "platform": platform,
        "scenarios": rows,
    });
    if !lacking.is_empty() {
        let summary = format!(
            "no stream was captured for {}; the rest are in {}",
            lacking.join(", "),
            out.display()
        );
        return Ok(Answer::new(value, Exit::Failed, summary).with_kind(ErrorKind::Io));
    }
    match CaptureSet::from_dir(&out) {
        Ok(set) => {
            let summary = format!(
                "captured {} scenarios from provider {} into {}; review them before committing, \
                 and compare with `provider compare {} --corpus <dir>`",
                set.captures.len(),
                set.provider_version,
                out.display(),
                out.display()
            );
            Ok(Answer::new(value, Exit::Ok, summary))
        }
        Err(e) => {
            Ok(Answer::new(value, Exit::Refused, e.to_string()).with_kind(ErrorKind::Refused))
        }
    }
}
