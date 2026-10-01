---
id: "013-supervised-wire-witness"
title: "Supervised adoption of one wire-witness sidecar per attempt"
status: draft
implementation: pending
created: "2026-09-26"
summary: >
  Amends 003 and 004 so Statecraft may adopt wire-witness as an exact linked
  producer, bracket one sidecar as its own durable effect, supply one
  per-attempt loopback proxy endpoint to the child, and report confinement and
  witness posture without treating testimony as authority or creating a
  general sidecar host.
amends:
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
extends:
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter-claude-code/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
interface_references:
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
    obtained: "2026-09-26"
    rationale: >
      Adopt the exact attempt-binding, stdio, and bracketed-lifetime contract
      rather than infer a sidecar protocol locally. The producer's
      code-formatted `AttemptBinding` heading canonicalizes exactly to
      `3-1-attemptbinding`.
obligations:
  - id: "I-1"
    kind: invariant
    text: "Statecraft mints the run id, attempt, and effect id before spawn, and every sidecar record repeats all three unchanged."
    anchor: "3-2-attempt-binding-and-effect-bracketing"
  - id: "I-2"
    kind: invariant
    text: "Only the witness sidecar parses hostile TLS, HTTP, SSE, or WebSocket traffic; the trusted supervisor treats protocol messages and artifacts as bounded data."
    anchor: "3-4-hostile-traffic-stays-out-of-the-supervisor"
  - id: "R-1"
    kind: requirement
    text: "Each attempt has at most one supervisor-owned witness sidecar and exactly one supplied per-attempt loopback endpoint when witness capture is requested."
    anchor: "3-3-one-sidecar-and-one-endpoint"
  - id: "R-2"
    kind: requirement
    text: "Every attempt records one accurate witness posture, while task or acceptance policy alone decides whether missing evidence refuses acceptance."
    anchor: "3-7-witness-posture-and-policy"
---

# 013: Supervised wire-witness adoption

## 1. Purpose

Specs 003 and 004 already own durable effects, provider supervision, and the
protected evidence boundary. They currently permit no process outside the
confinement to act for the child and explicitly refuse loopback. A witnessed
attempt therefore needs an authority amendment, not an implementation detail.

This spec permits one narrow linked producer and one narrow sidecar role. It
does not make Statecraft a plugin host, does not make wire-witness authoritative,
and does not let observation decide acceptance.

## 2. Territory

This spec owns no product code. A later implementation reaches the run record
through spec 003 and the generic adapter plus the current provider adapter
through spec 004, using only the `extends` edges above.

The linked producer input used to draft this contract is wire-witness commit
`f701a7b51e492ec98bf6b160bfd3e3f8bdf7990d`, tree
`db9513c5faf81427c18468119590b2c2a712b4e6`, spec 005 content digest
`sha256:a8195055f0efc508abf86c7fbf86f577cce5556d12d8f3dfe67fd486a988f65d`.
That producer spec was draft at the cited commit. Its owner later ratified the
exact bytes, but no release or Statecraft adoption follows from that act.

## 3. Behavior

### 3.1 Linked-producer adoption

wire-witness is linked or executed as a versioned product dependency. It is
never discovered from a runtime plugin directory and never loaded through a
general extension API. An adopted producer identity contains:

1. an exact immutable release version and artifact digest;
2. the `wire-witness.sidecar/1` interface identity;
3. the supported `AttemptBinding` and terminal-message schema versions;
4. the exact interface digest verified against the producer corpus;
5. the qualification record for that artifact on every supported platform;
6. the Statecraft release range that adopts it; and
7. the rollback identity that remains supported until the upgrade is accepted.

The draft commit in section 2 is sufficient to pin this proposed contract. It
is not a releasable dependency. Implementation refuses an unavailable,
unreleased, digest-mismatched, interface-stale, or unqualified producer rather
than selecting another executable from `PATH` or negotiating an undocumented
shape. An upgrade changes the adoption ledger, reruns interface verification
and compatibility fixtures, reruns platform qualification, and is reviewed as
a Statecraft dependency change. Rolling back restores the prior exact producer
and its compatible Statecraft identity. It never silently widens a version
range.

### 3.2 Attempt binding and effect bracketing

For every attempt, Statecraft owns `run_id` and `attempt`. If witness capture is
requested, Statecraft also mints a fresh non-empty `effect_id`. It writes and
durably acknowledges one sidecar-effect intent carrying the three values before
starting the sidecar. The sidecar receives:

```text
AttemptBinding { run_id, attempt, effect_id }
```

Every readiness, finding, exchange, and terminal message must repeat those
three values byte for byte. Statecraft binds records by the supplied tuple and
the dedicated stdio channel for this launched effect. It never establishes or
repairs binding from a timestamp, process id, child-provided header, connection
order, filesystem timing, log proximity, provider request id, or content.

The sidecar effect has exactly one closing outcome. The outcome repeats the
`effect_id` and records:

- the capture digest or an explicit reason no digest exists;
- the attestation identity or explicit `unknown`;
- `complete`, `incomplete`, `absent`, or `unknown` capture completion, with
  every supplied gap reason;
- `finished`, `failed`, `stopped`, `killed`, or `unknown` sidecar termination;
- the exact producer and interface identities; and
- the final witness posture from section 3.7.

EOF, malformed protocol, duplicate readiness, a duplicate terminal message,
or supervisor interruption never synthesizes a successful outcome. When no
closing outcome is durable, spec 003's existing fold retains the open effect
for reconciliation. A retry creates a new attempt and new effect identity. It
does not reuse the old CA, endpoint, or effect.

### 3.3 One sidecar and one endpoint

At most one wire-witness sidecar belongs to an attempt. Statecraft starts it,
owns its lifetime, retains the process handle for supervision, and terminates
it after the child and capture closure. A child cannot request, replace, or
start a sidecar through this contract.

When capture is requested, the sidecar reports one ready endpoint over the
dedicated protocol. Statecraft validates that the bound ready message reports
the single reserved per-attempt TCP endpoint, that its reported address is
loopback, and that it remains the endpoint expected by the sidecar
qualification. On Linux this validates the reported address, not OS-enforced
address restriction; section 3.6.1 records that residual explicitly.
Statecraft then supplies exactly that endpoint to the child in the constructed
environment. The child receives no sidecar discovery path, fallback proxy,
proxy list, or general loopback permission. A second endpoint, an endpoint not
carried by the bound ready message, a wildcard listener, or a non-loopback
reported address refuses child spawn.

The endpoint is a transport exception, not an identity. The binding remains
the tuple in section 3.2 even if a port number is reused after an attempt ends.

### 3.4 Hostile traffic stays out of the supervisor

The sidecar, not the trusted Statecraft supervisor, terminates child-facing
TLS and parses HTTP/1.1, HTTP/2, SSE, WebSocket, provider streaming events, and
malformed or unknown wire input. Statecraft reads only the bounded versioned
stdio protocol and bounded artifacts after custody closes. It treats every
sidecar string, path, count, digest, gap, and attestation field as untrusted
testimony until the applicable verifier and admission policy judge it.

Statecraft never imports a proxy parser, forwards hostile frames through its
run record decoder, or interprets a captured provider message as a command.
Protocol limits bound line size, message count, artifact size, and shutdown
time before any sidecar is launched.

### 3.5 Ephemeral CA custody

Every witnessed attempt uses one fresh CA. The sidecar creates the private key
after the effect intent is durable and retains it only in its memory. The key
is never passed through stdio, environment, command arguments, the run record,
the exchange directory, an artifact path, or supervisor memory. Statecraft may
receive only the child-visible CA certificate path from the bound ready
message, validates it as a regular bounded file in the attempt's sidecar area,
and supplies that certificate path to the child.

Sidecar termination destroys the in-memory key. Crash or forced termination is
reported as a termination state, never as proof of zeroization. A persistent
private-key path, a certificate outside the per-attempt area, or a second CA
refuses child spawn. If private-key material appears in a protocol field after
spawn, Statecraft refuses the attempt, stops the child and sidecar, and records
the custody violation. It never continues capture with exposed key material.

### 3.6 Confinement amendment

This section narrowly amends spec 004 section 3.18 rules 3, 6, 7, 8, 11, and
12. Every other protected path, writable-root, preflight, and residual rule
continues unchanged.

#### 3.6.1 The child

The child remains under spec 004's existing confinement. Its only new route to
a process outside that confinement is TCP connect to the exact per-attempt
loopback endpoint from section 3.3. It may not bind a listener, connect to any
other loopback port, connect to the sidecar by a Unix socket, signal it, or
reach a general local agent.

On macOS, the generated Seatbelt profile adds one network-outbound exception
for the exact loopback address and TCP port after all other loopback access is
denied. The launch self-test must connect to the supplied endpoint and must be
refused on another loopback port and a non-loopback address using that port.

On Linux, the existing Landlock ruleset adds TCP connect permission for the
endpoint port and the existing seccomp family restriction remains. Landlock
mediates the port, not the destination address. It therefore cannot prove that
the connection is loopback. Statecraft records `loopback-address-unenforced`
and the exact port as a residual. The launch self-test must connect to the
supplied endpoint and must be refused on another TCP port. This proves the
port restriction only; it does not prove the destination address. The sidecar
binds the reserved port before child spawn; a bind failure or loss of the
bound ready channel refuses spawn. No report may call the Linux address
restriction enforced.

#### 3.6.2 The sidecar

The sidecar is supervisor-owned but is not trusted to reach Statecraft evidence
or authority. It runs under a separate confinement profile whose writable set
is only its per-attempt output and certificate area plus its own bounded
temporary directory. It cannot read or write the run chain, override journal,
state authority, launch records, operator checkout, child workspace, another
attempt, provider configuration, Git state, credentials beyond the inherited
provider connection it proxies, or any publication mechanism.

On macOS, a separate Seatbelt profile permits binding only the selected
loopback endpoint, permits outbound network connections only as required by
the capture allowlist, and applies the protected-set and process-route denials
from spec 004. Its self-test proves the protected path denials, endpoint bind,
refusal of another listener, and refusal of a disallowed outbound target.

On Linux, a separate Landlock ruleset grants only its output paths, TCP bind on
the selected endpoint port, and outbound TCP ports required by the capture
allowlist, with `no_new_privs`, the existing signal and abstract-socket scopes,
and the existing seccomp family, user-namespace, and `io_uring` denials.
Landlock cannot restrict either bind or connect by address, and does not
mediate UDP. Those facts are recorded as
`sidecar-bind-address-unenforced`, `sidecar-connect-address-unenforced`, and
`sidecar-udp-unmediated`. Qualification must name any additional mechanism
that narrows them. Without one, no posture claims address-level confinement.

The sidecar's need to parse hostile traffic is not a reason to run that parser
inside the trusted supervisor. A sidecar confinement failure refuses witness
startup. It does not fall back to in-process capture.

#### 3.6.3 Records and residuals

The attempt records the child and sidecar confinement separately: platform,
mechanism, profile or ruleset digest, exact endpoint, resolved writable roots,
self-test results, producer qualification identity, and every residual above.
An endpoint exception does not erase spec 004 section 3.18 rule 12's existing
provider-configuration, Mach-service, activation, or host-listener residuals.
The new Linux endpoint residuals remain visible beside them.

### 3.7 Witness posture and policy

Every attempt created under this contract records exactly one posture. Readers
also expose one compatibility-only rendering for attempts that predate it:

| Posture | Meaning |
|---|---|
| `witnessed` | The bound sidecar finished, required artifacts passed integrity checks, and capture is recorded as complete. This is not acceptance. |
| `witness-not-requested` | The task and policy did not request witness evidence. |
| `witness-unavailable` | Witnessing was requested but the exact producer or supported platform mechanism was unavailable before child spawn. |
| `witness-failed` | A started sidecar or its protocol failed, including when the process exits before reporting ready. |
| `witness-incomplete` | A terminal result exists and names incomplete capture or gaps. |
| `witness-required-and-missing` | Applicable task or acceptance policy required witness evidence and no admissible evidence exists. |
| `witness-unqualified` | Evidence exists, but the producer, interface, confinement, integrity, or qualification identity is not one the policy admits. |
| `not-recorded` | The attempt predates this contract and contains no witness-posture field. A new attempt never records this value. |

Policy, not wire-witness, decides whether `witness-unavailable`,
`witness-failed`, `witness-incomplete`, `witness-required-and-missing`, or
`witness-unqualified` refuses acceptance. Optional-by-default is the proposed
policy posture. A later task may require witness evidence explicitly. A run
without evidence remains accurately labeled and is never retroactively
described as witnessed.

## 4. Observable negative cases

| Case | Required result |
|---|---|
| Producer version resolves but interface digest differs | Refuse witness startup; do not select a different executable. |
| Sidecar message changes `run_id`, `attempt`, or `effect_id` | Protocol finding; it closes no effect and binds no evidence. |
| Child announces a matching header | Ignored for binding. |
| Sidecar reports two endpoints | Refuse child spawn and close or reconcile the sidecar effect with the gap named. |
| Endpoint is wildcard or non-loopback | Refuse child spawn. |
| Sidecar reaches terminal without a capture digest | Record explicit absence or gap; never synthesize a digest. |
| Sidecar exits before ready | Record `witness-failed`, or retain an open effect if no outcome is durable; never render the started process as unavailable. |
| Sidecar exits after ready | Record `witness-failed` or retain an open effect if no outcome is durable; never infer success from child completion. |
| CA private key appears in a file or protocol field | Refuse the attempt and record the custody violation. |
| Linux record omits an address or UDP residual | The posture is unqualified and cannot be rendered as enforced. |
| Witness evidence is absent on a task that did not request it | `witness-not-requested`; the absence alone does not refuse acceptance. |
| Witness evidence is absent on a task that requires it | `witness-required-and-missing`; policy refuses acceptance. |
| Witness reports complete capture | Testimony only; it does not establish provider use, model compliance, correctness, or acceptance. |

## 5. Compatibility and adoption

This is an additive protocol-major-compatible Statecraft amendment but an
authority change to specs 003 and 004. Existing attempts have no witness
posture and render it as `not-recorded`, never `witness-not-requested`.
Existing unwitnessed attempts are not re-judged. A Statecraft release may adopt
this contract only with an exact released wire-witness identity, current
interface pin, compatibility fixtures, macOS and Linux confinement
qualification, and rollback evidence.

## 6. Out of scope

- Implementing the sidecar, evidence admission, or a wire-aware acceptance
  policy.
- A runtime plugin system, arbitrary sidecars, multiple witnesses, transparent
  host-wide proxying, or a general loopback grant.
- Binding by time, PID, child headers, connection order, filesystem timing, or
  log proximity.
- Persisting a CA private key or captured credential-bearing traffic.
- Deciding that a capture proves model behavior, correctness, acceptance, or
  provider qualification.
- Provider traffic, credential inspection, spend, remote administration,
  publication, release, or deployment.

## 7. Decisions recorded during implementation

None. This draft has no implementation authority.

## Verification

Implementation acceptance is not run while `implementation: pending`. Draft
review uses the repository gate, code gate, coupling gate, relationship report,
and exact interface verification against wire-witness commit
`f701a7b51e492ec98bf6b160bfd3e3f8bdf7990d`. Those checks establish corpus and
reference consistency only.

Registry completeness requires the generated entries
`.statecraft/derived/codebase-index/by-spec/013-supervised-wire-witness.json`
and
`.statecraft/derived/spec-registry/by-spec/013-supervised-wire-witness.json`.
Draft publication commits both results from the pinned `make refresh`, and the
gate must report the codebase index and spec registry fresh.
The AI review policy excludes `.statecraft/derived` from its authored-diff
input. Absence from that filtered path list is therefore not evidence of
absence from the exact head; the exact-head governance gate establishes the
presence and freshness of compiler output.
