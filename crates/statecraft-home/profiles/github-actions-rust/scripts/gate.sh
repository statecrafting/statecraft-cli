#!/bin/sh
# Rendered by Statecraft from profile github-actions-rust revision {{sc:profile.revision}}.
# The one definition of this repository's gate: `make gate` and `make code`
# run it locally, and CI runs the same script, so the two cannot drift. Only
# the repository-local .tooling/bin/spec-spine is used; a spec-spine elsewhere
# on PATH never answers for this repository.
set -eu

SS=.tooling/bin/spec-spine

usage() {
  echo "usage: gate.sh governance|code|couple" >&2
  exit 64
}

[ "$#" -eq 1 ] || usage

need_spec_spine() {
  if [ ! -x "$SS" ]; then
    echo "gate.sh: $SS is not installed; run scripts/statecraft/install-spec-spine.sh" >&2
    exit 3
  fi
}

case "$1" in
  governance)
    need_spec_spine
    "$SS" check --fail-on-warn
    "$SS" lint --fail-on-warn
    # Coverage is reported, not enforced: a new project's own sources are
    # unclaimed until it writes the specs that claim them. A project adds
    # --fail-on-untraced when its coverage debt is retired.
    "$SS" index coverage
    "$SS" index check --fail-on-unresolved
    if [ -x scripts/check-authored-content.sh ]; then
      scripts/check-authored-content.sh
    fi
    ;;
  code)
    cargo build --workspace --locked
    cargo test --workspace --locked
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo fmt --all --check
    ;;
  couple)
    # Pull requests only, with the event's two frozen endpoints: a three-dot
    # diff whose merge base is the pull request's own fork point.
    need_spec_spine
    : "${BASE_SHA:?gate.sh couple needs BASE_SHA}"
    : "${HEAD_SHA:?gate.sh couple needs HEAD_SHA}"
    body="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/statecraft-pr-body.txt"
    printf '%s' "${PR_BODY:-}" > "$body"
    "$SS" couple --base "$BASE_SHA" --head "$HEAD_SHA" --pr-body "$body"
    ;;
  *)
    usage
    ;;
esac
