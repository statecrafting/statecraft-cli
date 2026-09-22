#!/bin/sh
cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0
# Spec 002 section 3.14 rule 3: every delivered behavior is gated to a
# Statecraft project and is inert everywhere else. The manifest is the
# gate, not the presence of a specs directory: an unrelated corpus is not
# this product's to report on.
[ -f ".statecraft/environment.json" ] || exit 0
# Resolve the spec-spine binary for the repository this hook acts on:
# $SPEC_SPINE_BIN, then that repository's own release build, then PATH. A repo
# that builds its own binary must be governed by the one it builds; the PATH
# fallback keeps an adopter on the published CLI working (spec 093).
spec_spine_bin() {
  if [ -n "${SPEC_SPINE_BIN:-}" ] && [ -x "${SPEC_SPINE_BIN}" ]; then echo "${SPEC_SPINE_BIN}"; return 0; fi
  if [ -x "$1/target/release/spec-spine" ]; then echo "$1/target/release/spec-spine"; return 0; fi
  command -v spec-spine 2>/dev/null
}
sc=$(spec_spine_bin "${CLAUDE_PROJECT_DIR:-.}") || sc=''
[ -n "$sc" ] || { echo '[codebase-index] spec-spine absent, staleness check skipped (run /setup)'; exit 0; }
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
if ! "$sc" check --help >/dev/null 2>&1; then
  ver=$("$sc" --version 2>/dev/null)
  echo "[freshness] NOT READ: the binary at $sc does not carry the check verb, so neither committed tree was judged. It answers: ${ver:-(nothing)}. The verb needs spec-spine 0.18.0 or later; run /setup."
  exit 0
fi
# Spec 093 3.1: read the verdict, do not guess it. The body this replaced ran
# `check >/dev/null 2>&1` and printed one staleness remedy on every non-zero
# exit. Three of the four codes are not staleness, and regenerating clears
# none of them: exit 1 is a corpus that does not validate, exit 3 is a read
# that was not performed, and since spec 079 exit 2 itself carries an
# unresolved claim whose diagnostic is recomputed from the corpus every run.
out=$("$sc" check 2>&1); c=$?
[ "$c" = 0 ] && exit 0
case "$c" in
  2)
    named=0
    # The attributed prefixes the verb prints, not the bare word: the composed
    # exit code cannot say which refusal it is holding, and the report can
    # (spec 079 3.3).
    case "$out" in *'spec-registry: STALE'*|*'codebase-index: STALE'*)
      echo '[freshness] STALE: run `spec-spine compile` and `index` and commit the regenerated shards with the change that made them stale.'
      echo '[freshness] Not regenerated here: a write at session end leaves .statecraft/derived/ uncommitted, and the next run refuses a dirty tree.'
      named=1 ;;
    esac
    case "$out" in *'codebase-index: UNRESOLVED CLAIM'*)
      echo '[freshness] UNRESOLVED CLAIM: a spec claims a unit that does not resolve. That is not staleness and regenerating does not clear it, because the diagnostic is recomputed from the corpus on every run. Fix the spec or the tree; `spec-spine index diagnostics` lists them.'
      named=1 ;;
    esac
    [ "$named" = 1 ] || echo "[freshness] REFUSED: spec-spine check exited 2 with a report this hook does not recognise; it is not reported as fresh, and no remedy is guessed for it."
    ;;
  1)
    echo '[freshness] INVALID: the corpus does not validate, which is not staleness and regenerating does not clear it. Run `spec-spine check` and fix the violations it names.' ;;
  3)
    # Spec 093 3.2: the version read qualifies a non-answer, so it is asked
    # here and never on the happy path.
    ver=$("$sc" --version 2>/dev/null)
    echo "[freshness] NOT READ: the freshness read was not performed (exit 3: I/O, parse, schema or config). The binary at $sc answers: ${ver:-(nothing)}. Read the error from spec-spine check directly; regenerating repairs nothing here." ;;
  *)
    echo "[freshness] UNRECOGNISED: spec-spine check exited $c, which this hook does not know how to report; it is not reported as fresh." ;;
esac
true
