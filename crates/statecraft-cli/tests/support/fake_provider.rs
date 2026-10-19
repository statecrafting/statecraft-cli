//! The fake `claude` the run suites launch, and the producer stub's version
//! line, shared by `run_startup` and `application_equivalence`. Nothing here
//! spawns a real provider.

#![allow(dead_code)]

/// The stub answers as the linked producer: a fresh scaffold pins that
/// release exactly (spec 018 section 3.3), so initialization admits it.
pub fn as_linked(script: &str) -> String {
    let linked = format!("spec-spine {}", statecraft_home::producer::PRODUCER_VERSION);
    let stubbed = script.replace("spec-spine 0.20.0", &linked);
    assert!(stubbed.contains(&linked), "the stub reports a version");
    stubbed
}

/// The fake provider. See `run_startup`'s module comment for what it does
/// and does not stand in for.
pub const FAKE_PROVIDER: &str = r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267 (Claude Code)'; exit 0; fi
here="$(dirname "$0")"
output="$PWD/.fixture-output"
/bin/mkdir -p "$output"
settings=""
while [ "$#" -gt 0 ]; do
  case "$1" in --settings) settings="$2"; shift ;; esac
  shift
done
mode="$(/bin/cat "$here/mode" 2>/dev/null || echo faithful)"
/bin/cp "$settings" "$output/received-settings"
echo "$settings" > "$output/received-settings-path"
/bin/ls -A "$(dirname "$settings")" > "$output/exchange-listing"
/usr/bin/env > "$output/received-env"
/bin/cat > "$output/received-prompt"
echo launched >> "$output/launches"
echo $$ > "$output/pid"
log() { echo "$1" >> "$output/order"; }
session=11111111-1111-1111-1111-111111111111
escape() { /usr/bin/awk 'BEGIN { ORS = "" } { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); if (NR > 1) printf "\\n"; print }'; }
respond() {
  printf '{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup","hook_event":"SessionStart","session_id":"%s"}\n' "$session"
  printf '{"type":"system","subtype":"hook_response","hook_name":"SessionStart:startup","hook_event":"SessionStart","stdout":"%s","stderr":"","exit_code":%s,"outcome":"success","session_id":"%s"}\n' "$(printf '%s' "$1" | escape)" "$2" "$session"
}
# The first command registered for an event, read from the document given.
command_for() {
  /usr/bin/awk -v ev="\"$1\": [" 'index($0, ev) { inside = 1 } inside && /"command": / { sub(/^[^"]*"command": "/, ""); sub(/",?[ \t]*$/, ""); gsub(/\\\\/, "\001"); gsub(/\\"/, "\""); gsub(/\001/, "\\"); print; exit }' "$settings"
}
start_cmd="$(command_for SessionStart)"
gate_cmd="$(command_for PreToolUse)"
# One tool call: through the registered gate, then its effect if released.
tool() {
  rc=0
  if [ -n "$gate_cmd" ] && [ "$mode" != ignores-hooks ]; then
    printf '{"tool_name":"Bash","tool_input":{"command":"touch %s"}}' "$1" | /bin/sh -c "$gate_cmd" 2>> "$output/gate-stderr"
    rc=$?
  fi
  log "gate $1 exit=$rc"
  if [ "$rc" = 0 ]; then : > "$PWD/$1"; log "effect $1"; fi
}
if [ "$mode" != ignores-hooks ] && [ ! -f "$here/skip-start-hook" ]; then
  if [ -f "$here/substitute-hook" ]; then start_cmd="'$(/bin/cat "$here/substitute-hook")'"; fi
  if [ -n "$start_cmd" ]; then
    out="$(CLAUDE_PROJECT_DIR="$PWD" /bin/sh -c "$start_cmd" 2>/dev/null)"
    respond "$out" "$?"
  fi
fi
if [ -f "$here/replay-line" ]; then respond "$(/bin/cat "$here/replay-line")" 0; fi
if [ -f "$here/block-record" ]; then
  /bin/mkdir -p "$(/bin/cat "$here/block-record")"
fi
# A child writing the decision before the supervisor does (spec 002 section
# 3.37 rule 2), refused by spec 004 section 3.18: the gate's copy is beside
# the settings file.
if [ "$mode" = plant-decision ]; then
  if printf '{"decision":"admitted"}\n' > "$(dirname "$settings")/admission.json"; then
    log planted
  else
    log plant-refused
  fi
fi
if [ "$mode" = ignores-hooks ]; then
  : > "$PWD/sentinel-before-decision"
  log "effect sentinel-before-decision"
fi
if [ -f "$here/hostile" ]; then
  /bin/sh "$here/hostile" "$settings" > "$output/hostile-report" 2> "$output/hostile-errors"
  log "hostile exit=$?"
fi
if [ "$mode" = early-tool ]; then
  tool sentinel-early &
  early=$!
  /bin/sleep 0.5
  if kill -0 "$early" 2>/dev/null; then log "gate waiting before init"; fi
fi
/usr/bin/sed -n '1,3p' "$here/native.jsonl"
log "init emitted"
# Spec 003 section 3.5.1: structured write requests the session reports.
if [ -f "$here/write-request" ]; then /bin/cat "$here/write-request"; fi
if [ -n "${early:-}" ]; then wait "$early"; fi
if [ "$mode" = tool-then-block ]; then
  tool sentinel-released
fi
if [ "$mode" = block ] || [ "$mode" = tool-then-block ]; then
  log blocked
  while [ ! -f "$here/release" ]; do /bin/sleep 0.05; done
fi
tool sentinel-after
/usr/bin/sed -n '4,$p' "$here/native.jsonl"
"#;
