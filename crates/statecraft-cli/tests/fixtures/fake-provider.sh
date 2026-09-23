#!/bin/sh
# A LOCAL FAKE PROVIDER. Never a provider, never live evidence.
#
# Spec 002 section 3.30's local test route. It answers `--version` and, for a
# session, reads the prompt from standard input, takes the command from its
# last line, and writes a `stream-json` session in the shapes the recorded
# Claude Code 2.1.267 streams carry: a hook event, the init event, the tool
# request, then either the mid-stream denial and a result marked not executed
# or a result that ran, and the turn-capped terminal event, exit 1.
#
# It executes nothing. "Enforcement" here is whether `--settings` was passed:
# with it, the refused command is denied. Every capture launched against it is
# marked synthetic by `startup capture --synthetic`, and every result says so.
#
# FAKE_PROVIDER_MODE picks a behavior, so each branch of the acceptance's
# control flow can be driven deterministically:
#
#   faithful          the default, described above
#   ignores-settings  never denies: the refusal control executes
#   startup-fails     writes to stderr and exits 1 with no stream
#   request-only      requests the tool and never returns a result
#   malformed         writes a line that is not JSON
#   conflicting       writes two init events from two sessions
#   hang              backgrounds a descendant and hangs
#   signal            ends itself with SIGTERM
#   version-fails     `--version` exits 1
#   tamper            behaves faithfully, and first deletes $FAKE_TAMPER, the
#                     way a session that reached an earlier capture could
set -eu

mode="${FAKE_PROVIDER_MODE:-faithful}"

if [ "${1:-}" = "--version" ]; then
  [ "$mode" = version-fails ] && exit 1
  printf '2.1.267 (Claude Code)\n'
  exit 0
fi

settings=no
while [ "$#" -gt 0 ]; do
  case "$1" in
    --settings) settings=yes; shift ;;
    --settings=*) settings=yes ;;
  esac
  shift
done

prompt="$(cat)"
command="$(printf '%s\n' "$prompt" | awk 'NF { last = $0 } END { print last }')"

esc() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' | awk 'BEGIN { ORS = "" } NR > 1 { print "\\n" } { print }'
}

session="fake-session-$$"
id="toolu_fake_$$"
cwd="$(esc "$(pwd -P)")"
cmd="$(esc "$command")"
input="{\"command\":\"$cmd\",\"description\":\"Run $cmd\"}"

hook() { printf '{"type":"system","subtype":"hook_started","session_id":"%s","hook_name":"SessionStart:startup","hook_event":"SessionStart"}\n' "$1"; }
init() { printf '{"type":"system","subtype":"init","session_id":"%s","cwd":"%s","claude_code_version":"2.1.267","model":"fake","permissionMode":"default","tools":["Bash","Read","Edit"],"apiKeySource":"none"}\n' "$1" "$cwd"; }
request() { printf '{"type":"assistant","session_id":"%s","parent_tool_use_id":null,"message":{"role":"assistant","content":[{"type":"tool_use","id":"%s","name":"Bash","input":%s}]}}\n' "$session" "$id" "$input"; }
ran() { printf '{"type":"user","session_id":"%s","parent_tool_use_id":null,"message":{"role":"user","content":[{"type":"tool_result","content":"%s","is_error":%s,"tool_use_id":"%s"}]}}\n' "$session" "$1" "$2" "$id"; }
capped() { printf '{"type":"result","subtype":"error_max_turns","session_id":"%s","is_error":true,"terminal_reason":"max_turns","num_turns":2,"permission_denials":[%s]}\n' "$session" "$1"; }

case "$mode" in
  tamper)
    [ -n "${FAKE_TAMPER:-}" ] && rm -f "$FAKE_TAMPER"
    ;;
  startup-fails)
    printf 'error: the fake refuses to start\n' >&2
    exit 1
    ;;
  malformed)
    hook "$session"; init "$session"
    printf 'this is not an event\n'
    exit 1
    ;;
  conflicting)
    hook "$session"; init "$session"; init "other-$session"
    request; capped ""
    exit 1
    ;;
  hang)
    hook "$session"; init "$session"
    sleep 300 &
    exec sleep 300
    ;;
  signal)
    hook "$session"; init "$session"
    kill -s TERM $$
    ;;
  request-only)
    hook "$session"; init "$session"; request; capped ""
    exit 1
    ;;
esac

hook "$session"
init "$session"
request
case "$command" in
  "cargo publish"*)
    if [ "$settings" = yes ] && [ "$mode" != ignores-settings ]; then
      message="Permission to use Bash with command $cmd has been denied."
      printf '{"type":"system","subtype":"permission_denied","session_id":"%s","tool_name":"Bash","tool_use_id":"%s","decision_reason_type":"subcommandResults","message":"%s"}\n' "$session" "$id" "$message"
      printf '{"type":"user","session_id":"%s","parent_tool_use_id":null,"tool_use_result":"Error: %s","tool_result_meta":[{"id":"%s","non_execution_kind":"permission-rule"}],"message":{"role":"user","content":[{"type":"tool_result","content":"%s","is_error":true,"tool_use_id":"%s"}]}}\n' "$session" "$message" "$id" "$message" "$id"
      capped "{\"tool_name\":\"Bash\",\"tool_use_id\":\"$id\",\"tool_input\":$input}"
    else
      ran 'Exit code 101\nerror: manifest path `statecraft-absent/Cargo.toml` does not exist' true
      capped ""
    fi
    ;;
  "echo "*)
    ran "$(esc "${command#echo }")" false
    capped ""
    ;;
  *)
    ran "unexpected command" true
    capped ""
    ;;
esac
exit 1
