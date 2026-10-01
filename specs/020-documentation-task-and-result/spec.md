---
id: "020-documentation-task-and-result"
title: "Documentation task and structured result contracts"
status: draft
implementation: pending
created: "2026-09-27"
summary: >
  Amends Statecraft's run, adapter, acceptance, and command contracts with an
  immutable documentation-task document and a provider-neutral structured
  documentation result. It separates task authority from provider output,
  binds every claim to an explicit lifecycle grade and supporting references,
  preserves omissions and contradictions, and keeps provider completion apart
  from independent acceptance and publication.
amends:
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
extends:
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "019-context-packet-consumption"
obligations:
  - id: "I-1"
    kind: invariant
    text: "A documentation result is provider testimony bound to an immutable task, never acceptance, publication authority, or evidence merely because it is structured."
    anchor: "3-1-separate-documents-and-authorities"
  - id: "I-2"
    kind: invariant
    text: "A version 1 documentation task permanently prohibits apply-patch, commit, push, open-pull-request, publish, release, deploy, remote-admin, credential-read, and provider-select, and no task field can grant one of those operations."
    anchor: "3-3-authority-and-operation-limits"
  - id: "R-1"
    kind: requirement
    text: "Before execution, Statecraft records the complete documentation task, its context manifest, skill and schema identities, authority limits, required artifacts, review policy, deadline, and cost ceiling."
    anchor: "3-2-the-documentation-task"
  - id: "R-2"
    kind: requirement
    text: "A documentation result reports each claim, citation, evidence reference, lifecycle grade, unknown, contradiction, unsupported reason, omission, and review field without converting absence into success."
    anchor: "3-5-the-structured-result"
  - id: "V-1"
    kind: verification
    text: "Portable fixtures prove immutable task binding, closed schemas, provider-neutral delivery, claim-level provenance, explicit absence, and separation from acceptance and publication without live provider activity."
    anchor: "verification"
    inputs:
      - "crates/statecraft-run/tests/documentation_contract.rs"
      - "crates/statecraft-adapter/tests/documentation_result.rs"
      - "crates/statecraft-acceptance/tests/documentation_contract.rs"
      - "crates/statecraft-cli/tests/documentation_projection.rs"
---

# 020: Documentation task and structured result contracts

## 1. Purpose

Spec 019 binds immutable context packets to an attempt and can resolve a
citation back to retained packet content. It intentionally does not say what a
documentation task asks for, what a provider must return, or how a claim names
its citations, lifecycle grade, uncertainty, and review state.

Without those contracts, a prompt becomes the only task record and prose becomes
the only result. The system could not distinguish an omitted field from a
negative answer, a provider's completion statement from acceptance, or a
repository-supported claim from a plausible sentence. This amendment defines
two closed documents so those distinctions survive execution.

This spec is a proposal. It does not authorize implementation, provider use,
spend, credential access, application of a patch, publication, or ratification.

## 2. Territory and prerequisite

This spec owns no product code. A later implementation may change only the
existing units named by its `extends` edges:

- `statecraft-run` owns immutable documentation tasks and retained results;
- `statecraft-adapter` carries those bytes without learning their policy;
- `statecraft-acceptance` decodes and exposes the inputs later acceptance uses;
  and
- `statecraft-cli` projects the records through existing run and acceptance
  commands without becoming a second implementation.

Implementation is blocked until spec 019 is ratified and its exact adopted
spec-spine producer identity supplies the packet and freshness interfaces on
which this contract relies. A draft producer commit, local checkout, moving
branch, or compatible-looking document is not a substitute.

## 3. Behavior

### 3.1 Separate documents and authorities

One documentation attempt has three separately identified documents:

1. the spec 019 context-task manifest, which identifies repository inputs;
2. `statecraft/documentation-task/1`, which states the work and its limits; and
3. `statecraft/documentation-result/1`, which records what the provider
   returned.

The task is written into the spec 003 intent before any adapter process starts.
The provider may return only a result. It cannot amend the task, relax a limit,
replace an input, approve its own claims, accept its own output, authorize a
repository mutation, or authorize publication.

The result is testimony by the named provider and adapter. Structured output
makes the testimony mechanically inspectable; it does not make it true,
accepted, admitted as evidence, or fit to publish. Those judgments remain
independent.

### 3.2 The documentation task

The task is a closed document with these required members:

| Member | Required meaning |
|---|---|
| `schema` | Exactly `statecraft/documentation-task/1`. |
| `taskId` | A stable identifier unique within the registered project. |
| `runId`, `attempt` | The exact spec 003 attempt. |
| `candidate` | Repository id, immutable revision and tree for the work product under review or change. |
| `contextManifestDigest` | The exact spec 019 manifest identity. |
| `objective` | A bounded statement of the requested documentation outcome. |
| `audiences` | A nonempty ordered set of named audiences. |
| `outputFormat` | One closed format token plus its version. |
| `operations` | Explicit allowed and forbidden operation sets. |
| `requiredCapabilities` | Adapter capability tokens that must be present before spawn. |
| `skill` | Exact skill identity, version and content digest, or explicit `not-required`. |
| `resultSchema` | Exact structured-result schema identity and digest. |
| `deadline` | Absolute deadline and the clock source used to enforce it. |
| `costCeiling` | Currency, maximum amount, and enforcement mode, or explicit `no-spend`. |
| `artifacts` | Ordered expected artifact paths and disposition for each. |
| `reviewPolicy` | Required reviewers, semantic-review requirement and acceptance route. |
| `publication` | Exactly `prohibited` in version 1. |

The task also carries its own digest construction and digest. The construction
is versioned and covers every member except the digest field itself. Strings,
collections, paths, durations and amounts have explicit size and count bounds.
Unknown members refuse decoding. A result schema supplied only inside packet
content cannot replace the schema identity in the task.

The candidate is the object the requested artifact describes or changes. It is
not silently replaced by the prepared workspace candidate that later acceptance
judges. When they differ, both identities remain visible and the review policy
must explicitly permit that difference. A silent review policy prohibits the
divergence.

### 3.3 Authority and operation limits

Both operation sets are exhaustive, not illustrative. The version 1 allowed
set is exactly `read-context`, `draft-artifact`, and `propose-patch`. The
version 1 forbidden set is exactly `apply-patch`, `commit`, `push`,
`open-pull-request`, `publish`, `release`, `deploy`, `remote-admin`,
`credential-read`, and `provider-select`. An extra token or a missing token is
malformed and requires a new schema version.

The closed forbidden operation set is the authority for operation permission.
The required version 1 `publication: prohibited` member is a separate,
human-readable assertion of the required `publish` prohibition. During
closed-schema decoding of raw task bytes, a missing `publish` prohibition or a
publication value other than `prohibited` is malformed and refuses before
spawn. No document with that disagreement is schema-valid. A later schema
version may change both only by defining its own closed operation set and
publication assertion together; neither field overrides the other.

`draft-artifact` permits result bytes only. `propose-patch` permits a patch as a
result artifact, not a filesystem write. A path in `artifacts` is a logical
destination used for review. It is not write authority. Applying a reviewed
artifact is a later, separately authorized operation outside this spec.

The task records which context repositories and packet pages are required for
each expected artifact. It may narrow delivery from the spec 019 manifest but
cannot add content, infer a missing page, or weaken a required repository to
optional.

### 3.4 Adapter delivery and observation

The adapter request carries exact task bytes and digest separately from prompt,
context packet, skill, and result-schema bytes. The provider-neutral adapter
protocol adds no documentation policy. Before spawn, it verifies only that the
request is bounded, identities are internally consistent, and every required
capability is declared by the selected adapter.

The init event reports the task digest, context manifest digest, the exact skill
digest or explicit `not-required`, result schema digest, and capability set the
adapter says it applied. These are adapter observations under spec 004. They do
not prove provider receipt or use. Missing, mismatched, duplicated, or
contradictory required observations remain explicit and make the later result
ineligible for acceptance. A task that records skill as `not-required` requires
that exact observation and no skill digest.

The terminal event carries exact raw result bytes and reports their media type,
byte length and custody digest. The supervisor retains the raw bytes before
decoding. A decoding failure never replaces them with a repaired document.

### 3.5 The structured result

The result is a closed document with these required members:

1. `schema`, exactly `statecraft/documentation-result/1`;
2. `taskId`, `runId`, `attempt`, task digest, and context manifest digest;
3. adapter and provider identities and versions;
4. ordered output artifact descriptors with media type, logical path, digest,
   size, and exact raw-byte reference;
5. an ordered nonempty or explicitly empty `claims` collection;
6. ordered `unknowns`, `contradictions`, `unsupportedClaims`,
   `providerOmissions`, and `statecraftCitationFailures` collections;
7. execution evidence references and their claimed purpose;
8. `providerCompletion`, exactly `claimed-complete`, `claimed-incomplete`, or
   `not-reported`;
9. `reviewDisposition`, exactly the sentinel `not-reviewed`; and
10. `acceptanceResult`, exactly the sentinel `not-attempted`.

The result schema delivered before spawn prescribes the last two members as
constant literals. The provider emits those literals in its response; the
adapter and supervisor neither inject nor rewrite them. They are
Statecraft-defined schema sentinels carried in the raw provider result, not
provider-selected review or acceptance testimony. Any other value refuses
schema validation. The raw provider result is immutable. Review and acceptance
append separate records referring to its digest; they do not rewrite either
sentinel. A projection may show the latest independent review and acceptance
beside the provider document, but it must label their sources.

Every collection is present even when empty. Empty claims plus
`claimed-complete` is representable and reviewable, not automatically valid.
Unknown fields, duplicate identifiers, invalid lifecycle values, unbounded
strings, path escapes, and references outside the task refuse schema validation.

### 3.6 Claim contract

Each claim contains exactly:

| Field | Meaning |
|---|---|
| `claimId` | Result-local stable identifier. |
| `text` | The bounded assertion presented to a reader. |
| `lifecycleGrade` | One of `specified`, `approved`, `implemented`, `tested`, or `released`. |
| `citations` | Ordered references into the retained spec 019 packets. |
| `evidenceRefs` | Ordered references submitted to spec 005 evidence admission. |
| `unsupportedReason` | Explicit reason, or `not-applicable`. |
| `contradictionRefs` | Ordered links to contradiction records. |
| `reviewDisposition` | Exactly the sentinel `not-reviewed`. |

The delivered result schema likewise prescribes claim-level
`reviewDisposition` as the constant literal `not-reviewed`, which the provider
emits without selecting a review judgment. The adapter and supervisor do not
inject it. It is a Statecraft-defined sentinel in the immutable raw result, not
provider testimony. A later review record binds the result digest and
identifies each reviewed `claimId` with its independent disposition.
Projections join those records without rewriting the claim or its sentinel,
and label unreviewed claims separately from claims a reviewer judged.

Lifecycle grades are assertions requiring support at that exact grade. A
`released` claim is not satisfied by evidence of implementation, and an
`implemented` claim is not inferred from an approved spec. Claims about several
grades are split into separate claim records.

A claim with no supporting citation or evidence must carry an
`unsupportedReason`. It remains unsupported even when the provider marks the
whole result complete. A citation may resolve mechanically under spec 019 while
failing to support the claim semantically. An admitted evidence reference may
establish one evidence property while leaving the claim unsupported at its
stated lifecycle grade.

### 3.7 Unknowns, contradictions, omissions and failures

An unknown identifies the question, the missing fact, and why the task inputs
could not answer it. It is never replaced by a guess. A contradiction names all
incompatible claim or source references and does not choose a winner. An
unsupported-claim entry identifies the claim and missing support. A provider
omission identifies a task-required field or artifact absent from the result.
A Statecraft citation failure preserves the citation and the exact spec 019
resolution state.

The same condition may appear in more than one collection when the meanings are
independent. For example, a missing packet member may be both a citation failure
and the reason a claim is unsupported. Deduplication must not erase either
meaning.

### 3.8 Durable binding and retry

The task digest is part of the intent and adapter request. Result bytes bind
only when task id, run, attempt, task digest, context manifest digest, adapter
identity and provider identity match the attempt record. A mismatch, duplicate
terminal result, or conflicting digest is a finding, never resolved by choosing
the more favorable document.

A retry writes a new task document for its new attempt even when all semantic
fields are equal. It may refer to the same immutable context packets, skill and
schema only through their exact identities. Earlier task and result bytes stay
unchanged.

### 3.9 Command projections

`run show` adds the task identity, result identity, provider completion claim,
artifact descriptors, claim count, and counts for every explicit problem
collection. It reads the durable record and performs no validation or review.

`accept` may consume the immutable task and raw result as inputs to the later
acceptance contract. Until that contract exists, it reports documentation
acceptance as `not-declared`. Neither command applies a patch, writes an expected
artifact path, selects a provider, starts a retry, or publishes.

`not-declared` is Statecraft's command-level status for the absence of that
later contract. It is not a value of the provider result's `acceptanceResult`
member and is never decoded into, or written over, that member's initial
`not-attempted` value. The command projection labels the two sources separately
when it presents both.

Human and JSON output remain two renderings of one typed value under spec 006.
Historical attempts without these documents render `not-recorded`; their bytes
are not migrated or synthesized.

### 3.10 Observable negative cases

| Case | Required result |
|---|---|
| Result names another task digest | It does not bind to the attempt. |
| Adapter omits the applied result-schema digest | Delivery observation is incomplete and later acceptance is ineligible. |
| Provider returns prose instead of the required schema | Raw bytes are retained; schema validation fails. |
| Claim says `released` but cites only an approved spec | Citation may resolve, but the claim remains unsupported at `released`. |
| Citation resolves to equal text under another member identity | The citation fails under spec 019. |
| Provider marks an unknown as known without support | The assertion is a claim and the original unknown remains visible. |
| Result includes a patch for an expected path | The patch is a review artifact and is not applied. |
| Task and workspace candidates differ under a silent review policy | The divergence is prohibited. |
| Raw task bytes omit the `publish` prohibition or give another publication value | Closed-schema decoding marks the task malformed and refuses before spawn. |
| Provider says the task is complete | `providerCompletion` records the claim; acceptance remains independent. |
| Result requests publication | The request has no authority and version 1 publication remains prohibited. |
| Retry receives identical inputs | It still has a new attempt-bound task and result identity. |

## 4. Out of scope

- Ratifying or implementing this spec or spec 019.
- Defining semantic documentation acceptance, reviewer judgment, correction,
  application, or publication.
- Selecting or invoking a live provider, accessing credentials, or spending
  money.
- Treating provider output, a resolved citation, a digest, or schema validity as
  evidence admission or acceptance.
- Applying a proposed patch or writing an expected artifact path.
- Adding a provider-specific field to the neutral task or result schema.

## 5. Resolved decisions

**2026-09-27: task authority and provider testimony use separate immutable
documents.** The provider can answer a task but cannot mutate the authority that
bounded it.

**2026-09-27: lifecycle grade belongs to each claim.** One result-level grade
would hide mixed support and invite implementation, testing, or release to be
inferred from a weaker fact.

**2026-09-27: expected artifact paths are logical destinations, not write
authority.** The documentation lane produces reviewable bytes or patches.
Application and publication remain later, separately authorized acts.

## Verification

No implementation acceptance is declared while spec 019 and this spec remain
unratified. The `V-1` inputs name the portable fixture surface a later
implementation must add. Fixtures use deterministic fake adapter results and no
live provider, credentials, spend, network publication, or repository mutation.
