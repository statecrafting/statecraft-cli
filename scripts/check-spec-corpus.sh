#!/usr/bin/env bash
# Spec: specs/016-bounded-active-specs/spec.md
#
# Keeps active specifications readable and keeps historical journals outside
# the compiler input. Archives under docs/decisions/archive are intentionally
# outside this check.

set -uo pipefail

max_spec_bytes=81920
max_spec_lines=1200
max_decision_bytes=12288
max_decision_lines=200
root=specs

if [ "${1:-}" = "--self-test" ]; then
  [ "$#" -eq 1 ] || { echo "check-spec-corpus: --self-test takes no arguments" >&2; exit 3; }
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/spec-corpus-self-test.XXXXXX") || exit 3
  trap 'rm -rf "$tmp"' EXIT
  mkdir -p "$tmp/specs/001-valid"
  printf '%s\n' '# Valid' '## 5. Resolved decisions' 'Current rationale.' > "$tmp/specs/001-valid/spec.md"
  "$0" --root "$tmp/specs" >/dev/null || { echo "self-test: valid corpus was refused"; exit 1; }

  mkdir -p "$tmp/specs/002-invalid"
  printf '%s\n' '# Invalid' '## 5. Decisions recorded during implementation' > "$tmp/specs/002-invalid/spec.md"
  printf '%s\n' 'not a spec' > "$tmp/specs/002-invalid/handoff.md"
  if "$0" --root "$tmp/specs" >/dev/null 2>&1; then
    echo "self-test: journal heading and sidecar were accepted"
    exit 1
  fi

  echo "check-spec-corpus: self-test passed"
  exit 0
fi

if [ "${1:-}" = "--root" ]; then
  [ "$#" -eq 2 ] || { echo "check-spec-corpus: --root needs one directory" >&2; exit 3; }
  root=$2
elif [ "$#" -ne 0 ]; then
  echo "usage: check-spec-corpus.sh [--root DIR|--self-test]" >&2
  exit 3
fi

[ -d "$root" ] || { echo "check-spec-corpus: no such directory: $root" >&2; exit 3; }

status=0
count=0

while IFS= read -r file; do
  relative=${file#"$root"/}
  case "$relative" in
    */spec.md) ;;
    *)
      echo "active spec directory contains a sidecar: $file"
      status=1
      continue
      ;;
  esac

  count=$((count + 1))
  bytes=$(wc -c < "$file" | tr -d ' ')
  lines=$(wc -l < "$file" | tr -d ' ')
  if [ "$bytes" -gt "$max_spec_bytes" ]; then
    echo "active spec exceeds $max_spec_bytes bytes: $file ($bytes)"
    status=1
  fi
  if [ "$lines" -gt "$max_spec_lines" ]; then
    echo "active spec exceeds $max_spec_lines lines: $file ($lines)"
    status=1
  fi
  if grep -qE '^## 5\. Decisions recorded during implementation[[:space:]]*$' "$file"; then
    echo "active spec uses the retired implementation-journal heading: $file"
    status=1
  fi

  section=$(mktemp "${TMPDIR:-/tmp}/spec-decisions.XXXXXX") || exit 3
  awk '
    /^## 5\. Resolved decisions[[:space:]]*$/ { inside=1 }
    inside && /^## / && !/^## 5\. Resolved decisions[[:space:]]*$/ { exit }
    inside { print }
  ' "$file" > "$section"
  decision_bytes=$(wc -c < "$section" | tr -d ' ')
  decision_lines=$(wc -l < "$section" | tr -d ' ')
  rm -f "$section"
  if [ "$decision_bytes" -gt "$max_decision_bytes" ]; then
    echo "resolved-decisions section exceeds $max_decision_bytes bytes: $file ($decision_bytes)"
    status=1
  fi
  if [ "$decision_lines" -gt "$max_decision_lines" ]; then
    echo "resolved-decisions section exceeds $max_decision_lines lines: $file ($decision_lines)"
    status=1
  fi
done < <(find "$root" -mindepth 2 -type f | sort)

if [ "$count" -eq 0 ]; then
  echo "check-spec-corpus: no active specs found under $root" >&2
  exit 3
fi

if [ "$status" -eq 0 ]; then
  echo "check-spec-corpus: $count active specs are bounded and sidecar-free"
fi
exit "$status"
