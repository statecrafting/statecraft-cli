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
  contents); a typed authority graph governs who-owns-what. This repository is
  born governed and currently holds no product code: the corpus exists before
  the first line is written, and no spec here may claim otherwise.
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
# These five are spec-spine's corpus anchors, scaffolded by `spec-spine init`
# and kept as written. This list holds NO product anchor: see section 5.
unamendable:
  - "markdown-truth-boundary"
  - "json-truth-boundary"
  - "determinism-requirement"
  - "typed-authority-graph"
  - "refusal-rule"
---

# 000: Bootstrap spec system

This is the spec that defines what a spec *is* in this repository. Every later
spec, and the constitution, sit underneath it.

This repository was created on 2026-09-16 and contains no product code.

**Why `origin.retroactive` is absent.** The scaffolded template carries
`origin.retroactive: true` for a corpus adopting code that predates the graph.
No code predates this graph, so the key would assert a history that does not
exist. Constitution V (legacy as evidence) therefore has no subject here yet,
and a spec that claims territory does so as a forward claim whose units are
expected to be unresolved until the implementing change lands.

## 1. The authoring / derived boundary

Humans author markdown; the compiler owns the JSON under `.derived/`. A derived
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

## 5. No product anchor is frozen yet

Three of this product's principles are the ones a later amendment could not be
recovered from, because such an amendment would destroy the very record that
would have shown it happened:

- constitution VI, independent acceptance;
- constitution VII, no self-granted authority;
- constitution IX, evidence recorded outside the child.

**They are deliberately not in this spec's `unamendable` list.** Their text is
owned by `001-boundaries-and-authority`, which is `draft`: an `approved` spec
freezing unratified prose would be exactly the self-granted authority
constitution VII forbids, performed by the corpus on itself.

Adding the three anchors `independent-acceptance`,
`no-self-granted-authority` and `evidence-outside-the-child` to this spec is
therefore **proposed**, and is part of ratifying `001` (decision `D-03` in
`docs/decisions/00-founding-decisions.md`). Until the owner ratifies, the
principles are ordinary draft constitution text, amendable by the ordinary
route.

## 6. What this spec does not do

It does not describe the product. The product boundary, the component owners and
the claim vocabulary are `001-boundaries-and-authority`. It owns no code and
never will.

## Verification

Each line below is one command; no line may depend on a variable another set.
These assert the corpus's own shape, which exists today. They assert nothing
about product behavior, because none is implemented.

```verify:cli
spec-spine compile --check
spec-spine lint
spec-spine registry list
test -f standards/spec/constitution.md
test -f standards/spec/contract.md
grep -qF 'No product anchor is frozen yet' specs/000-bootstrap/spec.md
! grep -qE '^  - "(independent-acceptance|no-self-granted-authority|evidence-outside-the-child)"$' specs/000-bootstrap/spec.md
! grep -qF 'Frozen by spec 000' standards/spec/constitution.md
```
