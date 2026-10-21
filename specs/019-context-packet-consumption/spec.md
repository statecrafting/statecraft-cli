---
id: "019-context-packet-consumption"
title: "Immutable context-packet consumption across repository snapshots"
status: approved
implementation: complete
created: "2026-09-27"
summary: >
  Amends Statecraft's run, adapter, acceptance, command, and governance-producer
  contracts so one task may bind immutable spec-spine context packets from
  several independently identified repository snapshots, record intended and
  provider-visible delivery separately, and judge packet integrity, closure,
  freshness, citations, and evidence without reimplementing producer-owned
  selectors, closures, or digests.
amends:
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "009-governance-producer"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter-claude-code/" }, nature: additive }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "009-governance-producer"
obligations:
  - id: "I-1"
    kind: invariant
    text: "Each repository packet remains bound to its own immutable snapshot, and their composition never claims one atomic cross-repository Git state."
    anchor: "3-2-one-packet-one-snapshot-no-distributed-transaction"
  - id: "I-2"
    kind: invariant
    text: "Statecraft consumes producer-reported selector, closure, continuation, and digest facts without reconstructing their algorithms."
    anchor: "3-3-the-producer-boundary"
  - id: "R-1"
    kind: requirement
    text: "An attempt records one immutable multi-repository task manifest before execution and separately records intended delivery and provider-visible delivery."
    anchor: "3-5-intended-and-provider-visible-delivery"
  - id: "R-2"
    kind: requirement
    text: "Acceptance reports packet integrity, closure, freshness, citation resolution, and evidence admission as independent checks whose unknown or failed states never become success."
    anchor: "3-7-five-independent-acceptance-checks"
  - id: "V-1"
    kind: verification
    text: "Portable fixtures prove snapshot separation, immutable manifest identity, delivery separation, producer-owned resolution, and independent acceptance outcomes without live provider activity."
    anchor: "verification"
    inputs:
      - "crates/statecraft-run/tests/context_manifest.rs"
      - "crates/statecraft-adapter/tests/context_delivery.rs"
      - "crates/statecraft-acceptance/tests/context_packet_acceptance.rs"
---

# 019: Immutable context-packet consumption across repository snapshots

## 1. Purpose

Spec 003 binds an attempt to the governed contract for one unit of work. It
does not identify the repository content an agent may need from that repository
or from related repositories. Passing ambient checkout paths or copied prose in
a prompt would make the effective inputs mutable, leave omissions invisible,
and invite Statecraft to invent its own selection and closure rules.

The proposed spec-spine contracts for selected content, repository-scoped
context packets, and documentation freshness supply the missing producer facts.
They deliberately stop at one repository snapshot. Statecraft owns the next
boundary: bind several immutable packet identities to one task, deliver them to
one attempt, preserve what was intended separately from what a provider made
visible, and judge their later use without claiming a distributed Git
transaction.

This amendment fixes that consumer contract. It does not authorize
implementation against an unreleased producer. It also does not define a
documentation task, a documentation result, or documentation acceptance. Those
remain later contracts.

## 2. Territory and producer prerequisite

This spec owns no product code. A later implementation may change only the
existing units named by its `extends` edges:

- `statecraft-home` adopts the exact released producer interface;
- `statecraft-run` owns the task manifest, its immutable intent binding, and
  the durable delivery observations;
- `statecraft-adapter` carries the manifest through its provider-neutral
  request and event protocol, and `statecraft-adapter-claude-code` attaches
  its delivery testimony to the provider's init event (section 5,
  2026-10-21);
- `statecraft-acceptance` performs the independent checks; and
- `statecraft-cli` projects the added facts through the existing `run` and
  `accept` commands without becoming a second implementation.

The design input is spec-spine commit
`8451ade3823e7e0f18809a4968270387a0b1586a`, signed and carrying draft specs
then numbered 155, 156, and 157 in tree
`5f411d3cb20ab63d5453cab291b39b926a2b431e`; the producer's current numbers
are 155, 159 and 160 (section 5, 2026-10-21).
That identity is planning evidence only. The contracts are draft, pending, and
not released. Implementation is blocked until one exact released spec-spine
identity implements their required packet and freshness interfaces and this
repository adopts that identity under spec 009. A filesystem checkout, Git
branch, moving tag, or compatible-looking JSON is not an admissible substitute.

## 3. Behavior

### 3.1 The multi-repository task manifest

Before an attempt's effect, Statecraft writes one closed
`statecraft/context-task-manifest/1` document into the spec 003 intent. Its
top-level members are:

1. `schema`, exactly `statecraft/context-task-manifest/1`;
2. the exact run id and positive attempt number;
3. the task identity supplied by the later task contract, or explicit
   `not-recorded` until that contract exists;
4. the exact adopted producer identity and supported packet interface version;
5. an ordered nonempty `repositories` collection;
6. a closed delivery policy from section 3.5; and
7. `manifestDigest`, computed over the producer packet bytes and this
   Statecraft-owned composition document by the named Statecraft construction.

Each repository entry contains one stable local repository id, its role in the
task, `required` or `optional`, the producer-reported repository, revision,
tree, and dirty-state identity, the packet schema version, request digest,
closure digest, ordered page digests, terminal continuation state, completeness,
and the exact packet-byte references Statecraft retained. Repository ids are
unique. One packet page belongs to exactly one entry and appears exactly once.

The manifest includes the producer's omissions and warnings by reference to the
retained packet bytes. It never replaces them with a summary that could hide a
required omission. Every collection is explicit, including an empty omission
or warning collection.

The manifest digest uses a versioned Statecraft construction that includes the
ordered repository entries, delivery policy, exact packet byte lengths, and
their producer-reported page digests. It identifies this composition only. It
does not replace or recompute any producer digest and does not claim that all
packet bytes came from one transaction.

### 3.2 One packet, one snapshot, no distributed transaction

Each packet remains a fact about exactly the repository snapshot the producer
reports. Statecraft may compose packets whose revisions, trees, dirty states,
or observation times differ. It MUST preserve each identity separately and
MUST NOT mint a shared revision, shared tree, shared cleanliness fact, or
atomic-snapshot label.

The composition may say only that these exact packet identities were selected
for one task manifest. It cannot establish that the repositories were mutually
compatible, observed simultaneously, or unchanged before or after either
packet was assembled. Any compatibility assertion belongs to a separately
identified policy and is judged as its own fact.

A required repository with an incomplete terminal packet makes the manifest
ineligible for execution. An optional repository with a recorded optional
omission may remain eligible when the delivery policy permits it. No omission
is erased, downgraded, or filled from another checkout.

### 3.3 The producer boundary

Statecraft asks the adopted spec-spine producer for packet and freshness facts
through its supported typed library interface. If the adopted release exposes
only a supported CLI interface for one of those reads, Statecraft may invoke the
exact pinned executable and parse its versioned structured output as spec 003
already requires. It never reads `.statecraft/derived` directly.

The producer owns and Statecraft does not reimplement:

- selected-content selector normalization and projection;
- closure resolution, membership, and closure digests;
- packet ordering, deduplication, budgets, omissions, continuation, canonical
  packet bytes, and packet digests;
- snapshot binding and stale-snapshot detection; and
- documentation-manifest freshness when a later documentation contract supplies
  such a manifest.

Statecraft validates that retained bytes decode under the exact adopted schema
and that the producer-reported identities repeat consistently. It does not
recompute an expected closure, infer a missing selector, aggregate packet pages
into a new producer identity, or implement a fallback digest. Unsupported,
unreadable, unresolved, stale, incomplete, and producer-mismatched answers keep
those separate meanings and never become an empty packet.

### 3.4 Immutable custody and bounded handling

Statecraft retains the exact packet response bytes accepted into the task
manifest, with byte length, source operation, producer identity, and a
Statecraft custody digest over those bytes. The retained bytes are immutable:
retry creates another manifest and never updates an earlier one.

Packets are untrusted repository content. Before decoding, Statecraft applies
bounded total bytes, page count, member count, nesting depth, and string length.
It does not execute packet content, follow a path or URL found in it, expand an
embedded command, fetch a missing repository, or treat content as instruction
authority. A packet may inform a prompt only through the explicit delivery
policy.

The Statecraft custody digest proves only which bytes Statecraft retained. It
does not validate the producer's packet digest, establish trust in repository
content, or make the packet evidence. Those are separate checks.

### 3.5 Intended and provider-visible delivery

The task manifest carries an intended-delivery plan before spawn. For every
repository entry and packet page it says `required`, `optional`, or `withheld`,
plus the delivery channel and ordered identity. Version 1 supports one channel:
an exact manifest and packet bundle supplied through the adapter request as
bytes, never interpolated into command-line arguments.

The adapter request carries the complete manifest bytes, its digest, and the
bounded packet bundle separately from prompt bytes. The init event repeats the
manifest digest the adapter says it applied and lists each page digest it says
it made provider-visible. This is `adapter-observed` testimony. It is not
independent proof that the provider received, retained, parsed, or used the
bytes.

Provider-visible delivery has one state per intended page:

| State | Meaning |
|---|---|
| `reported-visible` | The trusted adapter event reports the exact page digest as made visible to the provider. |
| `reported-withheld` | The adapter reports that it withheld the page, with a reason. |
| `not-reported` | No trusted event accounts for the page. |
| `contradictory` | Events report incompatible states or another digest. |
| `unreadable` | The event stream cannot be decoded far enough to decide. |

The supervisor records intended delivery and provider-visible delivery as two
collections. It never copies the intended set into the observed set. A later
wire artifact admitted under spec 014 may add an independent transport fact,
but it never rewrites the adapter observation. An absent wire artifact leaves
transport `not-recorded`.

### 3.6 Durable attempt binding and retry

The intent records the complete manifest before the adapter process is spawned.
The manifest binds the run id, attempt, adopted producer identity, packet bytes,
delivery policy, and manifest digest. A mismatch between the intent and adapter
request refuses before spawn.

Delivery observations are appended as outcomes correlated by spec 003's effect
identity. A result or observation for another run, attempt, effect, manifest
digest, producer identity, or page digest does not bind. Duplicate or
contradictory terminal delivery observations are a finding and never collapse
to the more favorable one.

A retry creates a new attempt and a new task manifest, even when every packet
identity is unchanged. It may reuse retained immutable packet bytes only after
the acceptance-time freshness operation reports their current status. Reuse
does not update, relabel, or erase the earlier attempt.

### 3.7 Five independent acceptance checks

Acceptance evaluates five dimensions separately and returns every result. No
dimension is inferred from another, and a passing suite does not hide a failed
or unknown dimension.

#### 3.7.1 Packet integrity

For every page, Statecraft requires the exact retained bytes, supported schema,
declared byte length, producer identity, snapshot identity, request digest,
closure digest, page digest, completeness, omissions, and continuation fields.
It asks the adopted producer's supported validation operation to validate the
packet digest and page chain. Statecraft's custody digest is compared only with
the retained bytes and is reported separately.

The result is `pass`, `fail`, `unsupported`, `unreadable`, or `not-recorded`.
Only `pass` satisfies a required packet policy.

#### 3.7.2 Closure

Statecraft submits the recorded closure request and exact snapshot identity to
the adopted producer, then compares the producer's current answer with the
recorded closure digest and members. The producer decides membership and
digest. Statecraft reports `current`, `changed`, `missing`, `withdrawn`,
`stale`, `unsupported`, `unreadable`, or `not-recorded`, with every
producer-reported member difference.

This check does not merge the closures of several repositories. The manifest
contains one result per repository and an aggregate `current` only when every
required repository is `current`.

#### 3.7.3 Freshness

Freshness is evaluated at acceptance time against a caller-supplied stable,
clean current snapshot for each repository. The adopted producer resolves that
snapshot through the same supported snapshot-binding contract that assembled
the packet. The check performs no clone, fetch, checkout, pull, or remote
discovery. An absent, dirty, moving, unsupported, or unidentifiable current
snapshot is `unverified` for that repository.

A raw context packet is `fresh` only when the producer-reported current
repository identity and tree equal the packet's recorded repository identity
and tree, the current snapshot is clean, and the packet is complete under its
recorded requirement policy. A different tree is `stale` even if some selected
members happen to have equal bytes: proving that a declared dependency slice is
unchanged requires the later documentation-manifest contract. Statecraft does
not recompute a tree, selector, content digest, or packet digest to reach this
answer. It compares producer-reported identities and never infers freshness from
equal branch names, timestamps, or a clean working tree alone.

When a later documentation contract supplies a spine spec 160 documentation
manifest, Statecraft
records that producer's narrower typed freshness result for the manifest. It
does not generalize that result to the raw packet or to undeclared inputs.

The result per repository is `fresh`, `stale`, `incomplete`, or `unverified`.
Cross-repository freshness is `fresh` only when every required repository is
fresh, with every per-repository result retained. It still does not mean the
snapshots were atomic, compatible, correct, or accepted.

#### 3.7.4 Citations

This spec defines only the consumer check, not the later result schema. A caller
may supply an ordered set of claimed citations, each naming one repository id,
packet page digest, canonical producer member identity, content digest, and an
optional bounded span. Statecraft resolves the identity only through the
retained packet and the producer's supported selected-content validation.

Each citation is `resolved`, `missing`, `digest-mismatch`, `outside-packet`,
`unsupported`, `unreadable`, or `not-recorded`. Equal text at another identity
does not resolve a citation. A resolved citation proves that the exact packet
member was cited. It does not establish that the citation supports a claim,
that the claim is true, or that the provider used the member.

The later documentation task and result contract decides which claims require
citations and how claim-to-citation relationships are represented. This spec
does not preempt that decision.

#### 3.7.5 Evidence

Evidence remains governed by spec 005 and any separately ratified evidence-type
amendment. Acceptance verifies and admits each supplied evidence reference
under its own policy, dimensions, trust roots, subject binding, and absence
vocabulary. A packet is not evidence merely because it has a digest. A provider
statement that it saw or used a packet is testimony, not transport evidence.

The context result reports evidence admission as `admit`, `refuse`, or one of
spec 005's explicit absence states. It does not translate packet integrity,
closure, freshness, citation resolution, adapter testimony, or suite success
into evidence admission.

### 3.8 Acceptance eligibility and disposition

The exact task policy decides which repository roles, delivery observations,
packet checks, freshness states, citations, and evidence types are required.
The policy is recorded in the intent before execution. A result cannot weaken
it, and acceptance never substitutes a newer policy.

For a required fact, `unsupported`, `unreadable`, `unverified`, `unknown`,
`not-recorded`, a mismatch, or a negative result makes acceptance ineligible
with a stable reason naming the dimension and repository or citation. For an
optional fact, the state remains visible and may coexist with acceptance only
when the recorded policy permits that exact absence.

Acceptance still judges the candidate and authority set under spec 005. These
context checks are additional inputs to that judgment, not a replacement suite
and not a publication permission. A failed context check does not rewrite the
attempt's execution outcome.

### 3.9 Command projections

The existing `run show` and `accept` JSON documents add one
`contextManifest` member. Historical records without it render
`not-recorded`; their bytes are not rewritten.

`run show` folds only the durable record. It shows the manifest identity, each
repository snapshot and packet identity, intended delivery, adapter-observed
provider-visible delivery, and any admitted wire observation. It performs no
producer operation, repository read, or acceptance work.

`accept` shows the same recorded identities plus every result from section 3.7,
the exact policy used, eligibility, and reason. Human and JSON output are two
renderings of one typed value. The command binding performs no selector,
closure, digest, freshness, citation, or evidence algorithm of its own.

### 3.10 Observable negative cases

| Case | Required result |
|---|---|
| Two packets name different repository revisions | Both identities remain; the manifest makes no shared-snapshot claim. |
| One required packet has a required omission | Execution is ineligible and the producer omission remains visible. |
| Retained bytes differ from their custody digest | Packet integrity fails before semantic use. |
| Producer page validation is unavailable | Packet integrity is `unsupported`, never locally reconstructed. |
| Intended pages are absent from adapter events | Provider-visible delivery is `not-reported`; the intended set is unchanged. |
| The adapter reports another manifest digest | The observation is `contradictory` and does not bind to the attempt. |
| One current checkout is dirty | That repository's freshness is `unverified`; no other repository result hides it. |
| A citation finds equal text under another member identity | `outside-packet` or `digest-mismatch`, never `resolved`. |
| A provider claims it used every packet | Provider testimony remains separate from transport evidence and citation resolution. |
| A retry uses the same packet pages | A new immutable manifest is written for the new attempt. |

## 4. Out of scope

- Implementing or publishing spec-spine specs 155, 159, or 160.
- Adopting an unreleased producer, a filesystem dependency, or a moving Git
  identity.
- Defining documentation task, result, claim, contradiction, lifecycle-grade,
  review, publication, or semantic-support schemas.
- Claiming one atomic snapshot, compatibility, or transaction across Git
  repositories.
- Fetching, cloning, updating, or cleaning a repository during packet assembly
  or acceptance.
- Treating packet content, adapter testimony, a digest, or a citation as
  authority, evidence admission, correctness, acceptance, or publication.
- Live provider activity, credentials, spend, remote writes, release, or
  deployment.

## 5. Resolved decisions

**2026-09-27: the manifest composes identities, not repositories.** One packet
remains bound to one producer-reported snapshot. The Statecraft digest names
only the ordered composition and cannot imply atomicity the producer does not
provide.

**2026-09-27: intended and provider-visible delivery are distinct durable
facts.** Copying the requested page set into an observed field would convert
configuration into evidence. The adapter may testify to visibility, while an
independent wire artifact may later add transport evidence; neither rewrites the
other.

**2026-09-27: citation resolution stops before semantic support.** Packet
membership and content identity can be checked mechanically. Whether cited
content supports a result claim belongs to the later documentation result and
acceptance contracts.

**2026-10-21: the producer prerequisite is met, and the citations are
re-read.** Spec-spine 0.31.0 implements the three contracts section 2 waited
for, and this repository adopted it under spec 009 (`D-06`, pin `=0.31.0` in
`spec-spine.toml` and the root `Cargo.toml`). The producer renumbered them:
selected content stays spine spec 155, the repository-scoped context packet
is now spine spec 159, and documentation manifests and their freshness are
now spine spec 160. Its current 156 and 157 name unrelated documents, so this
spec's earlier "156" and "157" are read as 159 and 160 and are corrected in
sections 2, 3.7.3 and 4; no requirement changes.

**2026-10-21: the run record's half is pure over producer-reported facts.**
`statecraft-home` is the one crate that links the producer, so it decodes a
page under the adopted schema and hands `statecraft-run` the page's exact
bytes with the facts the page reports. `statecraft-run` applies section
3.4's bounds to the bytes first (16 MiB and 256 pages per manifest; per page
262,144 members, depth 64 and 4 MiB per string), then requires each
repository's pages to repeat their schema, build, snapshot, request digest and
closure digest, to name the adopted package and version, and to end at one
terminal page, and lets a page digest appear once per manifest. A
repository's completeness is its least complete page's.

**2026-10-21: the manifest digest construction.** `manifestDigest` is
`sha256:` over the bytes `statecraft/context-manifest-digest/1`, one LF, and
the manifest's JSON with `manifestDigest` empty. Every object in the document
is a fixed struct and every collection is ordered, so the bytes are fixed by
the value. The document carries each page's byte length and Statecraft
custody digest beside the producer's page digest, so the digest covers the
retained bytes without recomputing any producer digest.

**2026-10-21: where the facts are written.** The manifest is written once,
under `contextManifest`, into the attempt's `prepare-workspace` intent, and a
manifest naming another run or attempt refuses before any record. Retained
bytes are kept write-once in the product home under
`context/<custody hex>.json` and read back only when their digest matches.
The adapter turn carries no effect identity (spec 003 section 3.3.1 clause 9
leaves the attempt's intent to the legacy pairing), so a delivery
observation is correlated by the attempt's own key: it is a `Kind::Evidence`
record, subject `context-delivery`, whose binding repeats `runId`, `attempt`
and `manifestDigest`, and a record whose binding differs does not bind. A
report under another manifest digest makes every page `contradictory`. Two
reports are a `duplicate-observation` finding, and a page they disagree on is
`contradictory`; a report beside an unreadable observation is a
`contradictory-observation` finding and makes every page `contradictory`.

**2026-10-21: the version 1 channel's framing and its testimony.** The
adapter request carries the bundle as its own `context` member: the
manifest's exact bytes, its digest, and every page whose intended delivery is
`required` or `optional`, in manifest order, each with its custody digest. A
`withheld` page is never carried. Before spawn the bundle must be exactly the
intent's manifest and those pages, or the attempt refuses with a stable
`context-*` code; the supervisor holds no record, so the binding that wrote
the intent reads the manifest back and calls the check before `supervise`.
The child's input stream carries the prompt, then one header saying the pages
are repository content supplied as data and grant no authority, then each
page after a header that declares its repository, page digest and byte
length and says one newline follows: exactly that many bytes of the page,
then one LF separator, so page content cannot end its own frame. The command line carries none of it. The init event's optional
`context` member is the adapter's testimony: the manifest digest it applied
and the pages it wrote. The Claude Code adapter attaches it only to the init
event the provider itself emits, so a provider that never starts leaves every
page `not-reported`. A stream that breaks before any init event is
`unreadable`.

**2026-10-21: acceptance asks a producer port, and the policy is its own
record.** `statecraft-acceptance` judges through a `Producer` trait with four
questions (validate a page's exact bytes, answer the recorded closure request
at the recorded snapshot, bind a caller-supplied current snapshot, and say
what a page holds under a member identity); `statecraft-home`, the one crate
that links the producer, answers them. Custody is compared first, so bytes
that are not the retained bytes never reach the producer. Integrity also
requires the producer's facts to repeat the manifest's page digest,
snapshot, request and closure digests, schema, build, continuation position,
omissions and warnings. A citation whose page records the member as omitted
is `missing`; one the page neither carries nor omits, or whose span leaves
the member's lines, is `outside-packet`. The section 3.8 policy is a closed
`statecraft/context-acceptance-policy/1` document written once into the same
intent under `contextPolicy`: each of delivery and the five checks is
`required` or `optional`, plus the exact `(dimension, state)` absences an
optional fact may carry. A fact is required only when its dimension is
required and its repository (and, for delivery, its intended page) is
required; every other non-passing fact makes acceptance ineligible unless its
exact state is permitted. No citation or no evidence supplied is the fact
`not-recorded`. An aggregate is the first required repository's non-passing
state, in manifest order. Each reason is spelled
`context-<dimension>-<state>: <subject>`.

**2026-10-21: which producer interface answers which read.** Spec-spine
0.31.0's library assembles a packet only over a caller-supplied snapshot, and
its executable is what binds one, so `statecraft-home` assembles every page,
re-asks a recorded closure and binds a current snapshot through the pinned
executable (`spec-spine context packet ... --json`, following each
continuation to the terminal page), and decodes and validates each page
through the linked library (`ContextPacket`, which denies unknown fields, and
`packet_digest`). A refusal is read from the verdict's `error.kind`. An incomplete page is
the producer's finding (exit 1) and the page is the verdict's `report`; it is
retained as `context_packet_document` renders it, which is byte for byte what
a successful exit prints, so every retained page is the producer's canonical
document. Closure
re-submits the request the page recorded at the revision it recorded; the
member differences are the closure members present in one producer document
and not the other. The current snapshot is the clean working tree at HEAD,
bound under the recorded request with the identity the caller supplies; a
dirty tree is the producer's `dirty-tree` refusal, so it is `unverified`. A
member is looked up, and a recorded request, revision or repository drives a
producer call, only from a page whose digest reproduces. The release
offers no standalone validator for a continuation token, so the page chain is
judged by each page's reproduced digest, which covers its token, and by the
manifest's terminal structure.

**2026-10-21: the command bindings.** `run <path> <spec-id> --context <file>`
names a task file: `optionalOmission`, the section 3.8 `policy`, an optional
`task`, and ordered `repositories`, each with `id`, `role`, `requirement`,
`path` (relative to the file), the caller's `repository` identity, an
optional `revision`, its `delivery`, and the producer's `request` passed
through unread. The manifest is composed for the attempt the intent will
append and refused before the intent, as `context-ineligible`, when its packets
cannot be assembled or it may not execute, so a refused task appends no
attempt. A bundle that does not match the recorded manifest concludes the
attempt `refused` under the guard `context-bundle` before any process; a
delivery record that cannot be written is a refusal under `context-delivery`.
`accept <path> <run-id> --context <file>` names the current `checkouts` by
repository id (`path`, `repository`), the claimed `citations` and the
`evidence` admissions; without it, every check that needs a checkout or a
claim reports its absence. An ineligible judgment is the acceptance reason
`context-ineligible` with every reason, a finding (exit 1), before the suite
runs. `run show` adds `contextManifest`, one entry per attempt, and `accept`
adds `contextManifest`, the judgment; a historical attempt renders
`not-recorded` in both.

## Verification

Implementation acceptance is the portable fixture matrix of section 3.10 in
the owning crates, run through `make verify SPEC=019`. No provider runs: the
adapter tests use a real child that reads its input, and the command test a
fake provider that replays a recorded stream. The producer runs as the real
pinned executable over a clone of this repository's own corpus.

```verify:cli
cargo test -p statecraft-run --test context_manifest
cargo test -p statecraft-adapter --test context_delivery
cargo test -p statecraft-adapter-claude-code --lib execution::tests::delivery_testimony_rides_the_providers_init_event_only
cargo test -p statecraft-acceptance --test context_packet_acceptance
cargo test -p statecraft-home --test context_producer
cargo test -p statecraft-cli --test context_run
```

These establish packet custody, delivery and acceptance on fixtures. They do
not show that a provider used a packet, that a cited member supports a claim,
or that two repositories were compatible.
