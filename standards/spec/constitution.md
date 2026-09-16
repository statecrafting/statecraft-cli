# Constitution (tier 2)

Durable principles that govern this corpus. This document is **tier 2**: it is
subordinate to the bootstrap spec, whose `unamendable` anchors it may not
contradict, and it governs all ordinary specs.

> **Ratification status, 2026-09-16: principles VI to XIII are DRAFT.**
> Principles I to V are spec-spine's corpus principles, scaffolded by
> `spec-spine init` and adopted as written. Principles VI to XIII are this
> project's own, drafted from the founding handoff and from constraints
> inherited from the archived predecessor; they are claimed by spec
> `001-boundaries-and-authority`, which is itself `draft`. Nothing below has
> been ratified by the repository's owner, and **no principle below is frozen**:
> spec 000 deliberately holds no product anchor until `001` is ratified. The decision record is
> [docs/decisions/00-founding-decisions.md](../../docs/decisions/00-founding-decisions.md);
> what each principle answers is traced there.

**Normative hierarchy (highest wins):**

1. the bootstrap spec (`000`): non-overridable.
2. this constitution.
3. the contract: a normative summary of the bootstrap spec.
4. ordinary specs: feature-level claims within this envelope.

When two specs conflict, resolve in this order, then by the typed authority
graph.

---

## I. Markdown-only authored truth

Authored truth lives only in markdown with YAML frontmatter. If a fact governs
the system, it is written in a `spec.md` (or a standards document), never in a
derived artifact.

## II. Compiler-owned JSON machine truth

Machine-consumable truth is emitted by the compiler into the derived tree and is
read only through `spec-spine` subcommands. Hand-editing a derived artifact is a
workflow violation; ad-hoc parsing of one (`jq`/`awk`/`sed`) is equally
forbidden, because a typed read fails at the deserializer instead of silently
encoding a stale assumption.

## III. Spec-first development

A change to behavior begins with a change to a spec: the spec declares the units
it owns and the typed edges to its neighbours before the code is written. The
coupling gate enforces this at PR time. The escape valve is a named, scoped
waiver in the PR body, never a silent edit to an owner spec.

## IV. Determinism and validation

Every artifact-producing function is a pure function of (config, file contents):
the same inputs produce byte-identical output. Validation is mechanical, so
staleness is detectable by content-hash comparison alone.

## V. Legacy as evidence

Code that predates a governing spec is evidence, not a violation: a spec
claiming it declares `origin.retroactive: true` rather than masquerading as a
fresh `establishes` claim. Code adopted from outside the corpus is specced **as
found**, and the behavior the adopting spec would not have chosen is recorded
under a `## Known defects` heading. A defect recorded there is not thereby
blessed: it is what a later spec is written against.

---

## VI. A declaration of completion is not an acceptance

An agent's report that it finished is a **claim**: evidence of what it intended,
never evidence of what happened. Acceptance is a judgment this product forms
independently, over **identified candidate bytes** and against an **identified
trusted policy and base revision**, using checks it ran itself.

A run whose acceptance was not independently evaluated records **no** acceptance.
It never records a passing one. Where a richer structured outcome exists, that
outcome is what is read; a zero exit code is not inferred to mean accepted.

Proposed for freezing as `independent-acceptance` when this principle is
ratified; spec 000 section 5 says why it is not frozen yet.

## VII. A candidate cannot enlarge its own authority

Policy, the verifier, hooks, the check suite and the acceptance instructions are
read at the **trusted base revision**, never from the candidate under
evaluation. A change that proposes to alter any of them is an **authority
change**: it is separated from ordinary implementation and decided by a human,
and that decision is recorded outside the candidate.

A candidate that modifies its own governing rules has not widened its
permissions; it has produced a change that cannot be accepted by the rules it
was judged under.

Proposed for freezing as `no-self-granted-authority` when this principle is
ratified; spec 000 section 5 says why it is not frozen yet.

## VIII. Name the enforcement, or do not claim the protection

Prompt text, instructions, a checked-in convention and a separate worktree are
**not** security boundaries against hostile code. Every protective claim in this
corpus names the mechanism that enforces it and the residuals the mechanism
leaves unclosed.

A required capability that cannot be enforced causes **refusal**. Degradation is
permitted only when it is explicit, recorded on the attempt, and visible in the
outcome. No default bypasses permission enforcement; no flag exists whose only
effect is to remove a guard.

## IX. Outcomes are recorded outside the child, and recovery reconciles before it repeats

Refusals, interruptions and results are recorded by the **supervisor**, in a
place the supervised process cannot reach. `interrupted`, `refused`, `failed`,
`cancelled` and `completed` are distinct outcomes with distinct meanings, and a
retry is a **new attempt appended** to history: it never rewrites or erases what
came before.

State is recovered by reading the record, not by trusting memory. An effect
whose outcome is unknown is **reconciled before** it is retried. This product
makes no exactly-once promise about an external effect unless it names the
mechanism that delivers it.

Proposed for freezing as `evidence-outside-the-child` when this principle is
ratified; spec 000 section 5 says why it is not frozen yet.

## X. Intent, execution, verification and acceptance are separate records

Each carries the provenance of what it concerned: the policy, the verifier, the
base and the candidate, each identified by revision.

Original evidence **bytes** are preserved, together with the named hash
construction that identified them; a canonical record hash never substitutes for
a byte digest. Integrity, signature, issuer trust and subject binding are
reported **separately**, each from its own check, and admission is kept apart
from all of them. Trust roots are supplied independently of the evidence they
judge, so evidence never authorizes itself. **An unperformed check is `unknown`,
never a pass**, and absence is named rather than rendered as success.

## XI. A capability is declared and qualified, never assumed

Every execution adapter declares what it supports and what it cannot do. A
declaration becomes trustworthy by **passing one negative suite**, recorded per
binary version; an adapter with no such record runs, and is labelled unqualified
everywhere it appears.

A similar prompt, a copied skill, a shared vocabulary or a familiar file layout
establishes nothing about another provider's hooks, permissions, evidence or
cost controls.

## XII. Public claims are graded

**Specified**, **implemented**, **tested** and **released** are four different
statements, and this project makes them separately. Nothing here claims a grade
it cannot show the evidence for, and no grade is inferred from the completeness
of a specification.

## XIII. The local product owes nothing to a hosted one

The first useful workflow runs on one machine, on one repository, with no
account and no network service. A hosted service may later consume contracts
this product publishes explicitly; it never becomes a precondition for the local
product, and no local guarantee is deferred to it.

---

## Amendment

This constitution is changed by an ordinary spec that is `approved`, **claims
the affected text as an authority unit**, and contradicts no `unamendable`
anchor of the bootstrap spec.

The claim uses the ordinary ownership vocabulary over a section unit of this
file: `establishes` for a principle the spec adds, `refines` (with a named
`aspect`) for one it tightens, `co_authority` for one genuinely shared.

```yaml
refines:
  - aspect: "legacy-as-evidence"
    unit: { kind: section, file: "standards/spec/constitution.md", anchor: "v-legacy-as-evidence" }
```

The anchor is the heading slug, so `## V. Legacy as evidence` is
`v-legacy-as-evidence`. `amends` is **not** the instrument: its targets are spec
ids, and this file is not a spec.

Unlike an amended `spec.md`, which is a record of what the corpus held when it
was ratified and is therefore never edited to mention its successors, this
document is a standing statement of what is true now. It is edited in place, and
its history lives in the specs that claimed each section, and in git.
