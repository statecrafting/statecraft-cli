---
id: "036-offline-issuer-verbs"
title: "The offline issuer verbs: trust root-generate, trust issuer-sign and trust issuer-verify"
status: approved
implementation: in-progress
created: "2026-10-03"
summary: >
  Amends 006 for the command half of the offline issuer enrollment contract the
  owner approved on 2026-10-03 (docs/proposals/007-offline-enrolment-contract.md).
  Adds three verbs to the existing binary: root-generate writes a new offline
  owner seed only to an explicit path, with owner-only permissions and an
  explicit confirmation option; issuer-sign signs one enrollment, rotation or
  revocation only after strict decoding, history checks and a confirmation
  naming the exact payload digest; issuer-verify checks a detached
  authorization against an independently pinned owner key and, with the full
  option group, issuer eligibility under the operator's exact pin. Formats and
  verification are 035's; no verb reads the product home, contacts a network,
  opens a cell or store, or signs an arbitrary domain.
amends:
  - "006-command-surface"
extends:
  # The verbs, their custody code and their suite live in 006's crate; the
  # edge is declared here because `amends` does not make this spec an owner of
  # 006's code (001 section 5).
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "006-command-surface"
  - "007-family-envelope"
  - "035-offline-issuer-contract"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Every path is an explicit option; no verb reads the product home or a project, searches for a key, contacts a network, opens a cell or store, or signs an arbitrary domain."
    anchor: "3-1-the-verbs-and-their-boundary"
  - id: "R-2"
    kind: requirement
    text: "root-generate runs only with --confirm-new-root, takes 32 bytes from the initialized OS random source, and writes the seed only to --out by exclusive creation with owner-only permissions, never to stdout, stderr or any other file."
    anchor: "3-2-root-generate"
  - id: "R-3"
    kind: requirement
    text: "issuer-sign decodes and binds the request strictly, checks rotation and revocation against a pinned history, reports only public fields, and signs only when --confirm names the exact payload digest and the seed file is safe, canonical and the named root."
    anchor: "3-3-issuer-sign"
  - id: "R-4"
    kind: requirement
    text: "issuer-verify takes the owner key from an independent public pin, never from the signature record; full eligibility requires the pinned history under the operator's exact version and digest and an explicit time and scope."
    anchor: "3-4-issuer-verify"
  - id: "R-5"
    kind: requirement
    text: "Outputs are created exclusively and never overwrite; answers follow 006 section 3.3's exits and 007's envelope."
    anchor: "3-5-exits-and-renderings"
---

# 036: The offline issuer verbs

## 1. Purpose

The offline owner root that vouches for the platform issuer needs three acts
on a machine that never joins a cell: creating the root, signing an
authorization with it, and checking one. The owner approved the contract for
them on 2026-10-03 in `docs/proposals/007-offline-enrolment-contract.md` and
directed that the verbs join this product's existing binary under 006, with
the formats and verifier in the envelope crate under 005 (spec 035).

Adding verbs and a new kind of input (explicit custody files rather than a
target repository or the product home, which 006 section 3.6 names) changes
what 006 requires, so under 001 section 5 this is a new spec with an `amends`
edge. Owner approval of the direction is not ratification of this text. No
root has been generated and nothing real has been signed under it.

## 2. Territory

None of its own. The module `crates/statecraft-cli/src/trust.rs`, the three
verbs in `commands.rs` and `main.rs`, the dependencies `getrandom`, `hex` and
`zeroize` and the suite `tests/offline_trust.rs` are in 006's crate, reached
through the `extends` edge above.

006 section 3.2 makes a command a binding over one library operation. The
envelope crate is deliberately pure and refuses filesystem access and entropy
(035 section 2), so the custody those operations need (exclusive creation,
permissions, the random source, seed loading and confirmation) is implemented
here, in one module, and stated in this spec; no format or verification rule
is restated in it.

## 3. Behavior

### 3.1 The verbs and their boundary

| Command | What it does |
|---|---|
| `trust root-generate --out <new-seed> --public-out <new-public-json> --confirm-new-root` | Creates a new offline owner root. |
| `trust issuer-sign --kind <enrolment\|rotation\|revocation> --seed-file <seed> --root-key-id <id> --request <canonical.cbor> --identity <identity.created.cbor> --out <new-signature.json> [--history-root-set <pinned-v2.json>] [--confirm <payload-digest>]` | Signs one authorization. |
| `trust issuer-verify --kind <kind> --root-public <pinned-public.json> --request <canonical.cbor> --identity <identity.created.cbor> --signature <signature.json> [--history-root-set <pinned-v2.json> --pin-version <n> --pin-digest <hex> --at-physical <n> --at-logical <n> --scope <scope>]` | Verifies one authorization, and optionally eligibility. |

Every path is an explicit option. There is no home-directory default and no
key search. The verbs are dispatched before the product home is read, read no
registered project, contact no network, open no cell or store, enroll nothing
themselves and sign only under the three fixed domains of 035 section 3.1,
selected by `--kind`. Options are `--name value` or `--name=value`; a missing
required, repeated, unknown or positional argument is a usage error.

Every input file must be a regular file, not a symbolic link, of at most
1 MiB. An output must not exist in any form, a dangling link included, and its
parent must be an existing directory; nothing creates directories. Outputs are
created exclusively and flushed to disk. The verbs need owner-only file
permissions and refuse on a platform without them.

### 3.2 root-generate

Without `--confirm-new-root` the verb refuses and writes nothing. With it, it
takes 32 bytes from the operating system's initialized random source and
writes them as 64 lowercase hex characters and LF to `--out`, created with
mode `0600`. It then writes 035's owner public record to `--public-out`. The
answer names the key id, public key and both paths, never the seed. The seed
is held only in zeroizing buffers and appears in no answer, message, log or
other file. `--out` and `--public-out` must differ. If the public record
cannot be written after the seed was, the failure says the seed file exists
and is the only copy; no custody file is ever deleted.

### 3.3 issuer-sign

The request is decoded strictly as `--kind` (035 section 3.2) and the identity
as canonical `identity.created`, and the two are bound. An enrollment needs an
authorization time, from `not_before` or the identity's `at`. A rotation or
revocation requires `--history-root-set`, which must be a pinned V2 set whose
complete history verifies, and the request must extend it: a pinned root that
may authorize at the event's time, and a valid next rotation or revocation for
that identity. An enrollment refuses `--history-root-set`.

The verb then reports the decoded public fields, the payload digest (BLAKE3 of
the exact request bytes) and the root id. Without `--confirm` it refuses after
reporting them; with a `--confirm` that is not that exact digest it refuses.
Only then is the seed file read: it must be a regular, non-linked file with no
group or other permission bits, holding exactly 64 lowercase hex characters
and LF, and its key must be `--root-key-id`. No message names any byte of it.
The detached signature (035 section 3.3) is checked against the seed's public
key before `--out` is created. The answer contains only public fields.

### 3.4 issuer-verify

The owner key comes from `--root-public`, an independent pin, never from the
signature record. Without the full option group the verb performs 035 section
3.3's detached check only: no eligibility and no rollback. With it, all six
options are given together: the history must verify and equal the operator
pin exactly, `--root-public` must be one of its owner roots, and the answer
reports issuer trust (`pass`, `fail` with its reason, or `unknown`) for the key
the request concerns (the enrolled key, the successor or the revoked key) at
`--at-physical`/`--at-logical` in `--scope`. An evidence-ledger origin is
refused by the reader; no current time is substituted.

### 3.5 Exits and renderings

Exits follow 006 section 3.3: 0 done or verified and, with the full group,
eligible; 1 a negative verdict (a signature that does not verify, or issuer
trust other than `pass`); 2 a refusal (an absent confirmation, an unsafe or
noncanonical seed, an existing output, a malformed record, a binding or
history defect, a pin mismatch); 3 usage; 4 a failure nobody asked for (the
random source, a write that did not complete). `--json` answers are 007's
envelope with camelCase report keys; the custody files keep 035's member
names.

## 4. Out of scope

- The formats and verifier: spec 035.
- Interactive prompting; confirmation is the explicit option only.
- A finite-expiry enrollment renderer, root succession and root rotation.
- Any act on a real owner root: generating one and signing an enrollment are
  the owner's ceremony, not this spec's implementation or tests.

## 5. Resolved decisions

**2026-10-03: owner approved the direction via the 007 proposal.** The offline
verb surface was approved as proposed, together with 035's contract. The
approval authorizes this amendment and its implementation with tests on
public test-only seeds; it does not authorize generating a root or signing an
enrollment, and it does not ratify this text.

**2026-10-03: conservative choices where the proposal is silent.**

- `root-generate` also requires an explicit confirmation option,
  `--confirm-new-root`, like signing.
- Confirmation is the explicit `--confirm <payload-digest>` option on every
  run, interactive or not: the first run reports the fields and refuses, and
  the second names what was inspected. No terminal prompt exists.
- The seed file is read only after confirmation, so inspection never touches
  the secret.
- Every input, not only the seed, refuses symbolic links and nonregular files,
  with a 1 MiB bound.
- A negative verification verdict is exit 1, a finding; a record that cannot
  be read as its shape is exit 2.
- `issuer-sign` takes `--identity` for every kind and binds the request's
  identity to it.
- Full eligibility takes the operator pin as `--pin-version` and
  `--pin-digest`, because a history without its pin is not a pin.

## Verification

The suite runs the built binary against spec 035's frozen vectors and public
test-only seeds, in temporary directories with no product home. It
reproduces the frozen signature bytes for each kind and covers every refusal
listed above. These commands declare acceptance; they are not evidence of any
ceremony.

```verify:cli
cargo test -p statecraft-cli --test offline_trust
cargo test -p statecraft-cli --test family_envelope
cargo test -p statecraft-cli --test json_naming
cargo test -p statecraft-envelope --test issuer_contract
```
