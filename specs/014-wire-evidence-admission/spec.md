---
id: "014-wire-evidence-admission"
title: "Admission of wire-exchange evidence"
status: draft
implementation: pending
created: "2026-09-26"
summary: >
  Amends the run record, evidence envelope, and command projection so an exact
  statecraft/wire-exchange/v1 artifact can be bound to one run attempt and
  sidecar effect, judged deterministically under explicit policy, and shown
  without retaining secret values or confusing producer testimony with
  independent acceptance.
amends:
  - "003-work-and-run-semantics"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
extends:
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-envelope/" }, nature: additive }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "003-work-and-run-semantics"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
interface_references:
  - corpus: "wire-witness"
    spec: "003-redaction-custody-and-retention"
    digest: "sha256:86ba7b2ee74b02726fbc8b48e7f44f8f7ceb7492365196c7622c854afff54642"
    sections:
      - anchor: "3-1-retention-modes"
        digest: "sha256:e786af7a30c269f4eee8dfc296ba8306ad9fac24bc5a44ce778a0287ff46f68b"
      - anchor: "3-2-redaction-before-custody"
        digest: "sha256:3a4913bff457d1cb1333e249ca7badbc24c855900e81d86225cab5f3edcda892"
      - anchor: "3-3-durable-custody"
        digest: "sha256:c37e5251fb4ba7043e25e37aa79665c7b726eaa08245282d339358448f0407f2"
    obtained: "2026-09-30"
    rationale: "Bind metadata-only classification, mandatory redaction, and capture-manifest custody to the producer sections that define them."
  - corpus: "wire-witness"
    spec: "005-binding-and-sidecar-protocol"
    digest: "sha256:a8195055f0efc508abf86c7fbf86f577cce5556d12d8f3dfe67fd486a988f65d"
    sections:
      - anchor: "3-1-attemptbinding"
        digest: "sha256:68aafeb66bf494eda066bff13eb506fce5f9a8b3777505a74d98ec09fa8e11ca"
      - anchor: "3-2-versioned-stdio"
        digest: "sha256:d64434f79125e34a81f99f23670275a9d7946a02df05d34b8deee56e10fca8ec"
      - anchor: "3-3-bracketed-lifetime"
        digest: "sha256:b40040b08f584f275ea4f52b4281378c23fd365c783e888fb957acf81cd9c5f1"
      - anchor: "3-4-evidence-envelope-boundary"
        digest: "sha256:f9fc1f6171b7fd157397a32b9ff1e711da66c7e6edce56401738e85c9794ab20"
    obtained: "2026-09-26"
    rationale: "Admit the producer's exact attempt binding, closure, and evidence boundary rather than reconstruct its protocol in Statecraft."
obligations:
  - id: "I-1"
    kind: invariant
    text: "A wire-exchange artifact is judged only for the exact run id, attempt, and sidecar effect id it repeats unchanged."
    anchor: "3-2-exact-subject-and-producer-binding"
  - id: "I-2"
    kind: invariant
    text: "Transport testimony, schema validity, policy judgment, independent acceptance, provider qualification, and correctness remain separate facts."
    anchor: "3-8-no-testimony-becomes-authority"
  - id: "R-1"
    kind: requirement
    text: "Admission produces one deterministic admit or refuse outcome with one stable reason code and the evidence dimensions that led to it."
    anchor: "3-6-deterministic-admission"
  - id: "R-2"
    kind: requirement
    text: "Metadata-only retention is the default, and no retained field contains a secret value or raw credential-bearing traffic."
    anchor: "3-4-custody-retention-and-redaction"
  - id: "V-1"
    kind: verification
    text: "Portable fixtures prove admission, refusal, historical compatibility, and the additive run show projection without live provider activity."
    anchor: "verification"
    inputs:
      - "crates/statecraft-envelope/tests/wire_exchange_admission.rs"
      - "crates/statecraft-run/tests/wire_evidence_projection.rs"
      - "crates/statecraft-acceptance/tests/wire_evidence_policy.rs"
      - "crates/statecraft-cli/tests/wire_evidence_show.rs"
---

# 014: Admission of wire-exchange evidence

## 1. Purpose

Spec 005 can verify evidence bytes and decide admission, but it does not define
the facts carried by `statecraft/wire-exchange/v1`. Spec 003 correlates effects
by run id and effect id while preserving attempts. Spec 006 exposes their
read-only fold. None yet defines how a wire capture enters that boundary.

This amendment defines that narrow admission contract. It does not authorize a
sidecar, provider access, raw traffic retention, or implementation. The signed
spec 013 draft at commit
`92097682a6578f0d0309dbf831266bd76d705337` supplied planning context only. It
is absent from this branch, is not a dependency, and grants no authority here.

## 2. Territory and exact inputs

This spec owns no product code and establishes no compile unit. A later
implementation may change only the existing units named by its `extends`
edges. The three `amends` edges change approved requirements additively rather
than editing those specs in place.

The binding and sidecar protocol input is wire-witness commit
`f701a7b51e492ec98bf6b160bfd3e3f8bdf7990d`, tree
`db9513c5faf81427c18468119590b2c2a712b4e6`, and spec 005 content digest
`sha256:a8195055f0efc508abf86c7fbf86f577cce5556d12d8f3dfe67fd486a988f65d`.
The four section pins remain current against the later approved producer text.
The retention and redaction input is wire-witness commit
`0e3297859e1da5c3fb6249521a189af447878c1e`, tree
`67d41d5fc81a16da88285f8db8484212b25c5034`, and spec 003 content digest
`sha256:86ba7b2ee74b02726fbc8b48e7f44f8f7ceb7492365196c7622c854afff54642`.
The first cited commit was a draft. Neither commit nor later ratification is a
released producer identity or permission to integrate it.
The frontmatter `created` date records the draft's creation, not its most recent
amendment. An interface `obtained` date records when that reference was added or
refreshed and may therefore postdate `created` while the spec remains draft.

## 3. Behavior

### 3.1 Artifact identity and bounded input

The only artifact this amendment recognizes is
`statecraft/wire-exchange/v1`. Its evidence reference uses spec 005's reference
contract and names all of:

1. the artifact type and schema version;
2. the exact manifest bytes' SHA-256 digest and byte length;
3. the named digest construction;
4. the producer name, producer revision, and producer content digest;
5. the subject tuple in section 3.2; and
6. the capture completeness, custody, retention, and redaction facts below.

An absent member is explicit `unknown`, `unavailable`, or `not-recorded` as
applicable. It is never replaced with an empty value, zero, a nearby version,
or a moving reference. Unknown fields are retained when the envelope relays
them but do not gain meaning in this version.

Before semantic interpretation, the reader refuses duplicate object member
names as `malformed` and enforces these limits while tokenizing the input:

- at most 1,048,576 manifest bytes;
- at most 32 nested object or array levels;
- at most 4,096 object members plus array elements in total across the entire
  manifest;
- at most 1,024 elements in one array;
- at most 128 UTF-8 bytes in one member name; and
- at most 65,536 UTF-8 bytes in one string value.

Exceeding any limit is `malformed`. The decoder does not first construct an
unbounded value, and it does not follow paths, links, URLs, or embedded commands
found in hostile input.

### 3.2 Exact subject and producer binding

Each artifact repeats one `runId`, positive `attempt`, and `sidecarEffectId`
exactly as Statecraft supplied them. Statecraft locates the durable effect by
the spec 003 correlation key `(runId, sidecarEffectId)`, then independently
requires the repeated attempt to equal the attempt holding that effect. The
attempt is a binding fact, not a replacement correlation key.

The referenced intent must be the one sidecar intent for that attempt, and its
outcome must close that same effect exactly once. An absent, invalid, orphaned,
ambiguous, already closed, or mismatched effect fails subject binding as
`subject-mismatch`. Evidence bound to an earlier attempt instead refuses as
`stale` for the current attempt and cannot be adopted by retry. A later attempt
never inherits an earlier attempt's admission.

The producer revision and digest are compared with the producer identity that
the attempt recorded. Missing or unequal producer identity fails subject
binding. A compatible name or schema version alone is insufficient.

### 3.3 Required wire facts

The decoded manifest carries these independent facts:

| Fact | Required representation |
|---|---|
| capture | exact manifest digest, byte length, start and end disposition |
| completeness | `complete`, `incomplete`, or `unknown`, plus a sorted set of named gaps |
| requested identity | exact destination and protocol identity requested by the child, or an explicit absence state |
| served identity | exact destination and protocol identity reported as served, or an explicit absence state |
| usage | provider-reported values labeled `provider-testimony`, with units and provider field names |
| estimated cost | value, currency, method identity, and input identities, labeled `estimate` |
| attestation | attester identity and construction, or the explicit state `unknown` |
| retention | one result from section 3.4 with its reason |
| redaction | one result from section 3.4 with its reason and affected field classes |

Requested and served identities are never collapsed into one field. Equality
is an observed comparison only when both exact values are present. Provider
usage is testimony even when schema-valid. Estimated cost is never provider
billing, observed spend, or a basis for qualification. An absent attestation
identity is `unknown`, never anonymous success.

`complete` requires an empty gap set. `incomplete` requires at least one named
gap. `unknown` names why completeness could not be decided. Contradictory
completeness and gap values are malformed rather than normalized.

### 3.4 Custody, retention, and redaction

The reference binds the exact manifest bytes that admission judged. Custody
records where those bytes were obtained, which process wrote them, which
Statecraft process read them, and whether the bounded handoff completed. A
claim of custody without these identities is unknown custody.

Retention and redaction are separate facts:

- retention is `metadata-only`, `encrypted-content`, `discarded`, `failed`,
  or `unknown`;
- redaction is `not-needed`, `applied`, `failed`, or `unknown`.

`metadata-only` is the default and retains the manifest, digests, sizes,
identities, classifications, gaps, results, and reasons only. Raw request or
response bodies, headers carrying credentials, cookies, authorization values,
tokens, secret values, and raw credential-bearing traffic are never retained
under this default. The manifest itself must not contain them.

Before any Statecraft durable write, admission walks every decoded member and
string value. Under `metadata-only`, it refuses `prohibited-content` when the
member is outside the exact supported v1 manifest schema, when that schema
classifies the member as a raw request or response body, raw header map or
value, cookie, authorization value, token, secret, or credential, or when a
string value matches the exact secret-detector identity recorded in the
attempt policy. Classification and redaction authority comes exactly from the
pinned wire-witness spec 003 sections 3.1 through 3.3. The recognized artifact
reference and manifest boundary come from pinned wire-witness spec 005 section
3.4. Both are declared in `interface_references`; Statecraft loads no unpinned
producer schema. Classification comes from the versioned spec 003 sections, not
a field-name heuristic. A missing detector identity, detector failure, or value
that cannot be scanned within the section 3.1 bounds refuses admission rather
than being retained. The admission record keeps the manifest digest, policy
identity, subject identity, dimensions, outcome, and reason required by section
3.6. Its content-finding detail contains only the member path, detector identity
when applicable, and category. It never copies the candidate value. An
identical resubmission therefore refuses as `duplicate` at step 6 without
scanning the candidate again. Digest and length verification may occur in
quarantined memory before this screen; the manifest bytes are not then added to
Statecraft custody when the screen refuses them.

`encrypted-content` is unsupported until a separately ratified policy names
its purpose, consent, key custody, retention period, deletion behavior, and
authorized readers. Selecting it before then refuses admission. A redaction
result never proves completeness. If redaction removes a field required by the
schema or current policy, the artifact is incomplete with that gap named.

### 3.5 Policy requirement behavior

Witness evidence is optional by default. A separately authorized task policy
or acceptance policy may require `statecraft/wire-exchange/v1` for a named
attempt. The requirement binds the exact artifact type, minimum completeness,
required evidence dimensions, and any permitted absence states. It cannot
select a provider, trust root, credential, retention expansion, or live action.

When no policy requires the artifact, absence is `not-recorded` and changes no
other acceptance fact. When policy requires it, absent, unavailable, unknown,
stale, mismatched, or refused evidence makes acceptance ineligible with the
specific reason. It does not rewrite the attempt outcome or claim that the
underlying work was incorrect.

Historical policies with no wire-evidence member decode as an empty
requirement. Historical runs are not rewritten and their absent projection is
`not-recorded`.

### 3.6 Deterministic admission

Admission performs this ordered procedure over immutable input:

1. enforce bounded input and strict portable decoding;
2. require one supported artifact type, schema version, and construction;
3. resolve the referenced manifest bytes and verify length and digest;
4. verify exact producer and subject binding under section 3.2;
5. compute integrity, signature, issuer-trust, and subject-binding dimensions;
6. read the durable record for an existing artifact with the same manifest,
   policy, and subject identities and refuse it as `duplicate`;
7. apply the section 3.4 prohibited-content screen;
8. enforce the section 3.4 retention gate and refuse `encrypted-content` as
   `retention-forbidden` unless a separately ratified policy permits it;
9. validate required facts and cross-field consistency;
10. apply the exact recorded acceptance policy; and
11. append one admission record without changing prior records.

Every completed procedure yields exactly `admit` or `refuse`, one stable reason
code, the four evidence dimensions, and the policy identity. An operational
failure that prevents judgment records no admission and reports `unavailable`
or `unknown` at the read surface. It is never converted to an admission.

Stable refusal reasons include `unsupported-artifact`, `malformed`,
`digest-mismatch`, `producer-mismatch`, `subject-mismatch`, `duplicate`,
`stale`, `incomplete`, `redaction-failed`, `retention-forbidden`,
`prohibited-content`, `policy-mismatch`, `policy-unsatisfied`, and the existing
spec 005 reasons.
New reason strings may be preserved by older readers, but older readers do not
reinterpret them.

An identical duplicate is still `duplicate`: append-only history preserves
both observations and admission refuses ambiguity. Retry reads the durable
record first. It may resume only an operationally interrupted judgment over
the same bytes, policy, and subject; otherwise it begins no replacement and
reports `policy-mismatch`, `producer-mismatch`, or `subject-mismatch` for the
changed identity.

### 3.7 Additive `run show` projection

`run show <run>` folds wire evidence from that run's durable record only. It
performs no network, provider, sidecar, filesystem-discovery, or admission
work. Human and JSON renderings come from one value and add, per attempt:

- artifact, schema, construction, producer, manifest digest, and byte length;
- run id, attempt, sidecar effect id, and binding result;
- completeness and every named gap;
- custody, retention, and redaction results separately;
- requested and served identities separately;
- provider-reported usage labeled as testimony;
- estimated cost labeled as an estimate with method identity;
- attestation identity or `unknown`;
- four evidence dimensions; and
- admission outcome, reason, and policy identity, or the exact absence state.

The projection never renders secret values or raw captured traffic. Unknown,
unsupported, unavailable, unverified, stale, refused, and not-recorded remain
distinct. A malformed record produces a finding with its record position and
does not suppress other readable records.

### 3.8 No testimony becomes authority

The following grades are reported independently and no one implies another:

1. transport testimony says what the witness reported observing;
2. schema validity says the artifact is well formed;
3. policy judgment says whether Statecraft admitted that evidence;
4. independent acceptance judges the completed run under spec 005;
5. provider qualification binds an exact adapter and provider version to its
   own admitted evidence; and
6. correctness remains a claim supported by tests or other acceptance, not by
   traffic capture alone.

An admitted artifact can contain provider testimony that is false. A valid
schema can be refused by policy. A passing signature does not establish issuer
trust. Provider qualification cannot be minted from this artifact alone.

### 3.9 Compatibility and migration

All wire fields and the `run show` member are additive. Existing envelope and
run-record bytes remain readable. Missing new members take explicit absence
states, never inferred values. No migration rewrites a prior record. Writers
continue using the established canonical constructions and append-only chain.

Readers encountering an unknown artifact version preserve its reference and
report `unsupported-artifact`; they do not partially interpret it. A future
compatible reader may judge the preserved bytes under its own exact policy
without changing the earlier result.

## 4. Observable acceptance and negative cases

Implementation acceptance uses offline portable fixtures only. Each case
asserts the admission record and both renderings:

| Case | Expected observation |
|---|---|
| exact complete metadata-only artifact | admitted only when the recorded policy permits every computed dimension |
| optional artifact absent | `not-recorded`; no other acceptance fact changes |
| required artifact absent | acceptance ineligible with `policy-unsatisfied` |
| wrong run, attempt, or effect | `subject-mismatch`; no admission |
| wrong producer revision or digest | `producer-mismatch`; no admission |
| changed bytes or length | integrity fails with `digest-mismatch` |
| duplicate artifact | both records remain visible and admission refuses `duplicate` |
| earlier-attempt artifact | `stale`; retry does not adopt it |
| retry with a changed policy identity | `policy-mismatch`; no replacement begins |
| malformed, oversized, deeply nested, or duplicate-key input | bounded refusal `malformed` with no path or command followed |
| manifest contains a raw body, credential field, registered secret, or detector match | `prohibited-content`; candidate bytes are not retained by Statecraft |
| secret-detector identity is absent or its scan cannot complete | `prohibited-content`; admission fails closed without retaining candidate bytes |
| unknown schema or construction | `unsupported-artifact`; bytes are not guessed at |
| incomplete capture with named gaps | visible gaps; admission follows the exact minimum-completeness policy |
| redaction removed a required fact | `incomplete`, with redaction and the gap separately visible |
| `encrypted-content` selected without a separately ratified retention policy | `retention-forbidden`; no candidate bytes enter Statecraft custody |
| redaction or retention failure | the exact result remains visible; policy refuses when required |
| requested and served identities differ | both remain visible; no equality is inferred |
| provider usage with no independent corroboration | rendered only as provider testimony |
| estimated cost | rendered only as an estimate with method and input identities |
| absent attester | attestation identity is `unknown` |
| interruption before judgment | no admission record; retry is permitted only for the same immutable inputs |
| historical run and policy | readable with the additive member `not-recorded` |

The suite also scans every retained and rendered fixture to prove that no
secret value, credential header, token, cookie, or raw traffic body survives.

## 5. Out of scope

- implementing or authorizing the sidecar described by draft spec 013;
- provider invocation, spending, live capture, or provider qualification;
- choosing a provider, trust root, signing custodian, or retention expansion;
- storing raw credential-bearing traffic or secret values;
- changing the durable effect correlation key established by spec 003;
- reimplementing the witness protocol or interpreting hostile traffic in the
  supervisor; and
- claiming release, activation, observation, admission, or qualification from
  this draft.

## Verification

Draft review requires `make refresh`, `make gate`, `make code`, relationship
inspection for spec 014, and `interface verify` against the exact wire-witness
checkout. Those checks establish corpus consistency only.

Implementation acceptance, once separately authorized, adds the offline
fixture matrix in section 4 to the owning crates and runs it through
`make verify SPEC=014`. No live provider, secret, network capture, or metered
activity is part of verification. Until that implementation and acceptance
exist, the behavior is specified but not implemented, tested, released,
activated, observed, admitted, or qualified.
