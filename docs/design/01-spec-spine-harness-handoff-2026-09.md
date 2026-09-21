<!-- Spec: specs/002-environment-lifecycle/spec.md -->

# What spec-spine still carries, for the adapter that will take it

Prepared 2026-09-20 by a spec-spine session, as input to spec 002 §3.14. A
handoff, not a decision: every disposition below belongs to this product's
owner.

This note was first written to report that spec 002 §3.7 still named
`spec-spine init --with-kit` as a live counterparty. By the time it landed, #45
and #47 had already withdrawn that section and written §3.11 to §3.21, so that
half is deleted rather than merged. What is left is the half this product cannot
read off its own corpus: **what the one repository still holding a harness
actually holds**, and the contracts inside it that were each written after a
measured failure.

## 1. Where spec-spine stands

`spec-spine init`, `--with-kit`, `kit/`, `kit_embedded.rs`, the `.agents/` and
`.codex/` projections and `website/` are gone. No verb writes an `AGENTS.md`, a
`CLAUDE.md`, a `.claude/` directory, a skill, an agent brief, a hook, an MCP
configuration, a CI workflow or a `Makefile`. The retained producer,
`scaffold_init_json`, is a pure function of its argument that emits governance
starter content only, as data, and its tests assert no emitted path begins with
`.claude/`. That is §3.15's boundary, from the other side, and it holds.

spec-spine keeps a `.claude/` tree **for itself**, and is dismantling it in the
order that never leaves it unprotected:

| Class | State on 2026-09-20 |
|---|---|
| `.claude/rules/` (4 files) | **removed.** Folded into that repository's `AGENTS.md` as a `## Rules` section. They were never a candidate for a global home: they are spec-spine's own governance and would bind every project a user opens. |
| the push gate | **installed globally**, at `~/.claude/hooks/push-gate.sh`. Repository-agnostic: `git` and `jq`, no spec-spine. Copied rather than moved, because its tests cannot read `$HOME`. |
| `.claude/skills/` (10), `.claude/agents/` (4) | waiting on this product. |
| `.claude/settings.json` (PR gate, 2 session hooks, permissions) | waiting on this product. |

The ordering spec-spine is holding to: **this product delivers, spec-spine
confirms a session there still has its loop and its hooks, then `.claude/` goes,
with its governing spec superseded in the same change.** Doing it in the other
order is the failure the removal of the kit was written to avoid.

## 2. The inventory

Skills: `prime`, `next`, `build`, `verify`, `ship`, `shepherd`, `spec`,
`commit`, `code-review`, `setup`. Agents: `architect`, `explorer`,
`implementer`, `reviewer`.

They are already **repository-invariant**: every project-specific fact lives in
that project's `AGENTS.md`, which each skill ends by pointing at. That property
was built for a distribution that was then cancelled, and it is what makes
§3.14's "maintained once, copied into no repository" viable for them unchanged.

Three assertions in that set are worth keeping wherever the files land: no skill
names a gate flag its project's `AGENTS.md` omits; a read-only skill never
invokes a writing verb; each skill wraps the tool verbs it exists for.

## 3. The hook contracts

The part that is expensive to rediscover. Each was written after a measured
failure.

- **Read, never repair.** No hook may invoke a writing subcommand. A hook fires
  where it cannot commit what it regenerated, so a writing hook leaves the
  derived tree dirty; an orchestrator that refuses to start on a dirty tree then
  never starts, and one adopter's pipeline stalled eleven hours on dirt it had
  produced itself. The single sanctioned exception is a `compile` after an edit
  to a `spec.md`, where the session is live and can commit the result.
- **Binary resolution order**: `$SPEC_SPINE_BIN`, then the target repository's
  own `target/release/spec-spine`, then `PATH`. A repository that builds its own
  binary must be governed by the one it builds; the `PATH` fallback keeps an
  adopter on the published CLI working.
- **Resolve the target repository from the command, not from the session.** A
  multi-repository session pushes and edits in whichever tree the command names.
- **Read the verdict; never guess it.** `spec-spine check` has four answers and
  they are not interchangeable: `0` fresh, `1` a corpus that does not validate,
  `2` stale **or** an unresolved claim, `3` a read that was not performed. Only
  one of the four is repaired by regenerating. `clap` also spends `2` on an
  unknown subcommand, so a hook must establish the binary carries the verb
  (`check --help`) **before** reading its exit code, or a binary older than the
  verb reports a fresh tree as stale and sends the session to regenerate shards
  that were already correct.
- **A gate whose check did not run is not green.** Every non-zero code refuses.
- **The push gate** resolves the protected branch rather than assuming `main`:
  `$SPEC_SPINE_DEFAULT_BRANCH`, then the remote's own `HEAD`, then `main` as a
  floor. It refuses only a push that would actually update that branch, so a tag
  push from the default branch is allowed. It is anchored on the command that
  invokes the verb, so a `grep` or a heredoc merely containing the text still
  runs.
- **The PR gate** runs the coupling gate before `gh pr create` and refuses
  without a human-written waiver line in the body.

`.claude/settings.json` also carries a deny list: no `cargo publish`, no
`npm publish`, no `gh release create`, no force push, no `rm -rf` of the corpus
or the derived tree. That is a safety floor, not an adapter's optional extra.

**Whoever owns the files owns the assertions.** In spec-spine they are
`crates/spec-spine-core/tests/harness_hooks.rs` (1124 lines, which extracts each
hook body and runs it as a program over a matrix of command spellings and branch
names) and `harness_skills.rs` (~800). A hermetic test cannot read `$HOME`, so
these do not survive the move on their own: they are either reimplemented where
the files land, or the requirements become unenforced. It is not a large amount
of code, and it is the reason the ordering above is not negotiable.

## 4. Two facts worth checking against

1. **Spec ids moved.** spec-spine collapsed 27 specs into three and renumbered
   the survivors contiguously (000 through 097) on 2026-09-20. Any citation of a
   spec-spine ordinal written before that names a document that has moved or is
   gone. `docs/corpus-map.md` there resolves both directions; read it before
   trusting an ordinal you did not get from `registry list`.
2. **The instruction bridge is waiting on this side, by design.** spec-spine
   will not add `@.statecraft/AGENTS.md` to its root `AGENTS.md` until this
   product's initializer actually writes that file: an import of a file that does
   not exist is a broken instruction. §3.13 is the rule; when the initializer
   writes it, say so, and the bridge lands on the spec-spine side in one line.
