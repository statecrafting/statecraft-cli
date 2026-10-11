---
id: "037-closed-admission-gate"
title: "Envelope admission evaluates through action-gate's closed mode, with identical decisions and bytes"
status: approved
implementation: complete
created: "2026-10-07"
summary: >
  Amends 005 for how the envelope's admission::evaluate combines its
  requirements. It builds one closed action-gate gate per evaluation
  (action-gate-core 0.3.0, action-gate spec 004), with one required check per
  requirement registered in today's reason order, and maps every deny back to
  its RefusalCode. Decisions, refusal codes, their order, the serialized
  Admission and the policy-eval claim are unchanged, which a differential test
  against the frozen pre-change evaluator proves. The envelope stays pure and
  Apache-2.0; the policy digest stays the policy's identity.
amends:
  - "005-acceptance-and-evidence"
extends:
  # The evaluator, its new dependency and its tests live in 005's crate;
  # `amends` does not make this spec an owner of that code (001 section 5).
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-envelope/" }, nature: additive }
depends_on:
  - "005-acceptance-and-evidence"
obligations:
  - id: "R-1"
    kind: requirement
    text: "admission::evaluate builds one closed action-gate gate per evaluation, every check required, registered in the order the refusals are reported, and refuses with every deny in that order."
    anchor: "3-1-the-gate"
  - id: "R-2"
    kind: requirement
    text: "For every policy and input, the Admission value, its serialized bytes and the policy-eval claim equal those of the evaluator at 8ba0a8e."
    anchor: "3-3-what-does-not-change"
  - id: "R-3"
    kind: requirement
    text: "The envelope depends on action-gate-core exactly 0.3.0 with no default features, and its dependency closure stays pure and Apache-2.0 compatible."
    anchor: "3-4-the-dependency"
---

# 037: Closed admission gate

## 1. Purpose

Spec 005 section 3.11 keeps two judgments apart: `statecraft-acceptance`'s
`admit`, and the envelope's `admission::evaluate`, which the hosted platform
runs as its admission evaluator (its spec 004, B-10 to B-12). The second was a
hand-written deny-by-default combinator: it collected a refusal for every
requirement that failed and admitted only when there was none.

action-gate 0.3.0 adds that combinator as a closed evaluation mode
(action-gate spec `004-closed-evaluation-mode`, written with this evaluator
as one of its two consumers). This spec moves `admission::evaluate` onto it,
so the CLI and the platform run one published combinator instead of a private
one. **It changes no decision and no byte.** What a policy requires, what each
refusal is called and how it serializes stay 005's.

## 2. Territory

`crates/statecraft-envelope/src/admission.rs`, the crate's `Cargo.toml`, and
two test files, `tests/admission_differential.rs` and
`tests/admission_oracle/mod.rs`, all inside 005's directory unit through the
`extends` edge above. `statecraft-acceptance::admit` is untouched.

## 3. Behavior

### 3.1 The gate

`evaluate(policy, input)` builds one gate with `Gate::builder()`, registers
one check per requirement, requires every registered id with `require_all`
(which closes the gate), and calls `evaluate_exhaustive`. Checks are
registered in the order the refusals have always been reported:

1. one check per `require_artifacts` entry, in policy order (`artifact/<i>`);
2. for each verdict, in input order, its integrity, signature, issuer-trust
   and subject-binding checks (`verdict/<i>/<dimension>`);
3. `approver-is-submitter`;
4. `approval-count`.

Every check is registered whatever the policy says, and a requirement the
policy does not set allows, so a permissive policy admits and the set of ids
depends only on the input's shape. Each check always decides: an allow when
its requirement holds, otherwise one blocking deny. The admission is `admit`
exactly when `denials` is empty; otherwise it is a refusal whose reasons are
the denials, in registration order, each mapped back as section 3.2 says.

The predicates are those of 005 and the platform's B-10 to B-12, unchanged:
the signature requirement is derived from the three booleans as before
(`signature_requirement`), issuer trust is judged only under `Trusted`, and
the count excludes the submitter only when `approver_may_not_be_submitter` is
set and counts distinct principals only when `approvers_distinct` is.

### 3.2 From a deny to a refusal

action-gate's `Decision` has no structured payload (its spec 001 D-4), so a
check carries its refusal in the deny's `reason`, as the refusal code's
canonical JSON (the bytes `RefusalCode`'s serializer writes), and the adapter
reads it back with `RefusalCode`'s deserializer. For the seven codes this
evaluator produces the round trip is exact, which section 3.3's test proves.

A deny that does not decode is one the gate made itself
(`gate:deny:closed:*`). None can occur, because every requirement is
registered and every check decides. If one ever did, it becomes
`RefusalCode::Unknown` carrying the reason verbatim, so admission still
refuses: the adapter fails closed and never panics on it.

### 3.3 What does not change

For every `(policy, input)`: the `Admission` value; its serialized bytes,
including `reason` and the order of `reasons`; and the
`statecraft/policy-eval/v1` claim from `policy_eval_claim` and its canonical
bytes. `AdmissionPolicy`, its digest, `RefusalCode`, `Admission` and their
serde are untouched, as are `policy_eval_claim` and the public signatures of
`evaluate`, `AdmissionInput` and `Decision`. No `action_gate_core` type
appears in the envelope's public API.

The policy digest remains the policy's identity. The gate's `config_hash`
varies with the input's shape (one check per verdict and per required
artifact), so it identifies an evaluation's assembly, not a policy, and is not
recorded.

### 3.4 The dependency

`action-gate-core = { version = "=0.3.0", default-features = false }`: exact,
because a decision rule is adopted by review, and without `checks-common`, so
no `regex`. Three crates enter the envelope's own normal dependency closure:
`action-gate-core` 0.3.0, `action-gate-types` 0.1.0 and
`canonical-keysort-json` 0.1.0. Only the first two are new to this workspace's
`Cargo.lock`; `canonical-keysort-json` 0.1.0 was already locked here, through
`statecraft-run`'s `attest-ledger-core`. Each of the three is Apache-2.0 and
`unsafe_code = "forbid"`, none has a build script, and none reads a clock, the
environment, the file system or the network. The core's other dependencies
were already in the envelope's own closure before this change: `serde`,
`serde_json` and `hex` directly, and `sha2` 0.10.9 through `ed25519-dalek`
2.2.0 (the envelope's direct `sha2` is 0.11, a separate crate that also
stays). The crate's literal Apache-2.0
licence (005 section 3.15) is unaffected.

### 3.5 The platform

The platform consumes the envelope by git revision (005 section 3.17). Because
section 3.3 holds, a platform pinned before this change and one pinned after
it admit and refuse identically, so this spec requires no pin move. A later
move brings `action-gate-core` 0.3.0 into a graph that already carries 0.2.0
through Rahi. The two are semver-incompatible and Cargo keeps both; nothing
passes an `action_gate_core` value between them, because the envelope exposes
none and the shared `action-gate-types` 0.1.0 is one crate in both.

## 4. Out of scope

- `statecraft-acceptance::admit` and the receipt (005 sections 3.1 to 3.10).
- Moving the platform's pin, which is the platform's change.
- Any change to what a policy requires, a new refusal code, or a structured
  payload on action-gate's `Decision` (an action-gate types 0.2 question).

## 5. Resolved decisions

Taken by the agent drafting this spec; the owner ratifies them by approving it.

- **The refusal travels in the reason.** action-gate spec 004 section 6.1
  allows either a map by check id or a canonical refusal carried in the
  reason. The reason is chosen because it keeps each deny self-describing and
  needs no second table that could disagree with the checks.
- **Every check decides.** A check whose requirement is not set allows rather
  than abstaining, so `required_undecided` and `no_check_decided` are
  unreachable by construction and every deny is a check's own.
- **The oracle is frozen, not shared.** `tests/admission_oracle/mod.rs` is the
  pre-change body at `8ba0a8e`, copied character for character, never edited
  to make a test pass, on the pattern of `tests/legacy_cli/`.
- **Sweeps are exhaustive where the space allows, seeded where it does not.**
  Every evidence requirement against every single verdict, every approval
  requirement against every decision sequence of up to three, and 20,000
  seeded mixed inputs whose coverage the test asserts. No new dependency.
- **R-3 is measured on the resolved graph** (2026-10-10). A unit test in
  `admission.rs`, inside `--lib admission`, reads `cargo metadata --offline
  --locked` and asserts the exact pin with no default features, that the three
  crates section 3.4 names are the only ones `action-gate-core` adds to the
  envelope's normal closure, that no `regex` is in it, and that each of the
  three is Apache-2.0, forbids unsafe code, has no build script and names no
  clock, environment, file system, network, process or thread API. R-1 is held
  by the registration-order unit test, R-2 by the differential test; with all
  three covered and the acceptance green, implementation is complete.

## Verification

The differential test compares this evaluator with the frozen pre-change one;
the existing suites hold the serializations and the golden vectors.

```verify:cli
cargo test -p statecraft-envelope --test admission_differential
cargo test -p statecraft-envelope --test behavior
cargo test -p statecraft-envelope --test cli_compat
cargo test -p statecraft-envelope --test vectors
cargo test -p statecraft-envelope --lib admission
```
