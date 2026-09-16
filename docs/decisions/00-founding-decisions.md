<!-- Spec: specs/001-boundaries-and-authority/spec.md -->

# Founding decisions: what is intent, what is inherited, what is proposed

Prepared 2026-09-16, at the founding of this repository. Owned by spec
`001-boundaries-and-authority`.

**Adoption status: one row of section 3 is adopted in part. Every other row is
still a recommendation.** This record exists so the three kinds of statement
below are never confused with each other. Section 1 is the repository owner's
stated intent, recorded as given. Section 2 is inherited technical constraint:
facts about tools, licenses and a predecessor, each verified in the session that
wrote this file. Section 3 is a set of engineering recommendations that need the
owner's decision before anything is built on them. Section 5 is what has been
decided since, and is the only place a row becomes binding.

As of 2026-09-16, `000-bootstrap`, `001-boundaries-and-authority` and
`002-environment-lifecycle` are `approved`; `003` to `005` are still `draft`.
The constitution's principles VI to XIII are ratified, and VI, VII and IX are
frozen as spec 000 anchors. No code exists.

## 1. Stated intent, as given

Recorded from the founding handoff. These are not decisions this record proposes;
they are the frame the rest was drafted inside.

| ID | Intent |
|---|---|
| I-01 | Rebuild the CLI as a local environment for governed agent work: project onboarding, durable context, repeatable operations, observable execution, independently checked outcomes. |
| I-02 | The direction is illustrated by claude-observatory, Frame and swamp. Illustration is not adoption of any of their primitives. |
| I-03 | Rahi and spec-spine remain foundations. aicortex is the intended knowledge and coordination application. |
| I-04 | The replacement hosted platform is undecided. hqgit is being considered separately. Neither hqgit nor a new hosted control plane is selected. |
| I-05 | The product should work locally without a hosted account. |
| I-06 | The long-term direction is that this product owns installation and ongoing maintenance of the working environment. |
| I-07 | spec-spine owns specification semantics. This product consumes its supported commands and interprets their structured reports. It never becomes a second spec compiler. |
| I-08 | aicortex owns application knowledge and recall. Repository files stay understandable without it, and no second generic memory store is built here. |
| I-09 | Do not restore the old application wholesale, and do not start a broad implementation campaign. |
| I-10 | The primary interaction mode has not been selected. |

## 2. Inherited technical constraints

Facts, each verified on 2026-09-16 in the local checkouts named. A fact here
constrains design; it does not authorize anything.

| ID | Constraint | How it was established |
|---|---|---|
| C-01 | The installed spec-spine binary is **0.18.0**, and it is the one this corpus was compiled, linted and pinned against. | `spec-spine --version` at `~/.cargo/bin/spec-spine`. |
| C-02 | **0.19.0 exists** and is not installed: the development checkout at `~/DevWork/spec-spine` is version 0.19.0 with tag `v0.19.0` present. No feature of that tree may be described here as available. | `git tag`, `Cargo.toml` in that checkout. |
| C-03 | spec-spine 0.18.0's lifecycle is `status` in {draft, approved, superseded, retired} and `implementation` in {pending, in-progress, complete, n-a, deferred} or absent. `approved` plus `pending` is a work order; `draft` is never a claim about code; `approved` with an absent `implementation` makes an unresolved unit an **error**. | `standards/spec/contract.md`, scaffolded by the installed binary. |
| C-04 | A corpus with no code is a supported steady state. `index coverage --fail-on-untraced` **refuses** on a code-free tree rather than passing vacuously, so it must not be in this repository's gate yet. | `~/DevWork/spec-spine/docs/specify-first.md`. |
| C-05 | `compile --check` compares the corpus against the **committed** shard trees. Running it immediately after a plain `compile` in the same job passes unconditionally and proves nothing. | Same source, and the adoption guide's CI note. |
| C-06 | The shared Rust foundations are Apache-2.0 at version 0.1.0: `action-gate`, `attest-ledger`, `canonical-keysort-json`, `trust-window`. `canonical-keysort-json` is Rust only, with no npm package. | Each repository's `Cargo.toml` and README. |
| C-07 | `action-gate`'s evaluator returns the first decision and otherwise **allows**, including over an empty registry. A consumer must supply the required checks and its own deny-by-default ceiling. | Reuse assessment, with the core evaluator cited. |
| C-08 | `attest-ledger`'s anchor verifier checks against the public key **embedded in the anchor**. Signature validity therefore establishes no independently trusted issuer. | Same. |
| C-09 | The archived `governance-native` addon declares **AGPL-3.0**. Its `portable.rs` holds the strict portable-input scanner (duplicate keys, non-integer numeric tokens, unsafe integers, invalid Unicode, rejecting `-0`). `kernel-native` is Apache-2.0. | `Cargo.toml` of each, under `~/DevWork/archive/statecrafting/addon/`. |
| C-10 | The predecessor's corpus is 76 specs under `~/DevWork/archive/statecraft-cli`: a Rust umbrella binary plus TypeScript/Bun members, with Rust ports 111 to 118 and the security specs 128 and 129 merged. Its specs 126, 127, 130, 131, 132 remained `draft`. | The archived checkout's `specs/` frontmatter. |
| C-11 | The predecessor **measured**, not hypothesized, three failures this corpus is shaped by: a foreign-origin browser POST disarmed a fixture project; a fabricated token in the daemon environment reached the gate suite, hooks and the broker's push; a driven session could publish around the broker via a keyring-authenticated `gh` or via SSH. | Archived specs 128, 129, 125. |
| C-12 | This repository was created with GitHub's **Rust** `.gitignore` template and an Apache-2.0 `LICENSE`, at one commit, with no other content. The template is a hint from repository creation, not an adopted language decision. | `git log`, the tracked `.gitignore`. |
| C-13 | The ecosystem decision package's rows G-05 to G-07 fixed the four evidence dimensions, the independent-trust rule and the byte-reference rule. Its CLI rows were adopted by the owner on 2026-09-12 **for the predecessor repository**. | `~/DevWork/grand-refactor/07-revision-4-decision-package.md` sections 2 and 5. |
| C-14 | `tenant-tail` and `tenant-emit` are implemented and released (reported v0.4.0 and v0.3.0), and their contracts are tied to the retired factory's run-directory, stage and certificate model. Release existence is not registry availability or fitness. | Reuse assessment; releases not independently re-verified here. |
| C-15 | **spec-spine offers a `draft` spec as ready.** `registry plan` on this corpus names `002-environment-lifecycle` ready while every product spec is `draft`, because the lifecycle table makes `draft` plus `pending` schedulable. Ratification is therefore a project rule, not something the tool withholds. The predecessor reached the same conclusion and answered it with a per-project lifecycle policy (its spec 123). | `spec-spine registry plan` run against this corpus on 2026-09-16. |
| C-16 | **Seven spec-spine specs this product wants are approved and complete on spec-spine `main` and in no release**, including the pinned 0.18.0 and the newer 0.19.0: 087 authority snapshot, 088 change classified under the base's rules, 090 commit-boundary hook, 091 two ready specs can collide, 092 a mode-only or binary change is a change, 097 governed scope is declared. 098 (a blocking claim is not a stale shard) is `draft` and complete, landed 2026-09-16. | `git tag --contains` on each spec's first commit in `~/DevWork/spec-spine` returned no tag; `v0.19.0` is dated 2026-09-13. |
| C-17 | spec-spine's design note 06 (`docs/design/06-harness-and-distribution-2026-09.md`, 2026-09-15, **proposed, nothing filed**) assigns: repository lifecycle policy and gate commands to the adopting repository; harness package identity to spec-spine; and **recording which harness revision a worker actually resolved to this product**. It proposes a versioned namespaced package pinned by a repository declaration, and records the name-precedence trap (a personal skill resolves before a project skill of the same name). It explicitly does **not** choose how a lifecycle policy is spelled. | That file, sections 2, 3.2, 3.3, 3.6, 3.7. |
| C-18 | `spec-spine couple` compares two commits (`--base`/`--head`), so it cannot see a change being staged. It is a pull-request gate, not a pre-commit one. | Its flags, and the adoption guide's CI example. |

### The spec-spine facts this product consumes, and their release state

Kept as a table so specs `003` and `005` cannot quietly fill a gap with local
code. "Available" means: carried by a release this repository's pin admits.

| Fact needed | spec-spine source | Available at =0.18.0 | What this product does meanwhile |
|---|---|---|---|
| Corpus compiles; registry and index freshness | `compile`, `index`, `check` | **Yes** | Consumed directly. |
| The ready set and its blockers | `registry plan` | **Yes** | Consumed directly; eligibility is filtered by `003` section 3.1.1. |
| Which authority-set members a change touched | **088** | No (C-16) | Records `not-recorded`, names the gap, and still refuses to accept on the candidate's own suite. Builds no classifier (`005` section 3.3). |
| What the verifier read, as a snapshot | **087** | No (C-16) | Not consumed. No local substitute. |
| Two ready specs collide | **091** | No (C-16) | Not needed: one live attempt per repository (`003` section 3.7). Reopens `F-10`. |
| A mode-only or binary change is a change | **092** | No (C-16) | Not consumed; noted because it affects what `couple` sees. |
| Declared governed scope | **097** | No (C-16) | Not consumed. |
| Exit 2 can mean a blocking claim, not a stale shard | **098** | No, and `draft` (C-16) | `AGENTS.md` states both readings of exit 2 rather than the one 0.18.0 implies. |
| Harness package identity and revision | note 06 section 3.2 | No, unfiled (C-17) | Receipt field present, reading `not-recorded` (`005` section 3.4). |
| Lifecycle policy as a queryable fact | note 06 section 3.6 | No, unfiled (C-17) | Read from the target where declared; otherwise default plus an explicit override (`003` section 3.1.1). |

## 3. Proposed decisions, none adopted

Each row is a recommendation with its reason. Each can be adopted, amended or
rejected without editing a spec, which is why they live here and not in one.

### D-01: Language, runtime and packaging

**Recommendation.** Rust, one workspace, one binary named `statecraft`,
Apache-2.0. Distribute prebuilt per-target archives with checksums from a tag,
plus a shell installer; publish the binary crate to crates.io so
`cargo install` works. **No user-interface framework in the first slice**, and no
desktop shell.

**Reasons.** Every shared foundation this product would reuse is Rust and
Apache-2.0 (C-06), and reaching them from another runtime means either a native
binding layer or a reimplementation, which is how the predecessor ended with two
canonical-JSON implementations under a product whose whole claim was verifiable
hashes. The tool this product must interoperate with most closely, spec-spine, is
a Rust binary. A supervisor that owns a child's process tree, its constructed
environment and its deadline is a job for a language with no runtime of its own
between it and the operating system. The predecessor independently converged on
this: its umbrella was Rust from its spec 102, and its specs 111 to 114 ported
the members to Rust.

**Against.** A single-binary Rust product is slower to prototype than a scripted
one, and the predecessor's engine and web UI were TypeScript, so any code
recovered from them is a rewrite rather than a move. That cost is paid once, at
the start, which is now.

**Explicitly not decided by this row.** Whether a read-only observation surface
is eventually served by this binary, and in what form. Deferred as F-04.

### D-02: Repository layout

**Recommendation.** A Cargo workspace whose crates match the spec boundaries:
`statecraft-cli` (the binary), `statecraft-environment` (002),
`statecraft-run` (003), `statecraft-adapter` (004),
`statecraft-acceptance` (005). Provider adapters are separate crates added by
their own specs.

**Reason.** The spec corpus is the module boundary, so the coupling gate has
something real to hold, and spec 004's rule that no provider name appears in the
adapter seam becomes a compile-unit fact rather than a review convention.

### D-03: The corpus itself

**Status: adopted in part, 2026-09-16.** The owner ratified `001` and `002`,
and the three anchors this row carries were added to spec 000 in the same
change. `003` to `005` were deliberately left `draft`, so each meets one more
reading before code is written against it. The remainder of this row stays open.

**Recommendation.** Ratify specs `001` to `005` as a set, or amend them first.
Until then this repository is specified only, and partially.

**Reason.** A corpus its owner has not read should not be dispatched from.
Flipping a spec to `approved` is the act that records agreement; per C-03 it
should carry `pending` at the same time so it becomes a work order rather than a
settled claim.

**This row also carries the three `unamendable` anchors.** Spec 000 section 5
deliberately holds **no** product anchor: freezing constitution VI, VII and IX
while their text is owned by a `draft` spec would be the corpus granting itself
authority, which is what constitution VII forbids. Ratifying `001` is what makes
`independent-acceptance`, `no-self-granted-authority` and
`evidence-outside-the-child` addable to spec 000's list, and adding them is part
of this row rather than a separate act.

**Note the gap this row closes, per C-15.** spec-spine already offers a `draft`
plus `pending` spec as ready, so the tool does not withhold unratified work. The
guard is a project rule, stated in AGENTS.md, and later a per-project lifecycle
policy this product would carry for the repositories it drives. Until then,
`registry plan` naming a spec is not permission to build it.

### D-04: Who installs the working environment

**Recommendation.** Adopt the transition contract in spec 002 section 3.7: this
product never writes a path spec-spine's kit owns without a recorded, per-path,
operator-initiated and reversible ownership transfer. Standalone spec-spine use
stays viable. Do not edit, vendor or deprecate spec-spine's kit from this
repository.

**Reason.** I-06 wants this product to own the working environment, and two
installers silently claiming the same harness files is the failure that makes
both untrustworthy. The contract lets ownership move one path at a time, with a
digest recorded at the moment it moves, so `doctor` can tell a transferred file
from a drifted one from a foreign one.

**Open inside this row.** Whether this product should eventually install a
harness at all, or only ever adapt around one spec-spine installs. The contract
above is correct either way, which is why the question does not block it.

### D-05: The first usable workflow, and the interaction mode

**Recommendation.** Build **Mode B**, the CLI supervising the agent, first.
Expose Mode A, an agent calling this product's verbs, as the same verbs over the
same structured output once they exist. Answers I-10.

**Reason.** Spec 001 section 3.4 states it in full: constitution VI, VII, VIII,
IX and XI are only enforceable where there is a supervisor. A product built Mode
A first cannot keep its own constitution, and the predecessor's specs 119 and 129
are the record of discovering that after the fact. Mode A is not sacrificed: it
follows from the same verbs without a second surface.

### D-06: The spec-spine pin

**Recommendation.** Pin `required_version = "=0.18.0"`, the release this corpus
was authored and checked against. Adopting 0.19.0 is a separate change with its
own re-index and its own record.

**Reason.** C-01 and C-02. A pin is also not only about features: the coupling
gate's bypass floor is compiled into the binary, so two versions can judge the
same diff differently. The predecessor's CI broke on the day 0.19.0 was released
because its pin was a caret range; an exact pin cannot fail that way.

### D-07: The inherited evidence vocabulary

**Recommendation.** Carry the four evidence dimensions, the independent-trust
rule and the byte-reference rule forward into spec 005 as written, and record
here that they are **inherited, not re-decided**.

**Reason.** C-13: these were settled once, across the family, and re-deriving
them in a new repository would produce a second vocabulary for the same
judgments. The adoption on 2026-09-12 was recorded against the predecessor
repository, so carrying them here is a fresh choice about this repository even
though the semantics are not fresh.

**Consequence if rejected.** Spec 005 sections 3.5 to 3.7 need rewriting before
implementation, and any future reader of this product's evidence needs a
translation table.

### D-08: The binary name

**Recommendation.** `statecraft`, as the predecessor used. The hosted platform is
undecided (I-04), and this name commits to nothing hosted.

**Reason.** It is the name in the repository, the name the archived installer
shipped, and renaming later costs an installed-base migration for no current
benefit.

### D-09: The pointer file, and not a managed section

**Recommendation.** Adopt Frame's containment model in spec 002 section 3.8: this
product writes its own files at its own paths plus at most one pointer file, only
where no file exists at that path, and never appends to or merges into a user's
instruction file. Reject swamp's managed-section model, in which a tool owns a
delimited region inside a file the user also edits.

**Reason.** A managed section is a write into a user-owned file, so every upgrade
has to re-find its region in a file that changed underneath it, and a removal has
to prove it took its own bytes and nothing else. The pointer model makes the
ownership classes in section 3.2 decidable per path, which is what makes `doctor`
and `env remove` answerable at all.

**The prerequisite this carries, which the managed-section model does not.** The
pointer only works if the target harness loads it. Frame documents both halves:
Claude Code must be new enough to load a rules directory, and Codex CLI needs a
wrapper script because it has no equivalent. So an adapter's declared
prerequisites (`002` section 3.9) include "this harness loads a pointer at this
path", and an adapter whose harness does not **refuses to claim its paths** rather
than falling back to appending. Recorded here because it is the cost of the
choice, not an afterthought.

### D-10: The inherited cross-verifier fixture obligation

**Recommendation.** Record it as **dropped for now**, and reopen it only against a
named consumer.

**The obligation.** Revision 4's shared-contract-acceptance paragraph required the
CLI to own one fixture manifest tested by **both** a TypeScript and a Rust
verifier, and row G-04 placed reusable verifier code in "the CLI's existing
Apache-2.0 workspace".

**Why it has no home.** That workspace is the predecessor, now archived. `D-01`
makes this product Rust only. The TypeScript verifier lives in statecrafting 010,
which is a different repository with a different owner. Nothing in this corpus can
satisfy a two-language parity requirement, and pretending otherwise would be a
claim with no evidence (constitution XII).

**What dropping costs.** Cross-language parity was the mechanism that caught a
divergence in either direction. Without it, this product's Rust verifier is the
only reading of the fixtures, and a future TypeScript consumer inherits an
unverified assumption.

**If rejected**, G-04 needs re-adoption against this repository, naming which
repository hosts the TypeScript half and who maintains it. `005` section 4 points
here either way, so the obligation is visible rather than lost.

## 4. Deliberate deferrals

Deferred means: not in the first slice, named so it cannot be mistaken for an
oversight, and reopened by a concrete consumer need rather than by availability.

| ID | Deferred | Reopened by |
|---|---|---|
| F-01 | Hosted platform selection, any hosted control plane, and hqgit integration. | A hosted decision by the owner (I-04). |
| F-02 | Publication of any kind: push, pull request, merge, release, deploy. No verb in this corpus publishes. | A first slice that is accepted and reviewed, and a separate decision. |
| F-03 | Signing, key custody and trust-root enrollment. Spec 005 reports every signature `unsigned` and every issuer `unknown` until then. | An external consumer that requires a signed record. |
| F-04 | Any user-interface surface beyond command output, including a read-only web view. | A reviewable-outcome need that command output genuinely cannot serve. |
| F-05 | Adaptive autonomy and outcome scoring (`trust-window`). | A measured need, never the library's availability. A score must never override authorization. |
| F-06 | Cost ceilings, quota parking and model selection policy. | A run that spends against a metered resource without a floor. |
| F-07 | Breadth across providers. One adapter boundary and one adapter first. | A second provider, admitted by passing spec 004's suite. |
| F-08 | aicortex integration. Spec 005 describes a narrow optional one-way interface and implements none of it. | An aicortex producer contract that exists. |
| F-09 | Operating-system enforcement of spec 004's named residuals. | Its own spec, naming the mechanism and its own residuals. |
| F-10 | Scheduling across repositories, parallelism, and any work queue. | A single-repository loop that works, **and** a spec-spine release carrying spec 091's collision report (`C-16`). Parallelism is reopened by consuming that report, never by inventing a local footprint format. |
| F-11 | Adoption of `tenant-emit` and `tenant-tail`, and of the predecessor's export bundle and attestation formats. | A mapping from a real outcome of this product to their inputs (C-14). |

## 5. Adoption record

### 2026-09-16: D-03, adopted in part

The owner ratified `001-boundaries-and-authority` and `002-environment-lifecycle`.
Both are now `approved`; `002` keeps `implementation: pending`, which is what
makes it a work order rather than a settled claim (`C-03`). `003` to `005`
remain `draft` and are **not** dispatchable: `registry plan` will offer `003`
once `002` reports complete, and that offer is still not permission.

Three consequences landed in the same change, because each is part of this row
rather than a separate act:

- spec 000's `unamendable` list gained `independent-acceptance`,
  `no-self-granted-authority` and `evidence-outside-the-child`, and its section
  5 now records when they were frozen instead of why they were withheld;
- the constitution's ratification banner and the three per-principle notes on
  VI, VII and IX say frozen rather than proposed;
- spec 000's and spec 001's `verify:cli` blocks, which asserted the unratified
  state, now assert the ratified one. A ratification that left them alone would
  have had spec 000 accepting a state the same change had just ended.

### 2026-09-16: D-01 and D-02, adopted for language and layout only

Ratifying `002` is not layout-neutral: its territory is the forward claim
`crates/statecraft-environment/`, which presumes a Cargo workspace and therefore
Rust. Leaving that implicit would have let a layout be adopted by implication
rather than by decision, which is the confusion this whole record exists to
prevent. The owner decided it explicitly on the same day, so it is recorded
here.

**Adopted.** Rust, one Cargo workspace, crates matching the spec boundaries
(`D-02` as written), Apache-2.0.

**Not adopted, and still open.** Everything in `D-01` about distribution:
prebuilt per-target archives, checksums, the shell installer, publishing to
crates.io, and the binary's name. `F-02` defers publication and release, so
nothing here authorizes a tag or a registry push. The read-only observation
surface stays deferred as `F-04`.

No other `D-` row is adopted.

To adopt a further row, the owner can state which `D-` rows are accepted and
with what amendments. Adoption is then recorded here as plain text with its
date, and each ratified spec's frontmatter is flipped in the change that
dispatches it.

Publication, merges, releases and deployments are not covered by adopting any row
here. F-02 holds until it is separately lifted.
