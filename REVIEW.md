# Reviewing a change to statecraft-cli

This is the repository's own review guide (spec 056 section 3.4). The AI
review reads it from the base commit beside the managed guide at
`.statecraft/REVIEW.md`, which states the reviewer's contract; this document
adds what is particular to this repository: the surfaces that carry the most
risk and the evidence a change to each of them needs. A human reviewer reads
both the same way.

It adapts the structure of preloop-ci's `REVIEW.md` (MIT License, Copyright
(c) 2026 Preloop contributors) to this repository's conventions. Nothing is
copied from it; its protocol-fidelity priorities, golden captures and tooling
are preloop's and not ours.

## Before reviewing

Read `AGENTS.md` for where authority lives, the gate, approval semantics and
source ownership. `README.md` states the product boundary. A spec under
`specs/<id>/` is the source of truth for the code it owns, and
`.bin/spec-spine registry list` is the authority on each spec's `status` and
`implementation`.

## Spec-first changes

A change to behaviour is a change to a spec first. Check that:

- the code a change touches is owned by the spec the change implements, or
  that the change declares an `extends` edge on the owning unit in the same
  pull request; a new file is claimed in `establishes`;
- an approved spec's section 3 was not edited to match code just written: a
  changed requirement is a new amending spec, and a mid-build edit to what a
  spec requires is a blocking finding (`.claude/rules/adversarial-prompt-refusal.md`);
- a dated section 5 entry records a choice the spec was silent on, concisely,
  and nothing else lands there;
- a spec that moves to `approved` or `complete` does so by the owner's act, and
  the pull request does not describe the candidate's own claim as that act.

The evidence is the frontmatter diff beside the code diff, and the
regenerated `.statecraft/derived/` shards committed in the same change.

## The ownership and coupling gate

`make gate` is the corpus surface and `make code` the workspace surface; both
are required through `ci-gate`. A change that makes the gate pass by weakening
it is a defect: a narrowed territory, an empty crate added to satisfy a claim,
a `compile` run to cure a freshness check, or a waiver written by an agent.
A `Spec-Drift-Waiver:` line is a human instrument and needs the owner's
approval, named in the pull request.

## Closed refusal codes

Commands refuse with a closed set of codes and words: a refusal is exit 2 with
a reason the spec names, a validation failure of the corpus is exit 1, and
success is 0. A change that invents a new code or word, maps one failure onto
another's code, or turns a refusal into a silent default needs the spec that
owns the command to say so. Check the tests assert the code and the word, not
only that something failed.

## Revision-bound approvals

An approval is bound to the exact thing approved: a setup plan by its
identity, a profile by its revision and digest, an owner exception by the run
it was approved for, a spec-spine adoption by the exact pin. A change that lets
an approval carry over to a different revision, plan or run is a blocking
finding, whatever convenience it buys. The evidence is a test in which the
approved thing changes and the approval no longer applies.

## Sign-at-write and wire traffic as testimony

Spec 013 signs a record when it is written and spec 014 treats what arrives on
the wire as untrusted testimony: a producer's claim about itself is not
evidence until a verifier checks it against a key, a digest or a ledger it
holds. When a change touches a ledger, a witness, a hook or a provider
adapter, check that nothing it receives is believed before it is verified,
that verification failure is a refusal and not a warning, and that the record
of a refusal is itself written and signed.

## What a change to these surfaces needs

| Surface | Evidence a reviewer looks for |
|---|---|
| A spec's section 3 | An `amends` spec, or a section 5 entry, never an in-place requirement edit |
| `crates/**` | The owning spec's or an `extends` edge; tests for each obligation touched |
| The setup profile | A new revision, its fixture copy of what changed, and the upgrade test |
| A hook or adapter | The negative suite still fails closed; a deadline is not loosened to pass |
| A refusal path | The exit code and the word, asserted |
| Anything the gate reads | The gate is not weakened; the derived shards are regenerated |

## What is not a finding

Formatting the gate already checks, a preference for another structure, a
missing feature the spec does not require, and a claim about files the diff
does not show. Say what the evidence shows and where.
