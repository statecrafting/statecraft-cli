#!/bin/sh
cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0
# Spec 002 section 3.14 rule 3: every delivered behavior is gated to a
# Statecraft project and is inert everywhere else. The manifest is the
# gate, not the presence of a specs directory: an unrelated corpus is not
# this product's to report on.
[ -f ".statecraft/environment.json" ] || exit 0
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
spec_spine_resolve "${CLAUDE_PROJECT_DIR:-.}"; rrc=$?
[ -n "$sc_passed" ] && printf '%s' "$sc_passed" | sed 's/^/[freshness] /'
[ "$rrc" = 2 ] && { echo '[codebase-index] spec-spine absent, staleness check skipped (run /setup)'; exit 0; }
# Advisory (the Stop policy's first part): a binary the repository does not
# admit is reported as a check not performed, never converted into a block.
[ "$rrc" = 0 ] || { echo "[freshness] NOT PERFORMED: $sc_why"; exit 0; }
judge=$(spec_spine_judge)
# Every verdict line names its judge (contract 2 rule 6).
say() { echo "$1 $judge"; }
# Read-only by design. A session that has ended cannot commit a regenerated
# index, so writing one here leaves .statecraft/derived/ dirty; an orchestrator that
# refuses to start a session on a dirty tree then never starts one, and the
# pipeline stalls on dirt it produced itself.
#
# Spec 093 3.2, on spec 093's rule: establish the binary understands the verb
# BEFORE reading its exit code. clap spends exit 2 on an unrecognised
# subcommand and this tool spends exit 2 on staleness, so a binary predating
# the check verb (every release before 0.18.0) hands back the staleness code
# without having read either tree.
# Contract 4 as amended (H-4): the flag this hook passes is established too.
if ! "$sc" check --help 2>/dev/null | grep -q -- '--fail-on-unresolved'; then
  ver=$("$sc" --version 2>/dev/null)
  say "[freshness] NOT READ: the binary at $sc does not carry the check verb with --fail-on-unresolved, so neither committed tree was judged. It answers: ${ver:-(nothing)}. The verb needs spec-spine 0.18.0 or later; run /setup."
  exit 0
fi
# Spec 093 3.1: read the verdict, do not guess it. The body this replaced ran
# `check >/dev/null 2>&1` and printed one staleness remedy on every non-zero
# exit. Three of the four codes are not staleness, and regenerating clears
# none of them: exit 1 is a corpus that does not validate, exit 3 is a read
# that was not performed, and since spec 079 exit 2 itself carries an
# unresolved claim whose diagnostic is recomputed from the corpus every run.
out=$("$sc" check --fail-on-unresolved 2>&1); c=$?
[ "$c" = 0 ] && exit 0
case "$c" in
  2)
    named=0
    # The attributed prefixes the verb prints, not the bare word: the composed
    # exit code cannot say which refusal it is holding, and the report can
    # (spec 079 3.3).
    case "$out" in *'spec-registry: STALE'*|*'codebase-index: STALE'*)
      say '[freshness] STALE: run `spec-spine compile` and `index` and commit the regenerated shards with the change that made them stale.'
      echo '[freshness] Not regenerated here: a write at session end leaves .statecraft/derived/ uncommitted, and the next run refuses a dirty tree.'
      named=1 ;;
    esac
    case "$out" in *'codebase-index: UNRESOLVED CLAIM'*|*'codebase-index: fresh, but REFUSED'*)
      say '[freshness] UNRESOLVED CLAIM: a spec claims a unit that does not resolve. That is not staleness and regenerating does not clear it, because the diagnostic is recomputed from the corpus on every run. Fix the spec or the tree; `spec-spine index diagnostics` lists them.'
      named=1 ;;
    esac
    [ "$named" = 1 ] || say "[freshness] REFUSED: spec-spine check exited 2 with a report this hook does not recognise; it is not reported as fresh, and no remedy is guessed for it."
    ;;
  1)
    # Under --fail-on-unresolved exit 1 carries two readings (contract 4 as
    # amended), and a stale tree can ride along with either; each is named.
    named=0
    case "$out" in *'codebase-index: UNRESOLVED CLAIM'*|*'codebase-index: fresh, but REFUSED'*)
      say '[freshness] UNRESOLVED CLAIM: a spec claims a unit that does not resolve, and the gate refuses it. That is not staleness and regenerating does not clear it, because the diagnostic is recomputed from the corpus on every run. Fix the spec or the tree; `spec-spine index diagnostics` lists them.'
      named=1 ;;
    esac
    case "$out" in *'spec-registry: INVALID'*|*'spec-registry: REFUSED'*)
      say '[freshness] INVALID: the corpus does not validate, which is not staleness and regenerating does not clear it. Run `spec-spine check` and fix the violations it names.'
      named=1 ;;
    esac
    case "$out" in *'spec-registry: STALE'*|*'codebase-index: STALE'*)
      [ "$named" = 1 ] && say '[freshness] STALE as well: `spec-spine compile` and `index` clear that part only; commit the regenerated shards with the fix.' ;;
    esac
    [ "$named" = 1 ] || say '[freshness] INVALID: the corpus does not validate, which is not staleness and regenerating does not clear it. Run `spec-spine check` and fix the violations it names.' ;;
  3)
    # Spec 093 3.2: the version read qualifies a non-answer, so it is asked
    # here and never on the happy path.
    ver=$("$sc" --version 2>/dev/null)
    say "[freshness] NOT READ: the freshness read was not performed (exit 3: I/O, parse, schema or config). The binary at $sc answers: ${ver:-(nothing)}. Read the error from spec-spine check directly; regenerating repairs nothing here." ;;
  *)
    say "[freshness] UNRECOGNISED: spec-spine check exited $c, which this hook does not know how to report; it is not reported as fresh." ;;
esac
true
