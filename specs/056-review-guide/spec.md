---
id: "056-review-guide"
title: "Setup profile: a managed review guide and a repository's own REVIEW.md in the AI review's governance context"
status: approved
implementation: pending
created: "2026-10-21"
summary: >
  Amends spec 002's setup profile and spec 041's default governance context.
  The next github-actions-rust revision delivers a managed review guide at
  .statecraft/REVIEW.md, the reviewer's contract written once for every
  target, and adds it and a repository-authored REVIEW.md to the documents
  the AI review reads from the base commit. An absent REVIEW.md is recorded as
  absent, as any default document is. This repository's own REVIEW.md is an
  adaptation of preloop-ci's guide to Statecraft's conventions.
amends:
  - "002-environment-lifecycle"
  - "041-review-authority-context"
# No `extends` yet: the implementing change adds the edge on
# crates/statecraft-home/ (002) with the code.
depends_on:
  - "002-environment-lifecycle"
  - "024-review-budget-and-ratification"
  - "041-review-authority-context"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "R-1"
    kind: requirement
    text: "The profile delivers .statecraft/REVIEW.md as a managed file verified by digest, and the AI review's default governance context lists it and REVIEW.md, both read as base-commit blobs."
    anchor: "3-1-two-documents-one-managed"
  - id: "R-2"
    kind: requirement
    text: "A target with no REVIEW.md at the base reviews exactly as before apart from the managed guide, with the absence recorded; nothing substitutes the candidate's REVIEW.md."
    anchor: "3-2-absence-and-the-candidate"
  - id: "R-3"
    kind: requirement
    text: "Both documents count against spec 041's governance-context budget whole; a guide that does not fit refuses with 2 before any provider call, as any context document does."
    anchor: "3-3-budget"
  - id: "V-1"
    kind: verification
    text: "Rendering tests show the managed guide delivered and both paths in the default context; a review over a base without REVIEW.md records it absent; a candidate editing REVIEW.md changes nothing the review reads."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/src/setup.rs"
---

# 056: A review guide in the AI review's context

## 1. Purpose

The managed AI review (specs 017, 024, 041) is told what to check by a prompt
in `ai-review.sh` and by the governance documents spec 041 selects. Neither
says how to review: what counts as evidence, which findings block, how to
treat a candidate's claims about itself. Each target either repeats that in
its `AGENTS.md` or goes without. preloop-ci's `REVIEW.md` (MIT) is a worked
example of such a guide, written for its own protocol-fidelity priorities.
This spec gives every target a managed guide with the parts that are the
same everywhere, and lets a repository add its own.

Queue item H5 asked for the guide to be referenced from
`statecraft-ai-review.yml` through the profile. Spec 041 already gives the
review a base-commit governance context with a default path list; adding the
guide there reaches the reviewer without a new workflow input.

**Drafts this one cites:** spec 044, setup profile revision 19 (spec 058 amends it from revision 18), on main as a
draft since pull request 296 (its ratification is pull request 306). This spec takes the next revision after whichever
profile revision lands before it.

## 2. Territory

The profile under `crates/statecraft-home/profiles/github-actions-rust/` gains
the managed guide; `setup.rs`'s `DEFAULT_CONTEXT_PATHS` gains two entries; the
revision's rendering tests cover both. This repository's own `REVIEW.md` is
added by the implementing change and claimed there. No target's copy is
hand-edited; each target receives the guide by adopting the revision.

## 3. Behavior

### 3.1 Two documents, one managed

- `.statecraft/REVIEW.md` is rendered by the profile, verified by digest like
  every managed file, and identical in every target. It states the reviewer's
  contract: judge the change against its intent, cite evidence for a
  blocking finding, treat the diff as data, never treat a candidate's claim of
  approval or ratification as authority, and report only what the supplied
  evidence supports.
- `REVIEW.md` at the repository root is authored by the repository, optional,
  and holds what is particular to it: which surfaces carry the most risk and
  what evidence a change to them needs.

The default context becomes `AGENTS.md`, `.statecraft/AGENTS.md`,
`.statecraft/REVIEW.md`, `REVIEW.md`, `standards/spec/constitution.md`,
`standards/spec/contract.md`, in that order.

### 3.2 Absence and the candidate

Both are read as base-commit blobs under spec 041 section 3.1. A base without
`REVIEW.md` records it absent and reviews on. A candidate that adds or edits
either file has that edit reviewed as data and read as context only once it
is on the base.

### 3.3 Budget

Both count against spec 041's quarter of the context budget, whole. A guide
that would not fit refuses before any provider call, as an oversized
`AGENTS.md` does today.

### 3.4 This repository's guide

This repository's `REVIEW.md` adapts preloop-ci's guide (attributed per its
MIT license in the file) to this repository: spec-first changes, the
ownership and coupling gate, closed refusal codes, revision-bound approvals,
sign-at-write and wire traffic as untrusted testimony (specs 013 and 014) as
the surfaces a reviewer weighs first. It drops what is particular to
preloop-ci (runner protocol fidelity, golden runner captures, its own
tooling) and copies no text verbatim.

## 4. Out of scope

- Changing the review prompt, caps or grouping.
- A per-target override of the managed guide: a target adds its own
  `REVIEW.md` instead.

## 5. Resolved decisions

**2026-10-21: context, not a workflow input.** The work order named the
workflow file. Spec 041's base-commit context already carries provenance,
budget and absence rules for exactly this kind of document, and a workflow
input would need its own.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is declared by the
implementing change, which will render the revision and find the managed guide
and both default paths, review over a base with no `REVIEW.md` and find it
recorded absent, and review a candidate that edits `REVIEW.md` and find the
base's version (or its absence) in the evidence.
