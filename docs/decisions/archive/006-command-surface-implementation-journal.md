<!-- Historical record only. Not normative. -->
<!-- Moved from specs/006-command-surface/spec.md at base d77e011e39dbc4689e49492c4e528d860be7eb86. -->

# Archived implementation journal: 006-command-surface

This file preserves the chronological implementation record that formerly
occupied section 5 of the active spec. Current requirements live in the
active spec's behavior section, and current rationale lives in its bounded
`Resolved decisions` section. Git remains the authoritative change history.


Dated entries for choices §3 was silent on. None changes what it requires. The
entries below 2026-09-17 that concern `work`, `run` and `accept` were recorded
against `009` while that spec was separate; they are kept verbatim, because this
section is a history and a history is corrected by appending.

**2026-09-16: the JSON contract has its own view types.** §3.4 makes this
crate's JSON a contract. The library types behind the verbs are not that
contract, and deriving `Serialize` onto spec 002's `Outcome` and `Report` from
here would have been a change to 002's territory made by 006's change, which the
coupling gate refuses and should. So the wire shapes are declared here, where
the contract is, and the bindings map onto them. The two reasons point the same
way, which is usually the sign a boundary is in the right place.

**2026-09-16: the environment verbs refuse, and that is not a stub.** §3.1 says a
verb joins the tree as the behavior behind it lands, and `env plan`, `env
apply`, `env upgrade`, `env remove` and `doctor` all need a configured adapter
set. No spec ratifies a provider adapter, so there is nothing for them to plan
against. They exit 2 with the reason, which is the correct answer to a
precondition that is not met, rather than exit 0 having done nothing or a
"not implemented" message that reads like a defect. The verbs that need no
adapter, the four `project` verbs, work.

**2026-09-17: the entry above is superseded by spec 008.** It is kept rather than
rewritten, because it records what was true on its own date and this section is
a history. What has changed is its premise: spec `008` ratified Claude Code as
the first provider adapter and declares the corrective edge on this spec's
command crate. The configured adapter set is `declarations` in
`crates/statecraft-cli/src/adapters.rs`, and the environment verbs are bound to
it in `environment_verb` in `crates/statecraft-cli/src/main.rs`. So the
unconditional refusal for want of a ratified adapter no longer applies. A
prerequisite that is genuinely absent still refuses, under `002` section 3.9 and
`004` section 3.14, and that is the same correct answer to an unmet precondition
the entry above describes.

**2026-09-16: the product home is overridable by `STATECRAFT_HOME`.** §3.6 says
the binary reads its arguments, the target and the product home, and that no
configuration file may change a rule. An environment variable naming *where the
product's own state lives* changes no rule: it relocates the register. It earns
its place by making the end-to-end tests possible at all, since they must not
write into the developer's real home to check that registration writes nothing
into a target.

**2026-09-16: a relative path is resolved, not refused.** Spec 002 refuses a
relative path, and that refusal is about what the register stores. An operator
typing `project register .` has not made an error, so the binding makes the path
absolute against the working directory first. It does not canonicalize:
resolving symlinks would record a path the operator did not name.

**2026-09-16: the last gate flag joined with this change.**
`index check --fail-on-unresolved` was withheld while specs claimed crates they
had not written. `006` built the last one, so it is now enforced in `make gate`
and in CI. The cost is recorded in AGENTS.md: a new spec claiming a crate ahead
of its implementation now fails the gate, and removing the flag again would be
its own deliberate change.

**2026-09-17: every verb takes the target path as its first argument.** §3.1
names the verbs and not their arguments, and §3.10's last row makes arguments
naming no operation a usage error. The product works in *registered* targets, so
every verb needs to know which one; taking it as an argument rather than
inferring it from the working directory keeps it visible in the invocation that
produced a run record, which is §006 3.6's rule about ambient state.

**2026-09-17: `list` and `show` are reserved after `run`.** `run <id>` and `run
list` are ambiguous, so a run id may not be spelled either word. Stated here
rather than discovered: the alternative is an id that silently becomes a
subcommand.

**2026-09-17: the run id is the spec id.** §3.1 says the operator names a unit
of work, and `003` §3.4 makes a retry an appended attempt of the same run. A
fresh id per invocation would turn every retry into a new run and defeat that.

**2026-09-17: `--help` is answered before a verb is resolved.** `work --help`
has to work, and `work` alone is a group rather than a verb, so a help request
is recognised first and its topic may be a group. Exit 0: the question was asked
and answered. Help is not in the command tree's own list, because a usage error
listing it would offer help as a thing to do.

**2026-09-17: concluding an attempt does not release the workspace.**
`003`'s `workspace::release` says "used when a run ends; never during one", and
concluding an *attempt* is not a run ending. Measured while implementing: the
release removed the worktree and left the branch it had created, so the next
attempt could not prepare, which §3.12's "no automatic retry" rule depends on
working. The workspace is retained and the outcome record says so. When a run
ends is a question §3 does not answer and this entry does not answer either.

**2026-09-17: the slice's JSON carries an owning crate's own type where that
crate already derives it.** §3.4 makes this crate's JSON a contract and
§5 records why the environment verbs got view types: deriving `Serialize`
onto spec 002's `Outcome` from here would have been a change to 002's territory.
That reason does not apply to `Eligibility`, `ReviewableOutcome` and
`Acceptance`: each is already serialisable in its owning crate and each is the
shape its own spec fixed, and `005` §3.9's account in particular must not have a
second shape. So only wire shapes this crate had to invent get a view type
here. The visible consequence is that those three serialise their fields in
snake case while this crate's own views use camel case, which is recorded rather
than hidden; unifying it is a change to §3.4.

**2026-09-17: a preflight refusal concludes the attempt rather than leaving it
live.** `004` §3.3 refuses before any process is created, and the intent is
already durable by then (`003` §3.6). An intent with no outcome would send the
next run to reconciliation for an attempt that never started, so the refusal is
recorded as the attempt's outcome with the guard named.

**2026-09-17: the delta report is obtained in the target, not in the
workspace.** `005` §3.3.1 rule 2 requires the classifying binary to be resolved
independently of the candidate, and rule 3 keeps the invocation out of the
acceptance library. This is the caller side the decision record assigns to this
spec: the reader landed in #20 and `accept` is the verb that obtains a report.
Base and candidate are named explicitly so the report is about this change.

**2026-09-18: `run` refuses an unarmed target, and only `run`.** `002` §3.1
already requires arming as the consent to being driven and
`Registration::eligible` already reads it; §3.10 placed `run`'s other
preconditions in §3.3's vocabulary and was silent on this one, so the
binding drove a registered target that had consented to nothing. It is a
precondition, so a refusal (**2**) naming the path and the act that would
consent, and it is evaluated where the registration is already read, which puts
it ahead of the corpus report and therefore ahead of every effect: no workspace,
no appended attempt, no spawned provider. Only `run` drives, so only `run` is
gated: `work list`, `work show`, `run list`, `run show` and `accept` read, and
a registered target staying readable while unarmed is what `002` §3.1 records a
target for. The gate is on consent alone and not on `eligible`, which also folds
in qualification: those are `002`'s two independent conditions and conflating
them here would answer a question this change did not ask. Disarming withdraws
consent for the next invocation; it does not cancel a live attempt, which §3
does not provide and this entry does not add.

**2026-09-17: the policy digest is computed over the base's bytes, read with
`git show`.** `005` §3.3 requires every authority-set member to be read at the
trusted base. A base that carries no declared authority-set path at all is a
**refusal**, because a digest nobody can compute identifies no policy. Which
paths are members is `005` §3.3 case 2's declaration, by path, and the five this
repository declares are listed in the binding.

**2026-09-21: five verbs for `002`, and the spellings were chosen to match the
tree rather than the library.** `harness` and `startup` are new groups;
`session` is a third. Each groups a noun the way `project`, `env`, `home` and
`config` already do, and each verb after it is the act. The alternatives
considered were folding all five under `env`, which would have put a read of the
harness requirement beside `env apply` and invited the reading that one implies
the other, and folding the two `startup` verbs into one with a flag, which would
have made submitting evidence look like an option on recording rather than the
separate act §3.29 requires. `session payload` takes no path for the reason §3.11.1
states; the other four take one and none of them requires it to be registered,
because reading a requirement and recording a start are both things a target
does before it is driven.

**2026-09-22, authority: §3.11.2, recorded before the binding.** The launch
verb and the split of `startup qualify`'s exit 2. The alternative considered for
the launch was leaving it in the acceptance script and having the script write
its argument list into the submission from the same variables it spawned with.
That is the design `002` section 3.30 found insufficient, because the same
variables can be written twice differently and nothing checks that they were
not. The table row for `session payload` is corrected in the same change: it
named a `--digest` option that never existed, since the identity moved to the
`--json` rendering on 2026-09-21.

**2026-09-22: two producer capabilities this surface can use, recorded as
bounded opportunities and not as requirements.** spec-spine builds features
when an opportunity exists rather than waiting for a consumer's request, so the
two below are written down before they ship. Neither changes the pin
(`=0.20.0`), neither copies a producer internal, and adopting either is its own
change with its own re-index and bypass-floor review (`D-06`). Measured against
spec-spine's tree on 2026-09-22: its `v0.22.0` candidate is integrated, untagged
and unpublished, and carries neither capability.

*Readiness status on the ready set.* Producer contract: spec-spine spec 102
(`status: draft`, `implementation: pending`), which adds `status` to each
`registry plan --json` ready entry as a read-schema MINOR and changes no
partition, ordering or exit code; spec 101, which is in the `v0.22.0` candidate,
only documents that `ready` is a scheduling answer. Where it meets this
surface: section 3.8's join, which exists because the plan carries no status,
and spec `003` section 3.1.1's rule that ready is not ratified. **Version
prerequisite:** a published spec-spine whose `registry plan` read schema has
taken spec 102's MINOR, pinned here. **What it may and may not change:** it may
let the join cross-check two sources for `status` and refuse when they
disagree; it may not let the plan's field replace the lifecycle report as the
source of `status`, because approval stays this product's rule. **Prepared
now:** `crates/statecraft-cli/tests/producer_compatibility.rs` drives `work
list` against a stub emitting spec 102's shape with a `status` that
**contradicts** `registry list`, and asserts that the additive field is accepted
and that eligibility is still decided by `registry list`. It is a compatibility
fixture: it says what this consumer does with the shape, and nothing about
whether any release emits it.

*Portable verifier fixtures.* Producer contract: spec-spine spec 103
(`status: draft`, `implementation: pending`, its build on a sibling branch):
case directories of stored `payload.json` bytes and a `case.json` naming the
payload type, its schema version, a digest subject and the expected `match`,
`mismatch` or `refused` outcome with a reason from a closed set. Where it meets
this product: the envelope crate spec `005` owns, and the startup intent of
`002` section 3.31, which today identifies the project by its manifest digest
and not by the corpus attestation the work was scheduled from. **Version
prerequisite:** a published spec-spine release carrying spec 103's fixture set
as an artifact, with its index version. **Nothing is prepared**, because no
supported local mechanism supplies the fixtures without copying them out of an
unreleased branch. **The consumer-verification plan, exactly:** pin the release
that publishes the set; add a test that walks its index, feeds each case's
`payload.json` bytes, unmodified, to this product's attestation verification,
and asserts the case's expected outcome and reason, failing on any case it
cannot classify rather than skipping it; then, as a separate authority
amendment to `002` section 3.31, decide whether the startup intent records the
corpus `attestationHash` beside the manifest digest.

**2026-09-22, later: the producer state re-measured, and four integration
opportunities with their exact consumer checks.** The entry above measured a
producer that has since moved, so this one supersedes its producer facts and
leaves its reasoning standing. Measured from the remote on 2026-09-22: the
latest spec-spine **release** is `v0.21.0` (2026-09-20) on GitHub and on
crates.io for both `spec-spine-cli` and `spec-spine-core`. On its `main`, spec
102 (`#307`), spec 103 (`#301`) and spec 106 (`#308`) are merged with
`implementation: complete`, and spec 107 merged as `#309` (`6e123d2`) while this
entry was being written. All four are `status: draft` in that corpus and none
is in any release. The pin here stays `=0.20.0`, and nothing below repins it.

| Opportunity | Producer contract | Where it meets this product | Consumer check when a release carries it |
|---|---|---|---|
| readiness status on the ready set | 102: each `registry plan --json` ready entry gains `status`; measured by building `45becbb` from a clean export, the plan answers `schemaVersion` `0.3.0` with entries of `id`, `status`, `title`, where the pinned `0.20.0` answers `0.1.0` with `id` and `title` | section 3.8's join | `producer_compatibility.rs` now emits that measured shape key for key and still asserts the join decides; on adoption, add a cross-check that refuses when the two sources disagree, and keep `registry list` the source |
| portable verifier fixtures | 103: stored `payload.json` cases with an expected outcome and a closed reason set, as a published artifact | the envelope spec `005` owns | walk the released index, feed each case's bytes unmodified to this product's verification, assert the expected outcome and reason, and fail on any case it cannot classify |
| obligation references in operator evidence | 106: `registry obligation <spec>#<id> --json` resolves one obligation with its `sectionDigest` | spec `002` section 3.32's records name sections in prose | a startup record could cite the obligations it answers to (for example `002#3.32`) with their section digests, so a later reader can tell whether the rule it was judged under changed; that is an authority amendment to `002`, not a binding change here |
| context-closure identity in run records | 107: `registry closure --request <file or -> --json` resolves a request of specs, sections and obligations to members with identities and one order-independent `digest` | spec `003`'s run record and `002` section 3.31's intent | the intent could carry the closure digest of the work order the session was given, beside the manifest digest; consumer check: resolve a fixed request twice over an unchanged ledger and get one digest, change one member and get another |

None of the four is implemented here, because none has a released producer
contract, and a compatibility fixture is the only mechanism this repository
supports for an unreleased one. (Superseded in part by later entries: 102's
cross-check is `003` section 3.1.2 and 107's closure binding is `003` section
3.1.3, both implemented and both inert until a pinned producer carries them;
103 and 106 are not consumed at run time; `003` section 5 records an ignored
test that replays 103's fixtures only as evidence about a named producer
build.)

**2026-09-22: `startup trial`, recorded before its binding.** Section
3.11.4 adds the eighth verb and the trial section of `startup show`, for `002`
section 3.33. The verb requires the operator to state provider execution or a
fake, because a default would let a script spend a session nobody named.
No code changed with this entry.

**2026-09-22: `startup trial` bound.** Section 3.11.4's verb calls one
path: `run`'s launch, factored so `run` and the trial share it and differ only
in the plan (prompt, turn limit, deadline) and in the trial's watch. The
binding checks the trial's preconditions before an attempt is appended, adds
`run`'s own (a registered, armed target), and maps the library's judgement to
section 3.11.4's codes. `startup show` gains its `trial` section through the
library's `inspect`, so its binding did not change. The probe reports the
version its single `--version` call read, so the trial records it without a
second call.

**2026-09-22: section 3.8 rule 1 now names both sources of `status`.**
`work list` printed a fixed `registry list --json: status` for every row. Spec
003 section 3.1.2 records where each row's `status` came from, `registry list`
alone or `registry list` with the plan agreeing, and the row now prints that
value from the library rather than a constant. The join itself is unchanged,
as rule 2 requires; dropping it would still be its own change, and it cannot be
dropped while only `registry list` carries `implementation`.

`tests/producer_compatibility.rs` asserted the rule 003 section 3.1.2 replaced:
a plan `status` contradicting the list was ignored and the list decided. It
now asserts the new rule in two tests, a contradiction refused (exit 2) naming
both values, and an agreeing plan with the join still deciding.

**2026-09-22: the contract in `run`'s and `accept`'s answers.** `run`'s answer
gains `contract`, the binding its attempt's intent holds, and one summary
line. `accept`'s answer is the acceptance with `contract` beside it, flattened
so the acceptance's own members keep their places; a stale ledger is refused
with exit 2 before anything is judged, and `contract-moved` is a finding, exit
1, like the other acceptances that ran and found something. Both fields are
additive under section 3.4.

**2026-09-23: `startup trial` records whether the deadline stopped the
session.** The binding copies spec `004`'s new `timed_out` into the trial's
process end, so spec `002` rule 37 is judged from the supervisor's observation.
`startup_trial.rs` gains `a_session_stopped_before_its_first_event_is_uncertain_not_failed`,
whose fake writes nothing, so its answer does not depend on scheduling. The
older deadline test now holds under load for the same reason: a fake starved
past its deadline is `uncertain` whether or not it wrote anything.

**2026-09-23: where `work list` places a stale ledger and a refused pin.**
Section 3.10 names the missing-field refusal and, through `003` section 3.8,
the corpus that does not compile (a finding, **1**). It is silent on the two
answers `003` now separates from that finding (its section 5 entry of this
date). A stale ledger is **2**: a precondition, nothing read, cured by
recompiling. A producer that refused the target, such as a pin it does not
satisfy, is **2** as well, like the absent producer beside it. Either way nothing
was done, and an operator can act. `producer_compatibility.rs` pins all three
through the built binary against a stub producer. Two of them fail on the
previous build, which reported both refusals as a compile failure under exit 1.

**2026-09-23: the pinned producer carries 102, 106 and 107.** Under `=0.23.0`
the entries above that frame spec-spine's 102 and 107 as future describe
shipped behavior: `work list` compares plan and list `status`, and `run` binds
a closure. 106's obligations resolve, but this corpus declares none. 103's
fixtures are replayed only as evidence about a named build (`003` section 5).
Nothing in this spec's crate changed for the adoption beyond
`producer_compatibility.rs`'s module note, which named the old pin.

**2026-09-23: how the reconciliations of section 3.11.6 are rendered.** `run
show` adds `reconciliations`, every reconciliation record of the run in chain
order, each with its attempt, chain position, `verdict`, `basis` (`null` for
the older shape of `003` section 3.6.1 rule 6, rendered as releasing nothing),
`corroborated`, `observedLaunchState` and the record whole; the human
rendering prints one line per record. The reviewable account of `005` section
3.9 is unchanged. `run list` adds `reconciliation` to each attempt, the latest
one `003` rule 4 folds (`null` where there is none). `run`'s live-attempt
refusal adds `reconciliation`, the live attempt's `unknown` one, and names it
in the human rendering. A refusal of `run reconcile` that is not about the
lock is its own answer, with exit 2 unchanged.

**2026-09-23: the transfer verbs implemented, and what section 3.11.7 left to
the binding.** The three verbs are bindings in `src/transfer.rs` onto `002`
section 3.35's operation in the environment crate, and they join section 3.1's
table and the group list the tree prints in this change. Six choices the
section was silent on:

- **A registered target, as for the environment verbs.** An unregistered path
  is refused, exit 2, naming it, which is section 3.7's row for `env apply`
  applied to the verbs that change the same manifest.
- **Usage is shape; a blank value is a refusal.** A missing argument or a class
  word other than `user`, `adopted` or `managed` is exit 3 with nothing on
  standard output. An operator or reason that is present and blank is the
  library's refusal, exit 2, which is how section 3.11.5 already reads the
  same case for the override verbs. A class pair that parses but is not one of
  rule 1's four moves is exit 2 (`move-not-admitted`), because the arguments do
  name an operation and the operation declined.
- **A refusal names its rule, and a stale plan names what changed.** The JSON
  value is `{"refused": {"kind", "detail", "changed"}}`, where `kind` is one
  closed kebab-case word per refusal (`stale-plan`, `class-mismatch`,
  `move-not-admitted`, `no-source`, `adapter-not-claiming`,
  `instruction-file`, `modification`, `protected`, `escaping`,
  `not-relative`, `symbolic-link`, `directory`, `not-a-regular-file`,
  `missing`, `spelling`, `alias`, `journal-disagrees`, `no-manifest`,
  `unknown-transfer`, `later-transfer`, `class-changed`,
  `unrecorded-change`, `missing-operator-or-reason`, `busy`), and `changed`,
  present for `stale-plan`, lists which of the plan's inputs moved
  (`classes`, `path`, `file`, `manifest`, `undetermined`, or `identity`).
  `<plan-id>` is either the bare SHA-256 plan identity `transfer plan` prints
  as `identity`, which is what `002` rule 4 names, or the token it prints as
  `plan_id`, which carries that identity beside a short digest of each input;
  a still-current identity is never refused in either spelling.
- **A plan that finds the journal disagreeing still exits 0.** Rule 5 says
  `transfer plan` reports it; section 3.11.7 gives `plan` no finding code. The
  answer lists every disagreement and says that `apply` and `revert` will
  refuse until it is resolved.
- **Exit 4 covers the file as well as the manifest, and a write in force but
  not durable.** A file whose digest a plan needs and that cannot be read
  (anything but absent, which is a refusal) is a failure, which is section
  3.3's general meaning of 4. When the manifest was renamed into place and its
  directory could not be flushed, the answer is 4 with `{"failed":
  "not-durable", "in_force": ...}`, naming the record that is in force, so
  the code says what section 3.11.7 says and the value says what happened.
  A temporary file an interrupted earlier write left, which the write
  removes, is named in the answer (`write.removed_leftovers`).
- **`env apply` and `env upgrade` read the manifest under the lock they
  write it with** (`apply::apply_current`), so a transfer recorded between a
  read and a write is never erased; this is `002`'s one-writer rule reaching
  the binding, not a new rule here. When another writer holds the lock past
  the wait, or the manifest changed since it was read, nothing was written,
  so the answer is a refusal, exit 2, under section 3.3; a filesystem that
  cannot take the lock at all is a failure, exit 4, naming the lock file.

`tests/ownership_transfer.rs` exercises every acceptance and negative case of
`002` section 3.35 through the built binary, with `STATECRAFT_HOME`, `HOME` and
`PATH` constructed per test, including each stale-plan input, a second
spelling, a disagreeing journal, and another holder of the lock. Rule 1 admits
`managed` only where the adapter claims its paths, and the claude-code
adapter's credential prerequisite is satisfied only on macOS (`004` section
3.14): the tests that need a `managed` path assert it there, against a fake
provider and a synthetic qualification record, and elsewhere assert the
`adapter-not-claiming` refusal and report the rest skipped. The refusal is
also asserted on every platform by removing the qualification record, and the
library suite in the environment crate asserts the `managed` halves on every
platform with a test probe.

**2026-09-23: `--replace` on `env plan`, `env apply` and `env upgrade`.** Spec
`002` section 3.4 requires that replacing a drifted managed file needs the
operator to say so per path, and its section 5 entry of this date records the
mechanism. The binding adds one repeatable option and no verb:
`env plan <path> --replace <file>` and `env apply|upgrade <path> --replace
<file>=<plan-id>`. Arguments after the target are parsed strictly, for every
environment verb and `doctor`: anything other than `--replace` pairs, a
`--replace` with no path or with an option as its value, a consent with no
path or an identity that is not 64 hexadecimal digits, a consent given to
`env plan`, and `--replace` on `env remove` or `doctor` are usage, **3**. A
refused named path or a stale identity is **2** and nothing is written; a
failure to stage or rename is **4**; the outcome's own exit otherwise, so
`already-satisfied` alone is **0**. Each command still calls one library
operation (`plan_naming`, and `apply_consented_current`, which reads, plans
and writes the manifest itself). Both answers gain a `named` list, the plan's
with each replaceable path's plan identity, the apply's with what became of
each named path; the apply's answer also gains `swept`, the staged files an
interrupted replacement left and this one removed. The apply's outcome fields
keep their places. All are additive under section 3.4. `tests/env_replace.rs`
spawns the binary.

**2026-09-23: `env remove` names where the root bridge lives.** Spec `002`
section 3.13 rule 4 is now performed by `env remove` (its section 5 entry of
this date). The binding still calls one library operation, `remove_with`, and
passes it the one place this product puts a bridge, the root `AGENTS.md`, the
`import-bridge` kind and the import line, the path and line taken from
`statecraft-home` rather than restated. A bridge withheld for ambiguous or
unauthorized ownership is a withheld path in the existing answer, exit **1**.
The answer gains `notes`, flattened beside the outcome's fields, for what is
not a finding: a bridge line the manifest does not record, and a record
dropped because an interrupted removal had already taken its line back. A note
never changes the exit. Additive under section 3.4.
`tests/env_remove_bridge.rs` spawns the binary.

**2026-09-24, adopted (owner, 2026-09-25): a JSON naming convention, and
input documents that refuse unknown fields (owner Addendum 2, item N).**
Written as a proposal on 2026-09-24; the owner adopted it on 2026-09-25, and
the change that records the adoption implements it (the paragraphs after this
entry, dated 2026-09-25). The text below is the proposal as adopted.

*What the source says today* (main `17dbdb6`; the scan and the key list are in
the session evidence). Of the serialized types in `crates/*/src`, 116 structs
carry `rename_all = "camelCase"`, 63 structs have only one-word fields (the
case does not show), and **55 structs carry snake_case field names** because
they have no `rename_all`. Enums are consistent: 61 plain and 46 tagged enums
use `rename_all = "kebab-case"`; the exceptions are 2 camelCase-tagged, 1
snake_case-tagged, 1 lowercase and 4 untagged. The binary tests read 245
distinct keys: 53 camelCase, 14 snake_case (`spec_spine`, `plan_id`,
`evaluated_against`, `declared_by`, `manifest_digest`, `num_turns`,
`permission_denials`, ...), the rest one word. So one document can mix both:
`init apply --json` reports `observedSpecSpine` beside a manifest whose pins
say `spec_spine`.

Where the snake_case structs are: the environment manifest
(`statecraft-environment/src/manifest.rs`: `Entry`, `Pins`, `Project`,
`Transfer`, `Modification`, `Written`) and transfer records; the run record and
journal (`statecraft-run`: `record::Entry`, `Attempt`, `Workspace`, `Policy`,
`WorkItem`, `WorkList`); acceptance and evidence (`statecraft-acceptance`:
`Receipt`, `SuiteEntry`, `ReviewableOutcome`, `VerifierRecord`;
`statecraft-envelope`: `Dimensions`, `Root`, `RootSet`, `EvidenceVerdict`,
`Reference`); and the provider mirror (`statecraft-adapter-claude-code/src/stream.rs`),
whose names are the provider's.

*Proposed convention.* Keys this product authors are **camelCase**; string
enum values are **kebab-case**; a type whose names are someone else's (the
provider's stream, the producer's report mirror in `producer.rs`) keeps that
owner's names and says so in a comment. The persisted snake_case documents
above are **grandfathered**, not renamed in place: renaming a key in a
committed manifest or a hash-chained run record is a schema change for every
existing file, so each moves only with a `schemaVersion` bump and a reader
for the old version, as its owning spec decides.

*Enforcing test (design, not implemented).* In `statecraft-cli`'s tests, one
helper walks any `--json` value and asserts every object key matches
`^[a-z][a-zA-Z0-9]*$` except under a named, shrinking exemption list (the
grandfathered types' paths, the provider mirror); every binary test that
parses `--json` output calls it, so the convention is checked on real output
rather than on declarations. A second, source-level test lists every
`Serialize` type without `rename_all` and fails when one appears outside the
same exemption list, which catches a new type before any test prints it.

*Refusing unknown fields in input documents: what it would break.* Today four
types use `deny_unknown_fields` (`statecraft-run/src/overrides.rs`,
`statecraft-adapter/src/coverage.rs`, two in the provider stream). Making the
documents this product reads refuse unknown fields would:

1. **Environment manifest** (`.statecraft/environment.json`): an older binary
   reading a declaration written by a newer one would refuse instead of
   ignoring the field. This week alone added `role`, `pins.producer`,
   `project.setup` and transfer records, all additive and all read today by
   older builds. Needs a `schemaVersion` check first, so the refusal says
   "written by a newer version" rather than naming a field.
2. **Run records and the journal**: the same, across every recorded run;
   and a record is never rewritten, so an old binary could not read a run a
   newer one wrote.
3. **Evidence and receipts**: section 3.4 calls adding a field compatible;
   refusing unknown fields in a portable receipt makes every additive field a
   breaking one for verifiers. Strictness here is a spec `005` decision.
4. **Producer report mirror** (`producer.rs`): deliberately tolerant, so the
   producer can add a field without breaking this consumer; stays tolerant.
5. **Bundle metadata**: not implemented yet; can be strict from its first
   version at no cost.
6. **Home settings, the registry, adapter manifests**: the same forward
   incompatibility as 1, smaller in scope.

Recommended: strict **within** a `schemaVersion`, with an unknown field under
the current version refused and a newer version refused by name; items 4 kept
tolerant; item 5 strict from birth; items 1 to 3 changed only with their
owning spec's schema bump.

**2026-09-25: the convention, implemented.** What the adopted entry above
designed, and every choice it left open, decided here.

*The walker.* `crates/statecraft-cli/tests/support/json_naming.rs` holds one
helper that walks a parsed value and fails on any object key outside
`^[a-z][a-zA-Z0-9]*$`, and **the one exemption list**, `GRANDFATHERED`, which
names each grandfathered document, why it does not move now, its types and its
keys. Every binary test that parses `--json` output parses it through that
helper (`from_output` or `from_text`); a record read from disk is not `--json`
output and is not walked. The first full run flagged 54 distinct keys; every
one is either renamed below or named in the list.

*The source scan.* `crates/statecraft-cli/tests/json_naming.rs` reads
`crates/*/src/**/*.rs`, finds each item that derives `Serialize`, and computes
the name serde writes for each field, variant and struct-variant field from
`rename_all`, `rename_all_fields` and a member's own `rename`. A field must meet
the key rule and a variant's name the value rule (kebab-case, one word
included). This is the entry's "every `Serialize` type without `rename_all`",
made exact: a type without `rename_all` whose fields are all one word writes
conforming names and is not listed, and a struct variant's fields, which
`rename_all` on an enum does not reach, are checked too (that is how
`unrun_checks` and `session_id` were found). A second test refuses a list
entry whose type no longer needs it, so the list only shrinks. A hand-written
`Serialize` or a `json!` literal is invisible to the scan; the walker is what
sees those.

*The exemption list*, in full:

1. The environment manifest and its transfer journal (spec `002`): committed
   in every adopter, including the `project.setup` parameters. Profile
   revision 7's `ci.extra_required_jobs` joined that document while this
   change was open, so its key is listed with the others; a new key in an
   already grandfathered document is the same schema, not a new exemption.
2. The transfer plan's `plan_id` (spec `002`, and this section's entry on
   `transfer`): both specs name the field. The rest of the plan is renamed.
3. The project register's qualification reasons that carry data
   (`corpus-does-not-compile` and two siblings): persisted in `projects.json`,
   written with the kebab-case variant name as the key.
4. The setup profile's six results (`files-installed` and five more): spec
   `002`'s results table names them as kebab-case identifiers.
5. The startup and trial records (spec `002`): recorded and read back;
   `session_id`, `hook_name` and `exit_code` copy the provider's hook-event
   names, and `rel_path` names a required file.
6. The run record and journal (spec `003`), with the work list the adopted entry
   classed beside it.
7. The adapter protocol, the posture recorded with every attempt, and the
   qualification records in `qualifications.json` (spec `004`).
8. The provider stream mirror (spec `004`).
9. Acceptance and portable evidence (spec `005`), including the
   `incomplete-evidence` key an admission refusal carries.

The producer report mirror needs no entry: it is `Deserialize` only, prints
nothing, and spec-spine's names are already camelCase.

*Keys renamed*, because each is `--json` output only, is persisted nowhere
(each type derives `Serialize` and not `Deserialize`, or is never written),
and no other repository reads it. Under section 3.4 a rename removes a field,
so this entry is the change to this spec that makes it:

- `transfer plan`: `manifest_digest`, `recorded_without_journal`,
  `journal_disagreements`, and in `current`, `recorded_digest`,
  `matches_record`, `declared_by`, become `manifestDigest`,
  `recordedWithoutJournal`, `journalDisagreements`, `recordedDigest`,
  `matchesRecord`, `declaredBy`. `plan_id` stays.
- The initialization report: `conformance.out_of_contract` becomes
  `outOfContract`.
- The settings answers: `supplied_by` and `constrained_by` in a resolved or
  refused key, `digest_before` and `digest_after` in a removal, and
  `content_token` and `target_digest` in a stale consent become camelCase;
  `line_number` in an ignore-merge refusal becomes `lineNumber`.
- Spec `004`'s `Negotiation` (`missing_required`), adapter `Manifest`
  (`requires_commands`) and the Claude Code `Invocation` (`tool_restriction`,
  `settings_document`) become camelCase. None is written to disk or printed
  today; the source scan found them.

The owning specs record their halves: spec `002` and spec `004` section 5,
2026-09-25.

*What was made strict.* `home.json` and `tools.json` in the product home, the
only input documents in scope that carry a schema version: strict within
version 1, a newer version refused by its number before any member is read
(spec `002` section 5, 2026-09-25, with tests). Neither document has ever lost
a member, so no file this product wrote is refused.

*Not made strict, and why.*

- **Bundle metadata:** does not exist. No crate reads or writes one, so there
  is nothing to make strict; the adopted recommendation (strict from its first
  version) binds whoever introduces it.
- **The environment manifest, run records and the journal, evidence and
  receipts** (items 1 to 3): changed only with their owning spec's schema bump,
  as adopted.
- **The producer report mirror** (item 4): stays tolerant, as adopted.
- **The project register, `projects.json`:** carries no schema version, so an
  unknown member could only be refused by naming it, which is the forward
  incompatibility item 1 describes. It needs a version first; spec `002`'s.
- **Qualification records, `qualifications.json`:** no schema version; the same
  reason, spec `004`'s.
- **Adapter manifests:** carry no schema version (their `version` is the
  adapter binary's), and no adapter manifest is read from a file: each is
  constructed in code. Nothing to refuse.
- **`delivery.json` and `modifications.json` in the product home:** bare
  arrays with no version, the same reason as the register.

**2026-09-24, option (a) adopted 2026-09-25: the hand-written parser
against clap (owner Addendum 2, item P).** The owner adopted option (a) of the
assessment below: clap for per-verb arguments only, while verb resolution
(`Verb::parse`) and help before resolution stay as they are. This entry
implements nothing. Its conditions bind the change that will implement it:
that change lands only after the family exit and JSON contract (proposed for
this section in draft #118, adopted by the owner on 2026-09-25, landing with
the spec-spine 0.26.0 migration) is on `main`; it maps every clap error to exit
3 with a test for each verb, except clap's `DisplayHelp` and `DisplayVersion`
kinds (`--help`, `--version`), which exit 0 and are tested too; and it measures release binary size and clean
build time on one machine before and after. The assessment below is as
prepared, except that its binary-size row no longer quotes an unrebuilt
figure.

*What exists* (main `17dbdb6`). `commands.rs` (546 lines) resolves the verb
from the first two words by one closed `match` over `Verb::all()` (42
operations in 15 groups), accepts `--json` anywhere after the verb, answers
`--help`/`-h` before verb resolution so a group (`work --help`) has help, and
renders help from `Verb::all()` so a new verb cannot be missing from it. Each
verb's own arguments are then parsed by hand in `main.rs`, `manage.rs`,
`bind.rs` and `accept.rs`: 24 `Exit::Usage` returns and 11 separate
`eprintln!("usage: ...")` lines, with about 20 flags (`--plan`, `--profile`,
`--remote`, `--head`, `--attempt`, `--evidence`, `--deadline`,
`--verify-local`, `--replace`, `--force`, ...). There are no shell
completions.

| Property | Hand-written today | clap 4 (derive) |
|---|---|---|
| Verb help | one generated list; per-verb help is a line, not argument docs | per-verb and per-flag help generated from the same definitions |
| Error messages | consistent for an unknown verb; per-verb usage lines written by hand, 11 of them, and not all flags are named in them | uniform ("unexpected argument", "a value is required", suggestions for typos) |
| Shell completions | none | `clap_complete` generates bash, zsh, fish, PowerShell |
| Usage exit code | 3, section 3.3 | clap exits **2** on a usage error by default. Keeping 3 requires `try_parse` and mapping `clap::Error` to `Exit::Usage` ourselves, and `--help` and `--version` (clap's `DisplayHelp` and `DisplayVersion` kinds) to 0 |
| `--json` anywhere | explicit | a `global = true` flag; same behavior |
| `run <spec-id>` beside `run list|show|reconcile` | reserved words stated in `Verb::parse` | expressible (`args_conflicts_with_subcommands`), but the reservation must be restated and tested |
| JSON on usage errors | stderr text only | the same unless we render clap's error into the family envelope (see the family exit and JSON contract proposal, draft #118) |
| Dependencies | none added | about 12 crates (`clap`, `clap_builder`, `clap_lex`, `clap_derive`, `heck`, `anstream`, `anstyle*`, `colorchoice`, `strsim`, ...) plus `clap_complete`; `syn`, `quote` and `proc-macro2` are already in the lock via `serde_derive` |
| Binary size | not measured for this entry; the implementing change measures the release binary before its change | measured by the same change after it, on the same machine |
| Build time | none added | `clap_derive` adds a proc-macro compile to a clean build |

*Assessment.* The verb layer is small, closed and already consistent; clap's
gain there is modest. The gain is in the **per-verb arguments**, where the 11
hand-written usage lines drift from what is parsed, and in completions. The
cost that matters is not size but the exit contract: clap's own exit (2) would
collide with section 3.3's "refused", so any adoption must route every clap
error through `Exit::Usage` and test it for each verb.

*Options.* (a) Adopt clap for per-verb arguments only, keeping `Verb::parse`
and the help-before-resolution rule, mapping every clap error to exit 3 (the help and version displays exit 0), with
a test per verb that a bad flag exits 3 and names the flag; completions via
`clap_complete`. (b) Adopt clap for the whole tree. (c) Keep the hand-written
parser and add a small declarative flag table per verb that the usage lines
and a completion script are generated from. *Recommended, and adopted by the owner on 2026-09-25: (a)*, after the
family exit and JSON contract of draft #118 lands, so the usage-error envelope is decided
once. Measure binary size and clean-build time before and after as the
change's evidence.

**2026-09-25: the provenance fixture follows the adopted producer.** With
spec-spine 0.26.0 adopted (`docs/adoption/spec-spine.md`),
`edited_authored_inputs_are_customized_information_and_an_edited_template_is_drifted`
failed. Its stub executable still reported 0.25.0 while the project it
initializes is pinned to the linked producer. The test is about an executable
that satisfies that pin, so the stub now reports
`statecraft_home::producer::PRODUCER_VERSION` and follows each adoption. The
fixtures that state an older executable on purpose keep their versions.
