---
id: "054-ai-prescreen-attestation"
title: "An AI pre-screen is an advisory attestation shown beside approvals and never counted as one"
status: approved
implementation: complete
created: "2026-10-21"
summary: >
  Records the managed AI review's verdict on a revision as an advisory
  attestation naming the model, the prompt's digest, the guidelines' digest
  and the structured verdict. It is shown beside statecraft/approval/v1 and is
  never counted as an approval. It may order a review queue. A policy may let
  a deny refuse admission only on a fail-closed path, where a missing or
  unreadable pre-screen refuses too. An AI deciding, confidence-gated
  auto-approval, approval on error, substring parsing of a verdict and a
  prompt the gated agent can influence are forbidden.
amends:
  - "005-acceptance-and-evidence"
  - "024-review-budget-and-ratification"
extends:
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-envelope/" }, nature: additive }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "005-acceptance-and-evidence"
  - "024-review-budget-and-ratification"
  - "037-closed-admission-gate"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "I-1"
    kind: invariant
    text: "No pre-screen attestation is counted as an approval or a change request, and no pre-screen verdict ever admits a revision; at most it refuses, and only under a policy that opts in on the fail-closed path."
    anchor: "3-3-advisory-never-a-decision"
  - id: "I-2"
    kind: invariant
    text: "The prompt and guidelines a pre-screen is produced under are read from the base revision, never from the candidate the gated agent produced, and their digests are in the attestation."
    anchor: "3-2-what-the-record-binds"
  - id: "R-1"
    kind: requirement
    text: "The verdict is read from the review's structured verdict block only; output without a valid block is an error, never a verdict, and an error never approves."
    anchor: "3-4-forbidden"
  - id: "R-2"
    kind: requirement
    text: "A policy that opts in to refuse on a deny pre-screen also refuses when the pre-screen is missing, unreadable or for another revision; a policy that does not opt in evaluates exactly as spec 037 does."
    anchor: "3-3-advisory-never-a-decision"
  - id: "V-1"
    kind: verification
    text: "Admission over a revision with a no-findings pre-screen and no approval refuses for the missing approval; a findings pre-screen under an opting policy refuses and under a default policy changes nothing; the spec 037 differential test still passes for every policy without the option."
    anchor: "verification"
    inputs:
      - "crates/statecraft-envelope/tests/admission_differential.rs"
---

# 054: AI pre-screen attestation

## 1. Purpose

The managed AI review (spec 024, `scripts/ai-review.sh` in the profile) already
reads every pull request and returns a structured verdict on its head. That
verdict is useful to a human reviewer, as a way to order a queue and as a
second pair of eyes, and dangerous as anything more. A model's verdict is
produced under a prompt someone wrote, about content the gated agent wrote,
and it fails in ways that look like success. This spec gives the verdict a
place in the evidence, beside approvals and visibly not one, and closes the
routes by which an advisory signal becomes a decision.

## 2. Territory

`statecraft-envelope` gains the predicate, its claim and the opt-in policy
check; the managed review profile writes the record from its existing verdict
block. The review's prompt, caps and grouping (specs 017 and 024) are
unchanged.

## 3. Behavior

### 3.1 The predicate

`statecraft/ai-prescreen/v1`, subject a revision, claim:

| Field | Meaning |
|---|---|
| `model` | The model identifier the review called. |
| `prompt` | Digest of the prompt template, read at the base. |
| `guidelines` | Digest of the review guidelines, read at the base. |
| `verdict` | `findings` or `no-findings`, from the structured block. |
| `findings` | The count; the findings themselves stay in the review. |

### 3.2 What the record binds

The prompt and guidelines are read from the base revision, as spec 024
section 3.6 already requires the review to do, so the candidate cannot rewrite
the instructions it is judged by. Their digests make the record checkable: a
pre-screen produced under different instructions has a different record.

### 3.3 Advisory, never a decision

Admission never counts a pre-screen as an approval or a change request, and a
`no-findings` verdict never admits anything. A pre-screen is shown beside the
revision's approvals in every report that lists them, labelled advisory, and
may order a review queue.

A policy may set `refuse_on_prescreen_findings`. Under it, a `findings` verdict
refuses with a new closed reason, and so does a pre-screen that is missing,
unreadable or about another revision: the option exists only in its
fail-closed form. A policy without the field registers no new check, so its
evaluation, codes and claim bytes are exactly spec 037's.

### 3.4 Forbidden

- An AI as the decider: no path admits on a verdict.
- Confidence-gated auto-approval: the record carries no confidence, and
  nothing reads one.
- Approval on error: a review that errors, times out or is skipped produces no
  record, and under the opt-in that refuses.
- Substring parsing: the verdict is the last fenced JSON block's `verdict`
  field, as `ai-review.sh` already parses it; a text containing "APPROVE" is
  not a verdict.
- Prompts influenced by the gated agent: section 3.2.

## 4. Out of scope

- Changing what the AI review reviews or how it is capped.
- Any hosted-side handling of the predicate beyond carrying it in a bundle.

## 5. Resolved decisions

**2026-10-21: the opt-in is fail-closed or nothing.** A refusal on `findings`
that admitted when the review was missing would reward breaking the review.
The only form offered refuses on both.

**2026-10-31: one closed reason, naming the state.** The opt-in refuses with
`prescreen-refused`, whose one field `prescreen` is `findings`, `missing`,
`unreadable` or `other-revision`. One code keeps a reader's rule simple (any
`prescreen-refused` is the pre-screen check) while the state stays visible.
The check is registered last, after `approval-count`, so every reason order
spec 037 fixed is unchanged.

**2026-10-31: what the record's words are.** `prompt` and `guidelines` are
text digests as the producer computes them, `sha256:` followed by the hex, so
a reader compares them and never recomputes under a guessed algorithm. The
guidelines are the governance context the review already reads at the base
(spec 041), whose digest the evidence record already states. `model` is the
identifier the reviewer's own stream names for its calls; a stream that names
none is not a review the record can describe and fails with 4, like output
without a verdict block. `findings` is a non-negative integer that agrees with
the word; a claim where it does not is unreadable.

**2026-10-31: which pre-screen a bundle reads.** Among the bundle's
`statecraft/ai-prescreen/v1` attestations, one whose subject is the revision
outranks one about another, and among those the latest by the issuer's clock
is the record. A tombstoned or undecodable claim is `unreadable`, not a
divergence of the bundle: under a default policy it changes nothing, as
section 3.3 requires, and under the opt-in it refuses.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is run through
`make verify SPEC=054`, with no provider: the envelope's pre-screen tests, the
spec 037 differential, the bundle reader, and the profile's review script
against stub reviewers.

```verify:cli
cargo test -p statecraft-envelope --test ai_prescreen
cargo test -p statecraft-envelope --test admission_differential
cargo test -p statecraft-envelope --test decision_bundle
cargo test -p statecraft-envelope --lib admission::tests
cargo test -p statecraft-home --test setup_workflows a_review_writes_the_prescreen_record
```

These evaluate a revision with a `no-findings` pre-screen and no approval and
see the missing approval refuse; evaluate a `findings` pre-screen under an
opting policy and see it refuse, and under a default policy see nothing
change, down to the claim bytes; refuse a missing, unreadable and
other-revision pre-screen under the opting policy, in memory and from a
bundle; keep spec 037's differential passing for every policy without the
option while the evaluator is fed every pre-screen state; and show the
review script writing the record from its structured block, labelled
advisory, and writing none for a skip or a stream that names no model.
