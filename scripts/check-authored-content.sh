#!/usr/bin/env bash
# Spec: specs/001-boundaries-and-authority/spec.md
#
# The two authored-content rules of spec 001 section 3.6, made mechanical.
#
#   1. No authored file contains U+2014 (EM DASH).
#   2. No authored file carries an agent-session URL or a session-tracking
#      trailer.
#
# Scope is git-tracked files plus staged additions, minus the compiler-owned
# derived tree and minus this script for rule 2 (it necessarily contains the
# patterns it searches for). Findings are reported per rule, so one does not
# mask the other, and every finding names its file and line.
#
# `--text FILE...` applies the same two rules to text that is not a tracked
# file: a pull request's title and body, or a commit message, which become
# history under merge commits (AGENTS.md, "How a pull request is merged"). No
# file is exempt in that mode.
#
# `--self-test` runs this script's `--text` mode over built samples: every
# agent-session link form and trailer below must be refused, and each
# near-miss beside it must pass. It exits 0 when all do, and 1 naming each
# sample that did not.
#
# Exit 0 clean, 1 findings, 3 usage/environment.

set -uo pipefail

SELF="scripts/check-authored-content.sh"
status=0
mode=tree

if [ "${1:-}" = "--self-test" ]; then
  [ "$#" -eq 1 ] || { echo "check-authored-content: --self-test takes no arguments" >&2; exit 3; }
  me="$0"
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/authored-self-test.XXXXXX") || exit 3
  trap 'rm -rf "$tmp"' EXIT
  failed=0
  n=0
  # expect: 1 refused, 0 clean. One sample per line: "<expect> <text>".
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    expect=${line%% *}
    text=${line#* }
    n=$((n + 1))
    printf '%s\n' "$text" > "$tmp/sample"
    "$me" --text "$tmp/sample" > /dev/null 2>&1
    got=$?
    if [ "$got" -ne "$expect" ]; then
      echo "self-test: expected exit $expect, got $got: $text"
      failed=1
    fi
  done <<'SAMPLES'
1 see https://claude.ai/chat/0123abcd-4567-89ef-0123-456789abcdef
1 see https://claude.ai/code/0123abcd-4567-89ef-0123-456789abcdef
1 see https://claude.ai/code/session_01AbCdEfGhIjKlMnOpQrStUv
1 see https://claude.ai/code/session_011CUz9vQ8x
1 see https://chatgpt.com/codex/tasks/task_e_68d4c0ffee0123456789abcdef
1 Co-Authored-By: Claude <noreply@anthropic.com>
1 Generated with [Claude Code](https://example.invalid)
1 Session-Id: 0123456789abcdef
0 claude.ai/code is the product page
0 https://claude.ai/code/docs
0 claude.ai/code/session_ with no identifier
0 https://chatgpt.com/codex
0 a plain sentence
SAMPLES
  if [ "$failed" -ne 0 ]; then
    exit 1
  fi
  echo "check-authored-content: self-test passed ($n samples)"
  exit 0
fi

if [ "${1:-}" = "--text" ]; then
  mode=text
  shift
  [ "$#" -gt 0 ] || { echo "check-authored-content: --text needs at least one file" >&2; exit 3; }
  SELF=""
else
  cd "$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo "check-authored-content: not inside a git work tree" >&2
    exit 3
  }
fi

# Bash 3.2 is the floor (macOS system bash). Refuse anything older loudly rather
# than reporting a clean tree we never actually read.
if [ "${BASH_VERSINFO[0]:-0}" -lt 3 ]; then
  echo "check-authored-content: needs bash 3.2 or newer, found ${BASH_VERSION:-unknown}" >&2
  exit 3
fi

# Text files under version control. -z plus a NUL read keeps paths with spaces
# intact; the derived tree is compiler output and is not authored.
#
# Built with a read loop rather than `mapfile -d ''`, which needs bash 4.4.
# macOS ships /bin/bash 3.2, so a contributor whose PATH resolves to it would
# otherwise get an empty file list and a vacuous pass. This form runs on 3.2.
files=()
if [ "$mode" = text ]; then
  for f in "$@"; do
    [ -f "$f" ] || { echo "check-authored-content: no such file: $f" >&2; exit 3; }
    files+=("$f")
  done
else
  while IFS= read -r -d '' f; do
    case "$f" in
      .statecraft/derived/*|*.png|*.jpg|*.jpeg|*.gif|*.ico|*.pdf|*.node) continue ;;
    esac
    [ -f "$f" ] || continue
    files+=("$f")
  done < <(git ls-files -z --cached --others --exclude-standard)
fi

if [ "${#files[@]}" -eq 0 ]; then
  echo "check-authored-content: no authored files found" >&2
  exit 3
fi

# --- Rule 1: U+2014 ---------------------------------------------------------
# The byte sequence is built rather than written, so this file does not trip
# its own check and a reader can see exactly which codepoint is refused.
emdash=$(printf '\xe2\x80\x94')
em_hits=$(grep -n -F -- "$emdash" "${files[@]}" 2>/dev/null)
if [ -n "$em_hits" ]; then
  echo "U+2014 (EM DASH) is refused by spec 001 section 3.6.1:"
  printf '%s\n' "$em_hits" | sed 's/^/  /'
  echo "  Use a colon, semicolon, comma, parentheses, or two sentences."
  status=1
fi

# --- Rule 2: agent-session URLs and session-tracking trailers ---------------
scan=()
for f in "${files[@]}"; do
  [ "$f" = "$SELF" ] || scan+=("$f")
done

# One pattern per line, extended regex. Each is a link or trailer that ties
# repository content to an agent session; spec 001 section 3.6.2 refuses all of
# them, and refuses substituting another tracking link. A Claude session link
# has two historical forms, a UUID path and the `session_` path cloud sessions
# append to pull-request bodies; a Codex cloud task link is the third (spec 001
# section 5, 2026-09-25).
patterns='claude\.ai/(chat|code)/[0-9a-f-]{8}
claude\.ai/(chat|code)/session_[0-9A-Za-z]{8}
chatgpt\.com/codex/tasks/task_[0-9A-Za-z_]{8}
Co-[Aa]uthored-[Bb]y:.*(Claude|Codex|Copilot|Gemini|noreply@anthropic)
Generated with \[?(Claude Code|Codex)
(Session|Agent-Session|Run)-(Id|URL): *[0-9a-zA-Z_-]{8}
https?://[a-z0-9.-]*(anthropic|openai)\.com/[a-z/]*session'

if [ "${#scan[@]}" -gt 0 ]; then
  link_hits=$(printf '%s\n' "$patterns" | grep -v '^$' \
    | while IFS= read -r p; do grep -n -E -- "$p" "${scan[@]}" 2>/dev/null; done)
  if [ -n "$link_hits" ]; then
    echo "agent-session links and session trailers are refused by spec 001 section 3.6.2:"
    printf '%s\n' "$link_hits" | sed 's/^/  /'
    status=1
  fi
fi

if [ "$status" -eq 0 ]; then
  echo "check-authored-content: ${#files[@]} authored $([ "$mode" = text ] && echo text || echo file)(s) clean (U+2014, session links)"
fi
exit "$status"
