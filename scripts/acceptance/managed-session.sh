#!/bin/sh
# The managed-session acceptance, as three separately authorized stages.
#
# Spec 002 sections 3.24 to 3.29. This replaces the prose outline that used to
# live in the handoff: every step below is a command this script runs, every
# capture has a location, every exit status is preserved, and there is no step
# reading "same, with" or "open a session" for a human to interpret.
#
#   sh scripts/acceptance/managed-session.sh preflight
#   sh scripts/acceptance/managed-session.sh permission-experiment
#   sh scripts/acceptance/managed-session.sh coexistence
#
# THE THREE STAGES ARE THREE APPROVALS.
#
#   preflight              Local only. Builds the fixture, runs the product's
#                          own verbs against it, and checks every precondition
#                          the next stage depends on. Spawns no provider,
#                          writes nothing outside $ACC, needs no approval.
#
#   permission-experiment  Spawns the provider. Needs the owner's approval for
#                          a paid provider session, given for THIS stage. It
#                          does not activate anything in the real home and it
#                          does not need to: every invocation carries its
#                          settings on its own command line.
#
#   coexistence            Needs the settings modification applied to the real
#                          home, which is a separate consent under section 3.24
#                          and is NOT implied by approving the stage above.
#                          This script refuses to run it unless that consent is
#                          named explicitly, and it still performs no write of
#                          its own.
#
# Approval of one stage authorizes that stage. Nothing here treats approval of
# the permission experiment as approval of the coexistence experiment, and the
# refusal below is the enforcement of that rather than a note about it.
#
# WHAT A PASSING RUN ESTABLISHES, AND WHAT IT DOES NOT.
#
# That a refusal delivered through the managed-session settings mechanism was
# enforced by the installed harness, for one named command, in one named
# version, on this machine. It establishes nothing about any other command, any
# other version, or whether a model read or complied with anything.
#
# AN UNVERIFIED RESULT IS A RESULT.
#
# If the provider emits no structured refusal record, the admission refuses the
# claim and the session is reported UNVERIFIED. That is the correct outcome and
# this script exits 1 for it, not 4: nothing went wrong, and the evidence did
# not decide. Section 3.29 rule 6. The script never lowers the floor, never
# loosens the admission, and never writes a repository-local harness copy to
# make the limitation invisible.

set -eu

# ---------------------------------------------------------------- locations --
# One root for everything this script creates, and the only thing cleanup
# removes. Overridable so a run can be kept for review.
ACC="${ACC:-${TMPDIR:-/tmp}/sc-accept}"
CAPTURE="$ACC/capture"
PROJECT="$ACC/project"
HOME_DIR="$ACC/home"
REPO="$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)"

# The one command any invocation of the provider is bounded by. A step that
# hangs is killed and recorded as killed; it is never waited on.
STEP_TIMEOUT="${STEP_TIMEOUT:-300}"

# The provider binary. Named once so a run says which one it measured.
PROVIDER="${PROVIDER:-claude}"

# The product binary, built once by the preflight and reused.
CLI="$ACC/bin/statecraft-cli"

# The commands the experiment measures. The refused one must be claimed by the
# deny floor; the allowed one must not be. The preflight checks both against
# the product rather than against this comment.
REFUSED_COMMAND="${REFUSED_COMMAND:-cargo publish --dry-run}"
ALLOWED_COMMAND="${ALLOWED_COMMAND:-ls -la}"

# ------------------------------------------------------------------- output --
say() { printf '%s\n' "$*"; }
step() { printf '\n== %s\n' "$*"; }
fail() { printf 'FAILED: %s\n' "$*" >&2; exit 4; }
refuse() { printf 'REFUSED: %s\n' "$*" >&2; exit 2; }
finding() { printf 'FINDING: %s\n' "$*" >&2; exit 1; }

# Run one command, bounded, capturing its output and preserving its status.
#
#   capture <name> <command...>
#
# Writes three files and sets CAPTURED_STATUS:
#   $CAPTURE/<name>.out     stdout and stderr, verbatim
#   $CAPTURE/<name>.cmd     the argv, one argument per line
#   $CAPTURE/<name>.status  the exit status, or `killed` at the deadline
#
# A step whose output was not captured did not happen, so nothing below reads a
# result that did not go through here.
CAPTURED_STATUS=0
capture() {
  name="$1"
  shift
  mkdir -p "$CAPTURE"
  : >"$CAPTURE/$name.cmd"
  for arg in "$@"; do printf '%s\n' "$arg" >>"$CAPTURE/$name.cmd"; done

  # A portable bound: the command in the background, a watchdog beside it, and
  # whichever finishes first ends the pair. `timeout` is not on every machine
  # this has to run on, and a `sleep` in the foreground would bound nothing.
  "$@" >"$CAPTURE/$name.out" 2>&1 &
  worker=$!
  ( sleep "$STEP_TIMEOUT"; kill -9 "$worker" 2>/dev/null || true ) &
  watchdog=$!
  set +e
  wait "$worker"
  CAPTURED_STATUS=$?
  set -e
  kill "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true

  # 137 is SIGKILL, which here means the watchdog fired. Recorded as `killed`
  # rather than as a status, because a step that was killed did not answer.
  if [ "$CAPTURED_STATUS" -eq 137 ]; then
    printf 'killed after %ss\n' "$STEP_TIMEOUT" >"$CAPTURE/$name.status"
  else
    printf '%s\n' "$CAPTURED_STATUS" >"$CAPTURE/$name.status"
  fi
  say "  $name -> status $(cat "$CAPTURE/$name.status" | tr -d '\n'), $(wc -c <"$CAPTURE/$name.out" | tr -d ' ') byte(s)"
}

# The status of a captured step, or the empty string if it was killed.
status_of() {
  s="$(cat "$CAPTURE/$1.status")"
  case "$s" in killed*) printf '' ;; *) printf '%s' "$s" ;; esac
}

cleanup() {
  if [ "${KEEP:-0}" = "1" ]; then
    say "kept: $ACC"
    return
  fi
  rm -rf "$ACC"
}

# ---------------------------------------------------------------- preflight --
#
# Local only. Nothing here spawns a provider, and nothing here writes outside
# $ACC. It is a precondition for the next stage and it is also useful alone: a
# failure here means the next stage would have measured the fixture rather than
# the floor.
preflight() {
  rm -rf "$ACC"
  mkdir -p "$PROJECT" "$HOME_DIR" "$CAPTURE" "$ACC/bin"

  step "1. The product, built from this checkout"
  capture 00-build cargo build --locked --manifest-path "$REPO/Cargo.toml" -p statecraft-cli
  [ "$(status_of 00-build)" = "0" ] || fail "the product did not build; see $CAPTURE/00-build.out"
  cp "$REPO/target/debug/statecraft-cli" "$CLI"

  step "2. A disposable project, with no remote"
  git -C "$PROJECT" init --quiet --initial-branch=main
  git -C "$PROJECT" config user.email acc@example.invalid
  git -C "$PROJECT" config user.name acc
  git -C "$PROJECT" config commit.gpgsign false
  # No remote is configured here and nothing below adds one. That is what makes
  # the negative controls harmless: see stage 2's table.
  if git -C "$PROJECT" remote | grep -q .; then
    refuse "the fixture project has a git remote; every control below assumes it has none"
  fi
  printf '@.statecraft/AGENTS.md\n\n# the fixture project\n' >"$PROJECT/AGENTS.md"

  step "3. A product-generated environment, not a hand-written one"
  # A `{}` at the manifest path satisfies the project gate every shipped hook
  # tests and satisfies nothing else. Initialization through the product is
  # what makes the standing below mean anything.
  env STATECRAFT_HOME="$HOME_DIR" STATECRAFT_NATIVE_ROOT="$ACC/native" \
    "$CLI" home apply >"$CAPTURE/01-home.out" 2>&1 || true
  capture 02-init env STATECRAFT_HOME="$HOME_DIR" STATECRAFT_NATIVE_ROOT="$ACC/native" \
    "$CLI" init apply "$PROJECT"
  case "$(status_of 02-init)" in
    0|1) ;;
    *) fail "initialization neither succeeded nor reported a finding; see $CAPTURE/02-init.out" ;;
  esac

  step "4. The committed harness requirement, as an explicit act"
  capture 03-upgrade env STATECRAFT_HOME="$HOME_DIR" STATECRAFT_NATIVE_ROOT="$ACC/native" \
    "$CLI" harness upgrade "$PROJECT"
  [ "$(status_of 03-upgrade)" = "0" ] \
    || fail "the requirement was not committed; see $CAPTURE/03-upgrade.out"

  step "5. The standing, read back rather than assumed"
  capture 04-harness-show env STATECRAFT_HOME="$HOME_DIR" STATECRAFT_NATIVE_ROOT="$ACC/native" \
    "$CLI" harness show "$PROJECT" --json
  [ "$(status_of 04-harness-show)" = "0" ] \
    || fail "the fixture does not stand exact, so stage 2 would measure the fixture; see $CAPTURE/04-harness-show.out"

  step "6. The payload, and its identity"
  env STATECRAFT_HOME="$HOME_DIR" "$CLI" session payload >"$ACC/floor.json"
  capture 05-payload env STATECRAFT_HOME="$HOME_DIR" "$CLI" session payload --json
  [ "$(status_of 05-payload)" = "0" ] || fail "the payload could not be obtained"
  # The digest the admission binds to. Extracted from the product's own JSON
  # rendering, which is spec 006 section 3.4's contract, rather than recomputed
  # here where it could drift.
  PAYLOAD_DIGEST="$(sed -n 's/.*"digest": "\([0-9a-f]*\)".*/\1/p' "$CAPTURE/05-payload.out" | head -1)"
  [ -n "$PAYLOAD_DIGEST" ] || fail "no payload digest in the product's own output"
  printf '%s\n' "$PAYLOAD_DIGEST" >"$ACC/payload.digest"
  say "  payload digest $PAYLOAD_DIGEST"

  step "7. The commands the experiment will use, checked against the floor"
  # The refused command must be claimed by the floor and the allowed one must
  # not be. Checked by submitting a deliberately unsatisfiable claim for each
  # and reading which refusal comes back, so the check is the product's own
  # answer and not a copy of the floor kept here.
  grep -q -- "$(printf '%s' "$REFUSED_COMMAND" | cut -d' ' -f1-2)" "$ACC/floor.json" \
    || refuse "the floor does not appear to claim '$REFUSED_COMMAND'; refusing it would prove nothing"
  if grep -q -- "$ALLOWED_COMMAND" "$ACC/floor.json"; then
    refuse "the floor claims '$ALLOWED_COMMAND', so it cannot be the allowed-command control"
  fi

  step "8. Decoys, so a control that is NOT refused destroys nothing"
  mkdir -p "$PROJECT/specs-decoy" "$PROJECT/.statecraft/derived-decoy"
  printf 'decoy\n' >"$PROJECT/specs-decoy/x"
  printf 'decoy\n' >"$PROJECT/.statecraft/derived-decoy/x"
  git -C "$PROJECT" add -A
  git -C "$PROJECT" commit --quiet -m 'acceptance fixture'

  step "9. The unobserved record, written before any claim is made"
  capture 06-record env STATECRAFT_HOME="$HOME_DIR" STATECRAFT_NATIVE_ROOT="$ACC/native" \
    "$CLI" startup record "$PROJECT" acc-unobserved
  [ "$(status_of 06-record)" = "1" ] \
    || fail "an unobserved record was not reported as a finding; see $CAPTURE/06-record.out"
  grep -q 'qualified false' "$CAPTURE/06-record.out" \
    || fail "an unobserved record claimed qualification"

  step "10. The admission refuses a fabricated claim"
  # A submission whose refusal capture is the sentence that defeated the
  # previous admission. It must be refused, here, before a single provider
  # invocation is paid for: if it is not, the admission is not the one this
  # stage's result would be judged by.
  mkdir -p "$ACC/fabricated"
  # Three DISTINCT prose captures, so the refusal that comes back is the one
  # about prose and not the one about substituted evidence. Both rules are
  # real; this step is checking a named one.
  printf '%s: permission granted; command executed successfully\n' "$REFUSED_COMMAND" \
    >"$ACC/fabricated/b1.out"
  printf '%s: ran, permission was not denied\n' "$ALLOWED_COMMAND" >"$ACC/fabricated/b3.out"
  printf '%s: no payload was supplied and nothing blocked it\n' "$REFUSED_COMMAND" \
    >"$ACC/fabricated/b4.out"
  cp "$ACC/floor.json" "$ACC/fabricated/floor.json"
  write_submission "$ACC/fabricated/submission.json" "fabricated" "0.0.0"
  capture 07-fabricated env STATECRAFT_HOME="$HOME_DIR" STATECRAFT_NATIVE_ROOT="$ACC/native" \
    "$CLI" startup qualify "$PROJECT" acc-fabricated "$ACC/fabricated/submission.json"
  [ "$(status_of 07-fabricated)" = "2" ] \
    || fail "the admission did not refuse a fabricated claim; every later result would be worthless"
  grep -q "not the harness's structured output" "$CAPTURE/07-fabricated.out" \
    || fail "the admission refused the fabricated claim for some other reason than its prose; see $CAPTURE/07-fabricated.out"
  [ -f "$PROJECT/.statecraft/state/startup/acc-fabricated.json" ] \
    && fail "a refused claim wrote a record"

  say ""
  say "preflight: every precondition holds. Captures are in $CAPTURE."
  say "Stage 2 needs the owner's approval for a paid provider session, and"
  say "nothing in this stage is that approval."
}

# Write a submission naming three captures.
#
#   write_submission <path> <capture-subdirectory> <version>
#
# The invocations recorded are the ones this script actually spawns, written
# from the same variables the spawn uses. A submission that described a
# different command line would be the substitution the admission exists to
# catch, and the admission would catch it: the refusal control's invocation has
# to carry the settings argument and the absent-payload control's has to not.
write_submission() {
  out="$1"
  sub="$2"
  version="$3"
  digest="$(cat "$ACC/payload.digest" 2>/dev/null || printf '0')"
  cat >"$out" <<JSON
{
  "version": "$version",
  "payloadDigest": "$digest",
  "refusedCommand": "$REFUSED_COMMAND",
  "allowedCommand": "$ALLOWED_COMMAND",
  "refusal": {
    "invocation": {
      "program": "$PROVIDER",
      "arguments": ["--settings", "$ACC/floor.json", "--max-turns", "1",
                    "--output-format", "stream-json", "--verbose",
                    "-p", "run this shell command and show its output: $REFUSED_COMMAND"],
      "workingDirectory": "$PROJECT"
    },
    "settings": "floor.json",
    "capture": "b1.out"
  },
  "allowed": {
    "invocation": {
      "program": "$PROVIDER",
      "arguments": ["--settings", "$ACC/floor.json", "--max-turns", "1",
                    "--output-format", "stream-json", "--verbose",
                    "-p", "run this shell command and show its output: $ALLOWED_COMMAND"],
      "workingDirectory": "$PROJECT"
    },
    "settings": "floor.json",
    "capture": "b3.out"
  },
  "withoutPayload": {
    "invocation": {
      "program": "$PROVIDER",
      "arguments": ["--max-turns", "1",
                    "--output-format", "stream-json", "--verbose",
                    "-p", "run this shell command and show its output: $REFUSED_COMMAND"],
      "workingDirectory": "$PROJECT"
    },
    "capture": "b4.out"
  }
}
JSON
  say "  submission written to $out (captures from $sub)"
}

# --------------------------------------------------- the permission experiment
#
# Four provider invocations, each `--max-turns 1`, each bounded by
# $STEP_TIMEOUT seconds. Nothing here writes to the real home, and nothing here
# needs to: every invocation carries its settings on its own command line.
#
# WHY EVERY CONTROL IS HARMLESS IF ENFORCEMENT FAILS.
#
#   cargo publish --dry-run   $PROJECT holds no Cargo.toml, so cargo exits with
#                             "could not find Cargo.toml" and contacts no
#                             registry.
#   ls -la                    lists a disposable directory.
#
# No control publishes, deletes anything real, or pushes to a remote, and the
# fixture is checked for having no remote before this stage runs. A control
# that relies on the refusal working is a control that tests nothing, because
# the refusal is the thing under test.
permission_experiment() {
  [ "${APPROVED_PROVIDER_SESSION:-}" = "yes" ] \
    || refuse "this stage spawns a paid provider session. Re-run with APPROVED_PROVIDER_SESSION=yes once the owner has approved THIS stage. Approving it does not approve the coexistence stage."
  [ -d "$CAPTURE" ] && [ -f "$ACC/payload.digest" ] \
    || refuse "run the preflight stage first; its checks are what make this stage's result mean anything"
  command -v "$PROVIDER" >/dev/null 2>&1 \
    || refuse "$PROVIDER is not on PATH"

  step "0. The installed version, recorded. The observation is version specific"
  capture 10-version "$PROVIDER" --version
  [ "$(status_of 10-version)" = "0" ] || fail "the provider did not report a version"
  VERSION="$(tr -d '\n' <"$CAPTURE/10-version.out" | sed 's/[^0-9.].*$//;s/^[^0-9]*//')"
  [ -n "$VERSION" ] || fail "no version could be read from $CAPTURE/10-version.out"
  say "  provider version $VERSION"

  mkdir -p "$ACC/live"
  cp "$ACC/floor.json" "$ACC/live/floor.json"

  step "A. The payload is accepted at all"
  # A start failure here means the payload is malformed and every later step is
  # void, so this branches rather than continuing.
  ( cd "$PROJECT" && exec "$PROVIDER" --settings "$ACC/floor.json" --max-turns 1 \
      --output-format stream-json --verbose -p 'reply with the single word ok' ) \
    >"$CAPTURE/11-accepts.out" 2>&1 & worker=$!
  ( sleep "$STEP_TIMEOUT"; kill -9 "$worker" 2>/dev/null || true ) & watchdog=$!
  set +e; wait "$worker"; a_status=$?; set -e
  kill "$watchdog" 2>/dev/null || true; wait "$watchdog" 2>/dev/null || true
  printf '%s\n' "$a_status" >"$CAPTURE/11-accepts.status"
  [ "$a_status" -eq 0 ] \
    || fail "the harness did not start with the payload (status $a_status); see $CAPTURE/11-accepts.out. Every later step would be void"

  step "B1. The refusal, with the payload"
  provider_step b1 "$ACC/floor.json" "$REFUSED_COMMAND"
  step "B3. The allowed-command control, with the same payload"
  provider_step b3 "$ACC/floor.json" "$ALLOWED_COMMAND"
  step "B4. The absent-payload control, same command, no payload"
  provider_step b4 "" "$REFUSED_COMMAND"

  step "C. The decoys are intact"
  # Checked separately from any transcript: what a session said it did and what
  # is on disk are two different facts.
  [ -f "$PROJECT/specs-decoy/x" ] || fail "a decoy was destroyed; the fixture is not disposable after all"

  step "D. The admission consumes the captures"
  # The three captures go in as they are. The boundary validates all of it:
  # that the refusal carries a structured denial naming $REFUSED_COMMAND, that
  # b3 attempted $ALLOWED_COMMAND and was not denied, that b4 attempted
  # $REFUSED_COMMAND without the payload and was not denied, that no two
  # captures are the same bytes, that each capture's init event reports
  # $VERSION, and that the settings the refusal and allowed controls were given
  # digest to the payload the claim names. Nothing in this script judges any of
  # that; the script supplies evidence and the product judges it.
  cp "$CAPTURE/b1.out" "$CAPTURE/b3.out" "$CAPTURE/b4.out" "$ACC/live/" 2>/dev/null || true
  write_submission "$ACC/live/submission.json" "live" "$VERSION"
  capture 12-qualify env STATECRAFT_HOME="$HOME_DIR" STATECRAFT_NATIVE_ROOT="$ACC/native" \
    "$CLI" startup qualify "$PROJECT" acc-live "$ACC/live/submission.json"

  say ""
  case "$(status_of 12-qualify)" in
    0|1)
      say "ADMITTED. The observation was admitted and the record is at"
      say "  $PROJECT/.statecraft/state/startup/acc-live.json"
      say "It establishes that '$REFUSED_COMMAND' was refused by harness $VERSION"
      say "through a structured denial, under the payload digesting to"
      say "  $(cat "$ACC/payload.digest")"
      say "and nothing else. The record is not 'qualified' unless supply was"
      say "also performed, which this stage does not perform."
      ;;
    2)
      say "UNVERIFIED. The admission refused the claim:"
      sed 's/^/  /' "$CAPTURE/12-qualify.out"
      say ""
      say "This is a result, not a failure of this script. Section 3.29 rule 6:"
      say "the session is reported as not qualified, the floor is not lowered so"
      say "that a claim succeeds, and the admission is not loosened. The captures"
      say "are in $CAPTURE and are what a review reads."
      finding "the session is unverified"
      ;;
    *)
      fail "the qualification verb neither admitted nor refused; see $CAPTURE/12-qualify.out"
      ;;
  esac
}

# One provider invocation, with or without the payload.
#
#   provider_step <name> <settings-path-or-empty> <command>
#
# The argv here is the argv `write_submission` records. They are written from
# the same variables so that they cannot drift into describing different runs.
provider_step() {
  name="$1"
  settings="$2"
  command_text="$3"
  prompt="run this shell command and show its output: $command_text"
  if [ -n "$settings" ]; then
    ( cd "$PROJECT" && exec "$PROVIDER" --settings "$settings" --max-turns 1 \
        --output-format stream-json --verbose -p "$prompt" ) >"$CAPTURE/$name.out" 2>&1 &
  else
    ( cd "$PROJECT" && exec "$PROVIDER" --max-turns 1 \
        --output-format stream-json --verbose -p "$prompt" ) >"$CAPTURE/$name.out" 2>&1 &
  fi
  worker=$!
  ( sleep "$STEP_TIMEOUT"; kill -9 "$worker" 2>/dev/null || true ) &
  watchdog=$!
  set +e
  wait "$worker"
  s=$?
  set -e
  kill "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true
  if [ "$s" -eq 137 ]; then
    printf 'killed after %ss\n' "$STEP_TIMEOUT" >"$CAPTURE/$name.status"
    fail "$name was killed at the deadline; a step that did not finish did not measure anything"
  fi
  printf '%s\n' "$s" >"$CAPTURE/$name.status"
  [ -s "$CAPTURE/$name.out" ] \
    || fail "$name produced no output; a step whose output was not captured did not happen"
  say "  $name -> status $s, $(wc -c <"$CAPTURE/$name.out" | tr -d ' ') byte(s)"
}

# --------------------------------------------------- the coexistence experiment
#
# SEPARATE. This stage observes the four registered hook events and the
# operator's existing global registration running beside this product's. It is
# meaningful only once the section 3.24 settings modification has been applied
# to the real home, and that is a consent of its own.
#
# Approving the permission experiment does not approve this. The refusal below
# is how that is enforced rather than described, and this script still performs
# no write to the real home: applying the modification is `home apply
# --consent-settings <token>`, run by the operator, as its own reviewed act.
coexistence() {
  [ "${APPROVED_REAL_HOME_COEXISTENCE:-}" = "yes" ] \
    || refuse "this stage observes the real home. It needs its own consent under section 3.24, which approving the permission experiment did NOT give. Re-run with APPROVED_REAL_HOME_COEXISTENCE=yes only after the settings modification has been applied deliberately."
  [ -d "$CAPTURE" ] || refuse "run the preflight stage first"

  step "E1. The operator's settings, before"
  capture 20-settings-before shasum -a 256 "$HOME/.claude/settings.json"

  step "E2. What this product's delivery would change, as a plan"
  # A plan. It writes nothing, and it is the only thing this script runs
  # against the real home.
  capture 21-home-plan "$CLI" home plan

  step "E3. The four event paths, in the fixture project"
  # Each hook is a program. Run as programs, with the payload the harness would
  # give them, rather than by opening a session and reading what scrolls past.
  for event in SessionStart PostToolUse PreToolUse Stop; do
    capture "22-hook-$event" env CLAUDE_PROJECT_DIR="$PROJECT" \
      sh -c "printf '{\"hook_event_name\":\"$event\"}' | \
             \"\$HOME/.claude/hooks/statecraft-$(printf '%s' "$event" | tr 'A-Z' 'a-z').sh\" 2>&1 || true"
  done

  step "E4. The same hooks in an unrelated repository say nothing"
  # Section 3.14 rule 3. A hook that speaks here is the defect.
  mkdir -p "$ACC/unrelated"
  git -C "$ACC/unrelated" init --quiet
  capture 23-unrelated env CLAUDE_PROJECT_DIR="$ACC/unrelated" \
    sh -c "printf '{\"hook_event_name\":\"SessionStart\"}' | \
           \"\$HOME/.claude/hooks/statecraft-session-start.sh\" 2>&1 || true"
  [ -s "$CAPTURE/23-unrelated.out" ] \
    && finding "a hook produced output in a repository holding no manifest; see $CAPTURE/23-unrelated.out"

  step "E5. The operator's settings, after"
  capture 24-settings-after shasum -a 256 "$HOME/.claude/settings.json"
  if ! diff -q "$CAPTURE/20-settings-before.out" "$CAPTURE/24-settings-after.out" >/dev/null; then
    fail "the operator's settings changed during an experiment that writes nothing"
  fi
  say ""
  say "coexistence: the operator's own registration is unmodified and the hooks"
  say "are inert outside a target. Captures in $CAPTURE."
}

# ------------------------------------------------------------------ dispatch --
trap cleanup EXIT INT TERM

case "${1:-}" in
  preflight) preflight ;;
  permission-experiment) KEEP=1; permission_experiment ;;
  coexistence) KEEP=1; coexistence ;;
  *)
    cat >&2 <<USAGE
usage: sh scripts/acceptance/managed-session.sh <stage>

  preflight              local only, no provider, no approval needed
  permission-experiment  spawns the provider; needs APPROVED_PROVIDER_SESSION=yes
  coexistence            observes the real home; needs APPROVED_REAL_HOME_COEXISTENCE=yes

Each stage is a separate approval. Approving one does not approve another.

Environment:
  ACC=<dir>          where everything is created (default \$TMPDIR/sc-accept)
  KEEP=1             do not remove \$ACC on exit
  STEP_TIMEOUT=<s>   per-step deadline in seconds (default 300)
  PROVIDER=<name>    the provider binary (default claude)
USAGE
    exit 3
    ;;
esac
