<!-- Historical record only. Not normative. -->
<!-- Moved from specs/003-work-and-run-semantics/spec.md at base d77e011e39dbc4689e49492c4e528d860be7eb86. -->

# Archived implementation journal: 003-work-and-run-semantics

This file preserves the chronological implementation record that formerly
occupied section 5 of the active spec. Current requirements live in the
active spec's behavior section, and current rationale lives in its bounded
`Resolved decisions` section. Git remains the authoritative change history.


Dated entries for choices §3 was silent on. None changes what it requires.

**2026-09-16: the chain lives in the product home, not in the target.** §3.3
says one chain per registered repository and does not say where. §3.5 does
constrain it: the refusal count is written where the supervised process cannot
reach it, and that process runs inside a worktree under the target's
`.statecraft/state/`. A chain inside the target would be a chain the thing being
judged can edit, so the chain is in the product home, keyed by a digest of the
target's absolute path. An integration test asserts the chain path is under
neither the target nor the workspace.

**2026-09-17: the correction to section 3.7 removes a wait, not a bound.**
Spec-spine 091 was described here as carried by no release, which the move to the
`=0.20.0` pin falsified. Only the sentence was stale: this section rests on 091
as the future answer to `F-10` and never conditioned the concurrency bound on it,
so nothing it requires changed and no behavior did. The bound stays one live
attempt per repository because its reason was never tool support; it was that
cross-repository scheduling is out of scope for the first slice.

**2026-09-16: two reports are joined, because one does not carry status.**
§3.1.1 turns on a spec's `status`, and `registry plan --json` under 0.18.0
carries only `id` and `title`. `registry list --json` carries `status` and
`implementation`. Both are spec-spine's structured output, so joining them is
still reading rather than deriving. A ready spec absent from the lifecycle
report is excluded with "status is unknown" rather than assumed approved.

**2026-09-17: both reports are envelopes, and only one of them was read as
one.** §3.1 requires the product to parse spec-spine's structured output and
§3.1.1 fixes the join, and neither says what the outer shape of either answer
is. Measured against the pinned spec-spine 0.20.0 on 2026-09-17, against this
repository's own corpus and against a scratch one: `registry plan --json`
answers `{"ready": [...], "blocked": [...], "notSchedulable": N,
"schemaVersion": ...}` and `registry list --json` answers `{"items": [...],
"schemaVersion": ...}`. The plan half was already read through its own key; the
lifecycle half was read as a bare array, so every `work`, `run` and `accept`
invocation against a real corpus exited 4 with "expected an array of specs".
No test caught it because every fixture was a hand-built bare array, which is
the shape the parser wanted rather than the shape spec-spine gives it. The
lifecycle half is now read through `items`, a bare array is still accepted
because an older report that is one carries the same rows, and the tests carry
the measured envelope. Nothing §3 requires changed: this is the same read, of
the same two reports, finally performed on the bytes they actually contain.

**2026-09-17: the version a refusal names is the version.** §3.1 requires a
refusal to name the missing field and the spec-spine version, and
`spec-spine --version` prints `spec-spine 0.20.0`. Keeping the whole line made
every refusal read "spec-spine spec-spine 0.20.0 report ..." and put a program
name inside `specSpineVersion`, which `006` §3.4 makes a contract. The last
whitespace-separated token is taken, which is what this product already does
where it asks spec-spine the same question for the environment manifest's pins.

**2026-09-16: a torn tail is truncated before the next append.** §3.8 requires
recovery to read to the last complete record, report the tear, and append after
it, without rewriting earlier records. The trailing partial bytes are truncated
at the first append after the tear. That is not rewriting a record: those bytes
were never acknowledged, because acknowledgement is what `fsync` and a
terminating newline together mean here. A test asserts the earlier bytes are
identical before and after.

**2026-09-16: an empty chain is not a broken one.** `attest-ledger`'s
`verify_chain` reports `EmptyChain` for an empty slice, correctly for a ledger
that should have an anchor. On a repository's first run there is nothing to link
yet, so verification begins once there is a record. Reported here because it is
a behavior of a reused component this spec depends on, not a local invention.

**2026-09-16: attest-ledger is a pinned git dependency.** It is public and
unpublished, and `001` §3.2 says the record envelope is reused rather than
reimplemented. Vendoring a copy of a hash-linked ledger into the product that
depends on it would defeat the reuse. The workspace is `publish = false` and
`F-02` defers publication, so the usual objection does not apply yet;
un-pinning it, or moving to a released version, is its own change.

**2026-09-19: the identity is a top-level payload key, spelled in camelCase.**
§3.3.1 puts an identity on the record payload, whose existing keys are `kind`,
`run_id`, `attempt`, `subject`, `idempotency_key` and `detail`: Rust field names
serialized as written. Every other JSON this product emits is camelCase,
including the keys inside `detail` (`baseCommit`, `retryAllowed`, `outcome`) and
every CLI view. Two spellings were available: follow the neighbouring payload
keys, or follow the product's JSON contracts. `effectId` is chosen, because this
record is read through the contracts rather than through the struct, and the
serialization is therefore stated explicitly on the field rather than inherited
from the field's Rust name. Placing the identity inside `detail` was rejected:
`detail` is the untyped remainder, and a correlation key the fold depends on is
not a remainder.

**2026-09-19: presence is decided by the key, and an invalid value is a reported
defect rather than a decode failure.** §3.3.1 clause 3 needs three states where
an optional field offers two. An `Option` of the identity type does not give
them: a self-describing format's `null` deserializes to the same `None` as a
missing key, so an explicit `null` would be read as a record that never carried
an identity, which clause 3 forbids. The field is therefore a three-state value
of its own, defaulting to absent when the key is missing and decoding any
present value: a non-empty string is the identity, and anything else is retained
and reported as invalid. Decoding it as a plain string instead would fail
deserialization, and the fold reads payloads through a decoder that today
discards what it cannot decode, so the strictest-looking choice would have been
the one that loses the defect. What this preserves is the **JSON value** an
invalid identity carried, not necessarily the byte sequence that expressed it:
number formatting, string escaping and object key order are the encoder's. That
is sufficient, because a stored record is never rewritten by this product, so
the historical bytes on disk are untouched regardless; and a record emitted with
no identity omits the key and is byte-identical to what this product writes
today. Making the decoder refuse rather than discard is a separate change, in a
separate spec's territory, and is not made here.

**2026-09-22: section 3.1.2, recorded before implementation.** spec-spine
102 puts `status` in the plan report. Section 3.1.2 keeps the join, compares
the two `status` answers rather than preferring one, refuses a disagreement
and refuses a ledger that moved between bracketed reads. No code changed with
this entry.

**2026-09-22: section 3.1.2 implemented.** `report::join` is the pure half:
it takes the version, the plan's bytes and the two `list` answers, refuses
`moved` when the two differ, refuses a plan of two shapes as unreadable,
refuses a disagreement naming both values, and otherwise returns the report
with its `StatusSource`. `SpecSpineCli::corpus_report` reads `list`, `plan`,
`list` after `check` and hands the bytes over unchanged. Each `WorkItem`
carries `status_from`. The tests read the two producers' own recorded answers
(`testdata/producer/`, with their provenance), mutate a copy for the
contradiction, and pair two different recorded `list` answers for the move.
`tests/producer_candidate.rs` is ignored by default and runs the real read
against a binary and revision the operator names, on a scratch copy of this
corpus whose ledger that binary compiled. Measured with it on 2026-09-22:
spec-spine 0.20.0 (`v0.20.0`, `4d14cce6`) reads with `status` from `registry
list` alone, and the unreleased `3b67b63d` reads with the plan agreeing.

**2026-09-22: section 3.1.3, recorded before implementation.** `run` binds
each attempt to one closure the producer resolves, the unit of work's spec and
every obligation it declares, and writes it once into the intent. Every answer
other than a resolved closure is a named state, none of which stops the run.
Measured on the unreleased `3b67b63d`: a spec member alone does not carry the
spec's obligations, so the request names them. No code changed with this
entry.

**2026-09-22: section 3.1.3 implemented.** `contract.rs` builds the request
from the list row's declared obligations (`SpecLifecycle` now reads
`obligations`, empty under a producer that reports none), asks
`registry closure --help` before resolving so an unknown subcommand and a
stale ledger are never read from one exit code, and reads the producer's exit
0, 1 and 2 as resolved, unresolved and stale; anything else is unreadable.
`session::begin_bound` writes the binding into the intent's `detail.contract`
with the intent itself; `begin` is unchanged and writes none. `run` binds after
the eligibility check and before `begin_bound`; the trial of spec 002 section
3.33 binds `not-a-unit-of-work`. Measured with the ignored named-producer tests
on 2026-09-22: the unreleased `3b67b63d` binds a closure for
`002-environment-lifecycle` on a scratch copy of this corpus it compiled, and
resolving it again answers the same digest; the pinned 0.20.0 is
`unsupported`, naming itself. A third ignored test replays spec-spine 103's
portable verifier fixtures through a named build's `verify-attestation
--recompute`: `3b67b63d` reproduces all 11 cases from its own revision, and
0.20.0 reproduces 10, refusing the control as a version mismatch, which is
what fixtures bound to the tool that produced them should do. That test is
adoption evidence for a producer build; nothing in this product verifies these
attestations, and no verifier is built here to give the fixtures something to
test.

**2026-09-23: `check`'s exit status is carried into the refusal, not folded
into "does not compile".** Section 3.1.2 rule 3 reads the reports only after
`check` has said the ledger is fresh. The report source treated every non-zero
`check` as the section 3.8 row "a corpus that does not compile". The 2026-09-23
audit measured what that hides. Run with a `spec-spine` on `PATH` that did not
satisfy this repository's `=0.20.0` pin, `work list` said the corpus did not
compile, when the producer had refused the pin with exit 3 and judged nothing.
The section was silent on which answer is which, so this records the reading.
`check` exiting 1 is still `CorpusDoesNotCompile`, and the row is unchanged.
Exiting 2 is `LedgerStale`, the rule's freshness precondition unmet. Any other
end is `ProducerRefused`, naming the version, the exit status and the
producer's first line. Nothing is read in any of the three cases, and the
report never falls back to the derived tree. Which binary is resolved is
unchanged: the operator's `spec-spine`, never one the candidate chose, with
the target's pin enforced by the producer itself.

**2026-09-23: the pinned producer now carries both reports' `status` and
resolves closures.** The CLI pin moved to `=0.23.0` (decisions `D-06`, entry
of this date). Sections 3.1.2 and 3.1.3 were written against a pin that
carried neither, and each says so as a dated measurement, which stays as
written. Measured under the new pin, through the CLI, on this corpus:

- `registry plan --json` is read schema 0.7.0, and its one ready row carries
  `status`, which the join compares with `registry list` and finds agreeing.
- `registry closure` for `002-environment-lifecycle` resolves to one member
  and one digest.

So rule 2's comparison and section 3.1.3's binding are live against the pinned
producer, not only against a named candidate build. No rule changes. Section
3.1.2 rule 4's released shape without `status` stays readable, because a
target may pin an older producer.

**2026-09-23: the published producer's own answers are recorded, and the
fixture replay asserts outcomes, not only exit codes.**
`testdata/producer/released-0.23.0/` holds what the pinned 0.23.0 answered on
this corpus: `registry plan` and `registry list`, and two `registry closure`
answers, one resolved and one refusing a missing member. Until now every
closure this crate read was written by hand in the measured shape. `report.rs`
and `contract.rs` test the join and `interpret` against those bytes. The older
recorded sets stay, as the evidence they were.

The ignored `producer_candidate` fixture test now checks, per case:

- the exit code;
- the verifier's `ok`;
- the recompute outcome its recorded reason names (`match`,
  `contentMismatch`, where `non-canonical-bytes` is reported as a content
  mismatch, or `versionMismatch`);
- that a refusal before the recompute carries a structured error.

The verifier reports only an error kind for that last case, so no message
text is parsed. Run against the published build and the fixture set shipped
inside the published `spec-spine-core` 0.23.0 crate, all eleven cases
reproduce. Run against the 0.20.0 binary with the same set, four do not:
`control-untampered`, `flipped-verdict`, `minor-ahead-content-mismatch` and
`reformatted-same-values`, each read as a version mismatch where the set
expects `match` or `contentMismatch`. The same run binds `unsupported`, because 0.20.0 has no
closure verb, and reads `status` from the list alone. So an older producer is
refused by name where it lacks a capability, and nothing about it is read as
the newer contract.

**2026-09-23: section 3.1.4 implemented.** The journal is
`statecraft_run::overrides`, one file per repository at
`records/<key>.overrides.jsonl` in the product home beside the run record, and
the lock is `statecraft_run::lock`, `records/<key>.lock`, taken with a
non-blocking advisory `flock` through `rustix`, which the workspace already
carried through `tempfile` (the standard library's `File::try_lock` is newer
than the declared `rust-version`). `run` holds it from before the
intent until the process returns; `override grant` and `override revoke` take
it and refuse while it is held. `Override` gains three optional members, the
provenance word, the grant's time and the grant line's digest, serialized only
when present, so an override built in memory and one written before this
entry read as before. The intent's `admission` member is written by
`session::begin_admitted`, and `runs()` reads it back into `Attempt.admission`,
which `run list` and `run show` render; an intent without it is rendered
"admission not recorded". **One residual, measured while testing, that rule 3
names only in part:** editing the **last** line leaves nothing after it whose
link would notice, exactly as removing it does, so the operator or reason of
the most recent grant can be changed undetected. The binary test edits a line
that has a successor, which is refused as a broken journal. Tests:
`overrides` and `lock` unit tests, and `crates/statecraft-cli/tests/readiness_override.rs`
through the binary with a fake `spec-spine` offering a `draft` and a fake
provider replaying a recorded stream.

**2026-09-23: section 3.6.1 implemented.** `statecraft_run::reconcile` decides
and appends; the binding reads spec `002`'s launch records through
`launch::inspect`, which only reads, and passes the launch state, whether
`gate.log` holds an `admitted` line, the confirmed process id and whether a
process with that id exists (`kill(pid, 0)` through `rustix`, an observation
only) as facts. `session::runs` reads a reconciliation written under the
section: a conclusive one sets the attempt's outcome to `interrupted` and keeps
the reconciliation beside it, an `unknown` one leaves it live, and an older
shape is ignored. `session::begin_admitted` writes `follows` into the next
intent. The verb takes the repository lock of section 3.1.4 rule 7. Tests
through the binary, in `run_startup.rs`, drive real crash boundaries with the
fake provider: a launcher killed after admission (`outcome-unknown`, with and
without a released tool call in the gate log) and an intent with no launch
intent (`not-launched`); they check stale, conflicting, usage, unreadable
evidence, lock, `unknown` then `absent`, a second reconciliation, `follows`,
and that no launch is replayed. Unit tests cover the older shape and
`unrecorded`.

**2026-09-23: section 3.6.1, corrections from review.** Five choices the
section was silent on, fixed without changing what it requires. The attempt's
`gate.log` is read for rule 3 whatever the manifest says, so a repository with
no manifest cannot bypass the conflict, and a log that exists and cannot be
read fails the reconciliation (exit 4) rather than reading as "nothing
released"; its path is always among the files the observation read. The
launch state is mapped from spec `002`'s verdict exhaustively, with no
fallback word. `replaces` is the record's index in the chain
(`Chain::positioned_entries`), not an index into the decoded entries. Evidence
files are hashed as they are read, never held whole. Rule 4's "with the
reconciliation beside it" is carried by `run list`, and `run`'s refusal names
the live attempt's `unknown` reconciliation (spec `006` section 5). Tests
through the binary in `run_startup.rs`: a reconciliation while a real `run` is
blocked is refused and leaves the chain byte for byte; `absent` against
`launch-unknown` is declared only and releases; `confirmed` releases and the
next intent's `follows` names it; a concluded attempt, an attempt the record
does not carry and an empty operator or reason are refused; a released tool
call refuses `absent` with no manifest; an unreadable gate log fails and
writes nothing. A drop guard releases a blocked fake provider however a test
ends.

**2026-09-23: one repository key for the lock, the run record and the
journal.** Rule 7 is one lock per repository, rule 4 never reads one
repository's override for another and so implies one journal per repository,
and section 3.3 is one chain per registered repository. None of them says what
identifies the repository when its path can be typed more than one way. The
register compares paths component-wise, so `<root>`, `<root>/` and `<root>/.`
are one registration, while the lock, the chain and the journal were each keyed
by a digest of the bytes the operator typed. Measured by an independent review
and reproduced live: during a supervised attempt, `run <root>/` took a second
lock, passed the live-attempt refusal and began attempt 1 in a second chain.
The binary tests reproduce it on the previous build: an override granted
through one spelling is not seen through another, and a chain written under one
is read as no runs under another.

The choices, where the section is silent:

- **The key is the registration's stored root, as the register holds it.** The
  binding resolves a typed path through the register, refuses it if it is not
  registered, and passes the stored root on; `statecraft_run::repository::key`
  digests it exactly as the previous keys were digested. A home whose records
  were written through the registered spelling therefore keeps every key, and a
  canonicalized key was rejected because it would re-key exactly those homes
  (a temporary directory on macOS is `/var/...` as typed and `/private/var/...`
  canonical).
- **A symbolic link is not resolved.** A link to a registered root that is not
  itself registered stays unregistered and is refused, as it was, writing
  nothing; spec `006`'s decision of 2026-09-16 not to canonicalize a typed path
  stands. A link that is **also** registered makes two stored roots for one
  directory. Choosing either would leave the other a second lock, so every
  verb that keys a record here refuses both, exit 2, naming the roots, until one
  registration names the directory. The comparison is the directory's device
  and inode.
- **A re-registration keeps the root as first stored.** `project register
  <root>/` finds `<root>`, and replacing the stored spelling would re-key the
  repository's history. Arming is already preserved across a re-registration
  for the same reason.
- **Records filed under another spelling are never read as absent and never
  merged.** A home written by the previous build can hold a chain or a journal
  keyed by a spelling the register equates with the stored root. Its first
  record links to an anchor derived from that spelling, so it cannot be moved
  under the stored key without breaking its links, and reading the stored key's
  empty chain beside it would say "no history". So opening the chain or reading
  the journal checks the spellings an operator produces for the same
  registration (the root with and without a trailing separator, with a trailing
  `.`, and the forms making a relative `.` or `./` absolute produces), and when
  one has a file it fails, exit 4, naming the file and the spelling, having read
  and written nothing. A file counts only when it holds at least one byte and
  is a run record or an override journal: a lock file, and an empty file of
  either kind, carry no history, and both readers already read an empty file
  as absent. The failure names the remedy that applies (next item).
- **An explicit `project register` re-stores the spelling in exactly one
  case.** The previous build replaced the stored spelling on a
  re-registration, so a home registered as `/x/p`, run as `/x/p` and
  registered again as `/x/p/` has its history under `/x/p` and `/x/p/` stored,
  and every record verb fails on it. `project register <typed>` re-stores the
  registration under `<typed>` when the register already holds that path under
  another spelling, nothing that carries history is filed under the stored
  spelling, and a run record or journal is filed under exactly `<typed>`. That
  moves the key onto the only history the repository has, loses nothing and
  merges nothing, and it is an operator's explicit act; it is done under the
  stored root's lock and the answer says it re-stored. In every other case a
  re-registration keeps the stored spelling, as above. Where both spellings,
  or more than one other spelling, carry history, they are separate histories
  and nothing re-stores or merges them: the failure says so and names the one
  way on, which is the operator moving the other spelling's files out of the
  records directory, where this product never reads them. No requirement
  changes: which spelling the register holds was already this binding's
  choice, and the register gains only the operation that sets it.

*What this does not detect, named.* The set of spellings the register equates is
unbounded (`/x//p`, `/x/./p`), and the check covers the ones listed. A home
whose previous build was driven through another such spelling holds a file this
check does not find. Spec `002`'s local approval records, keyed by the typed
project path, are not per-repository records of this spec and are unchanged.

*What section 3.1.5 must add, when it is implemented.* The journal's state
authority (rule 2) and the file `adopt-prefix` preserves beside the journal
(rule 5) are per-repository records in the protected records directory. Each
must be named by `statecraft_run::repository::key` from the stored root, never
from a typed path, and must join the check above: listed in
`repository::HISTORY_SUFFIXES`, so a copy filed under another spelling fails
rather than reading as "neither file" (rule 2's "never had an override"), and
counted as history under the stored key when deciding whether a spelling may
be re-stored. A TODO in `repository.rs` marks the place.

**2026-09-23: a released repository lock no longer outlives its holder.** The
`overrides` and `lock` unit tests failed intermittently with "another process
holds this repository's lock" on a lock file no other test used. Measured on
macOS with `cargo test -p statecraft-run --lib` at the default thread count, on
the previous build: 1 failing run of 40 in one sample and 30 of 100 in a second,
across five tests. On this build: 0 of 100, twice. A `flock` belongs to the
open file description, and closing a descriptor releases it only when no other
descriptor refers to that description. A process spawned from another thread
holds a copy of every descriptor, close-on-exec ones included, from its creation
until its `exec`. Measured with 20000 take-and-close cycles while four threads
spawned `/usr/bin/true`: 439 were refused when spawning through `posix_spawn`,
and 2354 to 2621 when spawning through `fork` (a `pre_exec` hook or a `PATH`
lookup under a changed environment); none with no spawning. Releasing
explicitly (`flock(LOCK_UN)`) before closing gave 0 in every mode. The lock now
releases explicitly when dropped, so rule 7's lock is released by its holder
for every copy at once, and the operating system still releases it when the
holding process ends, with one bounded exception: a holder that ends without
dropping it (killed, say) leaves the lock held by a child it had just spawned
until that child's `exec` closes its copy, because every descriptor this
product opens is close-on-exec and no executed program keeps one. Linux's open-file-description locks were considered and
rejected: they belong to the description too, so a child's copy would hold them
the same way. The tests were not serialized and nothing retries.

**2026-09-23: PROPOSED, NOT ADOPTED. When a run ends, what base a later attempt
uses, and which bundle and harness it follows.** This entry binds nothing and
changes no behavior. The question is stated, with its measurements, as F7 of
spec `002` section 5's proposed entry of the same date ("One qualified release
bundle ..."), and the two options are its decision H-5: (A) a run's base
commit, bundle and harness requirement are resolved at its first attempt and
reused by every later attempt, and a run ends when an attempt concludes
`completed` and its acceptance is recorded, or when the operator ends it,
which releases the workspace and makes the next `run <spec>` a new run with an
ordinal; or (B) each attempt resolves its own base and the run's workspace is
moved to it, keeping the previous branch, and a run never ends. The proposal
prefers (A), and on 2026-09-24 the owner selected (A) as the design direction,
which adopts nothing. Until the owner adopts it in this spec's own change, the
established behavior stands: the run
id is the spec id (spec `006` section 5, 2026-09-17), a run never ends, and a
retry after the target's `HEAD` moved concludes `interrupted`. The proposal is
kept in one place so that this spec and spec `002` do not carry competing
versions of it. Spec `002`'s entry was adopted as its Part 9 step 1 on
2026-09-24; that adoption does not reach this spec, whose part (Part 9 step
4, H-5) is still not adopted here.

**2026-09-25: let chains collapsed with the measured rust-version floor.**
The workspace floor moved from 1.85, which never built, to 1.88 (evidence in
spec `002` section 5, same date). Clippy's `collapsible_if` then applies let
chains, and the nested `if` blocks it named in this spec's crates were
collapsed mechanically by `cargo clippy --fix` and `cargo fmt`. No behavior
changed.

**2026-09-25: attest-ledger is the crates.io release `=0.1.0`, not the git
revision (owner, 2026-09-25).** `attest-ledger-core` and `attest-ledger-types`
0.1.0 are published on crates.io, so the move the 2026-09-16 entry left as its
own change is made. Diffed before switching: both published crates record
their source commit as `23803ab` (`.cargo_vcs_info.json`), the parent of the
pinned `a9c3595`. Every file of `crates/core` and `crates/types` at `a9c3595`
is byte-identical to the published package (each crate's `Cargo.toml.orig`
included), except that the package adds the workspace `README.md`. The whole of
`23803ab..a9c3595` is metadata: the workspace `repository` and `homepage` move
from `stagecraft-ing` to `statecrafting`, one line each in `README.md`,
`CHANGELOG.md` and the bootstrap spec, and editor files. So the published
crates carry the old organisation in their `repository` field, and the code this
crate links is unchanged. The pin is exact (`=0.1.0`) because the record hash
is this crate's persisted format; the lock records the registry checksums.
The comparison is reproducible without trusting the published metadata: fetch
`https://crates.io/api/v1/crates/<crate>/0.1.0/download` for both crates and
unpack them, `git archive a9c3595` from `statecrafting/attest-ledger`, then
`diff -r` each of `crates/core` and `crates/types` against its package,
excluding only the files `cargo package` generates (`Cargo.toml`,
`Cargo.toml.orig` compared separately, `.cargo_vcs_info.json`, `Cargo.lock`).
Measured on 2026-09-25: no difference except the added `README.md`. The
`.cargo_vcs_info.json` commit is corroborating, not the evidence; the evidence
is the file comparison against the pinned revision itself.

**2026-09-25: the candidate test names its binary with
`STATECRAFT_SPEC_SPINE`.** Spec `002` section 5's entry of the same day,
adopted by the owner, makes `STATECRAFT_SPEC_SPINE` the one variable that
selects a spec-spine binary and retires the others. The ignored
`tests/producer_candidate.rs` read its binary from `STATECRAFT_PRODUCER_BIN`
and now reads it from `STATECRAFT_SPEC_SPINE`; `STATECRAFT_PRODUCER_REV` and
`STATECRAFT_PRODUCER_FIXTURES` stay, because they name a revision and a
fixture directory, not a binary. What the test checks, and that it is ignored
by default, are unchanged.

**2026-09-25: the contract binding and the report read both of spec-spine's
exit tables (owner, 2026-09-25: adopt 0.26.0).** Section 3.1.3's `stale` row
and the report's reading of a `check` that did not pass name the producer's codes as every release
below 0.26.0 spends them: stale 2, a pin not met 3. From spec-spine 0.26.0
(its spec 132) stale is 1, beside a member that does not resolve, and a refusal
to judge is 2; measured the same day against both published releases. A target
may pin either, and the code alone cannot say which table answered, so the
producer's words decide, as spec `002` section 5 records for `check` on this
date: "index is stale" or a `STALE` report line with nothing else is stale,
`refused:` or a pin not met is a refusal. Each state keeps its meaning: `stale`
is still a stale ledger, `unresolved` a member that does not resolve, and a
refusal is `unreadable` in the binding and `ProducerRefused` in the report.
`contract::interpret` and `report::check_refusal` carry it, tested against the
recorded lines of both releases in `each_exit_code_is_its_own_answer` and
`a_check_refusal_is_read_under_both_exit_tables`.

**2026-09-25: a configuration or containment refusal is not a stale ledger
(owner, 2026-09-25: adopt 0.27.0).** The entry above reads a refusal by
`refused:` or a pin not met. Measured the same day, invalid configuration exits
2 under both 0.26.0 and 0.27.0 with `spec-spine: config error:`, and 0.27.0 adds
a link leaving the repository (`refused:`) and a layout root that is not a plain
relative path (`config error:`), both 2 (spec-spine's 144). Both readers took
such a 2 as a stale ledger. They now share spec `002`'s widened
`names_refusal`, so each is `unreadable` in the binding and `ProducerRefused`
in the report; and `validation failed` at 1, 0.27.0's word for an unresolved
claim at a guarded reader (spec-spine's 145), is never stale. Each state keeps
its meaning. Tested with the recorded lines in
`each_exit_code_is_its_own_answer` and
`a_check_refusal_is_read_under_both_exit_tables`.
