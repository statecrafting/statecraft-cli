#!/bin/sh
command -v jq >/dev/null 2>&1 || { echo '[hook] jq missing, staleness check skipped'; exit 0; }
fp=$(jq -r '.tool_input.file_path // empty' 2>/dev/null)
[ -n "$fp" ] || exit 0
# The repo containing the edited file, not the session's project: an edit in a
# sibling checkout must not recompile this one.
root=$(git -C "$(dirname "$fp")" rev-parse --show-toplevel 2>/dev/null) || exit 0
# Spec 002 section 3.14 rule 3, applied to the repository the EDITED FILE
# is in rather than to the session: an edit in a sibling checkout is
# judged by that checkout's own manifest.
[ -f "$root/.statecraft/environment.json" ] || exit 0
# Spec 002 section 3.23 contract 2, as amended on 2026-09-24 (section 5):
# resolve the binary, then establish that the repository admits it. The same
# resolver is in all four hooks. A managed session's supervisor path
# (STATECRAFT_SPEC_SPINE) is the only candidate when it is set. Otherwise an
# explicit $SPEC_SPINE_BIN is the only candidate, and a broken or incompatible
# one refuses rather than falling back. Otherwise the repository's own
# target/release/spec-spine, then PATH, and the first one compatible with the
# repository's pin ([meta] required_version) judges; each one passed over is
# named. An unpinned repository takes the first candidate and says it is
# unpinned. Returns 0 with sc set, 1 with sc_why set (not performed), or 2
# when no candidate exists at all.
spec_spine_pin() {
  [ -f "$1/spec-spine.toml" ] || return 0
  awk '
    /^[[:space:]]*\[/ { t=$0; sub(/#.*/, "", t); gsub(/[[:space:]]/, "", t); inmeta = (t == "[meta]"); next }
    inmeta && /^[[:space:]]*required_version[[:space:]]*=/ {
      s=$0; sub(/^[^=]*=[[:space:]]*/, "", s)
      if (substr(s, 1, 1) == "\"") { s=substr(s, 2); i=index(s, "\""); if (i > 0) { print substr(s, 1, i-1); exit } }
    }' "$1/spec-spine.toml" 2>/dev/null
}
spec_spine_version() {
  "$1" --version 2>/dev/null | head -1 | awk '{print $NF}'
}
# 0 compatible, 1 incompatible, 2 not performed. An exact pin is compared with
# the reported version; any other requirement is put to the binary itself,
# which refuses at configuration load when its version does not satisfy it.
spec_spine_admits() {
  case "$3" in
    =*)
      want=$(printf '%s' "${3#=}" | tr -d '[:space:]')
      if printf '%s' "$want" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        [ -n "$4" ] || return 2
        [ "$4" = "$want" ] && return 0
        return 1
      fi ;;
  esac
  probe=$("$1" --repo "$2" config show 2>&1); prc=$?
  [ "$prc" = 0 ] && return 0
  if [ "$prc" = 3 ]; then case "$probe" in *'requires spec-spine'*) return 1 ;; esac; fi
  return 2
}
spec_spine_resolve() {
  sc=''; sc_rule=''; sc_ver=''; sc_passed=''; sc_why=''
  sc_pin=$(spec_spine_pin "$1")
  if [ -n "$sc_pin" ]; then sc_pinned="pin $sc_pin from $1/spec-spine.toml [meta] required_version"; else sc_pinned='unpinned'; fi
  if [ -n "${STATECRAFT_SPEC_SPINE:-}" ]; then
    if [ -f "$STATECRAFT_SPEC_SPINE" ] && [ -x "$STATECRAFT_SPEC_SPINE" ]; then
      sc=$STATECRAFT_SPEC_SPINE; sc_rule=supervisor; sc_ver=$(spec_spine_version "$sc"); return 0
    fi
    sc_why="the supervisor's binary STATECRAFT_SPEC_SPINE=$STATECRAFT_SPEC_SPINE is not an executable, and no other binary is consulted in a managed session"
    return 1
  fi
  if [ -n "${SPEC_SPINE_BIN:-}" ]; then
    remedy="Unset SPEC_SPINE_BIN, or point it at a binary that satisfies the pin; the pin itself moves only as a D-06 change"
    if [ ! -f "$SPEC_SPINE_BIN" ] || [ ! -x "$SPEC_SPINE_BIN" ]; then
      sc_why="the override SPEC_SPINE_BIN=$SPEC_SPINE_BIN names no executable ($sc_pinned). $remedy"
      return 1
    fi
    v=$(spec_spine_version "$SPEC_SPINE_BIN")
    if [ -z "$sc_pin" ]; then sc=$SPEC_SPINE_BIN; sc_rule=override; sc_ver=$v; return 0; fi
    spec_spine_admits "$SPEC_SPINE_BIN" "$1" "$sc_pin" "$v"
    case $? in
      0) sc=$SPEC_SPINE_BIN; sc_rule=override; sc_ver=$v; return 0 ;;
      1) sc_why="the override SPEC_SPINE_BIN=$SPEC_SPINE_BIN reports ${v:-no version}, which does not satisfy $sc_pinned. $remedy" ;;
      *) sc_why="the override SPEC_SPINE_BIN=$SPEC_SPINE_BIN (reports ${v:-no version}) could not be checked against $sc_pinned: its configuration probe failed. $remedy" ;;
    esac
    return 1
  fi
  found=''; n=0
  for c in "$1/target/release/spec-spine" "$(command -v spec-spine 2>/dev/null)"; do
    n=$((n+1)); if [ "$n" = 1 ]; then rule='repository build'; else rule=PATH; fi
    { [ -n "$c" ] && [ -f "$c" ] && [ -x "$c" ]; } || continue
    found=1
    v=$(spec_spine_version "$c")
    if [ -z "$sc_pin" ]; then sc=$c; sc_rule=$rule; sc_ver=$v; return 0; fi
    spec_spine_admits "$c" "$1" "$sc_pin" "$v"
    case $? in
      0) sc=$c; sc_rule=$rule; sc_ver=$v; return 0 ;;
      1) sc_passed="${sc_passed}passed over $c ($rule, reports ${v:-no version}): it does not satisfy $sc_pinned
" ;;
      *) sc_why="$c ($rule, reports ${v:-no version}) could not be checked against $sc_pinned: its configuration probe failed, so no later candidate is tried"
         return 1 ;;
    esac
  done
  if [ -n "$found" ]; then
    sc_why="no candidate satisfies $sc_pinned"
    return 1
  fi
  return 2
}
# The judge, for every line that reports a verdict: path, version, rule, pin.
spec_spine_judge() {
  j="judged by $sc (${sc_ver:-no version}, $sc_rule; $sc_pinned"
  if [ -n "${STATECRAFT_RUN_ID:-}" ] && [ "$sc_rule" != supervisor ]; then
    if [ -n "$sc_pin" ]; then j="$j; version-checked, identity not verified"; else j="$j; identity not verified"; fi
  fi
  printf '%s)' "$j"
}
spec_spine_resolve "$root"; rrc=$?
[ -n "$sc_passed" ] && printf '%s' "$sc_passed" | sed 's/^/[hook] /'
[ "$rrc" = 2 ] && { echo '[hook] spec-spine absent, staleness check skipped (run /setup)'; exit 0; }
[ "$rrc" = 0 ] || { echo "[hook] staleness check NOT PERFORMED: $sc_why"; exit 0; }
judge=$(spec_spine_judge)
case "$fp" in
  */specs/*/spec.md)
    # The one sanctioned write in these hooks: the session is live and can
    # commit the recompiled shards with the spec edit that made them stale.
    # Contract 2 rule 4 (Q-3 (b)): withheld in an unpinned repository, where
    # the binary that would rewrite the committed shards is not one the
    # repository adopted. The read-only check below still runs.
    if [ -z "$sc_pin" ]; then
      echo "[spec-registry] compile WITHHELD after spec edit: this repository is unpinned (no [meta] required_version in spec-spine.toml), so $sc is not a binary it adopted and does not rewrite its shards. The read-only check still runs; run spec-spine compile yourself with the binary you intend, or pin the repository"
    else
      "$sc" --repo "$root" compile >/dev/null 2>&1 \
        && echo "[spec-registry] recompiled after spec edit, $judge" \
        || echo "[spec-registry] compile FAILED after spec edit, run spec-spine compile ($judge)"
    fi ;;
esac
case "$fp" in
  */specs/*/spec.md|*/spec-spine.toml|*/.claude/settings.json|*/.mcp.json|*/.claude/agents/*.md|*/.claude/skills/*/*.md|*/.github/workflows/*.yml|*/standards/*|*/AGENTS.md|*/CLAUDE.md|*/Makefile|*/docs/*)
    echo "[spec-registry] check $judge:"
    "$sc" --repo "$root" check 2>&1 | tail -6 ;;
esac
true
