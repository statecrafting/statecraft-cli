#!/usr/bin/env bash
# Resolve merge conflicts in the compiler-owned shard tree, and nothing else.
#
# Run it during `git merge origin/main` on a branch, after every conflict in an
# authored file has been resolved BY HAND. Shards are compiler output, so their
# conflicts carry no information: this takes the incoming side's copy, runs
# `make refresh` to recompute them from the merged sources, and stages them.
#
# It refuses to touch anything else. In session 10 (2026-09-24) a mechanical
# "take theirs" over a conflicted merge deleted 2,700 lines of spec 002; a
# conflict in authored text is a decision, not a chore.
#
# Then it checks what the merge did to authored text: the branch's own
# non-shard change (its merge base to its head) must equal the merged tree's
# non-shard difference from the incoming side, compared as the set of lines
# each adds and removes per file, without context (context moves whenever the
# other side edited nearby). A difference is printed for review, never accepted
# silently: it is legitimate only where both sides edited the same lines.
#
# Exit 0 resolved and unchanged, 1 needs a person (an authored conflict, a
# conflict marker, or a changed authored diff), 3 usage (no merge in progress).

set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "resolve-shard-conflicts: not inside a git work tree" >&2
  exit 3
}
git rev-parse -q --verify MERGE_HEAD > /dev/null || {
  echo "resolve-shard-conflicts: no merge in progress (run it during 'git merge')" >&2
  exit 3
}

derived=.statecraft/derived
conflicted=$(git diff --name-only --diff-filter=U)
authored=$(printf '%s\n' "$conflicted" | grep -v "^$derived/" | grep . || true)
if [ -n "$authored" ]; then
  echo "resolve these authored conflicts by hand first; this script never resolves them:"
  printf '%s\n' "$authored" | sed 's/^/  /'
  exit 1
fi

shards=$(printf '%s\n' "$conflicted" | grep "^$derived/" || true)
if [ -n "$shards" ]; then
  echo "taking the incoming copy of $(printf '%s\n' "$shards" | wc -l | tr -d ' ') conflicted shard(s), then recomputing"
  # shellcheck disable=SC2086
  git checkout MERGE_HEAD -- $shards
fi
make refresh > /dev/null
git add -- "$derived"

markers=$(git grep -n --cached -E '^(<<<<<<<|>>>>>>>) ' || true)
if [ -n "$markers" ]; then
  echo "conflict markers remain in the index:"
  printf '%s\n' "$markers" | sed 's/^/  /'
  exit 1
fi

# Each added or removed line, prefixed with its file, sorted: a diff's content
# with its positions and context removed.
lines() {
  awk '/^\+\+\+ /{f=$2; next} /^--- /{next} /^[+-]/{print f " " $0}' | LC_ALL=C sort
}
base=$(git merge-base HEAD MERGE_HEAD)
own=$(git diff -U0 "$base" HEAD -- . ":(exclude)$derived" | lines)
merged=$(git diff -U0 --cached MERGE_HEAD -- . ":(exclude)$derived" | lines)
if [ "$own" != "$merged" ]; then
  echo "the merged authored diff differs from this branch's own change; review it before committing:"
  echo "  branch's own change:"
  git diff --stat "$base" HEAD -- . ":(exclude)$derived" | sed 's/^/    /'
  echo "  merged tree against the incoming side:"
  git diff --cached --stat MERGE_HEAD -- . ":(exclude)$derived" | sed 's/^/    /'
  exit 1
fi

echo "shards recomputed and staged; the authored diff is unchanged. Run 'make gate', then commit the merge."
