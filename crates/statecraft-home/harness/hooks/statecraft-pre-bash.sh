#!/bin/sh
command -v jq >/dev/null 2>&1 || { echo '[hook] jq missing, push gate and PR gate skipped'; exit 0; }
payload=$(cat)
cmd=$(printf '%s' "$payload" | jq -r '.tool_input.command // empty' 2>/dev/null)
hookcwd=$(printf '%s' "$payload" | jq -r '.cwd // empty' 2>/dev/null)
# The repo the command acts on, NOT the session's project: a multi-repo session
# pushes and opens PRs in whichever tree the command names. Honour an explicit
# `cd <dir>` prefix, else the hook's own cwd, then ask git for the toplevel.
target=$(printf '%s' "$cmd" | sed -n 's/^[[:space:]]*cd[[:space:]]\{1,\}\([^&;|]*\).*/\1/p' | head -1 | sed 's/[[:space:]]*$//')
target=$(printf '%s' "$target" | sed "s|^~|$HOME|")
[ -n "$target" ] || target="$hookcwd"
[ -n "$target" ] || target=.
root=$(git -C "$target" rev-parse --show-toplevel 2>/dev/null) || root="$target"

# Spec 002 section 3.14 rule 3, applied to the WHOLE hook and applied HERE,
# before the push gate. A global registration is inert outside a Statecraft
# project, and both halves of this hook are behaviors: refusing a push in an
# unrelated repository is exactly the collision rule 3 forbids, and it is the
# more damaging half because it stops an operation rather than printing a
# line. The gate is on the repository the COMMAND acts on, resolved above,
# not on the session's own project.
[ -f "$root/.statecraft/environment.json" ] || exit 0

# Spec 093 3.1: the protected branch is RESOLVED for the repository the command
# acts on, never assumed to be `main`. Highest wins: $SPEC_SPINE_DEFAULT_BRANCH,
# then the remote's own HEAD (a clone sets it, so most adopters need no
# configuration at all), then `main` as a compatibility floor. Each step falls
# back rather than refusing, and step 3 always answers, so the gate reaches a
# verdict for every repository. Resolution asks git and never the spec-spine
# binary: the push half of this hook runs on git alone, which is why it still
# protects a repository where the binary is absent or /setup has not been run.
default_branch() {
  if [ -n "${SPEC_SPINE_DEFAULT_BRANCH:-}" ]; then echo "${SPEC_SPINE_DEFAULT_BRANCH}"; return 0; fi
  h=$(git -C "$1" symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null) && [ -n "$h" ] && { echo "${h#origin/}"; return 0; }
  echo main
}

# Anchored on the command that actually invokes the push verb (spec 093
# 3.1). The old match was a substring test over the whole command, so it
# refused anything merely CONTAINING the text: the greps, heredocs and test
# fixtures that describe this gate could not run in a session it governs.
case "$cmd" in 'git push'*|*'&& git push'*|*'; git push'*)
    br=$(git -C "$root" branch --show-current 2>/dev/null); blk=0
    def=$(default_branch "$root")
    # Spec 093 3.2: the refspec forms are BUILT from the resolved name rather
    # than written as literal patterns. Every expansion is quoted inside the
    # pattern, which matches it literally: $SPEC_SPINE_DEFAULT_BRANCH is
    # user-supplied and git's own refname rules never see it, so an unquoted
    # `ma*n` would silently turn each refspec test into a wildcard.
    case "$cmd" in *"origin $def"|*"origin $def "*|*"HEAD:$def"|*"HEAD:$def "*|*":$def"|*":$def "*|*"origin +$def"*) blk=1 ;; esac
    # Spec 093 3.2, over the name 072 3.1 resolves: on the default branch,
    # refuse only a push that would actually UPDATE it. Fewer than two
    # positional arguments after the verb means there is no explicit refspec,
    # so the push follows the current branch; HEAD and the resolved name name
    # it outright. A tag push carries its own refspec and updates no branch,
    # and docs/releasing.md tells a maintainer to run one from the default
    # branch right after the release PR merges.
    if [ "$br" = "$def" ]; then
      # Strip to the ANCHORED occurrence of the verb, not merely the first one
      # in the string. A command can name the verb in an argument before it
      # ever runs one, and reading THOSE words as a refspec is nonsense: the
      # arguments that matter are the ones after the invocation the outer
      # case matched. ${var#pattern} strips the shortest matching prefix.
      case "$cmd" in
        'git push'*)     after=${cmd#git push} ;;
        *'&& git push'*) after=${cmd#*'&& git push'} ;;
        *'; git push'*)  after=${cmd#*'; git push'} ;;
        *)               after='' ;;
      esac
      # Keep only that push's own arguments, dropping anything chained after.
      rest=${after%%[;&|]*}
      npos=0; last=''
      for w in $rest; do case "$w" in -*) ;; *) npos=$((npos+1)); last=$w ;; esac; done
      { [ "$npos" -lt 2 ] || [ "$last" = HEAD ] || [ "$last" = "$def" ]; } && blk=1
      # Only that push was analysed, so a command chaining another is refused
      # outright: the walk cannot speak for a push it never looked at.
      case "$after" in *'&& git push'*|*'; git push'*) blk=1 ;; esac
    fi
    if [ "$blk" = 1 ]; then
      { echo "[push-gate] BLOCKED: this would update $def (repo: $root, branch: ${br:-unknown}). Work on a feature branch and open a PR through /ship. A tag push such as 'git push origin v1.2.3' is allowed."; } >&2
      exit 2
    fi ;;
esac

case "$cmd" in 'gh pr create'*|*'&& gh pr create'*|*'; gh pr create'*) ;; *) exit 0 ;; esac
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
[ -n "$sc" ] || { echo '[pr-gate] spec-spine absent, coupling gate skipped (run /setup)'; exit 0; }

# Read-only. The gate must never write into the repo it is judging: `index`
# rewrites committed shards, a hook cannot commit them, and doing it from here
# mutates a tree another session may be mid-build in.
# Spec 093 3.1: `check` has four answers and the gate reads the one it got.
# Exit 2 is stale. Exit 1 is a corpus that does not validate. Exit 3 is a read
# that was not performed, most often a binary that predates the verb (every
# release before 0.18.0), and calling that "stale" sent adopters to regenerate
# shards that were already correct. Every non-zero code still refuses: a gate
# whose check did not run is not green (spec 052 3.2, the same reasoning).
"$sc" --repo "$root" check >/dev/null 2>&1; cec=$?
case "$cec" in
  0) ;;
  2) # Spec 093 3.1, on spec 093's rule: exit 2 is the ONE ambiguous code. This
     # tool spends it on staleness and clap spends it on an unrecognised
     # subcommand, so a binary predating `check` was refused here with a
     # staleness message and sent to regenerate shards that were already
     # correct. The probe runs only on this arm, so spec 093 3.2's happy path
     # is untouched: exit 0 still costs one process.
     if "$sc" check --help >/dev/null 2>&1; then
       { echo "[pr-gate] BLOCKED: a committed shard tree is stale in $root."
         echo '[pr-gate] Run: spec-spine compile and index, whichever tree it named, then commit the shards, push, and retry.'; } >&2
     else
       ver=$("$sc" --version 2>/dev/null) || ver='(no answer to --version)'
       { echo "[pr-gate] BLOCKED: the freshness read was not performed in $root (the binary does not carry the check verb, and clap spent exit 2 on the unknown subcommand)."
         echo "[pr-gate] The binary is $sc, which answers: ${ver:-(nothing)}. The check verb needs spec-spine 0.18.0 or later. The tree has NOT been judged and is not known to be stale; regenerating repairs nothing here. Run /setup to install the floor."; } >&2
     fi
     exit 2 ;;
  1) { echo "[pr-gate] BLOCKED: the corpus in $root does not validate (spec-spine check exit 1)."
       echo '[pr-gate] Run: spec-spine check, fix the violations it names, and retry. The tree is not stale; staleness is not meaningful against a corpus that does not compile.'; } >&2
     exit 2 ;;
  3) # Spec 093 3.2: the version read qualifies a non-answer, so it is asked
     # only here, never on the happy path.
     ver=$("$sc" --version 2>/dev/null) || ver='(no answer to --version)'
     { echo "[pr-gate] BLOCKED: the freshness read was not performed in $root (spec-spine check exit 3)."
       echo "[pr-gate] The binary is $sc, which answers: ${ver:-(nothing)}. The check verb needs spec-spine 0.18.0 or later; below that the binary is too old to have read the tree, and the tree itself has not been judged. Run /setup to install the floor, or read spec-spine check directly for an I/O, parse or config error."; } >&2
     exit 2 ;;
  *) { echo "[pr-gate] BLOCKED: spec-spine check exited $cec in $root, which this gate does not recognise; it is not reported as fresh."; } >&2
     exit 2 ;;
esac
# Spec 093 3.13: the derived tree is asked about in all three states git
# distinguishes, and each is read on its own. `git diff` alone compares the
# index to the working tree, so a staged shard and an untracked one both
# passed a gate whose message said the shards were committed, and those are
# exactly the two states `git add` and a new spec's shards leave behind.
# A single HEAD-relative comparison is not the answer either: a staged edit
# restored in the working tree cancels against HEAD while both per-state reads
# still report the file, and one such comparison cannot say WHICH state it
# found, which the message has to name because the remedies differ.
#
# The derived directory is the tool's own typed answer, never a hardcoded path
# (spec 094 3.2).
#
# STATECRAFT AMENDMENT, spec 002 section 3.23 contract 6 and the Stop policy's
# second part. The inherited script announced the skip and continued here,
# citing the counterparty's spec 093 3.4. That wording conflicts with the rule
# the owner adopted on 2026-09-21: an enforcing operation gate refuses a failed
# or unavailable check, and "a gate whose check did not run is not green" admits
# no exception for a check whose configuration could not be read. This gate
# stands in front of `gh pr create`; a derived-tree state it never established
# is not a derived-tree state it may report as clean. The weaker behavior is not
# preserved merely because it was copied, and the message distinguishes the two
# remedies: an unperformed read is not a dirty tree and regenerating repairs
# nothing.
derived=$("$sc" --repo "$root" config show --json 2>/dev/null | jq -r '.layout.derived_dir // empty' 2>/dev/null) || derived=''
if [ -z "$derived" ]; then
  { echo "[pr-gate] BLOCKED: the derived-tree check was NOT PERFORMED in $root (spec-spine config show did not report layout.derived_dir)."
    echo "[pr-gate] The binary is $sc. The derived tree has not been judged and is not known to be clean; a gate whose check did not run is not green. Repair the configuration read (spec-spine config show --json must report layout.derived_dir) and retry. Regenerating shards repairs nothing here."; } >&2
  exit 2
fi

# Spec 093 3.13: the derived tree is asked about in all three states git
# distinguishes, and each is read on its own. Each read's exit status is read
# too: `git diff` answers with an empty line list both when the tree is clean
# and when the command failed, and treating the second as the first is the same
# defect as skipping the check. A read that did not run refuses, on the same
# contract 6.
dt_unstaged=$(git -C "$root" diff --name-only -- "$derived" 2>/dev/null); rc_unstaged=$?
dt_staged=$(git -C "$root" diff --cached --name-only -- "$derived" 2>/dev/null); rc_staged=$?
dt_untracked=$(git -C "$root" ls-files --others --exclude-standard -- "$derived" 2>/dev/null); rc_untracked=$?
if [ "$rc_unstaged" -ne 0 ] || [ "$rc_staged" -ne 0 ] || [ "$rc_untracked" -ne 0 ]; then
  { echo "[pr-gate] BLOCKED: a derived-tree read was NOT PERFORMED in $root (git diff exited $rc_unstaged, git diff --cached exited $rc_staged, git ls-files exited $rc_untracked)."
    echo "[pr-gate] An empty answer from a command that failed is not a clean tree. The derived tree under $derived has not been judged; fix the repository read and retry."; } >&2
  exit 2
fi
if [ -n "$dt_unstaged" ] || [ -n "$dt_staged" ] || [ -n "$dt_untracked" ]; then
  { echo "[pr-gate] BLOCKED: the derived tree under $derived is not committed in $root."
    if [ -n "$dt_unstaged" ]; then
      echo '[pr-gate] unstaged changes (git add, then commit):'
      printf '%s\n' "$dt_unstaged" | sed 's/^/  /'
    fi
    if [ -n "$dt_staged" ]; then
      echo '[pr-gate] staged changes (git commit):'
      printf '%s\n' "$dt_staged" | sed 's/^/  /'
    fi
    if [ -n "$dt_untracked" ]; then
      echo '[pr-gate] untracked files (git add, then commit):'
      printf '%s\n' "$dt_untracked" | sed 's/^/  /'
    fi
    echo '[pr-gate] Commit the regenerated shards with the change that made them stale, push, and retry.'; } >&2
  exit 2
fi

out=$("$sc" --repo "$root" couple --base "origin/$(default_branch "$root")" --head HEAD 2>&1); ec=$?
if [ $ec -ne 0 ]; then
  case "$cmd" in
    *--body*Spec-Drift-Waiver*) echo '[pr-gate] coupling gate failed; Spec-Drift-Waiver present after --body, allowing (CI honours the body-at-creation waiver).' ;;
    *) { echo "[pr-gate] BLOCKED: coupling gate failed in $root and no Spec-Drift-Waiver in the PR body:"
         echo "$out" | tail -25
         echo '[pr-gate] Either fix the coupling (claim every changed path in the spec being implemented, or add an extends edge naming the owning spec) or, with explicit human approval, include the Spec-Drift-Waiver line inline in --body (not --body-file) and retry. A waiver is a human instrument: never write one on your own authority.'; } >&2
       exit 2 ;;
  esac
fi
true
