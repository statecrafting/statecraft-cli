#!/usr/bin/env bash
# Rendered by Statecraft from profile github-actions-rust revision {{sc:profile.revision}}.
# The aggregate gate. It passes only when every job the TRUSTED policy names
# as required ended the way its event requires:
#
#   required          must be `success`; `skipped` is an unexpected skip
#   required-review   `success` carrying a review result (findings,
#                     no-findings, or a visible skipped:<class>)
#   owner-exception   required (success) for a pull request whose review
#                     returned findings, and for a release candidate whose
#                     review was skipped; otherwise inapplicable (revision 2)
#   rc-exception      revision 1's rule: the release-candidate case only
#   inapplicable      must be `skipped`, and the rule that admitted it is printed
#
# A required job missing from the needs record has vanished, and blocks. The
# required set is read from the policy at the base commit, never from the
# candidate, so a candidate cannot drop a job from the set that judges it.
#
# Inputs, all from the environment: NEEDS_JSON (toJSON(needs)), EVENT_NAME,
# HEAD_SHA, BASE_SHA (the pull request base, or the push's previous head),
# HEAD_REF (the pull request head ref; empty on push).
set -euo pipefail

: "${NEEDS_JSON:?ci-gate.sh needs NEEDS_JSON}"
: "${EVENT_NAME:?ci-gate.sh needs EVENT_NAME}"
: "${HEAD_SHA:?ci-gate.sh needs HEAD_SHA}"
BASE_SHA="${BASE_SHA:-}"
HEAD_REF="${HEAD_REF:-}"

POLICY=.statecraft/setup/github-actions-rust.json
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

say() {
  printf '%s\n' "$*"
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    printf '%s\n\n' "$*" >> "$GITHUB_STEP_SUMMARY"
  fi
}

# A precondition the gate cannot judge past: say why, and block.
stop() {
  say "BLOCK: $*"
  exit 1
}

# The trusted policy: the base's copy. A base without one is the adoption
# itself, the only case the candidate's copy is read, and it is said.
trusted=no
case "$BASE_SHA" in
  "" | 0000000000000000000000000000000000000000) ;;
  *)
    if git cat-file -e "${BASE_SHA}:${POLICY}" 2>/dev/null; then
      git show "${BASE_SHA}:${POLICY}" > "$work/policy.json"
      trusted=yes
    fi
    ;;
esac
if [ "$trusted" = yes ]; then
  say "policy: read at the base ${BASE_SHA}"
else
  if [ ! -f "$POLICY" ]; then
    stop "no policy at the base and none in the candidate (${POLICY})"
  fi
  cp "$POLICY" "$work/policy.json"
  say "policy: the base carries none, so the candidate's is read; this pull request adopts the gate and is an authority change"
fi

if ! jq -e --arg e "$EVENT_NAME" '.jobs | to_entries | all(.value | has($e))' "$work/policy.json" > /dev/null; then
  stop "the policy states no rule for event '${EVENT_NAME}'"
fi

pattern="$(jq -r '.review.release_branch_pattern // ""' "$work/policy.json")"
release_candidate=no
if [ -n "$pattern" ] && [ -n "$HEAD_REF" ]; then
  # shellcheck disable=SC2053
  if [[ "$HEAD_REF" == $pattern ]]; then
    release_candidate=yes
  fi
fi

review_result="$(printf '%s' "$NEEDS_JSON" | jq -r '.["ai-review"].outputs.result // ""')"

blocked=0
block() {
  say "BLOCK: $*"
  blocked=1
}

while IFS=$'\t' read -r job rule; do
  result="$(printf '%s' "$NEEDS_JSON" | jq -r --arg j "$job" 'if has($j) then .[$j].result else "vanished" end')"
  if [ "$result" = vanished ]; then
    block "required job '${job}' is not in the needs record: it vanished"
    continue
  fi
  if [ "$rule" = rc-exception ] || [ "$rule" = owner-exception ]; then
    exception_rule="$rule"
    case "$review_result" in
      skipped:*)
        if [ "$release_candidate" = yes ]; then rule=required; else rule=inapplicable; fi ;;
      findings)
        if [ "$exception_rule" = owner-exception ]; then rule=required; else rule=inapplicable; fi ;;
      *) rule=inapplicable ;;
    esac
    # A findings verdict is the final approver's refusal: only the owner's
    # exception, approved for this run, lets the head through.
    if [ "$exception_rule" = owner-exception ] && [ "$review_result" = findings ] && [ "$result" != success ]; then
      block "the AI review returned findings and the owner exception '${job}' was not approved for this run (it ended '${result}')"
      continue
    fi
  fi
  case "$rule" in
    required | required-review)
      case "$result" in
        success) ;;
        skipped)
          block "required job '${job}' was skipped where it applies (${EVENT_NAME}): an unexpected skip"
          continue ;;
        *)
          block "required job '${job}' ended '${result}'"
          continue ;;
      esac
      if [ "$rule" = required-review ]; then
        case "$review_result" in
          findings | no-findings | skipped:draft | skipped:fork | skipped:dependabot | skipped:oversized | skipped:transient)
            say "review: ${review_result}" ;;
          *)
            block "job '${job}' succeeded without a review result (got '${review_result}')"
            continue ;;
        esac
      fi
      say "ok: ${job} ${result}"
      ;;
    inapplicable)
      if [ "$result" = skipped ]; then
        say "ok: ${job} skipped, admitted because it is inapplicable on ${EVENT_NAME}"
      else
        block "job '${job}' is inapplicable on ${EVENT_NAME} and must be skipped, but ended '${result}'"
      fi
      ;;
    *)
      block "the policy names an unknown rule '${rule}' for '${job}'"
      ;;
  esac
done < <(jq -r --arg e "$EVENT_NAME" '.jobs | to_entries[] | select(.value.required) | [.key, .value[$e]] | @tsv' "$work/policy.json")

if [ "$release_candidate" = yes ]; then
  say "release candidate: ${HEAD_REF} matches ${pattern}"
fi

# An authority change is reported, never silently accepted: the candidate
# changes a file of the profile or its policy.
if [ "$trusted" = yes ]; then
  jq -r '.files[].path' "$work/policy.json" | sort -u > "$work/profile-paths"
  echo "$POLICY" >> "$work/profile-paths"
  git diff --name-only "${BASE_SHA}" "${HEAD_SHA}" | sort -u > "$work/changed"
  touched="$(sort -u "$work/profile-paths" | comm -12 - "$work/changed")"
  if [ -n "$touched" ]; then
    say "authority change: this candidate changes the gate that judges it:"
    say "$touched"
  fi
fi

if [ "$blocked" -ne 0 ]; then
  say "ci-gate: blocked"
  exit 1
fi
say "ci-gate: passed"
