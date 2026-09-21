<!-- Spec: specs/002-environment-lifecycle/spec.md -->

# What spec-spine has stopped shipping, and what this product has to deliver

Prepared 2026-09-20 by a spec-spine session. A handoff, not a decision: every
disposition below belongs to this product's owner.

Spec 002 §3.7 is a transition contract with `spec-spine init --with-kit`, so
that the two products do not both claim the same harness files. **That
counterparty no longer exists.** spec-spine removed `init`, `--with-kit`, the
`kit/` tree, the embedded kit, and the `.agents/` and `.codex/` projections in
its spec 092, and this note records what that leaves on each side of the line.

The short version: the contention spec 002 §3.7 was written to resolve is gone,
and what replaces it is an absence. Nothing writes an agent harness today. The
one repository still carrying one carries it for itself, is dismantling it in
the order below, and is now waiting on this product for the last three quarters
of it.

## 1. What spec-spine no longer does

| Gone | Was |
|---|---|
| `spec-spine init` | the project initializer |
| `--with-kit` | the harness installer |
| `kit/` and `kit_embedded.rs` | the harness, vendored and embedded in the binary |
| `.agents/`, `.codex/` | generated projections of that harness |
| `website/` | the documentation site |
| `scaffold_init_with(...)` | the kit-selecting API |

There is no verb that writes an `AGENTS.md`, a `CLAUDE.md`, a `.claude/`
directory, a skill, an agent brief, a hook, an MCP configuration, a CI workflow
or a `Makefile`. Adding one back is explicitly out of scope there.

**The retained producer** is `spec_spine_core::scaffold_init_json`, reachable
through the JSON facade. It is a pure function of its argument: writes nothing,
reads no environment, launches nothing. It emits governance starter content only,
as data, for the caller to write: the config, the constitution, the contract, the
spec templates, a bootstrap spec, and a `.gitignore` fragment. Its tests assert
that no emitted path begins with `.claude/`.

So the ownership classes in spec 002 no longer have a second claimant to
negotiate with. An adapter that manages `.claude/**` contends with nothing.

## 2. Where spec-spine's own `.claude/` stands

spec-spine keeps a `.claude/` tree for itself, deliberately, because removing it
before a replacement exists would leave that repository with no development
instruction and no hook enforcement. It is being dismantled in the order that
never leaves it unprotected. As of 2026-09-20:

| Class | State |
|---|---|
| `.claude/rules/` (4 files) | **removed.** Folded into `AGENTS.md`'s `## Rules` section. Not a candidate for a global home in the first place: they are that repository's governance and would bind every project a user opens. |
| the push gate | **copied to the user's global harness** at `~/.claude/hooks/push-gate.sh`, registered as a `PreToolUse(Bash)` hook. Repository-agnostic: needs `git` and `jq` and knows nothing about spec-spine. The project copy stays only because its tests cannot read `$HOME`. |
| `.claude/skills/` (10) | **waiting on this product.** |
| `.claude/agents/` (4) | **waiting on this product.** |
| `.claude/settings.json` (PR gate, 2 session hooks, permissions) | **waiting on this product.** |

The ordering is fixed and is the point of this note: **this product delivers a
global harness with a Claude Code adapter; spec-spine confirms a session there
still has its loop and its hooks; then `.claude/` goes, with its governing spec
superseded in the same change.** Doing it in the other order is the failure the
removal of the kit was written to avoid.

## 3. What the adapter has to carry, concretely

This is an inventory, not a design. Each item is currently enforced by tests in
the spec-spine repository (`crates/spec-spine-core/tests/harness_hooks.rs`, 1124
lines, and `harness_skills.rs`, ~800), and those tests read the files on disk. A
test cannot read `$HOME` and stay hermetic, so **whatever repository owns the
files has to own the assertions**. If this product takes the files, it takes
them; if it does not, they are deleted and the requirements become unenforced.
That is the whole cost of the move, and it is not a large amount of code.

### 3.1 Ten skills, four agents

Skills: `prime`, `next`, `build`, `verify`, `ship`, `shepherd`, `spec`,
`commit`, `code-review`, `setup`. Agents: `architect`, `explorer`,
`implementer`, `reviewer`.

They are already written to be **repository-invariant**: every project-specific
fact lives in that project's `AGENTS.md`, which each skill ends by pointing at.
That property was built for a distribution that then got cancelled, and it is
what makes a global home viable now. The asserted invariants worth preserving:
no skill names a gate flag its project's `AGENTS.md` omits; a read-only skill
never invokes a writing verb; each skill wraps the tool verbs it exists for.

### 3.2 The hooks, and the contracts they hold

Four events. Every one of them was written after a measured failure, and the
contracts are worth more than the shell.

- **Read, never repair.** No hook may invoke a writing subcommand. A hook fires
  where it cannot commit what it regenerated, so a writing hook leaves the
  derived tree dirty; an orchestrator that refuses to start on a dirty tree then
  never starts. One adopter's pipeline stalled eleven hours on dirt it produced
  itself. The single sanctioned exception is a `compile` after an edit to a
  `spec.md`, where the session is live and can commit the result.
- **Binary resolution order**: `$SPEC_SPINE_BIN`, then the target repository's
  own `target/release/spec-spine`, then `PATH`. A repository that builds its own
  binary must be governed by the one it builds; the `PATH` fallback keeps an
  adopter on the published CLI working.
- **Resolve the target repository from the command, not from the session.** A
  multi-repository session pushes and edits in whichever tree the command names.
- **Read the verdict; never guess it.** `spec-spine check` has four answers and
  they are not interchangeable: 0 fresh, 1 a corpus that does not validate, 2
  stale **or** an unresolved claim, 3 a read that was not performed. Only one of
  the four is repaired by regenerating. Exit 2 is also what `clap` spends on an
  unknown subcommand, so a hook must establish the binary carries the verb
  (`check --help`) **before** reading its exit code, or a binary older than the
  verb reports a fresh tree as stale.
- **A gate whose check did not run is not green.** Every non-zero code refuses.
- **The push gate** resolves the protected branch rather than assuming `main`:
  `$SPEC_SPINE_DEFAULT_BRANCH`, then the remote's own `HEAD`, then `main` as a
  floor. It refuses only a push that would actually update that branch, so a tag
  push from the default branch is allowed. It is anchored on the command that
  invokes the verb, so a `grep` or heredoc merely containing the text still runs.
- **The PR gate** runs the coupling gate before `gh pr create` and refuses
  without a human-written waiver line in the body.

### 3.3 Permissions

`.claude/settings.json` also carries an allow list and a deny list (no
`cargo publish`, no `npm publish`, no `gh release create`, no force push, no
`rm -rf` of the corpus or the derived tree). Deny entries are a safety floor and
should not silently become an adapter's optional extra.

## 4. Facts this product's corpus may be carrying stale

Offered as evidence to check, not as a finding about this repository.

1. **`spec-spine init --with-kit` is named in spec 002** (summary and §3.7) as a
   live counterparty. It does not exist in any released or unreleased spec-spine.
2. **The managed layout is real and in use.** spec-spine governs itself with
   `[layout] derived_dir = ".statecraft/derived"` (committed) and
   `state_dir = ".statecraft/state"` (ignored). `.statecraft/` as a whole is
   neither: a file under it that is in neither root is ordinary governed
   territory that every check sees. The coupling gate adds the **configured**
   derived root to its own bypass floor, so no `bypass_prefixes` entry is needed
   for it.
3. **The instruction bridge is blocked on this side.** spec-spine will not add
   `@.statecraft/AGENTS.md` to its root `AGENTS.md` until this product's
   initializer actually writes that file: an import of a file that does not exist
   is a broken instruction. When the initializer writes it, say so, and the
   bridge lands on the spec-spine side in one line.
4. **Spec ids moved.** spec-spine collapsed 27 specs into three and renumbered
   the survivors contiguously (000 through 097). Any citation of a spec-spine
   ordinal written before 2026-09-20, in this corpus, in a pinned adopter, or in
   a merged pull request, names a document that has moved or is gone.
   `docs/corpus-map.md` in that repository resolves both directions; read it
   before trusting an ordinal you did not get from `registry list`.

## 5. What a "one sweep" cleanup needs from each side

**This product**, before spec-spine can delete anything further:

- a harness delivery with a Claude Code adapter that installs the ten skills,
  the four agents and the three remaining hooks to a location a session in an
  arbitrary repository loads;
- the assertions from §3.2 owned wherever the files land;
- `.statecraft/AGENTS.md` written by the initializer, so the bridge is not an
  import of nothing.

**spec-spine**, on the same day that lands:

- delete `.claude/`, delete the two harness test files, remove the remaining
  `extra_hashed_inputs` entries (which restales every shard, so it belongs in
  the same commit), supersede the governing spec, and add the bridge line.

Nothing in the second list is difficult. All of it is blocked on the first.
