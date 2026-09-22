#!/bin/sh
# The managed-session acceptance, as separately authorized stages.
#
# Spec 002 sections 3.24 to 3.30. Every step below is a command this script
# runs, every capture has a location, every exit status is preserved with its
# meaning, and the product, not this script, constructs every provider
# invocation and judges every capture.
#
#   sh scripts/acceptance/managed-session.sh preflight
#   sh scripts/acceptance/managed-session.sh permission-experiment
#   sh scripts/acceptance/managed-session.sh coexistence
#   sh scripts/acceptance/managed-session.sh clean
#
# THE STAGES ARE SEPARATE APPROVALS.
#
#   preflight              Local only. Builds the fixture, runs the product's
#                          own verbs against it, checks, through the same
#                          launch and admission path stage 2 uses, that the
#                          admission refuses a prose claim, and drives `run`
#                          and `startup show` end to end against a local fake
#                          (spec 002 section 3.31), which is SYNTHETIC. Spawns
#                          no provider, writes nothing outside $ACC, needs no
#                          approval.
#
#   permission-experiment  Spawns the provider: at most THREE sessions, one per
#                          control, in order, each bounded by $STEP_TIMEOUT
#                          seconds, stopping at the first launch that does not
#                          complete. Each launch also runs the provider's
#                          `--version` once (a probe, not a session, bounded at
#                          30 seconds). No retry, no replay, no extra session.
#                          Refuses unless APPROVED_PROVIDER_SESSION=yes.
#                          Activates nothing in the real home: every launch
#                          carries its settings on its own command line.
#
#   coexistence            Observes the real home. Needs the section 3.24
#                          settings modification applied there, which is its
#                          own consent and is NOT implied by approving the
#                          stage above. Refuses unless
#                          APPROVED_REAL_HOME_COEXISTENCE=yes, and performs no
#                          write to the real home.
#
# THE LOCAL TEST ROUTE.
#
#   SC_ACCEPTANCE_FAKE_PROVIDER=<executable> runs the permission-experiment
#   stage's whole control flow against a local fake instead of the provider.
#   It never reads the provider approval, refuses to run if that approval is
#   also set, marks every capture synthetic (spec 002 section 3.30), and
#   reports its result as SYNTHETIC. A synthetic result is never a live
#   observation and nothing downstream treats it as one.
#
# WHAT AN ADMITTED LIVE RESULT ESTABLISHES, AND WHAT IT DOES NOT.
#
# That, in one named harness version on this machine, the refused command was
# refused through a structured denial and did not execute under the payload,
# the allowed command executed under the same payload and printed its expected
# output, and the refused command executed without the payload. The launch
# record is launcher-attested, not provider-authenticated. Nothing is
# established about any other command or version, or about whether a model
# read or complied with anything. The record is not "qualified": that also
# needs supply, which this stage does not perform.
#
# AN UNVERIFIED RESULT IS A RESULT, and this script exits 1 for it.
#
# Exit codes: 0 the stage did what it is for; 1 a finding (unverified, or an
# incomplete launch); 2 refused (an approval or a precondition is missing);
# 3 usage; 4 something failed that nobody asked for.

set -eu

# ---------------------------------------------------------------- locations --
# One root for everything this script creates. It is kept between stages,
# because stage 2 needs what stage 1 built; `clean` removes it, and only when
# it carries this script's marker.
ACC="${ACC:-${TMPDIR:-/tmp}/sc-accept}"
MARKER="$ACC/.sc-acceptance"
CAPTURE="$ACC/steps"
PROJECT="$ACC/project"
HOME_DIR="$ACC/home"
NATIVE_DIR="$ACC/native"
REPO="$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)"

# The per-session deadline, in seconds, handed to the product's own supervisor.
STEP_TIMEOUT="${STEP_TIMEOUT:-300}"
# The bound on each local step (the build, and each product verb).
LOCAL_TIMEOUT="${LOCAL_TIMEOUT:-900}"
# The provider, as named on PATH or by path.
PROVIDER="${PROVIDER:-claude}"
# The product binary. Built by the preflight unless one is named.
CLI="${CLI:-$ACC/bin/statecraft-cli}"

# ------------------------------------------------------------------- output --
say() { printf '%s\n' "$*"; }
step() { printf '\n== %s\n' "$*"; }
fail() { printf 'FAILED: %s\n' "$*" >&2; exit 4; }
refuse() { printf 'REFUSED: %s\n' "$*" >&2; exit 2; }
finding() { printf 'FINDING: %s\n' "$*" >&2; exit 1; }

# The product's verbs, against the fixture's home. Stage 2 does not override
# HOME: the provider authenticates as the operator.
product() {
  env STATECRAFT_HOME="$HOME_DIR" STATECRAFT_NATIVE_ROOT="$NATIVE_DIR" "$CLI" "$@"
}

# Every process a pid has started, deepest first, then the pid itself.
tree() {
  for child in $(pgrep -P "$1" 2>/dev/null || true); do
    tree "$child"
  done
  printf '%s\n' "$1"
}

# Kill a process and everything it started. The tree is read before anything
# is signalled, so a child cannot be orphaned out of it by its parent dying.
kill_tree() {
  pids="$(tree "$1")"
  for pid in $pids; do kill -s KILL "$pid" 2>/dev/null || true; done
}

# Run one local step, bounded, keeping its streams apart and its status whole.
#
#   bounded <name> <seconds> <command...>
#
# Writes, under $CAPTURE:
#   <name>.cmd     the argv, one argument per line
#   <name>.out     standard output
#   <name>.err     standard error
#   <name>.status  `exit N`, `signal N` or `timeout after Ns`
# and sets RAN to the exit code, or to the empty string when the step did not
# exit by itself. A step whose output was not captured did not happen, so
# nothing below reads a result that did not go through here.
RAN=""
bounded() {
  name="$1"
  limit="$2"
  shift 2
  mkdir -p "$CAPTURE"
  : >"$CAPTURE/$name.cmd"
  for arg in "$@"; do printf '%s\n' "$arg" >>"$CAPTURE/$name.cmd"; done
  rm -f "$CAPTURE/$name.fired"
  "$@" >"$CAPTURE/$name.out" 2>"$CAPTURE/$name.err" &
  worker=$!
  (
    sleep "$limit"
    : >"$CAPTURE/$name.fired"
    kill_tree "$worker"
  ) &
  watchdog=$!
  set +e
  wait "$worker"
  code=$?
  set -e
  kill_tree "$watchdog"
  wait "$watchdog" 2>/dev/null || true
  if [ -f "$CAPTURE/$name.fired" ]; then
    RAN=""
    printf 'timeout after %ss\n' "$limit" >"$CAPTURE/$name.status"
  elif [ "$code" -gt 128 ]; then
    RAN=""
    printf 'signal %s\n' "$((code - 128))" >"$CAPTURE/$name.status"
  else
    RAN="$code"
    printf 'exit %s\n' "$code" >"$CAPTURE/$name.status"
  fi
  say "  $name -> $(cat "$CAPTURE/$name.status"), stdout $(wc -c <"$CAPTURE/$name.out" | tr -d ' ') byte(s), stderr $(wc -c <"$CAPTURE/$name.err" | tr -d ' ') byte(s)"
}

# Whether a step's output holds a line. For the product's own JSON and human
# renderings, whose keys and phrases are fixed by this build.
holds() {
  grep -F -q -- "$2" "$CAPTURE/$1.out"
}

need_preflight() {
  [ -f "$MARKER" ] && [ -f "$ACC/preflight.ok" ] \
    || refuse "run the preflight stage first, with the same ACC; its checks are what make this stage's result mean anything"
  [ -x "$CLI" ] || refuse "the product binary $CLI is not there; run the preflight again"
}

# ---------------------------------------------------------------- preflight --
preflight() {
  if [ -e "$ACC" ] && [ ! -f "$MARKER" ]; then
    refuse "$ACC exists and was not created by this script; it is not removed. Choose another ACC"
  fi
  rm -rf "$ACC"
  mkdir -p "$ACC" "$PROJECT" "$HOME_DIR" "$CAPTURE"
  : >"$MARKER"

  step "1. The product"
  if [ "$CLI" = "$ACC/bin/statecraft-cli" ]; then
    bounded 00-build "$LOCAL_TIMEOUT" cargo build --locked --manifest-path "$REPO/Cargo.toml" -p statecraft-cli
    [ "$RAN" = 0 ] || fail "the product did not build; see $CAPTURE/00-build.err"
    mkdir -p "$ACC/bin"
    cp "$REPO/target/debug/statecraft-cli" "$CLI" || fail "the built binary could not be copied to $CLI"
  fi
  [ -x "$CLI" ] || fail "$CLI is not an executable"
  say "  product $CLI"

  step "2. A disposable project, with no remote"
  git -C "$PROJECT" init --quiet --initial-branch=main
  git -C "$PROJECT" config user.email acc@example.invalid
  git -C "$PROJECT" config user.name acc
  git -C "$PROJECT" config commit.gpgsign false
  if [ -n "$(git -C "$PROJECT" remote)" ]; then
    refuse "the fixture project has a git remote; nothing here may be able to push"
  fi
  printf '@.statecraft/AGENTS.md\n\n# the fixture project\n' >"$PROJECT/AGENTS.md"

  step "3. A product-generated environment"
  # 0 is complete. 1 is a finding the product documents: the settings
  # modification is withheld without consent, and initialization against the
  # published producer is partial. Anything else stops here.
  bounded 01-home "$LOCAL_TIMEOUT" product home apply
  case "$RAN" in 0|1) ;; *) fail "home apply: $(cat "$CAPTURE/01-home.status"); see $CAPTURE/01-home.err" ;; esac
  bounded 02-init "$LOCAL_TIMEOUT" product init apply "$PROJECT"
  case "$RAN" in 0|1) ;; *) fail "init apply: $(cat "$CAPTURE/02-init.status"); see $CAPTURE/02-init.out" ;; esac

  step "4. The committed harness requirement, as an explicit act"
  bounded 03-upgrade "$LOCAL_TIMEOUT" product harness upgrade "$PROJECT"
  [ "$RAN" = 0 ] || fail "the requirement was not committed; see $CAPTURE/03-upgrade.out"
  git -C "$PROJECT" add -A
  git -C "$PROJECT" commit --quiet -m 'acceptance fixture'

  step "5. The standing, read back rather than assumed"
  bounded 04-harness-show "$LOCAL_TIMEOUT" product harness show "$PROJECT" --json
  [ "$RAN" = 0 ] || fail "the fixture does not stand exact, so stage 2 would measure the fixture; see $CAPTURE/04-harness-show.out"

  step "6. The payload, recorded for review"
  bounded 05-payload "$LOCAL_TIMEOUT" product session payload --json
  [ "$RAN" = 0 ] || fail "the payload could not be obtained"

  step "7. The refused command fails harmlessly if enforcement does not hold"
  # The product refuses to launch when this path exists; checked here too so
  # the preflight says so before anything else.
  [ ! -e "$PROJECT/statecraft-absent" ] || refuse "$PROJECT/statecraft-absent exists"
  # `--manifest-path` names the manifest, so cargo searches no ancestor for
  # one. What an ancestor CAN still change is which toolchain a rustup proxy
  # selects, and an uninstalled one may be downloaded before cargo runs. So no
  # ancestor of the project may carry a toolchain file, and stage 2 exports
  # RUSTUP_AUTO_INSTALL=0 besides. Cargo configuration in an ancestor is
  # listed for review; `--dry-run` uploads nothing whatever it says.
  dir="$PROJECT"
  while :; do
    for f in rust-toolchain rust-toolchain.toml; do
      [ ! -e "$dir/$f" ] || refuse "$dir/$f would select the toolchain the refused command runs under; choose an ACC outside it"
    done
    for f in .cargo/config .cargo/config.toml; do
      [ ! -e "$dir/$f" ] || say "  note: $dir/$f is cargo configuration an unenforced refusal would read"
    done
    [ "$dir" = / ] && break
    dir="$(dirname -- "$dir")"
  done

  step "8. The unobserved record"
  bounded 06-record "$LOCAL_TIMEOUT" product startup record "$PROJECT" acc-unobserved
  [ "$RAN" = 1 ] || fail "an unobserved record was not reported as a finding; see $CAPTURE/06-record.out"
  holds 06-record 'qualified false' || fail "an unobserved record claimed qualification"

  step "9. The admission refuses a prose claim, through the launch stage 2 uses"
  # A local executable that prints section 3.29's sentence instead of a
  # session. It is launched by the product, marked synthetic, and the claim is
  # submitted. It must be refused as not the harness's structured output,
  # here, before a single provider session is paid for.
  cat >"$ACC/prose-provider.sh" <<'PROSE'
#!/bin/sh
if [ "${1:-}" = "--version" ]; then printf '0.0.0 (prose)\n'; exit 0; fi
cat >/dev/null
printf 'cargo publish --dry-run: permission granted; command executed successfully (%s)\n' "$$"
exit 0
PROSE
  chmod 700 "$ACC/prose-provider.sh"
  for control in refusal allowed-command without-payload; do
    bounded "07-prose-$control" "$LOCAL_TIMEOUT" product startup capture "$PROJECT" "$control" \
      "$ACC/prose" --program "$ACC/prose-provider.sh" --deadline 30 --synthetic
    # 1: the launch recorded output that is not one session, which is the point.
    [ "$RAN" = 1 ] || fail "the prose launch of $control: $(cat "$CAPTURE/07-prose-$control.status"); see $CAPTURE/07-prose-$control.out"
  done
  bounded 08-prose-qualify "$LOCAL_TIMEOUT" product startup qualify "$PROJECT" acc-prose "$ACC/prose"
  [ "$RAN" = 2 ] || fail "the admission did not refuse a prose claim; every later result would be worthless"
  holds 08-prose-qualify "not the harness's structured output" \
    || fail "the prose claim was refused for another reason; see $CAPTURE/08-prose-qualify.out"
  [ ! -e "$PROJECT/.statecraft/state/startup/acc-prose.json" ] || fail "a refused claim wrote a record"

  step "10. The run path, against a local fake (SYNTHETIC)"
  # Spec 002 sections 3.31 and 3.32, end to end through the product: `run`
  # writes its startup intent, supplies the required revision's SessionStart
  # hook and the attempt's admission gate in the settings it passes, confirms
  # the spawn, decides at the init event, and `startup show` reads the
  # judgement back. The fake stands in for a provider that honors hooks given
  # through `--settings`: it reads the commands from the document it was given
  # and runs them. Nothing in the operator's home registers anything, and
  # nothing here is live.
  runbin="$ACC/runbin"
  mkdir -p "$runbin"
  cp "$REPO/crates/statecraft-adapter-claude-code/testdata/stream/success.jsonl" "$runbin/session.jsonl"
  required="$(sed -n 's/^ *"required": "\([0-9a-f]\{64\}\)",*$/\1/p' "$CAPTURE/04-harness-show.out" | head -n 1)"
  display="$(sed -n 's/^ *"requiredDisplay": "\(h-[0-9a-f]*\)",*$/\1/p' "$CAPTURE/04-harness-show.out" | head -n 1)"
  [ -n "$required" ] && [ -n "$display" ] || fail "no required revision in $CAPTURE/04-harness-show.out"
  cat >"$runbin/spec-spine" <<'SPINE'
#!/bin/sh
# A synthetic scheduler input for the fixture: one ready unit of work.
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"acc-run","title":"synthetic run"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"acc-run","status":"approved","implementation":"pending"}]}' ;;
  *) exit 3 ;;
esac
SPINE
  cat >"$runbin/claude" <<'FAKE'
#!/bin/sh
# A LOCAL FAKE PROVIDER for the run path. Never a provider, never live evidence.
if [ "${1:-}" = --version ]; then printf '2.1.267 (Claude Code)\n'; exit 0; fi
here="$(dirname -- "$0")"
settings=""
while [ "$#" -gt 0 ]; do
  case "$1" in --settings) settings="$2"; shift ;; esac
  shift
done
cp "$settings" "$here/received-settings"
cat >/dev/null
# The first command registered for an event, read from the document given.
command_for() {
  awk -v ev="\"$1\": [" 'index($0, ev) { inside = 1 } inside && /"command": / { sub(/^[^"]*"command": "/, ""); sub(/",?[ \t]*$/, ""); gsub(/\\\\/, "\001"); gsub(/\\"/, "\""); gsub(/\001/, "\\"); print; exit }' "$settings"
}
start_cmd="$(command_for SessionStart)"
gate_cmd="$(command_for PreToolUse)"
[ -n "$start_cmd" ] && [ -n "$gate_cmd" ] || { echo "the settings register no startup hook or no gate" >&2; exit 3; }
session=11111111-1111-1111-1111-111111111111
out="$(CLAUDE_PROJECT_DIR="$PWD" sh -c "$start_cmd" 2>/dev/null)"
code=$?
esc="$(printf '%s' "$out" | awk 'BEGIN { ORS = "" } { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); if (NR > 1) printf "\\n"; print }')"
printf '{"type":"system","subtype":"hook_started","hook_name":"SessionStart:startup","hook_event":"SessionStart","session_id":"%s"}\n' "$session"
printf '{"type":"system","subtype":"hook_response","hook_name":"SessionStart:startup","hook_event":"SessionStart","stdout":"%s","stderr":"","exit_code":%s,"outcome":"success","session_id":"%s"}\n' "$esc" "$code" "$session"
sed -n '1,3p' "$here/session.jsonl"
# One harmless tool call, through the gate the run registered.
if printf '{"tool_name":"Bash"}' | sh -c "$gate_cmd" 2>/dev/null; then : >"$PWD/acc-sentinel"; fi
sed -n '4,$p' "$here/session.jsonl"
FAKE
  chmod 700 "$runbin/spec-spine" "$runbin/claude"
  runpath="$runbin:/usr/bin:/bin"
  for verb in register arm; do
    bounded "09-$verb" "$LOCAL_TIMEOUT" env PATH="$runpath" STATECRAFT_HOME="$HOME_DIR" \
      STATECRAFT_NATIVE_ROOT="$NATIVE_DIR" "$CLI" project "$verb" "$PROJECT"
    case "$RAN" in 0|1) ;; *) fail "project $verb: $(cat "$CAPTURE/09-$verb.status")" ;; esac
  done
  bounded 09-run "$LOCAL_TIMEOUT" env PATH="$runpath" STATECRAFT_HOME="$HOME_DIR" \
    STATECRAFT_NATIVE_ROOT="$NATIVE_DIR" "$CLI" run "$PROJECT" acc-run --json
  [ "$RAN" = 0 ] || fail "the synthetic run did not complete: $(cat "$CAPTURE/09-run.status"); see $CAPTURE/09-run.out"
  holds 09-run '"verdict": "unverified"' || fail "the run's startup verdict is not unverified; see $CAPTURE/09-run.out"
  bounded 09-startup-show "$LOCAL_TIMEOUT" env PATH="$runpath" STATECRAFT_HOME="$HOME_DIR" \
    STATECRAFT_NATIVE_ROOT="$NATIVE_DIR" "$CLI" startup show "$PROJECT" acc-run --json
  # 1: unverified is a finding. A run never reaches qualified.
  [ "$RAN" = 1 ] || fail "startup show: $(cat "$CAPTURE/09-startup-show.status"); see $CAPTURE/09-startup-show.out"
  holds 09-startup-show '"grade": "correlated"' \
    || fail "no acknowledgment of the attempt was correlated; see $CAPTURE/09-startup-show.out"
  holds 09-startup-show '"decision": "admitted"' \
    || fail "the startup decision did not admit the attempt; see $CAPTURE/09-startup-show.out"
  [ -e "$PROJECT/.statecraft/state/workspaces/acc-run/acc-sentinel" ] \
    || fail "the gated tool call did not run after admission"
  holds 09-startup-show "\"resolvedHarness\": \"$required\"" \
    || fail "the observed revision is not the required one; see $CAPTURE/09-startup-show.out"
  holds 09-startup-show '"supply": "supplied"' || fail "the run did not record its supply"
  holds 09-startup-show 'no live observation' || fail "the run's verdict does not name the missing class"
  say "  SYNTHETIC: run and startup show agree; the correlated revision is the required one,"
  say "  admitted before the gated tool call ran, in a fake stream. Nothing here is live."

  printf 'cli %s\n' "$CLI" >"$ACC/preflight.ok"
  say ""
  say "preflight: every precondition holds. Steps are recorded in $CAPTURE."
  say "The permission experiment needs APPROVED_PROVIDER_SESSION=yes, and"
  say "nothing in this stage is that approval."
}

# --------------------------------------------------- the permission experiment
#
# TWO PREMISES, AND WHAT GRADE EACH HAS. One `--allowedTools` followed by two
# rules that contain spaces: DOCUMENTED ("Comma or space-separated", example
# "Bash(git *) Edit") and READ STATICALLY from the installed 2.1.267 build,
# whose splitter keeps spaces inside parentheses; the argument construction is
# LOCALLY EXERCISED against a transcription of it. The floor's deny beating
# that grant: DOCUMENTED ("deny, then ask, then allow"; "can't be overridden
# by --allowedTools") and READ STATICALLY (a wildcard deny is checked before
# an exact allow is honored). Neither is OBSERVED until this stage runs live,
# and a fake cannot observe either. If either fails, the refusal control
# records an execution and the result is UNVERIFIED, never admitted.
#
# This stage starts no `run`: a run session would be a fourth session, and
# section 3.30 allows three. The run path is exercised by the preflight only,
# against a local fake.
#
# WHY EVERY CONTROL IS HARMLESS IF ENFORCEMENT FAILS. The product fixes the
# commands (spec 002 section 3.30): `cargo publish --dry-run --manifest-path
# statecraft-absent/Cargo.toml` names a manifest that does not exist, so cargo
# stops before resolving anything, searches no ancestor, and contacts no
# registry; `echo statecraft-allowed-control` prints a word. Each launch grants
# exactly those two commands, so an improvised variant is not granted.
permission_experiment() {
  if [ -n "${SC_ACCEPTANCE_FAKE_PROVIDER:-}" ]; then
    [ -z "${APPROVED_PROVIDER_SESSION:-}" ] \
      || refuse "SC_ACCEPTANCE_FAKE_PROVIDER and APPROVED_PROVIDER_SESSION are both set. The local route never uses the provider approval; unset one"
    program="$SC_ACCEPTANCE_FAKE_PROVIDER"
    [ -x "$program" ] || refuse "the fake provider $program is not an executable"
    origin=synthetic
    session=acc-synthetic
    say "LOCAL TEST ROUTE: $program is a fake. Every capture is SYNTHETIC."
  else
    [ "${APPROVED_PROVIDER_SESSION:-}" = "yes" ] \
      || refuse "this stage spawns up to three paid provider sessions. Re-run with APPROVED_PROVIDER_SESSION=yes once the owner has approved THIS stage. Approving it does not approve the coexistence stage."
    program="$PROVIDER"
    origin=launched
    session=acc-live
  fi
  need_preflight
  captures="$ACC/$origin"
  [ ! -e "$captures" ] \
    || refuse "$captures exists: a launch happens once. Run the preflight again for a fresh fixture"

  # Section 7 of the preflight: an unenforced refusal must not be able to make
  # a rustup proxy download a toolchain before cargo fails.
  export RUSTUP_AUTO_INSTALL=0
  launched=0
  for control in refusal allowed-command without-payload; do
    step "Launch: $control (session $((launched + 1)) of at most 3)"
    if [ "$origin" = synthetic ]; then
      bounded "10-$control" "$((STEP_TIMEOUT + 90))" product startup capture "$PROJECT" "$control" \
        "$captures" --program "$program" --deadline "$STEP_TIMEOUT" --synthetic
    else
      bounded "10-$control" "$((STEP_TIMEOUT + 90))" product startup capture "$PROJECT" "$control" \
        "$captures" --program "$program" --deadline "$STEP_TIMEOUT"
    fi
    launched=$((launched + 1))
    case "$RAN" in
      0) sed 's/^/  /' "$CAPTURE/10-$control.out" ;;
      1)
        sed 's/^/  /' "$CAPTURE/10-$control.out"
        finding "UNVERIFIED: the $control launch did not complete, and no further session was started ($launched of 3 launched). Its record is in $captures"
        ;;
      2) refuse "the $control launch was refused before anything ran: $(cat "$CAPTURE/10-$control.out")" ;;
      "") fail "the $control launch did not end by itself: $(cat "$CAPTURE/10-$control.status")" ;;
      *) fail "the $control launch: $(cat "$CAPTURE/10-$control.status"); see $CAPTURE/10-$control.err" ;;
    esac
  done

  step "The fixture project, after"
  # What a session said it did and what is on disk are two different facts.
  if [ -n "$(git -C "$PROJECT" status --porcelain)" ]; then
    git -C "$PROJECT" status --porcelain | sed 's/^/  /'
    finding "the project changed during the experiment; the captures are kept in $captures for review"
  fi

  step "The admission consumes the captures"
  bounded 11-qualify "$LOCAL_TIMEOUT" product startup qualify "$PROJECT" "$session" "$captures" --json
  say ""
  case "$RAN" in
    0|1)
      if [ "$origin" = synthetic ]; then
        say "ADMITTED (SYNTHETIC). The product's whole path ran against a local fake:"
        say "three launches, the admission, and a record marked synthetic, which never"
        say "qualifies. This is not live evidence of anything a provider does."
      else
        say "ADMITTED. The observation was recorded at"
        say "  $PROJECT/.statecraft/state/startup/$session.json"
        say "It is launcher-attested and version specific, and the record is not"
        say "qualified: supply is a separate evidence class this stage does not perform."
      fi
      printf '%s-admitted\n' "$origin" >"$ACC/result"
      ;;
    2)
      if holds 11-qualify '"operation": "qualification-refused"'; then
        printf '%s-unverified\n' "$origin" >"$ACC/result"
        say "UNVERIFIED. The admission refused the claim; see $CAPTURE/11-qualify.out"
        say "This is a result, not a failure of this script (spec 002 section 3.29"
        say "rule 6). The captures are kept in $captures."
        finding "the session is unverified"
      fi
      refuse "the qualification was refused before the evidence was judged; see $CAPTURE/11-qualify.out"
      ;;
    4) fail "the captures could not be read, so no claim was judged; see $CAPTURE/11-qualify.out" ;;
    "") fail "the qualification did not end by itself: $(cat "$CAPTURE/11-qualify.status")" ;;
    *) fail "the qualification verb answered $(cat "$CAPTURE/11-qualify.status"), which it does not document" ;;
  esac
}

# --------------------------------------------------- the coexistence experiment
#
# SEPARATE, and not run by anything in this repository's checks. It runs the
# four shipped hooks as programs, in the fixture project and in an unrelated
# repository, and checks the operator's settings are byte-identical before and
# after. It writes nothing to the real home. A hook that exits non-zero is a
# failure and is reported as one, not swallowed.
coexistence() {
  [ "${APPROVED_REAL_HOME_COEXISTENCE:-}" = "yes" ] \
    || refuse "this stage observes the real home. It needs its own consent under section 3.24, which approving the permission experiment did NOT give. Re-run with APPROVED_REAL_HOME_COEXISTENCE=yes only after the settings modification has been applied deliberately."
  need_preflight
  real_home="${STATECRAFT_HOME_REAL:-$HOME/.statecraft}"
  settings="$HOME/.claude/settings.json"
  [ -f "$settings" ] || refuse "$settings does not exist, so there is no modification to observe"

  step "E1. The operator's settings, before"
  cp "$settings" "$ACC/settings-before.json" || fail "could not copy $settings"

  step "E2. What this product's delivery would change, as a plan"
  bounded 20-home-plan "$LOCAL_TIMEOUT" "$CLI" home plan
  case "$RAN" in 0|1) ;; *) fail "home plan: $(cat "$CAPTURE/20-home-plan.status")" ;; esac

  step "E3. The shipped hooks, in the fixture project"
  bounded 21-harness "$LOCAL_TIMEOUT" "$CLI" harness show "$PROJECT" --json
  revision="$(sed -n 's/^ *"requiredDisplay": "\(h-[0-9a-f]*\)",*$/\1/p' "$CAPTURE/21-harness.out")"
  [ -n "$revision" ] || fail "no required harness revision in $CAPTURE/21-harness.out"
  hooks="$real_home/harness/$revision/hooks"
  [ -d "$hooks" ] || refuse "$hooks does not exist; the real home does not hold the required revision"
  failed=""
  for pair in SessionStart:session-start PostToolUse:post-edit PreToolUse:pre-bash Stop:stop; do
    event="${pair%%:*}"
    file="$hooks/statecraft-${pair#*:}.sh"
    printf '{"hook_event_name":"%s","tool_name":"Bash","tool_input":{"command":"true"}}' "$event" \
      >"$ACC/hook-$event.json"
    bounded "22-hook-$event" "$LOCAL_TIMEOUT" env CLAUDE_PROJECT_DIR="$PROJECT" \
      sh -c 'exec "$1" <"$2"' hook "$file" "$ACC/hook-$event.json"
    [ "$RAN" = 0 ] || failed="$failed $event($(cat "$CAPTURE/22-hook-$event.status"))"
  done
  [ -z "$failed" ] || fail "hooks failed in the fixture project:$failed; see $CAPTURE/22-hook-*.err"

  step "E4. The same hook in an unrelated repository says nothing"
  mkdir -p "$ACC/unrelated"
  git -C "$ACC/unrelated" init --quiet
  bounded 23-unrelated "$LOCAL_TIMEOUT" env CLAUDE_PROJECT_DIR="$ACC/unrelated" \
    sh -c 'exec "$1" <"$2"' hook "$hooks/statecraft-session-start.sh" "$ACC/hook-SessionStart.json"
  [ "$RAN" = 0 ] || fail "the hook failed in an unrelated repository: $(cat "$CAPTURE/23-unrelated.status")"
  [ ! -s "$CAPTURE/23-unrelated.out" ] || finding "a hook produced output in a repository holding no manifest; see $CAPTURE/23-unrelated.out"

  step "E5. The operator's settings, after"
  cmp -s "$ACC/settings-before.json" "$settings" \
    || fail "the operator's settings changed during an experiment that writes nothing"
  say ""
  say "coexistence: the hooks ran, the operator's settings are unchanged, and the"
  say "hooks are silent outside a target. Steps in $CAPTURE."
}

clean() {
  if [ ! -e "$ACC" ]; then
    say "nothing to remove at $ACC"
    return
  fi
  [ -f "$MARKER" ] || refuse "$ACC was not created by this script; it is not removed"
  rm -rf "$ACC"
  say "removed $ACC"
}

# ------------------------------------------------------------------ dispatch --
case "${1:-}" in
  preflight) preflight ;;
  permission-experiment) permission_experiment ;;
  coexistence) coexistence ;;
  clean) clean ;;
  *)
    cat >&2 <<USAGE
usage: sh scripts/acceptance/managed-session.sh <stage>

  preflight              local only, no provider, no approval needed
  permission-experiment  up to three provider sessions; needs APPROVED_PROVIDER_SESSION=yes
  coexistence            observes the real home; needs APPROVED_REAL_HOME_COEXISTENCE=yes
  clean                  removes \$ACC, only if this script created it

Each stage is a separate approval. Approving one does not approve another.

Environment:
  ACC=<dir>                           where everything is created (default \$TMPDIR/sc-accept)
  CLI=<path>                          a built statecraft-cli to use instead of building one
  STEP_TIMEOUT=<s>                    per provider session (default 300)
  LOCAL_TIMEOUT=<s>                   per local step (default 900)
  PROVIDER=<name>                     the provider binary (default claude)
  SC_ACCEPTANCE_FAKE_PROVIDER=<path>  the local test route: a fake, every capture synthetic
USAGE
    exit 3
    ;;
esac
