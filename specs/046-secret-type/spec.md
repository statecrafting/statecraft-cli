---
id: "046-secret-type"
title: "One secret type: redacted when shown, cleared when dropped, exposed only by name"
status: approved
implementation: complete
created: "2026-10-21"
summary: >
  Adds a small crate owning one secret-string type whose Debug and Display
  never show its value, whose bytes are zeroed when dropped, and whose value
  is read only through an explicit expose call. The provider token
  (statecraft-home) and the hosted credential (statecraft-hosted) migrate to
  it. Four structural rules adapted from preloop-ci are specified here and
  delivered in a later change that depends on spec 044's sg-scan.
amends:
  - "002-environment-lifecycle"
  - "038-hosted-client"
  - "042-per-run-provider-configuration"
establishes:
  - { kind: directory, path: "crates/statecraft-secret/" }
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "038-hosted-client", unit: { kind: directory, path: "crates/statecraft-hosted/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "038-hosted-client"
  - "042-per-run-provider-configuration"
# Spec 044 is not a dependency of this spec's acceptance: section 3.5 defers
# the rules to a change that adds the 044 edge once 044 is on main.
obligations:
  - id: "I-1"
    kind: invariant
    text: "No Debug, Display or serialization of a secret value shows it; the only way to read it is a call named expose."
    anchor: "3-1-the-type"
  - id: "R-1"
    kind: requirement
    text: "The provider token and every hosted access and refresh token are held in the secret type from the moment they are read until they are dropped."
    anchor: "3-3-migration"
  - id: "R-2"
    kind: requirement
    text: "A secret's bytes are overwritten with zeros when its holder drops it, including on an early return or a panic that unwinds."
    anchor: "3-1-the-type"
  - id: "V-1"
    kind: verification
    text: "Unit tests show redacted Debug and Display, refused serialization, zeroing on drop observed through a test hook, and that the two migrated holders render no value."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/src/provider_token.rs"
      - "crates/statecraft-hosted/src/credential.rs"
---

# 046: One secret type

## 1. Purpose

Two values this product holds are bearer material: the provider token spec
042 stores for a per-run configuration (`statecraft-home::provider_token`,
returned as a plain `String`) and the hosted access and refresh tokens spec
038 holds (`statecraft-hosted::credential::Credential`, plain `String` fields
with a hand-written redacting `Debug`). Each protection is local and
remembered: a new `derive(Debug)`, a `format!` or a log line can print either,
and nothing clears their bytes when they are dropped. One type makes the
protection structural, and the structural rules make the remaining ways to
leak one a check rather than a habit.

## 2. Territory

A new crate, `crates/statecraft-secret/`, which this spec establishes and owns:
it holds the type and nothing else, and depends only on `zeroize` (already in
the lockfile through `statecraft-cli`) and `serde`. Both `statecraft-home` and
`statecraft-hosted` depend on it; `statecraft-hosted` depends on no other
statecraft crate today, and a leaf crate keeps that true. The four rules of
section 3.4 are specified here and are not part of this spec's acceptance.

## 3. Behavior

### 3.1 The type

`SecretString` owns its bytes. `Debug` and `Display` both render
`[redacted]`, never the value, its length or a prefix. It implements
`Deserialize`, so a credential file can be read into it, and does **not**
implement `Serialize`: writing a secret is an explicit
`expose()`-and-write at the one site that persists it. `expose(&self) ->
&str` is the only accessor. `Clone` is implemented, so a holder may keep a
copy; each copy zeroes its own bytes on drop. Equality is not implemented on
the value, so a comparison cannot leak through timing or a test assertion.

### 3.2 Zeroing

Dropping a `SecretString` overwrites its buffer with zeros through `zeroize`,
which the compiler may not elide, before the memory is freed. This covers an
early return and an unwinding panic. It does not cover copies the operating
system or an allocator made before the value reached the type, a swapped-out
page, or bytes a child process received; those remain outside this spec.

### 3.3 Migration

`provider_token::read` returns `SecretString`; its caller in
`statecraft-home::confinement` exposes it once, at the point it writes the
per-run configuration file. `Credential`'s `access_token` and
`refresh_token` become `SecretString` and `Option<SecretString>`; its
hand-written `Debug` is replaced by the derived one, which now redacts by
type; `Store::save` exposes each once to write the file. The on-disk formats,
the file modes and every refusal are unchanged.

### 3.4 The four rules

Adapted from preloop-ci (MIT License, copyright 2026 Preloop contributors),
`rules/no-expose-in-loop.yml`, `no-inline-masking.yml`,
`no-raw-secret-replace.yml` and `no-sensitive-log-fields.yml`, rewritten for
this codebase, which has no masker and no `tracing`:

1. `no-expose-in-loop`: `.expose()` inside a `for` body or a closure is a
   warning; expose once, outside.
2. `no-secret-in-format`: an exposed value passed to `format!`, `print!`,
   `println!`, `eprint!`, `eprintln!`, `write!` or `writeln!` is an error.
3. `no-secret-splice`: `.replace(` with an exposed value as an argument is an
   error; nothing in this product rewrites text around a secret.
4. `no-sensitive-print-fields`: a print or format macro naming a binding
   called `token`, `access_token`, `refresh_token`, `authorization` or
   `cookie` is an error.

Each rule ships fixture files it must flag and is run over the tree, which
it must pass. Nothing in this repository runs `ast-grep` until spec 044's
`sg-scan` is adopted, so the rules land with that adoption (section 3.5).

### 3.5 Delivery order

Three changes: a dependency-only pull request adding the crate's manifest,
the workspace dependency on `zeroize` and the two dependent edges (nothing
else); then the crate and the migration, which complete this spec; then,
only once spec 044 is ratified and on main, a change that adds the rules
under `rules/` with an `sgconfig.yml`, adds `044-supply-chain-profile` to
this spec's `depends_on`, and records it with a dated section 5 entry. Until
that change, the type's redaction is enforced by the type and its tests, and
the rules are not claimed to run.

## 4. Out of scope

- Secrets passed to a child process's environment or files; spec 042 and the
  B3 gateway decide those.
- Memory locking, guard pages and core-dump suppression.
- Any key material in `statecraft-envelope`, which `trust` already wraps in
  `zeroize::Zeroizing`.

## 5. Resolved decisions

**2026-10-21: a leaf crate, not a module in either holder.** The provider
token is in `statecraft-home` and the credential in `statecraft-hosted`, which
does not depend on home; a type in either would make one depend on the other.
A leaf crate with one type is the smallest owner both can use.

**2026-10-31: the holder is not comparable either.** Section 3.1 withholds
equality from the value, so `Credential` drops its derived `PartialEq` and
`Eq` rather than hand-writing one that compares exposed values; the one test
that compared two credentials compares their fields, each token through
`expose`. The zeroing hook of V-1 is a `cfg(test)` thread-local in the crate
that records each dropped buffer after its first zeroing pass, so a unit test
observes the zeros without unsafe code; the second pass clears the spare
capacity. `zeroize` is stated once, in the root manifest's workspace
dependencies, and `statecraft-cli` takes it from there.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is the crate's unit and
documentation tests (the latter hold the two `compile_fail` cases: no
`Serialize`, no equality) and the two migrated holders' suites, run with no
provider and no cell.

```verify:cli
cargo test -p statecraft-secret --locked
cargo test -p statecraft-home --locked --lib provider_token
cargo test -p statecraft-hosted --locked --test hosted
```

The rules' fixtures and tree run join it with the section 3.5 follow-up.
