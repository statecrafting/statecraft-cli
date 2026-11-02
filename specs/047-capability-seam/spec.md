---
id: "047-capability-seam"
title: "Admission and qualification read what an adapter declares it can do, never which adapter it is"
status: approved
implementation: complete
created: "2026-10-21"
summary: >
  Amends spec 004's seam with a per-adapter capability declaration beside the
  closed six-token vocabulary: the facts admission, qualification and the
  witness path currently learn by naming the one adapter (a stream it can
  witness, hooks it honours in print mode, a wire sidecar it can route
  through, a per-run configuration directory). Every branch reads the
  declaration. A test adapter with a different declaration proves the seam.
  No second provider is added.
amends:
  - "004-execution-adapter"
  - "013-supervised-wire-witness"
  - "042-per-run-provider-configuration"
extends:
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter-claude-code/" }, nature: additive }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "004-execution-adapter"
  - "013-supervised-wire-witness"
  - "042-per-run-provider-configuration"
obligations:
  - id: "I-1"
    kind: invariant
    text: "No admission, qualification or launch decision outside an adapter's own crate compares an adapter's name; each reads a field of its capability declaration."
    anchor: "3-2-branch-on-the-declaration"
  - id: "R-1"
    kind: requirement
    text: "An adapter declares every capability field explicitly; a field it does not set is refused at registration, never defaulted to the first adapter's behaviour."
    anchor: "3-1-the-declaration"
  - id: "R-2"
    kind: requirement
    text: "A run that needs a capability its adapter does not declare refuses before spawn, naming the field, exactly as a required token does under spec 004 section 3.3."
    anchor: "3-3-refusal"
  - id: "V-1"
    kind: verification
    text: "A test adapter declaring no stream witness, no print-mode hooks and no sidecar is refused for a witnessed run and admitted for a plain one, through the same admission code the Claude Code adapter uses; a source check finds no adapter-name comparison outside adapter crates."
    anchor: "verification"
    inputs:
      - "crates/statecraft-adapter/tests/no_provider_names.rs"
---

# 047: Admission and qualification read what an adapter declares

## 1. Purpose

Spec 004 keeps provider names out of the seam crate, and six capability
tokens describe what a provider can assert about a session. Several decisions
outside that vocabulary are still made by reaching into the one adapter's
crate: `statecraft-home` admission builds Claude Code's argument list and
reads its version field, the witness path assumes its stream can be routed
through spec 013's sidecar, and spec 042 assumes its per-run configuration
directory. Each is correct for the one adapter and silently wrong for a
second. Spec B8 (a Codex adapter) is drafted to follow this one; this spec
makes those decisions read a declaration first, so the second adapter is a
new declaration, not a new branch in every caller.

## 2. Territory

`statecraft-adapter` gains the declaration type and its refusal;
`statecraft-adapter-claude-code` declares its values; `statecraft-home`
admission and `statecraft-cli` launch and witness read the declaration; a test
adapter lives under `statecraft-adapter`'s tests. No provider is added.

## 3. Behavior

### 3.1 The declaration

Beside `Manifest`, an adapter supplies `AdapterCapabilities`, a struct with
one field per fact a caller decides on, modelled on preloop-ci's
`ProviderCapabilities` (`crates/preloop-vm/src/lib.rs`, MIT), re-implemented:

| Field | Meaning |
|---|---|
| `stream_witness` | Its provider stream can be captured by spec 013's sidecar. |
| `print_mode_hooks` | Its provider runs repository hooks in non-interactive mode and reports a blocked action as an event. |
| `wire_sidecar` | Its provider can be routed through a loopback endpoint the supervisor reserves. |
| `per_run_config_dir` | It accepts a per-run configuration directory (spec 042) instead of the user's global one. |
| `settings_file_transport` | It reads a settings document from a file path the supervisor supplies. |
| `version_field` | The init field it reports the provider version in, or none. |

Unlike preloop-ci's, nothing defaults: an adapter sets every field, and
registration refuses a declaration with a field unset. Adding a field is an
amendment, as adding a token is under spec 004 section 3.2.

### 3.2 Branch on the declaration

Admission, qualification, `run --witness` and the per-run configuration
read the selected adapter's declaration. No code outside an adapter's own
crate compares an adapter name to decide anything; the existing source check
(`no_provider_names.rs`) is extended to the callers.

### 3.3 Refusal

A run that needs a field the adapter declares `false` (`--witness` and
`stream_witness`, for example) refuses before spawn with the stable reason
`capability-undeclared` naming the field, recorded like a required token's
refusal; a run that only prefers it runs, degraded and recorded.

### 3.4 The test adapter

A fixture adapter under the seam's tests declares `stream_witness`,
`print_mode_hooks` and `wire_sidecar` false. Through the same admission code,
a witnessed run with it is refused naming `stream_witness`, and a plain run
is admitted. It runs no provider.

## 4. Out of scope

- Any second provider (spec B8), and any change to the six tokens.
- Changing what the Claude Code adapter does; its declaration states today's
  behaviour.

## 5. Resolved decisions

**2026-10-21: explicit fields, no defaults.** preloop-ci defaults every field
to its first backend's behaviour so existing providers need no change. Here
the first adapter's behaviour is exactly what a second must not inherit by
accident, so an unset field refuses.

**2026-11-01: an adapter's own declaration is a struct literal; data goes
through `register`.** There is no `Default`, so a declaration written in Rust
with a field left out does not compile, and one supplied as data is refused by
`register`, naming the first unset field; an explicit `null` version field is
set to none, an absent one is unset.

**2026-11-01: what each caller needs.** `run` requires `per_run_config_dir`
and `settings_file_transport` on every launch, `print_mode_hooks` on a managed
one, and `stream_witness` with `wire_sidecar` under `--require-witness`; under
`--witness` alone those two are preferred, and when undeclared the sidecar is
not started and the outcome records `capabilityDegraded`. The refusal is
concluded beside spec 004's token preflight, under the guard
`capability-undeclared`. The qualification admission's launching operation
requires `print_mode_hooks`, `settings_file_transport` and `version_field`,
and its incomplete-init refusal names the declared version field.

**2026-11-01: the source check reads adapter names, not harness identities.**
A comparison against a literal opening with a provider name, or against an
adapter-name constant, in any crate's `src/` other than the adapter crates and
before its unit-test module, is a finding. Spec 002's harness identity is a
separate name and stays outside this check.


**2026-11-01: a field named in JSON is spelled by the naming convention.**
Where a refusal or a degraded outcome lists the fields it lacked, each is a
string enum value and so kebab-case under spec 006's JSON naming convention:
`stream_witness` is written `stream-witness`. Messages name a field as this
spec spells it.
## Verification

Ratified by the owner on 2026-10-31. Acceptance is declared by the
implementing change, which will run the test adapter through admission for a
witnessed and a plain run, the Claude Code adapter's suites unchanged, and the
extended source check.

```verify:cli
cargo test -p statecraft-adapter --test capability_seam
cargo test -p statecraft-adapter --test no_provider_names
cargo test -p statecraft-adapter --lib capabilities
cargo test -p statecraft-adapter-claude-code
cargo test -p statecraft-home --lib admission
cargo test -p statecraft-cli --lib adapters
```

The first runs the fixture adapter through `Needs::launch` and `fit`, the code
`run` reads: a witnessed run is refused naming `stream_witness`, a plain one is
admitted, and a declaration with a field unset is refused at registration. The
second is the extended source check; the fourth is the Claude Code adapter's
suites, its declaration included; the last two show that adapter's
declaration fits every launch it ran before.
