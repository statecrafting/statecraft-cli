<!-- Historical record only. Not normative. -->
<!-- Moved from specs/004-execution-adapter/spec.md at base d77e011e39dbc4689e49492c4e528d860be7eb86. -->

# Archived implementation journal: 004-execution-adapter

This file preserves the chronological implementation record that formerly
occupied section 5 of the active spec. Current requirements live in the
active spec's behavior section, and current rationale lives in its bounded
`Resolved decisions` section. Git remains the authoritative change history.


Dated entries for choices §3 was silent on. None changes what it requires.
The entries from 2026-09-17 onward that concern the first provider were
recorded against `008` while that spec was separate; they are kept verbatim,
because this section is a history and a history is corrected by appending.

**2026-09-19: a stdout read failure is reported as one, and is never a
completion.** The 2026-09-17 entry below separates terminal parsing from process
completion; it did not say what happens when the read itself fails. The
implementation answered by discarding the failure: the reader thread returned on
an `Err` from `lines()` and ignored the result of the trailing drain, and the
supervisor inferred the end of the stream from the channel disconnecting. That
inference is wrong in both directions. A read failure before a terminal event
was reported as `NoResult`, which claims the supervisor read the stream to its
end and found no result, an observation nobody made. A read failure while
draining after a valid terminal event was reported as nothing at all, and the
attempt was `completed`.

The reader now reports its own termination rather than letting the channel stand
for it, so a clean end of file and a failed read are distinguishable. A read
failure is a new `StreamError::ReadFailed` carrying the failure and the phase it
occurred in, and the attempt is `interrupted`, never `completed`. It is
`interrupted` rather than `failed` for the reason the 2026-09-16 entry below
gives: a stream the supervisor could not read judged nothing, so calling it
`failed` would claim an assessment of the work. The provider's terminal claim,
the trusted event prefix and the retained refusals are unchanged and stay
separate from this observation, which is what §3.1's division between the stream
and the result already requires.

Diagnostic precedence, where more than one is observed: **malformed**, then
**read failure**, then **no result**. A malformed line is a judgement about
bytes that did arrive and is the most specific. A read failure establishes that
the stream was never read to its end. Only a stream read to its end, carrying no
result, is `NoResult`.

Deadline enforcement, child-exit observation, descendant handling and the
no-blocking-join rule are untouched, and no field is added to the supervised
record. Validation is deterministic and does not reproduce an operating-system
race: a line that is not valid UTF-8 makes `lines()` fail in a real child, which
covers the failure before a terminal event through the process boundary; a
drain failure is injected through a private reader seam, since a trailing drain
copies bytes and cannot be made to fail by their content. `cargo test -p
statecraft-adapter --test deadline --locked` passes eleven tests, the ten it
carried plus the unreadable-stdout row, and the deadline, inherited-pipe,
malformed and draining rows are unchanged.

**2026-09-17: terminal parsing and process completion are separate.** The
deadline in §3.5.3 and the no-blocking-reader decision below cover the attempt
through pipe draining and child exit. A terminal event ends the trusted event
prefix, not deadline enforcement; a malformed line likewise ends parsing while
retaining the preceding events and the diagnostic. Cleanup continues under the
same deadline, and expiration interrupts even a child that already claimed
completion. Child exit is observed without blocking, and the remaining output
is drained without interpreting it. EOF alone cannot release
the deadline while the child is alive; child exit alone cannot release it while
a descendant holds the pipe. Exit status supplies no success authority. Prompt
delivery also belongs to that lifetime, so a child that does not read stdin
cannot delay the start of enforcement. These are implementation choices within
§3.1, §3.5.3 and §3.8, with no change to the ratified outcomes or qualification
contract. `cargo test -p statecraft-adapter --test deadline --locked` reproduced
five pre-repair hangs at the six-second outer limit for a one-second attempt:
terminal, malformed, inherited stdout, EOF with a live child, and unread prompt.
The repaired implementation passes that command's ten tests, including inherited
stdin, trailing-output draining, ordinary success, evidence preservation and
refusal precedence. Each fixture runs behind an independent outer timeout that
cleans up its process group and reaps the disposable supervisor process.

**2026-09-17: the fixture adapter is staged and copied into place.** Section 3.5
requires a fixture adapter that ships with the suite, and says nothing about how
it reaches the disk. Writing it directly at the path the suite is about to exec
opened a window: the suite's rows run as threads in one test binary, a sibling
thread that forks while the script is open for writing hands its child a
duplicate of that descriptor, and `execve` refuses a file any process holds open
for writing until that child reaches its own `exec`. It presented as an
intermittent Linux CI failure, `ExecutableFileBusy`, landing on a different row
each time, and it cost a re-run on several unrelated pull requests.

The helper now writes a staged file it never execs and has a **child process**
copy that file into place, so this process never opens the executed path for
writing and no fork of it can be holding a descriptor to it. Nothing about the
supervisor is involved or changed: the fixture is still a real script, still
exec'd through the same path, and every row of the suite asserts exactly what it
did. A new Linux-only reproduction, `cargo test -p statecraft-adapter --test
fixture_exec_race --locked`, forks children that outlive the fork while writer
threads write and exec their own fixtures. Against the original writer on Debian
bookworm with Rust 1.96.1 it refused 2 of 240 execs with `ExecutableFileBusy` on
each of three consecutive runs; after the repair, three consecutive runs refused
none. The same reproduction on darwin refused none either way, which is why it
is bounded to the platform where the failure was observed.

**2026-09-17: the inherited-input fixture saves stdin before backgrounding.**
Linux dash replaces an asynchronous command's stdin with `/dev/null` before
`<&0` can preserve it. The unchanged deadline suite reproduced PR #31's failure
in a disposable Debian bookworm container with Rust 1.96.0: nine tests passed,
but inherited stdin returned `completed` in approximately 11 ms. A bounded pipe
probe observed `/dev/null` at the descendant's fd 0 and a broken prompt pipe.
Saving the parent's stdin with `exec 3<&0`, then starting
`sleep 300 <&3 3<&- >/dev/null &`, retained the same pipe at the descendant's fd 0
after parent exit and kept the writer blocked. The probe confirmed blocking on
macOS sh too. With that fixture correction, the deadline command above passes
all ten tests on both platforms; inherited input additionally asserts that the
deadline elapsed and the descendant marker exists before checking cleanup.
The expected interruption, descendant cleanup and evidence assertions remain;
the supervisor implementation and ratified contract do not change.

**2026-09-16: the closed vocabulary is an enum, not a string.** §3.2 says adding
a token is an amendment and not a configuration change. A string token would let
a caller introduce a seventh; an enum means a seventh requires editing this
crate, which the coupling gate ties to editing this spec. A test asserts the set
has exactly six members.

**2026-09-16: a supervisor is never held past its own deadline, even by a
survivor.** §3.5.3 requires the kill; it does not say what happens if the kill
misses. CI answered that: a backgrounded process in the fixture survived the
group kill on Linux, still held the stdout pipe open, and the supervisor's
reader thread waited out the full 300 seconds after the deadline had correctly
fired at one. So the reader is dropped rather than joined when a deadline
fires. A supervisor that the supervised process can hold past its own deadline
is not one, and the surviving process is reported as a residual (§3.8) rather
than waited for.

**2026-09-16: descendants are killed with a process group, and `kill` is shelled
out to.** §3.5.3 requires the kill to reach descendants. The child is spawned
into its own process group and the group is signalled, which is the same
argument §3.6 makes about environments: a list of the pids somebody thought of
is not the set. Sending the signal shells out to `kill` rather than linking a
libc binding, because the dependency would be larger than the need. The form is
`kill -s KILL -- -PID`: the `--` is load-bearing, because a bare negative pid is
ambiguous with an option and the BSD and procps implementations disagree about
which it is. That disagreement is what made the failure above invisible on a
developer's machine and real on the runner. After the kill the group is probed
with signal 0, so a residual is observed rather than assumed absent. On a
non-Unix platform the descendants are not killed and the attempt says so, rather
than reporting a clean termination it did not achieve.

**2026-09-16: the fixture adapter is a real child process.** §3.5 requires a
fixture that ships with the suite. It is a `sh` script this crate writes and
spawns, not a function the suite calls: the stream, the deadline and the kill
are properties of a process, and a fixture that skipped the boundary would let
all three regress unnoticed. The fixture reads and discards the prompt from
stdin, so a regression to a command-line prompt fails a test.

**2026-09-16: a malformed stream and a missing result event are both
`interrupted`.** §3.8 gives the second explicitly and says only that the first
is "not `completed`". They land on the same outcome for the same reason: spec
003 §3.4 defines `failed` as the work having been judged and not held, and a
stream the supervisor cannot read judged nothing. Calling either `failed` would
claim an assessment nobody made.

**2026-09-16: the withheld-credential list is a refusal on construction, not a
filter.** §3.6 says publication credentials are not placed in the child. The
child inherits nothing, so the only route in is a blueprint, and that is what is
refused, with the withholding recorded as a degradation. The list names the
credential families that exist today; it is a backstop against a well-meaning
blueprint, not a containment boundary, and §3.6's residuals still stand.

**2026-09-17: the request's workspace is the child's working directory, and a
workspace that is not one is refused before spawn.** §3.1 puts the prepared
workspace in the request, and spec `003` §3.2 says the operator's checkout is
never edited and that no session runs in it. Neither says which line makes that
true of a spawned process, and nothing did: the supervisor built the child's
program, arguments, constructed environment and pipes, and never set a working
directory, so the child inherited the supervisor's own. That is the operator's
checkout whenever the command was started there, and a live provider run
measured exactly it, reporting the caller's directory in the child's init
event. The path was carried through the protocol and dropped at the boundary,
which is worse than never carrying it: every reader of the request had reason
to believe it was honored. The child is now spawned with the request's
workspace as its working directory. A workspace that does not exist, or that
exists and is not a directory, is refused before anything is spawned, naming
the path, on the error channel the supervisor already returns and in the shape
§3.3 uses for a required capability: the platform's own answer is an `ENOENT`
raised after the fork, which reads the same as an adapter binary that is not
there. Nothing §3 requires changed. This is §3.1's workspace and `003` §3.2's
isolation, enforced where a process actually acquires a directory. The suite
covers it with a fixture that reports the directory it is running in and writes
a marker there through a relative path, so a caller in one directory and a
workspace in another are separated by observation rather than by argument.

**2026-09-16: the provider-name rule is a test that greps this crate.** §3.8
makes a provider name in this territory a defect. A rule nobody can run is a
rule that decays, so `tests/no_provider_names.rs` scans the crate's own sources
for the provider names that exist today. It cannot catch a provider nobody has
heard of, and it does catch the one a future change would reach for.

**2026-09-17: the execution boundary owns the invocation's settings transport.**
Section 3.1 declares a settings document but leaves its transport unspecified.
Installed Claude Code 2.1.267's `--help` accepts `--settings <file-or-json>` as
additional settings. The adapter supplies the complete `Invocation` to its
execution boundary, including its program identity, and adds one `--settings`
argument naming an absolute, randomly named private temporary JSON file outside
the request workspace. The file is created exclusively (0600 on Unix), written
before spawn and held by the library until supervision returns, including an
interruption or spawn error. It is then removed. Temporary storage resolving
inside the workspace is refused. Forced termination of the supervisor itself
can leave the file behind; this is scoped cleanup, not hostile-child isolation.
A cleanup error after execution is retained as `settingsCleanupError` in the
execution evidence, alongside the existing terminal and refusal records. It
does not erase them or change the provider's claim or the outcome mapping.

The document is serialized as declared, never interpolated into shell text or
put in the environment. A second `--settings` in the invocation's argument
vector is rejected rather than silently selecting one document over the other.
The provider's native command-line precedence applies: managed settings remain
above this source, omitted keys retain their lower-source values, and permission
lists merge across scopes, per the provider's
[settings documentation](https://code.claude.com/docs/en/settings#settings-precedence)
read on this date. The constructed document contains only `permissions.deny`;
no hook, settings-source selector or environment entry is added. An empty deny
list follows the same transport and adds no denial policy. This discharges the
missing settings wiring recorded below, without changing qualification or the
mid-stream event wording, which was still unresolved on that date and which
Section 3.9 now settles.

Before repair, `cargo test -p statecraft-adapter-claude-code --test
settings_transport --locked` failed all three settings-reading child cases:
the real execution boundary passed no settings argument, so the fixture could
not read its declared document. After repair, the same command passes five
cases: special-character denial delivery, concurrent attempts, timeout after
a terminal denial, malformed-stream cleanup, and retention of denial evidence
when settings cleanup itself fails. These are synthetic transport checks, not
a provider qualification measurement.

**2026-09-17: run qualification is persisted posture, not target qualification.**
Sections 3.16 and 3.8, and sections 3.4 and 3.7, already require the labels.
The run binding reads the provider probe's paired qualification answer and uses
`004`'s `Posture`, including observed capabilities and process residuals. That
posture is stored in the attempt outcome's extensible detail. It is not the
environment's target verdict, and no qualification record is created by running.

The immediate outcome and `run show` expose the same recorded posture with its
source record, through an additive `posture` field. `006` section 3.4 explicitly
permits additive JSON fields; the existing seven run fields, their types, exit
codes and closed outcome words stay unchanged. The previous native-stream test's
exact field count describes that repair's scope, not a ratified prohibition on
additions. The acceptance library owns the read-only fold under the additive
edge above, reusing the seam's posture type and rendering. Historical attempts
with no posture report `not-recorded`; inspection never requalifies them from
current files. `cargo test -p statecraft-cli --test qualification --locked`
reproduced six missing-label failures before repair: all fixture runs completed,
but their JSON posture qualification was absent. The same six tests pass after
repair, exercising absent and mismatched records and matching synthetic evidence,
human and JSON output, retries and read-only inspection after removing the
provider and changing the qualification file. They confer no live qualification.

**2026-09-17: missing initialization does not erase a readable terminal denial.**
The native execution bridge's `map_stream` error path discarded the mapped
events when initialization was missing, even with a parsed terminal result
carrying denials. The result survived in detail while the supervisor counted
zero refusals. The repair shares the terminal denial-to-event mapping between
the successful and error paths. The error path retains only those independently
readable refusal events; it invents no initialization and keeps `NoInit`, an
`interrupted` execution and the provider's unchanged completed claim. The run
supervisor then counts the events and records `refused` under `003` sections
3.4 and 3.5 and this spec's section 3.11. Missing initialization without a denial
remains `interrupted`, as `004` section 5's malformed-stream decision requires.

This is implemented in `execution.rs` and `stream.rs` and tested by
`native_denied_success_without_init_keeps_refusals_and_the_stream_error` in the
provider negative suite and
`run_without_init_persists_denial_accounting_claim_and_diagnostic` in the CLI
native-stream suite. Both replay the recorded denied terminal event through a
real child with initialization removed. The CLI test reopens the persisted
accounting and outcome, checks the verbatim denial and diagnostic, and verifies
that acceptance does not run. Its undenied missing-init control stays interrupted.
No ratified requirement, ownership edge or acceptance command changes.

**Separate pre-existing finding, not repaired here:** the generic supervisor
leaves its deadline loop on a result or malformed line before calling
`child.wait()` and joining its reader. Either wait can therefore outlive the
deadline. This requires a separate supervisor repair under `004`, not an
extension of this refusal-accounting remediation.

**2026-09-17: native stream decoding belongs beside the provider mapping.**
The `run` binding sent native JSONL straight to `004`'s `event`-tagged parser,
although this provider emits `type`-tagged lines. The existing `map_stream` and
`outcome` bridge therefore never received a running child's output. The repair
uses `004`'s same process supervisor with a typed reader and terminal predicate,
then calls both existing mappings in this crate. The generic entry point still
uses its strict parser. Process creation, workspace cwd, environment, deadline
and descendant handling remain in `004`; the binding calls this provider's
entry point under the existing corrective edge on `006`.

The provider claim remains separate from refusal accounting: a denied success
is passed to `003` with its completed claim, mapped termination and refusal
events, so the supervisor counts the denials once and records `refused`.
An additive conclusion entry point separates that claim from the mapped
termination without changing the existing record fields. A turn cap supplies
`interrupted`. Unmapped terminal states and malformed or truncated streams
retain a diagnostic and cannot produce a completed outcome. Mapped events,
terminal fields and process diagnostics are retained in the outcome record's
existing extensible detail.
The closed outcomes and the command's serialized view are unchanged.

This wiring does not forward `Invocation.settings`, supply missing
`unqualified` labels, or widen the constructed environment with `USER`. Those
are separate findings and dependencies for live qualification, not claims this
repair discharges. At the time of this repair, section 3.9's mid-stream
refusal-event contradiction remained an owner amendment. Section 3.1 now
distinguishes the mid-stream notification from the terminal refusal record;
the mapper carries the former as progress and takes refusal evidence from
terminal `permission_denials` only.

**2026-09-17: the applied set reports what the invocation put into effect, minus
what the init event contradicts.** Spec §3.3 wants the init event to carry
what was *actually applied*, and §3.13.6 makes a declared-but-unapplied token a
qualification failure. §3.12 measured that this provider's init event witnesses
**one** of the five tokens directly, the tool set, and only under removal. So
neither extreme works: reporting everything granted would make §3.13.6 vacuous,
and reporting only what init proves would fail qualification on every real run.
The adapter reports what it put into effect and drops what init contradicts,
which today is exactly one thing (`hook-enforcement` before any hook event is
seen). The applied **tool set** stays `not-recorded`, which is §3.12's own answer
and is unaffected by this entry.

**2026-09-17: a terminal state §3.13's table does not list is reported, not
mapped.** The table covers `success` and `max_turns`. A subtype nobody measured
(`error_during_execution`, say) is returned as an unmapped terminal state, the
way spec §3.5 case 4 returns a malformed stream. An outcome this adapter
invented would be an outcome nobody measured.

**2026-09-17: `credential-path` is the presence of the mechanism, not of a
credential.** §3.14 measured `apiKeySource: "none"` and concluded the keychain
answers on `darwin`. Checking that a credential *works* means spending one,
which §4 puts out of scope, so the observable fact is the platform. On a
platform where §3 took no measurement the prerequisite reads **absent** rather
than assumed, because every finding in §3.9 to §3.14 is a fact about `darwin`.

**2026-09-17: `D-09`'s pointer prerequisite is not a fourth prerequisite here.**
Spec 002 §3.8 makes "this harness loads a pointer at this path" a declared
prerequisite, and §3.15 of this spec lists three that are not it. For this
harness the mechanism is the `@path` import in `CLAUDE.md`, which this provider
loads, so it is satisfied by construction and cannot be absent. The three §3.15
names are the ones that can be, and the declaration names exactly those.

**2026-09-17: the environment half's two rows reach the real declaration through
a dev-dependency.** §3.8's absent-prerequisite and colliding-path rows are
behaviors of the adapter model spec 002 owns, so the `## Verification` block runs
them in that crate's suite. Testing them against a declaration restated in the
test file would test a copy, and the copy is what drifts, so
`statecraft-environment` dev-depends on this adapter's crate. Cargo permits a
cycle through dev-dependencies; the library's own dependency graph is unchanged
and nothing in its `src/` may name the adapter.

**2026-09-17: the five verbs this spec unbinds take the exit the operation
returns.** The `extends` edge on spec 006's crate changes a refusal whose reason
this spec falsified, and 006 §3.7 fixes the exits for three of the five cases.
For the rest the exit is derived rather than chosen: `env plan` takes the exit
the apply it previews would take (a plan-level refusal is 2, a withheld path is
1, otherwise 0), because a preview whose exit disagrees with the operation it
previews is the one thing the verb exists to prevent. `env upgrade` is `env
apply` against the current declarations, which is spec 002 §3.6's own position.

**2026-09-17: a target must be registered before any environment verb runs.**
006 §3.7 requires `env apply` against an unregistered target to refuse naming the
path, and registration is the precondition all five verbs share. The binding
applies it to all five rather than to apply alone, which adds no rule: it applies
one 002 already has to the four verbs whose row 006 did not spell out.

**2026-09-19: the transport read failure is covered where this adapter maps
it.** Spec `004`'s entry of the same date adds `StreamError::ReadFailed` and
makes such a failure `interrupted`. §3.9's transport and §3.8's negative cases
are silent on which crate demonstrates that the new diagnostic survives this
adapter's own mapping, and the mapping is this spec's. The coverage is a unit
test beside the existing ones in this crate's execution module: a real child
emits a valid init event and then a line that is not valid UTF-8, and the test
asserts the observed outcome is `interrupted`, the stream error is the read
failure, the absent provider claim is reported as absent, the evidence carries
the diagnostic, and the settings file is still removed. No production behavior
in this crate changes: `stream_error` already forces `interrupted` and already
reaches the evidence as a string, and this records that both now hold for a
diagnostic that did not previously exist. No §3.8 row is added or altered.

The row names its own deadline, as the 2026-09-18 entry below requires of every
fixture here. The deadline is not what this row measures: the fixture has to
reach its unreadable line for a read failure to exist at all, and on a loaded
machine the one second the other rows share expires first, which reports the
absent init rather than the read failure. Measured: under six concurrent test
processes the shared deadline produced the absent init in 29 of 30 runs, and a
named 30-second deadline produced the read failure in 30 of 30.

**2026-09-18: the settings fixture consumes its prompt after the concurrency
barrier, and each test names its own deadline.** Both are properties of the
`## Verification` fixture, which §3 is silent on, and neither changes what
§3.9's transport or §3.8's negative cases require. The fixture's `cat` of the
prompt returns only at stdin EOF, and EOF needs every copy of the write end
closed, including one a concurrently spawned child inherited. Read before the
barrier, the two children of the concurrent test could each wait on the other:
one blocked on an EOF it could not reach, so it never signalled arrival, and the
peer spun in the barrier until both hit the deadline. The ordering requirement
removes that mutual wait. It does not remove every wait a stray descriptor can
cause, and the fixture's comment says so at the line it constrains.

The deadline becomes a per-test argument, short only where the deadline is the
subject and generous where it is incidental, matching the convention
`statecraft-adapter`'s negative suite already uses. That is mitigation, not a
fix: it widens the margin against a delayed EOF and leaves the behavior in
place. The timeout test keeps its 5 second budget and its 10 second bound, so
§3.8's bounded-cleanup row is asserted against the same numbers as before.
Nothing is disabled, retried or suppressed, and the four substantive guarantees
are unchanged. The residual is an EOF an inherited descriptor can still delay;
no finite clean run establishes that it cannot recur. Whether `supervise_stream`
gating completion on stdout EOF is itself a defect is a contract question for
spec `004` and is untouched here.

**2026-09-20: the first recorded qualification of this pair, and the live run it
admitted.** §3.16 makes a qualification an act an operator performs against a
named pair and records, and the `## Verification` note below says the act is
never re-derived by a check. No such record was found in the scope searched: this
repository's history, the default product home, and the two trial homes retained
from the 2026-09-17 measurements. Within that scope this is the first
performance of it. Broader provenance was not searched and is not claimed.
Nothing §3 requires changes; the measurements are §3.15's and §3.16's, taken
rather than described.

*What was measured, and against what.* Product source `94362ae` on `main`, a
clean tree, the binary built from it at
`sha256:726bb1208310786155abb1c3ee4b247c69bbbdcd35e7c1727071c7a8bc442578`.
Adapter `claude-code` build `0.0.0`. Provider `/Users/bart/.local/bin/claude`,
answering `2.1.267 (Claude Code)`, which is
`capabilities::MEASURED_PROVIDER_VERSION` and the version the committed streams
under `testdata/stream/` were captured from. Platform `darwin`, macOS 25.5.0.
The commands: `cargo test -p statecraft-adapter --test negative_suite --locked`,
18 passed with `suite_1` to `suite_8` all present; this spec's ten declared
commands, `PATH="$PWD/.tooling/bin:$PATH" make verify SPEC=008`, passed;
`004`'s seven, `make verify SPEC=004`, passed; and `make code` over the same
tree, 537 tests, build, clippy with warnings denied, formatting.

*Which adapter each §3.13 row was actually established against.* §3.5's
own table runs against the **fixture adapter**, which is what lets it run with
no real provider installed, and §3.4 says a suite pass by a sibling
adapter does not transfer. So `suite_1` to `suite_8` passing establishes the
seam, not this adapter. The evidence that does bear on this pair is the
provider-specific replay of the streams committed under `testdata/stream/`,
captured from 2.1.267 and driven through a real child and this crate's own
`execution::supervise`. Row by row:

| §3.5 row | Evidence bearing on **this** adapter | What the child was | Established |
|---|---|---|---|
| 1, a refusal retained beside a completed result | `negative_cases::native_denied_success_keeps_progress_claim_turns_and_exactly_one_refusal`; `a_denied_session_that_calls_itself_a_success_is_refused_and_never_completed` | recorded 2.1.267 stream replayed through a real child | yes |
| 2, a required capability the manifest lacks refused before spawn | `a_required_token_this_manifest_does_not_declare_refuses_before_any_spawn`, against this adapter's own manifest | none spawned, which is the assertion | yes |
| 3, a hung child killed at the deadline **with its descendants** | `settings_transport::timeout_after_terminal_denial_cleans_settings_and_retains_evidence` reaches the deadline and `interrupted` through this crate's execution path | a fixture shell child, not the provider | deadline and `interrupted` yes; **descendants no**: the descendant assertion exists only in `suite_3`, against the fixture adapter, and no hung real provider was ever measured |
| 4, a malformed stream reported as malformed | `a_malformed_stream_is_reported_as_malformed_and_never_a_clean_completion`; `native_malformed_and_truncated_streams_retain_progress_and_report_the_error` | recorded streams, mutated and truncated | yes |
| 5, an absent cost `unknown` and never zero | `a_reported_cost_is_carried_and_an_absent_one_is_unknown_and_never_zero` | recorded streams | yes |
| 6, a declared token the adapter does not honor fails qualification | `a_refusal_that_must_be_evidence_may_not_be_expressed_as_tool_set_removal` asserts `fails qualification` against this adapter's own denial mechanism and `tool-removed.jsonl` | recorded stream | this adapter's mechanism yes; the **runtime declared-not-applied discrepancy** path is asserted only in `suite_6`, against the fixture adapter |
| 7, records identical whichever implementation produced them | `native_recorded_success_crosses_the_process_seam` shows this adapter's records equal its own mapping and read through the generic seam | recorded stream replayed through a real child | **no**: `suite_7` compares two **fixture** implementations, and no comparison involving this adapter and a second implementation exists |
| 8, the posture declares every command the run will need | mechanism asserted in `statecraft-adapter`'s `a_posture_omitting_a_check_suite_command_is_refused_naming_the_command` | none | **no, and the product binding makes it vacuous**: `adapters::child_environment` supplies `manifest.requires_commands` as both the declared command set and the check-suite command set, so the two are equal by construction and the guard cannot fire |

Row 8's gap is not a reading of the code alone. The trial below declared
`python3 tests/check_wordcount.py` as its acceptance, `REQUIRES_COMMANDS` is
`["claude", "git"]`, and nothing refused at plan time: the environment reported
`applied` and the command resolved only because the blueprint copies the
operator's whole `PATH`. That is the case §3.13 row 8 exists to refuse, reaching
an attempt undetected.

*The record, and that it is the record and not a label.* Written to
`<product home>/qualifications.json` as `adapters.rs` reads one: adapter
`claude-code`, binary version `0.0.0`, suite version `008.3.9`, date
`2026-09-20T17:55:18Z`, provider version `2.1.267`. Its effect was measured on
both sides of writing it. Before: `env plan` reported the adapter `refused`,
`missing: ["qualification-record"]`, and planned zero writes. After: `claiming`,
and two writes, `.claude/statecraft/instructions.md` and `CLAUDE.md`. `env
apply` then wrote both and the manifest recorded both with their digests. That
is §3.15's refusal and its release, observed rather than asserted, and it is also
why an `env apply` exit code is not evidence that any path was installed.

Four facts, which this entry keeps apart because collapsing them is how a label
comes to stand for evidence it does not have. **A record was written**, naming
the pair. **The product honored it**, measured on both sides of writing it.
**The live run persisted the label**, readable from a fresh process. **Whether
the record rests on sufficient qualification evidence is not established**: the
table above leaves row 7 unestablished for this adapter, row 3's descendant
clause unestablished for this adapter, row 6 partial, and row 8 both
unestablished and vacuous in the product binding. A reader of the `qualified`
label should read it as "a record naming this pair exists and the product found
it", which is what the code computes, and not as "every §3.13 row has been
established against this adapter", which it is not.

*The live run.* A disposable corpus outside this repository, registered, armed,
and based at its own commit `30665d5`: one approved spec declaring one Python
module and a fixed acceptance suite present at that base. `statecraft-cli run
<corpus> 001-word-count-tool --json`, started `2026-09-20T17:56:15Z`, 57
seconds, exit 0. Outcome `completed`, provider claim `completed`, zero refusals,
base unmoved, capabilities `structured-refusals`, `turn-limit` and `cost-report`
all applied with none degraded, and the posture recorded `qualification:
"qualified"`. The provider implemented the module and committed it, leaving the
prepared work tree clean at candidate `52b51dd`.

*The acceptance.* `statecraft-cli accept <corpus> 001-word-count-tool --json`
answered `accepted` and minted a receipt naming base `30665d5`, candidate
`52b51dd`, one check (`spec-spine verify 001-word-count-tool --json`, exit 0),
policy digest `22b8c544...`, product `0.0.0`, spec-spine `0.20.0`, adapter
`0.0.0`, harness revision `not-recorded`, attempt `001-word-count-tool/1`, and
no authority path touched. No earlier `accepted` answer was found in the scope
searched. The historical run reported in PR 35 is not one: its trial spec
declared no suite, so `accept` correctly answered `no-acceptance`, and that
result stands as recorded.

*The receipt does not bind the suite §3.12 says it binds.* The plan declared at
the trusted base is one command, `python3 tests/check_wordcount.py`, which
`spec-spine verify 001-word-count-tool --plan` prints at `30665d5`. The receipt
carries one entry, and it is `spec-spine verify 001-word-count-tool --json` with
exit 0. §3.12 requires the ordered suite with **each command and each exit
code**; what is bound is the verifier invocation and the verifier's exit code,
so neither the declared command nor its exit code appears anywhere in the
receipt. At one declared command the difference is already total, and it grows
with the plan: an aggregate verdict cannot say which command failed, which never
ran, or in what order they went. `005` §3.4 is where that requirement lives and
this spec does not amend it; the measurement is recorded here because it is what
this trial produced.

*What the posture makes durable, and what an unmet obligation leaves undone.* A
fresh `run show` after the accept reports the posture, including
`qualification: "qualified"`, sourced to `attempt#001-word-count-tool/1`. It
reports the acceptance as `none` with reason `suite-did-not-run`, the receipt
freshness as `not-recorded`, and an authority note reading "no acceptance has
been run for this run". The run record carries `intent`, `accounting` and
`outcome` and nothing else: `accept` wrote nothing. So the run's qualification
posture is durable and the acceptance is not.

That is an **unmet obligation, not a design choice**, and this entry corrects an
earlier reading of it. `006` §3.1 gives `accept` as the verb that "judges the
candidate independently and **records the acceptance or its absence**". `005`
§3.1 requires candidate, base and policy to be "all named in the record", and
makes an unidentifiable one "**no acceptance**, recorded as such". §3.9.1
requires `not-attempted` to be "**recorded** rather than left blank", and says a
reader "must never have to infer from a missing receipt whether the suite ran
and failed, or never ran at all". The account produced here is exactly that
inference, and it resolves it wrongly: it asserts that no acceptance has been
run for a run that had just been accepted.

`005`'s 2026-09-16 entry, "nothing in this crate acts", is not an exemption from
this. It scopes to the acceptance **library**: `mint` returns a value and the
crate performs no external effect, which is how "a receipt is not a permission"
is implemented. The recording obligation falls on the verb's binding, which
`009` owns in the crate `006` owns. Repairing it belongs to `003`, `005` and
`009` together and not to this spec, which is why nothing here is amended; what
is recorded here is that the obligation is unmet and how it presents.

*Two bounded checks taken without the provider, because they bear on what a
receipt means here.* A candidate committed into the same prepared workspace that
rewrote its own `## Verification` block to a marker-writing command and appended
a line to `.claude/` was refused: `acceptance: failed`, reason
`authority-change`, members `acceptance-instructions` and `hooks`, no receipt,
exit 1. The marker file existed afterwards, so that candidate's own acceptance
instructions ran before the refusal. Two distinct things follow, and they are
not the same kind of thing. That the instructions came from the candidate is an
**unmet obligation**: `005` §3.2 already says the suite is run "from the
instructions read **at the base**", and `SpecSpineVerify::run_suite` runs
`spec-spine verify` with the prepared workspace as its working directory, whose
HEAD is the candidate. That the plan executed **before** the authority verdict
was computed is a **sequencing question §3 does not settle** either way, and
turning it into a stated rule needs its own amendment. Both live in `005` and
neither is amended here. A second candidate carrying the base's unimplemented stub under the base's
own acceptance was refused `suite-did-not-pass` with zero unrun checks and no
receipt.

*What this does not establish.* One run is not repeatability. The PR 35 run's
qualification posture is not established from retained evidence: no
`qualifications.json` exists at the default product home or in either retained
trial home from that date, and the run records retained there are `interrupted`
attempts carrying no posture field at all, so it is recorded here as unknown
rather than as unqualified, and its `no-acceptance` result is left exactly as it
was reported. Nor does this establish a conforming independent acceptance path:
trusted-base instruction loading, command-level receipt evidence and durable
recording are each unmet above, and a receipt produced while all three are
outstanding evidences that a verifier exited zero over named bytes, which is
less than §3.12 asks a receipt to bind.

*An ambiguity in the contract, named rather than resolved.* §3.4 defines
the act: "An adapter binary version is qualified only by a recorded pass of the
negative suite in §3.13", and §3.13 makes that table runnable with no real
provider installed. The `## Verification` note below says instead that "the live
measurement stays where §3.16 puts it: a qualification act performed by an
operator against a named binary version and recorded", and that "what the suite
checks is the consequence rather than the act". These do not name the same act.
Under the first, what was done today is the act, completely. Under the second,
the act is a live measurement that no spec defines: nothing states what it must
cover, what would fail it, or how its result enters the record, and
`qualification::record` takes a provider version, a suite version and a date
with no live-measurement input at all. This entry does not choose between the
two readings. It records that the first is satisfied to the extent the row table
above allows, that the second has no definition to satisfy, and that resolving
which one §3.16 means is the owner's.

*Where this evidence lives.* Under
`~/DevWork/statecraft-cli-evidence/2026-09-20-live-qualification/`, outside this
repository and outside any temporary directory: 46 files with an `INVENTORY.md`,
a `SHA256SUMS` verified after copying, the command logs, the isolated product
home's register, qualification record and run-record chain, the environment
manifest, the binary under test, and a git bundle carrying the trial corpus's
complete history including the accepted candidate `52b51dd`, the hostile
candidate `1eecdf8` and the failing-suite candidate `44451e1` under
`refs/evidence/`. The originals under `/private/tmp/statecraft-trial-2026-09-20/`
are preserved unchanged. That archive is an evidence archive and is **not**
receipt persistence: the product still records no acceptance, which is the
defect the archive documents.

*The declared acceptance did not pass on every attempt, and that is retained.*
The first `make verify SPEC=004` of the day failed at command 2, in
`settings_transport::timeout_after_terminal_denial_cleans_settings_and_retains_evidence`,
and so did the first `make verify SPEC=008` of the branch that carries this
entry. Both passed on a later attempt against the same tree, and the same test
also passed a full `make code`, ten consecutive isolated runs of the single
test, and six consecutive runs of its whole test binary. Under a temporary local
instrumentation of the fixture, not committed, every observed failure had the
same shape: the spawned child produced no event, no terminal record and none of
the files it writes as its first actions, including one written to an absolute
path outside the workspace, and the attempt ran to its 5 second deadline. The
2026-09-18 entry above records a residual for this fixture, a delayed end of
file on an inherited descriptor. A child that never wrote anything is a
different signature and is not claimed to be that residual recurring. No retry,
timeout or assertion was changed to obtain any pass recorded above. The
consequence for this spec is stated rather than softened: its declared
acceptance is not yet reliably repeatable on this machine, and the cause is an
open question in this fixture's own territory.

**2026-09-21: the suite label `008.3.9` is not renamed by this consolidation.**
Section 3.16 makes a qualification bind to a named binary version and a named
suite, and the suite this adapter was qualified against was recorded as
`008.3.9` while the provider sections were spec `008`. A record binds to the
label that was recorded. Renaming it here would leave every existing
qualification record naming a suite that no longer exists, which is the one
thing section 3.16 exists to prevent, so the label stands and the section it
was minted from is now section 3.8. A future suite revision mints a new label;
it does not retitle this one.

**2026-09-21: the two back-edges become `extends` alone, not `depends_on`.**
While the provider half was spec `008`, its frontmatter could name `005` and
`006` as dependencies: the seam did not, so the graph stayed acyclic. Merging
the two documents makes one spec that both rests on `005` and `006` and is
rested on by them, and `compile` refuses the cycle (`V-014`).

The resolution is not to weaken either claim. Both `extends` edges stay, and
they are the edges that carry the real relationship: they name the exact unit
reached and the nature of the reach, which `depends_on` never did. What is
dropped is only the coarser second statement of the same fact. The direction
that survives in `depends_on` is the one that was always true of the seam: `005`
depends on this spec, and so does `006`.

This is a cost of the consolidation and is recorded as one. A reader who wants
to know what the provider half needs from acceptance and from the command
surface reads the `extends` edges, which say more than the dropped lines did.

**2026-09-22, authority: the adapter hands its caller the hook responses, the
session id and the settings bytes it wrote.** Spec `002` section 3.31 measures
which harness revision answered a run from a `SessionStart` hook's output, and
records supply from the operation that performs it. Both readings are this
adapter's: §3.9 owns the native stream and the settings file. So the `system`
event's typing gains the three `hook_response` fields the recorded `2.1.267`
streams carry (`stdout`, `exit_code`, `outcome`), and an execution carries,
beside what it already carried, every `hook_response` in the order read with its
session id, the init event's session id, and the exact bytes written to the
settings file, read back from that file before the spawn. Additive: no mapping,
outcome, classification or existing field changes, and the generic seam gains
nothing. A field the provider does not emit stays absent, and no fixture
invents one.

**2026-09-22, authority: a run's constructed environment carries four
attempt-binding names.** Section 3.6 constructs the child environment from an
allowed set, and §3.14 names the two that set held. Spec `002` section 3.31
rule 17 adds `STATECRAFT_RUN_ID`, `STATECRAFT_ATTEMPT`,
`STATECRAFT_STARTUP_NONCE` and `STATECRAFT_HARNESS_SELECTED` for a run, so that
a hook in the session can acknowledge which attempt and which selected revision
it is running under. None is a credential, none carries one, each is set from a
value this product generated or recorded, and the environment stays
constructed: what the child receives is still the complete, recorded set.

**2026-09-22, authority: the deadline suite's contract is corrected before the
tests are.** `crates/statecraft-adapter/tests/deadline.rs` runs ten fixtures
with a one-second deadline starting at `spawn` and asserts, for every one, that
the child's init and refusal events were retained, that supervision returned
inside four seconds, and, behind that, a six-second outer limit. The first
assertion needs a precondition §3.5 case 3 does not promise, that the child is
`execve`d, runs and is read inside that second; the 2026-09-21 measurement in
spec `002` section 5 shows a loaded machine spending a whole deadline in
`execve`. The four-second bound is scheduler-sensitive in the same way: after
the deadline the supervisor spawns `kill` twice, and nothing bounds how long a
spawn takes. Neither assertion can fail only when the product is wrong.

**What the product promises about time, exactly.** The deadline runs from the
spawn. A hung attempt is never ended before it. When it fires, the supervisor
stops waiting on the child, on its pipes and on the prompt writer, keeps what
the reader had already delivered, kills the process group, probes it, and
returns without joining a reader a survivor could hold. The latency after the
deadline is that kill sequence's, and no bound on it is promised.

The corrected suite keeps every property and gives each the measurement that
can establish it:

1. **Deadline and cleanup, through a real process**, for the six fixtures that
   hang: a terminal event then a hang, a malformed line then a hang, exit with
   an inherited output pipe, end of file while the child lives, a prompt the
   child never reads, and exit with an inherited input pipe. The deadline stays
   one second from `spawn`. Unconditionally: supervision returns no earlier
   than the deadline; returns within sixty seconds, a bound chosen to separate
   the defect it exists to catch, a supervisor held by the child's 300-second
   hang, from `kill` latency, and which is therefore a statement that the
   supervisor was not held and not a measure of promptness; the outcome is
   `interrupted`; no survivor is reported; the child and any descendant it
   started are dead by process id. What was retained is checked against the
   child's own trace, written after each line it emitted: every retained event
   is, in order, one the child emitted, within the trusted prefix. Full
   retention is not asserted here, because it needs the child to have run.
2. **Event order and retention, deterministically**, in the supervisor's unit
   tests, through the private reading seam with an injected clock: the scripted
   stream is delivered, the clock is advanced past the deadline only once the
   reader has asked for bytes beyond it, and the child is a real process in its
   own group so the kill is real. That establishes, with no race, that a
   terminal event followed by a hang is retained whole and ends `interrupted`
   with the provider's claim kept, that a malformed line keeps the events
   before it and its diagnostic, that end of file with a live child and a
   blocked prompt writer both hold the supervisor until the deadline and no
   longer, and that evidence delivered before the interruption survives it.
   Production passes the real clock; nothing else about the seam changes.
3. **Completion, through a real process**, for the four fixtures that end by
   themselves: success, trailing output drained, exit with no result, and
   unreadable output. The deadline is not what they measure, so each names a
   sixty-second one as a watchdog, which the 2026-09-18 entry's convention
   already requires, and they keep their exact assertions on events, outcome
   and diagnostic. A fixture that has not started within sixty seconds reports
   a timeout, which these assertions distinguish from the property under test.

The outer limit on each disposable worker process becomes 120 seconds. It is a
watchdog that kills and reaps a worker the supervisor failed to release, it runs
its cleanup before any assertion, and it is not evidence of anything the product
promises. Nothing is retried, serialized or repeated to obtain a pass.

**2026-09-22: the deadline suite, measured before and after.** Before: with the
fixture's first line delayed two seconds, a stand-in for the `execve`
starvation measured on 2026-09-21, `terminal_then_hang` failed on
`matches!(run.events.first(), Some(Event::Init { .. }))` while the supervisor
behaved correctly, returning `interrupted` at 1.03 seconds against its
one-second deadline. That is the defect the entry above names: a test that
fails with the product right. After: the suite carries that shape as an
eleventh real-process row, `a_child_that_has_not_started_by_the_deadline`, and
passes all twelve tests, the worker included. The supervisor's eight new seam
tests pass, and two deliberate mutations, applied and reverted, show what they
catch: removing the post-deadline drain fails the terminal-then-hang test, and
letting end of file release the supervisor fails the end-of-file and
blocked-writer tests. The seam's fixtures bound their own blocking at sixty
seconds, so a supervisor that wrongly waits on them fails by outcome rather than
holding the test run.

**2026-09-22, authority: a caller may watch a supervised launch, confirm the
spawn before the prompt is delivered, and stop the process at an event.** Spec
`002` section 3.32 needs two things only this supervisor can give it. First, a
point between the spawn and the prompt: rule 22 persists a spawn confirmation
and delivers the prompt only after it, so a confirmation that cannot be
persisted leaves a process that was never given work. Second, a decision at an
event: rule 26 decides at the first non-startup event and stops the process
group on a refusal. So the supervisor gains an optional **watch** the caller
supplies. It is told the process id immediately after the spawn call returns a
process and before the prompt writer starts, and may refuse, in which case the
process group is killed, no prompt is written, and the supervision reports the
refusal and an interrupted outcome. It is shown each decoded event, in order,
on the supervising thread, and may ask for the process to be stopped, in which
case the group is killed as at the deadline, the events read so far are kept,
and the supervision reports why it was stopped. A supervision that stopped this
way is `interrupted`: the process did not end by itself. Nothing changes for a
caller that supplies no watch: its spawn, prompt timing, deadline, descendant
handling and outcome are what they were, and the deadline suite is unchanged.
The generic seam still names no provider; the claude-code adapter passes its
native events to the watch unmapped, and adds a reading of one native event as
a hook response or an init session id so the caller need not re-derive the
provider's spelling. The same rule 25 supplies a managed run's hooks per
invocation, so an invocation may carry a `hooks` value in its settings beside
the deny rules; the exact-bytes rule of the settings document is unchanged and
now covers both, and an invocation that registers no hooks writes what it
wrote before.

**2026-09-22: a test that writes a script and execs it is the `ETXTBSY` race,
wherever it is.** CI on this change's pull request failed one row of the
negative suite, `the_prompt_reaches_the_child_on_a_stream_and_no_argument_carries_it`,
with `ExecutableFileBusy`. The test wrote its own script in place and exec'd it,
so a sibling thread's fork could hold that file open for writing at the moment
of the exec: the race `fixture::write` already documents and avoids, at a site
that bypassed it. The product was not involved. The staging is now a public
helper, `fixture::install_script`, and every test in this spec's two crates that
execs a script it wrote goes through it: the failing row, four execution tests,
the settings-transport fixture, and three probe tests, the last being the
second site the 2026-09-19 repair left.

**2026-09-22: the survivor probe waits for reaping, for a bounded time.** CI on
a documentation-only pull request, with code identical to a `main` that had
passed, failed the deadline suite's `late-start` row: supervision reported
"process group still answers after SIGKILL". The fixture's shell runs `sleep`
in the foreground before it emits anything; at the deadline the group is
killed, the shell is reaped by the supervisor, and the `sleep` is reparented
and remains a zombie until its new parent collects it. Signal 0 succeeds on a
zombie, and the probe asked once, immediately, so it raced that reaping and
reported a process the kill had ended as a survivor. The probe now repeats for
up to two seconds and reports only what still answers then, naming the
possibility of an uncollected zombie. A process that genuinely survives
`SIGKILL` is still reported, two seconds later than before, and a supervision
with no survivor pays nothing. The suite's assertion that no survivor is
reported is unchanged.

**2026-09-23: the supervisor reports that its deadline fired.** `Supervised`
gains `timed_out`, set from the supervisor's own observation and carried
unchanged through the Claude Code adapter's mapping. Before this, a caller had
to infer a deadline from an interrupted outcome and the absence of a stream
error. That inference fails when the mapping adds an error of its own: a stream
stopped before `init` also carries "no init event", and spec `002` section
3.33's trial judged such a session a failed launch rather than an uncertain one.
Outcomes, stream errors and every existing field are unchanged. The seam tests
assert the flag for a deadline, a normal completion and an exit with no result.

**2026-09-23: section 3.17 implemented, and the choices it was silent on.**
Nothing §3.17 requires changed. The reading, the comparison and the verdicts
are `statecraft-adapter`'s new `coverage` module; which tree each input is read
from is `statecraft-cli`'s `coverage` module; the attempt record is written
through the edges this spec already declares on `003`'s and `006`'s crates, and
`002`'s crate carries the declared list (its own section 5 of this date).

*The two inputs, and how they stay independent.* The requirement is
`spec-spine verify <spec> --plan --json`, run with `SpecSpineCli`'s binary, the
one work selection runs, in the tree being read; each `commands` entry is read
by `coverage::classify`. The allowance is the adapter manifest's
`requires_commands` plus `project.commands` read from the declaration's bytes.
`Coverage::compare` takes the plan and the allowance as separate arguments and
nothing derives one from the other. `adapters::child_environment_with` now
gives `construct` the allowance as the posture's commands and the plan's
programs as the suite's, so `environment` reports `refused` for the same
missing program; without a suite (the environment verbs) it compares nothing,
and never passes the manifest's list as both.

*The plan's reading.* The envelope must name verb `verify`, say `ok: true` with
exit code 0, and carry a report whose `specId`, `commands` and `skipped` are
present; a missing `commands` is unreadable, not empty. `specId` is recorded
and not compared with the asked id, because spec-spine resolves a short form to
the directory name. A skipped block is `{tag, count}`, as spec-spine emits it.

*The simple-command reading.* Within a double-quoted span a `'` is literal and
opens no single-quoted span, as POSIX has it; the metacharacters are still
refused there, since the rule exempts single-quoted spans only. A line break is
`\n` or `\r`. The first word ends at the first space or tab. A first word
holding `/` is `path` only when it holds no `=` and no quote; otherwise it is
`unparsed`.

*Digests.* SHA-256, lowercase hex, over the serialization of what was read:
the parsed plan (`specId`, `commands`, `skipped`, `acceptanceFrom` when
present), the allowance (its entries with their sources, and the declaration
record), and the declared list. So whitespace or member order in spec-spine's
output does not move a digest, and a change to either input does.

*What planning refuses, and how.* A malformed declaration, an unreadable plan
and a `refused` verdict each return exit 2 with `guard: posture-coverage`,
`phase: planning`, the reason and, where one was computed, the coverage.
Nothing is appended and no process is created. The intent records the planning
reading under `postureCoverage`: `phase`, `spec`, `verdict`, `planDigest` and
`allowanceDigest`.

*What launch refuses, and how.* The base's declaration is read with `git
ls-tree` (no entry is absent) and `git show`; the base's tree is exported with
`git archive` into a fresh directory under the system temporary directory,
which is removed once the plan is read. A digest that differs from planning's is
named in the coverage's `drift` (`allowance`, `suite plan`), which refuses
whatever the verdict. The attempt concludes `refused` under `posture-coverage`
with `postureCoverageRefusal` beside the posture, which the existing mapping
reports as exit 1, a finding. A read that fails at launch refuses the same way
and records no `posture.coverage`, so it reads as not checked, with the reason
in the refusal.

*An absent coverage.* `posture.coverage` is `not-checked` on an attempt that
has none, and deserializes that way from a record written before this date.
The managed-startup trial reads the declaration at its base too, so a malformed
one refuses it as it would a run (rule 1); its verdict is `not-applicable` and
it reads no plan.

*Fixtures.* Every fake `spec-spine` in the binary's existing tests now answers
`verify <spec> --plan --json` with an empty plan, the only change to them. The
new suite, `statecraft-cli`'s `posture_coverage`, drives the binary with a fake
spec-spine that answers the plan from the directory it runs in and logs that
directory, so planning and launch are observed reading different trees.

**2026-09-23: the launch reading, corrected after review.** The entry above
is kept as written; four of its implementation choices were wrong or silent,
and nothing §3.17 requires changes.

*The base is the intent's.* The launch reading took
`session.workspace.base_commit`, which for a reused workspace is that
worktree's `HEAD`: the first attempt's base, or a commit a session made there.
Rule 4 names the attempt's base commit, and the intent records it as
`baseCommit`. `Session` now carries that value (`base_commit`, written beside
the workspace's own), and the launch reading uses it. So a second attempt after
a committed change reads the new commit, and a commit made inside the
workspace is never read as the base. The launch reading also takes the spec
from the planning coverage rather than the run id.

*The export is a temporary index, not an archive.* `git archive` honors
`export-ignore` and `export-subst`, so an attribute could hide or rewrite the
suite. The base's tree is now read into an index file of its own (`read-tree`)
and every entry checked out into a private `tempfile` directory
(`checkout-index --all`), with system and global git configuration disabled,
hooks and fsmonitor off; only the target's own repository configuration still
applies. Gitlinks are not populated, as before.

*The plan read has a deadline.* The reader runs under the supervisor's
`capture`, in its own process group, with 60 seconds to answer
(`PLAN_DEADLINE`). At the deadline the group is killed and the read is
refused as unreadable, naming the deadline.

*A launch reading that could not be made is recorded as such.*
`posture.coverage` holds `{"unread": <reason>}` for it, and `run show` renders
that reason beside the coverage line, so it no longer reads as a bare
`not checked`.

**2026-09-23: the base export's Git invocation, per the second review.** Every
inherited `GIT_*` variable is removed before system and global configuration
are switched off, so none can name another index, directory, object store or
injected configuration; the registered target is named as `safe.directory`,
because switching global configuration off also drops an operator's own
`safe.directory`. Filters the target's own `.git/config` defines still run
during `checkout-index`: that file is the operator's, not committed content,
and spec `004` section 3.18 rule 3 keeps it out of the child's writable roots.

**2026-09-25: spec `006`'s JSON naming convention, this spec's half (adopted by
the owner on 2026-09-25; spec `006` section 5 of that date).** `Negotiation`
(`missingRequired`), the adapter `Manifest` (`requiresCommands`) and the Claude
Code `Invocation` (`toolRestriction`, `settingsDocument`) now serialize in
camelCase. None of the three is written to disk, printed by a verb, or read by
another repository today; spec `006`'s source scan found them. The protocol
(`Request`, `AttemptIdentity`, `AdapterResult`), the posture recorded with every
attempt, the qualification records in `qualifications.json` and the provider
stream mirror are grandfathered: the first is a wire contract, the next two are
persisted, the last is the provider's own spelling. An adapter manifest is not
made strict: it has no schema version (its `version` is the binary's) and is
never read from a file; `qualifications.json` has no schema version either.

**2026-09-25: the suite plan reads both `--json` envelopes (owner,
2026-09-25: adopt 0.26.0).** spec-spine 0.26.0's verdict envelope (schema
1.0.0, its spec 132) removes `ok` and carries `outcome`. `parse_plan` required
`ok`, so it would have refused every plan a 0.26.0 binary answered. It now
reads either envelope and stays as strict: exit 0 and `ok` true, or exit 0 and
`outcome` `ok`; an envelope that says neither is unreadable, never an empty
plan. `the_plan_parses_strictly` carries the 0.26.0 shape as measured on this
repository the same day.
