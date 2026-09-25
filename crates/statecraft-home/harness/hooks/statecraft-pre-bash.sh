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
spec_spine_resolve "$root"; rrc=$?
[ -n "$sc_notice" ] && printf '%s\n' "$sc_notice" | sed 's/^/[pr-gate] /' >&2
[ -n "$sc_passed" ] && printf '%s' "$sc_passed" | sed 's/^/[pr-gate] /' >&2
[ "$rrc" = 2 ] && { echo '[pr-gate] spec-spine absent, coupling gate skipped (run /setup)'; exit 0; }
# Contract 2 rules 2 and 3 with contract 6: a binary the repository does not
# admit is a check that did not run, and a gate whose check did not run is not
# green.
if [ "$rrc" != 0 ]; then
  { echo "[pr-gate] BLOCKED: the freshness read was NOT PERFORMED in $root: $sc_why."
    echo '[pr-gate] The tree has not been judged and is not known to be stale; regenerating repairs nothing here.'; } >&2
  exit 2
fi
judge=$(spec_spine_judge)

# Read-only. The gate must never write into the repo it is judging: `index`
# rewrites committed shards, a hook cannot commit them, and doing it from here
# mutates a tree another session may be mid-build in.
# Spec 093 3.1: `check` has four answers and the gate reads the one it got.
# Exit 2 is stale. Exit 1 is a corpus that does not validate. Exit 3 is a read
# that was not performed, most often a binary that predates the verb (every
# release before 0.18.0), and calling that "stale" sent adopters to regenerate
# shards that were already correct. Every non-zero code still refuses: a gate
# whose check did not run is not green (spec 052 3.2, the same reasoning).
# Contract 4 as amended (H-4): the gate's unresolved-claim flag, so this gate
# refuses what the merge gate refuses. Exit 1 then carries two readings and
# the report text says which; exit 2 is stale and nothing else.
cout=$("$sc" --repo "$root" check --fail-on-unresolved 2>&1); cec=$?
# spec-spine has two exit tables (spec 002 section 5, 2026-09-25, "both exit
# tables"). From 0.26.0 a stale tree exits 1 and 2 is a refusal to judge, which
# names itself; below it stale is 2 and a refusal 3. A refusal is read first,
# by its words, so a 0.26.0 refusal is never sent to regenerate.
if [ "$cec" != 0 ]; then
  case "$cout" in *'spec-spine: refused:'*|*'requires spec-spine'*)
    { echo "[pr-gate] BLOCKED: spec-spine refused to judge the tree in $root (check exit $cec): $(printf '%s\n' "$cout" | head -1)"
      echo '[pr-gate] The tree has NOT been judged and is not known to be stale; regenerating repairs nothing here.'
      echo "[pr-gate] $judge"; } >&2
    exit 2 ;;
  esac
fi
case "$cec" in
  0) ;;
  2) # Spec 093 3.1, on spec 093's rule: exit 2 is the ONE ambiguous code. This
     # tool spends it on staleness and clap spends it on an unrecognised
     # subcommand, so a binary predating `check` was refused here with a
     # staleness message and sent to regenerate shards that were already
     # correct. The probe runs only on this arm, so spec 093 3.2's happy path
     # is untouched: exit 0 still costs one process.
     # Contract 5: the verb AND the flag this gate passes. A binary whose
     # `check` lacks `--fail-on-unresolved` spent clap's 2 on the argument.
     if "$sc" check --help 2>/dev/null | grep -q -- '--fail-on-unresolved'; then
       { echo "[pr-gate] BLOCKED: a committed shard tree is stale in $root."
         echo '[pr-gate] Run: spec-spine compile and index, whichever tree it named, then commit the shards, push, and retry.'
    echo "[pr-gate] $judge"; } >&2
     else
       ver=$("$sc" --version 2>/dev/null) || ver='(no answer to --version)'
       { echo "[pr-gate] BLOCKED: the freshness read was not performed in $root (the binary does not carry check --fail-on-unresolved, and clap spent exit 2 on the unknown subcommand or argument)."
         echo "[pr-gate] The binary is $sc, which answers: ${ver:-(nothing)}. The check verb needs spec-spine 0.18.0 or later. The tree has NOT been judged and is not known to be stale; regenerating repairs nothing here. Run /setup to install the floor."
    echo "[pr-gate] $judge"; } >&2
     fi
     exit 2 ;;
  1) named=0
     case "$cout" in *'codebase-index: UNRESOLVED CLAIM'*|*'codebase-index: fresh, but REFUSED'*)
       { echo "[pr-gate] BLOCKED: the index in $root records an unresolved claim, which the gate refuses (spec-spine check --fail-on-unresolved exit 1)."
         echo '[pr-gate] Run: spec-spine index diagnostics, then fix the spec or write the unit it claims. Regenerating does not clear it: the diagnostic is recomputed from the corpus on every run.'; } >&2
       named=1 ;;
     esac
     case "$cout" in *'spec-registry: INVALID'*|*'spec-registry: REFUSED'*)
       { echo "[pr-gate] BLOCKED: the corpus in $root does not validate (spec-spine check exit 1)."
         echo '[pr-gate] Run: spec-spine check, fix the violations it names, and retry. Staleness is not meaningful against a corpus that does not compile.'; } >&2
       named=1 ;;
     esac
     case "$cout" in *'spec-registry: STALE'*|*'codebase-index: STALE'*)
       [ "$named" = 1 ] && echo '[pr-gate] The same report also names a stale shard tree: spec-spine compile and index clear that part only; commit the shards with the fix.' >&2 ;;
     esac
     if [ "$named" = 0 ]; then
       case "$cout" in *'spec-registry: STALE'*|*'codebase-index: STALE'*)
         # spec-spine 0.26.0's table: stale alone is exit 1.
         { echo "[pr-gate] BLOCKED: a committed shard tree is stale in $root."
           echo '[pr-gate] Run: spec-spine compile and index, whichever tree it named, then commit the shards, push, and retry.'; } >&2 ;;
       *)
         { echo "[pr-gate] BLOCKED: the corpus in $root does not validate (spec-spine check exit 1)."
           echo '[pr-gate] Run: spec-spine check, fix the violations it names, and retry. The tree is not stale; staleness is not meaningful against a corpus that does not compile.'; } >&2 ;;
       esac
     fi
     echo "[pr-gate] $judge" >&2
     exit 2 ;;
  3) # Spec 093 3.2: the version read qualifies a non-answer, so it is asked
     # only here, never on the happy path.
     ver=$("$sc" --version 2>/dev/null) || ver='(no answer to --version)'
     { echo "[pr-gate] BLOCKED: the freshness read was not performed in $root (spec-spine check exit 3)."
       echo "[pr-gate] The binary is $sc, which answers: ${ver:-(nothing)}. The check verb needs spec-spine 0.18.0 or later; below that the binary is too old to have read the tree, and the tree itself has not been judged. Run /setup to install the floor, or read spec-spine check directly for an I/O, parse or config error."
    echo "[pr-gate] $judge"; } >&2
     exit 2 ;;
  4) # spec-spine 0.26.0's table: the read failed.
     { echo "[pr-gate] BLOCKED: the freshness read failed in $root (spec-spine check exit 4: I/O, git or internal): $(printf '%s\n' "$cout" | head -1)"
       echo '[pr-gate] The tree has NOT been judged and is not known to be stale; regenerating repairs nothing here.'
       echo "[pr-gate] $judge"; } >&2
     exit 2 ;;
  *) { echo "[pr-gate] BLOCKED: spec-spine check exited $cec in $root, which this gate does not recognise; it is not reported as fresh."
    echo "[pr-gate] $judge"; } >&2
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
    echo "[pr-gate] The binary is $sc. The derived tree has not been judged and is not known to be clean; a gate whose check did not run is not green. Repair the configuration read (spec-spine config show --json must report layout.derived_dir) and retry. Regenerating shards repairs nothing here."
    echo "[pr-gate] $judge"; } >&2
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
    echo "[pr-gate] An empty answer from a command that failed is not a clean tree. The derived tree under $derived has not been judged; fix the repository read and retry."
    echo "[pr-gate] $judge"; } >&2
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
    echo '[pr-gate] Commit the regenerated shards with the change that made them stale, push, and retry.'
    echo "[pr-gate] $judge"; } >&2
  exit 2
fi

out=$("$sc" --repo "$root" couple --base "origin/$(default_branch "$root")" --head HEAD 2>&1); ec=$?
if [ $ec -ne 0 ]; then
  case "$cmd" in
    *--body*Spec-Drift-Waiver*) echo '[pr-gate] coupling gate failed; Spec-Drift-Waiver present after --body, allowing (CI honours the body-at-creation waiver).' ;;
    *) { echo "[pr-gate] BLOCKED: coupling gate failed in $root and no Spec-Drift-Waiver in the PR body:"
         echo "$out" | tail -25
         echo '[pr-gate] Either fix the coupling (claim every changed path in the spec being implemented, or add an extends edge naming the owning spec) or, with explicit human approval, include the Spec-Drift-Waiver line inline in --body (not --body-file) and retry. A waiver is a human instrument: never write one on your own authority.'
    echo "[pr-gate] $judge"; } >&2
       exit 2 ;;
  esac
fi
echo "[pr-gate] passed, $judge"
true
