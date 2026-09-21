---
id: "010-managed-environment-and-initialization"
title: "The Statecraft-managed environment: one global home, one project area, one initialization flow, and where authority comes from"
status: draft
implementation: pending
created: "2026-09-20"
summary: >
  spec-spine withdraws its kit and its public initializer, so the transition
  contract in 002 section 3.7 has no second installer to coexist with and this
  product becomes the sole user-facing initializer and environment manager. This
  spec is that realignment. It fixes one Statecraft-managed global environment
  under the product home and one per-project area under .statecraft/; it makes
  the reusable harness a single global source delivered by adapters rather than
  copied into every repository; it fixes the governance producer boundary as the
  spec-spine library's files-as-data result, with an explicit layout and a closed
  set of paths this product will place; it fixes the root instruction bridge as a
  tracked managed modification rather than ownership of a user's file; it fixes
  configuration authority as four layers with provenance rather than one
  last-writer-wins merge; it fixes the solo and team boundary so every capability
  has a local implementation and an unreachable platform is an honest unavailable
  state rather than a downgrade to solo; and it moves this repository's compiled
  artifacts to .statecraft/derived/ in one bounded step.
establishes:
  # Claimed by the change that writes it, which is this one. Section 2 says why
  # the claim could not be in the draft: the gate carries
  # `index check --fail-on-unresolved`, so a spec claiming a crate it has not
  # written yet fails. `008` set the precedent.
  - { kind: directory, path: "crates/statecraft-home/" }
amends:
  # Section 3.7's transition contract is withdrawn and the committed declaration
  # gains a project block; section 3.11 below states exactly what changes and
  # what is retained. Declared once, here: the amended spec.md is not edited.
  - "002-environment-lifecycle"
  # Section 3.6's "no configuration file may change a rule" gets its precise
  # reading in section 3.6 below: a layer supplies a value, never a rule, and
  # every value carries the layer that supplied it.
  - "006-command-surface"
extends:
  # The project area, the committed declaration and the ownership model are 002's
  # territory. This spec amends them: the manifest carries a project declaration
  # and a modification list, and section 3.7's kit-coexistence machinery is
  # withdrawn with the installer that motivated it. Corrective, not additive.
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: corrective }
  # Every verb this spec names is a binding inside the crate 006 owns as one
  # directory unit, which is how 006 section 3.1 admits a verb to the tree.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
  # The authored-content check skips the compiled artifacts by path, and section
  # 3.9 moves them. The script is 001's unit, so the edge is declared rather
  # than discovered by the coupling gate.
  - { spec: "001-boundaries-and-authority", unit: "scripts/check-authored-content.sh", nature: corrective }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "006-command-surface"
---

# 010: The Statecraft-managed environment

## 1. Purpose

Spec `002` section 3.7 is a transition contract between two installers. It was
written when `spec-spine init --with-kit` wrote a session harness and this
product wanted to own the working environment, and its whole job was to keep the
two from silently claiming the same files. spec-spine is withdrawing the kit and
the public initializer. The second installer is going away, so a contract whose
subject is the collision between two installers has no subject left, and the
open question inside `D-04` (whether this product installs a harness at all, or
only ever adapts around one spec-spine installs) is answered by the other side
of it disappearing.

That answer arrives with obligations this corpus has not written down. If this
product is the only initializer, it owns: the global environment the operator
has, the project area a repository gets, the reusable harness that must not be
copied into every repository, the governance starter files it now has to obtain
from a library rather than from a command, the user's own instruction file it
must not take over, and the question of which configuration layer is allowed to
decide what. This spec fixes those, and it fixes them as one change because they
are one boundary: an initialization flow that cannot say where its configuration
came from is not an initializer, it is a file copier.

It also fixes the solo and team boundary, because that is where an initializer
is most tempted to lie. The complete local capability set works with no account,
no login, no hosted connection, no paid plan and no platform-issued token. A
project may be enrolled into a team, and for an enrolled project the platform is
the coordination authority for shared approvals, eligibility and policy. An
unreachable platform is an honest unavailable state. It is never a downgrade to
solo ownership, and a local approval never satisfies a required shared one.

## 2. Territory

`crates/statecraft-home/`.

Amended by the `extends` edges in the frontmatter, and by nothing else:
`crates/statecraft-environment/` (the committed declaration gains a project
block and a modification list; section 3.7's kit machinery is withdrawn) and
`crates/statecraft-cli/` (the verbs of section 3.7 below).

Not this spec's territory: run semantics (`003`), the execution adapter (`004`),
acceptance (`005`), the provider adapter (`008`), the integration slice (`009`).
This spec adds no run, no scheduler and no second ledger. Where it needs a work,
run or acceptance semantic, it reuses the one that exists.

The claim on `crates/statecraft-home/` was deliberately absent from the draft
that proposed this spec: the gate carries `index check --fail-on-unresolved`, so
a spec claiming a crate it has not written yet fails. `008` set the precedent,
and the claim arrived with the change that wrote the crate.

## 3. Behavior

### 3.1 One Statecraft-managed global environment

One global environment, at the product home: `~/.statecraft/`, overridden by
`STATECRAFT_HOME`. The override already exists (`006` section 5) and is
unchanged; what changes is that the home now has a declared shape rather than
two files that happened to be written there.

| Path under the home | Holds | Owner |
|---|---|---|
| `home.json` | The home schema version, the operator's name, and personal defaults. | this spec |
| `tools.json` | Installed tools and harness revisions: for each, what was requested and what actually resolved. | this spec |
| `harness/<revision>/` | The one canonical harness source: skills, agent definitions, rules, hooks and adapter templates. | this spec |
| `delivery.json` | What each native-discovery adapter was asked to deliver, where, and what the delivery check observed. | this spec |
| `approvals/` | Local approval records, one file per project. | this spec |
| `projects.json` | The register of targets. | `002`, unchanged |
| `qualifications.json` | Provider qualification records. | `008`, unchanged |

Three rules the shape has to keep:

1. **Extend, never replace.** This spec reads and writes only the files it owns.
   The register and the qualification records are read through their owners and
   are never rewritten here, so adopting this shape loses no existing record.
2. **An absent home is an empty home, not an error.** Every read-only verb
   answers with no home present, which is what makes a fresh machine usable
   before anything is installed.
3. **No credential lives here.** The home holds no provider credential and no
   platform token. Provider credentials stay where the provider keeps them
   (`008` section 3.6), and this product does not relocate them.

### 3.2 One per-project area

Inside a target repository, exactly four paths, and no others:

| Path | Committed | Holds |
|---|---|---|
| `.statecraft/environment.json` | yes | The environment declaration: pins, managed-file ownership, tracked modifications, and the project block of section 3.6. |
| `.statecraft/AGENTS.md` | yes | The Statecraft-managed project instructions. |
| `.statecraft/derived/` | yes | spec-spine's compiled artifacts. |
| `.statecraft/state/` | no | Runtime state. Ignored. |

The existing locations are preserved and are not moved: `spec-spine.toml` at the
repository root, `specs/` at the repository root, `standards/spec/` where it is,
and every ordinary project document where it is.

**`.statecraft/` as a whole is never ignored.** Ignoring it would take the
committed declaration, the managed instructions and the compiled artifacts out
of version control in one line, and the three of them are exactly what makes a
project governed. A `.gitignore` that ignores the whole directory is a refusal
with that reason, not a warning.

The committed declaration carries **no secret, no machine-specific absolute path
and no personal preference**. A declared value that is an absolute path, or that
begins with `~`, is refused at write time and reported by `doctor`: a remote
worker resolving the same declaration must be able to satisfy it independently,
and a path under one operator's home cannot be satisfied by anyone else.

### 3.3 The root instruction bridge

The root `AGENTS.md` stays the user's file. This product takes one **managed
modification** of it and never ownership of it:

> the first line is exactly `@.statecraft/AGENTS.md`

Observable rules:

1. If no root `AGENTS.md` exists, it is created holding the import line.
2. If one exists, the import is inserted as the first line and **every other
   line is preserved**, except that leading blank lines left behind by the
   insertion are not accumulated.
3. Insertion is **idempotent**: applying it to an already-bridged file changes
   nothing, and a file that carries the import somewhere other than the first
   line ends with exactly one copy of it, first.
4. The modification is recorded in `.statecraft/environment.json` as a
   modification (path, kind, the exact inserted line, the digest before and the
   digest after), not as a managed entry. Removal removes the inserted line and
   nothing else, and only while the file still begins with it.

**Old generated content is classified, never swept.** A root file whose bytes
are byte-identical to a known generated `AGENTS.md` is reported as
`generated-unmodified`, naming the revision it matched. Anything else is
`customized`. Neither is deleted, neither is rewritten, and project policy is
never moved out of a customized file by this product: an ambiguous body is
preserved and the conflict is reported.

**The import line is a project convention, not a proven universal mechanism.**
Claude Code documents expansion of an `@path` import; Codex's documentation does
not establish an equivalent. So the line alone is never evidence of delivery,
and section 3.4 is what an adapter has to satisfy instead.

### 3.4 One global harness, delivered by adapters, copied into no repository

Skills, agent definitions, rules, hooks and adapter templates are maintained
once, in `harness/<revision>/` under the home. A revision is **content
addressed**: its identity is a digest over its own files, so two homes holding
the same bytes hold the same revision and a changed byte is a different
revision.

A project receives **no copy**. A supported managed session needs no
project-local generic harness directory, and an initialization that wrote one
would be reintroducing the thing this realignment removes.

Native agent discovery may still need something in a native location such as
`~/.claude/` or `~/.agents/`. Those are **adapters**: small links, generated
files, plugins or launch configuration that point at the one canonical source.
Four rules bound them:

1. An adapter never repoints an agent home, never moves an authentication store,
   and never changes personal permissions. Unrelated native user configuration
   is preserved, and a delivery that would have to rewrite a user's settings
   file is not performed.
2. Every delivered name is Statecraft-namespaced, so it cannot collide with a
   generic user skill of the same purpose.
3. Every delivered behavior is **gated to Statecraft projects**: it applies only
   inside a repository holding `.statecraft/environment.json`, and is inert
   everywhere else. An unrelated repository is unaffected by global integration.
4. Writing into a native location is an **explicit operator action**. It happens
   under `home apply`, never as a side effect of a read, a test or a project
   operation.

**Delivery is evaluated, not assumed.** A file existing, or matching a digest, is
not delivery. Each harness has a documented load rule, and this product evaluates
it against the actual tree:

| Verdict | Meaning |
|---|---|
| `reached` | Following the harness's documented load rule from its entry file arrives at `.statecraft/AGENTS.md`. The chain is named. |
| `not-reached` | The rule was evaluated and does not arrive there. The reason is named. |
| `unverified` | The harness has no documented rule this product can evaluate. Reported as unverified, never as delivered. |

Where a harness's rule is evaluable and does not reach the managed file, the
adapter may place **one pointer file, and only where no file exists at that
path**, which is `002` section 3.8 unchanged. Where the rule already reaches the
managed file, **nothing is injected**: a second copy of an import that native
loading already performs is a duplicate, not a belt and braces.

### 3.5 The governance producer boundary

Governance starter files come from the spec-spine **library**, and from nowhere
else. The call is

`spec_spine_core::scaffold_init_json(config_json) -> Result<String, Error>`

returning the existing serialized `Scaffold` files-as-data shape with the
existing `ScaffoldFile` fields. It performs no write and no environment
discovery. The removed `spec-spine init` command is not invoked, no governance
template is vendored into this repository, and there is no fallback installer
built from old kit bytes.

The layout is passed **explicitly**, never defaulted:

```
specs_dir     = "specs"
standards_dir = "standards/spec"
derived_dir   = ".statecraft/derived"
state_dir     = ".statecraft/state"
```

The **contract set** is closed. This product places exactly these, and treats
every other returned path as out of contract:

1. `spec-spine.toml` at the repository root;
2. `<standards_dir>/constitution.md` and `<standards_dir>/contract.md`;
3. `<standards_dir>/templates/**`;
4. `<specs_dir>/000-bootstrap/spec.md`;
5. `.gitignore`, treated as a **fragment**.

An out-of-contract path is **not written**, is named in the report, and makes the
producer's conformance `non-conforming` for that run. That is a finding about the
producer, not a failure of the project: every in-contract path still reconciles.

Reconciliation is `002`'s ownership model and adds nothing to it. A contract path
that does not exist is written and recorded `managed`. A contract path that
already exists is **adopted**: recorded with the digest observed, depended on,
and never rewritten. Existing configuration and authored standards or specs are
not disposable templates.

The `.gitignore` fragment is **merged**: the lines it contributes that are not
already present are appended inside one marked block, and no unrelated entry is
replaced, reordered or removed. Merging is idempotent.

The producer identity (name and exact version) is recorded in the report and in
the declaration's pins. An exact, reproducible dependency is required; a
filesystem path to a sibling checkout is not one, and is never committed.

### 3.6 Configuration authority

There is **one** policy and approval model, with explicit provenance. There is no
single last-writer-wins merge.

| Layer | May supply | May not |
|---|---|---|
| Personal defaults (`home.json`) | A default for a key the project does not require and team policy does not constrain. | Anything the project requires; anything a team constraint forbids. |
| Project configuration (committed declaration) | Project behavior, and requirements that bind every layer below and beside it. | Weakening a team constraint. |
| Team policy (platform, when enrolled) | Shared constraints. | Nothing here is overridden locally. |
| Explicit run choice (an argument) | A value inside every applicable constraint. | A value outside one. |

Resolution answers per key with the value, the layer that supplied it, and every
layer that constrained it. Three rules make it honest:

- A run choice outside a constraint is **refused**, naming the constraint and its
  origin. It is never silently clamped to the nearest allowed value.
- A required policy or a required piece of evidence that is missing resolves to
  **unknown**, and unknown is not success.
- **A candidate cannot choose or weaken the policy that judges it.** The trusted
  configuration for a run is read at the trusted base revision, which is `001`
  section 3.5.1. A candidate diff touching the declaration is an authority
  change, which is `001` section 3.5.2, and the resolution reports it as one and
  keeps using the base's answer.

**This is the precise reading of `006` section 3.6**, which says the binary
reads no configuration file that could change a rule an owning spec fixed. It
still does not. A layer supplies a **value**, never a rule: the rules stay in the
specs that own them, every supplied value carries the layer that supplied it into
the record, and a value no layer is allowed to supply is refused rather than
applied. The declaration and `home.json` are read as the target repository and
the product home, which section 3.6 of `006` already names as two of the three
things the binary reads.

Six postures stay distinct and are never inferred from one another: **approval**,
**eligibility**, **registration**, **qualification**, **arming** and **execution
posture**. Registering makes a target visible; qualification is a read-only
verdict; arming consents to being driven; an approval is about a subject, not
about a repository; eligibility is the join; and the execution posture is `004`'s.

**A resolved run is frozen.** A run resolves its harness revision and its tools
once, and records both the requested identity and the identity that actually
resolved. A later global upgrade changes the home and changes nothing about a run
already resolved.

### 3.7 Initialization, and the verbs

One user-facing initialization flow, spelled the way this binary already spells a
preview and a performance (`env plan` and `env apply`):

| Command | What it does |
|---|---|
| `home show` | The resolved global environment: paths, personal defaults, tools, harness revisions, and each adapter's delivery verdict. Reads only. |
| `home plan` | What `home apply` would write, inside the home and outside it. Writes nothing. |
| `home apply` | Creates or repairs the home and performs adapter delivery. The one operation that writes into a native agent location. |
| `init plan <path>` | Every project change initialization would make. Writes nothing. |
| `init apply <path>` | Performs it. |
| `migrate plan <path>` | The one-time relocation of section 3.9. Writes nothing. |
| `migrate apply <path>` | Performs it. |
| `project enroll <path> <team>` | Records team enrollment in the project declaration. |
| `project unenroll <path>` | Removes it. |
| `config show <path>` | The resolved configuration for a run in that project, per key, with provenance. |
| `approval grant <path> <subject> <operator> <reason...>` | Records a local approval for one subject. |
| `approval show <path> <subject>` | The eligibility of one subject, and the authority behind it. |

`init apply` performs these steps, in this order, and reports each separately:

1. **home**: resolve and check the global environment. No platform login, no
   network, no account.
2. **plan**: compute every project change.
3. **reconcile**: preserve every user file; write only managed content.
4. **governance**: obtain the starter files through the real spec-spine library
   (section 3.5).
5. **project**: write the declaration and the instruction bridge.
6. **corpus**: compile, index and check the intended corpus through spec-spine.
7. **register**: register the project and evaluate its qualification.

Then it stops. **Arming and execution are separate explicit acts** and are not
steps of initialization.

**The bootstrap cycle is avoided by that order, not by an exception.**
Qualification is step 7 because the corpus it judges is created by step 6.
Nothing in steps 1 to 6 requires the project to be qualified, and `init` never
requires a prior qualified state.

**An interrupted or repeated initialization is safe.** Each step is idempotent;
the last completed step is recorded under `.statecraft/state/`; a re-run
re-reconciles and rewrites no authored file. The outcome is `complete` only when
every step reported done, `partial` when any step was withheld or skipped, and
`refused` when a precondition stopped it before any write. Partial work is never
reported as complete.

`init plan` computes the same plan `init apply` performs, from the same code, so
the preview is a preview.

### 3.8 Solo and team

**The complete local capability set requires no platform.** Local
initialization, project management, execution, approvals, eligibility, policy,
evidence, recovery, dashboard operation and governance verification all have
local implementations, and none of them requires a Statecraft platform account,
a platform login, a hosted connection, a paid plan or a platform-issued
authorization token. This is constitution XIII, made operational. Model-provider
credentials are a separate matter and are required only by the provider actually
selected.

**Enrollment is explicit and project-scoped.** It is recorded in that project's
committed declaration and nowhere else, so enrolling one project changes no other
project's authority. The default is solo.

For an enrolled project the platform is the coordination authority for shared
approvals, eligibility and policy. When it cannot be reached:

- the project does **not** become solo-owned;
- work that does not require remote authority proceeds under the project's
  recorded policy;
- a required shared approval is `pending` or `refused`, never granted, and
  **a local approval never satisfies one**.

The hosted platform is **not implemented here**. What is implemented is the local
boundary, the seam a platform would satisfy, and the honest unavailable states.
The shipped implementation of that seam reaches nothing and reports
`unavailable`; no fixture is presented as a live integration, and no document
here claims one.

### 3.9 The one-time relocation of the compiled artifacts

For this repository and for bounded fixtures, `.derived/` moves to
`.statecraft/derived/`, and the configuration, ignore rules, check surface, CI
and documents that name the old path move with it in the same change.

- If a repository holds **both** a non-empty `.derived/` and a non-empty
  `.statecraft/derived/`, the move is **refused**, naming both. It is never
  resolved by choosing one.
- The operation touches exactly one repository root. It sweeps no sibling
  repository and touches no real home directory.
- It is exposed as its own operation and is not a step of `init`.

### 3.10 The operation boundary a dashboard uses

Every operation in section 3.7 is a typed request and a typed outcome in this
spec's crate. The command surface is one caller of it. A local dashboard is a
second caller of the **same** operations: it implements no second settings
engine, no second scheduler and no second policy model.

A hosted platform and a local dashboard share operation semantics. The local
runner owns local processes; the platform coordinates team authority. **One run
has one execution owner**, and no background loop is added by this spec.

No user interface is implemented here. `F-04` stands, and the grade this spec may
claim about a dashboard is the typed boundary and nothing above it.

### 3.11 What replaces the kit transition contract

`002` section 3.7 is withdrawn as a transition contract: with the kit and the
public initializer removed, there is no second installer to coexist with, and
keeping the machinery would be permanent scaffolding for an event that finished.

Three parts of it are **retained**, because each is general rather than
kit-specific, and each still protects a user-owned file:

1. A `foreign` finding names an **owner**, not only a path.
2. `shadowed` stays: a digest match is not evidence that a file is in force.
3. Ownership transfer stays per path, explicit, operator-initiated, reversible,
   and recorded with the digest observed at the moment of transfer and with the
   producer revision it was evaluated against.

There is no deprecation program, no dual installer, no compatibility shim and no
generalized legacy-migration framework. The replaced model has no backward
compatibility requirement.

### 3.12 Observable negative cases

| Case | Required behavior |
|---|---|
| Fresh solo initialization in a temporary home with no platform credential and no platform reachability | `init apply` completes; every step reports done; nothing asked for an account, a login or a token. |
| `init apply` on an unarmed, unregistered project | The project is registered and qualified; it is **not** armed and nothing is executed. |
| A root `AGENTS.md` that exists and carries the user's own content | Its content is preserved; the import is the first line, appears exactly once, and a second run changes nothing. |
| A supported adapter whose harness load rule is evaluable | Delivery of `.statecraft/AGENTS.md` is reported `reached`, naming the chain. |
| A harness with no documented load rule this product can evaluate | Reported `unverified`, never `delivered`, and nothing is injected on the strength of a file existing. |
| A supported managed session | No project-local generic harness copy is required, and initialization writes none. |
| Existing user-global agent settings present before `home apply` | Byte-identical afterwards. |
| An unrelated repository on the same machine | Unaffected by global integration, and the delivered behavior is inert there. |
| A project override and an explicit run choice | Both recorded in the resolved configuration, each with the layer that supplied it. |
| A global upgrade after a run resolved | The recorded resolution of that run is unchanged. |
| The real spec-spine library and commands against `derived_dir = ".statecraft/derived"` | The scaffold is produced, the corpus compiles and indexes, and `check` passes at the new path. |
| A `.gitignore` that would ignore all of `.statecraft/` | Refused, naming the line: governed project metadata stays governed and only runtime state is excluded. |
| A solo project with a local approval for a subject | Eligible, with the authority recorded as the local operator. |
| A project enrolled into a team | Only that project's declaration changes; a sibling project's authority is untouched. |
| An enrolled project whose platform is unreachable, for a subject that requires shared approval | `pending`, never granted; a local approval does not satisfy it; the project is still enrolled. |
| A candidate diff that weakens a constraint in the declaration | Resolution uses the base revision's answer and reports an authority change. |
| An initialization interrupted after some steps, then repeated | No authored file is destroyed, the completed steps are not redone destructively, and the outcome is `partial` until every step reports done. |
| A producer result carrying a path outside the contract set | Not written; named; the producer's conformance for that run is `non-conforming`. |
| Both `.derived/` and `.statecraft/derived/` present | The relocation refuses and names both. No file is moved. |
| A declared value that is an absolute path or begins with `~` | Refused at write time, with the key named. |

## 4. Out of scope

Implementing a hosted platform, a platform protocol or a platform client; any
user interface, which `F-04` defers; publication, release and distribution,
which `F-02` defers; multi-machine environment sync; a package registry or a
plugin marketplace; migrating sibling repositories or any real home directory;
adding a provider adapter, which is `008`'s boundary and `F-07`'s deferral; and
any new work, run or acceptance ledger, because `003` and `005` already own
those semantics and a second one would be the failure this product exists to
avoid.

## 5. Decisions recorded during implementation

Dated entries for choices section 3 was silent on. None changes what it requires.

**2026-09-20: the producer dependency is an exact crates.io pin, and the
boundary it names is not yet satisfied.** Section 3.5 requires an exact,
reproducible dependency and forbids a filesystem path to a sibling checkout.
This build depends on `spec-spine-core = "=0.21.0"` with `default-features =
false`, which drops the `symbol-resolution` feature and its pinned tree-sitter
grammars; the scaffolder needs none of them. That release **still returns
`AGENTS.md` and three `.claude/rules/` files** alongside the contract set,
because trimming the producer is another repository's change and has not
shipped. This product places neither and reports the producer `non-conforming`,
which makes an initialization against the real library `partial`.
`crates/statecraft-home/tests/producer_integration.rs` asserts exactly that, and
`the_producer_is_not_yet_conforming` is the single test that changes when the
trimmed producer releases. No fixture substitutes for the boundary: where a
conforming answer is needed, the suite derives one from the real library's own
bytes and labels it a fixture.

**2026-09-20: the governance files are written by this crate, and every rule
about whether to write them is still 002's.** Section 3.5 reconciles through
spec 002's ownership model. `statecraft_environment::plan::plan` computes what
is written and what is withheld, exactly as it does for an adapter, against a
declaration this crate synthesizes for the pseudo-harness
`statecraft-governance`. The write loop is here rather than
`apply::perform`, because a governance starter file's source is a **template**
and not an adapter, and adding a source-kind field to `Declaration` would have
broken every construction site in spec 008's crate for a field spec 008 does not
need. The decisions stay in one place; only the write does not.

**2026-09-20: a contract path already on disk is adopted, and adoption happens
before planning.** Section 3.5 says an existing path is depended on and never
rewritten. Spec 002's plan reaches that outcome for a path the manifest records
as `adopted`, so the reconcile step records the adoption first and the
governance step then withholds the write with 002's own reason. Nothing new
decides it.

**2026-09-20: the native root is a parameter, never a read of the process
environment.** `STATECRAFT_NATIVE_ROOT` exists and is read at the command
surface only. Every operation takes the parent directory explicitly, so an
operation always states where it would write, and a test stays out of the
operator's real home without mutating a process-wide variable that its
neighbours in the same test binary would also see.

**2026-09-20: hooks ship in the harness and are delivered by nobody.** Section
3.4 refuses to rewrite a user's own settings file, and every mechanism for
wiring a hook into the harnesses this product knows about goes through one. So
`hooks/statecraft-gate.sh` is part of the canonical harness, is content
addressed with the rest of it, and is installed by the operator if they want it.
Native delivery links skills and agent definitions and names no hook, and a test
holds that.

**2026-09-20: the delivery verdict is evaluated from the file tree, and no
verdict comes from a provider run.** Section 3.4 requires delivery to be
evaluated rather than assumed, and a provider run costs credit and proves one
session rather than a rule. So a harness's documented load rule is followed from
its entry file through `@path` imports against the real tree. `claude-code`
reports `reached` with the chain; `codex-cli` reports `unverified`, because its
documentation does not establish the expansion. Neither verdict claims anything
about a live session, and the suite spends nothing.

**2026-09-20: what the relocation rewrites, and what it leaves.** Section 3.9
moves the artifacts and the configuration together. The operation rewrites a
**quoted string literal** in `spec-spine.toml` and an ignore pattern that starts
with the old directory, and nothing else. Prose in a comment that mentions the
old location is left alone: an operation that edited English would be guessing
at meaning. The documents are updated by the change that performs the move,
which is this one.

**2026-09-20: the manifest goes to version 2 with no migration.** Section 3.2
adds the project block and the tracked modifications. Nothing is released and no
version-1 file exists outside a test, so a migration would be machinery
maintained for nobody. A version-1 file is refused by version, which is the
existing behavior and names the number it found.

**2026-09-20: two references to the old location are left for their owners.**
`crates/statecraft-run/tests/negative_cases.rs` builds a `.derived/` directory as
a temptation the product must not read, and `docs/design/00-boundaries-and-reuse.md`
and `docs/decisions/00-founding-decisions.md` mention the old path in prose.
Both are other specs' territory (`003` and `001`), the negative control still
holds against any shard directory, and widening this change to reach them would
be editing a spec's territory for a cosmetic improvement. They are recorded here
as a follow-up rather than swept.

## Verification

Each line is one command. Every row of section 3.12 is one integration test
named after the row it covers, so a row that stops being covered shows up as a
deleted test rather than as a quietly weakened assertion.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
spec-spine index check --fail-on-unresolved
test -f crates/statecraft-home/src/lib.rs
cargo test -p statecraft-home --test negative_cases
```
