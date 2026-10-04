---
id: "035-offline-issuer-contract"
title: "Offline issuer authorization: three fixed domains, detached owner signatures and the pinned RootSetV2"
status: draft
implementation: in-progress
created: "2026-10-03"
summary: >
  Amends 005 for the shared half of the offline issuer enrollment contract the
  owner approved on 2026-10-03 (docs/proposals/007-offline-enrolment-contract.md).
  Adds three closed signing domains (trust.issuer-enrolment,
  trust.issuer-rotation, trust.issuer-revocation) under the unchanged family
  preimage, strict key, signature and payload decoding, the detached signature
  and owner public records, the RootSetV2 reader with its digest and complete
  history verification, issuer eligibility at an explicit time and scope, and
  the operator version and digest pin that refuses rollback. The envelope crate
  stays pure: no file, clock, entropy or network. Custody and the offline verbs
  are 036's; adoption in a cell is the platform's.
amends:
  - "005-acceptance-and-evidence"
extends:
  # The new module and its suite live in 005's crate; `amends` does not make
  # this spec an owner of that code (001 section 5).
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-envelope/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "005-acceptance-and-evidence"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Each of the three record kinds signs under exactly one fixed domain with the unchanged family preimage; cross-kind and cross-family verification fails even for identical bytes."
    anchor: "3-1-domains-and-strict-encodings"
  - id: "R-2"
    kind: requirement
    text: "Enrollment, rotation and revocation payloads decode strictly from exact canonical bytes and bind to the exact original identity.created fact."
    anchor: "3-2-payloads"
  - id: "R-3"
    kind: requirement
    text: "The detached signature and owner public records have exactly their named members; selectors are checked and grant no authority."
    anchor: "3-3-detached-records"
  - id: "R-4"
    kind: requirement
    text: "A RootSetV2 is read strictly, verified as a complete history in which any defect refuses the whole set, and named by the digest of its complete canonical encoding."
    anchor: "3-4-rootsetv2"
  - id: "R-5"
    kind: requirement
    text: "Issuer eligibility is answered at an explicit evidence time and scope with the stated rotation, revocation and window rules, and no clock is read."
    anchor: "3-5-eligibility"
  - id: "R-6"
    kind: requirement
    text: "Only the operator pin of an exact version and digest admits a set; rollback and same-version replacement refuse."
    anchor: "3-6-operator-pin"
  - id: "R-7"
    kind: requirement
    text: "A V2 document is never handed to a permissive reader, and a legacy V1 set cannot establish a trusted V2 issuer."
    anchor: "3-7-the-boundary-reader"
---

# 035: Offline issuer authorization

## 1. Purpose

The platform's issuer key is online. Its trust must come from an offline owner
root that never enters a cell, a backup or a DAG, or a verifier holding only the
evidence can read who the issuer says it is and cannot conclude that anyone
vouched for it. The owner approved the contract for that root on 2026-10-03,
in `docs/proposals/007-offline-enrolment-contract.md`, and directed that the
shared envelope crate carry its formats and verifier under 005 and the command
surface carry the offline verbs under 006.

This spec is the 005 half. It changes what 005 section 3.6 permits a root set
to be, adds signing domains to a crate whose preimage 005 freezes, and adds a
second root-set wire shape; each is a change to what an approved spec requires,
so under 001 section 5 it is a new spec with an `amends` edge. Owner approval of
the direction is not ratification of this text, and nothing here claims the
contract is released or adopted by any cell.

## 2. Territory

None of its own. The module `crates/statecraft-envelope/src/issuer.rs`, the
three domain constants and the strict verification method in `src/sign.rs`, the
suite `tests/issuer_contract.rs` and the vectors under
`testdata/vectors/issuer/` are in 005's crate, reached through the `extends`
edge above and already claimed by 005's directory unit.

The crate stays what 005 section 3.11 made it: every function is pure. It owns
formats and verification; it reads no file, clock or environment, generates no
entropy and holds no seed in any serialized type.

## 3. Behavior

### 3.1 Domains and strict encodings

The preimage is unchanged byte for byte:
`ASCII("statecraft-envelope/v1\n") || ASCII(domain) || 0x0a || payload_bytes`.
The closed domain list gains three constants:

| Constant | Domain | Payload |
|---|---|---|
| `DOMAIN_ISSUER_ENROLMENT` | `trust.issuer-enrolment` | The canonical enrollment map of section 3.2 |
| `DOMAIN_ISSUER_ROTATION` | `trust.issuer-rotation` | The exact canonical `identity.key_rotated` v1 fact envelope |
| `DOMAIN_ISSUER_REVOCATION` | `trust.issuer-revocation` | The exact canonical `identity.key_revoked` v1 fact envelope |

The record kind alone selects the domain; no input string or payload member
chooses it. An entry or attestation signature is none of these, and a
signature under any domain fails under every other, even over identical bytes.

A public key is 64 lowercase hexadecimal characters for a valid, non-weak
Ed25519 point. A key id is BLAKE3-256 of the raw 32 public-key bytes, as 64
lowercase hex. A signature is 128 lowercase hex for 64 bytes. Wrong lengths,
non-hex, uppercase, invalid or small-order keys are refused, and the new
records verify with strict Ed25519 (`verify_strict`), which also refuses a
small-order `R` and a noncanonical `S`. The existing `verify` keeps its rule for
entries and attestations. Strict parsing messages never echo their input.

### 3.2 Payloads

**Enrollment.** The platform's existing canonical map, snake_case keys:
`{issuer_id, public_key, scope, not_before?, not_after?}`. `issuer_id` is the
lowercase-hex BLAKE3 of the issuer's `identity.created` bytes; `public_key` is
the online key; `scope` is `platform`; each window is `{physical, logical}`.
The signed bytes are the canonical DAG-CBOR of that map, never JSON text, CBOR
of a JSON string or a hash alone. An absent window is omitted, never null.
Windows are inclusive and compared lexicographically; an inverted window, an
unknown member and any other scope refuse.

**Identity.** The bytes must be canonical `identity.created` v1 with body
exactly `{publicKey, keyId, kind: "service", at}`, `keyId` derived from
`publicKey`. The identity is the hash of those exact original bytes, and its
`at` is preserved. An enrollment binds when its `issuer_id` is that hash and
its `public_key` is that key. The enrollment's authorization time is
`not_before`, or the identity's `at` when `not_before` is omitted.

**Rotation.** The complete `identity.key_rotated` v1 envelope with body exactly
`{identity, from, to, toKeyId, effective}`: `toKeyId` derived from `to`, and
`to` distinct from `from`.

**Revocation.** The complete `identity.key_revoked` v1 envelope with body
exactly `{identity, key, since, reason}`, `reason` nonempty.

Every payload decodes only from canonical bytes that re-encode to themselves:
trailing bytes, noncanonical integers or key order, duplicate keys, unknown
members at any level, a fact `extra` member and a version other than 1 refuse
before any trust evaluation. Both fact constructions are byte for byte the
platform's.

### 3.3 Detached records

The detached signature is public JSON with exactly
`{schema_version: 1, kind, root_key_id, payload_digest, signature}`, where
`kind` is `enrolment`, `rotation` or `revocation` and `payload_digest` is
BLAKE3 of the exact canonical payload bytes. Verification checks, in order: the
kind is the one expected, `root_key_id` is the independently pinned root's key
id, the digest matches the payload, and the signature verifies strictly under
the kind's domain. The selectors grant no authority.

The owner public record is exactly
`{schema_version: 1, public_key, key_id, scope: "platform"}`, with `key_id`
derived from `public_key`. Both records refuse duplicate members at any depth,
non-integer or non-portable numbers, unknown members and any other schema.

The detached check verifies the payload strictly, the identity strictly, their
binding, and the signature. It answers neither eligibility nor rollback.

### 3.4 RootSetV2

A second root-set wire shape, never permissive extra members on the current
`RootSet`. Its members are exactly
`{schema_version: 2, root_set_version, origin: "pinned", owner_roots,
enrolments, rotations, revocations}`.

An owner root is `{key_id, public_key, scope: "platform", status, not_before?,
not_after?, since?}`, with status `active`, `revoked` or `excluded`. It is an
enrollment authority, never a direct signer of entries or attestations. Each
authorization item is `{root_key_id, payload, signature}`, where `payload` is
the section 3.2 enrollment map or the complete fact envelope in its JSON value
form; its canonical bytes are reconstructed and the signature verified over
them. There is no trusted duplicate online-key row and no `signed_by`.

The set is operator configuration, not a root-signed document. Its digest is
BLAKE3 of the canonical portable-value encoding of the complete V2 object,
every root authorization included, so any change to a signature, selector,
payload, root or version names a different set.

Verification is of the complete history, and any defect refuses the whole set:

- owner root key ids are unique and derived; a revoked root carries `since`
  and only a revoked root does;
- every authorization names a pinned root that may authorize at the event's
  time (enrollment `not_before`, rotation `effective`, revocation `since`): an
  excluded root never, a revoked root only before its `since`, and only inside
  its inclusive window; and its signature verifies strictly;
- one enrollment per identity, and no key enrolled twice;
- rotations apply in `effective` order per identity; each names the current
  key as predecessor, takes effect strictly after it did, and introduces a key
  never seen in any identity's history, so duplicates, forks, cycles and
  cross-identity transitions refuse;
- each revocation names a key of that identity's approved history, at most one
  per key.

### 3.5 Eligibility

At an explicit evidence time and scope, a key in no verified history is
`unknown`, which includes a key known only from the DAG. Otherwise, in order:
a scope other than the enrollment's refuses; revocation refuses at and after
`since`; the enrollment window refuses outside its inclusive bounds; the
enrolled key is eligible from its enrollment time and strictly before the
first rotation's `effective`; a rotated-in key is eligible at and after its
`effective` and strictly before the next. Signatures before a revocation or
rotation keep their prior eligibility. No clock is read and no current time
is substituted.

### 3.6 Operator pin

The operator pins an exact `root_set_version` and digest. A verified set is
admitted only when it equals that pin. Against the pin in force, a lower
version refuses as rollback, the same version with a different digest refuses
as replacement, the same version and digest is unchanged, and a higher version
advances, naming the old and new pins that `trust.root-set-changed` records.
No number of valid signatures authenticates a rollback or an omitted
revocation; only this pin refuses them.

### 3.7 The boundary reader

Current readers ignore unknown JSON members, so a V2 document is never handed
to one. The boundary reader refuses duplicate members, dispatches on
`schema_version` (2 to the V2 reader, any other value refused), and reads a
document without one as legacy V1 with its exact members only. A legacy set
remains unsigned and verification-only: it yields no V2 set, and its
`signed_by` proves nothing.

## 4. Out of scope

- The offline verbs, seed files, entropy and confirmation: spec 036.
- Adoption: the platform's loader upgrade, its pin move, the
  `trust.root-set-changed` entry and its runbook are its spec 004's.
- Root succession or root rotation: a compromised root needs a new pinned
  configuration approved by the operator.
- A finite-expiry enrollment renderer.

## 5. Resolved decisions

**2026-10-03: owner approved the direction via the 007 proposal.** The three
domains, exact-byte enrollment signing, detached signatures in RootSetV2 and
the offline verb surface were approved as proposed. That approval authorizes
this amendment and its implementation with golden vectors; it does not
authorize generating a root or signing an enrollment, and it does not ratify
this text.

**2026-10-03: conservative choices where the proposal is silent.**

- HLC `physical` is bounded by the canonical decoder's portable range
  (`2^53-1`), not `i64::MAX`, because the existing decoder refuses larger
  integers and a value it cannot read back cannot be signed.
- A RootSetV2 enrollment must carry `not_before`: its items carry no identity
  fact, so the `at` fallback is available only to the detached check.
- One enrollment per identity and one per key across the set, which makes the
  no-resurrection rule structural; a second issuer is a second identity.
- An exact duplicate rotation or revocation refuses, as a conflict.
- A rotation must take effect strictly after its predecessor did, so no two
  keys of one identity are eligible at the same instant.
- Revocation `reason` is any nonempty text; no vocabulary is fixed.

The vectors under `testdata/vectors/issuer/` are frozen as committed bytes for
this contract, derived from public test-only seeds; 005 section 3.16's
provisional status for the other five vectors is unchanged.

## Verification

The suite derives every vector from public labelled test seeds and compares it
with the committed bytes, then exercises each required case of the proposal
that belongs to the shared contract. These commands declare acceptance; they
are not evidence of adoption.

```verify:cli
cargo test -p statecraft-envelope --test issuer_contract
cargo test -p statecraft-envelope --test cli_compat
cargo test -p statecraft-envelope --test vectors
```
