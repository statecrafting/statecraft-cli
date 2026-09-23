---
id: "000-bootstrap"
title: "Bootstrap spec system"
status: approved
# This spec defines what a spec is; it owns no code, so there is nothing
# to implement. `n-a` keeps `registry plan` from offering it (spec-spine 045).
implementation: n-a
created: "2026-09-16"
summary: >
  Foundational contract for this repository: authored truth lives only in
  markdown (+ YAML frontmatter); machine-consumable truth is compiler-emitted
  JSON only; every artifact is a deterministic function of (config, file
  contents); a typed authority graph governs who-owns-what. This repository was
  born governed: the corpus existed before the first line of product code, and
  no spec here may claim code that does not exist.
# The corpus documents that restate this spec. The contract is this spec's
# normative summary (constitution, normative hierarchy, tier 3), the templates
# are the shape it requires of an ordinary spec, and constitution principles I
# to V are the corpus half of the durable text. Product principles VI to XIII
# are owned by 001 and are not claimed here.
establishes:
  - "standards/spec/contract.md"
  - { kind: directory, path: "standards/spec/templates/" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "i-markdown-only-authored-truth" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "ii-compiler-owned-json-machine-truth" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "iii-spec-first-development" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "iv-determinism-and-validation" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "v-legacy-as-evidence" }
# The first five are spec-spine's corpus anchors, scaffolded by `spec-spine init`
# and kept as written. The three product anchors below them were added when the
# owner ratified `001`, which is what made freezing their text legitimate: see
# section 5.
unamendable:
  - "markdown-truth-boundary"
  - "json-truth-boundary"
  - "determinism-requirement"
  - "typed-authority-graph"
  - "refusal-rule"
  # Added 2026-09-16 with the owner's ratification of `001`, per decision D-03.
  # Until that ratification these three were deliberately absent: freezing prose
  # owned by a `draft` spec would have been the corpus granting itself authority.
  - "independent-acceptance"
  - "no-self-granted-authority"
  - "evidence-outside-the-child"
---

# 000: Bootstrap spec system

This is the spec that defines what a spec *is* in this repository. Every later
spec, and the constitution, sit underneath it.

This repository was created on 2026-09-16 with no product code. The crates it
holds now were written afterwards, each under the spec that claims it.

**Why `origin.retroactive` is absent.** The scaffolded template carries
`origin.retroactive: true` for a corpus adopting code that predates the graph.
No code predates this graph, so the key would assert a history that does not
exist. Constitution V (legacy as evidence) therefore has no subject here yet,
and a spec that claimed territory did so as a forward claim whose units were
expected to be unresolved until the implementing change landed. None is
forward today, and the gate now refuses one.

## 1. The authoring / derived boundary

Humans author markdown; the compiler owns the JSON under `.statecraft/derived/`
(`.derived/` until spec `002` section 3.19 moved it). A derived
artifact is never hand-edited, and it is read only through `spec-spine`
subcommands, never by parsing the JSON directly. Anchor:
`markdown-truth-boundary`, `json-truth-boundary`.

## 2. The typed authority graph

Specs declare typed edges (`establishes`, `extends`, `refines`, `supersedes`,
`amends`, `co_authority`, `constrains`, `references`) and the units they own
(file / section / symbol / directory / crate / module). Authority is derived by
walking the graph; it is never asserted in prose alone. Anchor:
`typed-authority-graph`.

## 3. Determinism

Every artifact-producing function is a pure function of (config, file contents).
The same inputs produce byte-identical output, so staleness is detectable by
content-hash comparison alone. Anchor: `determinism-requirement`.

## 4. Refusal

A change to an owned path that is not accompanied by an authoring edit to its
owning spec is refused at pull-request time, unless a human records a named,
scoped waiver in the pull-request body. An agent never writes that waiver on its
own authority. Anchor: `refusal-rule`.

## 5. The three product anchors, and when they were frozen

Three of this product's principles are the ones a later amendment could not be
recovered from, because such an amendment would destroy the very record that
would have shown it happened:

- constitution VI, independent acceptance, frozen as `independent-acceptance`;
- constitution VII, no self-granted authority, frozen as
  `no-self-granted-authority`;
- constitution IX, evidence recorded outside the child, frozen as
  `evidence-outside-the-child`.

**They were deliberately absent from this spec's `unamendable` list until
2026-09-16.** Their text is owned by `001-boundaries-and-authority`, which was
`draft`: an `approved` spec freezing unratified prose would have been exactly
the self-granted authority constitution VII forbids, performed by the corpus on
itself. The absence was the corpus declining to grant itself something.

On 2026-09-16 the repository's owner ratified `001`, which is the act decision
`D-03` named as the precondition. Adding the three anchors was part of that
row rather than a separate step, and it is recorded in the same change. What
freezes here is the *principle*, not its wording: an anchor forbids
contradiction, and ordinary editorial amendment of the surrounding prose
remains available to an `approved` spec that claims the heading.

## 6. What this spec does not do

It does not describe the product. The product boundary, the component owners and
the claim vocabulary are `001-boundaries-and-authority`. It owns no code and
never will.

## Verification

Each line below is one command; no line may depend on a variable another set.
These assert the corpus's own shape, which exists today. They assert nothing
about product behavior, which specs `002` to `006` verify.

The last three lines of the previous revision were written inverted, asserting that the three product
anchors were **absent** and that the constitution carried no freeze marker. That
was the acceptance of the unratified state, and the owner's ratification of
`001` on 2026-09-16 is what made inverting them correct. A ratification that did
not touch this block would have left spec 000 asserting a state the same change
had just ended.

```verify:cli
spec-spine compile --check
spec-spine lint
spec-spine registry list
test -f standards/spec/constitution.md
test -f standards/spec/contract.md
grep -qE '^## 5\. The three product anchors' specs/000-bootstrap/spec.md
grep -qE '^  - "independent-acceptance"$' specs/000-bootstrap/spec.md
grep -qE '^  - "no-self-granted-authority"$' specs/000-bootstrap/spec.md
grep -qE '^  - "evidence-outside-the-child"$' specs/000-bootstrap/spec.md
grep -qF 'Frozen by spec 000 as `independent-acceptance`' standards/spec/constitution.md
grep -qF 'Frozen by spec 000 as `no-self-granted-authority`' standards/spec/constitution.md
grep -qF 'Frozen by spec 000 as `evidence-outside-the-child`' standards/spec/constitution.md
```
