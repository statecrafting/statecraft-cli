---
id: "007-family-envelope"
title: "The family envelope: every --json answer in the exit and JSON contract spec-spine speaks"
status: draft
implementation: in-progress
created: "2026-09-25"
summary: >
  Amends 006 sections 3.3 and 3.4. Every --json answer this product writes
  becomes the envelope spec-spine has written since 0.26.0 (its spec 132
  section 3.4): schemaVersion, tool, verb, outcome, exitCode, summary, then
  report for exit 0 or 1 and error for 2, 3 or 4, where error.kind is one of
  the family's ten tokens. No exit number moves. A usage error under --json is
  an envelope on stdout. The replacement of 002 section 3.23's check
  translation, which #118 proposed for the same change, is left to the
  spec-spine 0.28.0 adoption, which changes the producer answer it would read.
  Replaces the section 5 proposal of #118, which the amendment model adopted
  on 2026-09-25 (001 section 5) no longer admits.
amends:
  # Section 3.4's rendering ({value, exit, summary}) is replaced and 3.3's
  # vocabulary gains a JSON spelling. 006 is not edited to record it.
  - "006-command-surface"
extends:
  # Section 3.1 changes the rendering in crates/statecraft-cli/src/render.rs and
  # every binding that constructs a refusal or failure. `amends` does not make
  # this spec an owner of 006's code (001 section 5, the amendment model, Part 1
  # item 1), so the edge is declared here, in the same change.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: corrective }
depends_on:
  - "001-boundaries-and-authority"
  - "006-command-surface"
---

# 007: The family envelope

## 1. Purpose

spec-spine and this product are one family: an operator scripts against both,
and Statecraft reads spec-spine's answers. From 0.26.0 spec-spine writes every
verdict in one envelope with one exit table (0 ok, 1 finding, 2 refused, 3
usage, 4 failed) and a closed set of error kinds. This product already has the
same five exit numbers (006 section 3.3) but writes its JSON as
`{value, exit, summary}`, so a caller reading both tools parses two header
shapes and learns a refusal's class from prose.

This spec makes the product's `--json` the family envelope, and moves nothing
else. It was proposed as a dated 006 section 5 entry in #118 (2026-09-24),
before the amendment model; under that model a change to what an approved spec
requires is a new spec with an `amends` edge, and this is it.

## 2. Territory

None of its own. The code it changes is 006's, reached through the `extends`
edge above: the rendering in `crates/statecraft-cli/src/render.rs`, the verb's
dotted name in `commands.rs`, the usage and failure paths in `main.rs`, and the
sites in `bind.rs` and `slice.rs` that name a specific error kind.

## 3. Behavior

### 3.1 The envelope

Every `--json` answer is one JSON object on stdout with these members and no
others:

| Member | Value |
|---|---|
| `schemaVersion` | `"1.0.0"`, this envelope's own version, on its own axis. Adding a member is MINOR; removing or retyping one, or an `error.kind` outside section 3.2, is MAJOR. |
| `tool` | `"statecraft-cli"`, the executable's recorded name (006 section 3.5). |
| `verb` | The operation's spelling with its words joined by `.`: `env.apply`, `work.list`, `doctor`, as spec-spine names `index.check`. |
| `outcome` | The exit's word: `ok`, `finding`, `refused`, `usage` or `failed`. |
| `exitCode` | The exit's number, equal to the process status. |
| `summary` | The human rendering, without its trailing newline. |
| `report` | Present exactly when the exit is 0 or 1: the value 006 section 3.4 renders, its own fields unchanged. |
| `error` | Present exactly when the exit is 2, 3 or 4: `{kind, message, details?}`. `message` equals `summary`; `details` is the value the answer already carried, omitted when it would only repeat `message`. |

Keys are sorted and the object ends with one newline, as spec-spine writes its
envelope. `value` and `exit` are gone: that is the removal 006 section 3.4
calls a change to the spec, and it is why this is an amendment rather than an
additive change. Human output is unchanged.

### 3.2 Error kinds

`error.kind` is one of the family's ten tokens, spelled as spec-spine spells
them: `validation`, `stale`, `not-found`, `drift`, `refused`, `config`, `io`,
`schema`, `usage`, `internal`. The set is closed: a refusal or failure that
fits none is a finding to report to the family, not a reason for a local
token.

Each exit has a default kind, and a site that knows better names its own:

| Exit | Default | Sites that name their own |
|---|---|---|
| 2 | `refused` | an unregistered target (`bind::unregistered_answer`, a `RegistryError::NotRegistered`) is `not-found`; a producer report (`slice::report_error_answer`) passes the producer's class through: a stale ledger is `stale`, a report without the shape read (a missing field, an unreadable report) is `schema` |
| 3 | `usage` | none |
| 4 | `io` | none today: every failure this product constructs is a read or write of its own store or the target that did not complete |

`internal` is for a defect in this product. A finer kind at a site that has
the default is a MINOR change.

### 3.3 Usage errors

Without `--json`, a usage error is unchanged: text on stderr, nothing on
stdout, exit 3. With `--json` anywhere in the arguments, it is an envelope on
stdout with `error.kind` `usage`, exit 3, and nothing on stderr. Its `verb` is
the operation's dotted name when the words name one, and otherwise the first
two words as typed, dotted, because a verb that does not exist has no name to
give.

### 3.4 What does not change

- **No exit number moves** for any verb or any condition. 006 section 3.3's
  table stands, including owner decision I-3 (a withheld path is `partial`,
  exit 1).
- **No payload changes.** Every field a verb reported in `value` is in
  `report` or `error.details` under the same name.
- **002 section 3.23's `check` translation stays.** #118 proposed removing it
  in the same change, reading the producer's `error.kind` instead. spec-spine
  0.28.0 (its spec 152) changes the answer that would be read: `check --json`
  stops reporting an unresolved claim as `"fresh": false` and names it in
  `unresolvedClaims`. Building the removal against 0.27.0's answer would be
  rebuilt at that adoption, so it belongs to it, as its own amendment of 002.

## 4. Out of scope

- The removal of the `check` translation (section 3.4).
- Help output. `--help` answers text, with or without `--json`, because it is
  not an operation (006 section 3.1).
- Any document other than a verb's answer: the manifest, run records and
  captures keep their own schemas.

## 5. Decisions recorded during implementation

**2026-09-25: exit 4 defaults to `io`, not `internal`.** #118 assigned `fail()`
in `main.rs` `io` and left the other failure sites unassigned. Every one of
them (a record not stored, a manifest not read or written durably, a plan's
read of the target) is a read or write that did not complete, so the default
is `io` and `internal` is left for a defect, which is what spec-spine means by
it.

**2026-09-25: `details` is omitted when it repeats `message`.** Most refusals
carry their detail string as their value, and repeating it under `details`
would give a consumer two copies to disagree. spec-spine's `error` carries
`violations` in the same place for the same reason: structure, when there is
some.

**2026-09-25: every binary test holds the envelope.** Every test that parses
`--json` output already parses it through `tests/support/json_naming.rs`,
which now also asserts section 3.1 on each answer (the header, `outcome` and
`exitCode` agreeing, exactly one of `report` and `error`, a kind from the
set). `tests/family_envelope.rs` adds the one test that runs all 42 operations.

## Verification

Each line is one command. The binary tests spawn the built executable, because
what the envelope promises (that `exitCode` is the process status) is a
property of a process.

```verify:cli
cargo test -p statecraft-cli --lib render::
cargo test -p statecraft-cli --test family_envelope
cargo test -p statecraft-cli --test json_naming
```
