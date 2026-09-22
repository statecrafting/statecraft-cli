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
# Resolve the spec-spine binary for the repository this hook acts on:
# $SPEC_SPINE_BIN, then that repository's own release build, then PATH. A repo
# that builds its own binary must be governed by the one it builds; the PATH
# fallback keeps an adopter on the published CLI working (spec 093).
spec_spine_bin() {
  if [ -n "${SPEC_SPINE_BIN:-}" ] && [ -x "${SPEC_SPINE_BIN}" ]; then echo "${SPEC_SPINE_BIN}"; return 0; fi
  if [ -x "$1/target/release/spec-spine" ]; then echo "$1/target/release/spec-spine"; return 0; fi
  command -v spec-spine 2>/dev/null
}
# Spec 093 3.2: exit 3 is a shape the verb DOCUMENTS, a read that was not
# performed, so reporting it as an unrecognised shape is the wrong answer to a
# known one. The `Stop` hook has named the binary and its version here since
# spec 093; this is the same sentence at the other end of the session. Spec 093
# 3.3's `unknown (check exit N)` fallback stays for every code neither hook
# recognises, which is what it is for.
spec_spine_unknown_half() {
  if [ "$1" = 3 ]; then
    v=$("$2" --version 2>/dev/null)
    echo "NOT READ (check exit 3: I/O, parse, schema or config). The binary at $2 answers: ${v:-(nothing)}. Read spec-spine check directly; regenerating repairs nothing here"
  else
    echo "unknown (check exit $1)"
  fi
}
if sc=$(spec_spine_bin "${CLAUDE_PROJECT_DIR:-.}") && [ -n "$sc" ]; then
  # Spec 093: establish the binary understands the verb BEFORE reading its exit
  # code. A binary predating `spec-spine check` rejects the unknown subcommand,
  # and clap spent exit 2 on that, which is the code this tool spends on
  # staleness.
  # A session told its shards are stale when they are not will regenerate and
  # commit artifacts that were already correct. `--version` is answered by every
  # binary ever released, so asking is safe against any version.
  ver=$("$sc" --version 2>/dev/null | awk '{print $NF}')
  if ! "$sc" check --help >/dev/null 2>&1; then
    reg="spec-spine ${ver:-?} predates the \`check\` verb, rebuild or reinstall (see /setup)"
    idx="$reg"
  else
  # Spec 062: one verb, both committed trees. Read-only, exactly as the two
  # primitives it composes are: a bare `compile` or `index` would repair a
  # stale committed tree as a side effect of reading it, hiding the defect.
  #
  # The composed exit code is the more severe of the two halves, so it cannot
  # say WHICH tree moved. The report lines can, and this hook reads them back
  # rather than guessing from the code.
  out=$("$sc" check 2>&1); c=$?
  case "$out" in *'spec-registry: fresh'*) reg='fresh' ;;
    *'spec-registry: STALE'*) reg='STALE, run spec-spine compile and commit the shards' ;;
    *'spec-registry: INVALID'*) reg='INVALID, the corpus fails validation, which regenerating does not clear (run spec-spine check for the violations)' ;;
    *) reg="$(spec_spine_unknown_half "$c" "$sc")" ;;
  esac
  # Spec 093 3.3: since spec 079 exit 2 carries three distinguishable refusals
  # and the composed code cannot say which. Matching STALE alone reported the
  # half regeneration fixes while dropping the half it does not, and left the
  # unresolved-only case to the `unknown` fallback below.
  case "$out" in *'codebase-index: fresh'*) idx='fresh' ;;
    *'codebase-index: STALE'*)
      case "$out" in
        *'codebase-index: UNRESOLVED CLAIM'*) idx='STALE plus UNRESOLVED CLAIM: run spec-spine index for the stale shard(s), which does not clear the unresolved claim(s)' ;;
        *) idx='STALE, run spec-spine index' ;;
      esac ;;
    *'codebase-index: UNRESOLVED CLAIM'*) idx='UNRESOLVED CLAIM: a spec claims a unit that does not resolve, which regenerating does not clear (run spec-spine index diagnostics for the list)' ;;
    *) idx="$(spec_spine_unknown_half "$c" "$sc")" ;;
  esac
  fi
else
  reg='spec-spine CLI absent, run /setup'; idx='spec-spine CLI absent, run /setup'
fi
echo "[session-freshness] spec registry: $reg; codebase index: $idx"
true
