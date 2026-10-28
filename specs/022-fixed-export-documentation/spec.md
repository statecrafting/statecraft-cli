---
id: "022-fixed-export-documentation"
title: "Fixed-export documentation workflow"
status: approved
implementation: complete
created: "2026-09-27"
summary: >
  Amends Statecraft's run, adapter, acceptance, and command contracts with a
  bounded offline documentation workflow over revision-pinned context exports.
  It preserves immutable task, delivery, raw result, validation, review,
  correction, and acceptance records as separate facts; proves the workflow
  with a deterministic fake adapter; and grants no live provider, repository
  application, remote export, or publication authority.
amends:
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
extends:
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "019-context-packet-consumption"
  - "020-documentation-task-and-result"
  - "021-documentation-acceptance"
obligations:
  - id: "I-1"
    kind: invariant
    text: "A fixed-export documentation run preserves immutable input, delivery, raw result, validation, review, correction, acceptance, application, and publication as separate records and authorities."
    anchor: "3-1-the-fixed-export-profile"
  - id: "R-1"
    kind: requirement
    text: "The offline profile consumes only revision-pinned retained bytes, invokes only an explicitly selected local adapter, performs no network or repository mutation, and records every output before interpretation."
    anchor: "3-3-bounded-offline-execution"
  - id: "R-2"
    kind: requirement
    text: "The pilot proves complete task-to-acceptance behavior with deterministic fixtures and a fake adapter while withholding live provider, application, remote export, and publication authority."
    anchor: "3-10-the-deterministic-pilot"
  - id: "V-1"
    kind: verification
    text: "Portable fixtures prove exact export binding, offline adapter execution, raw-result custody, independent validation and review, append-only correction, and the absence of repository, network, provider, and publication effects."
    anchor: "verification"
    inputs:
      - "crates/statecraft-run/tests/fixed_export_documentation.rs"
      - "crates/statecraft-adapter/tests/fixed_export_adapter.rs"
      - "crates/statecraft-acceptance/tests/fixed_export_acceptance.rs"
      - "crates/statecraft-cli/tests/fixed_export_documentation.rs"
---

# 022: Fixed-export documentation workflow

## 1. Purpose

Specs 019 through 021 define the context, task, result, review, and acceptance
contracts for documentation work. They intentionally do not choose a first
end-to-end operating profile. Without one bounded profile, an implementation
could prove each document in isolation while leaving input custody, adapter
selection, output retention, correction, or side-effect boundaries ambiguous.

This amendment defines the first profile: documentation from fixed retained
exports, executed and judged offline. The profile connects the existing
contracts without adding a second schema or weakening any authority boundary.
Its first implementation is a deterministic pilot with a fake adapter. It is
not evidence that any live provider received, understood, or completed a task.

This spec is a proposal. It authorizes no implementation, live provider use,
credential access, spend, repository mutation, remote export, publication, or
ratification.

## 2. Territory and prerequisites

This spec owns no product code. A later implementation may change only the
existing units named by its `extends` edges:

- `statecraft-run` owns the profile selection and immutable workflow record;
- `statecraft-adapter` owns the provider-neutral offline invocation boundary;
- `statecraft-acceptance` judges the complete retained workflow; and
- `statecraft-cli` exposes preparation, execution, inspection, review, and
  acceptance through existing command families without duplicating policy.

Implementation is blocked until specs 019, 020, and 021 are ratified and their
prerequisites are implemented against one adopted producer identity. The pilot
must use the same public contracts a later qualified adapter would use. A test
helper that bypasses those contracts does not implement this workflow.

No MCP server, witness service, live provider, network transport, hosted review,
remote artifact store, or publication system is a prerequisite. Adding one is
separate work with its own authority and evidence.

## 3. Behavior

### 3.1 The fixed-export profile

The profile token is `fixed-export-documentation/1`. It composes, without
merging, these immutable subjects:

1. the spec 019 context-task manifest and every retained packet byte sequence;
2. the spec 020 documentation task, skill, and result schema;
3. the selected spec 004 adapter identity and capability declaration;
4. the raw adapter event stream and raw terminal result;
5. decoded result and artifact records;
6. spec 021 reviews, corrections, and acceptance attempts; and
7. one workflow record that identifies each subject and its current state.

The workflow record is `statecraft/fixed-export-documentation/1`. It contains
the registered project, run and attempt, prepared workspace candidate, profile,
every subject identity and digest above, state transitions, stable reasons, and
its own digest. It contains references, not mutable copies of policy facts.

The profile does not collapse intended delivery into observed delivery, raw
output into a valid result, validation into semantic review, review into
acceptance, acceptance into application, or application into publication.
Each remains a separately identified record with separately checked authority.

### 3.2 Preparation from fixed exports

Preparation succeeds only from exact bytes already retained within the
protected run boundary. For every repository packet, Statecraft records the
producer identity, repository identity, revision, tree, dirty-state identity,
page ordering, page digests, closure result, continuation state, omissions, and
warnings required by spec 019.

No preparation step follows a branch, tag, symbolic revision, filesystem
symlink outside the prepared workspace, result-supplied URL, or ambient current
directory. No step fetches, refreshes, recompiles, repairs, or completes an
export. Incomplete or unreadable input stays incomplete or unreadable.

The documentation task binds the exact manifest, skill bytes, result-schema
bytes, expected artifact descriptors, review policy, `no-spend` cost ceiling,
and `publication: prohibited`. Its allowed operations are `read-context`,
`draft-artifact`, and, when requested, `propose-patch`. A proposed patch remains
an output artifact and never becomes permission to apply it.

Preparation writes the complete spec 003 intent before adapter selection or
spawn. Repeating preparation with identical inputs reports the existing
identity. It neither creates a competing intent nor rewrites the first one.

### 3.3 Bounded offline execution

The adapter is selected only from the project and run policy already recorded
under specs 002 and 004. The provider cannot select itself. The profile requires
these declared capabilities:

- exact task-byte delivery;
- exact packet, skill, and schema-byte delivery;
- bounded local input with no network requirement;
- structured terminal result custody; and
- deterministic fake execution for the pilot.

Before spawn, Statecraft verifies that every request byte reference resolves
inside the retained run record, every digest agrees, the selected adapter has
the required capabilities, the task says `no-spend`, and all prohibited
operations remain prohibited. Failure produces a refusal before process start.

The pilot adapter is an in-process or local test double selected explicitly by
fixture identity. It receives the same provider-neutral request and emits the
same event protocol required of any adapter. It performs no network request,
credential read, paid operation, shell escape, repository write, clock read, or
random choice. A real provider executable is never a fallback.

An implementation must not infer that a process is offline merely because a
fixture expects no network. The test environment supplies a deterministic
denial boundary and proves that attempted network, credential, or repository
writes fail the test and are reported as effects or refusals.

### 3.4 Delivery and raw-result custody

The request records intended delivery before spawn. Adapter init events record
adapter-observed delivery after spawn. The two collections are compared but
never merged. In the absence of independently admitted transport evidence,
observed transport remains `not-recorded` exactly as spec 021 requires.

Every adapter event is retained in order with its exact bytes, sequence
position, media type, length, and custody digest. The first terminal result is
retained before parsing. A second terminal result, event after termination,
digest disagreement, sequence gap, or task mismatch is preserved as a finding.
Statecraft never chooses the more favorable output.

Invalid JSON, an unexpected media type, truncated bytes, or a schema mismatch
does not erase the raw result. The workflow records the validation failure;
the independent final judgment then derives `rejected` or `no-acceptance`
under spec 021. It does not ask the adapter to silently repair or reinterpret
output.

### 3.5 Validation and citation resolution

After custody is durable, Statecraft performs these operations independently:

1. decode exact bytes against the task-bound closed result schema;
2. bind the result to task, run, attempt, manifest, adapter, and provider;
3. validate artifact count, logical paths, media types, sizes, and digests;
4. ask the adopted producer to resolve every context citation under spec 019;
5. submit declared evidence references to spec 005 admission; and
6. report every provider omission, unsupported claim, unknown, contradiction,
   and Statecraft citation failure without deduplication across meanings.

Each operation records `pass`, `fail`, `unknown`, `unsupported`, `unreadable`,
or `not-recorded`, its exact subject, and stable reasons. An earlier pass does
not supply a later one. Schema validity does not establish citation support;
citation resolution does not establish semantic support; evidence admission
does not establish the lifecycle grade of every claim.

Validation reads retained bytes only. It does not write an expected artifact
path, materialize a patch into a checkout, invoke a formatter on authored
content, check an external link, or fetch a missing citation.

### 3.6 Reviewable artifacts and proposed patches

Every result artifact remains an immutable blob inside the run record. A
projection may render a safe local copy for a human only in a run-scoped
scratch location declared by the implementation. The projection records source
digest and rendered digest, refuses path escape and replacement, and is never
the acceptance subject in place of the retained bytes.

A documentation patch is parsed as data. Its declared base revision, affected
paths, content digest, binary status, and applicability findings are reported.
Applicability may be inspected against a disposable prepared workspace only
when the trusted-base policy allows that read-only check. The patch is not
applied to the candidate or operator checkout, staged, committed, pushed, or
published.

Generated and authored material stay distinguishable in both artifacts and
review. A generated passage cannot be represented as an authored source, and a
proposed replacement cannot erase the provenance of the bytes it would change.

### 3.7 Human review

An authorized human reviews the exact retained artifacts and claim records
through spec 021. The reviewer separately judges audience fit, completeness,
clarity, lifecycle grades, citation support, evidence, unknowns,
contradictions, omissions, links, examples, and format-specific requirements.

The fake adapter, test author, provider identity, and workflow implementation
cannot grant review authority. A fixture may supply a fake reviewer authority
only inside the deterministic test trust root, where it proves mechanics and
not real-world approval.

Review submission appends a record. It does not edit raw output, fix an
artifact in place, approve application, or publish. A recommendation to accept
remains one input to the independent acceptance operation.

### 3.8 Correction and re-review

A correction creates new immutable result or artifact bytes under spec 021.
It identifies the superseded digest, correction author and authority, reason,
changed claims, and changed artifacts. The workflow preserves both generations
and makes the newer one a new acceptance subject.

Correction can originate from a human-authored replacement or from a new
adapter attempt. The former has no provider-completion claim. The latter uses a
new attempt and new task identity even if context inputs are unchanged. Neither
route overwrites an earlier record or inherits its review and acceptance.

Every corrected subject repeats schema validation, binding, artifact checks,
citation resolution, evidence admission, semantic review, freshness, and final
acceptance. Unchanged sub-results may be referenced for explanation, but only a
new complete judgment can accept the corrected tuple.

### 3.9 Acceptance, application, and publication

Acceptance is exactly the spec 021 result over the full retained tuple. The
workflow records it as `accepted`, `rejected`, or `no-acceptance` and includes
all dimensions and reasons. Provider completion, successful fake execution,
valid schema, resolved citations, a review recommendation, or a clean proposed
patch cannot independently produce `accepted`.

An accepted artifact remains inert. Applying it to an authored file is a new
operation requiring explicit owner authority, a candidate identity, conflict
policy, and its own verification. This spec defines none of those and provides
no application command.

Publication is another later operation and another authority decision. No
profile state, including `accepted`, permits commit, push, pull-request
creation, comment, release, deployment, upload, or remote synchronization.
There is no automatic transition from acceptance to application or from
application to publication.

### 3.10 The deterministic pilot

The first implementation proves one complete workflow using committed, bounded
fixtures:

1. two revision-pinned repository packet exports with fixed clocks and stable
   producer answers;
2. one documentation task requesting a reviewable artifact and proposed patch;
3. one exact skill and one exact closed result schema;
4. a deterministic fake adapter emitting init, progress, and terminal events;
5. claims at several lifecycle grades, including one insufficient-grade claim;
6. one resolved citation, one unresolved citation, one unknown, one
   contradiction, and one provider omission;
7. a first artifact requiring correction and a corrected second artifact;
8. fake authorized review records for both generations; and
9. a first non-accepted judgment followed by acceptance of the corrected tuple.

The pilot asserts exact bytes and digests at every boundary. It also runs
negative fixtures for a moving input, mismatched task, missing delivery event,
duplicate terminal result, malformed schema, citation failure, unauthorized
reviewer, stale manifest, path escape, attempted repository write, attempted
network use, attempted credential read, and attempted publication.

The fixture's final acceptance proves only deterministic conformance to these
contracts. It does not qualify a live adapter, provider, transport, reviewer,
producer release, repository application path, or publication path.

### 3.11 Command workflow

The existing command families expose the workflow without creating a second
policy layer:

- preparation records the fixed-export profile in the run intent;
- the existing run operation executes only the selected adapter and records
  events and raw output;
- `run show` projects identities, delivery states, result custody, validation,
  review, correction, and acceptance without recomputing them;
- the review binding appends an authorized spec 021 review; and
- `accept` judges the complete current subject and appends its result.

Exact command spelling is an implementation decision recorded before code if
the existing spec 006 grammar does not already determine it. No command in this
profile applies an artifact or publishes. JSON and human output remain two
renderings of the same typed values. Historical runs render absent profile
fields as `not-recorded` and are never synthesized into fixed-export runs.

### 3.12 Observable negative cases

| Case | Required result |
|---|---|
| One packet names a moving branch | Preparation refuses before adapter start. |
| Fake adapter capability is missing | Selection refuses; no process starts. |
| Intended bytes lack an init observation | Adapter-observed delivery fails independently. |
| No wire evidence contract is adopted | Observed transport is `not-recorded`. |
| Terminal bytes do not satisfy the schema | Raw bytes remain durable and validation fails. |
| Citation resolves but does not support the claim | Resolution passes and semantic review fails the claim. |
| Proposed patch applies cleanly | Applicability may pass; the patch remains unapplied. |
| Reviewer is the fake provider identity | Authority fails and no acceptance is minted. |
| Corrected artifact changes one byte | Prior review and acceptance do not bind it. |
| Corrected tuple is accepted | No repository application or publication follows. |
| Adapter attempts a network or credential read | The pilot fails and records the prohibited effect. |
| A real provider binary is installed | The pilot still selects only its exact fake adapter. |

## 4. Out of scope

- Ratifying or implementing specs 019, 020, 021, or this spec.
- Qualifying or invoking a live provider, MCP server, witness service, network
  transport, hosted reviewer, or remote artifact store.
- Reading credentials, spending money, checking live external links, or
  exporting retained content or metadata.
- Applying, staging, formatting, committing, pushing, opening a pull request,
  commenting, publishing, releasing, or deploying an artifact.
- Treating deterministic fixtures as evidence of provider delivery, transport,
  reviewer authority, producer adoption, or real documentation quality.

## 5. Resolved decisions

**2026-09-27: the first workflow is fixed-export and offline.** This exercises
the complete contract with immutable retained inputs while avoiding a hidden
dependency on live transport, credentials, provider availability, or spend.

**2026-09-27: the pilot uses the production contracts and a fake adapter.** A
test-only shortcut around task, event, custody, review, or acceptance records
would prove a different workflow.

**2026-10-22: the workflow record's shape and preparation's checks.** The
run-record half (`crates/statecraft-run/src/documentation/workflow.rs`) fixes
section 3.1's record: the profile, project, run, attempt and workspace
candidate; the subjects (manifest digest, every packet page by repository,
revision, tree, dirty state, page and custody digests, the task digest, the
skill digest or `not-required`, the result-schema digest, the selected
adapter and its declared capabilities, every retained event, the raw
result's custody digest, and the review, correction and acceptance
digests); the transitions (`prepared`, `executed`, `validated`, `reviewed`,
`corrected`, `judged`, each with a reason and time); the findings; and a
digest over every other member. Each transition appends a new version as an
evidence record (`fixed-export-workflow`); the current workflow is the
latest version. Preparation refuses a revision that is not a full object id
(`moving-input`), a page whose bytes are not retained under its custody
digest (`not-retained`), a task that permits spend, skill bytes that are not
the pinned ones, a task and manifest that name different subjects, and other
inputs for an attempt already prepared (`competing-workflow`); the same
inputs report the existing version. Events are retained one line at a time,
under custody, before parsing; a line that is not JSON is kept as
`application/octet-stream`; the first `result` event is the terminal one,
and a second terminal, an event after it, or none at all is a finding. Preparation
records the run policy's adapter by name and version, the only one execution
may select, and the same computation without the append (`subjects`) lets a
caller refuse before it writes an intent.

**2026-10-28: the offline boundary and the fake.** The adapter half
(`crates/statecraft-adapter/src/fixed_export.rs`) states section 3.3's five
capabilities as their own closed set (`exact-task-bytes`,
`exact-input-bytes`, `offline-input`, `result-custody`,
`deterministic-fake`), apart from spec 004's six provider tokens, which say
what an adapter can ask of a provider rather than what it does with the
request. Selection takes the first available declaration whose name and
version are exactly the workflow's, and refuses (`adapter-not-selected`,
`capability-missing`) before any process exists. Admission refuses a
workflow that is not `prepared`, an adapter other than the one its run policy
names, a request for other subjects, a task that
permits spend, and a context or documentation bundle that is not the
recorded bytes, with spec 019's and 020's own refusal words. The pilot's fake
(`fixed-export-fake` version `1`) writes the standard event protocol from a
fixture script; it reaches no network, credential, repository, publication,
shell, clock or randomness, and a scripted attempt at one goes to a denial
boundary that refuses it, keeps it, and emits a structured refusal under the
guard `fixed-export-boundary`. Recording keeps every line before parsing it,
records spec 019's and 020's delivery folds, keeps the first terminal
result's raw bytes through spec 020's `record_result`, and appends the
`executed` and `validated` versions. The workflow names as findings each
prohibited effect (`prohibited-effect`), each delivery finding
(`delivery ...`), a line that is not an event (`unreadable-event`), a
terminal event with no result (`no-result`), and a stated length, digest or
media type that is not the bytes' (`raw-length-mismatch`,
`raw-digest-mismatch`, `media-type`).

**2026-10-28: the workflow judgment and the patch as data.** The
acceptance half (`crates/statecraft-acceptance/src/fixed_export.rs`) takes
spec 021's judgment of the current subject unchanged and the workflow's
latest version, and its outcome is never better than spec 021's. It
withholds acceptance (`no-acceptance`) when the workflow is unreadable, not
this profile's, does not hold its digest, never reached `validated`, names
no adapter, or names another task, manifest, raw result or attempt than the
judged subject, and for every workflow finding; a `prohibited-effect`
finding, or a proposed patch with a `path-escape`, rejects. A subject's
identity is `statecraft/fixed-export-subject-digest/1` over spec 021's core
tuple after corrections, so a corrected byte is a new subject that an
earlier judgment does not name. A patch artifact (disposition `patch`) is
read only as data: its `base-commit:` line or `not-recorded`, the paths its
`---` and `+++` lines name, its digest, and whether it is binary;
applicability is `not-recorded`, because this implementation runs no
applicability check, and the patch is never applied. Recording appends the
judgment as one `fixed-export-acceptance` evidence record with its digest,
then the workflow's `judged` version naming that digest, with the outcome as
its reason.

**2026-10-28: the command bindings, artifact custody, and where the pilot
is accepted.** Section 3.11's operations are library bindings in
`crates/statecraft-cli/src/fixed_export.rs`, and no command line verb is
added: the only adapter that declares the profile is the pilot's fake, and a
verb that selected it would put a test double on the product's surface.
`prepare` refuses from the subjects alone, then writes the spec 003 intent
with the manifest and task, then the `prepared` version; the same inputs
report it and begin nothing. `execute` selects and admits as the adapter
half does, runs the fake, records every output, and concludes the attempt
`completed` with a raw result, `failed` without one, or `refused` when
selection or admission refuses, counting each prohibited effect as a refusal
under `fixed-export-boundary` or the refusal under `fixed-export-admission`.
`submit_review` and `submit_correction` are spec 021's review-submission
binding: each decodes the closed record, refuses one for another attempt or
task, and appends only for an identity the trusted-base policy grants, under
the authority it claims and before its `notAfter`, never for the result's
provider or adapter; a correction's new artifact bytes are retained first,
and every changed artifact must be among them. Each appends the workflow's
`reviewed` or `corrected` version. `accept` judges the workflow beside spec
021's judgment and records it under `fixedExportAcceptance`; `run show`
renders the latest version under `fixedExport`, and every attempt outside
the profile renders `not-recorded`. A terminal event's raw result may carry
each artifact's exact bytes (`artifacts`, by `rawRef`), and recording keeps
them as blobs before the result that names them is decoded; this additive
member is how section 3.6's artifacts reach the run record at all. The
command's own `accept` still supplies no documentation manifest, so its
spec 021 judgment keeps context freshness `not-recorded` and accepts
nothing; the pilot reaches `accepted` by handing the binding spec 021's
judgment over the recorded chain with the producer's answers as fixtures,
which is section 3.10's "stable producer answers" and qualifies no producer.

**2026-09-27: acceptance is the end of this profile's authority.** Application
and publication remain distinct future operations because even a fully
accepted artifact cannot authorize mutation or an externally visible act.

## Verification

Implementation acceptance is the portable fixture surface `V-1` names, run
through `make verify SPEC=022`. Fixtures use fixed local bytes, the fake
adapter and fixed review identities, fixed clocks, a deterministic denial
boundary and disposable workspaces; they make no provider call, network
read, credential read, spend, operator-checkout mutation, remote export or
publication.

```verify:cli
cargo test -p statecraft-run --test fixed_export_documentation
cargo test -p statecraft-adapter --test fixed_export_adapter
cargo test -p statecraft-acceptance --test fixed_export_acceptance
cargo test -p statecraft-cli --test fixed_export_documentation
```

These establish preparation from retained, pinned exports; exact selection
and pre-spawn admission; event, result and artifact custody before
interpretation; a workflow judgment never better than spec 021's; the
pilot's corrected generation accepted after a first that is not; and that
nothing is applied or published. They qualify no live adapter, provider,
transport, reviewer, producer release, application path or publication path.
