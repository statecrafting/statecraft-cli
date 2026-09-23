#!/bin/sh
# A LOCAL FAKE PROVIDER for the managed-startup trial's local route (spec 002
# section 3.33). Never a provider, and nothing it produces is live evidence.
#
# The acceptance script installs it as `claude` in a directory of its own,
# beside `native.jsonl` (the recorded 2.1.267 stream) and an optional `mode`
# file. It reads the SessionStart and PreToolUse commands from the settings
# document it was given, streams a SessionStart response, the recorded init
# event, one Read request for the sentinel, its result, and the recorded
# terminal event. `mode` selects how faithfully: `faithful`, `gate-ignored`
# (runs the read without consulting the gate) or `ignores-hooks` (runs
# neither registration).
if [ "${1:-}" = --version ]; then echo '2.1.267 (Claude Code)'; exit 0; fi
here="$(dirname "$0")"
settings=""
while [ "$#" -gt 0 ]; do
  case "$1" in --settings) settings="$2"; shift ;; esac
  shift
done
mode="$(cat "$here/mode" 2>/dev/null || echo faithful)"
cat >/dev/null
echo launched >> "$here/launches"
session=11111111-1111-1111-1111-111111111111
escape() { awk 'BEGIN { ORS = "" } { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); if (NR > 1) printf "\\n"; print }'; }
command_for() {
  awk -v ev="\"$1\": [" 'index($0, ev) { inside = 1 } inside && /"command": / { sub(/^[^"]*"command": "/, ""); sub(/",?[ \t]*$/, ""); gsub(/\\\\/, "\001"); gsub(/\\"/, "\""); gsub(/\001/, "\\"); print; exit }' "$settings"
}
start_cmd="$(command_for SessionStart)"
gate_cmd="$(command_for PreToolUse)"
start() {
  out="$(CLAUDE_PROJECT_DIR="$PWD" sh -c "$start_cmd" 2>/dev/null)"
  rc=$?
  printf '{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup","hook_event":"SessionStart","session_id":"%s"}\n' "$session"
  printf '{"type":"system","subtype":"hook_response","hook_name":"SessionStart:startup","hook_event":"SessionStart","stdout":"%s","stderr":"","exit_code":%s,"outcome":"success","session_id":"%s"}\n' "$(printf '%s' "$out" | escape)" "$rc" "$session"
}
request() {
  printf '{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_trial","name":"Read","input":{"file_path":"STATECRAFT-TRIAL-SENTINEL"}}]},"session_id":"%s"}\n' "$session"
}
execute() {
  content="$(tr -d '\n' < "$PWD/STATECRAFT-TRIAL-SENTINEL")"
  printf '{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_trial","content":"%s","is_error":false}]},"session_id":"%s"}\n' "$content" "$session"
}
blocked() {
  printf '{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_trial","content":"blocked by a PreToolUse hook","is_error":true}]},"session_id":"%s"}\n' "$session"
}
case "$mode" in
  faithful)
    start; sed -n '3p' "$here/native.jsonl"; request
    if printf '{"tool_name":"Read"}' | sh -c "$gate_cmd" 2>/dev/null; then execute; else blocked; fi
    ;;
  gate-ignored) start; sed -n '3p' "$here/native.jsonl"; request; execute ;;
  ignores-hooks) sed -n '3p' "$here/native.jsonl"; request; execute ;;
esac
sed -n '4,$p' "$here/native.jsonl"
