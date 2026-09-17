---
id: "007-shared-evidence-envelope"
title: "The shared evidence envelope: one owner for the bytes two products exchange"
status: approved
implementation: complete
created: "2026-09-16"
summary: >
  The statecraft platform wrote an evidence envelope against this product's
  serializations and staged it pending transfer. This spec receives it: it
  claims crates/statecraft-envelope/, records its provenance and the Apache-2.0
  term it crossed under, and makes it the single owner of the reference, the
  four evidence dimensions, admission and the three names for absence, which
  005 declared and the platform had copied. It resolves the wire ambiguity in
  Recorded<T> by reserving the three absence words and refusing a present value
  that would collide with one, rather than by ordering the variants; it fixes
  what a compatibility fixture must prove, and emits the fixtures from this
  product's own serializer; and it fixes how the platform depends on this crate
  so the staged copy does not become a fork.
establishes:
  - { kind: directory, path: "crates/statecraft-envelope/" }
extends:
  # Spec 005 owns its crate as one directory unit, so the edge names that unit:
  # the files touched are `Cargo.toml`, `src/{absence,dimensions,evidence}.rs`,
  # `tests/negative_cases.rs` at two call sites the added members reach, and the
  # new `tests/envelope_compat.rs`, which is the cross-consumer half of section
  # 3.4 and has to live beside the product whose output it pins. Each keeps its
  # name, its public API and its serialization; what changes is that a type is
  # re-exported rather than declared a second time.
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "005-acceptance-and-evidence"
---

# 007: The shared evidence envelope

## 1. Purpose

Spec 005 declared the vocabulary this product reports evidence in: a reference
with a named construction, four dimensions with closed value sets, admission
kept apart from them, and three names for absence. The statecraft platform then
had to read and write the same records, read those serializations out of this
repository at `ee0fa7d`, and implemented them a second time in a crate it
staged as `crates/statecraft-envelope/`, explicitly pending transfer here.

Two implementations of one wire format is not a duplication to tidy up later. It
is **two answers to the same question**, and the two had already diverged before
either product shipped: the platform's reader takes `"not-recorded"` as an
absence, this product's reader takes it as a present string, and both are
faithful to the same written contract. A receipt read by the two products
therefore says different things depending on who is reading, which is precisely
what a receipt exists to prevent.

This spec ends that by receiving the crate and making it the one owner.

## 2. Territory

`crates/statecraft-envelope/`.

It also touches spec 005's crate, under the `extends` edges in the frontmatter:
`absence`, `dimensions` and `evidence` stop declaring the shared types and
re-export them. **Nothing 005 requires changes.** Its value sets are the same
sets, its rules about which value belongs to which dimension are still enforced
by the types, its serializations are byte for byte what they were, and the
public names a reader of `statecraft-acceptance` uses are unchanged. What moved
is where the type is defined.

The crate arrived from another repository. `crates/statecraft-envelope/PROVENANCE.md`
records the origin repository, commit, spec and licence, and every change made
on the way in; section 3.5 fixes what that record must contain.

## 3. Behavior

### 3.1 One owner, and who re-exports

These types have exactly one definition, in `statecraft-envelope`:

| Type | Was declared in |
|---|---|
| `Absence`, `Recorded<T>` | `statecraft-acceptance::absence` and the staged crate |
| `Construction`, `Embedded`, `Reference` | `statecraft-acceptance::evidence` and the staged crate |
| `Integrity`, `Signature`, `IssuerTrust`, `SubjectBinding`, `Dimensions` | `statecraft-acceptance::dimensions` and the staged crate |
| `Admission`, `RefusalCode`, `AdmissionPolicy` | `statecraft-acceptance::dimensions` and the staged crate |

`statecraft-acceptance` re-exports each of them under the name it already had.
A re-export rather than a conversion is deliberate: an adapter between two
definitions is a third place for the two to disagree, and it would have to be
written in whichever direction the caller happened to need.

What stays in `statecraft-acceptance` is this product's **judgment** over those
types, because judgment is 005's subject and not the envelope's: `admit` decides
admission from the four summary statuses under a policy, `integrity` answers the
dimension from a reference and the bytes in hand, `issuer_trust_may_pass` is
005 section 3.5's gate, and `Statement` is 005's own vocabulary that nothing
outside this product writes.

The envelope's own evaluator (`admission::evaluate`) answers a different
question over a different input: per-artifact verdicts with reasons, approvals
and a submitter. Neither function is the other's fallback, and neither is
deleted in favour of the other.

**A member added to a shared type is additive or it is not added.** The envelope
extends `RefusalCode` with members and `AdmissionPolicy` with fields; every
added field is skipped at its default, and every added member is optional on the
wire, so a value this product writes is unchanged. A change that would alter an
existing serialization is a change to spec 005, decided there.

### 3.2 `Recorded<T>`: the three words are reserved

`Recorded<T>` is untagged: a present value is written as itself, an absence as
one of `none`, `not-recorded`, `stale`. For `T = String` the two overlap
exactly, and the bytes do not say which was meant.

**Declaring one variant before the other does not resolve this**, and this spec
records that plainly because it was the staged crate's answer. Serde's untagged
deserializer tries variants in declaration order, so the order picks which
reading wins locally. It does not make the other reading unreachable, it does
not stop a writer emitting the colliding bytes, and it leaves two readers free
to choose differently. It moves the disagreement.

The resolution, in two parts, which together make the encoding injective:

1. **On the wire, the three words are absences.** Every decoder reads
   `"none"`, `"not-recorded"` and `"stale"` as the absence they name, for every
   `T`, in every position.
2. **A present value that would serialize to a reserved word cannot be
   encoded.** The checked constructor refuses to build one, and the serializer
   refuses to write one, so the ambiguous bytes are never produced by either
   product. A type says what it could collide with through the `Recordable`
   trait; a type whose serialized form is not a bare string cannot collide and
   says so in one line.

The encoding is **unchanged**: the same values produce the same bytes as before.
The only values now refused are the ones that were ambiguous.

**Legacy decoding.** Records written by this product at or before `8f6591f`
could carry a present string equal to a reserved word, because nothing refused
it. Such a record now decodes as **the absence**, in both products. This is a
deliberate reinterpretation of bytes that were ambiguous when written, and it is
the reading this product intended: `harness_revision` is written as
`"not-recorded"` precisely to say that no harness revision was observed
(005 section 3.4). The cost is named rather than hidden: a record that genuinely
meant the literal string `not-recorded` reads as an absence and cannot be
re-encoded. No such record is known to exist, and
`testdata/fixtures/cli/recorded-collision-legacy.json` holds the bytes a
pre-007 build would have written for one, so how they read is pinned by a test
rather than by this paragraph.

**Why not a versioned or tagged representation.** An explicit tag
(`{"recorded":{...}}` or a `version` member) would remove the ambiguity at the
encoding layer rather than by reservation. It is rejected here for one reason:
it changes every byte this product has already written, including every
`harness_revision` in every receipt, and 005 section 3.7's rule that historical
records are never rewritten to satisfy a newer rule applies to this product's
own records first. Reservation buys the same injectivity at the price of three
string values that no caller wants. If a future contract needs a present value
that is exactly one of the three words, a tagged representation is the answer,
and it is a new schema version rather than a reinterpretation.

### 3.3 The reference

The reference keeps this product's field names, their order, and the
construction spellings `file-bytes-sha256` and
`canonical-record-sha256/<version>`. The envelope adds one construction
(`statecraft-object-blake3`, the platform's native copy) and three optional
fields (`producer_digest`, `native`, `subject`), each omitted when absent.

A construction this build does not know is **preserved verbatim and reported
unknown**, never guessed at. 005 section 3.7's rule then answers it the way it
answers a canonical record: `integrity` reads `unknown`, because a file-byte
hash answers a different question.

### 3.4 What a compatibility fixture must prove

Three properties, and they are independent. A suite that checks only the second
is the suite that would have missed the `Recorded<T>` divergence entirely, since
both readings round-trip perfectly.

1. **Provenance.** Each fixture is bytes **emitted by this product's
   serializer**, not derived by hand from its serde attributes. The fixtures
   here were emitted by `statecraft-acceptance` at `8f6591f`. Reproducibility is
   not left to the commit message: `crates/statecraft-envelope/tests/legacy_cli/`
   holds that serializer transcribed and frozen, sharing no code with the shared
   types, and a test requires it to still write every committed fixture byte for
   byte. A fixture with no emitting case is refused.
2. **Byte preservation.** Each fixture deserializes into the shared type and
   re-serializes identically, member order included.
3. **Semantic interpretation.** The value means what this product meant. Each
   fixture is read for its meaning, and the suite covers, by name: every
   absent-or-defaulted field (`AdmissionPolicy`'s added fields, the reference's
   added fields, a receipt written before `authority_paths_touched` existed);
   unknown enum values, both the members preserved as `Unknown` (construction,
   refusal code) and the ones refused outright (a fifth dimension value, and a
   value belonging to another dimension); and **all three reserved strings**,
   bare and in the receipt field that motivated the type.

The suite runs from both sides. `crates/statecraft-envelope/tests/cli_compat.rs`
reads the fixtures through the shared types.
`crates/statecraft-acceptance/tests/envelope_compat.rs` checks the direction an
operator cares about: that `mint`, `admit`, `Dimensions::unsigned_today` and
`Reference::over_file_bytes`, the functions this product actually calls, still
produce those exact bytes.

### 3.5 Provenance and the licence

A crate received from another repository carries a record, in
`PROVENANCE.md` beside it, naming: the origin repository, commit and spec; the
status it was staged under; the licence it crossed under; what it was written
from; and every change made on the way in.

The crate's `Cargo.toml` states `license = "Apache-2.0"` **literally**, not by
workspace inheritance. The workspace value is the same today. The crate crossed
a boundary whose repository LICENSE is not Apache-2.0, and the term it crossed
under is a property of the crate; an inherited value would say the same words
today and silently stop saying them if the workspace ever changed. This
repository's `LICENSE` is untouched, as the source ownership table requires.

### 3.6 The native golden vectors stay provisional

`testdata/vectors/` holds five golden vectors for shapes this product does not
write: the entry, the fact envelope, the tombstone, the attestation. They are
**provisional, and this transfer does not freeze them.** They freeze at the
platform's first signed entry, under the platform's own constitution VIII, and
no such entry exists. Until then a vector change is an ordinary change, and
nothing in this repository may cite them as a frozen interoperability contract.

What this spec does freeze is the CLI-facing half: the serializations in section
3.1, which were frozen by 005 before this crate existed and are unchanged by it.

### 3.7 The platform's dependency

The staged copy stops existing. The platform consumes this crate the way this
repository already consumes an unpublished sibling (`attest-ledger-core` in
`statecraft-run`): a git dependency pinned to a revision of this repository.

The transfer is complete when all four hold:

1. This repository claims the crate and carries it (this change).
2. The platform's `statecraft-evidence` depends on it by pinned revision rather
   than by path.
3. The platform's `crates/statecraft-envelope/` is deleted and its spec 003
   stops claiming it.
4. The platform's `docs/design/03-shared-contract-resolution.md` records this
   spec as the CLI owner's answer to its three open questions.

Items 2 to 4 are changes in the platform repository and are that repository's to
merge. Until the revision this change lands as exists, the pin cannot be
written, which is the one ordering constraint in the handoff: **this change
first, the platform's second.** No publication of either is authorized by this
spec.

## 4. Out of scope

- **A second trust model.** This crate's `roots` (keyed, windowed, with an
  anchor origin) and 005 section 3.6's `trust` (named roots, eligible or not)
  are different models of different things, they share no bytes, and neither is
  a copy of the other. They are deliberately not merged here. Section 5 records
  it as an open question rather than a finding.
- **Unknown field preservation.** Unknown enum *members* are preserved; an
  unknown *field* of a struct is dropped on read, because no type here carries
  an extras map. Recorded in section 5, with the test that pins the current
  behaviour.
- **Publication.** Nothing here publishes, and the crate is `publish = false`
  at version `0.0.0`. Deferral `F-02` holds.
- **Adopting the platform's native path.** This product writes no entry, fact,
  tombstone or attestation. Owning the types is not using them.

## 5. Decisions recorded during implementation

Dated entries for choices section 3 was silent on. None changes what it requires.

**2026-09-16: re-export, not an adapter.** Section 3.1 says one owner and does
not say how the other side reaches it. A compatibility adapter between two
definitions was considered and rejected: it keeps both definitions alive, so it
keeps the divergence possible, and it has to be written in a direction. The cost
of the re-export is that `Reference`'s struct literal now needs three more
fields at its construction sites; the two in this repository use functional
update from `over_file_bytes` instead.

**2026-09-16: a refusal from `admit` names one reason and writes no `reasons`
array.** The envelope's `Admission::Refuse` gained a `reasons` list, skipped when
empty. This product's `admit` returns on the first hard refusal, so it fills
`reason` and leaves `reasons` empty, which is why its output is byte for byte
what it was. `Admission::reasons()` reads either shape back as the list it is, so
no reader has to know which producer wrote the value.

**2026-09-16: the frozen encoder is a test fixture, not a second implementation.**
`tests/legacy_cli/` looks like the duplication this spec exists to remove. It is
the opposite: the fixtures are only evidence if something that is not the shared
types can still produce them, and after the re-export the acceptance crate no
longer can. It is never edited to make a test pass.

**2026-09-16: the reserved-word check is a trait, not a serialization probe.**
Deciding whether a present value collides could be done by serializing it to a
JSON value and looking. A `Recordable` trait with a defaulted method does it at
compile time instead, costs nothing at run time, and does not quietly make the
type JSON-only. The price is a one-line impl per payload type; `Statement` has
one.

**2026-09-16: the eight hand-derived fixtures were correct.** The platform's
staging commit derived eight fixtures by hand from this product's serde
attributes and labelled them as such. Emitting the same cases from the
serializer produced byte-identical output for all eight. The replacement is
about what the fixtures are evidence of, not about a defect in them.

**Open, for the owner, not decided here:**

- Whether `trust::RootSet` and `roots::RootSet` should converge, and in which
  direction. They answer different questions today and neither product reads the
  other's.
- Whether the shared structs should carry an extras map so an unknown field
  survives a round trip. It is a wire-compatibility improvement with an API cost,
  and it is not needed until a producer writes a field this build has not seen.
- Whether `VerifierRecord` (005 section 3.6) and the envelope's
  `EvidenceVerdict` verifier identity should be one type. They overlap in intent
  and not in shape.

## Verification

Each line is one command. The compatibility suite of section 3.4 is the
acceptance: it runs from both sides, and each side fails independently.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
cargo test -p statecraft-envelope --test cli_compat
cargo test -p statecraft-acceptance --test envelope_compat
cargo test -p statecraft-envelope --test vectors
test -f crates/statecraft-envelope/PROVENANCE.md
grep -q 'license = "Apache-2.0"' crates/statecraft-envelope/Cargo.toml
```
