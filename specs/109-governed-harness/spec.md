---
id: "109-governed-harness"
title: "Governed harness: the spec-spine kit as the loop this repo runs"
status: draft
created: "2026-09-09"
implementation: complete
depends_on:
  - "110-corpus-merge"
  - "102-crate-scaffold"
establishes:
  - "AGENTS.md"
  - "CLAUDE.md"
  - "Makefile"
  - ".mcp.json"
  - ".claude/settings.json"
  - { kind: directory, path: ".claude/skills/" }
  - { kind: directory, path: ".claude/agents/" }
  - { kind: directory, path: ".claude/rules/" }
extends:
  # 100 established the governance workflow and 110 owns it since the
  # merge; 109 replaces its body with the kit's govern.yml chain under the
  # filename 100 chose, so the owner keeps "there is a governance gate on
  # every PR" and 109 owns its shape.
  - { spec: "110-corpus-merge", unit: ".github/workflows/spec-spine.yml", nature: additive }
  # 110 owns the configuration file; 109 adds the version pin, the hashed
  # governance inputs and the ownership ratchet, which are harness policy.
  - { spec: "110-corpus-merge", unit: "spec-spine.toml", nature: additive }
summary: >
  The harness is what an agent may do in this repository, so it is
  claimed by a spec and held by the coupling gate. This spec adopts the
  spec-spine Claude Code kit (spec-spine 0.18.0, kit spec 081): ten
  repository-invariant skills forming the governed loop, four pipeline
  agents carrying this repo's project layer, four rules, the read-only
  hook set, a Makefile whose `make gate` is the one definition of the
  gate that CI and every session run, and the pin that refuses an older
  binary. It replaces the hand-copied five-skill harness that predated
  `spec-spine check`, and turns on the ownership ratchet now that every
  source file is specifically claimed.
---

# 109: Governed harness

## 1. Purpose

Every earlier session in this repository ran a harness copied by hand from
an early spec-spine kit: five skills, hooks that wrote to the tree at session
end, a PR gate that regenerated the index it was meant to judge, and a CI
workflow pinned to spec-spine 0.10.0 while `/setup` installed whatever was
current. Nothing owned those files, so nothing refused a drift between what
CI enforced and what a session was told to run.

The kit that spec-spine ships since its spec 048 (revised by 064, 074, 075
and 081) is the same loop this repo runs one spec per session: prime, pick,
build, verify, ship, shepherd. Adopting it wholesale, and claiming it here,
gives the harness one owner and the gate one definition.

## 2. Territory

- `AGENTS.md`: the cross-agent init protocol and "Working the backlog",
  the kit template with this repo's project layer filled in.
- `CLAUDE.md`: project overview and conventions; build commands name
  `make gate`.
- `Makefile`: the kit composite (`gate`, `refresh`, `verify`, and the
  manifest-probed `test`, `build`, `fmt`, `clippy`).
- `.claude/settings.json`: the kit hook set and a permission allow-list
  extended with the cargo verbs.
- `.claude/skills/`: the ten kit skills, byte-identical to the kit.
- `.claude/agents/`: the four kit agents with the project layer applied.
- `.claude/rules/`: the three floor rules plus the path-scoped
  `derived-artifacts-are-compiler-output.md`.
- `.mcp.json`: the empty MCP server template.
- `.github/workflows/spec-spine.yml` (extends spec 100): the kit
  `govern.yml` body under the filename 100 established.
- `spec-spine.toml` (extends spec 100): `[meta] required_version`, the
  hashed-input globs, and `[coupling] require_ownership = true`.

Out of the kit's territory and deliberately not shipped: the `.githooks/`
merge driver and its `.gitattributes` stanza. This repo runs one spec per
pull request against committed shard trees, which is the case the kit's own
guidance says wants the staleness gate and not the driver.

## 3. Behavior

### 3.1 The skills are not customized

Every `SKILL.md` is byte-identical to the kit and ends with a
`## Project layer` section naming what it reads from `AGENTS.md`,
`spec-spine.toml` and the rules. A kit update is therefore a copy, not a
merge: `diff -r` against the kit's `.claude/skills/` MUST be empty.

### 3.2 The gate has one definition

`make gate` runs, in order: `spec-spine check --fail-on-warn`,
`spec-spine lint --fail-on-warn`, `spec-spine index coverage
--fail-on-untraced`, `spec-spine couple --base $(BASE) --head HEAD`. The
`Governed loop (pull request)` step in `.github/workflows/spec-spine.yml`
runs the same four verbs with `--pr-body` added to `couple`, and
`AGENTS.md` "Working the backlog" step 5 names `make gate`. The three MUST
stay identical; a step one of them adds and another omits is a step every
session skips.

`--fail-on-unresolved` is off by decision (spec-spine spec 050): spec 108 is
approved and pending, so `statecraft_cli::members` is legitimately
unresolved until it is built. The `Makefile` comment names the condition
under which the flag turns on.

The Rust stack gate (`cargo fmt --check`, `cargo clippy --all-targets --
-D warnings`, `cargo test`, release build) stays in `ci.yml` (spec 102);
the kit's `probe` and `build` jobs are not duplicated in the govern
workflow.

### 3.3 The hooks read and never write

Per spec-spine spec 046: `SessionStart` and `Stop` report freshness through
`spec-spine check`; `PostToolUse` recompiles after a `spec.md` edit (the one
sanctioned write) and checks staleness after a hashed-input edit;
`PreToolUse` refuses a push to the default branch and blocks `gh pr create`
on a stale tree, uncommitted shards, an absent or too-old binary, or a red
coupling gate without an inline `Spec-Drift-Waiver:`. Every hook resolves
the binary as `$SPEC_SPINE_BIN`, then `./target/release/spec-spine`, then
`PATH`, and asks `--version` before believing an exit code.

### 3.4 The binary is pinned

`[meta] required_version = "0.18.0"` (caret range). A binary predating the
`[meta]` table refuses with an unknown-field parse error, which is the pin
working against every older release at once. `AGENTS.md` and `/setup`
name the same floor.

### 3.5 The ownership ratchet is on

`[coupling] require_ownership = true`. `spec-spine index coverage` reports
16/16 source files specifically claimed once spec 102 claims `tests/`
(the integration tests its scaffold created). A new source file in a
feature PR MUST be claimed by the implementing spec in the same change.

### 3.6 The project layer lives in the agents

The four agents carry what the skills do not: the Rust stack gate, the
exit-code discipline (0 ok, 1 operational failure, 2 usage), and the
product-surface guards from `CLAUDE.md` (required `--posture` on stamps,
`--confirm <name>` on fleet remove, no bypass flags, stable `--output json`
envelopes, no local stamping or kubeconfig access). Re-applying that layer
is the one merge step a kit update needs.

## 4. Acceptance

- `spec-spine --version` reports 0.18.0 or later and `spec-spine check`
  exits 0 on the merged tree.
- `make gate` exits 0 on the merged tree; `.github/workflows/spec-spine.yml`
  is green on the PR that lands this spec.
- `diff -r <spec-spine>/kit/.claude/skills .claude/skills` is empty.
- `spec-spine index coverage --fail-on-untraced` exits 0.
- `spec-spine lint --fail-on-warn` exits 0 (the L-008 and L-010 findings the
  old glob form produced are gone).
- `.claude/skills/init/` no longer exists; `/prime` is the init protocol.
- Editing `.claude/settings.json` without editing this spec fails `couple`
  with `C-001`.

## 5. Out of scope

- The `.githooks/` merge driver (see Territory).
- A domain-specialist agent; none is needed while the surface is one crate.
- Codex and other-agent harness directories (`.agents/`, `.codex/`) that
  are not part of the kit; they are not claimed here.

## 6. Status (2026-09-09)

Adopted from spec-spine 0.18.0 / kit at spec 081. Born `draft` with
`implementation: complete`; approval is a human flip.
