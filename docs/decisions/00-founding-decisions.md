<!-- Spec: specs/001-boundaries-and-authority/spec.md -->

# Founding decisions: what is intent, what is inherited, what is proposed

Prepared 2026-09-16, at the founding of this repository. Owned by spec
`001-boundaries-and-authority`.

**Adoption status as of 2026-09-24: `D-03` is adopted in full, `D-01` and
`D-02` are adopted for language and layout only, and `D-06` carries an amendment
adopted on 2026-09-24. Every other row of section 3 is
still a recommendation.** Section 5 carries each adoption with its date and is
the only place a row becomes binding; the line you are reading is a summary of
it and never a substitute. This record exists so the three kinds of statement
below are never confused with each other. Section 1 is the repository owner's
stated intent, recorded as given. Section 2 is inherited technical constraint:
facts about tools, licenses and a predecessor, each verified in the session that
wrote this file. Section 3 is a set of engineering recommendations that need the
owner's decision before anything is built on them. Section 5 is what has been
decided since, and is the only place a row becomes binding.

As of 2026-09-16, every spec in the corpus is `approved` except
`006-command-surface`, which is a proposal written the same day. The
constitution's principles VI to XIII are ratified, and VI, VII and IX are frozen
as spec 000 anchors. `002` is implemented; `003` to `005` are specified and not.

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

Facts, each verified on 2026-09-16 in the local checkouts named, except where a
row names a later date. A fact here constrains design; it does not authorize
anything.

**Re-established on 2026-09-17, under the 0.20.0 pin.** Moving the pin (`D-06`)
made four rows false as written, and a false constraint is worse than no
constraint because the specs cite it: `C-01` and `C-02` named the installed
version, `C-16` counted seven spec-spine specs as unreleased, and `C-18` said the
coupling gate cannot see a change being staged. Each is corrected below with the
date and the command that established it. Nothing else in this section moved.

| ID | Constraint | How it was established |
|---|---|---|
| C-01 | The binary this corpus is compiled, linted and pinned against is the release `required_version` in `spec-spine.toml` names, installed at the repository-local `.tooling/bin/spec-spine`. The shared `~/.cargo/bin/spec-spine` is not this repository's binary and is not consulted by `make`. Which release that is, and since when, is recorded in `docs/adoption/spec-spine.md`, which also keeps this row's dated history (restated 2026-09-24 under `D-06` as amended). | `.tooling/bin/spec-spine --version`; `make tools` installs it from `required_version`. |
| C-02 | Unreleased producer work is never described here as available. The development checkout at `~/DevWork/spec-spine` may be ahead of every release; **no feature of an unreleased tree may be described here as available**. Which releases exist and which one is adopted is recorded in `docs/adoption/spec-spine.md`, which also keeps this row's dated history (restated 2026-09-24 under `D-06` as amended). | `git tag --sort=-creatordate` in that checkout; `cargo search spec-spine-cli`. |
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
| C-16 | **2026-09-17: all seven are released, and the pin now admits them.** 087 authority snapshot and 088 change classified under the base's rules landed in `v0.19.0`; 090 commit-boundary hook, 091 two ready specs can collide, 092 a mode-only or binary change is a change, 097 governed scope is declared and 098 a blocking claim is not a stale shard landed in `v0.20.0`, where 098 is also `approved` rather than `draft`. **101** (an unresolved claim exits as a validation failure) arrived with them and was not in the original count: it moves an unresolved claim from exit 2 to exit 1, which is the change this repository's own instructions had to follow. Availability is not consumption: the table below says what this product does with each, and adopting one is its own change. | `git tag --contains <first commit>` on each spec's directory in `~/DevWork/spec-spine`, run 2026-09-17; the earlier reading returned no tag because it predated both releases. |
| C-17 | spec-spine's design note 06 (`docs/design/06-harness-and-distribution-2026-09.md`, 2026-09-15, **proposed, nothing filed**) assigns: repository lifecycle policy and gate commands to the adopting repository; harness package identity to spec-spine; and **recording which harness revision a worker actually resolved to this product**. It proposes a versioned namespaced package pinned by a repository declaration, and records the name-precedence trap (a personal skill resolves before a project skill of the same name). It explicitly does **not** choose how a lifecycle policy is spelled. | That file, sections 2, 3.2, 3.3, 3.6, 3.7. |
| C-18 | **2026-09-17:** `spec-spine couple` still compares two commits by default, and 0.20.0 adds `--include-uncommitted` (spec 081), which unions `git diff HEAD` into the range so a pre-commit run judges the change being committed. The capability now exists; **this repository has not adopted it**, in CI or in a hook, and `make couple` does not pass it. So the operative fact is unchanged for anyone reading the gate: what CI judges is a pushed range. | `spec-spine couple --help` under 0.20.0; spec 081 §3.1 and §3.4 in `~/DevWork/spec-spine`. |

### The spec-spine facts this product consumes, and their release state

**Read the ordinals below through the corpus map (appended 2026-09-23).** Every
spec-spine ordinal in `C-16` and in this subsection was written before
spec-spine renumbered its corpus on 2026-09-20, so each now resolves to a
different document. `~/DevWork/spec-spine/docs/corpus-map.md` gives: 087 is now
070, 088 is 071, 091 is 072, 092 is 073, 097 is 078, 098 is 079 and 101 is 080;
090 was removed, its requirements now in 094. The dated statements are kept as
written, because they record what was read then.

Kept as a table so specs `003` and `005` cannot quietly fill a gap with local
code. "Available" means: carried by a release this repository's pin admits.

**Available is not consumed.** Under the 0.20.0 pin every row below is available,
which is a different statement from every row being used. Two rows changed
behaviour by being available at all (092 at the coupling gate, 098 with 101 at
the exit code); 088 has since had its amendment and is read (#20); the rest
still need one before anything reads them.

**Two approved specs held a release claim this pin made false. The owner decided
both on 2026-09-17 and both are corrected:**

- `005` section 3.3 said 088 is carried by no release and conditioned its
  `not-recorded` behaviour on that with an explicit "until then". The wait was
  over, so the **reason** was re-attributed to this product rather than to
  spec-spine. The falsehood had reached the record: the note this product wrote
  said the installed spec-spine carried no such report, and no test held it. One
  now does. **Superseded on 2026-09-17 by #20**, which discharged the obligation
  itself: `not-recorded` is now what a missing or unusable report produces, not
  the standing verdict.
- `003` section 3.7 said the same of 091, while resting on it only as the future
  answer to `F-10`. Only the sentence was stale; the concurrency bound never
  rested on tool support.

**2026-09-17: reading the released report is done, and using it end to end is
not.** It was scheduled as its own authority change and merged as #20
(`571354d`): `005` section 3.3 now fixes how the report is obtained and which
structural class witnesses which member, and the acceptance crate reads the
envelope's bytes. Implemented and tested is the grade claimed, and the evidence
is `make verify SPEC=005` on the merged sha, whose plan runs
`cargo test -p statecraft-acceptance --test negative_cases` and asserts
`crates/statecraft-acceptance/src/delta.rs` exists.

What is not done is the caller side. Section 3.3.1 rule 3 keeps the invocation
out of the acceptance library, and no verb runs `spec-spine delta` yet: that is
`009`'s integration slice, which is `approved` with `implementation: pending`.
So nothing here claims the integration works end to end, and no run has produced
an authority-set verdict from a live report.

*Superseded, recorded 2026-09-23:* `009` was folded into `006`, and `accept`
now obtains the report itself, in the target and not the workspace
(`crates/statecraft-cli/src/accept.rs`, `006` section 5, 2026-09-17). The
paragraph above is kept as the state it described.

| Fact needed | spec-spine source | Available at =0.18.0 | What this product does meanwhile |
|---|---|---|---|
| Corpus compiles; registry and index freshness | `compile`, `index`, `check` | **Yes** | Consumed directly. |
| The ready set and its blockers | `registry plan` | **Yes** | Consumed directly; eligibility is filtered by `003` section 3.1.1. |
| Which authority-set members a change touched | **088** | **Yes**, from `v0.19.0` | **Consumed by the acceptance library, and not yet invoked by any verb.** Spec `005` section 3.3 was amended and implemented on 2026-09-17 (#20, `571354d`): the crate reads a `spec-spine delta --json` envelope and maps its structural classes onto the authority-set members `001` section 3.5 enumerates, so a report naming no member now lets acceptance rest on the candidate's own suite. Revision 4 row CLI-08's obligation, integrate the report once released, is **discharged**. Obtaining the report is deliberately not the library's act (section 3.3.1 rule 3), and the verb that would perform it belongs to `009`, `approved` and pending, so `not-recorded` is still what every real run produces today. |
| What the verifier read, as a snapshot | **087** | **Yes**, from `v0.19.0` | Not consumed. No local substitute, and none needed to consume it later. |
| Two ready specs collide | **091** | **Yes**, from `v0.20.0` | Still not needed: one live attempt per repository (`003` section 3.7, corrected 2026-09-17). `F-10`'s tool-support condition is met; its other condition, a single-repository loop that works, is not. |
| A mode-only or binary change is a change | **092** | **Yes**, from `v0.20.0` | Consumed by construction: the gate now completes diff membership from `git diff --name-status`, so a mode-only or binary change is judged rather than dropped. Strictly more paths checked, never fewer. |
| Declared governed scope | **097** | **Yes**, from `v0.20.0` | Not consumed, and inert: it widens the `C-002` universe only when `[coverage] governed_scope` is non-empty, and this repository leaves it empty. Verified by diffing `config show` across the two versions. |
| Exit 2 can mean a blocking claim, not a stale shard | **098**, with **101** | **Yes**, from `v0.20.0` | Consumed. An unresolved claim exits 1 and a stale shard exits 2, so `AGENTS.md` states one reading per code instead of two readings of one code. |
| Harness package identity and revision | note 06 section 3.2 | No, unfiled (C-17) | Receipt field present, reading `not-recorded` (`005` section 3.4). |
| Lifecycle policy as a queryable fact | note 06 section 3.6 | No, unfiled (C-17) | Read from the target where declared; otherwise default plus an explicit override (`003` section 3.1.1). |

## 3. Proposed decisions, none adopted

Each row is a recommendation with its reason. Each can be adopted, amended or
rejected without editing a spec, which is why they live here and not in one.

**Where each row stands, as a reading aid and never as the record.** Section 5 is
the record; this table is a summary of it and is not a substitute. A row that is
merely *unadopted* is still live: it is a recommendation nobody has decided, not
one that has lapsed.

| Row | Standing | Where it was settled, if it was |
|---|---|---|
| `D-01` | Adopted for language and layout; the distribution and naming half is **open** | Section 5, 2026-09-16 |
| `D-02` | Adopted, then **amended** 2026-09-21: a crate has exactly one owning spec, and a spec may own more than one crate | Section 5, 2026-09-16 and 2026-09-21 |
| `D-03` | **Superseded.** Every spec in the corpus is now `approved` | Section 5, and `registry list` |
| `D-04` | **Superseded.** The kit transition contract is withdrawn with the installer that motivated it | Spec `002` sections 3.7 and 3.21 |
| `D-05` | Mode B is built; exposing Mode A over the same verbs is **open** (`F-04`) | Spec `001` section 3.4 |
| `D-06` | **Open as standing policy**, amended 2026-09-24: the pin is exact and repository-local, stated once, and each adoption is recorded in the adoption ledger | `spec-spine.toml`, the root `Cargo.toml`, `docs/adoption/spec-spine.md`; section 5, 2026-09-24 |
| `D-07` | Carried into spec `005` as written, recorded as inherited and not re-decided | Spec `005` section 2 |
| `D-08` | **Open.** The executable is `statecraft-cli` until this is decided | Spec `006` section 3.5 |
| `D-09` | Adopted in practice by the pointer model | Spec `002` sections 3.8 and 3.13 |
| `D-10` | Recorded as dropped for now, reopened only against a named consumer | This row |

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

**Amended 2026-09-21**, in section 5: a crate has exactly one owning spec, and a
spec may own more than one crate. Read that entry before this row.

### D-03: The corpus itself

**Superseded 2026-09-21.** This row recommended ratifying specs `001` to `005`
as a set, or amending them first, and recorded that until then the repository was
specified only and partially. Every spec in the corpus is now `approved`, and
`registry list` is the authority on that rather than this file. The reason the
row gave still holds and is now a standing rule rather than a recommendation: a
corpus its owner has not read should not be dispatched from, which is why
AGENTS.md says an agent never ratifies. The 2026-09-16 entry in section 5 records
the adoption in part; the ratifications since are each recorded in their own
spec.

### D-04: Who installs the working environment

**Superseded 2026-09-21.** This row recommended adopting the transition contract
in spec `002` section 3.7, under which this product never wrote a path
spec-spine's kit owned without a recorded, per-path, operator-initiated and
reversible ownership transfer. spec-spine then withdrew the kit and its public
initializer, so there is no second installer to coexist with and the contract has
nothing to hold. Spec `002` section 3.7 is the withdrawal and section 3.21 states
what replaces it, including the three parts retained because each is general
rather than kit-specific: a `foreign` finding names an owner, `shadowed` stays,
and ownership transfer stays per path, explicit, operator-initiated, reversible
and recorded.

The question the row answered, I-06's "this product owns the working
environment", is now answered by the whole of spec `002`: it owns it, and it is
the only installer.

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

**Recommendation.** Pin `required_version` exactly, and install the pinned binary
**into the repository**. Adopting a newer spine is a separate change with its own
re-index, its own bypass-floor review and its own record.

**Reason.** C-01 and C-02. A pin is also not only about features: the coupling
gate's bypass floor is compiled into the binary, so two versions can judge the
same diff differently. The predecessor's CI broke on the day 0.19.0 was released
because its pin was a caret range; an exact pin cannot fail that way.

An exact pin is only half of it, and 2026-09-17 measured the other half. The pin
was satisfied by whatever `~/.cargo/bin/spec-spine` held, which is one binary
shared by every project on the machine: work in the spec-spine checkout replaced
it with 0.20.0, and every governed read in this repository then refused on the
version check. The refusal is the good case. The bad one is a project whose pin
happens to admit the replacement, which is then governed by a version it never
adopted and cannot tell. So the binary is installed at `.tooling/bin`,
gitignored, by `make tools`, which reads the version from `required_version` so
the number is authored once. `make` prefers the local copy; CI uses only it.

**Amended 2026-09-24 (owner decision A-1, adopted in section 5).** Three
rules, so that adopting a release is its own change and needs no waiver:

1. **One stated source for each pin.** `required_version` in
   `spec-spine.toml` is the only place the CLI pin is stated; the linked
   library's exact version is stated once, in `[workspace.dependencies]` of
   the root `Cargo.toml`. Root documents, specs and this record refer to those
   files and do not restate the number. A report that must name the version
   derives it from the build.
2. **Each application of this decision is recorded in the adoption ledger,**
   `docs/adoption/spec-spine.md`, which no spec claims. This row keeps only the
   decision. The dated entries that stood here (2026-09-17, 2026-09-23 and
   2026-09-24) were relocated there verbatim.
3. **The CLI and the linked library are one producer identity** (owner
   Addendum 2 of 2026-09-24, bundle decision H-3 (a): equality, replacing the
   earlier two-field choice). Both are the same spec-spine release, they move
   in the same change, and the ledger entry records one identity: the release
   (tag and target revision) with each artifact's checksum and install digest
   under it. Two pins naming different releases are a defect, refused by a
   test, not a combination to qualify.

**Consequence if rejected.** The pin stays exact and repository-local, but each
adoption edits this record, which spec `001` claims, and so needs either an
authoring edit to spec `001` or an owner waiver, as 0.25.0 did.

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
| F-10 | Scheduling across repositories, parallelism, and any work queue. | Two conditions. The second, **a spec-spine release carrying spec 072's collision report**, was met by `v0.20.0` on 2026-09-17 (`C-16`). The first, **a single-repository loop that works**, is **not recorded as met**. Its original supporting sentence, "specs `008` and `009` are drafts", was true when written and is stale: both are `approved` and implemented (see the 2026-09-19 entry in section 5). Whether the loop *works* is the owner's judgment and not a status field, so the deferral stands until section 5 records it met. Parallelism is reopened by consuming that report, never by inventing a local footprint format. |
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

### 2026-09-16: D-03, adopted in full

The owner ratified `003-work-and-run-semantics`, `004-execution-adapter` and
`005-acceptance-and-evidence`, completing the row. All three keep
`implementation: pending`, so each is a work order and none is a claim about
code. `registry plan` now offers `003`, and offering it is finally the same
thing as permission, which it was not for the whole of this repository's first
day.

`002` moved to `implementation: complete` in the change that implemented it. The
grade that claims is *implemented for its own territory*: the crate. The
operator commands `002` names were deliberately not bound to a process there,
which is what `006` below exists to fix.

### 2026-09-16: spec 006 proposed, not ratified

Specs `002` to `005` each describe operator verbs and each own a library crate,
so the product is specified, partly implemented, and not runnable. `006` claims
`crates/statecraft-cli/` and binds the verbs to a process, with a closed
exit-code vocabulary that separates a refusal from a failure from a finding.

It is `draft`. It was written as a proposal rather than as part of any
implementing change precisely because a binary that appears as a side effect of
a feature is a binary nobody designed.

### 2026-09-16: spec 006 ratified

The owner ratified `006-command-surface` the same day it was proposed. It keeps
`implementation: pending`, so it is a work order.

Ratifying it settles one thing `D-01` deliberately left open only in part: the
**executable's name** is `statecraft-cli`, stated in `006` section 3.5 rather
than inherited from Cargo's package-name default. `D-01`'s recommendation of
`statecraft` is still not adopted, and adopting it later is a change to that
section and to one `[[bin]]` stanza. What is refused is the name being settled
by a build-tool default nobody recorded agreeing to.

Everything else in `D-01`'s packaging half remains open, and `F-02` still defers
publication and release.

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

### 2026-09-17: spec 007 ratified, and its reading of ambiguous bytes accepted

The owner ratified `007-shared-evidence-envelope`. Its frontmatter flips to
`approved` in this change. `implementation: complete` was already true, the code
having landed with the spec rather than after it.

**What ratification accepts beyond the spec itself.** Section 3.2 reserves
`"none"`, `"not-recorded"` and `"stale"` for every `Recorded<T>` and refuses a
present value that would serialize to one of them. The consequence that needs
saying in the owner's voice is the backward one: a record this product wrote at
or before `8f6591f` carrying a present string equal to one of those words now
decodes as the absence, in both products. That is accepted here as a **decision
taken under ambiguity, not a recovery of what the writer meant**. The bytes
cannot say which reading was intended and nothing later can make them say it.
For `harness_revision` the writing convention does settle it (005 section 3.4
writes `"not-recorded"` precisely to say no revision was observed); for an
arbitrary `Recorded<String>` field it does not. So the reading is chosen rather
than discovered, and is recorded as chosen.
`testdata/fixtures/cli/recorded-collision-legacy.json` holds the bytes a pre-007
build would have written, and a test on each side asserts that the old reader
said present and every reader now says absent, so the cost is pinned by evidence
rather than by this paragraph.

The alternative of a tagged or versioned representation is rejected for the
reason section 3.2 gives: it rewrites every `harness_revision` in every receipt
already written, and 005 section 3.7 forbids rewriting historical records to
satisfy a newer rule. If a future contract needs a present value equal to one of
the three words, that is a new schema version, not a second reinterpretation of
these bytes.

**What ratification does not settle.** `D-10` stays dropped, and the reason is
narrower than the new fixture suite might suggest. Section 3.4's suite runs from
two crates of **this** repository, `statecraft-envelope` reading the fixtures
through the shared types and `statecraft-acceptance` checking that the functions
this product calls still produce those bytes. Both are Rust, both are here, and
the platform consumes the same crate rather than reading the fixtures with an
independent implementation. That is a useful check and it is not the
two-language parity `G-04` asked for, so nothing here is to be read as
satisfying that obligation.

The three questions section 5 leaves open (whether the two `RootSet` models
converge, whether the shared structs carry an extras map, whether
`VerifierRecord` and the envelope's verifier identity are one type) stay open
and are the owner's. The native golden vectors stay provisional: freezing them
needs a first signed entry under the platform's constitution VIII, and none
exists. No `D-` row is adopted by this ratification.

**Publication, scoped to this change.** The owner authorized publishing this
change on 2026-09-17: the branch, its pull request and its merge, so the
platform can pin its dependency to the commit this lands as. That authorization
covers this change and no other. `F-02` is not lifted: nothing here tags a
release or pushes to a registry, and `crates/statecraft-envelope/` stays
`publish = false` at version `0.0.0`.

### 2026-09-19: corpus status reconciled; no row adopted

A documentation change measured this repository against what its own documents
claimed and corrected the claims. Nothing here adopts a `D-` row, lifts a
deferral, or changes an acceptance requirement.

**What was measured, and with what.** `make status` at spec-spine `0.20.0`
reports ten specs, `000` to `009`, all `approved`, with `registry plan` offering
nothing schedulable. `registry list` reports `000` and `001` as
`implementation: n-a`, because they own prose and no code, and `002` to `009`
as `implementation: complete`. `ls crates` and `cargo test --workspace` report
seven member crates and 515 passing tests, none ignored.
`cargo run -p statecraft-cli -- --help` prints fifteen bound verbs.
`crates/statecraft-run/Cargo.toml` carries `attest-ledger-core` pinned to
`a9c3595`. Against a disposable target with a scratch `STATECRAFT_HOME`, and
with no provider session launched, `env apply` under a missing qualification
record reports `applied: 0 path(s) written`, exits 0, writes no managed byte,
and still creates `.statecraft/environment.json` carrying the pins with an
empty `entries` list.

**What this corrects.** The preamble above described 2026-09-16, when `006` was
a proposal and `003` to `005` were unimplemented; it is left as the dated
statement it is. The adoption-status line is restated to match section 5 rather
than to add to it. `F-10`'s supporting clause named `008` and `009` as drafts,
which they no longer are, and the correction is factual only: `F-10` stays
deferred, and so do `F-01` through `F-09` and `F-11`.

**Publication, scoped to this change.** The owner authorized publishing this
change on 2026-09-19: the branch, its pull request and its merge through the
normal protected process. That authorization covers this change and no other,
and it carries no waiver, no policy change and no branch cleanup.

**What is still not claimed.** Nothing is released. The workspace stays at
version `0.0.0` with `publish = false`, and `F-02` is not lifted: nothing here
tags a release or pushes to a registry.
The distribution half of `D-01`, including the binary's name (`D-08`), stays
open. A spec being `approved` still grants no grade above *specified*, and every
implemented-or-tested claim corrected in this change names the command that
produced its evidence.

To adopt a further row, the owner can state which `D-` rows are accepted and
with what amendments. Adoption is then recorded here as plain text with its
date, and each ratified spec's frontmatter is flipped in the change that
dispatches it.

### 2026-09-21: D-02 amended; the corpus consolidated to seven specs

The owner directed a consolidation of the spec corpus and decided the layout
question it raises. Recorded here because it changes an adopted row.

**What changed.** Eleven specs become seven. Four pairs, each of which described
one subject across two documents, are merged into the spec that held the subject
first: `009`'s work, run and accept bindings into `006`; `007`'s shared envelope
into `005`; `008`'s first provider into `004`; and `010`'s managed environment
into `002`. No crate is merged, renamed or deleted, and no requirement is
weakened: each absorbed spec's section 3 moves whole, keeping its wording, and
every citation in the tree moves with it from an explicit map.

**What this amends in `D-02`.** The row as adopted on 2026-09-16 reads "crates
matching the spec boundaries", and its stated reason is that the spec corpus is
the module boundary, so the coupling gate has something real to hold. Three of
the seven specs now own two crates each: `002` owns `statecraft-environment` and
`statecraft-home`, `004` owns `statecraft-adapter` and
`statecraft-adapter-claude-code`, and `005` owns `statecraft-acceptance` and
`statecraft-envelope`. **The amended row is: a crate has exactly one owning
spec, and a spec may own more than one crate.** One direction of the
correspondence is kept, and it is the direction the coupling gate uses: every
compile unit still resolves to exactly one spec, so the gate still holds a real
unit per claim, and `index coverage` still reports every source file
specifically claimed.

**What the amendment costs, stated rather than smoothed.** `D-02`'s reason named
one case specifically: spec `004`'s rule that no provider name appears in the
adapter seam becomes a compile-unit fact rather than a review convention. That
rule survives the merge unchanged, and it survives as a compile-unit fact,
because the two crates are untouched and `cargo test -p statecraft-adapter
--test no_provider_names` is the mechanism it was always enforced by. What is
lost is the second, weaker guard: two specs can no longer disagree about it,
because there is one spec. Each merged spec's section 2 now states the
separation its two crates hold and why, which is where a reader will look.

A second cost was found by the compiler rather than by reading. Merging a spec
that depended on `005` and `006` into one they depend on makes a cycle
`compile` refuses (`V-014`), in `004` and again in `002`. It is resolved in both
by carrying the relationship on the `extends` and `amends` edges alone, which
name the exact unit reached and its nature, and never by dropping a claim. Each
is recorded in its own spec's section 5.

**What is not adopted here.** No other `D-` row moves, no deferral is lifted,
and nothing is released. `D-01`'s distribution half and `D-08` stay open, and
`F-02` holds.


### 2026-09-24: D-06 amended so that an adoption needs no waiver

The repository owner decided this in the session request of 2026-09-24 (item
A-1: "I adopt this direction"), and ratifies it by merging the change that
carries this entry. `D-06`'s recommendation and reason are unchanged. Its three
added rules are in the row itself: one stated source for each pin
(`spec-spine.toml` for the CLI, the root `Cargo.toml` for the linked library),
per-release entries in `docs/adoption/spec-spine.md`, and one producer
identity: the CLI and the library are the same release and move together
(H-3 (a), owner Addendum 2 of 2026-09-24).

**Why.** The 0.25.0 adoption (#102) edited this record, which spec `001`
claims, so the coupling gate raised `C-001` and the change merged only under a
waiver the owner granted for that pull request alone. Recording each adoption
in a file no spec claims, and stating the pin once in files no spec claims,
removes that cause without weakening the gate: nothing claimed moves, and the
record's decisions still change only with an authoring edit to spec `001`.

**What moved, and how.** The three dated entries under `D-06` (2026-09-17,
2026-09-23, 2026-09-24) and the dated history of rows `C-01` and `C-02` were
relocated verbatim to the ledger. `C-01` and `C-02` are restated as rules that
name no release. No requirement text changed in the relocation.

Publication, merges, releases and deployments are not covered by adopting any row
here. F-02 holds until it is separately lifted.
