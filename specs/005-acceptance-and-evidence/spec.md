---
id: "005-acceptance-and-evidence"
title: "Independent acceptance, the receipt, the separately reported evidence dimensions, and the envelope two products exchange"
status: approved
implementation: in-progress
created: "2026-09-16"
summary: >
  How a claim of completion becomes, or fails to become, an acceptance. Fixes
  the identification of candidate bytes and the trusted base, the authority-set
  rule that stops a candidate from being judged by rules it wrote, the receipt
  that binds repository, base, candidate, suite, exit codes and policy digest,
  the four evidence dimensions reported separately from admission with closed
  value sets, independently supplied trust roots, the preservation of original
  bytes and named hash constructions, and the three names for absence, none of
  which reads as success. It also owns the envelope crate the statecraft
  platform wrote against these serializations and transferred here, which makes
  one owner for the reference, the dimensions, admission and the absence words
  rather than two implementations of one wire format.
establishes:
  - { kind: directory, path: "crates/statecraft-acceptance/" }
  - { kind: directory, path: "crates/statecraft-envelope/" }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
---

# 005: Independent acceptance, the receipt, the evidence dimensions, and the envelope

## 1. Purpose

This is the spec the product exists for. Everything else prepares the conditions
under which a result can be judged by something other than the thing that
produced it.

Sections 3.11 to 3.17 answer a second question, which arrived after the first
was settled. This spec declared the vocabulary the product reports evidence in:
a reference with a named construction, four dimensions with closed value sets,
admission kept apart from them, and three names for absence. The statecraft
platform then had to read and write the same records, read those serializations
out of this repository at `ee0fa7d`, and implemented them a second time in a
crate it staged pending transfer here.

Two implementations of one wire format is not a duplication to tidy up later. It
is **two answers to the same question**, and the two had already diverged before
either product shipped: the platform's reader took `"not-recorded"` as an
absence, this product's reader took it as a present string, and both were
faithful to the same written contract. A receipt read by the two products
therefore said different things depending on who was reading, which is precisely
what a receipt exists to prevent. Receiving the crate here, under the spec that
declared the vocabulary, is what ends it.

## 2. Territory

`crates/statecraft-acceptance/` and `crates/statecraft-envelope/`.

Two crates and one spec. They are not one crate because the envelope is the
half another product depends on: section 3.17 fixes how the platform takes this
dependency, and a platform that had to pull in the acceptance library to read a
receipt would be a fork waiting to happen. Inside this repository the split is
the one section 3.11 draws: the envelope declares the shared types, and the
acceptance crate re-exports rather than redeclaring them.

The **semantics** of the four evidence dimensions in §3.5 were decided outside
this repository, in the ecosystem decision package's rows G-05 to G-07. This spec
carries them forward as an inherited constraint and freezes their version and
serialization here. It does not re-decide them, and it does not assert that the
inheriting was ratified for this repository: see `D-07` in the decision record.

The envelope crate arrived from another repository.
`crates/statecraft-envelope/PROVENANCE.md` records the origin repository,
commit, spec and licence, and every change made on the way in; section 3.15
fixes what that record must contain.

## 3. Behavior

### 3.1 What is judged

Acceptance is evaluated over three identified things, all named in the record:

- **the candidate**: a commit sha in the prepared workspace, plus an assertion
  that the work tree was clean and that HEAD did not move during the suite. *Amended by section 3.19 rule 2:* both are computed from the
  attempt's branch and the workspace read as data, not through the workspace's
  own Git files.
- **the trusted base**: a commit resolved at run start (`003` §3.2), from which
  the authority set is read.
- **the policy**: the authority set as it exists **at the base**, identified by a
  digest over its bytes.

An acceptance whose candidate, base or policy cannot be identified is not a
failing acceptance. It is **no acceptance**, recorded as such.

### 3.1.1 Which outcomes are even eligible

Acceptance is attempted for exactly one of `003` section 3.4's five outcomes:
**`completed`**. It says nothing about acceptance on its own, which is why
acceptance is a separate judgment; but it is the only outcome that means the
attempt reached its own end and left a candidate to judge.

For the other four, acceptance is **not attempted**, and that is recorded rather
than left blank:

| Attempt outcome | Acceptance record |
|---|---|
| `failed` | `not-attempted`, reason `attempt-failed`. No receipt. |
| `refused` | `not-attempted`, reason `attempt-refused`, with the refusal count. No receipt. |
| `interrupted` | `not-attempted`, reason `attempt-interrupted`. No receipt, because nothing identifies stable candidate bytes. |
| `cancelled` | `not-attempted`, reason `attempt-cancelled`. No receipt. |

`not-attempted` is a fourth thing, distinct from an acceptance that failed and
from the three names for absence in section 3.8. A reader must never have to infer
from a missing receipt whether the suite ran and failed, or never ran at all.

*Amended by section 3.19 rule 3:* a `completed` attempt whose suite cannot be run
inside the protected evidence boundary is `not-attempted` with reason
`boundary-unavailable`.

### 3.2 Independence

The suite is run by this product, in the prepared workspace, from the
instructions read **at the base**. Three consequences:

1. An agent's statement that it finished is recorded as a claim, in a field named
   for a claim, and is never read as a result.
2. Where a tool emits a structured report, that report is what is read.
   `spec-spine verify` and the corpus checks have structured outcomes, and a
   zero exit code is not substituted for one.
3. A check that did not run is `unknown`. It is never a pass, and the count of
   checks that did not run is part of the outcome.

*Amended by section 3.19:* the suite runs inside the protected evidence
boundary of spec `004` section 3.18.

### 3.3 The authority-set rule

If the candidate's diff touches the authority set (`001` §3.5), the acceptance
records an **authority change** and does not accept on the strength of the
candidate's own suite. It reports which members were touched, and that a human
decision recorded outside the candidate is required.

This holds even when the candidate's suite passes, and especially then.

**Who computes "which members were touched", and who must not.** Classifying a
change under the base's rules is spec-spine's job, not this product's: its spec
088, `a change is classified under the base's rules`. Revision 4 row CLI-08 ruled
on exactly this: integrate 088's report once released, and do not invent current
support.

**2026-09-17: it is released, and this product reads it.** Spec 088 landed in
spec-spine `v0.19.0` and is carried by the pinned `=0.20.0` (constraint `C-16`).
The half of CLI-08 that was a wait ended when the pin moved; the half that is an
obligation is discharged here. Reading spec-spine's answer and computing one are
different acts, and only the second is forbidden: this product still builds no
classifier of its own.

So the split is:

1. **Corpus-side members**: the specs themselves and the spec-spine
   configuration. The classification comes from **spec-spine's delta report**,
   obtained as §3.3.1 requires and read as §3.3.2 fixes. Where no usable report
   is in hand (§3.3.3) the verdict reads `not-recorded`, names why and the
   installed spec-spine version, and **still refuses to accept** on the
   candidate's own suite. Refusing without the report is available; classifying
   without it is not.
2. **Repository-artifact members**: the check suite, the verifier, the hooks, and
   any acceptance instructions that live outside a `spec.md`. Membership is
   declared **here, by path**. The delta report gives each such path a structural
   class, which is useful detail and is not a membership answer.
3. **The environment manifest** (`002` section 3.3), which spec-spine cannot know
   about at all, because it is this product's own record of what it manages. Its
   diff is computed here, and only that.

#### 3.3.1 How the report is obtained

`001` §3.5 rule 1 requires every member of the authority set to be read at the
**trusted base revision**, never from the candidate. The classifier is part of
that, so three things are fixed rather than left to a caller:

1. The report is `spec-spine delta --base <trusted base> --head <candidate>
   --json`, whose envelope is spec-spine's own (its spec 034).
2. The **binary is resolved independently of the candidate**, from the pin the
   registered repository declares at the base. A candidate that chose the binary
   that classifies it would be classifying itself, which is what 088's own `D-1`
   exists to prevent on the configuration side and what this rule prevents on the
   binary side.
3. The acceptance library does **not** run it. It reads the envelope's bytes.
   §3.9 is why: nothing in this spec's crate acts, and the invocation belongs to
   the caller that already holds both revisions (`003` §3.2).

#### 3.3.2 Which class witnesses which member

Spec 088 classifies **structurally, by path, under the base's rules**, and its
class names are not this product's member names. Its `policy` is
`spec-spine.toml` and the paths the base lists in `[index] extra_hashed_inputs`;
its `verification` is a spec's own `verify:cli` plan; there is no `hooks` class
at all. A repository's hook scripts, `Makefile` or CI workflow therefore arrive
as `implementation`, `unowned`, `bypassed`, or `policy` when the base happens to
hash them, and none of those answers whether the path is an authority-set member
**here**.

So the mapping is written down, and every member it names is one `001` §3.5
already enumerates. Nothing here adds a member to that set.

| 088 class | Read here as |
|---|---|
| `policy` on the base's own `spec-spine.toml` | the **policy** member. |
| `policy` on any other path | **no member.** The path carries the class because the base hashes it (`[index] extra_hashed_inputs`), and whether it is a member here is case 2's declaration by path. Reading the class as membership would let this repository widen its own authority set by extending a configuration list, and would put `README.md` in it. |
| `constitutional` | the **policy** member: the tier-2 document that governs every spec. |
| `lifecycle` | the **policy** member, named by `001` §3.5 in as many words: the lifecycle policy `003` §3.1.1 reads. |
| `authority` | the **policy** member: the ownership and dependency edges that decide which spec governs a unit, and therefore which acceptance instructions judge it. |
| `verification` | the **acceptance instructions** member, for the instructions that live inside a `spec.md`: a spec's `verify:cli` plan. |
| `requirement` | no member. See below. |
| `implementation`, `derived`, `bypassed`, `unowned` | no member. |
| `unknown` | no answer at all (§3.3.3). |

The `policy` split is the one that would have gone wrong silently. Its two rows
are one class doing two jobs: spec-spine's configuration is corpus-side and is
this product's to refuse on, while every other hashed input is a repository
artifact whose membership case 2 keeps here, by path. This repository hashes
`README.md`, `AGENTS.md`, `docs/**` and `.claude/rules/**`, none of which `001`
§3.5 makes a member, so a path-blind reading would have made a README edit an
authority change and would have made the authority set editable from
`spec-spine.toml`.

`requirement` is the row to read twice, because excluding it is a choice and not
an oversight. A spec body edit is the ordinary shape of work in this corpus: the
coupling gate requires the owning `spec.md` in the same diff, so nearly every
candidate carries one. Reading it as an authority change would refuse every
candidate this product could ever judge, which is not a stricter rule but an
inoperative one. What a spec *requires* is settled by review of that spec; what
*judges* the candidate is the member set above, and a candidate that weakens its
own acceptance arrives as `verification`, which is a member.

That exclusion is a **membership** answer, which case 2 above keeps as this
product's own. 088 supplies the class; it never supplies the membership.

#### 3.3.3 When there is no answer

Four conditions make the corpus-side verdict `not-recorded`, and under every one
of them acceptance is **still refused** on the candidate's own suite:

1. no report was supplied;
2. the report declares a schema this build does not read;
3. the report uses a class this build cannot place, including 088's own
   `unknown`, which is a path spec-spine could not place either;
4. the report does not cover a path the acceptance is judging, which makes it a
   report about some other change.

The recorded reason MUST say what this product asked for and what came back, and
MUST name the spec-spine version that answered. It MUST NOT assert anything about
what a spec-spine **release** carries. "The installed spec-spine carries no such
report" was true until 2026-09-17 and is false under the current pin: a record
making a claim of that shape becomes false when the pin moves under it, and a
false sentence in evidence is worse than a missing one. Where the absence is that
nothing asked, the reason attributes it to this product. A reader of an old
record and a reader of a new one are then reading the same claim about the same
thing.

#### 3.3.4 What reading the report permits, and what it does not

With a usable report in hand, no corpus-side member witnessed, no declared
repository artifact in the diff and the environment manifest untouched,
acceptance **may** rest on the candidate's own suite. That conclusion was
unreachable before: the corpus-side answer was always absent, so every candidate
was refused for want of it. It is the only conclusion this integration adds.

It adds nothing else. Acceptance is not publication (§3.9) and a receipt is not a
permission. Spec 088 §3.5 is explicit that its own `priorPolicy.required: false`
means only that no structural class changed, and not that a change is safe,
correct or approved; that field is recorded verbatim beside the verdict, because
it is spec-spine's answer to spec-spine's question, and it is never read as an
acceptance.

This product does not build a second change classifier for case 1, and declaring
membership by path in case 2 is not one: it says which paths matter here, never
how the base would classify a change to them. That prohibition was never
conditional on the report being unreleased, and reading the report does not
soften it. A local reimplementation would answer a slightly different question
than the verifier the family will standardise on, which is the failure `001`
Section 3.2 exists to prevent.

### 3.4 The receipt

A receipt is minted only when the suite passed over a candidate whose HEAD did
not move and whose work tree stayed clean. It binds:

repository identity; base revision; candidate sha; the ordered suite with each
command and **each exit code**; the policy digest; the versions of this product,
of spec-spine and of the adapter; the **resolved harness revision** (below); the
attempt identity; and the authority-set paths the candidate touched.

**The harness revision is this product's to record.** spec-spine's design note 06
section 2 fixes the seam in both directions: spec-spine publishes a harness
package identity, and only the thing that launched the worker can observe which
revision that worker actually resolved. That is this product. A receipt that names
the spec-spine binary version but not the harness revision cannot answer which
skills and rules were in force, and the note's name-precedence trap makes the
question real: a personal skill can shadow a repository's committed copy while the
governed copy sits untouched on disk.

No such package exists yet: note 06 section 3.2 is a proposal, not a release. So
the field is present and reads **`not-recorded`** today, per section 3.8, rather
than being omitted and added later. Omitting it would make every receipt minted
before the package silently unanswerable on the point.

A receipt is evidence that a specific suite passed over specific bytes under a
specific policy. It is not a permission, and nothing about it authorizes a later
effect on its own.

A receipt for a head that is no longer the branch's is **`stale`**. Stale is a
reported state, not an error and not a pass.

### 3.5 The four evidence dimensions, and admission

Reported separately, each from its own check, each with a closed value set:

| Dimension | Values |
|---|---|
| `integrity` | `pass` / `fail` / `unknown` |
| `signature` | `pass` / `fail` / `unsigned` / `unknown` |
| `issuerTrust` | `pass` / `fail` / `unknown` |
| `subjectBinding` | `pass` / `fail` / `unknown` / `not-applicable` |

`admission` is **kept apart** from all four: `admit` / `refuse`, with a reason
code. Incomplete required evidence refuses admission and names what was missing.

Rules that are part of the vocabulary, not commentary on it:

- `unsigned` belongs only to `signature`.
- `not-applicable` belongs only to `subjectBinding`, and only for a record type
  that has no subject. A missing expected subject is `unknown`.
- `issuerTrust` can pass only after a passing `signature` against an eligible
  root. A passing signature alone never establishes issuer trust.
- An unperformed check is `unknown` in its own dimension, and does not lower or
  raise another.

**What this product reports today, once implemented:** every `signature` as
`unsigned` and every `issuerTrust` as `unknown`, because nothing is signed. That
is the honest report, not a gap in the implementation.

### 3.6 Trust roots

Trust comes from roots supplied **independently of the evidence being judged**.

- An absent root yields `unknown`.
- A positively revoked or excluded root yields `fail`.
- A chain anchored on a key the evidence itself carries **cannot** satisfy a
  trusted-issuer policy, whatever its internal consistency.
- The record preserves the verifier's identity and version, the root-set
  identity, the coverage of what was checked, and the reason the verifier
  stopped if it stopped early.

A verifier never executes anything the evidence carries.

### 3.7 Bytes, digests and constructions

Original evidence bytes are preserved. A reference to them carries: type, schema
version, the SHA-256 digest and byte length of the **bytes**, the named hash
construction, and where relevant the container and selector for an embedded
record.

A canonical record hash never substitutes for a file-byte digest, and the
construction is part of the reference's identity. Any new construction is
**versioned**; historical records are never rewritten to satisfy a newer rule,
and historical evidence stays verifiable by the construction it was written
under.

Strict portable-input validation is a separate concern from key-ordered
serialization: rejecting duplicate keys, non-integer numeric tokens,
out-of-range integers and invalid encoding is a validator, and
`canonical-keysort-json` is not one. Where this product needs that guarantee it
adopts one versioned policy with shared fixtures, recorded per `001` §3.2.

### 3.8 The three names for absence

In every reported outcome, absence is one of three, and none reads as success:

- `none`: the run recorded that nothing of this kind happened.
- `not-recorded`: no record of this kind exists, including every field a future
  contract will add.
- `stale`: a record exists for a revision that is no longer current.

A narrative supplied by a model is labelled `narrative` and is kept apart from
what a machine observed, which is labelled with the record it came from.

### 3.9 The reviewable outcome

One read-only account per run, folded from the records and nothing else. Every
value names the record it came from. It shows what was requested beside what was
applied, the claim beside the independent result, each dimension separately, the
receipt or its absence by name, and every refusal.

Publication is not part of this spec. An accepted candidate with a receipt is a
reviewable outcome; whether anything is published from it is a later decision,
and no verb in this corpus publishes.

### 3.10 Observable negative cases

| Case | Required behavior |
|---|---|
| The agent reports success; the suite fails | Outcome `failed`, no receipt. The claim is retained in a field named for a claim. |
| The agent reports success; the suite never ran | **No acceptance** recorded, and the unrun checks counted. Not a pass, not a fail. |
| The attempt outcome is `refused`, `failed`, `interrupted` or `cancelled` | Acceptance `not-attempted` with the reason named. Never an empty result a reader must interpret. |
| No usable delta report: none supplied, a schema this build does not read, a class it cannot place, or a report about a different change | Authority-set verdict `not-recorded`. The reason names what was asked for, what came back and the **installed spec-spine version**, and never what a release carries (§3.3.3). Acceptance still refused on the candidate's own suite. No locally built classifier. |
| The delta report names a corpus-side member | `authority change` recorded, naming the member. No acceptance on the candidate's own suite. |
| The delta report classes a hashed input (`README.md`, `docs/**`) as `policy` | **Not** a corpus-side member. The class is recorded as detail; membership for that path is case 2's declaration (§3.3.2). |
| The delta report is read, names no member, and the diff touches no repository artifact and not the environment manifest | Acceptance **may** rest on the candidate's own suite. The only conclusion reading the report adds (§3.3.4). |
| A spec's `verify:cli` plan changed | The **acceptance instructions** member is touched, so it is an authority change, even though a body edit to the same file alone is not (§3.3.2). |
| spec-spine's `priorPolicy.required` is `false` | Recorded verbatim beside the verdict, and never read as an acceptance: 088 §3.5 says it means only that no structural class changed. |
| No harness package exists | The receipt's harness-revision field reads `not-recorded`. It is never omitted. |
| HEAD moved during the suite | No receipt; the attempt is `interrupted` (`003` §3.4). |
| The work tree was dirty at the end of the suite | No receipt, naming the dirty paths. |
| The candidate's diff touches the authority set and its suite passes | `authority change` recorded; no acceptance on the candidate's own suite. |
| The policy digest cannot be computed at the base | No acceptance; the reason is recorded. |
| A tool exits zero but its structured report says failed | The report wins. |
| A tool emits no structured report where one is expected | `unknown` for what the report would have carried, and the missing report is named. |
| Evidence carrying a self-anchored chain, judged under a policy requiring a trusted signature | `signature` may `pass`; `issuerTrust` is `fail` or `unknown` per §3.6; `admission` is `refuse` with a reason. |
| The same intact unsigned evidence under two policies | `admit` under a policy allowing unsigned evidence, `refuse` under one requiring a trusted signature. Neither result implies the evidence is trusted. |
| A byte-level mutation of preserved evidence | `integrity: fail`. Never repaired, never silently re-canonicalized. |
| A record written under an older hash construction | Verifiable under that construction; never rewritten to the newer one. |
| A receipt for a head the branch has moved past | `stale`, reported as `stale`. |
| A field a future contract will add, absent today | `not-recorded`. Never `none`, and never omitted. |

### 3.11 One owner, and who re-exports

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
Section 3.5's gate, and `Statement` is 005's own vocabulary that nothing
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

### 3.12 `Recorded<T>`: the three words are reserved

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
(section 3.4). The cost is named rather than hidden: a record that genuinely
meant the literal string `not-recorded` reads as an absence and cannot be
re-encoded. No such record is known to exist, and
`testdata/fixtures/cli/recorded-collision-legacy.json` holds the bytes a
pre-transfer build would have written for one, so how they read is pinned by a test
rather than by this paragraph.

**Why not a versioned or tagged representation.** An explicit tag
(`{"recorded":{...}}` or a `version` member) would remove the ambiguity at the
encoding layer rather than by reservation. It is rejected here for one reason:
it changes every byte this product has already written, including every
`harness_revision` in every receipt, and section 3.7's rule that historical
records are never rewritten to satisfy a newer rule applies to this product's
own records first. Reservation buys the same injectivity at the price of three
string values that no caller wants. If a future contract needs a present value
that is exactly one of the three words, a tagged representation is the answer,
and it is a new schema version rather than a reinterpretation.

### 3.13 The reference

The reference keeps this product's field names, their order, and the
construction spellings `file-bytes-sha256` and
`canonical-record-sha256/<version>`. The envelope adds one construction
(`statecraft-object-blake3`, the platform's native copy) and three optional
fields (`producer_digest`, `native`, `subject`), each omitted when absent.

A construction this build does not know is **preserved verbatim and reported
unknown**, never guessed at. Section 3.7's rule then answers it the way it
answers a canonical record: `integrity` reads `unknown`, because a file-byte
hash answers a different question.

### 3.14 What a compatibility fixture must prove

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

### 3.15 Provenance and the licence

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

### 3.16 The native golden vectors stay provisional

`testdata/vectors/` holds five golden vectors for shapes this product does not
write: the entry, the fact envelope, the tombstone, the attestation. They are
**provisional, and this transfer does not freeze them.** They freeze at the
platform's first signed entry, under the platform's own constitution VIII, and
no such entry exists. Until then a vector change is an ordinary change, and
nothing in this repository may cite them as a frozen interoperability contract.

What this spec does freeze is the CLI-facing half: the serializations in section
3.11, which were frozen by 005 before this crate existed and are unchanged by it.

### 3.17 The platform's dependency

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

### 3.18 Whether the contract an attempt was bound to still holds

Recorded on 2026-09-22 before the implementation it authorizes, for the binding
spec `003` section 3.1.3 writes into an attempt's intent. `accept` judges a
candidate against the spec it was built for, and a spec that moved while the
attempt ran is a different contract. The judgement is made from the producer's
own answers, never from a digest this product computes.

1. **When.** After the eligibility of section 3.1.1 and before anything else
   is observed, `accept` asks the producer, in the target, to resolve the
   request the intent recorded, and compares the answer with the binding.
2. **The words.**

   | Comparison | When | Effect on the acceptance |
   |---|---|---|
   | `current` | the new digest equals the bound one | none; the answer names the digest |
   | `changed` | the digest differs; every member whose identity differs is named, by kind and key, with both identities | **no acceptance**, reason `contract-moved` |
   | `withdrawn` | an obligation bound as in force is now withdrawn | **no acceptance**, reason `contract-moved`, naming it |
   | `missing` | a bound member no longer resolves (the producer's exit 1) | **no acceptance**, reason `contract-moved`, naming it |
   | `stale` | the producer refuses a stale ledger (its exit 2) | refused before anything is judged; nothing is recorded, and the answer says to refresh the ledger |
   | `not-recorded` | the intent carries no `bound` contract (an attempt from before section 3.1.3, or one bound as `unsupported`, `unresolved`, `stale` or `unreadable`), or the producer answering now cannot resolve closures | none; the answer's contract reads `not-recorded` under section 3.8, with the reason |

   `withdrawn` and `missing` are reported beside `changed` where both hold,
   because each names what an operator must look at.
3. **The run record is never rewritten.** The comparison belongs to the
   acceptance that made it, and is carried in that acceptance's answer: the
   bound digest, the digest now, the word, and every member named by rule 2.
   The attempt's intent keeps the contract it was bound to, and a later
   acceptance of the same attempt compares again.
4. **The receipt is unchanged.** Its bytes are a compatibility contract with
   the platform (section 3.14) that this section does not move. The comparison
   travels beside the receipt in the answer, never inside it, so a receipt
   says nothing about the contract, and a reader who needs to know reads the
   answer that minted it. Adding the contract to the receipt is a change to
   section 3.14's fixtures and is not made here.
5. **What `current` does not establish.** That the candidate satisfies the
   contract; only that the contract it was authorized against is the one the
   producer resolves now. Acceptance still rests on sections 3.1 to 3.3.

### 3.19 The suite runs inside the protected evidence boundary

An authority amendment, settled by the owner on 2026-09-23 with spec `004`
section 3.18 and recorded before the implementation it authorizes. That section
confines every execution this product makes of content a confined child could
have written, and the suite of section 3.2 is such content: it runs in the
prepared workspace, which the session wrote. Run outside the confinement, the
suite would be a route from the child's bytes to the protected set.

**Rule 1: the suite is confined.** The suite runs under the confinement of spec
`004` section 3.18, with the attempt's workspace, its temporary directory and a
per-attempt cache root as its writable roots. The toolchains' cache and home
variables (for Cargo, `CARGO_HOME`; for others, their documented equivalents)
point into that cache root, which starts empty for each acceptance; the
operator's own caches are readable and not writable. A suite that cannot run
that way fails or does not run, and is recorded as such under section 3.2's
third consequence; the writable roots are never widened to make it pass.

**Rule 2: the candidate's checks are made without the workspace's own Git
files.** Section 3.1's assertion that the work tree was clean and that HEAD did
not move is computed by this product from the attempt's branch in the target's
common Git directory and the workspace read as data (spec `004` section 3.18
rule 4): the branch's commit is recorded before and after the suite, and the
workspace's files are compared with that commit's tree, never through `git
status` run via the workspace's `.git` file or its administrative directory,
which the child can rewrite. Cleanliness is judged before the suite, against
the branch's commit, using ignore rules read from the trusted base and never
from the workspace. After the suite, only the branch's commit is compared, and
files the suite creates are not part of the candidate.

**Rule 3: refusal.** When the boundary cannot be established, `accept` refuses
with exit code 2, runs nothing and writes no receipt, and the acceptance record
is `not-attempted` with reason `boundary-unavailable`, beside section 3.1.1's
reasons. It is never a failing acceptance.

**Rule 4: records written before this section.** A receipt written before this
section records a suite run without confinement; it is read as it was and is
not re-judged.

## 4. Out of scope

Signing and key custody; hosted admission; export bundles and corpus
attestations; publication of any kind; the optional one-way interface that would
later hand an outcome to aicortex, which is described here as a boundary and
implemented by neither side. Legacy factory certificate formats
(`tenant-emit`, `tenant-tail`) are assessed in
`001` section 3.8 and adopted by nothing here.

**One inherited obligation this spec does not carry, deliberately.** Revision 4's
shared-contract-acceptance paragraph required one fixture manifest tested by
**both** a TypeScript and a Rust verifier. The workspace that hosted the
TypeScript verifier is being archived, and `D-01` makes this product Rust only,
so the obligation has no home here. It is neither quietly satisfied nor quietly
dropped: `D-10` in the decision record is the choice between dropping it and
re-adopting it against a named consumer.

- **A second trust model.** This crate's `roots` (keyed, windowed, with an
  anchor origin) and section 3.6's `trust` (named roots, eligible or not)
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

Dated entries for choices §3 was silent on. None changes what it requires. The
entries concerning the envelope were recorded against `007` while that spec was
separate; they are kept verbatim, because this section is a history and a
history is corrected by appending.

**2026-09-17: the report is read, and what that decided.** The owner authorized
the 088 integration as an authority change on its own, which is the authority the
entry below said this section did not have. Four choices §3 was silent on were
made in the course of it, and each is here rather than in a commit message
because each is a choice a reviewer could reasonably have made differently.

*The class-to-member mapping is fixed in the spec, not in the code.* 088's eleven
classes are structural and are not this product's member names, so somebody had
to say which witnesses which. Leaving it to the implementation would have put an
authority decision in a match arm. §3.3.2 is that mapping, and every member it
names is one `001` §3.5 already enumerates: the integration reads a new answer,
it does not widen the set the answer is about.

*`policy` is read against the path, not the class alone.* The first mapping
written here was path-blind, and it was wrong in a way that would not have shown
up in a test: this repository hashes `README.md`, `AGENTS.md`, `docs/**` and
`.claude/rules/**`, so every one of them arrives classed `policy`, and a blind
reading would have made a README edit an authority change and made the authority
set extendable by editing a list in `spec-spine.toml`. §3.3.2 splits the class:
the base's own configuration is corpus-side, and every other hashed input keeps
its membership where case 2 put it, which is here, by path.

*`requirement` is not a member, and that is the load-bearing half.* Including it
would have been the conservative-looking choice and would have made the
integration inoperative: the coupling gate puts an owning `spec.md` in nearly
every diff, so every candidate would have been an authority change and nothing
would ever rest on its own suite. The candidate that weakens the acceptance
judging it is not lost by the exclusion; it arrives as `verification`, which is a
member. `unknown` is the opposite case and is treated as no answer at all.

*The crate reads the report and does not run spec-spine.* The 2026-09-16 entry
below records that nothing in this crate acts, and that property is kept: the
bytes are handed in. It is not only tidiness. `001` §3.5 rule 1 reads every
authority-set member at the trusted base, and which binary classifies is part of
what must not come from the candidate, so §3.3.1 fixes the invocation contract
for the caller instead of burying a process spawn in the library that judges.

*A report about another change is refused.* §3.3.3 case 4 is not in 088 and is
not a doubt about it: the report answers about the diff it was given, and
checking that it covers the paths being judged is what stops a stale or
misaddressed report from being read as an answer about this candidate.

**2026-09-17: the corrected reason is attributed to this product, and is not a
new capability.** Section 3.3 named spec-spine 088 as carried by no release,
which the move to the `=0.20.0` pin falsified, and the falsehood had reached the
record: the corpus-side note this crate wrote said in so many words that the
installed spec-spine carries no change-classification report. Under the current
pin it does.

Two answers were available. Reading the report is what CLI-08 asks for and is a
capability this change does not add: it is its own change, and `AGENTS.md`
requires an authority change to be separated from the work it would authorize, so
bundling it here is the thing that rule forbids. The other is to say the true
thing about what this product does, which is that it does not read the report.
That is what the note now says, and it carries the installed version beside it so
a reader can see which spec-spine was asked and told nothing.

What did not change: the verdict is still `not-recorded`, acceptance is still
refused on the candidate's own suite, and this crate still builds no classifier
of its own. The prohibition in section 3.3 was never conditional on the report
being unreleased.

**2026-09-16: the vocabulary rules are types, not conventions.** §3.5 says
`unsigned` belongs only to `signature` and `not-applicable` only to
`subjectBinding`. Each dimension is therefore its own enum, so neither value is
constructible in the wrong place. A single shared value enum with a rule in
prose would have made both mistakes reachable, and the rules are described as
part of the vocabulary rather than as commentary on it.

**2026-09-16: `admission` is a separate type from the dimensions.** §3.5 keeps
it apart, and the reason is worth stating: a dimension says what a check found,
admission says what a policy decided about those findings. One type carrying
both would make "this evidence is intact" and "this evidence is acceptable" the
same sentence, which is exactly what §3.10's two-policy row exists to
distinguish.

**2026-09-16: `Recorded<T>` carries an absence rather than an `Option`.** §3.8
fixes three names for absence and says none reads as success. An `Option::None`
carries no name, so every optional field in a reported outcome is
`Recorded<T>`: present, or absent as one of the three. That is what makes "a
field a future contract will add reads `not-recorded`, never `none`, never
omitted" a property of the serialization rather than a rule somebody remembers.

**2026-09-16: the authority verdict refuses on `not-recorded`.** §3.3 says the
verdict reads `not-recorded` and that acceptance is still refused on the
candidate's own suite. Those are two statements, and the implementation makes
the second follow from the first: `may_accept_on_own_suite` is false whenever
the corpus-side answer is absent, not only when a member was touched. Refusing
without the report is available; classifying without it is not.

**2026-09-16: integrity is answered only under the construction the reference
names.** §3.7 forbids a canonical record hash substituting for a file-byte
digest. A reference under a construction this build cannot evaluate reports
`unknown` rather than falling back to a file-byte hash, because the fallback
would answer a different question and look like an answer to this one.

**2026-09-16: nothing in this crate acts.** §3.9 says publication is not part of
this spec and that no verb in this corpus publishes. The crate has no function
with an external effect: a receipt is a value, `mint` returns one, and there is
nothing to call that would do anything with it. That is the implementation of
"a receipt is not a permission".

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
- Whether `VerifierRecord` (section 3.6) and the envelope's
  `EvidenceVerdict` verifier identity should be one type. They overlap in intent
  and not in shape.

**2026-09-22: section 3.18, recorded before implementation.** An attempt's
intent will carry the contract spec 003 section 3.1.3 binds, and `accept`
compares it with the producer's resolution now. A contract that changed, lost a
member or withdrew an obligation is no acceptance, reason `contract-moved`; a
stale ledger refuses before anything is judged; an unbound or unresolvable
contract reads `not-recorded` in the answer, and the receipt's bytes do not
change. No code changed with this entry.

**2026-09-22: section 3.18 implemented.** `contract::compare` is pure over
the binding and a resolution: it keys members as `spec:`, `section:` and
`obligation:` and quotes the identity the producer gave each, the content hash,
the section digest, or the obligation's own fields other than its key, so a
change is named rather than inferred. `contract::check` is the one operation
`accept` calls: it finds the attempt's intent, reads its binding, asks the
producer to resolve the same request only where the binding is `bound`, and
compares. `NoAcceptance::ContractMoved` carries the comparison. The receipt is
untouched, as rule 4 requires.

**2026-09-25: let chains collapsed with the measured rust-version floor.**
The workspace floor moved from 1.85, which never built, to 1.88 (evidence in
spec `002` section 5, same date). Clippy's `collapsible_if` then applies let
chains, and the nested `if` blocks it named in this spec's crates were
collapsed mechanically by `cargo clippy --fix` and `cargo fmt`. No behavior
changed.

**2026-09-25: the delta report reads both `--json` envelopes (owner,
2026-09-25: adopt 0.26.0).** spec-spine 0.26.0's verdict envelope (schema
1.0.0, its spec 132) removes `ok` and carries `outcome`; the delta report
inside it is unchanged at schema 0.1.0. `Envelope` required `ok`, so every
0.26.0 delta would have read as not an envelope. `Envelope::ok` and the new
`Envelope::outcome` are both optional, and a verb answered only when it exits
0 and its envelope says so in either form; one that says neither did not
answer. `VerbDidNotAnswer` names what the envelope said (`ok=` or `outcome=`)
in place of a boolean. Section 3.18's comparison table names the producer's
stale code as 2, which is 0.25.0's; from 0.26.0 a stale ledger is 1, read by its words as
spec `003` section 5 records on this date. Tested against a verbatim 0.26.0
delta (`testdata/delta/spec-spine-0.26.0-pin-move.json`) and in
`both_envelopes_are_read_and_one_that_says_neither_is_refused`.

**2026-09-25: the delta report reads schema 0.2 (owner, 2026-09-25: adopt
0.27.0).** spec-spine 0.27.0 moves the delta report to schema 0.2.0 (its spec
142): a twelfth class, `relocation`, for a section moved between specs with a
proven `relocates` edge, and a `relocations` list. Both are additive, but on a
`0.x` line the minor is the breaking position, so this build read only `0.1.z`
and every 0.27.0 report would have been an absence under section 3.3.3 case 2.
`READS_DELTA_SCHEMAS` is now `0.1` and `0.2`. The `relocations` list is not
read. `relocation` is not in section 3.3.2's table, and this entry does not add
it: the table is a membership answer and is ratified text. A report that uses
it is therefore an absence under case 3, which is the refusal the section
already prescribes for a class this build cannot place. Placing it (a
relocation moves requirement text that `delta` has proven unchanged, so reading
it as `requirement`, no member, is the proposal) is the owner's decision. An
unresolved claim at the merge base is a `validation` error envelope with no
report under 0.27.0 (spec-spine's 145), where 0.26.0 gave a `stale` one; either
way it is not a report and the answer is an absence. Tested against a verbatim
0.27.0 delta (`testdata/delta/spec-spine-0.27.0-readme-edit.json`) in
`the_delta_report_reads_the_0_27_0_schema`,
`a_relocation_class_is_an_absence_until_the_table_places_it` and
`a_validation_error_envelope_is_not_a_report`.

## Verification

Each line is one command. §3.10's twenty-two rows are integration tests named
after the rows they cover, in `tests/negative_cases.rs`. The delta reader's row
is tested against a captured envelope the **pinned binary itself** wrote
(`testdata/delta/`), so a reader that agreed only with invented JSON would fail.

The compatibility suite of section 3.14 is the
acceptance: it runs from both sides, and each side fails independently.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
cargo test -p statecraft-acceptance --test negative_cases
test -f crates/statecraft-acceptance/src/receipt.rs
test -f crates/statecraft-acceptance/src/delta.rs
cargo test -p statecraft-envelope --test cli_compat
cargo test -p statecraft-acceptance --test envelope_compat
cargo test -p statecraft-envelope --test vectors
test -f crates/statecraft-envelope/PROVENANCE.md
grep -q 'license = "Apache-2.0"' crates/statecraft-envelope/Cargo.toml
cargo test -p statecraft-acceptance --lib contract
```
