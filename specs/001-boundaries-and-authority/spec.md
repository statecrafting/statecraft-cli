---
id: "001-boundaries-and-authority"
title: "The product boundary, the component owners, and the separation of authority"
status: approved
# Records decisions and owns prose. There is no code behind it and never
# will be, so `n-a` keeps it out of the ready set (spec-spine 045).
implementation: n-a
created: "2026-09-16"
summary: >
  What this product is, what it is not, and who owns each capability it
  depends on. Fixes the claim vocabulary (specified / implemented / tested /
  released), the two interaction modes and which one is built first, the rule
  that separates an authority change from an implementation change, and the
  reuse disposition for every shared component this repository would otherwise
  reimplement. Owns the product half of the constitution and the two founding
  records under docs/. It owns no code.
establishes:
  - { kind: section, file: "standards/spec/constitution.md", anchor: "vi-a-declaration-of-completion-is-not-an-acceptance" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "vii-a-candidate-cannot-enlarge-its-own-authority" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "viii-name-the-enforcement-or-do-not-claim-the-protection" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "ix-outcomes-are-recorded-outside-the-child-and-recovery-reconciles-before-it-repeats" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "x-intent-execution-verification-and-acceptance-are-separate-records" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "xi-a-capability-is-declared-and-qualified-never-assumed" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "xii-public-claims-are-graded" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "xiii-the-local-product-owes-nothing-to-a-hosted-one" }
  - "docs/decisions/00-founding-decisions.md"
  - "docs/design/00-boundaries-and-reuse.md"
  - "scripts/check-authored-content.sh"
depends_on:
  - "000-bootstrap"
---

# 001: The product boundary, the component owners, and the separation of authority

## 1. Purpose

This repository is a local environment for **governed agent work**: it prepares a
repository, selects a unit of work, supervises an agent through it in an isolated
workspace, judges the result independently, and retains a reviewable account of
what happened.

Four things go wrong without a written boundary, and all four went wrong in the
archived predecessor before its specs 119 to 129 corrected them: the product
grows a second spec compiler, it grows a second memory store, it accepts an
agent's own word for its outcome, and it lets the thing being judged edit the
rules it is judged by. This spec fixes the boundary so those are refusals rather
than regressions.

It owns no code. It owns the product half of the constitution and the two
founding records under `docs/`.

## 2. Territory

Constitution principles VI to XIII; `docs/decisions/00-founding-decisions.md`;
`docs/design/00-boundaries-and-reuse.md`;
`scripts/check-authored-content.sh`.

Not this spec's territory: the corpus contract (`000`), the environment
lifecycle (`002`), run semantics (`003`), the execution adapter (`004`),
acceptance and evidence (`005`).

## 3. Behavior

### 3.1 What this product owns

Repository registration and onboarding; the managed working environment inside a
registered repository; workspace preparation; supervision of an execution
adapter; run state and its recovery; independent acceptance; and the reviewable
outcome. It works on one machine, on one repository, with no account.

### 3.2 What this product does not own, and who does

| Capability | Owner | This product's relationship |
|---|---|---|
| Specification semantics, compilation, ownership analysis, freshness, verification contracts | spec-spine | Consumer of its supported commands and **its structured reports**. Never a second compiler, and never an ad-hoc read of `.derived/`. |
| Application knowledge, recall, coordination semantics | aicortex | No dependency in the first slice. `005` places the interface out of scope; the narrow, optional, one-way boundary it would be is described in `docs/design/00-boundaries-and-reuse.md` section 5, and implemented by neither side. |
| Service chassis, identity, persistent cell enforcement | Rahi | Not a required local daemon, not a process sandbox, not a desktop framework. A future hosted backend consumes explicit contracts from this product. |
| Ordered pure checks and decision composition | action-gate | Adapted at the boundary: this product supplies the required checks and the deny-by-default ceiling, because the library's own fallthrough is allow. |
| Hash-linked records, signing, verification | attest-ledger | Reused for the record envelope. Durability, crash recovery, and independently supplied issuer trust stay here. |
| Canonical JSON serialization | canonical-keysort-json | Reused. Key sorting is not strict portable-input validation; see `005`. |
| Outcome scoring | trust-window | Deferred, with no consumer. A score never overrides authorization. |
| Scoped temporal facts | fact-fold | Not this product's concern. |
| Legacy factory certificates | tenant-emit, tenant-tail | Assessed, not adopted. |

A shared name is not a shared semantic. Before any new shared mechanism is
designed here, its disposition is recorded in
`docs/design/00-boundaries-and-reuse.md` as one of: **reuse**, **extend the
owner**, **adapt at the boundary**, **recover from archive**, or **new, with a
stated mismatch**.

### 3.3 The claim vocabulary

Four grades, stated separately, never inferred from one another:

- **specified**: an approved spec describes it.
- **implemented**: code exists that a reader can run.
- **tested**: a named check exercises it, including its negative cases.
- **released**: a versioned artifact a third party can install.

Observable rule: a document in this repository that calls a behavior
implemented, tested or released names the evidence in the same sentence. A spec
being `approved` grants no grade above *specified*.

Where this repository stands, restated whenever it changes rather than left to
age:

| Date | Grade |
|---|---|
| 2026-09-16 | Specified only, and not fully: no spec but `000` was approved, and no code existed. |
| 2026-09-16 | `000` to `005` specified. `002` additionally **implemented and tested within its own territory**, the evidence being `crates/statecraft-environment/`, 59 tests, and one integration test per row of `002` section 3.10 named after the row it covers. Nothing released; `F-02` defers publication. |

The second row is deliberately narrower than "002 is implemented". The commands
`002` names are not bound to a process by that crate, so the grade it claims is
*implemented for its own territory* and nothing about a command line. `006`
proposes the binary that would change that, and until `006` is ratified and
implemented, no document here may call this product runnable.

### 3.4 The two interaction modes

They are different products and are not conflated:

- **Mode A, the agent invokes the CLI.** An agent already running in a session
  calls this product's verbs to register a repository, read ready work, prepare
  a workspace, or request an independent acceptance. The product is a tool the
  agent uses. It observes only what the agent chooses to tell it, so a refusal
  the agent does not report is invisible, and nothing in this mode can satisfy
  constitution IX.
- **Mode B, the CLI supervises the agent.** This product launches the agent
  process, holds the event stream, counts refusals itself, owns the child's
  environment, and decides the outcome. Constitution VI, VII, VIII, IX and XI
  are only enforceable here, because only here is there a supervisor.

**Recommendation: build Mode B first**, and expose Mode A as the same verbs over
the same structured output once they exist. Mode A built first produces a tool
that cannot keep its own constitution, and the archived predecessor's specs 119
and 129 are the record of discovering that after the fact. Mode A is not
abandoned: every verb `003` and `005` define emits machine-readable output, so
Mode A follows without a second surface.

This ordering is a **proposed decision** (`D-05`), not an adopted one.

### 3.5 An authority change is not an implementation change

The **authority set** of a registered repository is: its policy, **including the
lifecycle policy `003` section 3.1.1 reads from it**, its check suite, its
verifier, its hooks, the acceptance instructions the product reads, and the
environment manifest that says which files this product manages.

Membership here is a question about *this* product's trust boundary. It is not
the same question as how a base's rules would classify a change to one of these
paths, and `005` section 3.3 fixes which of the two answers each member takes.

Three observable rules:

1. Every member of the authority set is read at the **trusted base revision** of
   a run, never from the candidate. `005` fixes how the base is identified.
2. A candidate whose diff touches the authority set is reported as an
   **authority change**. It is not accepted on the strength of the check suite
   it proposes; it requires a human decision recorded outside the candidate.
3. A run never widens its own permissions. There is no flag whose only effect is
   to remove a guard, and the product ships no default that bypasses permission
   enforcement.

### 3.6 Authored-content rules that hold repository-wide

These are mechanical and checked by `scripts/check-authored-content.sh`:

1. No authored file contains U+2014. Use a colon, semicolon, comma, parentheses
   or two sentences. U+2013 is permitted only for numeric or section ranges.
2. No file in this repository, and no commit message, pull-request body, issue,
   review or release note, contains an agent-session URL or a session-tracking
   trailer, and none substitutes another tracking link.
3. `LICENSE` and any `NOTICE` are preserved. Recovering behavior from an
   archived component does not relicense it: `docs/design/00-boundaries-and-reuse.md`
   records each source's license, and an AGPL source's *behavior and fixtures*
   may be reimplemented from a written description, while its code may not be
   copied into this Apache-2.0 tree.

### 3.7 Observable negative cases

| Case | Required behavior |
|---|---|
| A document calls a behavior `implemented` with no named evidence | Refused by review; 3.3 is the rule it violates. |
| An authored file contains U+2014 | `scripts/check-authored-content.sh` exits non-zero and names the file and line. |
| An authored file contains an agent-session URL | Same check, same exit, named separately from the U+2014 finding. |
| A design proposes a new shared mechanism with no disposition row | Refused: 3.2 requires the row, including the mismatch that justifies `new`. |
| A candidate's diff touches the authority set and the run accepts it on its own suite | Violates 3.5.2; `005` is where the mechanism lives. |
| A verb is added in Mode A that has no Mode B equivalent | Refused: 3.4 makes Mode B the supervisor, and a Mode-A-only verb has no supervisor to record its outcome. |

## 4. Out of scope

Hosted platform selection; publication, release and distribution; adaptive
autonomy; a rich user interface; breadth across many providers; and any
aicortex, Rahi or hqgit integration. Each is deferred by name in
`docs/decisions/00-founding-decisions.md` and none is a prerequisite for the
first slice.

This spec also does not choose the language, runtime or packaging. That
recommendation is `D-01` in the decision record, where it can be adopted or
rejected without editing a spec.

## Verification

Each line below is one command. These assert the authored foundation, which
exists today. They assert nothing about product behavior, because none is
implemented.

```verify:cli
test -f docs/decisions/00-founding-decisions.md
test -f docs/design/00-boundaries-and-reuse.md
test -x scripts/check-authored-content.sh
scripts/check-authored-content.sh
grep -qF 'XII. Public claims are graded' standards/spec/constitution.md
grep -qiF 'specified' standards/spec/constitution.md
grep -qF 'Frozen by spec 000 as `independent-acceptance`' standards/spec/constitution.md
grep -qF 'D-01' docs/decisions/00-founding-decisions.md
grep -qF 'D-05' docs/decisions/00-founding-decisions.md
```
