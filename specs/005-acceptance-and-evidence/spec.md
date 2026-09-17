---
id: "005-acceptance-and-evidence"
title: "Independent acceptance, the receipt, and the separately reported evidence dimensions"
status: approved
implementation: complete
created: "2026-09-16"
summary: >
  How a claim of completion becomes, or fails to become, an acceptance. Fixes
  the identification of candidate bytes and the trusted base, the authority-set
  rule that stops a candidate from being judged by rules it wrote, the receipt
  that binds repository, base, candidate, suite, exit codes and policy digest,
  the four evidence dimensions reported separately from admission with closed
  value sets, independently supplied trust roots, the preservation of original
  bytes and named hash constructions, and the three names for absence, none of
  which reads as success.
establishes:
  - { kind: directory, path: "crates/statecraft-acceptance/" }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
---

# 005: Independent acceptance, the receipt, and the evidence dimensions

## 1. Purpose

This is the spec the product exists for. Everything else prepares the conditions
under which a result can be judged by something other than the thing that
produced it.

## 2. Territory

`crates/statecraft-acceptance/` (forward claim; unresolved until implemented).

The **semantics** of the four evidence dimensions in §3.5 were decided outside
this repository, in the ecosystem decision package's rows G-05 to G-07. This spec
carries them forward as an inherited constraint and freezes their version and
serialization here. It does not re-decide them, and it does not assert that the
inheriting was ratified for this repository: see `D-07` in the decision record.

## 3. Behavior

### 3.1 What is judged

Acceptance is evaluated over three identified things, all named in the record:

- **the candidate**: a commit sha in the prepared workspace, plus an assertion
  that the work tree was clean and that HEAD did not move during the suite.
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

### 3.3 The authority-set rule

If the candidate's diff touches the authority set (`001` §3.5), the acceptance
records an **authority change** and does not accept on the strength of the
candidate's own suite. It reports which members were touched, and that a human
decision recorded outside the candidate is required.

This holds even when the candidate's suite passes, and especially then.

**Who computes "which members were touched", and who must not.** Classifying a
change under the base's rules is spec-spine's job, not this product's: its spec
088, `a change is classified under the base's rules`, is approved and complete on
spec-spine main and **in no release**, including the pinned 0.18.0 (constraint
`C-16`). Revision 4 row CLI-08 already ruled on exactly this: integrate 088's
report once released, and do not invent current support.

So the split is:

1. **Corpus-side members**: the specs themselves and the spec-spine
   configuration. The classification comes from **spec-spine's delta report**
   once a release carries it. Until then this product records the authority-set
   verdict as `not-recorded`, names the missing spec-spine capability and its own
   pinned version, and **still refuses to accept** on the candidate's own suite.
   Refusing without the report is available; classifying without it is not.
2. **Repository-artifact members**: the check suite, the verifier, the hooks, and
   any acceptance instructions that live outside a `spec.md`. Membership is
   declared **here, by path**. The delta report gives each such path a structural
   class, which is useful detail and is not a membership answer.
3. **The environment manifest** (`002` section 3.3), which spec-spine cannot know
   about at all, because it is this product's own record of what it manages. Its
   diff is computed here, and only that.

**What the delta report answers, and what it does not.** Spec 088 classifies
**structurally, by path, under the base's rules**, and its class names are not
this product's member names. Its `policy` is `spec-spine.toml` and the paths the
base lists in `[index] extra_hashed_inputs`; its `verification` is a spec's own
`verify:cli` plan; there is no `hooks` class at all. A repository's hook scripts,
`Makefile` or CI workflow therefore arrive as `implementation`, `unowned`,
`bypassed`, or `policy` when the base happens to hash them, and none of those
answers whether the path is an authority-set member **here**.

So this product MUST define authority-set membership **by path** for the members
that are repository artifacts: the check suite, the verifier, the hooks, and any
acceptance instructions that live outside a `spec.md`. The delta report is used
for the corpus-side classes only. Membership is this product's question;
classification under the base's rules is spec-spine's, and reading the second as
an answer to the first would leave the repository-artifact members unchecked
while the verdict still read as complete.

This product does not build a second change classifier for case 1 while waiting,
and declaring membership by path in case 2 is not one: it says which paths matter
here, never how the base would classify a change to them.
A local reimplementation would answer a slightly different question than the
verifier the family will standardise on, which is the failure `001` section 3.2
exists to prevent.

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
| The installed spec-spine carries no delta report | Authority-set verdict `not-recorded`, naming the missing capability; acceptance still refused on the candidate's own suite. No locally built classifier. |
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

## 4. Out of scope

Signing and key custody; hosted admission; export bundles and corpus
attestations; publication of any kind; the optional one-way interface that would
later hand an outcome to aicortex, which is described here as a boundary and
implemented by neither side. Legacy factory certificate formats
(`tenant-emit`, `tenant-tail`) are assessed in
`docs/design/00-boundaries-and-reuse.md` and adopted by nothing here.

**One inherited obligation this spec does not carry, deliberately.** Revision 4's
shared-contract-acceptance paragraph required one fixture manifest tested by
**both** a TypeScript and a Rust verifier. The workspace that hosted the
TypeScript verifier is being archived, and `D-01` makes this product Rust only,
so the obligation has no home here. It is neither quietly satisfied nor quietly
dropped: `D-10` in the decision record is the choice between dropping it and
re-adopting it against a named consumer.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what it requires.

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

## Verification

Each line is one command. §3.10's seventeen rows are integration tests named
after the rows they cover, in `tests/negative_cases.rs`.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
cargo test -p statecraft-acceptance --test negative_cases
test -f crates/statecraft-acceptance/src/receipt.rs
```
