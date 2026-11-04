---
id: "068-finding-severity"
title: "AI review findings carry a severity, and an answered low finding passes ci-gate without the owner exception"
status: draft
implementation: pending
created: "2026-11-02"
summary: >
  Amends spec 024's review and the ci-gate rule of spec 062. Each AI review
  finding carries a severity (`low`, `medium`, `high`) and a category. A
  head whose findings are all `low`, each answered by a `Review note
  (<sha8>):` line in the pull request's body, passes ci-gate without the
  owner's review exception; a finding of any other severity, and any change
  that touches correctness, security, authority or ratification, still needs
  it. Known false positives are handled through review.context_paths.
amends:
  - "024-review-budget-and-ratification"
  - "062-review-verdict-status"
depends_on:
  - "024-review-budget-and-ratification"
  - "041-review-authority-context"
  - "062-review-verdict-status"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Every finding in the review's evidence record carries a severity, one of `low`, `medium` or `high`, and a category, one of `correctness`, `security`, `authority`, `ratification` or `other`; a finding without either is read as `high`."
    anchor: "3-1-severity-and-category"
  - id: "R-2"
    kind: requirement
    text: "ci-gate passes a head whose findings are all `low` and `other`, each answered by a `Review note (<sha8>):` line for that head in the pull request's body, without the review exception; any other finding, or one left unanswered, keeps the exception required."
    anchor: "3-2-an-answered-low-finding"
  - id: "R-3"
    kind: requirement
    text: "A known false positive is handled by a document named in review.context_paths, read at the base commit; the candidate's copy never answers its own finding, and no separate list is built."
    anchor: "3-3-known-false-positives"
---

# 068: finding severity and answered low findings

## 1. Purpose

On 2026-11-02 the owner reported, for a consuming repository, that 43% of
pull-request runs needed the owner's review exception. Many findings that
held a run were low-stakes: a comment that named a file the change's own
stack adds, a wording nit, a false positive about context the reviewer does
not see. Every one of them costs an owner approval and a rerun. This spec
lets the author answer a low finding in the open, on the reviewed head,
while keeping the exception for everything that matters.

**Status: draft, for the owner to ratify.** The owner's note of 2026-11-02
lists this item as "For the owner to ratify"; nothing here is built until
it is.

## 2. Territory

The review prompt and its evidence record (spec 024, the profile's
`ai-review.sh`), and ci-gate's reading of a `findings` verdict (spec 062,
the profile's `ci-gate.sh`). A new profile revision renders both. No
product verb changes.

## 3. Behavior

### 3.1 Severity and category

The review prompt asks for each finding's severity (`low`, `medium`,
`high`) and category (`correctness`, `security`, `authority`,
`ratification`, `other`), and the evidence record keeps both. A finding
whose severity or category is missing or not one of these words is read as
`high`, so a malformed answer never relaxes the gate.

`low` covers wording, style, redundancy and hypothetical misuse: a finding
that names no input on which the change misbehaves. A finding that names
such an input is `medium` or `high`, whatever its category.

### 3.2 An answered low finding

A review note is a line of the pull request's body that begins
`Review note (<sha8>): <finding path>` and goes on with the answer, where
`<sha8>` is the first eight characters of the reviewed head. ci-gate passes
a head without the exception when every finding is `low` and `other` and
each has a review note for that head. The body is the author's, and only
its state when ci-gate runs counts. A finding of any other severity or category, a
finding without a note, a note for another head, and any change that spec
062 already routes to the exception (an authority change, a ratification)
keep the exception required. The gate records which notes it read.

### 3.3 Known false positives

Known false positives are handled through `review.context_paths` (spec
041): the project names a document of them among the context paths, and the
reviewer reads it at the base commit with the other governance context, so
the finding is not raised. A change cannot add the entry that answers its
own finding, because the candidate's copy is never read. No separate list
or matching rule is added.

## 4. Out of scope

- Rerunning a job whose failure matches a runner fault (the owner's item D,
  later).
- Any change to which changes need the exception besides `low` findings.

## 5. Resolved decisions

**2026-11-02: drafted from the owner's note, for ratification.** The note
reads, for this item: "For the owner to ratify: findings get a severity. A
head whose findings are all low and each answered by a Review note (<sha8>)
passes ci-gate without the exception; correctness, security, authority and
ratification changes still need it. Optionally, a project list of known
false positives read at the base." Open questions for the owner: whether
`medium` should ever pass.

**2026-11-04: updated from the owner's rustev handoff, still a draft.** The
handoff (rustev m1 2026-11-02, item C) settles three points, folded into
section 3. A low-only head passes when the pull request's body has a
`Review note (<sha8>):` line for each finding, so a note is a body line and
not a comment. `low` covers wording, style, redundancy and hypothetical
misuse. False positives are handled through `review.context_paths`, so no
separate list is built.

## Verification

Declared by the implementing change once ratified: a rendered workflow
whose review returns only answered `low` findings passes ci-gate with no
exception run; one `medium`, an unanswered `low`, a note for another head,
or a candidate-added false-positive entry each keeps the exception required.
