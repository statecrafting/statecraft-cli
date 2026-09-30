---
id: "029-no-provider-directories"
title: "A governed project needs no provider-specific directory, and this product writes none"
status: draft
implementation: in-progress
created: "2026-09-30"
summary: >
  Makes the absence of provider-specific project directories (`.claude/`,
  `.codex/`, `.agents/`, `.agent/` and their equivalents) a supported and
  tested operating condition. No adapter declares a managed or adopted path
  inside one. The Claude Code adapter's owned instructions file moves out of
  `.claude/statecraft/` into content this product already manages, and its
  pointer, where one is still needed, imports `.statecraft/AGENTS.md`
  directly. A project that already holds the old file converges through the
  ordinary upgrade and removal rules. A qualification case starts with every
  provider directory absent and proves initialization, the governed verbs and
  context delivery work and recreate none of them. Brings the adapter into
  line with spec 002 section 3.12 and spec 008 section 3.14, and amends the
  adapter declaration of spec 004.
amends:
  # The Claude Code adapter's declared file set (004's adapter declaration)
  # and section 3.8's pointer target. Approved specs stay unedited; registry
  # relationships reports the edges.
  - "004-execution-adapter"
  - "002-environment-lifecycle"
extends:
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter-claude-code/" }, nature: corrective }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: corrective }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "004-execution-adapter"
  - "008-harness-delivery"
obligations:
  - id: "R-1"
    kind: requirement
    text: "No adapter declares a managed or adopted path inside a provider-specific project directory, and no verb of this product creates one in a target."
    anchor: "3-1-no-provider-directory-in-a-target"
  - id: "R-2"
    kind: requirement
    text: "The Claude Code adapter delivers its instructions through `.statecraft/AGENTS.md` and the global harness; its pointer, when the harness's load rule does not already reach the managed instructions, is one root file that imports `.statecraft/AGENTS.md`."
    anchor: "3-2-where-the-adapter-s-content-goes"
  - id: "R-3"
    kind: requirement
    text: "A project holding the previous `.claude/statecraft/instructions.md` converges on upgrade: removed when its digest matches the recorded one, reported and left when drifted, and an emptied `.claude/` directory the product created is removed."
    anchor: "3-3-convergence-of-existing-projects"
  - id: "V-1"
    kind: verification
    text: "A fixture with every provider directory absent is initialized, applied, checked, run and diagnosed; context delivery is evaluated as reached; and the tree afterwards holds no provider directory."
    anchor: "verification"
    inputs:
      - "crates/statecraft-cli/tests/no_provider_directories.rs"
      - "crates/statecraft-environment/tests/retirement.rs"
---

# 029: No provider directories

## 1. Purpose

The owner's requirement, 2026-09-30:

> Neither spec-spine nor Statecraft requires provider-specific project
> directories for configuration, context discovery, execution, or governance.
> Their absence is a supported and tested operating condition.

`.statecraft/` in the project and the product home are this product's own and
are unaffected. Root `AGENTS.md` stays the portable entry point.

Most of this is already required. Spec 002 section 3.12 allows exactly four
paths in the per-project area. Spec 008 section 3.14 says a project receives no
copy of the harness and that every delivered behavior is gated to Statecraft
projects from the global home. spec-spine's producer emits no path under
`.claude/`, and its own tests assert that.

The one place this product still writes a provider directory is its own Claude
Code adapter. Measured on `main` at `c0469a3`
(`crates/statecraft-adapter-claude-code/src/environment.rs`), its declaration
names two files in the target:

| Path | Class | Contents |
|---|---|---|
| `.claude/statecraft/instructions.md` | managed | the facts the harness cannot express, and a note that the file is managed |
| `CLAUDE.md` | pointer | `@.claude/statecraft/instructions.md` |

So `env apply` on a fresh repository created `.claude/`, and a test that
starts with provider directories absent and asserts none is recreated failed on
this product first. This spec removes that write and makes the absence tested.

## 2. Territory

This spec owns no product code. The implementation changes only the units its
`extends` edges name: the adapter's declaration, the global harness's adapter
template in `statecraft-home`, the plan and apply rules in
`statecraft-environment`, and the plan's JSON view and the tests that drive the
binary in `statecraft-cli`.

## 3. Behavior

### 3.1 No provider directory in a target

A provider-specific project directory is a directory whose name a provider's
client reads as its own project configuration: `.claude/`, `.codex/`,
`.agents/`, `.agent/`, `.cursor/`, `.gemini/`, and any other an adapter's
harness documents. No adapter declares a managed or adopted path inside one.
No verb (`init`, `env apply`, `env upgrade`, `home apply`, `run`, `accept`,
`transfer`) creates one in a target. An existing one is user class (002
section 3.8): it is never read for control, rewritten or removed, except the
convergence of section 3.3.

This is a rule about targets. Provider configuration in the user's home, which
spec 008 section 3.14 governs, is unaffected.

### 3.2 Where the adapter's content goes

The adapter's unexpressible facts are harness facts, identical in every
project, so they belong to the global harness under the home (008 section
3.14), delivered once and gated to Statecraft projects. The adapter adds
nothing project-specific.

Context reaches a session through the chain spec 008 section 3.14 already
evaluates:

1. root `AGENTS.md`, whose first line is the bridge to `.statecraft/AGENTS.md`
   (002 section 3.13), for a harness whose documented load rule reads it;
2. otherwise, the globally delivered, project-gated session-start behavior
   that loads `.statecraft/AGENTS.md`, where the harness documents one and
   delivery evaluates `reached`;
3. otherwise, one root pointer file under 002 section 3.8's rule (written only
   where no file exists at its path), whose content imports
   `.statecraft/AGENTS.md` directly. For the Claude Code adapter that file is
   `CLAUDE.md` containing `@.statecraft/AGENTS.md`.

A root pointer file is a file, not a directory, and is permitted by this
section (section 5).

### 3.3 Convergence of existing projects

A project initialized before this spec holds `.claude/statecraft/instructions.md`
as a managed path and possibly `CLAUDE.md` as a pointer to it.

- `env plan` names both as leaving the adapter's declaration.
- `env upgrade` removes the instructions file when its digest matches the
  manifest's, under 002 section 3.6's removal rule, and removes the
  `.claude/statecraft/` and `.claude/` directories only when this removal left
  them empty. A drifted file is reported and left.
- A managed pointer whose digest matches is rewritten to the new import. A
  drifted or foreign `CLAUDE.md` is left and reported, and delivery is
  evaluated from what is there.
- Nothing is removed by `doctor`, `run` or any read.

### 3.4 Observable negative cases

| Case | Required behavior |
|---|---|
| `init apply` on a repository with no provider directory | No provider directory exists afterwards; delivery is evaluated and reported |
| `env apply` with a user's own `.claude/` present | Nothing inside it is read for control, written or removed |
| `env upgrade` where `.claude/statecraft/instructions.md` matches its recorded digest | Removed; the emptied directories the product created are removed |
| The same file drifted | Reported with both digests and left |
| A user file at `CLAUDE.md` | Classed `foreign`, left; the adapter reports degraded unless another link of section 3.2's chain reaches the instructions |
| An adapter declaration naming a path under `.claude/` | Refused at plan time, naming the adapter and the path |

## 4. Out of scope

- This repository's own `.claude/`, which spec 008 section 3.22 removes in a
  fixed order, and the `.claude/` authority path `accept` declares for it.
- spec-spine's own repository layout; its producer already emits no provider
  path.
- Provider configuration in the user's home.

## 5. Resolved decisions

**2026-09-30: provider-specific content moves out of the target, not into a
new project path.** Spec 002 section 3.12 fixes the per-project area at four
paths, so relocating the adapter's file to, say, `.statecraft/adapters/` would
trade one violation for another. The content is the same in every project and
belongs to the global harness.

**2026-09-30: one root pointer file is permitted.** The requirement names
directories. This section permits one provider-named root
pointer file as the last link of the delivery chain, because without it an
unmanaged session in a client that reads neither root `AGENTS.md` nor a
delivered session-start behavior finds no project instructions. Which
clients read root `AGENTS.md` natively is measured per client by the
delivery evaluation, which writes no pointer where the chain already reaches.

**2026-09-30: the pointer is declared unconditionally, and written only where
no file exists.** The adapter's declaration is one file, `CLAUDE.md` holding
`@.statecraft/AGENTS.md`. Declaring it only where section 3.2's chain does not
already reach would make the declaration a function of the tree, which 004's
static declaration is not today. This build keeps the declaration static; the
conditional pointer is the first item left open below.

**2026-09-30: the facts live in the global harness template.**
`statecraft_home::harness::shipped()` appends the adapter's unexpressible facts
to the Claude Code adapter template it already ships, so the harness revision
changes with them. `env plan` and `doctor` still report them from the
declaration.

**2026-09-30: retirement is one rule for `env apply` and `env upgrade`.** The
two verbs share one library operation (002 section 3.6), so section 3.3's
removal happens on either. A plan names each file it will remove as `retire`,
and the JSON view carries them as `retired`. Only an entry that is `managed`,
from an adapter source, in the `reference` role and without a transfer is
retired, and only when the adapter that wrote it is configured, claims its
paths and no longer declares the path. Adopted, transferred and authored
entries, and entries of an adapter no longer configured, are never retired.

**2026-09-30: nothing is retired where the adapter does not claim.** An adapter
that does not claim its paths writes nothing, so the same apply could not
rewrite a managed pointer importing the retired file. Removing the file there
would break the import it left. Convergence therefore happens where the adapter
claims, in one apply that rewrites the pointer and removes the file.

**2026-09-30: removal is guarded like a write.** A retired path reached through
a symbolic link is withheld and left. The digest is read again at removal, so a
file edited after the plan is withheld as drifted with both digests, and its
record stays. A file already gone loses only its record. Directories are
removed bottom-up only while empty, never past the target root, so a user's
file beside the retired one keeps its directory.

**2026-09-30: section 3.3's first bullet reads as the plan's rendering.** The
pointer does not leave the declaration; its bytes change. `env plan` names the
instructions file as `retire` and the pointer as a write replacing existing
bytes, which is what the operator acts on.

**2026-09-30: what is left `in-progress`.** Two behaviors are not built.
(1) The pointer is written wherever `CLAUDE.md` is absent, even where root
`AGENTS.md` already reaches `.statecraft/AGENTS.md` for this harness; section
3.2 and the second resolved decision above say no pointer is written there.
(2) A foreign `CLAUDE.md` still reports the adapter `degraded` without asking
whether another link of the chain reaches (section 3.4, fifth row). Both need
the adapter's declaration or readiness to consult the delivery evaluation in
`statecraft-home`, which the environment crate cannot see today.

**2026-09-30: the binary's transfer tests follow the declaration.** With
`CLAUDE.md` the adapter's only path, a move of an adapter path to `managed`
through the binary meets 002 section 3.35's rule 2 (an instruction file stays
`user`) before rule 1. The binary-level tests assert that refusal; rule 1 and
the `managed` halves stay asserted by `statecraft-environment`'s
`tests/transfer.rs` with a test adapter.

## Verification

Each line is one command. The binary fixture is an isolated home, a stub
`spec-spine` and a fresh repository with no provider directory; the library
fixture is a test adapter whose declaration shrinks between two applies.

```verify:cli
cargo test -p statecraft-cli --test no_provider_directories
cargo test -p statecraft-environment --test retirement
cargo test -p statecraft-adapter-claude-code --lib environment
```
