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
# Resolve the spec-spine binary for the repository this hook acts on:
# $SPEC_SPINE_BIN, then that repository's own release build, then PATH. A repo
# that builds its own binary must be governed by the one it builds; the PATH
# fallback keeps an adopter on the published CLI working (spec 093).
spec_spine_bin() {
  if [ -n "${SPEC_SPINE_BIN:-}" ] && [ -x "${SPEC_SPINE_BIN}" ]; then echo "${SPEC_SPINE_BIN}"; return 0; fi
  if [ -x "$1/target/release/spec-spine" ]; then echo "$1/target/release/spec-spine"; return 0; fi
  command -v spec-spine 2>/dev/null
}
sc=$(spec_spine_bin "$root") || sc=''
[ -n "$sc" ] || { echo '[hook] spec-spine absent, staleness check skipped (run /setup)'; exit 0; }
case "$fp" in
  */specs/*/spec.md)
    # The one sanctioned write in these hooks: the session is live and can
    # commit the recompiled shards with the spec edit that made them stale.
    "$sc" --repo "$root" compile >/dev/null 2>&1 \
      && echo '[spec-registry] recompiled after spec edit' \
      || echo '[spec-registry] compile FAILED after spec edit, run spec-spine compile' ;;
esac
case "$fp" in
  */specs/*/spec.md|*/spec-spine.toml|*/.claude/settings.json|*/.mcp.json|*/.claude/agents/*.md|*/.claude/skills/*/*.md|*/.github/workflows/*.yml|*/standards/*|*/AGENTS.md|*/CLAUDE.md|*/Makefile|*/docs/*)
    "$sc" --repo "$root" check 2>&1 | tail -6 ;;
esac
true
