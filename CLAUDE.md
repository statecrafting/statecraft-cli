# CLAUDE.md: statecraft-cli

## Project Overview

The successor to OPC: one Rust binary named `statecraft` with two
faces, CLI subcommands for humans and an MCP server (stdio) for
agents, both calling the Statecraft control plane's API under the same
identity, guards, and JSON shapes. Thesis and decided constraints:
`specs/001-cli-mcp-thesis/spec.md`. The backlog is the spec corpus:
`spec-spine registry plan` names the ready set, and one session
implements one spec (`AGENTS.md`, "Working the backlog").

## Repository Structure

```
specs/       Feature specs, the authoritative design record
standards/   spec-spine constitution, contract, templates
.derived/    Compiler output (committed shards; never hand-edit)
.claude/     the spec-spine kit: skills, agents, rules, hooks (spec 009)
src/ tests/  the statecraft crate (spec 002 onward)
Makefile     `make gate` (read-only governed loop), `make refresh`
```

## Governance

Governed by spec-spine 0.18.0 or later (`spec-spine.toml`, owned by
spec 000; the harness by spec 009): specs are the source of truth;
read `.derived/**` only through `spec-spine` subcommands; after editing
any `specs/*/spec.md`, run `make refresh` and commit the shards with
the edit. Before every commit, `make gate` and the cargo gates below
must exit 0. `[coupling] require_ownership` is on: claim every new
source file in the implementing spec, in the same change.

## Build Commands

```bash
make refresh                 # spec-spine compile && spec-spine index
make gate                    # check, lint, index coverage, couple (read-only)
make verify SPEC=00N         # one spec's declared acceptance
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release        # target/release/statecraft
```

## Key Conventions

- **Guards are product surface.** The required `--posture` flag on
  stamps and the `--confirm <name>` on fleet remove exist by design;
  never add a bypass flag.
- **JSON output shapes are API.** The MCP face reuses the CLI's JSON
  envelopes; treat them as versioned contracts from the first verb.
- **The CLI never bypasses the platform.** No local stamping, no
  direct kubeconfig access; it triggers and watches governed verbs.
- Apache-2.0; rustls only (no native-tls).
