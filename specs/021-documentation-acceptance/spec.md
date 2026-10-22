---
id: "021-documentation-acceptance"
title: "Independent documentation acceptance and review"
status: approved
implementation: complete
created: "2026-09-27"
summary: >
  Amends Statecraft's run, acceptance, and command contracts with an
  independent documentation-acceptance record. It keeps intended delivery,
  observed transport, schema validation, citation resolution, semantic review,
  and acceptance as separate facts; requires claim-grade support, preserved
  unknowns and contradictions, valid artifacts, and a fresh context manifest;
  and grants no application or publication authority.
amends:
  - "003-work-and-run-semantics"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
extends:
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "003-work-and-run-semantics"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "019-context-packet-consumption"
  - "020-documentation-task-and-result"
obligations:
  - id: "I-1"
    kind: invariant
    text: "Documentation acceptance is an independent judgment over identified task, result, context, artifact, evidence, and review records; no producer or transport fact substitutes for another."
    anchor: "3-1-the-acceptance-subject"
  - id: "R-1"
    kind: requirement
    text: "Acceptance reports delivery, transport, schema, citations, semantic support, artifact checks, context freshness, authority, and review disposition separately before deriving an overall result."
    anchor: "3-3-independent-dimensions"
  - id: "R-2"
    kind: requirement
    text: "Every accepted claim is supported at its stated lifecycle grade, every required unknown and contradiction remains visible, and no accepted result authorizes application or publication."
    anchor: "3-5-claim-and-content-review"
  - id: "V-1"
    kind: verification
    text: "Portable fixtures prove independent dimensions, semantic-review authority, claim-grade support, artifact and freshness checks, correction history, privacy-bounded retention, and the absence of application or publication effects."
    anchor: "verification"
    inputs:
      - "crates/statecraft-acceptance/tests/documentation_acceptance.rs"
      - "crates/statecraft-run/tests/documentation_review.rs"
      - "crates/statecraft-cli/tests/documentation_acceptance.rs"
---

# 021: Independent documentation acceptance and review

## 1. Purpose

Spec 020 makes a documentation result structured and keeps provider completion
separate from acceptance. Structure alone cannot decide whether a citation
supports a sentence, whether the sentence states the right lifecycle grade,
whether a required warning was omitted, or whether an example works. Those are
review judgments over exact inputs, not properties of valid JSON.

This amendment defines the independent judgment. It preserves the boundaries
already established by specs 005, 019, and 020: intended delivery is not
observed transport, a resolved citation is not semantic support, provider
completion is not acceptance, and acceptance is not application or publication.

This spec is a proposal. It authorizes no implementation, live provider use,
reviewer impersonation, repository mutation, publication, or ratification.

## 2. Territory and prerequisite

This spec owns no product code. A later implementation may change only the
existing units named by its `extends` edges:

- `statecraft-run` retains append-only review and correction records;
- `statecraft-acceptance` owns the checks and final judgment; and
- `statecraft-cli` binds review submission and acceptance inspection to those
  libraries without adding policy.

Implementation is blocked until specs 019 and 020 are ratified and implemented
against an adopted producer identity. A passing fixture for this design cannot
qualify an unreleased context producer or accept a real documentation artifact.

## 3. Behavior

### 3.1 The acceptance subject

One documentation acceptance judges an immutable core tuple:

1. registered repository and trusted base under spec 005;
2. run id, attempt, and prepared-workspace candidate;
3. spec 019 context-task manifest bytes and digest;
4. spec 020 documentation-task bytes and digest;
5. raw provider-result bytes and custody digest;
6. decoded result schema identity;
7. every exact output-artifact byte sequence and digest;
8. admitted evidence records; and
9. the acceptance policy read from the trusted base.

The complete acceptance subject is that core tuple plus the exact ordered
review and correction records submitted for judgment. A review or correction
record cannot include itself in the subject it names. Acceptance binds the
ordered record set only after every member has its own stable digest.

An absent, unreadable, mismatched, moving, or ambiguous member yields no
acceptance, with its reason. The evaluator never repairs raw bytes, selects a
more favorable duplicate, follows a result-supplied URL, or fetches a missing
input.

The authority-set rule in spec 005 still applies. A candidate that changes the
rules or review instructions that judge it cannot accept itself, even when all
documentation-specific checks pass.

### 3.2 Intended, observed, validated, reviewed, accepted

The acceptance record carries these facts as distinct values:

| Fact | Source | What it can establish |
|---|---|---|
| Intended delivery | Documentation task and context manifest | What was required and planned. |
| Adapter-observed delivery | Spec 019 and spec 020 init observations | What the adapter reports it made provider-visible. |
| Observed transport | Independently admitted transport evidence, when available | What the evidence contract establishes about bytes crossing a boundary. |
| Schema validation | Statecraft's bounded decoder | Whether exact result bytes satisfy the declared closed schema. |
| Citation resolution | Adopted spec-spine operation through spec 019 | Whether an exact cited member and digest resolve. |
| Reviewer judgment | Authorized review records | Whether content supports a claim and meets the task. |
| Acceptance | `statecraft-acceptance` | Whether the complete policy is satisfied. |

No row is copied into another. Observed transport is `not-recorded` unless an
independently ratified, implemented, and adopted evidence contract can supply
it. Adapter testimony does not fill that absence. A future wire-evidence spec
may add a source, but this spec does not depend on a draft proposal or reserve
its schema.

### 3.3 Independent dimensions

The record reports every dimension, even after one fails:

1. `taskBinding`: task, run, attempt, candidate, schema, skill, policy, and
   context identities agree;
2. `intendedDelivery`: all required packet, skill, schema, and task bytes were
   in the immutable delivery plan;
3. `adapterObservedDelivery`: trusted adapter events account for those exact
   identities;
4. `observedTransport`: independent evidence result or explicit absence;
5. `schemaValidation`: exact raw result bytes satisfy the bounded schema;
6. `citationResolution`: every required citation resolves under spec 019;
7. `evidenceAdmission`: every required evidence reference is decided under
   spec 005;
8. `semanticReview`: claim support, omissions, unknowns, contradictions, links,
   examples, format, and audience fit are reviewed;
9. `artifactValidation`: each expected artifact exists exactly once and passes
   its format-specific checks;
10. `contextFreshness`: packet and documentation-manifest freshness at the
    judgment time;
11. `authority`: reviewer and acceptance authority are identified and eligible;
    and
12. `publicationBoundary`: no application or publication effect occurred.

Each dimension is `pass`, `fail`, `unknown`, `unsupported`, `unreadable`, or
`not-recorded`, plus stable reasons and subject references. A required dimension
satisfies policy only with `pass`. Optionality must come from the task's recorded
policy and never from the result or reviewer.

### 3.4 Reviewer records and authority

A reviewer submits `statecraft/documentation-review/1` as an append-only record.
It names the immutable core tuple from section 3.1, reviewer identity,
authority source, review time, every claim disposition, artifact disposition,
requested correction, and overall recommendation. It has its own digest. The
later acceptance record names that review digest in its ordered record set;
the review does not recursively name itself.

The reviewer cannot be the provider identity or adapter identity that produced
the result. A self-declared reviewer role inside provider output grants no
authority. Version 1 recognizes `repository-owner` and `owner-delegated-reviewer`
only when the trusted-base policy identifies the exact authority. Missing,
expired, conflicting, or unverifiable authority is `unknown`, never approval.

A review recommendation is `accept`, `revise`, or `reject`. It is an input to
acceptance and does not itself mint the acceptance result. Multiple reviews are
preserved. The policy decides quorum and how conflicts are resolved; Statecraft
does not choose the most favorable review.

### 3.5 Claim and content review

For every spec 020 claim, semantic review answers separately:

- whether the claim text is clear and bounded;
- whether each citation resolves to the stated packet member;
- whether cited content actually supports the text;
- whether evidence establishes the asserted lifecycle grade;
- whether stronger contrary material exists in delivered context;
- whether an unknown or contradiction should have been reported; and
- whether the claim is permitted for the intended audience and output format.

The claim disposition is `supported`, `unsupported`, `contradicted`,
`insufficient-grade`, `needs-correction`, or `not-reviewed`. Only `supported`
satisfies a required claim. A claim marked `implemented` needs implementation
evidence; `tested` needs identified test evidence and its exact subject;
`released` needs immutable release evidence. Approval, green CI, provider prose,
or a resolved citation does not imply a stronger grade.

A required claim with disposition `not-reviewed` makes the `semanticReview`
dimension `not-recorded`. The judgment is incomplete and the overall result is
`no-acceptance`, not `rejected`. A completed review that judges a required claim
`unsupported`, `contradicted`, `insufficient-grade`, or `needs-correction`
makes `semanticReview` fail and may produce `rejected` once the complete
judgment runs.

Every provider-reported unknown and contradiction is reviewed for faithful
preservation in the artifact. Review also checks for unknowns, contradictions,
unsupported claims, provider omissions, or Statecraft citation failures that
the result should have reported but did not. Silence never becomes an empty
collection by inference.

### 3.6 Artifact validation

Each task-required artifact must exist exactly once at its logical path, match
its declared media type and digest, remain within size limits, and contain every
required section. Format-specific validators operate only on retained bytes and
write nothing.

Links are checked according to the task policy. Internal anchors and relative
links are deterministic reads over the artifact bundle. External links are
`not-recorded` unless exact network access was separately authorized and its
result retained; this spec grants no such access.

Examples are classified as `illustrative` or `executable` by the task, never by
the provider after execution. Illustrative examples receive semantic review.
Executable examples pass only when the trusted-base acceptance instructions
name a deterministic command, its exact environment is available, and the
recorded structured result passes. An unrun example is `unknown`.

Authored and generated content remain distinguishable. A generated artifact or
patch cannot overwrite authored content during validation. Artifact validation
does not stage, apply, format, commit, push, or publish anything.

### 3.7 Freshness and manifest closure

Acceptance asks the adopted producer for the spec 019 packet checks and the
documentation-manifest freshness result against caller-supplied stable, clean
current snapshots. It records the exact observation time and producer identity.
It neither reads derived shards directly nor rebuilds producer algorithms.

The documentation manifest must enumerate every source identity on which the
artifact and review depend, including the task, context packets, skill, result
schema, artifact bytes, examples, and review instructions. A missing dependency
is an incomplete manifest, not a fresh one. A producer answer of stale,
incomplete, unsupported, unreadable, or not-recorded prevents acceptance.

Freshness says only that declared inputs remain current under the producer's
contract. It does not establish semantic correctness, reviewer authority,
transport, evidence admission, acceptance, or publication fitness.

### 3.8 Corrections and re-review

A correction is a new immutable artifact or result record referring to the
superseded digest, correction author, reason, changed claims, and changed
artifact bytes. It never rewrites raw provider output, an earlier review, or an
earlier acceptance attempt.

Any correction invalidates prior schema, citation, semantic, artifact,
freshness, and final-acceptance results for the corrected subject. Re-review
uses the complete corrected tuple and writes new review and acceptance records.
Statecraft does not copy a prior pass onto changed bytes.

### 3.9 Final result and retention

The overall result is `accepted`, `rejected`, or `no-acceptance`. `accepted`
requires every policy-required dimension to pass, the required review quorum to
recommend acceptance, no unresolved required correction, and a stable subject
through record creation. `rejected` means the complete judgment ran and at least
one required dimension has the explicit state `fail`. Any required dimension in
state `unknown`, `unsupported`, `unreadable`, or `not-recorded` makes the
judgment incomplete and yields `no-acceptance` with reasons. An unknown, absent,
unsupported, unreadable, moving, or authority-changing subject also yields
`no-acceptance`.

The record names every dimension and reason. It also repeats that acceptance
does not authorize applying a patch, modifying an authored file, committing,
pushing, opening a pull request, publishing, releasing, or deploying.

Version 1 retains full raw task, result, artifact, review, correction, and
acceptance bytes in the protected run record. Export outside that boundary is
metadata-only by default: identities, digests, sizes, states, and reasons, not
content. Even metadata can reveal repository activity when values are small or
predictable. Any shared or remote export policy therefore requires a separate
privacy analysis for enumeration, correlation, and digest-guessing risks. This
spec authorizes no export.

### 3.10 Command projections

`run show` displays recorded review and correction identities without judging
them. `accept` performs and records the documentation dimensions as part of the
existing acceptance operation only when their complete prerequisites exist.
Historical records render every new field as `not-recorded`.

A later implementation may add a review-submission binding only in the same
change that implements the library operation and authority checks. No command
accepts free-form approval, rewrites an existing review, applies an artifact,
or publishes. Human and JSON output remain two renderings of one value.

### 3.11 Observable negative cases

| Case | Required result |
|---|---|
| Provider result is valid and says complete | Schema may pass; semantic review and acceptance remain independent. |
| Every intended page appears in adapter events | Adapter-observed delivery may pass; observed transport remains separately sourced. |
| Citation resolves but does not support the sentence | Citation resolution passes and claim review is `unsupported`. |
| Claim says `tested` with implementation-only evidence | Claim review is `insufficient-grade`. |
| Result omits a contradiction present in delivered context | Semantic review fails and names the omission. |
| Required claim remains `not-reviewed` | Semantic review is `not-recorded`; overall result is `no-acceptance`. |
| Executable example was not run | Example result is `unknown`, never pass. |
| External link was not authorized for checking | Link result is `not-recorded`. |
| Artifact bytes change after review | Earlier review does not bind and a new review is required. |
| Context producer reports stale | Freshness fails regardless of semantic quality. |
| Provider lists itself as reviewer | Reviewer authority is unknown or fails; it is not accepted. |
| Required authority dimension is `unknown` | Overall result is `no-acceptance`, not rejection. |
| Every dimension passes | Result may be accepted, but no application or publication authority follows. |

## 4. Out of scope

- Ratifying or implementing specs 019, 020, or this spec.
- Defining provider transport or a wire-evidence schema.
- Automatically choosing reviewers, resolving human disagreement, or replacing
  semantic review with a score.
- Network link checking without separate authority.
- Applying, staging, committing, pushing, opening a pull request, publishing,
  releasing, or deploying an accepted artifact.
- Exporting retained content or metadata to a remote system.

## 5. Resolved decisions

**2026-09-27: semantic review remains necessary.** Schema validity and citation
resolution are mechanical facts. Neither can decide whether evidence supports a
claim at its stated grade or whether an artifact communicates faithfully.

**2026-09-27: transport is optional only by explicit policy and never inferred.**
Adapter observation remains useful testimony, while independent transport
evidence stays `not-recorded` until an adopted contract supplies it.

**2026-09-27: shared retention defaults to metadata only.** Full bytes stay in
the protected local run record. A later export needs privacy analysis because
small or predictable identities may still disclose content through guessing.

**2026-10-22: the review and correction records' closed shape.** The
run-record half (`crates/statecraft-run/src/documentation/review.rs`) fixes
section 3.4's members: `subject` is section 3.1's core tuple (repository,
trusted base, run, attempt, workspace candidate, manifest, task, result
custody and result-schema digests, every artifact's logical path and digest,
the admitted evidence references, and the policy digest); `reviewer` is an
identity, an `authority` of `repository-owner` or `owner-delegated-reviewer`
and the `authoritySource` it cites; each claim and each artifact carries a
disposition and reasons (an artifact is `acceptable`, `needs-correction`,
`rejected` or `not-reviewed`); requested corrections name a claim id or a
logical path. A correction names its subject, the digest it `supersedes`,
author, reason, and at least one changed claim, changed artifact identity or
corrected result. Both are sealed by `statecraft/documentation-record-digest/1`
over every other member, so neither names itself. Each is one evidence
record (`documentation-review`, `documentation-correction`), read back in
record order; a record that does not decode or whose digest does not hold is
listed `unreadable`, never dropped.

**2026-10-22: what the judgment reads, and the choices section 3 left
open.** `crates/statecraft-acceptance/src/documentation_acceptance.rs`
judges over inputs it is handed and fetches nothing. The trusted-base policy
is `statecraft/documentation-acceptance-policy/1` at
`.statecraft/documentation-acceptance.json`, read at the trusted base: its
reviewer grants (identity, authority, optional `notAfter`), the `quorum` of
eligible reviews that must recommend `accept` with no requested correction,
whether `observedTransport` is required, and `maxArtifactBytes`; its digest
is the subject's policy digest. Optionality: `semanticReview` follows the
recorded task's `reviewPolicy.semanticReview`, `observedTransport` follows
the policy, and every other dimension is required. `observedTransport` is
`not-recorded` until an adopted evidence contract supplies it. Citation
resolution, evidence admission and packet freshness come from spec 019's
judged facts over the claims' citations and evidence references;
documentation-manifest freshness is the producer's `documentation manifest
freshness` answer, `not-recorded` when no manifest is supplied. A review
binds only when it names the effective subject, which is the result's
artifacts with each binding correction applied in record order; a reviewer
whose identity is the result's provider or adapter fails `authority`, and a
reviewer with no grant, a conflicting grant, another authority or an expired
grant is `unknown`. Artifact validation reads retained bytes by digest, and
checks presence exactly once, digest, size, UTF-8 text and every internal
`#anchor` link against the artifact's headings; external links are not
checked, because the task names no link policy and this spec grants no
network. `publicationBoundary` fails when an expected artifact path is among
the candidate's changed paths, and is `not-recorded` when those are not
known. A required `fail` is `rejected`; any other non-pass is
`no-acceptance`.

**2026-10-22: a dimension's name as a value.** Section 3.3 names each
dimension as an identifier, and two of them, `semanticReview` and
`observedTransport`, are also member names in the task and the policy, where
they stay camelCase. Where a dimension is a value, in the judgment's
`dimension` member and in its reasons, it is written in kebab-case
(`task-binding`, `artifact-validation`, ...), because spec 006's JSON naming
convention makes enum values kebab-case and section 3 does not say otherwise.

**2026-10-22: how `accept` and `run show` bind the judgment.** `accept`
judges an attempt's documentation only when its intent records a spec 020
task, beside the existing verdict and never part of it, and appends the
judgment as one `documentation-acceptance` evidence record; an attempt with
no task records nothing and renders `not-recorded`. It reads the policy with
`git show <base commit>:.statecraft/documentation-acceptance.json` in the
registered repository, the candidate as the workspace's `HEAD`, the changed
paths as the workspace's tracked and untracked differences from the base,
and resolves the claims' citations through spec 019's checks with the
operator's `accept --context` checkouts and evidence admissions. Until spec
022's workflow supplies a documentation manifest, its freshness is
`not-recorded`, so no documentation is accepted through the command before
then. Spec 020's `acceptance` word reads `judged` when this judgment ran.
`run show` lists every review and correction identity, an unreadable one
included, without judging it. No review-submission verb is added: section
3.10 permits one only with the library operation and authority checks it
binds, and a review record is written through `record_review` until a later
change adds one.

## Verification

Implementation acceptance is the deterministic fixture surface `V-1` names,
run through `make verify SPEC=021`. The owner ratified this spec on 2026-10-01 in
commit `1a0d3a155da9615fb83f72f106570c2964594703`; its frontmatter has
been `status: approved` since that commit. Fixtures use retained local bytes,
fixed review authorities and fixed clocks; they make no provider call, network
read, repository mutation, publication or spend.

```verify:cli
cargo test -p statecraft-run --test documentation_review
cargo test -p statecraft-acceptance --test documentation_acceptance
cargo test -p statecraft-cli --test documentation_acceptance
```

These establish the twelve dimensions, reviewer authority and quorum, claim
grades as review dispositions, artifact and freshness checks, corrections
that unbind earlier reviews, and the boundary. They do not judge whether a
real document is good: that is a reviewer's record, and this spec only
refuses to let anything else stand in for it.
