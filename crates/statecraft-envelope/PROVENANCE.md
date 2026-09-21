# Provenance of `statecraft-envelope`

This crate was not written here. It is recorded rather than absorbed, because a
crate that arrives with no history is a crate whose decisions nobody can
re-derive.

## Where it came from

| | |
|---|---|
| Origin repository | `statecraft` (the platform), `https://github.com/statecrafting/statecraft` |
| Origin commit | `d7dc24b`, 2026-09-16, "adopt: record P-01 to P-13 and A-01 to A-08, ratify 001 to 005, resolve the shared contract, implement spec 003's foundation" |
| Origin path | `crates/statecraft-envelope/` |
| Origin spec | statecraft spec `003-evidence-intake-and-storage`, sections 3.1 to 3.5 |
| Status there | **staged**, explicitly pending transfer to this workspace (the platform's P-03 condition, and resolution R-7 of its `docs/design/03-shared-contract-resolution.md`) |
| Licence there | `license = "Apache-2.0"` at crate level, deliberately unlike that repository's own LICENSE |
| Received under | statecraft-cli spec `005-acceptance-and-evidence`, sections 3.11 to 3.17 |

The crate was staged in the platform repository and never consumed there as a
finished thing: the platform's own design record says the CLI does not consume
it until it is transferred, and that the transfer is a CLI change that claims
it. This is that change.

## What it was written from

The crate's contents are derived from three sources, and every module names its
own:

- **hqgit's format descriptions** (specs 011, 013, 017, 019, 020, 027, 064 at
  `e4450f6`) for canonical DAG-CBOR, BLAKE3 identity, the entry, the
  attestation, the tombstone and the anchor origin.
- **statecraft-cli spec 005** for the reference, the constructions, the four
  dimension enums, admission, the refusal codes, the policy and the three names
  for absence. Those serializations were read out of this repository's source at
  `ee0fa7d` and preserved byte for byte.
- **the platform's specs 003 and 004** for the verdict, the root set, the
  admission evaluator and the portable-input scanner.

## The licence

`Cargo.toml` states `license = "Apache-2.0"` literally rather than inheriting
the workspace value, even though the workspace value is the same today. The
crate crossed a repository boundary whose LICENSE is not Apache-2.0, and the
term it crossed under belongs to the crate. An inherited licence would say the
same words today and would silently stop saying them if the workspace ever
changed. This repository's `LICENSE` is untouched, as the source ownership table
in `AGENTS.md` requires.

## What changed on the way in

Nothing that changes a byte on the wire. In full:

1. **`Cargo.toml`** rewritten for this workspace: explicit dependency versions
   in this repository's style instead of `workspace = true` inheritance, the
   `[package.metadata.spec-spine]` pointer moved from the platform's spec 003 to
   this repository's spec 005, the lint tables stated per crate, and the
   provenance and licence notes above.
2. **`src/lib.rs`**: `#![forbid(unsafe_code)]` stated in the crate (the platform
   carried it as a workspace lint), and the module header updated to say where
   the crate now lives.
3. **`src/absence.rs`**: rewritten to resolve the `Recorded<T>` wire ambiguity
   the platform's resolution R-8 recorded as a finding for this repository's
   owner. Variant order was the staged crate's answer and is not a complete one;
   see spec 005 section 3.12 and the module's own documentation. The encoding is
   unchanged; what is new is that the ambiguous encoding is refused.
4. **`src/dimensions.rs`**: `Dimensions::unsigned_today` (this product's spec 005
   section 3.5 constructor, which had stayed behind) and `Admission::reasons`
   (reads a refusal's reasons whether the producer wrote one or many) added.
   Both additive, neither changes a serialization.
5. **`testdata/fixtures/cli/`**: the eight hand-derived fixtures replaced with
   bytes emitted by this repository's own serializer, and seven added. All eight
   were byte-identical to the hand-derived ones before replacement.
6. **`tests/compat.rs`** became `tests/cli_compat.rs`, extended with the frozen
   CLI encoder in `tests/legacy_cli/`, semantic assertions beside the byte
   assertions, and the reserved-string and unknown-member cases.

The eleven remaining `src/` modules and the five golden vectors arrived
unmodified.

## What is still provisional

`testdata/vectors/` holds five **provisional** golden vectors for the native
shapes (entry, fact, tombstone, attestation). The transfer does not freeze them,
and neither does spec 005. They freeze at the platform's first signed entry,
under the platform's own constitution VIII; that entry does not exist. Until it
does, a vector change is an ordinary change. See `tests/vectors.rs`.
