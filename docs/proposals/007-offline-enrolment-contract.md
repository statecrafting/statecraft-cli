# Offline issuer enrollment contract: draft for owner approval

Grade: proposed, not specified, implemented, tested or released. Requested under
CLI spec 007 and platform spec 004 on 2026-10-03. Baselines: CLI
`0748fc721a5e51baff7c1d137be6e8756f7b9088`; platform
`b4f29c5304fbd0a4df307b2c078d558ef526860c`. Historical design: archived platform
spec 016 G-08 at `f3d001c19c3f232a5560626e1183f1d73bd0bcb3`.

## Placement and approval boundary

The current CLI spec 005 owns `crates/statecraft-envelope/`; spec 007 owns the
command JSON envelope and does not own cryptographic envelopes. This draft is
recorded through 007 as requested, with no ownership transfer. Before building,
propose and ratify a separate behavioral amendment of 005 for shared types,
domains and verifier, and of 006 for offline command behavior, under 001's
amendment rule. Spec 007's JSON rendering applies to those future commands.
Platform 004 owns custody, issuer identity establishment and root-set adoption.

No crate code, binary, dependency pin, seed, signature, root configuration or
approval status changes in this proposal. The platform consumes the current
pinned crate until a separately reviewed shared-contract delivery and pin move.
The companion platform draft contains the runbook, catalog and placeholder
root-set file. Those placeholders are not deployed configuration.

## Key formats and signature domains

Seed: exactly 32 cryptographically random bytes, persisted offline as 64
lowercase hexadecimal characters followed by LF. The public key is the raw
32-byte Ed25519 verifying key rendered as 64 lowercase hex characters. The key
id is BLAKE3-256 of the raw public-key bytes, rendered as 64 lowercase hex
characters. A signature is exactly 64 bytes, represented as 128 lowercase hex
characters in the detached public JSON result and root set. Reject wrong
lengths, non-hex, uppercase, invalid/weak public keys and noncanonical signatures.
Use strict Ed25519 verification for the new records.

Keep the existing preimage construction byte-for-byte:

```text
ASCII("statecraft-envelope/v1\n") || ASCII(domain) || 0x0a || canonical_payload_bytes
```

Add these proposed constants to the closed domain list only after ratification:

| Constant | Domain | Payload |
|---|---|---|
| DOMAIN_ISSUER_ENROLMENT | `trust.issuer-enrolment` | Canonical enrollment map below |
| DOMAIN_ISSUER_ROTATION | `trust.issuer-rotation` | Exact canonical `identity.key_rotated` fact envelope |
| DOMAIN_ISSUER_REVOCATION | `trust.issuer-revocation` | Exact canonical `identity.key_revoked` fact envelope |

An entry or attestation signature is not any of these. Cross-kind verification
must fail even for identical bytes. Algorithm/domain are fixed by record kind,
not chosen by an input string or payload. The seed is never serialized in the
shared types, printed, logged or accepted as an argument/environment value.

The generator's public-only record is exactly `{schema_version: 1, public_key,
key_id, scope: "platform"}`. The verifier consumes this minimal record after
the independent full-id comparison; publication may wrap it in a ceremony
record naming date and reviewed revisions without changing the pin.

## Enrollment payload and detached signature

Preserve the platform's existing canonical enrollment construction. Its exact
keys are snake_case, despite the historical spec's descriptive camelCase:

```text
{
  issuer_id: lowercase-hex BLAKE3(identity.created.cbor),
  public_key: lowercase-hex online Ed25519 public key,
  scope: "platform",
  not_before?: {physical: integer, logical: integer},
  not_after?: {physical: integer, logical: integer}
}
```

Canonical bytes are the existing portable-value DAG-CBOR encoding of this map,
not JSON text, CBOR of a JSON string, a hash alone, or a rebuilt identity fact.
Optional absent windows are omitted, never null. HLC physical is in
0..=i64::MAX, logical in 0..=u32::MAX; comparisons are lexicographic and window
endpoints inclusive. Reject inverted windows and empty/unsupported scopes.
The online key id is derived from its raw public bytes. The identity bytes must
be canonical `identity.created` v1, kind Service, with the same public key and
key id; identity is its envelope hash. Preserve the original timestamp.

The current renderer emits `not_before` at the supplied timestamp and omits
`not_after`. That is an unbounded enrollment, explicitly shown for owner
approval. A finite expiry requires a future reviewed renderer option; this
proposal does not silently change already rendered bytes.

Detached signature output, public-only JSON with no extra members:

```text
{schema_version: 1, kind: "enrolment", root_key_id: <owner key id>,
 payload_digest: <BLAKE3 of exact canonical payload bytes>, signature: <128 hex>}
```

For rotation/revocation, `kind` is `rotation`/`revocation`, selecting the fixed
domain and fact schema. `payload_digest` and `root_key_id` are checked selectors;
they do not grant authority. The signature covers the full payload through the
family preimage. The root must already be independently pinned.

## Signature placement and versioned root set

Introduce an explicit RootSetV2 format, not permissive unknown fields attached
to the current RootSet. Top-level members are exactly:

```text
{schema_version: 2, root_set_version: integer, origin: "pinned",
 owner_roots: [...], enrolments: [...], rotations: [...], revocations: [...]}
```

`schema_version` identifies the wire shape; `root_set_version` is a monotonic
operator configuration revision. Origin is independently established by the
configuration loader, never upgraded from evidence by an input assertion.
An owner root entry is `{key_id, public_key, scope, status, not_before?,
not_after?, since?}`. Its scope is `platform`; status is active/revoked/excluded.
It is an enrollment authority, not a direct online entry/attestation signer.
The configured owner keys must match the independently authenticated public
fingerprint record. Do not enroll RSA login keys, Rauthy secrets, age keys or
CLI local user roots. Reject duplicate/mismatched owner key ids.

Each `enrolments` item is `{root_key_id, payload, signature}`. The payload is the
map above; reconstruct its canonical bytes and verify the owner's signature
before admitting the online key. There is no independently trusted duplicate
online-key row. `rotations` and `revocations` items have the same three members,
with `payload` the complete canonical fact envelope of the appropriate kind.
Its root signature is stored here as a detached authorization and referenced
alongside the unchanged fact in the DAG. A normal online-signed DAG entry does
not itself constitute root authorization.

No `signed_by` text is proof of a signature. V2 removes that legacy metadata
rather than interpreting it as authorization. The root set as a whole is pinned
operator configuration, not claimed to be root-signed. Its digest is BLAKE3 of
the canonical portable-value encoding of the complete V2 object, including all
root authorizations. The operator pins the exact version and digest, records
old/new digests in `trust.root-set-changed`, and refuses rollback or same-version
replacement. No number of valid individual signatures authenticates a config
rollback or an omitted revocation; those are prevented by this operator pin.

Current readers can ignore unknown JSON members. Therefore V2 is never handed
to a current loader: upgrade the boundary to reject unknown/duplicate fields
and unsupported schemas first. A legacy v1 set may retain explicit unsigned
verification-only behavior, but cannot establish a hosted trusted issuer.
Approve that migration and the golden vectors before adopting the new pin.

## Rotation, revocation and verification

Rotation signs the existing canonical fact body `{identity, from, to, toKeyId,
effective}` inside the full `identity.key_rotated` v1 envelope. Require an
existing signed enrollment of that identity and predecessor key, a new distinct
Ed25519 key, correct derived key id and unambiguous effective HLC. Reject forks,
duplicate/conflicting events, cycles, cross-identity transitions and unsupported
fields. The predecessor remains eligible strictly before effective; the new
key is eligible at and after effective, intersected with applicable windows.
The identity id remains the original `identity.created` envelope hash.

Revocation signs the existing `{identity, key, since, reason}` body inside the
full `identity.key_revoked` v1 envelope. Require that the named key belongs to
that identity's approved enrollment/rotation history. Revocation takes priority
at and after `since`; signatures before it retain their prior eligibility.
Reject conflicting records and never allow a later enrollment to resurrect a
revoked key. Owner exclusion always refuses; an invalid root authorization
refuses the configuration rather than granting partial trust.

The pure shared verifier accepts the pinned owner root set, exact payload,
record kind and detached signature. It verifies cryptography and the complete
history before producing issuer eligibility at an explicit evidence HLC and
scope. It performs no I/O or current-time substitution. Root authorization is
checked at enrollment `not_before`, rotation `effective`, revocation `since`;
if enrollment omits `not_before`, its identity fact's `at` is required. A revoked
owner root cannot authorize an event at/after its own `since`; exclusion always
refuses. The operator must reapprove a new pinned configuration for a root
compromise; no implicit root succession or root rotation is introduced here.

## Proposed offline verbs

Use the existing CLI binary after a separate command-surface amendment; do not
add a new binary or runner. The pure envelope crate owns format/verifier, not
filesystem operations or entropy generation. Planned commands:

```text
statecraft-cli trust root-generate --out <new-offline-seed-file> --public-out <new-public-json>
statecraft-cli trust issuer-sign --kind <enrolment|rotation|revocation> --seed-file <offline-seed-file> --root-key-id <pinned-id> --request <canonical.cbor> --identity <identity.created.cbor> --out <new-signature.json>
statecraft-cli trust issuer-verify --kind <kind> --root-public <pinned-public.json> --request <canonical.cbor> --identity <identity.created.cbor> --signature <signature.json>
```

All paths are explicit. There is no home-directory default or automatic key
search. Root generation uses the initialized OS CSPRNG, exclusive creation and
owner-only permissions, without stdout secret output. Seed loading rejects
symlinks, nonregular files, wrong permissions and noncanonical seed files.
Rotation/revocation signing additionally requires `--history-root-set
<pinned-v2.json>` and validates the predecessor and identity history before
signing. Its full-eligibility verifier additionally takes that pinned history
and explicit `--at-physical`/`--at-logical` and scope; the simple verify command
above checks only the detached authorization and identity binding, not issuer
eligibility or rollback. Reject evidence-sourced history as a pin.
Signing requires explicit owner inspection/confirmation of decoded public
fields, request digest and root id. Noninteractive execution without a separate
explicit confirmation option is refused; the approval record contains only
public fields. Refuse existing outputs and never overwrite custody files.
Verification requires an independent public pin, not a public key taken from
the signature output. All JSON answers follow spec 007. No verb contacts a
network, opens a cell/store, signs an arbitrary domain or enrolls itself.

## Required tests before implementation is qualified

These are planned acceptance cases, not executed tests or passing evidence.
Use conspicuously public deterministic test-only seeds and frozen byte vectors;
never production ceremony material.

| Case | Expected result |
|---|---|
| Seed/public/key-id/signature golden vector | Exact bytes match across shared verifier, offline CLI and platform; hash raw public bytes. |
| Canonical enrollment round trip and preserved identity timestamp | Signature verifies; identity id matches exact original fact bytes. |
| Modified issuer/public key/scope/window/identity, wrong root or wrong domain | Refuse; no trusted issuer or store mutation. |
| Noncanonical CBOR, trailing bytes, duplicate/unknown fields, malformed/weak keys, invalid signature | Refuse before trust evaluation. |
| DAG-only/self-anchored key, absent independently pinned root | `unknown`; trusted-issuer admission refuses. |
| Only `signed_by` metadata or legacy v1 direct key row | No V2 trusted enrollment. |
| Enrollment at not_before/not_after and immediately outside | Inclusive boundaries pass; outside refuses. |
| Rotation before/at/after effective | Old historical signature passes before; old at/after refuses; new at/after passes. |
| Wrong predecessor, identity/key-id mismatch, fork/cycle/conflicting events | Refuse complete configuration. |
| Revocation before/at/after since | Earlier eligible signatures remain valid; at/after fail, even after a later enrollment. |
| Root window/revocation/exclusion at authorization time | Outside or positively refused root cannot authorize. |
| Root-set version rollback/same-version changed digest/omitted revocation | Operator pin refuses; no claim of protection from event signatures alone. |
| V2 digest changes when signature or authorization changes | New verified_under digest; canonical vectors freeze after reviewed adoption. |
| Offline command refuses existing files, symlinks, unsafe modes, bad seed, unconfirmed signing | Exit refusal/failure; no secret/log/stdout leak or overwritten files. |
| Platform first entry and principal identity | Exact original identity fact is first platform entry; Service principal uses that identity id. |

Implement actual tests only after approval; freeze precise vector bytes and
command exit classifications in the implementation amendment. No empty verify
block or command against tests that do not exist claims readiness here.

## Owner decision table

| Item | Options | Recommended default | Consequence |
|---|---|---|---|
| Actual shared owner | Amend 005/006; transfer ownership to 007 | Amend 005/006 | Preserves current crate ownership; 007 governs command JSON only. |
| Domains and V2 schema | Approve draft; revise format | Review and approve this explicit format | Enables separate implementation, strict migration and cross-product vectors. |
| Enrollment expiry | Accept current unbounded window; require finite expiry | Explicitly decide before signing | Finite expiry requires renderer work; no signed bytes silently change. |
| Event-time semantics | Proposed effective/since rules; another explicit rule | Proposed rules after vector review | Preserves historical valid signatures and refuses compromised future ones. |
