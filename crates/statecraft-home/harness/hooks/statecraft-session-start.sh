#!/bin/sh
cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0
# Spec 002 section 3.14 rule 3: gated to a Statecraft project by the
# manifest, and inert everywhere else.
[ -f ".statecraft/environment.json" ] || exit 0
# Spec 002 section 3.31: a managed run's startup acknowledgment. Printed only
# when the launching run named its attempt in this environment, after the gate
# above, as one tab-separated line on stdout. It reports which revision
# directory this script is in; it writes nothing.
if [ -n "${STATECRAFT_STARTUP_NONCE:-}" ]; then
  sc_revision=$(CDPATH='' cd -- "$(dirname -- "$0")/.." 2>/dev/null && pwd -P) || sc_revision=unresolved
  printf 'statecraft-startup\tv1\tnonce=%s\trun=%s\tattempt=%s\tselected=%s\tproject=%s\troot=%s\n' \
    "$STATECRAFT_STARTUP_NONCE" "${STATECRAFT_RUN_ID:-}" "${STATECRAFT_ATTEMPT:-}" \
    "${STATECRAFT_HARNESS_SELECTED:-}" "$(pwd -P)" "$sc_revision"
fi
# Spec 002 section 3.23 contract 2, as amended on 2026-09-24 (section 5):
# resolve the binary, then establish that the repository admits it. The same
# resolver is in all four hooks. One variable selects the binary (section 5,
# 2026-09-25): STATECRAFT_SPEC_SPINE. In a managed session (STATECRAFT_RUN_ID
# set) it is the supervisor's resolved path and the only candidate. Outside
# one, a non-empty value is the operator's override and the only candidate,
# and a broken or incompatible one refuses rather than falling back. The
# retired SPEC_SPINE_BIN is reported as ignored and never selects. Otherwise
# the repository's own
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
  sv_out=$("$1" --version 2>/dev/null) || return 0
  printf '%s\n' "$sv_out" | head -1 | awk '{print $NF}'
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
  # A pin not met is exit 3 below spec-spine 0.26.0 and exit 2 from it, worded
  # the same under both (spec 002 section 5, 2026-09-25, "both exit tables").
  case "$prc" in 2|3) case "$probe" in *'requires spec-spine'*) return 1 ;; esac ;; esac
  return 2
}
spec_spine_resolve() {
  sc=''; sc_rule=''; sc_ver=''; sc_passed=''; sc_why=''; sc_notice=''
  sc_pin=$(spec_spine_pin "$1")
  if [ -n "$sc_pin" ]; then sc_pinned="pin $sc_pin from $1/spec-spine.toml [meta] required_version"; else sc_pinned='unpinned'; fi
  if [ -n "${SPEC_SPINE_BIN:-}" ] && [ -z "${STATECRAFT_SPEC_SPINE:-}" ]; then
    sc_notice="ignored SPEC_SPINE_BIN=$(printf '%s' "$SPEC_SPINE_BIN" | tr '\n\r' '  '): that name is retired and selects nothing; set STATECRAFT_SPEC_SPINE to choose the binary"
  fi
  if [ -n "${STATECRAFT_SPEC_SPINE:-}" ] && [ -n "${STATECRAFT_RUN_ID:-}" ]; then
    if [ -f "$STATECRAFT_SPEC_SPINE" ] && [ -x "$STATECRAFT_SPEC_SPINE" ]; then
      sc=$STATECRAFT_SPEC_SPINE; sc_rule=supervisor; sc_ver=$(spec_spine_version "$sc"); return 0
    fi
    sc_why="the supervisor's binary STATECRAFT_SPEC_SPINE=$STATECRAFT_SPEC_SPINE is not an executable, and no other binary is consulted in a managed session"
    return 1
  fi
  if [ -n "${STATECRAFT_SPEC_SPINE:-}" ]; then
    remedy="Unset STATECRAFT_SPEC_SPINE, or point it at a binary that satisfies the pin; the pin itself moves only as a D-06 change"
    if [ ! -f "$STATECRAFT_SPEC_SPINE" ] || [ ! -x "$STATECRAFT_SPEC_SPINE" ]; then
      sc_why="the override STATECRAFT_SPEC_SPINE=$STATECRAFT_SPEC_SPINE names no executable ($sc_pinned). $remedy"
      return 1
    fi
    v=$(spec_spine_version "$STATECRAFT_SPEC_SPINE")
    if [ -z "$sc_pin" ]; then sc=$STATECRAFT_SPEC_SPINE; sc_rule=override; sc_ver=$v; return 0; fi
    spec_spine_admits "$STATECRAFT_SPEC_SPINE" "$1" "$sc_pin" "$v"
    case $? in
      0) sc=$STATECRAFT_SPEC_SPINE; sc_rule=override; sc_ver=$v; return 0 ;;
      1) sc_why="the override STATECRAFT_SPEC_SPINE=$STATECRAFT_SPEC_SPINE reports ${v:-no version}, which does not satisfy $sc_pinned. $remedy" ;;
      *) sc_why="the override STATECRAFT_SPEC_SPINE=$STATECRAFT_SPEC_SPINE (reports ${v:-no version}) could not be checked against $sc_pinned: its configuration probe failed. $remedy" ;;
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
# Spec 093 3.2: exit 3 is a shape the verb DOCUMENTS, a read that was not
# performed, so reporting it as an unrecognised shape is the wrong answer to a
# known one. The `Stop` hook has named the binary and its version here since
# spec 093; this is the same sentence at the other end of the session. Spec 093
# 3.3's `unknown (check exit N)` fallback stays for every code neither hook
# recognises, which is what it is for.
# spec-spine 0.26.0's table adds 2 for a refusal to judge, which names itself
# (a pin not met among them; it was 3 below 0.26.0), and 4 for a read that
# failed (spec 002 section 5, 2026-09-25, "both exit tables").
spec_spine_unknown_half() {
  v=$("$2" --version 2>/dev/null)
  case "$3" in *'spec-spine: refused:'*|*'requires spec-spine'*)
    echo "NOT READ (check exit $1: spec-spine refused to judge the tree). The binary at $2 answers: ${v:-(nothing)}. Read spec-spine check directly; regenerating repairs nothing here"
    return ;;
  esac
  case "$1" in
    3) echo "NOT READ (check exit 3: I/O, parse, schema or config). The binary at $2 answers: ${v:-(nothing)}. Read spec-spine check directly; regenerating repairs nothing here" ;;
    4) echo "NOT READ (check exit 4: I/O, git or internal). The binary at $2 answers: ${v:-(nothing)}. Read spec-spine check directly; regenerating repairs nothing here" ;;
    *) echo "unknown (check exit $1)" ;;
  esac
}
spec_spine_resolve "${CLAUDE_PROJECT_DIR:-.}"; rrc=$?
[ -n "$sc_notice" ] && printf '%s\n' "$sc_notice" | sed 's/^/[session-freshness] /'
[ -n "$sc_passed" ] && printf '%s' "$sc_passed" | sed 's/^/[session-freshness] /'
if [ "$rrc" = 0 ]; then
  # Spec 093: establish the binary understands the verb BEFORE reading its exit
  # code. A binary predating `spec-spine check` rejects the unknown subcommand,
  # and clap spent exit 2 on that, which is the code this tool spends on
  # staleness.
  # A session told its shards are stale when they are not will regenerate and
  # commit artifacts that were already correct. `--version` is answered by every
  # binary ever released, so asking is safe against any version.
  ver=$sc_ver
  # Contract 4 as amended (H-4): the verb and the gate's unresolved-claim flag
  # are both established before the exit code means anything.
  if ! "$sc" check --help 2>/dev/null | grep -q -- '--fail-on-unresolved'; then
    reg="spec-spine ${ver:-?} predates the \`check\` verb or its --fail-on-unresolved flag, rebuild or reinstall (see /setup)"
    idx="$reg"
  else
  # Spec 062: one verb, both committed trees. Read-only, exactly as the two
  # primitives it composes are: a bare `compile` or `index` would repair a
  # stale committed tree as a side effect of reading it, hiding the defect.
  #
  # The composed exit code is the more severe of the two halves, so it cannot
  # say WHICH tree moved. The report lines can, and this hook reads them back
  # rather than guessing from the code.
  out=$("$sc" check --fail-on-unresolved 2>&1); c=$?
  case "$out" in *'spec-registry: fresh'*) reg='fresh' ;;
    *'spec-registry: STALE'*) reg='STALE, run spec-spine compile and commit the shards' ;;
    *'spec-registry: INVALID'*) reg='INVALID, the corpus fails validation, which regenerating does not clear (run spec-spine check for the violations)' ;;
    *) reg="$(spec_spine_unknown_half "$c" "$sc" "$out")" ;;
  esac
  # Spec 093 3.3: since spec 079 exit 2 carries three distinguishable refusals
  # and the composed code cannot say which. Matching STALE alone reported the
  # half regeneration fixes while dropping the half it does not, and left the
  # unresolved-only case to the `unknown` fallback below.
  case "$out" in
    *'codebase-index: fresh, but REFUSED'*) idx='REFUSED by the gate: the index is fresh but records an unresolved claim (--fail-on-unresolved), which regenerating does not clear (run spec-spine index diagnostics for the list)' ;;
    *'codebase-index: fresh'*) idx='fresh' ;;
    *'codebase-index: STALE'*)
      case "$out" in
        *'codebase-index: UNRESOLVED CLAIM'*) idx='STALE plus UNRESOLVED CLAIM: run spec-spine index for the stale shard(s), which does not clear the unresolved claim(s)' ;;
        *) idx='STALE, run spec-spine index' ;;
      esac ;;
    *'codebase-index: UNRESOLVED CLAIM'*) idx='UNRESOLVED CLAIM: a spec claims a unit that does not resolve, which regenerating does not clear (run spec-spine index diagnostics for the list)' ;;
    *) idx="$(spec_spine_unknown_half "$c" "$sc" "$out")" ;;
  esac
  fi
elif [ "$rrc" = 1 ]; then
  reg="NOT PERFORMED: $sc_why"; idx='NOT PERFORMED (as above)'
else
  reg='spec-spine CLI absent, run /setup'; idx='spec-spine CLI absent, run /setup'
fi
if [ "$rrc" = 0 ]; then
  echo "[session-freshness] spec registry: $reg; codebase index: $idx; $(spec_spine_judge)"
else
  echo "[session-freshness] spec registry: $reg; codebase index: $idx"
fi
true
