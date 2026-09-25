#!/bin/sh
# Rendered by Statecraft from profile github-actions-rust revision {{sc:profile.revision}}.
# The one definition of this repository's gate: `make gate` and `make code`
# run it locally, and CI runs the same script, so the two cannot drift. Only
# the repository-local .tooling/bin/spec-spine is used; a spec-spine elsewhere
# on PATH never answers for this repository.
set -eu

SS=.tooling/bin/spec-spine

# The project's governance parameters (revision 4), rendered from its setup
# block. Each default keeps a revision-3 project's behaviour except the base
# rule, which is a new refusal.
DEFAULT_BRANCH='{{sc:default_branch}}'
ENFORCE_COVERAGE={{sc:governance.enforce_coverage}}
AUTHORED_CONTENT='{{sc:governance.authored_content}}'
AUTHORED_CONTENT_TEXT={{sc:governance.authored_content_text}}
GATE_EACH_COMMIT={{sc:governance.gate_each_commit}}
REQUIRE_SIGNED_COMMITS={{sc:governance.require_signed_commits}}
REQUIRE_DEFAULT_BASE={{sc:governance.require_default_base}}

usage() {
  echo "usage: gate.sh governance|code|couple|couple-group|base|text|commits|pin" >&2
  exit 64
}

[ "$#" -eq 1 ] || usage

need_spec_spine() {
  if [ ! -x "$SS" ]; then
    echo "gate.sh: $SS is not installed; run scripts/statecraft/install-spec-spine.sh" >&2
    exit 3
  fi
}

# A declared authored-content script is required: deleting it, or dropping
# its executable bit, refuses rather than passing silently.
need_authored_content() {
  if [ ! -f "$AUTHORED_CONTENT" ]; then
    echo "gate.sh: governance.authored_content names $AUTHORED_CONTENT, which is absent" >&2
    exit 1
  fi
  if [ ! -x "$AUTHORED_CONTENT" ]; then
    echo "gate.sh: governance.authored_content names $AUTHORED_CONTENT, which is not executable" >&2
    exit 1
  fi
}

# The exact pin a spec-spine.toml states, read as install-spec-spine.sh reads
# it; empty when there is none.
pin_of() {
  awk '
    /^[[:space:]]*\[/ { section = $0; gsub(/[[:space:]]/, "", section); next }
    section == "[meta]" && /^[[:space:]]*required_version[[:space:]]*=/ { print; exit }
  ' "$1" 2>/dev/null | sed -n 's/^[^=]*=[[:space:]]*"=\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)"[[:space:]]*$/\1/p'
}

# The pull request a merge-queue entry was built from, by its group ref.
queue_pr() {
  printf '%s' "$1" | sed -n 's|.*/pr-\([0-9][0-9]*\)-[0-9a-f]*$|\1|p'
}

case "$1" in
  governance)
    need_spec_spine
    "$SS" check --fail-on-warn
    "$SS" lint --fail-on-warn
    if [ "$ENFORCE_COVERAGE" = true ]; then
      "$SS" index coverage --fail-on-untraced
    else
      # Reported, not enforced (governance.enforce_coverage is false): a new
      # project's own sources are unclaimed until it writes the specs that
      # claim them.
      "$SS" index coverage
    fi
    "$SS" index check --fail-on-unresolved
    if [ -n "$AUTHORED_CONTENT" ]; then
      need_authored_content
      "./$AUTHORED_CONTENT"
    else
      echo "gate.sh: no authored-content script is declared (governance.authored_content), so none runs"
    fi
    ;;
  code)
    # A workspace with no member crates yet judges nothing and says so, the
    # guard the hand-written CI this profile replaced had: every
    # `cargo --workspace` verb refuses a virtual manifest with no members.
    # `metadata --no-deps` resolves nothing, so it answers on such a manifest.
    meta=$(cargo metadata --no-deps --format-version 1)
    case "$meta" in
      *'"workspace_members":[]'*)
        echo "gate.sh: the workspace has no member crates yet; build, test, clippy and fmt judge nothing"
        exit 0
        ;;
    esac
    cargo build --workspace --locked
    cargo test --workspace --locked
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo fmt --all --check
    ;;
  pin)
    # For the CI caches: the pin as a step output.
    version=$(pin_of spec-spine.toml)
    if [ -z "$version" ]; then
      echo "gate.sh: spec-spine.toml [meta] states no exact required_version (=X.Y.Z)" >&2
      exit 2
    fi
    echo "version=$version"
    ;;
  base)
    # A stacked pull request merges into another branch and is never judged
    # against the default branch (revision 4, rule 5).
    if [ "$REQUIRE_DEFAULT_BASE" != true ]; then
      echo "gate.sh: governance.require_default_base is false; the base is not judged"
      exit 0
    fi
    : "${BASE_REF:?gate.sh base needs BASE_REF}"
    if [ "$BASE_REF" != "$DEFAULT_BRANCH" ]; then
      echo "gate.sh: this pull request's base is '$BASE_REF', not the default branch $DEFAULT_BRANCH: open it off $DEFAULT_BRANCH, or merge the one below it first" >&2
      exit 1
    fi
    echo "the base is the default branch $DEFAULT_BRANCH"
    ;;
  text)
    # The pull request's title and body, which become the merge commit's
    # message: from the event on pull_request, through the API on
    # merge_group, whose event carries neither (revision 4, rule 3).
    if [ "$AUTHORED_CONTENT_TEXT" != true ]; then
      echo "gate.sh: governance.authored_content_text is false; the title and body are not judged"
      exit 0
    fi
    need_authored_content
    tmp="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
    text="$tmp/statecraft-pr-text.txt"
    case "${EVENT_NAME:-}" in
      pull_request)
        printf '%s\n%s\n' "${PR_TITLE:-}" "${PR_BODY:-}" > "$text"
        ;;
      merge_group)
        : "${GROUP_HEAD_REF:?gate.sh text needs GROUP_HEAD_REF}"
        : "${REPO:?gate.sh text needs REPO}"
        pr=$(queue_pr "$GROUP_HEAD_REF")
        if [ -z "$pr" ]; then
          echo "gate.sh: cannot read the pull request number from $GROUP_HEAD_REF" >&2
          exit 1
        fi
        gh api "repos/$REPO/pulls/$pr" --jq '.title, (.body // "")' > "$text"
        ;;
      *)
        echo "gate.sh: text judges a pull_request or merge_group event, not '${EVENT_NAME:-}'" >&2
        exit 1
        ;;
    esac
    "./$AUTHORED_CONTENT" --text "$text"
    ;;
  commits)
    # Every commit in the change's base..head, not only its head (revision 4,
    # rules 3 and 4): under merge commits each one lands on the default
    # branch. A step of the governance job, never a job of its own, so it
    # cannot be skipped into a green gate.
    if [ "$GATE_EACH_COMMIT" != true ] && [ "$REQUIRE_SIGNED_COMMITS" != true ] && [ "$AUTHORED_CONTENT_TEXT" != true ]; then
      echo "gate.sh: no per-commit check is enabled (governance.gate_each_commit, governance.require_signed_commits, governance.authored_content_text)"
      exit 0
    fi
    : "${BASE_SHA:?gate.sh commits needs BASE_SHA}"
    : "${HEAD_SHA:?gate.sh commits needs HEAD_SHA}"
    if [ "$REQUIRE_SIGNED_COMMITS" = true ]; then
      : "${REPO:?gate.sh commits needs REPO}"
    fi
    if [ "$AUTHORED_CONTENT_TEXT" = true ]; then
      need_authored_content
    fi
    tmp="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
    here=$(pwd)
    head_pin=$(pin_of spec-spine.toml)
    commits=$(git rev-list --reverse "$BASE_SHA..$HEAD_SHA")
    echo "judging $(printf '%s\n' "$commits" | grep -c . || true) commit(s) in $BASE_SHA..$HEAD_SHA"
    fail=0
    for c in $commits; do
      short=$(git rev-parse --short "$c")
      if [ "$REQUIRE_SIGNED_COMMITS" = true ]; then
        # GitHub's verification, the one branch protection's signed-commits
        # rule reads.
        verified=$(gh api "repos/$REPO/commits/$c" --jq '.commit.verification.verified' 2>/dev/null) || verified=unreadable
        if [ "$verified" != true ]; then
          echo "gate.sh: $short is not signed with a key GitHub verifies ($verified)" >&2
          fail=1
        fi
      fi
      if [ "$AUTHORED_CONTENT_TEXT" = true ]; then
        msg="$tmp/statecraft-msg-$short.txt"
        git log -1 --format=%B "$c" > "$msg"
        if ! "./$AUTHORED_CONTENT" --text "$msg"; then
          echo "gate.sh: $short's message breaks the authored-content rules" >&2
          fail=1
        fi
      fi
      if [ "$GATE_EACH_COMMIT" = true ]; then
        # The commit's own tree, its own gate.sh, and the spec-spine release
        # its own spec-spine.toml pins.
        wt="$tmp/statecraft-commit-$short"
        git worktree add -q --detach "$wt" "$c"
        pin=$(pin_of "$wt/spec-spine.toml")
        bin=""
        log="$tmp/statecraft-gate-$short.log"
        : > "$log"
        if [ -z "$pin" ]; then
          # Written to the log too, which the refusal below prints: the gate
          # never ran for this commit, and the log must say why.
          echo "gate.sh: $short's spec-spine.toml states no exact pin, so no gate can judge it" | tee "$log" >&2
        elif [ "$pin" = "$head_pin" ] && [ -x "$SS" ]; then
          bin="$here/$SS"
        else
          root="$tmp/statecraft-spec-spine-$pin"
          [ -x "$root/bin/spec-spine" ] || cargo install spec-spine-cli --version "=$pin" --locked --root "$root"
          bin="$root/bin/spec-spine"
        fi
        script="$wt/scripts/statecraft/gate.sh"
        if [ ! -f "$script" ]; then
          echo "$short carries no scripts/statecraft/gate.sh; the running copy judges it"
          script="$here/scripts/statecraft/gate.sh"
        fi
        if [ -n "$bin" ] && mkdir -p "$wt/.tooling/bin" && ln -sf "$bin" "$wt/.tooling/bin/spec-spine" \
          && (cd "$wt" && sh "$script" governance && cargo fmt --all --check) > "$log" 2>&1; then
          echo "$short: the gate and the format check pass at its own tree"
        else
          echo "gate.sh: $short fails the gate or the format check at its own tree" >&2
          cat "$log" 2>/dev/null || true
          fail=1
        fi
        git worktree remove --force "$wt"
      fi
    done
    if [ "$fail" -ne 0 ]; then
      echo "gate.sh: a commit in $BASE_SHA..$HEAD_SHA was refused; rebuild the branch" >&2
      exit 1
    fi
    echo "every commit in $BASE_SHA..$HEAD_SHA passes"
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
    pr=$(queue_pr "$GROUP_HEAD_REF")
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
