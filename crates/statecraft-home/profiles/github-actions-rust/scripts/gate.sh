#!/bin/sh
# Rendered by Statecraft from profile github-actions-rust revision {{sc:profile.revision}}.
# The one definition of this repository's gate: `make gate` and `make code`
# run it locally, and CI runs the same script, so the two cannot drift. Only
# the repository-local .tooling/bin/spec-spine is used; a spec-spine elsewhere
# on PATH never answers for this repository.
set -eu

SS=.tooling/bin/spec-spine

usage() {
  echo "usage: gate.sh governance|code|couple|couple-group" >&2
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
  couple-group)
    # A merge-queue entry (revision 3): the group's own endpoints, which are
    # the entry's change on the speculative base it lands on. A waiver is read
    # from the entry's pull request and honoured only when the group changes
    # no path that pull request does not change; otherwise the group is
    # judged with no waiver, so a waiver never covers more than it was
    # granted for.
    need_spec_spine
    : "${BASE_SHA:?gate.sh couple-group needs BASE_SHA}"
    : "${HEAD_SHA:?gate.sh couple-group needs HEAD_SHA}"
    : "${GROUP_HEAD_REF:?gate.sh couple-group needs GROUP_HEAD_REF}"
    : "${REPO:?gate.sh couple-group needs REPO}"
    tmp="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
    pr=$(printf '%s' "$GROUP_HEAD_REF" | sed -n 's|.*/pr-\([0-9][0-9]*\)-[0-9a-f]*$|\1|p')
    if [ -z "$pr" ]; then
      echo "gate.sh: cannot read the pull request number from $GROUP_HEAD_REF" >&2
      exit 1
    fi
    git diff --name-only "$BASE_SHA" "$HEAD_SHA" | sort -u > "$tmp/statecraft-group-paths"
    gh api --paginate "repos/$REPO/pulls/$pr/files" \
      --jq '.[] | .filename, (.previous_filename // empty)' | sort -u > "$tmp/statecraft-pr-paths"
    body="$tmp/statecraft-pr-body.txt"
    extra=$(comm -23 "$tmp/statecraft-group-paths" "$tmp/statecraft-pr-paths")
    if [ -z "$extra" ]; then
      gh api "repos/$REPO/pulls/$pr" --jq '.body // ""' > "$body"
      echo "the group changes only #$pr's paths: its waiver, if any, applies"
    else
      : > "$body"
      echo "the group changes paths #$pr does not, so no waiver applies:"
      printf '%s\n' "$extra"
    fi
    "$SS" couple --base "$BASE_SHA" --head "$HEAD_SHA" --pr-body "$body"
    ;;
  *)
    usage
    ;;
esac
