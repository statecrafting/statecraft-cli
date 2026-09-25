---
id: "002-environment-lifecycle"
title: "Project registration and the Statecraft-managed environment: one global home, one project area, one initialization flow, and where authority comes from"
status: approved
implementation: in-progress
created: "2026-09-16"
summary: >
  How a repository becomes a target this product may work in, and how the
  product installs and maintains the working environment inside it. Fixes the
  three ownership classes (managed, adopted, user), the committed manifest that
  records every managed byte with its source and digest, the pins the
  environment records, what an upgrade does when a managed file has drifted,
  what removal leaves behind, and how an agent-harness adapter declares what it
  owns. Preserving a pre-existing user instruction file is a refusal, not a
  merge. Sections 3.11 to 3.21 carry the realignment that followed spec-spine
  withdrawing its kit and its public initializer, which left this product the
  sole user-facing initializer: one Statecraft-managed global environment under
  the product home and one per-project area under .statecraft/, the reusable
  harness as a single global source delivered by adapters rather than copied
  into every repository, the governance producer boundary as the spec-spine
  library's files-as-data result, the root instruction bridge as a tracked
  managed modification rather than ownership of a user's file, configuration
  authority as four layers with provenance rather than one last-writer-wins
  merge, and a solo and team boundary in which every capability has a local
  implementation and an unreachable platform is an honest unavailable state.
  Sections 3.22 and 3.23 record the counterparty's state at the moment it handed
  its harness over, the order the last of that harness moves in, and what any
  content delivered through the harness mechanism has to satisfy. Section 3.24
  is the one write into a harness's own settings file that section 3.14 rule 1
  admits: narrow, marked, reversible, refused by default, and able to add a
  refusal but never a permission. Sections 3.25 to 3.28 carry the owner's
  resolutions of 2026-09-21: the committed required harness identity beside the
  per-session resolved one, what a managed session records about delivery and
  what that record may not claim, the separation of global adapter registration
  from managed-session permission delivery, and the rule that an unmarked
  registration is the user's however much it resembles shipped content. Section
  3.29 is the same day's narrow authority amendment over the admission itself:
  what a claimed live observation has to carry, which controls the qualification
  boundary enforces rather than describes, and that unverified is the answer
  when the evidence cannot decide. Section 3.30 is the 2026-09-22 amendment
  that sharpens it: each control is judged from correlated structured events
  as refused or executed, a capture is one session read whole, and an
  invocation is bound to its settings by the launch that performed it rather
  than by a description written afterwards. Section 3.31 is the same day's
  amendment for `run`: two write-once startup records per attempt, supply
  recorded by the launch that performs it, the selected revision carried in the
  launch configuration, the answering revision measured from a startup
  acknowledgment in the session's own stream, a mismatch that refuses the
  attempt, and a required revision never recorded as an observed one.
  Section 3.32 corrects four of 3.31's claims: an intent is not a launch, so
  a launch is four write-once records read back as named states and an
  uncertain one is never replayed; the acknowledgment is a correlated one and
  not execution provenance; a managed run supplies its startup hook and an
  admission gate per invocation instead of relying on global registration;
  and startup identity is decided before governed work is released rather than
  refused after the session ends.
  Section 3.34 amends one clause of 3.30 rule 8: a capture may end with one
  allowlisted `task_summary` event after its terminal event, of a closed
  shape, naming the session, and never read.
  Section 3.33 separates the managed-startup trial from the permission
  experiment: one run attempt with a read-only sentinel, one session and one
  version probe, its own approval, named outcomes for absent, unbound, late,
  conflicting and mismatched hook evidence, and three words for effects before
  admission.
establishes:
  - { kind: directory, path: "crates/statecraft-environment/" }
  - { kind: directory, path: "crates/statecraft-home/" }
  # Evidence for the 2026-09-21 round, in this spec's own directory. Claimed
  # as a SECTION rather than as a whole file: a whole-file claim raises L-008,
  # because nothing hashes the file, and the two cures the lint offers are a
  # covering glob in [index] extra_hashed_inputs, which would restamp every
  # shard in the corpus whenever a handoff paragraph changes, and a section
  # unit, which is hashed through its own span and stales only this spec's
  # shard. The second is right: this is evidence, and evidence changing is a
  # fact about this spec and about nothing else.
  #
  # Not a second place for a requirement. Every requirement is in this file
  # and every decision is a dated entry in section 5. The handoff carries what
  # a spec should not: measurement that ages, which is commits, digests, test
  # counts, a producer matrix, a rollback plan, and the live-session script
  # still to be approved.
  - { kind: section, file: "specs/002-environment-lifecycle/handoff-2026-09-21.md", anchor: "7-truthful-implementation-state-of-spec-002" }
  # The acceptance, as a program rather than as prose. Claimed as a whole file:
  # `scripts/**/*` is in [index] extra_hashed_inputs, so it is witnessed and
  # raises no L-008, and a change to it restamping every shard is correct for a
  # file that decides what this corpus may claim about enforcement.
  - "scripts/acceptance/managed-session.sh"
amends:
  # Section 3.6's "no configuration file may change a rule" gets its precise
  # reading in section 3.16: a layer supplies a value, never a rule, and every
  # value carries the layer that supplied it.
  - "006-command-surface"
extends:
  # Every verb sections 3.11 to 3.21 name is a binding inside the crate 006 owns
  # as one directory unit, which is how 006 section 3.1 admits a verb to the tree.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
  # The authored-content check skips the compiled artifacts by path, and section
  # 3.19 moves them. The script is 001's unit, so the edge is declared rather
  # than discovered by the coupling gate.
  - { spec: "001-boundaries-and-authority", unit: "scripts/check-authored-content.sh", nature: corrective }
  # One test in 004's crate was repaired while this spec's round was measuring
  # against it: `settings_transport.rs`'s deadline attempt raced its own
  # subject. The repair is to the test's fixture and its assertions only, and
  # 004's required behavior is untouched, which is why the nature is
  # corrective. The edge is declared rather than left for the coupling gate to
  # discover, and the diagnosis is a dated entry in section 5.
  # Section 3.37 rule 2 adds one entry point there, `supervise_with_in`, which
  # writes the settings file into the attempt's exchange directory; additive,
  # and the edge keeps the nature it was first declared with.
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter-claude-code/" }, nature: corrective }
  # Section 3.30 rule 12's launch is supervised by the process-group supervisor
  # 004 already owns, so the raw capture it needs is added there rather than
  # written a second time here. Additive: no existing behavior of the
  # supervisor changes. The same section's rule 7 types three stream fields in
  # the claude-code crate above, which is also additive; that edge keeps the
  # nature of the repair it was first declared for.
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
# Not `depends_on`: 006, which depends on this spec. The realignment half needs
# 006's command tree and its exit-code vocabulary, and while that half was spec
# `010` it could say so without a cycle. Merged, the `extends` and `amends`
# edges above carry the relationship, and they name the unit and the nature,
# which `depends_on` never did. See the 2026-09-21 entry in section 5.
---

# 002: Project registration and the Statecraft-managed environment

## 1. Purpose

Two jobs that are usually conflated, separated here because they consent to
different things:

- **Registration** records that a repository is a target, with a read-only
  qualification verdict. It changes nothing inside the repository.
- **Environment installation** writes files into the repository so that agent
  sessions and this product's own checks have a working loop.

This product owns installation and ongoing maintenance of the working
environment. That is more than relocating templates: what makes it a product is
knowing which bytes it owns, what to do when one has changed underneath it, what
a pin means, and what removal is obliged to leave.

Sections 3.11 to 3.21 carry the realignment that made it the *sole* owner.
spec-spine withdrew its kit and its public initializer, so the transition
contract §3.7 once held has no second installer to coexist with, and this
product became the only user-facing initializer and environment manager. That
half fixes where the environment lives, globally and per project, where its
authority comes from, and what a producer at the boundary is allowed to hand
back.

## 2. Territory

`crates/statecraft-environment/` and `crates/statecraft-home/`.

Two crates and one spec. The split is the one the realignment draws: the
environment crate owns a *target repository's* managed bytes, and the home crate
owns the product's own global environment and the flow that initializes a
project. A change to what this product writes into someone else's repository and
a change to what it keeps in its own home are different blast radii, and keeping
them separate compilation units is what makes that visible.

The committed manifest at `.statecraft/environment.json` **in a target
repository** is this spec's format, not this repository's territory.

## 3. Behavior

### 3.1 Registration and qualification

`project register <path>` records an absolute path under the product's own home
and evaluates a **read-only** qualification. It writes nothing inside the target.

Qualification reports a verdict and its reasons, never a bare boolean:

| Verdict | Meaning |
|---|---|
| `qualified` | A git work tree, a resolvable base revision, and a spec-spine corpus that compiles. |
| `ungoverned` | A git work tree with no corpus. Visible, with reasons; never scheduled. |
| `unqualified` | Not a git work tree, or a corpus that does not compile. Visible, with reasons; never scheduled. |

A repository is **armed** separately from being registered. Arming consents to
being driven. It does not consent to an execution posture: that is `004`.

### 3.2 The three ownership classes

Every path inside a target is exactly one of:

- **managed**: written by this product, listed in the manifest with its source
  identity and content digest. The product may rewrite it on upgrade and must
  remove it on removal.
- **adopted**: it existed before, and the product depends on it but does not own
  it. Recorded in the manifest as adopted, with the digest observed at adoption.
  Never rewritten.
- **user**: everything else. Never read for control purposes, never moved, never
  rewritten.

The classes are **disjoint and exhaustive by construction**: a path the product
writes that is not in the manifest is a defect, and `doctor` reports it as
`unmanaged-write`.

### 3.3 The manifest

`.statecraft/environment.json`, committed. Runtime state lives under
`.statecraft/state/` and is gitignored; the manifest is not state, it is the
record of what this product owns in this repository.

Each entry carries: path; class (`managed` or `adopted`); the adapter or
template that is its source; the source identity; the SHA-256 digest and byte
length of the content written or observed; and the timestamp of the write.

The manifest header carries the **pins**: this product's version, the spec-spine
version the environment was installed against, and the adapter set. Pins are
recorded, never silently satisfied: a mismatch is a `doctor` finding.

### 3.4 Install, plan, upgrade

`env plan` prints what `env apply` would do and writes nothing. `env apply`
performs it. `env upgrade` re-plans against a newer product or adapter version.

Upgrade writes only a managed file whose on-disk digest **matches** its manifest
entry. For any other managed file the write is withheld and the file is reported.
The overall result is one of:

- `applied`: every planned write succeeded.
- `partial`: some writes were withheld; every withheld path is named with its
  reason. This is a success of the upgrade's contract, not a silent one.
- `refused`: a precondition failed and nothing was written.

An upgrade never resolves a conflict by choosing. Replacing a drifted managed
file requires the operator to say so per path.

### 3.5 Drift, and what `doctor` reports

`doctor` is the diagnostic surface and is read-only. Per manifest entry it
reports exactly one state:

| State | Meaning |
|---|---|
| `present` | On disk, digest matches. |
| `drifted` | On disk, digest differs. Left alone. |
| `missing` | In the manifest, absent on disk. |
| `foreign` | On disk and claimed by another installer, named by package identity where one exists (§3.7). Refused for management. |
| `shadowed` | Digest matches the manifest, but this is not the copy a session would resolve. The shadowing claimant is named (§3.7). |

Beyond entries, `doctor` reports pin mismatches, the adapters configured versus
available, and absent provider prerequisites. It never repairs. A diagnostic
that fixes what it measures cannot be trusted to measure it.

### 3.6 Removal

`env remove` deletes every `managed` path whose digest still matches, and no
other byte. A drifted managed path is reported and left: the operator changed it,
so it is theirs now. Adopted and user paths are never touched, and nothing is
restored to a remembered earlier state, because the product never recorded one.

Removal with no manifest present **refuses**. The alternative is guessing which
files were the product's, and a wrong guess deletes a user's work.

### 3.7 The transition contract with `spec-spine init --with-kit` (withdrawn)

**Withdrawn.** spec-spine removed the kit and the public initializer, so there is
no second installer to coexist with and the machinery would be permanent
scaffolding for an event that finished. Section 3.21 states the withdrawal, and
names the three parts of it that are **retained** because each is general rather
than kit-specific and each still protects a user-owned file: a `foreign` finding
names an owner and not only a path, `shadowed` stays because a digest match is
not evidence that a file is in force, and ownership transfer stays per path,
explicit, operator-initiated, reversible and recorded.

The number is kept rather than reused. Citations to §3.7 elsewhere in the corpus
resolve here and find the withdrawal, which is the answer they need.

### 3.8 User instructions are preserved

A pre-existing `AGENTS.md`, `CLAUDE.md`, `.cursorrules` or equivalent is **user**
class. It is never read for control purposes, never moved, never rewritten, and
never merged into.

Where an adapter needs a session-time entry point, the product writes its own
file at its own path and, at most, one pointer file, and only where no file
exists at that path. An existing file at a pointer path is reported as `foreign`
and the adapter reports itself degraded; it does not append.

This is the containment model Frame uses, and it is chosen **against** the
managed-section model (a tool owning a delimited region inside a file the user
also edits) that swamp uses. The choice, its reason and the prerequisite it
carries are `D-09` in the decision record: a pointer works only if the target
harness loads it, so "this harness loads a pointer at this path" is one of the
declared prerequisites in §3.9, and an adapter whose harness cannot **refuses to
claim its paths** rather than falling back to appending.

### 3.9 Adapters

An agent-harness adapter declares: the harness it targets; the exact set of paths
it would manage; the facts it cannot express in that harness; and the
prerequisites it needs present. Adapters are additive and independent, and two
adapters may not declare the same path.

An adapter whose harness is absent, or whose prerequisites are missing, **refuses
to claim its paths** and says which prerequisite is absent. It does not write
files for a harness that is not there.

### 3.10 Observable negative cases

Every row is required behavior. The first set belongs to registration and
the target repository's environment; the rest to the global home, the
project area and initialization.

| Case | Required behavior |
|---|---|
| `project register` on a path that is not a git work tree | Verdict `unqualified` with the reason; nothing written inside the path. |
| `project register` on a repository with no corpus | Verdict `ungoverned` with reasons; visible, never scheduled. |
| `env apply` where a planned managed path already exists and is not in the manifest | Path classed `foreign`, withheld, named; result `partial`. No overwrite. |
| `env upgrade` where a managed file has drifted | Write withheld, path named with digest expected and found; result `partial`. |
| `env remove` with no manifest | Refused, with the reason. No deletion. |
| `env remove` where a managed file has drifted | That file is reported and left; every matching file is removed. |
| Two adapters declaring the same path | Refused at plan time, naming both adapters and the path. |
| An adapter for an absent harness | Refuses to claim its paths, names the absent prerequisite, and does not write. |
| A pointer path already holding a user file | Classed `foreign`; the adapter reports degraded; the file is not appended to or replaced. |
| A path written by the product but absent from the manifest | `doctor` reports `unmanaged-write`. |
| A managed path whose digest matches but which a session would not resolve | `doctor` reports `shadowed` and names the claimant. A digest match alone is never reported as `present`. |
| `doctor` run against a drifted environment | Reports every state; exits non-zero on any `drifted`, `missing`, `foreign` or `unmanaged-write`; repairs nothing. |
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

### 3.11 One Statecraft-managed global environment

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
| `qualifications.json` | Provider qualification records. | `004` (which absorbed `008`), unchanged |

Three rules the shape has to keep:

1. **Extend, never replace.** This spec reads and writes only the files it owns.
   The register and the qualification records are read through their owners and
   are never rewritten here, so adopting this shape loses no existing record.
2. **An absent home is an empty home, not an error.** Every read-only verb
   answers with no home present, which is what makes a fresh machine usable
   before anything is installed.
3. **No credential lives here.** The home holds no provider credential and no
   platform token. Provider credentials stay where the provider keeps them
   (`004` section 3.14), and this product does not relocate them.

### 3.12 One per-project area

Inside a target repository, exactly four paths, and no others:

| Path | Committed | Holds |
|---|---|---|
| `.statecraft/environment.json` | yes | The environment declaration: pins, managed-file ownership, tracked modifications, and the project block of section 3.16. |
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

### 3.13 The root instruction bridge

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
and section 3.14 is what an adapter has to satisfy instead.

### 3.14 One global harness, delivered by adapters, copied into no repository

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

1. An adapter never repoints an agent home and never moves, reads or rewrites an
   authentication store. Unrelated native user configuration is preserved. A
   user's settings file is touched only under the **consented settings
   modification** of section 3.24, which is narrow, marked, reversible and
   refused by default; outside that, a delivery that would have to rewrite it is
   not performed.
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
path**, which is section 3.8 unchanged. Where the rule already reaches the
managed file, **nothing is injected**: a second copy of an import that native
loading already performs is a duplicate, not a belt and braces.

### 3.15 The governance producer boundary

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

### 3.16 Configuration authority

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

The project block carries one member another spec reads: `commands`, the
command allowance of spec `004` section 3.17, a list of bare program names.
It supplies a value (which programs a posture allows), not a rule; the
comparison is `004`'s.

**This is the precise reading of `006` section 3.6**, which says the binary
reads no configuration file that could change a rule an owning spec fixed. It
still does not. A layer supplies a **value**, never a rule: the rules stay in the
specs that own them, every supplied value carries the layer that supplied it into
the record, and a value no layer is allowed to supply is refused rather than
applied. The declaration and `home.json` are read as the target repository and
the product home, which section 3.16 of `006` already names as two of the three
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

### 3.17 Initialization, and the verbs

One user-facing initialization flow, spelled the way this binary already spells a
preview and a performance (`env plan` and `env apply`):

| Command | What it does |
|---|---|
| `home show` | The resolved global environment: paths, personal defaults, tools, harness revisions, and each adapter's delivery verdict. Reads only. |
| `home plan` | What `home apply` would write, inside the home and outside it. Writes nothing. |
| `home apply` | Creates or repairs the home and performs adapter delivery. The one operation that writes into a native agent location. |
| `init plan <path>` | Every project change initialization would make. Writes nothing. |
| `init apply <path>` | Performs it. |
| `migrate plan <path>` | The one-time relocation of section 3.19. Writes nothing. |
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
   (section 3.15).
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

### 3.18 Solo and team

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

### 3.19 The one-time relocation of the compiled artifacts

For this repository and for bounded fixtures, `.derived/` moves to
`.statecraft/derived/`, and the configuration, ignore rules, check surface, CI
and documents that name the old path move with it in the same change.

- If a repository holds **both** a non-empty `.derived/` and a non-empty
  `.statecraft/derived/`, the move is **refused**, naming both. It is never
  resolved by choosing one.
- The operation touches exactly one repository root. It sweeps no sibling
  repository and touches no real home directory.
- It is exposed as its own operation and is not a step of `init`.

### 3.20 The operation boundary a dashboard uses

Every operation in section 3.17 is a typed request and a typed outcome in this
spec's crate. The command surface is one caller of it. A local dashboard is a
second caller of the **same** operations: it implements no second settings
engine, no second scheduler and no second policy model.

A hosted platform and a local dashboard share operation semantics. The local
runner owns local processes; the platform coordinates team authority. **One run
has one execution owner**, and no background loop is added by this spec.

No user interface is implemented here. `F-04` stands, and the grade this spec may
claim about a dashboard is the typed boundary and nothing above it.

### 3.21 What replaces the kit transition contract

Section 3.7 is withdrawn as a transition contract: with the kit and the
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

### 3.22 The counterparty's state, and the order the last of its harness moves in

Prepared 2026-09-20 by a spec-spine session and handed to this product as input
to section 3.14. It is **evidence from a counterparty, not an instruction to this
corpus**: every disposition in it belongs to this product's owner, and nothing
here binds beyond what sections 3.13 to 3.15 already required. It is recorded in
the spec it informs rather than filed beside the corpus, because a design note
that lives outside the spec is a second place for a requirement to be written
down and the first place people stop reading.

**What spec-spine no longer does.** It removed the initializer and the kit in
one change:

| Gone | Was |
|---|---|
| `spec-spine init` | the project initializer |
| `--with-kit` | the harness installer |
| `kit/`, `kit_embedded.rs` | the harness, vendored and embedded in the binary |
| `.agents/`, `.codex/` | generated projections of that harness |
| `website/` | the documentation site |

No verb there writes an `AGENTS.md`, a `CLAUDE.md`, a `.claude/` directory, a
skill, an agent brief, a hook, an MCP configuration, a CI workflow or a
`Makefile`. What survived is section 3.15's producer seen from the other side:
`scaffold_init_json` is a pure function of its argument, emits governance starter
content only, as data, and its own tests assert that no emitted path begins with
`.claude/`. So the ownership classes of section 3.2 have no second claimant left
to negotiate with, and an adapter that manages `.claude/**` contends with
nothing.

**Where that repository's own `.claude/` stands**, measured 2026-09-20. It keeps
one for itself, because removing it before a replacement exists would leave it
with no development instruction and no hook enforcement, and it is being
dismantled in the order that never leaves it unprotected:

| Class | State |
|---|---|
| `.claude/rules/` (4 files) | **removed**, folded into that repository's `AGENTS.md` as a `## Rules` section. Never a candidate for a global home: they are its own governance and would bind every project a user opens. |
| the push gate | **installed globally** at `~/.claude/hooks/push-gate.sh`. Repository-agnostic: `git` and `jq`, and no spec-spine. Copied rather than moved, because its tests cannot read `$HOME`. |
| `.claude/skills/` (10), `.claude/agents/` (4) | waiting on this product. |
| `.claude/settings.json` (a PR gate, two session hooks, permissions) | waiting on this product. |

**The ordering is fixed, and it is the reason this section exists.** This product
delivers a global harness with a Claude Code adapter; spec-spine confirms that a
session there still has its loop and its hooks; **then** its `.claude/` goes,
with its governing spec superseded in the same change. Doing it in the other
order is the failure the removal of the kit was written to avoid, and no schedule
pressure converts one order into the other.

**The instruction bridge waits on this side by design.** spec-spine will not add
`@.statecraft/AGENTS.md` to its root `AGENTS.md` until this product's initializer
actually writes that file, because an import of a file that does not exist is a
broken instruction rather than an early one. Section 3.13 is the rule and the
initialization flow of section 3.17 writes the file, so the condition is
satisfiable today; what remains is telling the counterparty, which is one line on
its side and nothing on this one.

### 3.23 What delivered harness content must satisfy

Section 3.14 fixes the **mechanism**: one content-addressed source under the
home, adapters that point at it, delivery evaluated rather than assumed. This
section fixes what any **content** delivered through that mechanism has to
satisfy, whether this product authors it or adopts it from the counterparty.
Adopting the inventory below is the owner's act; what adoption costs is stated
here so the decision is not made by discovering the cost afterwards.

**The inventory offered, and adopted on 2026-09-21.** Ten skills (`prime`,
`next`, `build`, `verify`, `ship`, `shepherd`, `spec`, `commit`, `code-review`,
`setup`) and four agents (`architect`, `explorer`, `implementer`, `reviewer`).
They are already **repository-invariant**: every project-specific fact lives in
that project's `AGENTS.md`, which each skill ends by pointing at. That property
was built for a distribution that was then cancelled, and it is what makes
section 3.14's "maintained once, copied into no repository" viable for them
unchanged.

The owner adopted the whole inventory as Statecraft harness content, subject to
the repository-invariant project-layer boundary above. What is delivered is a
**Statecraft-namespaced equivalent** of each, which is section 3.14 rule 2 and
not a new condition. The four event behaviors are adopted with it:
`SessionStart`, `PostToolUse`, `PreToolUse` and `Stop`, including the push gate
and the pull-request gate and the assertions each carries. `Stop` is adopted
under the advisory policy below, not as a gate.

**Adoption is delivery, and delivery is not authorization.** A delivered skill
may be invoked; it acquires no standing permission by being delivered. Nothing
in this adoption lets a skill publish, merge, release or execute without the
authorization that operation independently requires, and a skill that reads as
though it did is a skill to fix rather than an authority to infer. The deny
floor below and section 3.14 rule 3's project gate both continue to apply to
every adopted name.

**The project layer keeps its facts.** Repository invariance is a condition on
the delivered content, checked rather than assumed: a generic skill does not
hardcode any project's crate layout, gate commands or workflow, and a project's
`AGENTS.md` stays the authority for those. That applies to the counterparty's
own repository as much as to any other, and an adopted file carrying
spec-spine's specifics is an adoption defect.

**Three assertions hold for a delivered skill**, wherever the file lands:

1. No skill names a gate flag its project's `AGENTS.md` omits.
2. A read-only skill never invokes a writing verb.
3. Each skill wraps the tool verbs it exists for, rather than restating them.

**Seven contracts hold for a delivered hook.** Each was written after a measured
failure, and the contract is worth more than the shell that carries it:

1. **Read, never repair.** No hook may invoke a writing subcommand. A hook fires
   where it cannot commit what it regenerated, so a writing hook leaves the
   derived tree dirty; an orchestrator that refuses to start on a dirty tree then
   never starts, and one adopter's pipeline stalled eleven hours on dirt it had
   produced itself. The single sanctioned exception is a `compile` after an edit
   to a `spec.md`, where the session is live and can commit the result.
2. **Resolve the binary in order**: `$SPEC_SPINE_BIN`, then the target
   repository's own `target/release/spec-spine`, then `PATH`. A repository that
   builds its own binary must be governed by the one it builds; the `PATH`
   fallback keeps an adopter on the published CLI working. A bare name resolved
   from `PATH` alone is whichever copy the last unrelated project installed.
3. **Resolve the target repository from the command, not from the session.** A
   multi-repository session pushes and edits in whichever tree the command names.
4. **Read the verdict; never guess it.** `check` has four answers and they are
   not interchangeable: `0` fresh, `1` a corpus that does not validate, `2` stale
   or an unresolved claim, `3` a read that was not performed. Only one of the
   four is repaired by regenerating.
5. **Establish the verb before reading its exit code.** `clap` also spends `2` on
   an unknown subcommand, so a hook confirms the binary carries the verb
   (`check --help`) first. Without that, a binary older than the verb reports a
   fresh tree as stale and sends the session to regenerate shards that were
   already correct.
6. **A gate whose check did not run is not green.** Every non-zero code refuses.
7. **A branch gate resolves the protected branch rather than assuming `main`**:
   `$SPEC_SPINE_DEFAULT_BRANCH`, then the remote's own `HEAD`, then `main` as a
   floor. It refuses only a push that would actually update that branch, so a tag
   push from the default branch is allowed, and it is anchored on the command
   that invokes the verb, so a `grep` or a heredoc merely containing the text
   still runs. A pull-request gate runs the coupling gate before the create verb
   and refuses without a human-written waiver line in the body.

**An end-of-turn hook advises; an operation gate enforces.** Contract 6 above
is a rule about **gates**, and a harness's end-of-turn event is not one. The
owner settled the policy on 2026-09-21 and it has three parts:

1. **Stop is advisory, and reports the result accurately.** It says what the
   check answered, including that the check could not be performed. It does not
   convert that answer into a block.
2. **An enforcing operation gate refuses a failed or unavailable check.** A push
   gate, a pull-request gate and any other gate standing in front of an
   operation refuse on every non-zero code and on a check that did not run,
   which is contract 6 unchanged.
3. **Stop never prevents a useful failure handback.** A session that has
   something to report, including a failure, hands it back. A stale derived tree
   or a read that was not performed is a fact the handback carries, never a
   reason to withhold it. The asymmetry is the point: a gate that wrongly
   refuses costs an operation that can be retried, and an end-of-turn block that
   wrongly fires costs the report of why the work failed, which is the thing
   nobody can reconstruct afterwards.

This changes no hook's repair behavior. Contract 1 stands exactly as written:
read, never repair, with the single sanctioned exception of a `compile` after an
edit to a `spec.md`, where the session is live and can commit the result. No
part of the Stop policy authorizes a hook to write.

**Two exit vocabularies, and no numeric passthrough between them.** Contract 4
reads spec-spine's codes. Spec `006` §3.3 fixes this product's own. They are
different closed vocabularies that share the integers, and three of the four
overlapping values disagree:

| Code | spec-spine `check` | Statecraft command (`006` §3.3) |
|---|---|---|
| 0 | fresh | did what was asked, found nothing wrong |
| 1 | the corpus does not validate | a finding: a diagnostic state, a withheld write |
| 2 | stale, or an unresolved claim | refused: a precondition was not met |
| 3 | the read was not performed | usage error: no such operation |
| 4 | not used | failed |

Propagating a spec-spine code as a Statecraft code would report a stale tree as
a refusal and an unperformed read as a usage error. Where this product runs a
governance verb and answers in its own vocabulary, it **translates**:

| spec-spine `check` answered | this product reports |
|---|---|
| 0 fresh | 0 |
| 1 the corpus does not validate | 1, a finding: the check ran and the corpus is what it found |
| 2 stale, or an unresolved claim | 1, a finding, and the two readings are distinguished in the text, not in the code |
| 3 the read was not performed | 4, a failure: nothing about the corpus was established |
| the binary is absent, or lacks the verb | 2, a refusal: a precondition was not met and nothing was done |

An enforcing gate collapses the same answers to green and not-green, where only
`0` is green. That is contract 6, and it is not a different translation: it is
this one, read by something that has only two outcomes to spend.

**A deny list is a safety floor, not an adapter's optional extra.** Where a
delivery carries permissions at all, the refusals travel with it: no publish
verb, no release verb, no force push, no recursive removal of a corpus or a
derived tree.

**Whoever owns the files owns the assertions.** In spec-spine the three skill
assertions and the seven hook contracts are enforced by
`crates/spec-spine-core/tests/harness_hooks.rs` (1124 lines, which extracts each
hook body and runs it as a program over a matrix of command spellings and branch
names) and `harness_skills.rs` (about 800). A hermetic test cannot read `$HOME`,
so neither file survives the move on its own: they are reimplemented where the
files land, or the requirements become unenforced. That cost is small and it is
not optional, and it is the second reason section 3.22's ordering is not
negotiable.

The harness this build ships under section 3.14 was deliberately small, and the
adoption above does not change the judgment behind it: the point of a global
harness is that it is one source, not that it is a large one. What grew is the
inventory the owner decided to carry, and every added file still pays the same
price, which is the assertion that judges it.

### 3.24 The consented settings modification

Section 3.14 rule 1 admits exactly one write into a harness's own settings file.
It is the narrowest thing that lets a hook the harness ships actually fire, and
everything about it is shaped so that a user who never consents is in the same
position as before this section existed.

**What it may carry, and nothing else.** Two kinds of line:

1. A **hook registration** whose command resolves inside the canonical harness
   under the product home. Never a command assembled from anything else.
2. A **deny entry**, which is a refusal. Where such an entry lands is fixed by
   section 3.27: not in the user's global deny list by default, because a deny
   entry carries no project gate and acquires none from the scripts registered
   beside it.

A merge may add a refusal. It may never add or widen a permission: no allow
entry, no `ask` downgraded, no existing deny removed, weakened or reordered. The
deny list travels as a floor (section 3.23), and a floor that a delivery can
lower is not one. `settings.local.json` is the user's own override layer and is
never written at all.

**Consent is a separate act from installation.** The modification is named in the
`home apply` plan before anything is written, in the exact lines it would add,
and is **refused by default**: installing skills and agents does not perform it,
and neither does any read, test or project operation. It is performed only when
the operator consents to that modification specifically. Consent to one revision
is not consent to the next: a modification whose lines have changed is presented
again.

**One recorded modification, several valid locations.** All Statecraft-managed
insertions are tracked by one recorded modification. They may occupy multiple
syntactically valid locations. Each insertion is identified by its exact
content, structural location, and recorded provenance. Unrelated bytes are
preserved, and removal occurs only when Statecraft can establish that the
content is an intact insertion it owns.

That is the contract the owner approved on 2026-09-21, replacing a requirement
that every managed line occupy one physically contiguous marked region. The
replacement is about **representation only**. What the region requirement was
carrying is not physical adjacency but attributability, and attributability is
what the three identifying properties above supply: the same bytes, in the same
place in the document's structure, with a record in the home that says this
product put them there. Adjacency was one way to get it, and in a syntax with
two non-adjacent insertion points it is not an available one. Section 5's
2026-09-21 entries measure why.

The record itself is unchanged: the home records **one modification** (path, the
exact content, the digest before and the digest after) the way section 3.13
records the root instruction bridge, never as a managed entry and never as
ownership of the file. Outside the recorded insertions nothing is rewritten,
reordered or reformatted, and the file's own shape is preserved.

**The file stays strict JSON.** The representation this section permits is the
one the harness's own parser already accepts. It does not extend to JSON5, to
comments outside string values, to a deny entry that refuses nothing and exists
only to mark a boundary, or to a metadata key the harness does not support. A
marker this product needs travels inside content the harness already reads as
content, or it does not travel.

**Reversible, and only while ownership can be established.** Removal removes
exactly the insertions this product can establish it owns, and nothing else. An
insertion whose content no longer matches what was recorded is **reported and
left**: an edited insertion is a user's file again, and this product does not
take it back. Where ownership cannot be established, the insertion stays and
the outcome says which one and why; losing the evidence costs a refusal
nothing. A marker resembling this product's is not by itself proof of
ownership, and a pre-existing user refusal is never claimed as a managed
insertion. Applying the modification twice changes nothing.

**A conflict is named, not resolved.** Where the user already registers a hook on
the same event with a different command, both remain and the situation is
reported. This product does not decide which of two hooks a user wants.

**What this does not become.** It is not a general settings manager, not a
migration, and not a path to any other key. A settings key this section does not
name is not writable by any code path, and adding one is an amendment to this
section rather than a use of it.

### 3.25 The required harness identity and the resolved one

Section 3.14 makes a harness revision content addressed. This section says who
records which revision, and what a managed session does when the two answers
disagree. The owner settled it on 2026-09-21.

**Two records, and they are not the same record.**

- The **required** identity is a **committed project requirement**. It travels
  with the repository, it is reviewed like any other committed change, and it
  is what the project says its managed sessions must run against.
- The **resolved** identity is recorded **per managed session**, separately,
  and says which revision actually answered. Section 3.16's last rule already
  freezes it; this section fixes that the requirement it was resolving against
  is a committed one rather than whatever the home happened to hold.

Keeping them apart is what makes disagreement visible. One record that is
rewritten as it is read cannot disagree with anything, which is precisely the
failure this section refuses.

**The full digest is the integrity proof.** A revision's identity is the digest
over its files, as section 3.14 fixes it. The **full** digest is what the
required record carries and what an integrity comparison uses. A short
identifier derived from it is a **display** convenience: legible in a plan, in a
verdict and in a log, and never on its own the thing an equality check is
performed against. A truncated identifier that two revisions could share is not
a proof, whatever the odds are.

**Managed execution refuses.** A session that would be managed under a required
identity refuses when the required content is **missing** (no such revision is
installed), **corrupt** (a revision is installed under that identity and its
files no longer digest to it), or **mismatched** (the resolved revision is not
the required one). The refusal is a refusal in this product's vocabulary: a
precondition was not met and nothing was done.

**Four things stay possible under that refusal**, because a refusal that
prevents diagnosis is worse than the state it refuses: **inspection** (what is
required, what is installed, what each digests to), **diagnosis** (`doctor`
reporting the disagreement), **planning** (what an apply or an upgrade would
do), and an **explicit upgrade**.

**Two things this product never does.** It never **silently selects the latest
installed revision**: an installed revision that is not the required one is a
mismatch to be reported, not a substitute to be chosen, and "the newest one is
probably right" is how a project loses the ability to say what it ran. And it
never **rewrites the project requirement during a read**: inspection, doctor
and plan are reads, and a read that repairs its own precondition destroys the
evidence that the precondition was unmet, which is the same defect the gate
refuses in `compile` (AGENTS.md, "New sessions").

**An upgrade is an explicit reviewed project change.** Changing the required
identity is a committed change to the repository, proposed and reviewed like
one. It is never a side effect of installing, of running, or of a newer
revision appearing under the home.

**An upgrade renews settings consent when the content changes.** Where the
consented settings modification of section 3.24 embeds the revision, a new
required identity produces different modification content, and different
content is presented for consent again. That is section 3.24's rule
("consent to one revision is not consent to the next") reached from the other
direction, and it is stated here so an upgrade path cannot satisfy itself by
reusing a consent given for other bytes. Where the modification content is
byte-identical across the upgrade, there is nothing new to consent to and
nothing is asked.

### 3.26 Startup delivery evidence

Section 3.14 fixed a three-valued verdict over a harness's documented load
rule. This section fixes what a managed session **records** about delivery, and
what that record is and is not allowed to claim. The owner settled it on
2026-09-21.

**What is recorded.** Seven things, at the start of a managed session:

| Recorded | What it is |
|---|---|
| project identity | which repository, and the manifest that makes it a target |
| instruction-file identities | each instruction file reached, by path and digest |
| required harness identity | the committed requirement of section 3.25, full digest |
| resolved harness identity | what actually answered, full digest |
| adapter identity | which adapter performed the delivery, and its own identity |
| load chain | the files traversed, entry first, managed file last |
| delivery status | the section 3.14 verdict |

**Three statements, kept distinct and never substituted for one another.**

1. **The documented load chain reaches a file.** A rule was evaluated against
   the tree and arrives. This is a statement about the tree and the rule.
2. **Its bytes were resolved and supplied.** The file was read, it digests to
   what is recorded, and its content was handed to the session. This is a
   statement about what this product did.
3. **A live session demonstrated the expected behavior.** A real session was
   observed behaving as the instructions require. This is a statement about a
   session, and only a session can produce it.

Each is strictly weaker evidence for the next, and none of them implies the
one after it. A record that states the first must not be read as the second,
and a record that states the second must not be read as the third.

**What a digest never proves.** A digest establishes that bytes are the bytes.
An acknowledgement establishes that something emitted an acknowledgement.
Neither establishes that a model **read**, **understood** or **complied with**
the instructions, and no field in this record makes that claim. The three
words are listed because each is a distinct overclaim and all three are easy to
write by accident.

**The three verdicts keep their meanings.** `reached`, `not-reached` and
`unverified` mean exactly what section 3.14's table says. The new evidence is
carried in **added fields**, narrowly defined, alongside the verdict. In
particular `reached` is not quietly widened to mean supplied, and is not
narrowed to require a live observation: a rule that arrives is a rule that
arrives, and the fact that this is weaker than what an operator often wants is
the reason it is reported separately rather than merged into a single richer
word. Redefining an existing verdict changes the meaning of every record
already written under it, which is a migration and not an improvement.

### 3.27 Global adapter registration is not managed-session permission delivery

A narrow amendment, settled by the owner on 2026-09-21 and recorded before the
implementation it authorizes. It separates two things this build had joined:
registering an adapter **globally**, and delivering the deny floor to a
**managed session**.

**Section 3.14 rule 3 is unchanged and is the reason.** Every delivered
behavior applies only inside a repository holding
`.statecraft/environment.json`, and is inert everywhere else. An unrelated
project is unaffected by global integration.

**The floor does not go into the user's global deny list by default.** A
refusal written into the user-global `permissions.deny` applies to every
session that user runs, in every repository, managed or not. Hook *scripts* can
be project-gated because a script can test for the manifest and exit; a deny
entry is evaluated by the harness before anything of this product's runs, so it
carries no gate and acquires none from the scripts shipped beside it. Those
entries are therefore **not project-gated merely because the hook scripts
are**, and this product does not install them there by default.

**The floor is delivered through a supported managed-session settings
mechanism**, carrying the canonical Statecraft content. Claude Code documents a
`--settings` argument for exactly this shape of need. Documentation is not
evidence: the **installed version** and the **effective behavior** are verified
before this product relies on the mechanism, and an unverified mechanism is an
unavailable one.

**Three conditions on whatever global registration remains.** A global hook
registration, if used at all, is **inert outside a Statecraft project**.
Delivered skills and agents stay **namespaced and project-gated**, which is
section 3.14 rules 2 and 3. And consent asked for a write into a global native
settings file **describes the actual scope of that write**: a modification that
takes effect in every repository is presented as one, whatever the scripts it
registers do afterwards.

**The floor is never weakened to solve a delivery problem.** Section 3.23's
refusals are a floor, and a floor a delivery can lower is not one. Where an
ordinary unmanaged session cannot receive the scoped floor, the answer is to
**report that session as not qualified** for the managed-execution claim.
Two repairs are specifically refused: lowering the floor so that delivery
succeeds, and writing a repository-local generic harness copy so the
limitation stops being visible. The second would also reintroduce exactly what
section 3.14 removes.

### 3.28 An unmarked registration is the user's, and resemblance is not ownership

Settled by the owner on 2026-09-21. Section 3.24 already says that a conflict
is named rather than resolved; this section says what "named rather than
resolved" means for a registration that already exists, and closes the one
route by which this product could take one over by accident.

**Existing registrations are preserved.** The user's own hook registrations
stay, including the global push gate §3.22 records at
`~/.claude/hooks/push-gate.sh`. Coexistence is the behavior to exercise:
this product's delivery runs alongside them and is tested doing so. Where a
later replacement is wanted, it is prepared as an **exact plan** and performed
as its own reviewed act, never folded into a delivery.

**Resemblance is never ownership.** This product does not delete, take
ownership of, or rewrite an **unmarked** registration, and it does so least of
all when the registration looks like content this product ships. A user who
copied a shipped hook, or wrote one that converged on the same commands, owns
what they wrote. Ownership is established the way section 3.24 establishes it,
by exact content, structural location and recorded provenance together; a
marker resembling this product's satisfies one of the three and proves
nothing.

**The keys this product does not touch.** `defaultMode`, `allow` entries, `ask`
entries, model selection, and every other user setting section 3.24 does not
name. That section's closing rule is the general form: a key it does not name
is not writable by any code path. This section names the ones an integration
is most tempted by, because each of them would make a delivery smoother and
each is the user's decision.

**Qualification is measured, not read off the configuration.** Where this
product judges whether a session is qualified, it inspects **effective
behavior**. A deny entry present in a settings file establishes that the entry
is configured; it does not establish that it was enforced, in this version, in
this session, for this command. The two are different claims and only the
second qualifies a session.

### 3.29 What admits a live observation

A narrowly scoped authority amendment, settled by the owner on 2026-09-21 and
recorded before the implementation it authorizes. Section 3.26 fixed that a
live observation is the third evidence class and that only a session produces
one. Section 3.28 fixed that qualification is measured rather than read off the
configuration. This section fixes **what a claimed observation has to carry**
before it is admitted as one, because the first implementation of that
admission read the provider's prose and a sentence saying the opposite of a
refusal satisfied it.

**The defect this closes, exactly.** The admission accepted a transcript that
contained the command text together with any of the words `permission`,
`blocked`, `refus`, `denied` or `not allowed`. The transcript
`cargo publish --dry-run: permission granted; command executed successfully`
satisfies that predicate, and was admitted as evidence that the command had
been refused. The repair is not a longer word list and not a more elaborate
reading of natural language: a claim about enforcement is admitted from
**structured evidence the harness itself emits**, or it is not admitted.

**Rule 1: provider prose alone cannot establish enforced refusal.** Text a
model emitted is a statement by the model. A refusal that survives as evidence
is a structured record the harness produced, of the kind spec `004` section 3.4
requires a refusal-bearing restriction to produce, naming the tool, the
tool-use id and the tool input verbatim. Where the supported adapter exposes
that record, it is what the admission reads. Prose may accompany it and is
never what is read.

**Rule 2: the controls are part of the evidence, not part of a procedure.**
An observation qualifies only when all three of the following are present and
each one succeeded:

| Control | What it is | Why it is required |
|---|---|---|
| the refusal | the claimed command, run with the managed-session payload | the observation itself |
| the allowed command | a command no floor entry claims, run with the same payload | without it the refusal is consistent with a payload that refuses everything |
| the absent payload | the same claimed command, run with no payload | without it the refusal is evidence for the operator's own configuration rather than for this payload |

These are conditions on the evidence a qualification boundary accepts. A
document that describes them, a checklist that recommends them, or an operator
who remembers them is not what this rule means: the boundary that admits the
observation refuses one whose controls are absent or whose controls did not
behave as the table says.

**Rule 3: missing, contradictory, substituted or mismatched evidence refuses
qualification.** A capture that is empty, that does not parse as the harness's
own structured output, that reaches no terminal event, that carries no refusal
record for the claimed command, or that carries a refusal for the allowed
command, refuses the claim. Two controls presenting the same captured bytes is
substituted evidence and refuses the claim, because one capture cannot be two
measurements.

**Rule 4: evidence for another invocation or settings payload cannot qualify
this one.** The observation is bound to the invocation that produced it: the
program and arguments as spawned, the working directory, the settings payload
by digest, and the harness version the capture itself reports. A payload digest
that is not this build's, a version that disagrees with the capture, a refusal
control whose invocation does not carry the payload, or an absent-payload
control whose invocation does carry one, all refuse the claim.

*Amended by section 3.37 rule 4:* for a confined launch, the program and arguments are the provider's as handed to the confinement; the confinement is bound apart from them, by its mechanism and profile or ruleset digest.

**Rule 5: the same rules govern every route.** Construction, deserialization,
and conversion from any other qualification type reach an admitted observation
only through these rules. A record read back from a file is re-checked against
them before it is treated as qualified, so writing the word into a file by hand
is not a weaker route to the same claim; it is not a route at all.

**Rule 6: unverified is the answer when the evidence cannot decide.** Where the
supported adapter cannot expose evidence sufficient to distinguish a permission
refusal from a model's statement about one, the result stays unverified and the
session is reported as not qualified. Section 3.27's last paragraph already
refuses the two repairs that would hide this. This section adds the third: the
admission is not loosened so that a claim succeeds. A truthful inability to
qualify is the correct outcome, and an invented proof is not an outcome at all.

**What is preserved.** The original captured bytes and their provenance are
kept with the record, not summarized into it. The admission is reviewable
because what it was made from is kept and can be re-read, which is the property
section 3.26 already relies on and which rules 3 and 5 now depend on.

### 3.30 What each control must demonstrate, and what binds a capture to its launch

A narrowly scoped authority amendment, settled by the owner on 2026-09-22 and
recorded before the implementation it authorizes. It sharpens section 3.29
rules 2 to 4. Rules 1, 5 and 6 are unchanged and govern everything below.

**The defects this closes, exactly.** The admission written under section 3.29
read each capture's assistant turns for tool-use **requests** and treated a
request with no matching denial entry as a control that ran. A request is not
an execution, and the absence of a denial entry is not the presence of a
result. It matched a denial to a command by the command text alone, so a
denial on another tool, or on another tool use, carrying the same text
satisfied it. It kept only the last init event and the last terminal event, so
a capture holding two sessions, or two disagreeing terminal events, was read as
one. And it bound an invocation to its settings by checking that a
`--settings` token appeared somewhere in a caller-authored argument list and,
separately, that caller-supplied bytes digested to the payload. Neither check
relates the argument to the bytes, and the argument list was written by the
same caller after the fact.

**Rule 7: a control is judged from correlated structured events.** Everything
below is read through the supported adapter's own types (spec `004` section
3.9), extended there where a field it carries was not yet typed, and never
through a second parser in this spec's crate. The fields read are the ones the
recorded Claude Code `2.1.267` streams under
`crates/statecraft-adapter-claude-code/testdata/stream/` carry: the session id
on every event; the init event's version and working directory; each assistant
`tool_use` block's id, tool name and input; each `tool_result` block's
tool-use id, error flag and content, with the carrying event's
`tool_result_meta` and its `non_execution_kind`; the mid-stream
`permission_denied` event's tool-use id, tool name and decision reason; and the
terminal event's `permission_denials`. A field the provider does not emit is
not required, and no fixture invents one.

**Rule 8: a capture is one session, read whole.** A capture is admissible only
when it holds exactly one init event and exactly one terminal event, the
terminal event is its last event, no turn precedes the init event, every
system, assistant, user and terminal event names the same session, tool-use ids
are unique, every tool result names a tool use earlier in the same capture and
no tool use has two results, every denial entry and every mid-stream denial
names a tool use in the same capture whose tool name agrees, every denial
entry's input is that tool use's input verbatim, and the init event's working
directory is the directory the launch recorded. Anything else is conflicting,
duplicated, mixed-session or incomplete evidence and refuses the claim.

**Rule 9: each tool use is classified, not inferred.** The governed tool is the
one the floor's entries name, `Bash`, and a command is its input's `command`
string compared exactly. For one tool use:

| Classification | What must be present |
|---|---|
| refused | a terminal denial entry for its id, and its tool result marked as not executed by `non_execution_kind` |
| executed | a tool result for its id, no `non_execution_kind` on it, no denial entry and no mid-stream denial for its id |
| unresolved | anything else: a request with no result, a result marked not executed with no denial behind it, or a denial whose result is not marked |

An unresolved use refuses the claim. A request with the expected command text
under another tool's name is not a use of the governed tool, and a denial on it
proves nothing about the floor.

**Rule 10: what each control proves.** Section 3.29 rule 2's table, stated as
outcomes:

| Control | Required outcome | Not required |
|---|---|---|
| the refusal | at least one governed use of the claimed command, and every such use **refused** | anything about the command's own exit |
| the allowed command | at least one governed use of the allowed command, every such use **executed**, and at least one whose result is not an error and whose content, trailing line breaks removed, is exactly the expected output | |
| the absent payload | at least one governed use of the claimed command, and every such use **executed** | the command succeeding |

A control whose capture carries any other tool use refuses the claim, because
evidence about a session that did something else is not evidence about this
one. **Permission success and command success are different facts.** The
absent-payload control proves that the claimed command reached execution with
no permission refusal, under the same grant and without the payload. The
command is chosen so that it fails harmlessly once it runs, and it is expected
to: `cargo publish --dry-run --manifest-path statecraft-absent/Cargo.toml` names
a manifest path that does not exist, so cargo stops before resolving anything,
searches no ancestor directory, and contacts no registry. A result flagged as an
error there is the command failing, which is what it was chosen to do, and it
is not a refusal unless the harness marks it as one.

A denied command that also shows evidence of executing is not a refusal. Two
uses of the claimed command in the refusal control, one refused and one
executed, refuse the claim.

**Rule 11: terminal and process conditions are judged per observation, and
kept.** A control is measurable only when the process ended by itself inside
its deadline, no signal ended it, nothing in its process group outlived it,
its exit code is `0` or `1`, and that code agrees with the terminal event's
error flag the way every recorded stream agrees (`0` with `is_error: false`,
`1` with `is_error: true`). Its terminal reason is `completed` or `max_turns`.
The turn cap is admitted because the measurement shows the tool result arriving
before the capped terminal event (`max-turns.jsonl`, `--max-turns 1`), so the
cap ends a session whose control has already produced its evidence. An
`api_error`, a terminal state spec `004` section 3.13 does not map, a timeout, a
signal or a survivor leaves the control unmeasured and refuses the claim. The
terminal state and the process end are preserved in the record either way.

**Rule 12: the launch is the evidence of the invocation.** Section 3.29 rule 4's
binding is made by the operation that launches the process, not by a
description written afterwards. This product's own launch (spec `006` section
3.11.2) constructs the argument vector, writes the payload to a settings file,
resolves and digests the executable, reads its version, supervises the process
in its own process group under a deadline, and records together: the executable
as requested and as resolved, with its digest; the version it reported; the
argument vector and working directory; the prompt, which travels on standard
input and never in an argument; the settings path, the exact settings bytes, and
their digest before the launch and after the process ended; standard output and
standard error, separately and verbatim; the exit code, signal, timeout and
survivors; a capture identity; and the control it is.

The admission recomputes the argument vector this build constructs for the
recorded control, commands and settings path, and requires it **exactly**. So
a payload control carries one `--settings` argument naming the recorded file,
and the absent-payload control carries none in either spelling, `--settings
<path>` or `--settings=<path>`; a second settings argument, a substituted path,
a reordered or additional argument, or bytes that changed while the process ran
refuse the claim. Because the prompt is not an argument, no text in it can be
read as an option. Three controls are three launches: their capture
identities, their sessions and their tool-use ids are pairwise distinct, and
two controls naming one session are substituted evidence however differently
their bytes are formatted.

**The trust boundary.** A launch record is **launcher-attested**. It
establishes that this product started this executable with these arguments,
this working directory and these settings bytes, and received these bytes back.
It does not establish that the provider **loaded** the settings: that is
inferred from behavior, which is what the three controls are for. It does not
authenticate its own origin either. A record edited by hand after the fact and
still consistent is admitted, because nothing here signs it, and no field and
no rendering claims cryptographic provenance. What makes it reviewable is that
the bytes are kept, which is section 3.29's closing paragraph.

**Synthetic captures stay synthetic.** A capture launched against a fake
provider is marked `synthetic` by the launching operation, at the operator's
explicit request. Synthetic evidence runs the whole admission, so the path is
testable end to end, and an observation admitted from any synthetic control is
**never** a live observation: the record does not qualify and every rendering
says synthetic. The mark prevents this product's own fixtures from being
presented as earned; it does not detect a forgery, which the paragraph above
already disclaims.

**Records written before this section.** They carry no launch record. They
still deserialize, their bytes and provenance stay in the file untouched, and
the admission refuses them for the missing launch, so they read as not
qualified. No record is rewritten, migrated or deleted.

**The experiment these rules judge.** The acceptance script is the operator's
route, and its contract is part of this amendment:

- **Commands.** Refused: `cargo publish --dry-run --manifest-path
  statecraft-absent/Cargo.toml`, which the floor's `Bash(cargo publish*)`
  claims. Allowed: `echo statecraft-allowed-control`, whose expected output is
  `statecraft-allowed-control` and which no floor entry claims.
- **One grant, identical in all three.** Every launch carries
  `--allowedTools` naming exactly those two commands. Without a grant a
  non-interactive session refuses an unapproved command whatever the payload
  says, so the absent-payload control could never show execution and the
  refusal could not be attributed to the payload. With the grant identical,
  the payload is the only difference between the refusal and the absent-payload
  launches. That the provider's deny entry prevails over the grant is a premise
  the refusal control **tests**: if it does not, the refusal control records an
  execution and the claim is refused. The payload itself still carries no allow
  entry, which is section 3.24's and section 3.28's rule and is untouched.
- **Turns.** `--max-turns 1`, for rule 11's measured reason.
- **Sessions.** At most **three** provider sessions: the refusal, the allowed
  command and the absent payload, in that order. The first control whose launch
  does not complete ends the experiment, and no later session is started. There
  is no retry, no replay after an uncertain outcome, and no conditional extra
  session. Each launch also runs the provider's `--version` once, which is a
  version probe and not a session.
- **Bounds.** Each session has its own deadline (default 300 seconds), each
  version probe 30 seconds, so the whole stage is bounded by three sessions and
  three probes.
- **Approvals.** The provider stage refuses unless
  `APPROVED_PROVIDER_SESSION=yes`; the real-home coexistence stage refuses unless
  `APPROVED_REAL_HOME_COEXISTENCE=yes`. Neither implies the other.
- **The local test route.** `SC_ACCEPTANCE_FAKE_PROVIDER=<path>` runs the
  provider stage's whole control flow against a local executable instead of the
  provider. It never reads the provider approval, refuses to run when that
  approval is also set, marks every capture synthetic, and reports its result as
  synthetic.

*Amended by section 3.37 rule 4:* for a confined launch, the argument vector recomputed and required exactly is the provider's as handed to the confinement; the wrapper's own arguments are not part of it, and the confinement is recorded and bound apart.

### 3.31 The startup record a run writes, and the harness revision that answered

A narrowly scoped authority amendment, settled by the owner on 2026-09-22 and
recorded before the implementation it authorizes. Section 3.26 fixed what a
managed session records at its start, and section 3.25 fixed that the resolved
identity is recorded per session and says which revision actually answered.
Neither was true of `run`: it wrote no startup record at all, and the
`startup record` and `startup qualify` verbs filled the resolved identity in
with the **required** one, so a revision nobody measured read as `exact` and
resolved. This section fixes when a run's record is written, what binds it to
the attempt, what measures the answering revision, and what each grade of that
measurement establishes.

**The defect this closes, exactly.** A required revision is not an observed
one, and a verified directory on disk is not proof that the launched session
used it. Recording the requirement as the resolution is the substitution
section 3.26 forbids between its first and third statements, applied to the
harness instead of to the instructions.

**Rule 13: a run's startup evidence is two write-once records per attempt,
under the attempt's identity.** Both live under the project's ignored runtime
state, at `.statecraft/state/startup/runs/<run>/<attempt>/`, where `<run>` and
`<attempt>` are the run id and attempt number spec `003` section 3.4 assigned.
*Amended by section 3.37:* the directory is now in the product home, and the
gate's files are in a separate exchange directory.

| Record | Written | Holds |
|---|---|---|
| `intent.json` | after the attempt's intent is appended and every preflight has passed, **before** the process is created | the attempt identity; the project identity; the workspace the session starts in; the load chain and instruction-file identities evaluated **in that workspace**; the required identity; the standing before launch; the **selected** revision; the adapter identity; the resolved program; the payload's digest, length and argument; and a fresh binding nonce |
| `record.json` | after the process ended, or after the launch failed | the section 3.26 record: its seven fields and the added evidence, plus the attempt identity, the digest of the `intent.json` bytes it finalizes, the provider's session id and version as its stream reported them, the process outcome, the settings bytes the adapter actually wrote, and the **observed** revision with the grade of its evidence |

Neither is ever rewritten. A record already present under the attempt's
identity refuses the write, so a previous attempt's evidence is never
overwritten and never adopted by a later attempt. `record.json` names the
digest of the intent it finalizes, so the pair is judged together and an intent
changed after the fact no longer binds.

**Rule 14: preflight refusal and a launch are different facts.** A run refused
before its attempt is appended (section 3.25's refusals) writes neither record.
An attempt concluded `refused` before a process was created (the adapter's
preflight, an unresolvable executable) writes neither record, and its refusal
stays in the attempt record where spec `003` puts it. `intent.json` exists if
and only if this product was about to create the process. A record read without
its intent is not evidence of a launch.

**Rule 15: a recording failure stops or reports, never proceeds silently.** If
`intent.json` cannot be written, nothing is launched: the attempt is concluded
`refused` with the guard `startup-record` and the reason. If `record.json`
cannot be written after the process ended, the attempt is concluded with its
real outcome, its detail says the record was not stored and why, and `run`
exits 4, spec `006` section 3.10's failed row. If this product's own process
ends between the two writes, the intent stays and no record is fabricated, and
reconciliation (spec `003` section 3.6) owns the attempt record. *Amended by
section 3.32 rules 22 and 23:* this rule first said such an attempt reads as
launched and interrupted, and an intent does not establish either.

**Rule 16: supply is recorded by the launch that performs it.** In a run, the
bytes this product hands to the session are two things, and each is recorded
by the operation that hands it. The **instruction chain** is in the workspace
this product prepared and starts the session in: the launch reads each file the
load rule reaches in that workspace immediately before the spawn and records
its digest. The **settings payload** is the document the adapter writes: the
adapter reports the exact bytes it wrote to the settings file it passes, and the
record carries their digest. The supply is `supplied` only when the process was
created, the chain reached the managed file, and the written bytes digest to
the payload this build records; a spawn that failed is `failed`; a chain that
does not arrive is `not-attempted`. Supply established this way says the bytes
were in the session's working tree and on its command line at spawn. It does
not say the provider read them, which is section 3.26's first and third
statements, unchanged.

**Rule 17: the selected revision is the launch configuration, and the observed
revision is measured.** The run **selects** the required revision after the
standing has established that it is installed and intact, and never any other
(section 3.25: no latest, no substitute). A project that commits no
requirement selects nothing. The selection is carried to the session in its
constructed environment, beside the attempt binding: `STATECRAFT_RUN_ID`,
`STATECRAFT_ATTEMPT`, `STATECRAFT_STARTUP_NONCE` and
`STATECRAFT_HARNESS_SELECTED`. None of them is a credential and none carries
one.

What **answered** is measured by the shipped `SessionStart` hook. When the
nonce is present in its environment and the manifest gate of section 3.14 rule
3 passes, it prints one acknowledgment line on its standard output, and writes
nothing:

```text
statecraft-startup<TAB>v1<TAB>nonce=<n><TAB>run=<id><TAB>attempt=<k><TAB>selected=<digest-or-none><TAB>project=<dir><TAB>root=<revision-dir>
```

`root` is the revision directory the executing script is in, resolved by the
script from its own path. Claude Code `2.1.267` reports each hook's standard
output in its stream as a `hook_response` event carrying the session id, the
hook event, the exit code and the output verbatim, which the recorded streams
under `crates/statecraft-adapter-claude-code/testdata/stream/` show. The
adapter types those fields (spec `004` section 3.9) and hands the responses to
the run; this spec's crate judges them.

**Rule 18: what an acknowledgment must satisfy, and what each failure is.** The
observed revision is admitted only from exactly one acknowledgment, in a
`hook_response` event for `SessionStart` that exited `0`, in this attempt's own
stream, whose session id is the session id of that stream's init event, and
whose nonce, run, attempt, selected revision and project all equal what
`intent.json` recorded (the project compared as the canonical workspace path).
The revision directory it names is then read and digested, and that full digest
is the **observed** identity. Anything else is **unverified**, and the record
names which:

| Evidence | Recorded as |
|---|---|
| no acknowledgment in any `SessionStart` response | `unverified: absent` |
| a line that begins the acknowledgment and does not parse | `unverified: malformed` |
| another attempt's nonce | `unverified: replayed` |
| another run or attempt number, or another selection | `unverified: wrong-attempt` |
| another project directory | `unverified: wrong-project` |
| a session id that is not the stream's own | `unverified: wrong-session` |
| a hook that exited non-zero | `unverified: hook-failed` |
| two acknowledgments naming different directories | `unverified: conflicting` |
| a revision directory that is not directly inside this home's harness store | `unverified: foreign-revision` |
| a revision directory that cannot be read and digested | `unverified: unreadable-revision` |

An unverified observation leaves the resolved identity absent, and absent is
not a match: the standing stays `exact` with nothing resolved, which section
3.25 already makes not qualified. An admitted observation becomes the resolved
identity and the standing is evaluated against it. Its directory's current
bytes are what the digest is over, so a revision that was altered after it
answered reads as what it now is.

**Rule 19: a mismatch observed after launch refuses the attempt.** Section 3.25
refuses managed execution under a mismatch, and a run cannot detect one before
the session starts, because the acknowledgment is emitted by the session. So a
mismatched standing, measured from an admitted acknowledgment, is counted as a
refusal under the guard `harness-identity` when the attempt is concluded, the
attempt is `refused` (spec `003` section 3.5), and `accept` treats it as spec
`005` section 3.1.1 treats any refused attempt. *Amended by section 3.32 rule
26:* this rule first let the session run to its end and refused it afterwards,
which section 3.25's "nothing was done" does not admit. An unverified
observation is not a mismatch; for a project that commits a requirement it
now withholds governed work under section 3.32 rule 26, and it never qualifies
the attempt. A project that commits no requirement records
whatever was observed and is `unrequired`, which never qualifies.

**Rule 20: what each grade establishes, and does not.**

| Grade | Establishes | Does not establish |
|---|---|---|
| installed integrity | the required directory's files digest to the committed full digest, before launch, and again when the record is written | that anything used them |
| launch configuration | this product selected that revision and named it, with the attempt binding, in the environment it constructed for the process it created | that the provider or any hook read the environment |
| correlated acknowledgment | see section 3.32 rule 27, which replaces this row | see section 3.32 rule 27 |

*Amended by section 3.32 rule 27.* This row first said the acknowledgment
establishes that a hook script located in the named revision directory
executed. It does not: the line's origin is not authenticated, the provider's
response does not name the command that printed it, and the directory digest
is taken when the record is written, not when anything ran. The grade is the
**correlated acknowledgment**, and rule 27 fixes what it does and does not
establish. The acknowledgment remains **launcher-attested**, like section
3.30's launch record. Section 3.32 rule 25 replaces the operator's global
registration with a per-invocation one, so a managed run no longer depends on
ambient registration for its startup path.

**Rule 21: a run attempt's judgement, and why `qualified` is not reachable
through a run.** The judgement read back from the two records is one of:

| Verdict | When |
|---|---|
| `not-launched` | the attempt exists and has no `intent.json` |
| `launch-unknown`, `outcome-unknown`, `spawn-failed` | section 3.32 rule 23; an intent with no record is one of the first two and is never `interrupted` |
| `interrupted` | a record whose process was created and did not end by itself as one readable session |
| `mismatched` | an admitted acknowledgment naming a revision other than the required one |
| `not-admitted` | section 3.32 rule 26: a requirement is committed and no correlated acknowledgment was admitted at the startup decision |
| `unverified` | launched and recorded, and not qualified for any other reason, each one named |
| `qualified` | the record's `qualifies()` conjunction holds |

A run session is not one of section 3.29's three controls, and rule 4 of that
section refuses evidence for another invocation as evidence for this one. So a
run attempt's observation is always `not-observed`, and `qualified` is not
reachable through a run: a completed run with a matching acknowledgment is
`unverified`, and it says the live observation is the missing class. The
judgement is recomputed on every read from the bytes in the two records,
including section 3.29 rule 5's re-admission, so a field edited by hand changes
the judgement rather than asserting one, and one attempt's records are never
read as another's.

### 3.32 Launch states, the correlated acknowledgment, per-invocation startup delivery, and when governed work is released

A narrowly scoped authority correction to section 3.31, settled by the owner on
2026-09-22 and recorded before the implementation it authorizes. It closes four
defects, each a claim the evidence did not support.

1. **An intent was read as a launch.** Section 3.31 wrote `intent.json` before
   the process was created and read an intent with no record as launched and
   interrupted. This product can stop after writing the intent and before the
   spawn, and after the spawn and before anything else is written; neither
   inference holds.
2. **An acknowledgment was read as execution provenance.** Section 3.31 rule 20
   said the acknowledgment establishes that a hook script in the named
   directory executed, and in the same row that nobody authenticates who
   printed the line. The second is right, so the first cannot be.
3. **The managed startup path was ambient.** A run delivered only the deny
   floor, and the `SessionStart` hook that acknowledges a start reached the
   session only if the operator had registered it globally. A managed run's
   startup evidence depended on the operator's home rather than on what the
   run supplied.
4. **A mismatch was refused after the fact.** Section 3.25 says a refused
   managed execution is one where "nothing was done". Section 3.31 rule 19 let
   the session run to its end and then concluded it refused.

**Rule 22: a launch is four write-once records, each a separate fact.** Every
file lives in the attempt's directory of section 3.31 rule 13 and is created
exclusively: an existing file refuses the write, and nothing is ever replaced.

| Record | Written | Establishes | Does not establish |
|---|---|---|---|
| `intent.json` | after every preflight has passed, before the spawn is attempted | this product was about to attempt a spawn, with this configuration | that a process was created |
| `launched.json` | immediately after the spawn call returned a process, **before** the prompt is delivered to it | a process with this id was created for this attempt | anything the process did |
| `admission.json` | at the startup decision of rule 26 | the decision, its reason, and when it was made | that the provider honored it |
| `record.json` | after the process ended, after the launch failed, or after the confirmation of a spawn could not be persisted | the completion, as section 3.31 rule 13 describes it, and which of the earlier records this product wrote | anything the earlier records do not |

The order is fixed: intent, spawn, confirmation, prompt, decision, record. The
prompt is written to the process only after `launched.json` is persisted, so a
confirmation that cannot be persisted stops the process before it has been
given any work, and the record says so. A spawn and a file write are not one
atomic transaction, and nothing here pretends they are: between the spawn
returning and the confirmation being on disk there is a window in which a
process exists and no record says so, and rule 23 names what that window reads
as.

*Amended by section 3.37:* the four records live in the product home, and the gate's files in a separate exchange directory.

**Rule 23: the launch states, read back, and what each does not prove.**

| What is on disk | State | What it means | What it does not mean |
|---|---|---|---|
| no intent | `not-launched` | this product attempts no spawn before the intent is persisted, so it created no provider process for this attempt | that nothing else happened: the workspace was prepared |
| an intent, nothing after it | `launch-unknown` | intent persisted; this product stopped before confirming a spawn | that no process exists, or that one does |
| an intent and a confirmation, no record | `outcome-unknown` | a process with the recorded id was created and given its prompt; its outcome is unknown | that it was interrupted, or that it had no effect |
| a record whose spawn call failed | `spawn-failed` | the operating system reported that no process was created | anything about the workspace beyond what the preparation did |
| a record whose confirmation could not be persisted | `interrupted` | a process was created, and stopped before its prompt was delivered | that stopping it undid anything |
| a record of a completed launch | section 3.31 rule 21, with `not-admitted` added by rule 26 | as there | as there |

Absence of a final record never proves an interruption, and absence of a
record never proves that no side effect occurred. `launch-unknown` and
`outcome-unknown` are the honest words for a crash, and each names the files it
read.

**Rule 24: an uncertain launch is never replayed automatically.** The run
record's attempt stays live when this product stops mid-launch, and spec `003`
section 3.6 blocks a retry of an intent whose outcome is unknown. `run` refuses
the next attempt, names the live attempt and its launch state, and gives the
operator the inspection to perform: `startup show <path> <run-id> --attempt
<n>`, the process id where one was confirmed, and the workspace where effects
may have landed. Nothing here infers an outcome to free the lock. The one way
to free it is an operator's reconciliation (spec `003` section 3.6.1, the
`run reconcile` verb of spec `006` section 3.11.6), which the answer names;
until that section, this build had no such verb and the answer said so.

**Rule 25: a managed run supplies its startup hook and its gate explicitly.**
Where the project commits a requirement, the run's settings document is the
deny floor of section 3.27 plus two hook registrations, and nothing else:

- `SessionStart`, matcher `startup`: the selected revision's
  `hooks/statecraft-session-start.sh`, by absolute path in the installed
  revision directory whose integrity section 3.25 has just checked.
- `PreToolUse`, matcher `*`: the attempt's **admission gate**, a script this
  product writes once into the attempt's directory before the spawn. It is
  launcher content, not harness content, so it is in no revision's digest and
  no harness upgrade changes it.

The mechanism is the one section 3.27 already relies on: Claude Code documents
`--settings <file-or-json>` as an additional settings source, and documents
`hooks` as a settings key. Whether `2.1.267` honors hooks supplied that way, in
`--print` mode, and passes the session's environment to them, is
**unobserved** in a live run. Nothing here writes the operator's home, and a
managed run no longer needs section 3.24's global registration to be
acknowledged. The intent records the exact document by digest and length, each
registration by event, matcher, command and the digest of the script it names,
and, separately, the digest of the deny floor alone. The run's document is not
the floor's bytes, so under section 3.29 rule 4 no qualification of the floor
payload is evidence for a run's document, and the record never reads one as
the other. Where the project commits no requirement, nothing is selected, the
document is the floor alone, no gate is written, and the record says work was
not gated by a startup decision.

*Amended by section 3.37:* the gate script is written into the attempt's exchange directory, not beside the launch records.

**Rule 26: startup identity is an admission prerequisite, and governed work is
released only by the decision.** Section 3.25 promises prevention, so the
decision is made before governed work is released, not after the session ends.
*Governed work* is every tool call the session makes: the channel through which
a session changes anything. The gate refuses every tool call until
`admission.json` records `admitted`, waits a bounded time for a decision that
has not yet been written, and refuses when that time passes.

The launcher decides at the first event in the attempt's stream that is not a
`SessionStart` hook event: in the recorded `2.1.267` streams, the init event,
which follows every `SessionStart` `hook_response`. At that point it judges the
acknowledgments it has read under section 3.31 rule 18, and writes:

| Decision | When |
|---|---|
| `admitted` | exactly one correlated acknowledgment is admitted, and the standing evaluated against it is `exact` |
| `refused: mismatched` | an admitted acknowledgment names a revision other than the required one |
| `refused: not-established` | no acknowledgment is admitted, for any reason rule 18 names |

On a refusal the launcher stops the process group at once and concludes the
attempt `refused`, under the guard `harness-identity` for a mismatch and
`startup-admission` otherwise. A stream that ends before the decision point is
decided at its end, the same way.

What this boundary establishes, and what it does not:

- **Establishes:** a tool call that the provider routed through the gate did
  not run before `admitted` was written, and did not run after a refusal. The
  gate appends each consultation to the attempt's `gate.log`, so the record can
  say whether the gate was consulted at all.
- **Does not establish:** that the provider honors the registration. A provider
  that ignores it runs neither the acknowledgment nor the gate; the decision is
  then `refused: not-established`, the process is stopped, and the refusal is
  **retrospective**: the record says effects before termination are not
  excluded. Nor does it establish that nothing happened outside tool calls (the
  provider's own startup, other hooks), or that stopping the process undid
  anything. Prompt termination is not proof of no effect.

*Amended by section 3.37:* the gate reads its copy of the decision, and writes its log, in the exchange directory; the log is child-attested.

**Rule 27: the correlated acknowledgment.** The grade section 3.31 called
`acknowledged` is **`correlated`**, in the record, in the API and on the
command line. An admitted correlated acknowledgment establishes, and no more:

- a `hook_response` for `SessionStart` in this attempt's stream, exit `0`,
  carrying the stream's own init session id, held one line whose nonce, run,
  attempt, selection and project equal the intent;
- the directory that line names is directly inside this home's harness store,
  and its files, read when the record is written, digest to the recorded
  identity;
- the per-invocation registration of rule 25 named the script in the selected
  revision directory.

It does **not** establish which process printed the line: the provider's
response does not name the command, and the nonce is in the session's
environment, so any hook or process that runs there can print it. It does not
establish that the script at that path is the one that ran, that its bytes when
it ran equal the bytes digested, or that any other file of the revision was
loaded. "Executed" is never claimed. A record written before this section
carries the grade `acknowledged`; it is read as `correlated` with the same
fields and judged the same, and nothing is rewritten.

**Rule 28: the evidence words do not collapse.** Each names one fact and none
implies another: **installed** (a revision directory whose files digest to its
name), **selected** (named in the launch configuration), **supplied** (bytes
in the work tree and on the command line at spawn), **correlated** (rule 27),
**admitted** (rule 26's decision released governed work), and **qualified**
(section 3.29's live observation, unreachable through a run). A record whose
evidence is insufficient under these rules, including one written before them,
stays inspectable and is never qualified by being read.

### 3.33 The managed-startup trial, and why it is not the permission experiment

Settled by the owner on 2026-09-22 and recorded before the implementation it
authorizes. It corrects a reading, adds one bounded experiment, and states what
the ordering in section 3.32 rule 26 assumes of the provider.

**The correction.** Section 3.30's permission experiment launches its three
controls through `startup capture`. Each carries the deny floor's payload and
nothing else: no `SessionStart` registration, no admission gate, no `run`, no
intent and no decision. Its result says whether the floor's deny rule was
enforced against one command on one version. It says nothing about whether a
provider runs hooks supplied through `--settings`, reports them before `init`,
consults a `PreToolUse` gate or waits for it, which are the premises section
3.32 rules 25 and 26 rest on. The handoff's statement of what is outstanding
put that uncertainty beside the permission stage's approval as though running
the stage would retire it; it would not, and the handoff is corrected. The
two experiments answer two questions, their results are reported separately,
and neither result, nor both together, is a qualification: section 3.29's
observation is unreachable through a run (rule 28), and the trial adds no
route to it.

**Rule 29: the trial asks one question.** On the installed provider version,
when a managed run supplies its startup hook and its admission gate through
`--settings` in `--print --output-format stream-json --verbose` mode:

1. does the startup hook's response arrive in the attempt's own stream,
   carrying this attempt's binding;
2. does it arrive before the decision point;
3. is the gate consulted before a requested tool executes; and
4. does a tool execute only after the decision admitted the attempt?

**Rule 30: the trial is a run attempt, not a second launcher.** It goes
through the same preparation (intent, gate, settings document, sentinel of
rule 31), the same supervisor and launch watch, the same finalization and the
same run record as `run`. It differs from `run` in four values and in nothing
else: the run id is the fixed `statecraft-startup-trial`; the prompt is rule
31's instruction; the turn limit is `--max-turns 3`; and the deadline is the
operator's, 120 seconds by default and never more than 300. It consults no
scheduler, because the trial is not a unit of work, and it refuses in a
project that commits no harness requirement, because an ungated run cannot
answer rule 29.

**Rule 31: a harmless sentinel.** Before the intent is written, the launcher
writes `STATECRAFT-TRIAL-SENTINEL` into the attempt's workspace: one line
carrying a fresh nonce. The prompt asks the session to read that file once
with the `Read` tool and to reply with its contents. `Read` changes nothing,
and the workspace is the run's own disposable one. A tool result for a `Read`
request that carries the nonce, with no non-execution note from the harness,
is the evidence that the tool executed; the nonce exists nowhere else, so a
reply cannot carry it without the read. This is the provider's report of the
execution, not a disk observation, and the trial says so.

**Rule 32: the budget is one session and one version probe, and it is spent
once.** The run path's own version probe is the only probe. The trial verb
refuses when the run `statecraft-startup-trial` holds any attempt, live or
concluded, so an uncertain launch is never replayed and a completed trial is
never repeated: a second trial needs a fresh project. The supervisor enforces
the deadline and stops the process group at it; the acceptance script bounds
the whole verb separately. There is no retry of any kind.

**Rule 33: provider execution is a stated act.** The verb refuses unless the
operator names what it is doing: `--provider-session`, which runs the provider,
or `--synthetic`, which states that the executable is a local fake, marks every
trial record synthetic, and can never be read as a provider observation. The
acceptance script's `managed-startup` stage refuses unless
`APPROVED_MANAGED_STARTUP_SESSION=yes`. That approval is not
`APPROVED_PROVIDER_SESSION`, neither satisfies the other, and the local test
route refuses when either is set.

**Rule 34: what the trial keeps.** `trial.json`, written once in the attempt's
directory beside section 3.32's four records, carries: the origin
(`provider-session` or `synthetic`); the sentinel's path, nonce and digest; the
settings bytes the adapter wrote, verbatim; the provider version the probe and
the init event reported; a timeline of the stream by line (each `SessionStart`
`hook_started` and `hook_response`, whether the response carried an
acknowledgment line, the init event, each tool request by id and name, each
tool result by id with whether it executed and whether it carried the nonce,
and the terminal event); the line and the event kind the decision was made at;
the gate's log; the process end (exit or signal, whether the supervisor stopped
it, and surviving processes); and the judgement of rules 35 to 37. Nothing is
removed afterwards. After writing, the verb reads the attempt's records back
from disk and judges them again; a reloaded judgement that differs from the
one written is a failure, not a result. `startup show` renders the trial's
section for the trial's run.

**Rule 35: the hook evidence has a name for each shape.**

| Hook evidence | When |
|---|---|
| `correlated` | exactly the acknowledgment rule 27 describes, read before the decision point |
| `absent` | no `SessionStart` response anywhere in the stream |
| `unbound` | a `SessionStart` response before the decision point, whose acknowledgment did not bind; the kind is section 3.31 rule 18's word |
| `late` | acknowledgments only after the decision point, which rule 26 never waits for |
| `conflicting` | acknowledgments naming different revisions |
| `mismatched` | correlated, naming a revision other than the required one |

**Rule 36: effects before admission get one of three words, never more.**

| Word | When |
|---|---|
| `excluded` | the decision was `admitted`, every tool request in the stream has a gate consultation, every consultation released the call only on `admitted`, and every executed tool result follows the decision line |
| `demonstrated-possible` | a tool executed with no consultation to account for it, or executed before the decision line, or executed while the decision refused, or after the gate withheld or refused it |
| `unobserved` | neither: nothing executed, or the stream is incomplete, so the records cannot say |

Each word is scoped to tool calls. None covers the provider's own startup,
other hooks, or anything outside a tool call, and the record says so beside the
word (rule 26).

*Amended by section 3.37 rule 3:* under confinement the gate consultations these rules read are child-attested; a trial recorded before section 3.37 is read as it was.

**Rule 37: the verdict.**

| Verdict | When |
|---|---|
| `established` | hook evidence `correlated`, the decision `admitted`, at least one tool request, every request consulted, the sentinel executed carrying its nonce after the decision line, effects `excluded`, and a process that ended by itself with no survivor |
| `not-established` | a completed launch in which any of those fails; every failing condition is a named reason |
| `uncertain` | a launch state of `launch-unknown`, `outcome-unknown`, `spawn-failed` or `interrupted`, or a process the deadline stopped |

A synthetic trial reaches these words with its origin beside them, and a
synthetic `established` is a statement about the procedure, never about a
provider.

**What an established trial does not establish.** Which process printed the
acknowledgment (rule 27); that the hook's bytes when it ran equal the bytes
digested; that a refused decision blocks a tool call, since an admitted trial
never refuses and a refusal would be a second session; anything about another
version; anything about the deny floor, which is section 3.30's question; or
qualification.

**The ordering the gate implements, and what it assumes.** The gate opens
exactly when `admission.json` exists holding `"decision":"admitted"`, and that
file is written when the launcher reads the first stream event that is not a
`SessionStart` `hook_started` or `hook_response` (the 2026-09-22 entry's fourth
choice). A tool call requested before that event reaches a gate that polls for
the file every tenth of a second for thirty seconds: an admitted decision
releases it, a refusal refuses it with the provider's blocking code `2`, and no
decision within the wait refuses it the same way and logs `withheld`. The gate
never releases on a timeout. Five provider behaviors are premises, and the
trial observes each or shows that it failed:

- **P1.** A `SessionStart` hook supplied through `--settings` runs in
  `--print` mode and is reported as a `hook_response` before `init`. Recorded
  only for a globally registered hook (the committed 2.1.267 streams); through
  `--settings`, unobserved. If it arrives after `init`, it is `late`.
- **P2.** That hook receives the session's environment, which carries the
  binding. Unobserved. If not, the evidence is `unbound`.
- **P3.** A `PreToolUse` hook supplied through `--settings` runs before the
  tool executes, the provider waits for it up to the registration's
  `timeout` (sixty seconds, which is also the documented default), and exit `2`
  blocks the call. Documented; unobserved here. If the provider does not wait,
  or runs the tool anyway, the effects word is `demonstrated-possible`.
- **P4.** `Read` needs no approval inside the working directory in the
  default permission mode. Documented. If it is denied, the sentinel does not
  execute and the verdict names that.
- **P5.** No event other than a `SessionStart` hook event precedes `init`. If
  one does, the decision is made there with no init session read, the
  acknowledgment is judged `wrong-session`, and the attempt is refused. That is
  fail-closed, and the trial records the event kind the decision was made at
  so the cause is visible rather than inferred.

None of these is weakened to make the trial pass: not the gate's wait, not the
decision point, not the refusal on a missing session.

### 3.34 One allowlisted event after the terminal event

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before the implementation it authorizes. It amends section 3.30 rule 8
in one clause, "the terminal event is its last event", and nothing else in
sections 3.29 and 3.30. Rules 1 to 7 and 9 to 12 are unchanged and govern
everything below.

**The gap this closes, exactly.** The one authorized permission experiment
(section 5, the 2026-09-23 entry on the two live experiments) stopped at its
first session because Claude Code `2.1.267` wrote a `system` event of subtype
`task_summary` after its `result` event, and rule 8 refuses any event after the
terminal one. The measured trailer, event 12 of the refusal control's capture
(stream `cc9b6e59…ea11`), is exactly:

```json
{"type":"system","subtype":"task_summary","detail":null,"uuid":"…","session_id":"…"}
```

with the capture's own session id. The same capture carries a second
`task_summary`, before the terminal event, whose `detail` is a string. The
recorded streams under `crates/statecraft-adapter-claude-code/testdata/stream/`
carry neither. Nothing in this amendment treats a `system` event as harmless
because of its type, and nothing treats a summary as proof that no further work
happened: the trailer is admitted as an event that carries nothing the
admission reads, and refused whenever it could carry more.

**Rule 13: at most one trailer, and only this one.** A capture may carry, after
its terminal event, at most one further event, the **trailer**, and only when
every condition below holds. Otherwise rule 8 applies as written and the
capture is out of order.

1. **A complete terminal event precedes it.** The capture already holds exactly
   one init event and exactly one terminal event, in that order, and the
   terminal event deserializes as the adapter's result type. A trailer never
   completes a capture that has no terminal event, and a trailer before the
   terminal event is not a trailer (see "Before the terminal event").
2. **It is the last event.** Nothing follows the trailer. A second trailer, a
   second terminal event, or any other event after it refuses the capture.
3. **Its shape is closed.** Its line is one JSON object whose members are
   exactly `type`, `subtype`, `detail`, `uuid` and `session_id`: none missing,
   none added. `type` is `system`. `subtype` is on the closed list, which has
   one entry, `task_summary`, and whose provenance is the capture named above.
   `detail` is JSON `null` or a string. `uuid` is a non-empty string.
   `session_id` is a non-empty string.
4. **It names the capture's session.** `session_id` equals the session every
   other event in the capture names. A trailer naming another session is
   mixed-session evidence under rule 8; one naming none, or an empty one, is
   not the closed shape of condition 3 and is refused as out of order.
5. **The shape is read by the adapter.** Rule 7 applies: the closed shape is a
   type in the supported adapter's crate (spec `004` section 3.9), and this
   spec's crate does not parse the line a second way.

Anything else after the terminal event refuses the capture, including, for
avoidance of doubt: an `assistant` or `user` turn; a tool request or tool
result; a `permission_denied` or hook event; a `rate_limit_event` or any event
of a type the adapter does not map; a `system` event of any other subtype; a
`task_summary` with any additional member such as a tool-use id; and a second
`result`.

**Rule 14: the trailer is not evidence.** No field of the trailer is read for
any decision: not the classification of a tool use (rule 9), not a control's
outcome (rule 10), not the terminal or process judgement (rule 11), not the
version or the binding (rules 4 and 12). `detail` is never read, so no prose in
it can qualify, refuse, supply a denial, supply a result, or stand in for a
control. The testable form of this rule: **the admission's judgement of a
capture that carries an admitted trailer is identical to its judgement of the
same capture with the trailer's line removed.** A trailer never turns a
refused capture into an admitted one or the reverse.

**Before the terminal event.** Unchanged. A `task_summary` before the terminal
event is a `system` event naming the session, counted for rule 8's session
check and carrying nothing read, as every `system` event of a subtype the
admission does not name already is. A `rate_limit_event` before the terminal
event is an event of a type the adapter does not map, which spec `004` section
3.1 classifies as progress; its position already counts. This amendment
accounts for both deliberately, and it admits neither after the terminal event
except the one trailer rule 13 describes.

**Bounds.** The trailer is read in the same single pass over the capture that
rule 8 already makes, and the admission reports the trailers it admitted from
that pass rather than reading a capture again. The line the trailer is read
from is the line the adapter read the event from: a capture whose events and
non-blank lines do not correspond one to one is unreadable. The product sets no byte limit on a capture today and
this amendment adds none; the launch's deadline (section 3.30, "Bounds") bounds
what a session can write, and rule 13 bounds what may follow the terminal event
to one line of a closed shape.

**Bytes and order are preserved.** The capture's bytes are kept whole,
trailer included, in the order the provider wrote them. Nothing strips,
rewrites or reorders the trailer before the admission reads it or after.

**Where it applies.** Everywhere rule 8 applies through the admission's reading
of a capture: the permission experiment's admission, and `startup capture`'s
reading of whether a launch completed as one session. It does not change the
managed-startup trial (section 3.33) or a run's startup records (sections 3.31
and 3.32), which do not read a capture through rule 8.

**Historical records.** The refusal control's record from the 2026-09-23
experiment keeps its verdict, `incomplete` under rule 8 as then written, and its
archive is not rewritten. Replaying its bytes through the amended reading is
**offline regression evidence**: it is labeled a replay, it is not a live
observation, and it neither counts toward nor spends any provider session. A
record read back later is re-judged under the rules in force (section 3.29
rule 5), and that single launch record cannot qualify anything, because an
admitted observation needs all three controls.

**The command surface is unchanged.** No verb, flag or exit code is added.
`startup capture` and `startup qualify` report the trailer when one was
admitted, as one line naming its event number and subtype and saying it was not
read.

**Acceptance.** Positive: the measured trailer shape, with `detail: null` and
with a string `detail`, after a complete terminal event, admitted; the
trailer-removed identity of rule 14, over both an admitted and a refused
capture; and the archived capture's bytes, replayed offline, reading as one
complete session with its trailer reported. Negative, each refusing the
capture: a second trailer; any other `system` subtype after the terminal
event; a `rate_limit_event` after it; an assistant or user turn after it; a
second terminal event after the trailer; a trailer with an added member, a
missing member, a non-string non-null `detail`, an empty `uuid`, or a
`session_id` that is absent or names another session; and a trailer with no
terminal event before it.

### 3.35 Per-path ownership transfer, as an operator's act

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before the implementation it authorizes. Section 3.21 retains, from
the withdrawn section 3.7, that ownership transfer is "per path, explicit,
operator-initiated, reversible, and recorded with the digest observed at the
moment of transfer and with the producer revision it was evaluated against".
This section is the operation.

**The gap, exactly.** The manifest can carry a `transfer` on an entry, and
`env plan` and `doctor` read one, but nothing creates one and nothing reverses
one. A path this product would manage and finds occupied is withheld as
`foreign` on every apply, and the only way to change that is to edit the
committed manifest by hand.

**Rule 1: three classes, four moves, one path.** The classes are section 3.2's.
A transfer names exactly one repository-relative path, its current class and
the class it is to take, and is one of:

| From | To | What changes |
|---|---|---|
| `user` | `adopted` | An `adopted` entry is recorded with the digest observed now. Nothing is written to the file. |
| `user` | `managed` | A `managed` entry is recorded with the digest observed now, and a `transfer` naming the prior claimant. Only for a path this product would itself write: one an installed adapter declares, with that adapter as its source. A later `env apply` or `env upgrade` may then rewrite it; the bytes it held before are not kept by this product. |
| `adopted` | `user` | The entry is removed. |
| `managed` | `user` | The entry is removed. The file stays, byte for byte; `env remove` no longer deletes it. |

`adopted` to `managed` and back is two transfers through `user`, each recorded.
A path this product has no source for cannot become `managed`: management
means upgrades rewrite it, and there would be nothing to rewrite it with.

**Rule 2: never inferred.** A transfer happens only when an operator asks for
it by path. Nothing moves a path between classes because its bytes resemble
something this product generates, because a producer returns it, or because a
directory holds it. Initialization and upgrade never transfer. In particular a
user instruction file is `user` class under section 3.8 and **cannot** be
transferred to `adopted` or `managed` by this operation. The list is closed, by
file name at any depth: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.cursorrules`,
`.windsurfrules`, and `.github/copilot-instructions.md`. A pointer file this
product wrote under section 3.8 is already `managed` and may be released to
`user`. The root instruction bridge of section 3.13 is a modification, not an
entry, and is not transferable either.

**Rule 3: what may be named.** The path is relative, uses forward slashes, has
no empty, `.` or `..` component, and names a regular file that exists. No
component may be a symbolic link. A directory is refused, so nothing is adopted
wholesale. Anything under `.statecraft/` and anything under a `.git`
component are refused. Every refusal names the
rule and writes nothing.

**Rule 4: plan, then apply.** `transfer plan` reports, and writes nothing: the
path, its current class as the manifest and the file say, its digest and length
now, the resulting entry, the producer revision, and a **plan identity**, the
SHA-256 of the path, both classes, the file's digest and the manifest's digest.
`transfer apply` takes the same path and classes, that plan identity, an
operator name and a reason, recomputes the plan, and refuses unless the
identity is equal: a file edited, a manifest changed or a class that is no
longer the one named in between is a stale plan, refused, with what changed
named. The operator name is recorded as supplied and not authenticated.

**Rule 5: the record.** An applied transfer changes the manifest entry as rule
1 says and appends one record to the transfer journal, a `transfers` list
inside `.statecraft/environment.json` itself, so that section 3.12's four
paths stay four: its
identity, the path, both classes, the digest and length observed, the producer
revision it was evaluated against (the `spec-spine-core` version this build
links), the operator as supplied, the reason, the time, and the manifest's
digest before. Nothing else in the manifest and no byte of any file changes.
The journal is append-only. A manifest whose journal disagrees with its
entries is reported by `transfer plan`, and `transfer apply` and `transfer
revert` refuse on it.

**Rule 6: reversal.** `transfer revert` names a recorded transfer, an operator
and a reason, and applies the inverse move when, and only when, that transfer
is the latest for its path, the path's class is still the one it produced, and
the file's digest is what this product's record says it should be: the digest
the manifest entry records now, where the transfer produced an entry (so a
rewrite this product made and recorded, such as an `env apply` after a move
to `managed`, is not an intervening change), and the digest the transfer
observed, where it removed one. An unrecorded edit, a later transfer or a
missing file refuses the reversal and names which. Reversal changes ownership,
never content: reversing a move to `managed` does not bring back bytes an
apply replaced. A reversal is a new journal record that names the one it
reverses; the reversed record is kept.

**Rule 7: repeating a satisfied request.** Checked before the plan identity:
applying a transfer whose latest journal record already moved the same path
between the same classes, with the path still in that class and the file's
digest what rule 6 says it should be, reports `already-satisfied` and writes
nothing, whatever plan identity was given. Any other request whose named
current class is not the path's class is refused.

**Rule 8: the producer boundary is unchanged.** Adopting a conforming producer
(section 3.15) transfers nothing. A user's pre-existing instruction files stay
`user`, and section 3.13's recognition of a generated, unmodified root
`AGENTS.md` is not a transfer.

**The command surface.** Spec `006` section 3.11.7 adds `transfer plan`,
`transfer apply` and `transfer revert`.

**Compatibility.** A manifest written before this section has no journal and
reads as having no transfers. An entry carrying a `transfer` with no journal
record is read as before, reported by `transfer plan` as recorded without a
journal, and never rewritten. A build that predates this section and rewrites
the manifest does not carry the `transfers` list; none is distributed
(`F-02`), and a manifest that lost its journal reads as above.

**Acceptance.** In an isolated home and fixture repositories, through the
binary: `user` to `adopted` and back; `user` to `managed` for an adapter path
that `env apply` withheld as `foreign`, after which `env apply` writes it, and
the reversal, after which it is withheld again; `managed` to `user`, after
which `env remove` leaves the file. Negative: a stale plan after the file, the
manifest or the class changed; a named class that is not the path's; `user` to
`managed` for a path no adapter declares; a root `AGENTS.md`; a symbolic link,
a `..` path, a directory, `.statecraft/environment.json` and a `.git` path;
a reversal after an intervening edit or a later transfer; and a repeated
request reported `already-satisfied` with nothing written. Every refusal
leaves every byte of the repository as it was.

### 3.36 When this spec's implementation is complete

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before it is applied. Since section 3.24 moved `implementation` from
`complete` to `in-progress`, no section has said what would move it back, so
the field has been read either as waiting on a live result no local work can
supply or as ready to flip because the product fails closed. This section
fixes the rule. It changes no requirement in sections 3.1 to 3.35.

**Rule 1: the facts, kept apart.** *Implemented* is a property of this
repository's code measured against sections 3.1 to 3.35. The provider evidence
is not one fact but several. Section 3.32 rule 28 keeps installed, selected,
supplied, correlated, admitted (the startup decision of its rule 26) and
qualified apart. This section adds two words of its own: a session was
**observed** (section 3.29's three controls were captured), and an observation
was **observation-admitted** (sections 3.29 and 3.30 accepted it). Neither kind
of admission is a qualification, and neither implies the other. An
observation-admitted observation is not the verdict `qualified` of section 3.31
rule 21: it makes no run, trial or session qualified, and this section treats
it only as evidence under section 3.27. Where section 3.29 says an observation
"qualifies", it speaks of the floor payload for the invocation the observation
is bound to, and this section does not read that as a session's qualification.
*Activation* is this product acting on a real home. *Counterparty completion*
is what section 3.22 leaves to another repository. Only the first is what the
frontmatter's `implementation` field records; the others are reported beside
it, each on its own evidence, and none is inferred from another. None of them grants the
field, and none withholds it except through a requirement of sections 3.1 to
3.35 that mandates its result (rule 5).

**Rule 2: every normative requirement is accounted for.** `implementation:
complete` requires that, on one revision of `main`, every requirement of
sections 3.1 to 3.35 is in exactly one of these classes, and nothing is in
none:

| Class | What it needs |
|---|---|
| implemented | Reachable through this product's command surface where the requirement is about the product's behavior, and exercised by a named test or declared acceptance command. Code no verb reaches is **not** this class. |
| deferred | Deferred by name in section 4, or by an adopted deferral row of the decision record (an `F-` row, not a proposed one), as either stood before this section; or by a scope change the owner approved. That approval names the requirement by section and rule, says why, and is the owner's own act (a review approval, a commit, or a dated statement), cited in the section 5 entry that records it. A dated section 5 entry an agent writes records that approval and never substitutes for it. A requirement is not deferred by being hard. |
| external | Decidable only by a provider session, a real home, a release, or another repository's act, **and** the requirement does not mandate a particular result. Its current disposition is recorded, whatever it is. A requirement that mandates a result and depends on such an act is accounted for only when that result is established; until then it is unresolved and blocks `complete` (rule 5). |

**Rule 2a: a frozen obligation is never deferred or external.** Each
principle spec `000` freezes is accounted for, in this spec's territory, as
implemented, whether or not a section of 3.1 to 3.35 restates it; that
includes Constitution IX's placement of records where the supervised process
cannot reach them. It is not deferred by any route in rule 2, including a
decision-record deferral such as `F-09`'s of operating-system enforcement; it
is not external because a platform or an operator's act would be needed to
meet it; and recording it as a residual does not account for it. Until it is
implemented it is unresolved and blocks `complete`.

**Rule 3: failing closed is not the same as implemented.** A requirement that
the product *do* something is not satisfied by the product reporting that it
cannot. A requirement that the product *refuse* or *report* something is
satisfied by the refusal or the report. A requirement that makes the product's
reliance on a mechanism conditional on evidence is satisfied only by that
evidence or by the product demonstrably not relying on the mechanism; where
another requirement of these sections itself relies on the mechanism, the
second reading is not available.

**Rule 4: the producer and acceptance.** The pinned producer is the published
crate, resolving with no Git or path override, and conforms under section 3.15
on that revision. Every spec's declared acceptance passes there, run serially,
with every ignored, skipped or unavailable check named. A skipped or
unavailable check that is the only evidence for a requirement leaves that
requirement unaccounted for under rule 2.

**Rule 5: experiments need a disposition; a mandated result needs the result.**
Every live experiment the owner authorized has a recorded disposition
(established, unverified, not admitted, or not run with the reason), with its
evidence kept and its original verdict never rewritten. A positive result is
required where a requirement of sections 3.1 to 3.35 mandates the result
itself rather than the procedure that measures it. Section 3.29 rule 6 is not
such a requirement: it makes `unverified` the correct answer when evidence
cannot decide. **Section 3.27 is one.** It requires that "the installed version
and the effective behavior are verified before this product relies on the
mechanism, and an unverified mechanism is an unavailable one", and section
3.32 rule 25 relies on that mechanism to deliver a managed run's settings
document: the deny floor, the startup hook and the admission gate. So section
3.27 is unresolved, and blocks `complete`, until the behavior a run relies on
is established for the installed provider version: the deny floor through an
admitted observation of sections 3.29 and 3.30, and hooks supplied through
`--settings` through an established trial of section 3.33. Evidence for one
version is not evidence for another, and a matching version string is not by
itself the same provider. The evaluation names the installed binary by path
and digest and shows that each piece of evidence recorded that digest; evidence
that does not record the binary's digest cannot be bound under this rule and
does not count, and recording it on the run and trial paths is a change to
sections 3.31 and 3.33. The deny floor a run supplies, digested alone as
section 3.32 rule 25 records it, must equal the payload the observation is
bound to under section 3.29 rule 4, and the trial's startup-hook and gate
registrations must match the run's by event, matcher and script digest.
Section 3.32 rule 25 says a floor-only observation is not evidence for a run's
whole document; whether it is enough for the floor inside that document is the
owner's decision, and until the owner makes it, section 3.27 is unresolved.
What this rule requires is the observation's admission and the established
trial; it does not require that any session be `qualified`, which section 3.31
rule 21 makes unreachable through a run (restated in section 3.32 rule 28). The alternative, a `run` that treats the
mechanism as unavailable for an unverified version, would be a change to
section 3.32 and is the owner's to make; this section does not make it.

**Rule 6: how it is applied.** The evaluation is a dated section 5 entry
naming the revision, the count of implemented requirements, every requirement
that is deferred or external with its class and evidence, each frozen
obligation of rule 2a with its evidence, and a statement that no requirement is
unresolved. The field then moves in its own lifecycle-only change, which is the
owner's act; this section makes it so. A requirement later found
unmet, or a new requirement, moves it back to `in-progress` in the change that
finds or adds it.

### 3.37 Where an attempt's launch records live, and what the child is given

An authority amendment, settled by the owner on 2026-09-23 with spec `004`
section 3.18 and recorded before the implementation it authorizes. Sections
3.31 and 3.32 put an attempt's launch directory at
`.statecraft/state/startup/runs/<run>/<attempt>/` inside the target, beside the
workspace the child works in, and put the gate's script and log in the same
directory as the intent, the launch record, the admission decision and the
record. A child can therefore read and rewrite the files its own startup
judgement is recomputed from, and can write the admission decision before the
supervisor does. Constitution IX does not admit that. Section 3.36's accounting
covers this section's requirements as it covers those of sections 3.1 to 3.35.

**Rule 1: the launch records move to the product home.** An attempt's
`intent.json`, `launched.json`, `admission.json`, `record.json`, and a trial's
`trial.json`, live in the product home under the repository's records, one
directory per run and attempt, keyed as the run record is keyed. Their names,
contents, write-once rules and judgement are unchanged; only the directory
moves. The child can neither read nor write them (spec `004` section 3.18 rule
2).

**Rule 2: the exchange directory.** The files the child is given live in a
separate per-attempt exchange directory in the product home (spec `004` section
3.18 rule 5): the admission gate script, the settings document the provider is
given, a copy of the admission decision, and the gate log, created empty before
launch. The gate reads the decision and writes its log there. The supervisor
makes `admission.json` durable in the launch records first and then writes the
exchange copy by creating a new file and renaming it into place; the launch
records' copy is the evidence of what was admitted, and the exchange copy is
only what the gate reads. The settings document's digest is taken before launch
and again when the record is written, as before, and a difference is recorded
as before. The gate log is **child-attested**: the supervisor copies it into the
launch records when it writes `record.json`, bounded in size, read without
following a link, and labelled as written by a process inside the confinement.

**Rule 3: what rests on the gate log.** Section 3.32 rule 26's gate withholds
every tool call until the decision admits; that withholding is enforced by the
gate reading a file the child cannot write, and is unchanged. What the log
reports afterwards (each consultation, and section 3.33's `established` and
`excluded`, which rest on every tool request having a gate consultation) is
child-attested evidence under spec `004` section 3.18's confinement. A trial
recorded before this section, including the one established observation of
section 5's 2026-09-23 entry, was recorded without confinement and is read as it
was; a trial recorded after it states that its consultations are
child-attested. Section 3.36 rule 5 relies on an established trial and inherits
that statement.

**Rule 4: `startup trial` and `startup capture` are confined too.** Both start
a provider and refuse, as a preflight with nothing launched, when the boundary
cannot be established (spec `004` section 3.18 rule 9). A capture writes the
settings file it hands the provider into that capture's exchange directory, not
into the directory the operator names; the operator's directory receives the
capture records after the provider has exited, and a directory inside the
project stays refused, as before. Section 3.29 rule 4 and section 3.30 rule 12
bind the provider's program and arguments as handed to the confinement; the
confinement is bound apart from them by its mechanism and its profile or
ruleset digest, and the wrapper's own arguments are not part of the invocation
(spec `004` section 3.18 rule 10). A confined observation and an unconfined one
are not the same invocation, and no earlier observation is re-judged.

**Rule 5: records written before this section.** Launch records already in a
target's `.statecraft/state/startup/runs/` are read where they are, judged as
before, and labelled as written where the child could reach them. They are not
moved, rewritten or re-judged as protected, and no attempt that wrote them is
reported as confined. A confined child cannot write that tree (spec `004`
section 3.18 rule 2), so no record can be added to it after this section.

## 4. Out of scope

Installing the product itself; provider authentication; hosted registration;
multi-machine environment sync; and any write to a target outside the manifested
set. Publication, release and distribution are deferred by `F-02`; how this
product is itself packaged is recommendation `D-01`.

Implementing a hosted platform, a platform protocol or a platform client; any
user interface, which `F-04` defers; publication, release and distribution,
which `F-02` defers; multi-machine environment sync; a package registry or a
plugin marketplace; migrating sibling repositories or any real home directory;
adding a provider adapter, which is `004`'s boundary (it absorbed `008`) and `F-07`'s deferral; and
any new work, run or acceptance ledger, because `003` and `005` already own
those semantics and a second one would be the failure this product exists to
avoid.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what it requires.
The entries from 2026-09-20 concern the realignment and were recorded
against `010` while that spec was separate; they are kept verbatim, because
this section is a history and a history is corrected by appending.

**2026-09-16: the command surface is not in this crate.** §3 names
`project register`, `env plan`, `env apply`, `env upgrade`, `env remove` and
`doctor`. Those are the operator's vocabulary; this spec's territory is
`crates/statecraft-environment/`, and D-02 gives the binary its own crate
(`statecraft-cli`), which no ratified spec owns. So every behavior above is
implemented here as a library operation, and binding it to a process waits for
the spec that owns a binary. The grade this spec claims is therefore
*implemented* for its own territory, and nothing about a command line.

**2026-09-16: timestamps are formatted in-crate.** The manifest is committed and
read by people, so `written_at` is RFC 3339 rather than a count of seconds. A
calendar dependency for one field was not worth it in a crate whose job is to
own as few bytes as possible, so `time::rfc3339_utc` does the conversion and is
tested against the Gregorian century rules that a naive implementation gets
wrong.

**2026-09-16: `unmanaged-write` is decided by declaration, not by memory.** §3.2
calls a path the product wrote but did not record a defect, and this product
keeps no second record to compare against. `doctor` therefore reports
`unmanaged-write` for a path that is present on disk, declared by one of the
configured adapters, absent from the manifest, and claimed by no other
installer. The last clause is what keeps it distinct from `foreign`.

**2026-09-16: `shadowed` needs a resolver this product does not have.** §3.7
says a shadow is reported only where the product can observe one. `ShadowResolver`
is the seam, and the shipped implementation observes nothing, because no harness
package format exists to interrogate. A digest match therefore still reports
`present` today, and the state is reachable and tested through an injected
resolver rather than dead code.

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

**2026-09-20: the trimmed producer was tested before it was released, under a
temporary override that is not committed.** Section 3.5 permits a local source
override for development verification and forbids committing one. Measured on
this date against the sibling spec-spine working tree at
`eb37615f34c5c5327d05e87189f08b4c4af38c0c`, branch
`120-the-engine-ships-governance-not-an-environment`, three paths dirty,
reached through a `[patch.crates-io]` entry added to this workspace's manifest
and reverted immediately afterwards: `scaffold_init_json` returns only the
contract set, the conformance reads `conforming`, and the end-to-end
initialization reports **`complete`** where the released `0.21.0` reports
`partial`. The three tests that assert today's non-conformance fail against
that tree, which is exactly what they are for. **Nothing about a release is
claimed**: the committed dependency stays `=0.21.0`, and adopting the trimmed
producer is a pin bump plus those three inversions, as its own change.

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
a temptation the product must not read, and `docs/decisions/00-founding-decisions.md`
mentions the old path in prose.
Both are other specs' territory (`003` and `001`), the negative control still
holds against any shard directory, and widening this change to reach them would
be editing a spec's territory for a cosmetic improvement. They are recorded here
as a follow-up rather than swept.

**2026-09-20: ratified, and the draft exception it needed is retired.** This
spec was written and built as a `draft`, which AGENTS.md records as the one
exception this corpus has granted. The owner ratified it on the date above, so
`status` is `approved` and `implementation` is `complete` against the evidence
this file's Verification block declares. The exception was for this realignment
and nothing else: the passages in AGENTS.md and CLAUDE.md that carried it are
removed by this change, and the general rule they qualified is unchanged. No
agent ratified anything.


**2026-09-21: the dependency on `006` becomes `extends` and `amends` alone.**
While the realignment was spec `010`, it could name `006` in `depends_on`: this
spec did not, so the graph stayed acyclic. Merged, `002` both depends on `006`
and is depended on by it, and `compile` refuses the cycle (`V-014`). The two
edges that carry the real relationship stay and say more than the dropped line
did: the `extends` edge names the exact crate the verbs are bound in, and the
`amends` edge names what §3.16 reads differently in `006` §3.6. The same
resolution was taken in `004` on the same date and for the same reason.

**2026-09-21: the spec-spine harness handoff is folded in here, and two contracts
the shipped hook does not yet meet.** The handoff arrived as
`docs/design/01-spec-spine-harness-handoff-2026-09.md` and is now §3.22 and
§3.23. It was filed rather than folded by mistake: a design note beside the
corpus is a second place for a requirement to live, and AGENTS.md's source
ownership table has recorded since the founding record moved that this repository
keeps no design directory. Nothing was dropped in the move; the half reporting
that §3.7 still named a live counterparty was already discharged by §3.7 and
§3.21 before the note landed, and the half about spec-spine's renumbered ordinals
is AGENTS.md's "Citing another corpus", written in this same change.

Folding it in made two gaps measurable that a loose file had left unmeasured.
The harness `hooks/statecraft-gate.sh` that `harness::shipped()` carries predates
§3.23 and does not satisfy contract 2 or contract 5: it invokes a bare
`spec-spine` resolved from `PATH` with no `$SPEC_SPINE_BIN` and no repository-local
`target/release/spec-spine` ahead of it, and under `set -eu` it reads the exit
code without first establishing that the binary carries the verb, so `clap`
spending `2` on an unknown subcommand is indistinguishable from a stale tree.
Both are real against the contracts and neither is repaired here: a change to a
hook is an authority change under AGENTS.md "Approval semantics", decided on its
own and never bundled with the change that would authorize it. The gap is
recorded, named precisely, and left for that decision. Contracts 1, 3, 4, 6 and 7
the shipped hook already meets, and its project gate is the test §3.14 rule 3
requires, which is what makes it inert outside a Statecraft project.

**2026-09-21: the two hook contracts are repaired, and the assertions now live
here.** The entry above recorded `hooks/statecraft-gate.sh` failing §3.23
contracts 2 and 5 and left the repair for its own decision, because a change to
a hook is an authority change. The owner asked for it, so this is that change
and nothing else rides along with it.

The hook now resolves `$SPEC_SPINE_BIN`, then the target repository's own
`target/release/spec-spine`, then `PATH` (contract 2); probes `check --help` and
`lint --help` before reading any exit code, so a binary older than the verb is
refused by name instead of reported as a stale tree (contract 5); reads each of
`check`'s four answers as itself and exits with the code it was given, rather
than letting `set -e` collapse them (contract 4); refuses when no binary can be
executed inside a Statecraft project (contract 6); and takes the target
repository as its argument rather than from the session's working directory
(contract 3). It remains inert outside a Statecraft project, which is §3.14 rule
3 and a different thing from contract 6.

**§3.23's "whoever owns the files owns the assertions" is now discharged for the
hook.** `crates/statecraft-home/tests/harness_hooks.rs` extracts the shipped body
and runs it as a program against each contract, with a stub `spec-spine` that
records which binary was chosen. Asserted by execution rather than by reading the
source for a phrase, because a script that mentions a contract in a comment would
pass the second and fail the first. Measured against the pre-repair body, eight of
its ten tests fail; the two that pass are the properties that body already had.

Two defects in the first draft of those tests are worth recording, because both
produce a green suite that establishes nothing. Its fixture set `PATH` to the
stub directory alone, which removed `git` along with everything else, so the hook
found no repository and exited 0 having run nothing, and three assertions of the
form "every line of the witness is X" passed on an empty witness. The fixture now
puts a `git` shim in that directory and `assert_only_ran` refuses an empty
witness first. Separately, a scan for writing verbs over the whole body tripped on
the word "re-indexing" inside a message; the answer was not to reword the message
but to assert over the binary in command position, since a scan that cannot tell
an executed word from a printed one gets worked around rather than fixed.

**2026-09-21: the owner amended §3.14 rule 1, and this spec is no longer
`complete`.** §3.22 records that spec-spine's `.claude/settings.json` waits on
this product. Measuring the two sides against each other made the contradiction
plain: rule 1 as written refused every write to a user's settings file, and
`delivery::NEVER_TOUCHED` enforced that in code, so the four hook events and the
deny list had no route here and the leanout could not finish. Three ways out were
put to the owner: spec-spine keeps that file as its own governance, rule 1 is
amended, or the operator pastes a wiring snippet. **The owner chose the
amendment**, and §3.24 is it.

What §3.24 deliberately is not: a settings manager. It carries two kinds of line
and no others, it may add a refusal and never a permission, and a key it does not
name is not writable by any code path. It is modelled on §3.13's root instruction
bridge, which is this corpus's existing answer to touching a file it does not own:
a marked region, recorded as a modification with the digest either side, removed
only while intact, and reported rather than reclaimed once a user has edited it.

**This change is the authority change and nothing else.** AGENTS.md separates one
from the implementation it would authorize, so no code moves here.
`delivery::NEVER_TOUCHED` still lists `settings.json`, `native_destination` still
links only `skills/` and `agents/`, and no consent flow exists. §3.24 is therefore
**specified and not implemented**, and `implementation` moves from `complete` to
`in-progress` rather than staying a claim this tree does not support. That also
makes the remaining work schedulable, which is where the next session should find
it.

**Two corrections to §3.22's inventory, measured 2026-09-21 rather than taken from
the note.** A history is corrected by appending, so §3.22 stands and this is the
correction. That `settings.json` carries **four** hook events, not three:
`SessionStart`, `PostToolUse`, `PreToolUse` (which holds the push gate and the
PR gate together) and `Stop`. And there is a fifth class the note did not
mention, `.claude/agent-memory/`, whose disposition nobody has decided. The rest
of §3.22 was checked and holds, including the global push gate at
`~/.claude/hooks/push-gate.sh`, which is installed and registered.

**2026-09-21: the fifth class belongs to a sibling product, and this one delivers
no agent memory.** The correction above left `.claude/agent-memory/` undecided.
The owner's disposition is that persistent agent memory is not harness content at
all: it belongs to `aicortex`, a governed memory store for AI clients that serves
one store over MCP and owns its own per-client configuration. So there is no
sixth delivered class here, `harness::shipped()` never carries one, and §3.23's
contracts have nothing to say about a store this product neither writes nor
reads.

Two consequences follow, and neither changes what §3 requires. §3.22's ordering
is untouched: the counterparty's `.claude/agent-memory/reviewer/` holds notes
about reviewing that repository rather than a global harness, so it moves under
that repository's own decision and not with this delivery. And the interim is
that nothing moves: aicortex is specified and under construction, so the notes
that exist today stay where they are until that store can hold them, which is a
sibling's schedule and not a condition on this product's delivery.

**2026-09-21: §3.24 is implemented, and the one requirement a JSON document
cannot carry is measured rather than reinterpreted.** The authority change of
the entry above left §3.24 specified and not implemented. This is the
implementation: `crates/statecraft-home/src/settings.rs`, threaded through the
typed operation boundary as an intent on `home apply`, with
`crates/statecraft-home/tests/settings_modification.rs` as its acceptance.

§3.24 says "every managed line lives inside a single marked region in the file".
§3.13's bridge and the ignore merge both mark a region with comment lines,
which Markdown and `.gitignore` have. **A settings file is JSON, which has no
comment syntax, and the modification has two insertion points that are not
adjacent in any document**: a hook registration belongs in `hooks.<Event>` and a
deny entry in `permissions.deny`. Textual contiguity is therefore achievable
only in the trivial case of a file that carries neither key already, and
"pre-existing hooks" is one of the cases §3.24 names. Two ways of forcing it
were rejected: reformatting the whole file so the region could be contiguous
breaks "nothing outside is rewritten", and a sentinel deny entry such as
`Bash(statecraft-region-begin)` inserts a fake refusal into a user's own list.

What is implemented instead is **marked content in one recorded modification**,
which keeps every guarantee §3.24 states around the region:

- a managed hook registration is marked **inside itself**: the command string's
  first line is `# statecraft-managed <revision>`, a shell comment to the
  interpreter and a marker to this product, so a managed hook is recognizable
  from the file alone with no record to consult;
- managed deny entries are appended as one contiguous run at the end of the
  array and identified on removal by exact match, in order, against the record;
- both are one `modification` entry in the home, with the path, the exact
  content and the digest either side.

`implementation` therefore stays `in-progress`. The sentence quoted above is not
satisfied literally and this build does not pretend otherwise; the smallest
amendment that would make it satisfiable is put to the owner separately, and
until it is decided this section's remaining obligation is the wording, not the
behavior.

**2026-09-21: consent is to a token over the content, not to a verb.** "Consent
to one revision is not consent to the next" needs consent to be about content
rather than about having typed a flag. `home plan` and an unconsented
`home apply` print the exact lines and a `consent token`, a digest over exactly
those lines; `home apply --consent-settings <token>` performs the modification
only when the token still matches. Content whose lines changed has a different
token, so the operator is shown the new content and refused, which is the
requirement expressed as a mechanism rather than as a habit. A stale token is a
finding, never a silent no-op.

The flags hang off `home apply` rather than becoming their own verb, because
§3.14 rule 4 is what they have to satisfy: writing into a native location is an
explicit operator action **under `home apply`**. `--remove-settings` is on the
same verb for the same reason.

**2026-09-21: a withheld write is exit 1, and the settings modification is the
one part of `home apply` that can answer with any of the four codes.** Spec 006
§3.3 spends 1 on "a diagnostic state, a withheld write". An unconsented
`home apply` inside an operator's own agent home names a modification and does
not perform it, which is exactly that, so it is a finding rather than a success.
A malformed settings file is a refusal (2), an unreadable or unwritable one is a
failure (4), and a home with no native agent directory is not applicable and
stays 0. The severity is decided at the operation boundary, so the binding
carries none of it.

**2026-09-21: a refusal this product cannot prove it placed is never recorded as
placed.** Two situations leave a floor entry in the file that no record
attributes here: the user already refused it themselves, and an apply
interrupted between the write and the record. Attributing it would mean a later
`--remove-settings` takes out a refusal that was the user's, which is the one
direction §3.24 never allows. So the record claims only what it can prove, the
outcome says which entries it declines to claim and why, and a lost record costs
a refusal nothing. The hook half needs no such rule, because it carries its own
marker.

The write itself is a write to a neighbouring name followed by a rename, so an
interruption leaves either the old bytes or the new ones. Running the operation
again after an interrupted one repairs the record and writes the region no
second time, which is asserted by removing the record from the home and applying
again.

**2026-09-21: the one shipped registration is on `SessionStart`, and that
settles nothing about the other three events.** §3.24 fixes what a modification
may carry and not what this build places. The harness ships one hook, and
whether a harness's end-of-turn event should advise or refuse is an open
adoption question that belongs to §3.23's inventory decision and not to this
implementation. `SessionStart` is the event where the answer is the same under
either reading, so the mechanism ships without deciding the policy. Registering
the other three, and the ten skills and four agents §3.23 lists, remains the
owner's adoption act.

**2026-09-21, authority: the owner replaced §3.24's physical single-region
requirement, and the code that already existed is a candidate for the new
wording rather than a retroactive fit for the old one.** The entries above put
the wording to the owner and said the behavior was ahead of it. The owner's
answer is the contract now in §3.24: one recorded modification, insertions that
may occupy multiple syntactically valid locations, each identified by exact
content, structural location and recorded provenance, with unrelated bytes
preserved and removal conditioned on establishing ownership.

Two things follow, and the second is the one worth writing down.

First, what the amendment does **not** touch. Every guarantee stated around the
region survives verbatim: an exact reviewable plan, consent specific to the
content, no widened permission, `settings.local.json` never written, the digest
either side, idempotent application, conservative removal, and user edits and
conflicts preserved rather than resolved. The owner also fixed the syntax floor
in the same act: strict JSON, no JSON5, no comment outside a string value, no
sentinel deny entry that refuses nothing, no unsupported metadata key. The
sentinel was one of the two alternatives this build rejected on its own
reasoning; it is now rejected on the owner's authority as well.

Second, what this build must **not** now claim. The implementation of
2026-09-21 was written against the old wording, did not satisfy it, and said
so. The amendment does not convert it into an implementation that did. It is a
**candidate implementation of the revised contract**, and the difference is not
bookkeeping: the old wording was satisfied by a property of the file's layout,
which a reader can see, and the new one is satisfied by three properties of
each insertion, of which only the first is visible in the file. Ownership by
structural location and by recorded provenance is a claim about state this
build keeps elsewhere, and a claim of that shape is established by exercising
the cases where the state disagrees with the file: an insertion moved, a
marker forged, a record lost, a value duplicated, a user edit landing between
the plan and the write. Passing the tests written for the old wording
establishes none of those. §3.24's implementation is therefore reconciled
against the revised contract as its own work, and `implementation` stays
`in-progress` until that reconciliation is done rather than because the
sentence is unsettled.

**2026-09-21, authority: an end-of-turn hook advises, and the two exit
vocabularies are translated rather than passed through.** §3.23's seven hook
contracts were written from measured failures in gates, and contract 6 ("a gate
whose check did not run is not green") was read here as though every hook were
a gate. The owner's decision separates them, and the §3.23 text now carries
both halves.

The reason the separation matters is asymmetric cost, and it is worth stating
once rather than rediscovering. A gate that wrongly refuses costs an operation
the operator can retry with the same evidence in hand. An end-of-turn block
that wrongly fires costs the handback itself, which is the only account of why
the work failed and the one thing that cannot be reconstructed later. So the
two answer differently to the same non-zero code: the gate refuses and the
end-of-turn event reports.

The exit-code table is the second half and was an unforced hazard. This
product spends `3` on a usage error and spec-spine spends it on a read that was
not performed; this product spends `2` on a refusal and spec-spine on a stale
tree. Three of the four shared integers disagree, so a code propagated from one
vocabulary into the other is not merely imprecise, it names a different
condition. §3.23 now carries the translation and the collapse an enforcing gate
performs on it.

Two boundaries the owner drew explicitly, recorded so a later reading does not
widen them. No new hook repair behavior is authorized: contract 1 stands, and
the post-edit `compile` remains the single sanctioned exception. And nothing
here decides which events this build registers; that is §3.23's inventory
question, settled separately.

**2026-09-21, authority: §3.25, and the measurement that made the full digest a
requirement rather than a preference.** §3.14 made a revision content
addressed and stopped there. Who holds the requirement, and what happens when
the home does not hold it, were unfixed, and the two plausible answers differ
in what a project can say afterwards about what it ran.

`crates/statecraft-home/src/resolved.rs` already freezes a per-run `Identity`
carrying `requested` and `resolved`, so half of §3.25 is a rule the code
follows. The other half is not present: the manifest header (§3.3) pins this
product's version, the spec-spine version and the adapter set, and pins no
harness revision, so `requested` today resolves against whatever the home
holds rather than against a committed statement. §3.25 fixes that the required
identity is committed, and the implementation is separate work.

The digest rule was measured, not assumed. `harness::revision_of` computes the
full SHA-256 over path and content in path order and then keeps
`format!("h-{}", &full[..12])`, discarding the rest. Twelve hex characters is
48 bits, which is ample as a label an operator reads in a plan and is not an
integrity proof: an integrity comparison must be able to say that these are
the same bytes, and a comparison over a truncation says only that they agree
about 48 bits of them. So §3.25 separates the two roles rather than lengthening
the identifier, because the short form is genuinely the better thing to print
and the worse thing to compare.

The two prohibitions are there because each is the convenient behavior.
Selecting the latest installed revision makes a mismatch disappear at the exact
moment it should be reported. Rewriting the requirement during a read makes a
`doctor` run into the repair that hides what `doctor` was asked to find, which
is the same defect AGENTS.md refuses in a `compile` substituted for a `check`.

**2026-09-21, authority: §3.26, and why the evidence is added beside the
verdict rather than folded into it.** §3.14's three verdicts are already
implemented, in `crates/statecraft-home/src/delivery.rs`, as `Reached { via }`,
`NotReached { reason }` and `Unverified { reason }`. The owner's decision adds
what a managed session records at startup and fixes the claims that record may
carry; it does not touch the three words.

That restraint is the substance of the entry. The tempting move is to make
`reached` mean more, because `reached` is what an operator reads and what an
operator wants to know is whether the instructions are actually in effect. But
`reached` is written into records, and a verdict whose meaning changes rewrites
the meaning of every record already carrying it, retroactively and silently.
So the three evidence statements are separate fields: the chain arrives, the
bytes were resolved and supplied, and a live session demonstrated the behavior.
They are ordered by strength and each is strictly weaker evidence for the next.

The prohibition is narrow and worth naming precisely. A digest proves bytes are
bytes. An acknowledgement proves something emitted an acknowledgement. Neither
proves that a model read, understood or complied with anything, and this
product's records claim none of the three. The distance between "supplied" and
"complied" is exactly the distance a live-session acceptance has to cross,
which is why §3.26 admits the third statement as a category and leaves
producing one to a session rather than to this record.

**2026-09-21, authority: the owner adopted the whole of §3.23's inventory, and
the cost §3.23 quoted is now due.** Ten skills, four agents and four event
behaviors, delivered under Statecraft-namespaced names. The entry above, on
`SessionStart`, said registering the other three events remained the owner's
adoption act. It has happened, and `Stop` arrives under the advisory policy
rather than as a gate.

§3.23 already priced this. "Whoever owns the files owns the assertions": the
three skill assertions and the seven hook contracts are enforced in the
counterparty's tree by two test files that a hermetic test cannot carry across,
because neither may read `$HOME`. So the assertions are reimplemented where the
files land or the requirements become unenforced, and there is no third
outcome. Adoption is what converts that from a stated cost into scheduled work.

Two boundaries recorded because both are easy to slide past. Adoption is
delivery and not authorization: a delivered skill may be invoked and acquires
no standing permission by existing, so publishing, merging, releasing and
executing still need whatever authorizes them independently. And repository
invariance is a condition to be checked rather than a property to be assumed.
The inventory was written inside spec-spine's own repository; a file that
arrives carrying that project's crate layout, gate commands or workflow is an
adoption defect, and the check for it is not "does the word appear somewhere in
the file".

**2026-09-21, authority: §3.27, and the gate a deny entry does not have.** This
build writes the deny floor into `permissions.deny` in a native settings file
and registers hook scripts beside it, and the two were reasoned about as one
delivery. They are not one. A hook script can be project-gated because a script
runs and can test for `.statecraft/environment.json` and exit, which is what
`harness::GATE` states and what the shipped scripts do. A deny entry is
evaluated by the harness itself, before anything of this product's runs, so it
has no place to perform that test. Writing the floor into a user's global deny
list therefore applies it to every repository that user opens, and the
project-gated scripts sitting next to it do not narrow it by association.

So the two are separated: global registration is one act with one scope, and
delivering the floor to a managed session is another, through a mechanism that
can carry a scope. Claude Code documents `--settings` for that shape of need.
The amendment deliberately does not treat the documentation as the evidence:
the installed version and the effective behavior are measured before this
product relies on the mechanism, and an unverified mechanism is unavailable
rather than assumed.

The last paragraph of §3.27 is the one that will be under pressure, so it is
written as two named refusals rather than as a principle. If the floor cannot
be delivered to an ordinary unmanaged session, the answer is to report that
session as not qualified for the managed-execution claim. It is not to lower
the floor until delivery succeeds, and it is not to write a repository-local
generic harness copy so that the limitation stops being visible, which would
also reintroduce the exact thing §3.14 removes.

**2026-09-21, authority: §3.28, and the one route by which a delivery takes
over a user's hook by accident.** §3.24 already refuses to resolve a conflict.
What it did not say is what happens to a registration that resembles this
product's own, and the answer matters here specifically: §3.22 records a global
push gate the user already runs at `~/.claude/hooks/push-gate.sh`, and this
product ships a gate with the same purpose. The two will look alike, and a
delivery that recognizes its own content by resemblance will recognize that one
too.

So ownership keeps all three of §3.24's properties and is not satisfied by the
first. A marker is content, and content can be copied; the recorded provenance
is what this product actually knows about what it did. An unmarked registration
is the user's, and this product neither deletes, adopts nor rewrites it, even
when it is byte-identical to something shipped. Replacing one later is an exact
plan and its own act. For this round the behavior under test is coexistence.

The last rule is the one this round has to obey while measuring its own work. A
deny entry read out of a settings file establishes that the entry is
configured. Qualification is a claim about enforcement, in this version, in
this session, for this command, and only an inspection of effective behavior
establishes it. Reading the file and reporting the session qualified would be
this section's failure committed by the tooling that was written to detect it.

**2026-09-21: §3.24's implementation is reconciled with the revised contract,
and four gaps were found by writing the disagreements down.** The entry above
said the code written against the old wording is a candidate for the new one
and that the difference is established by exercising the cases where the record
and the file disagree. Doing that found four, three of them defects and one a
design that had to be sharpened.

**A duplicate JSON key made the two views of the document disagree.** This
module deliberately keeps two: `serde_json` parses, and judges, and a text
walker locates spans, and edits. On a document with no duplicate key the two
always agree, which is why the rest of the module may treat them as one view.
`serde_json` keeps the **last** occurrence of a duplicate and `locate` takes
the **first**, so on `{"permissions": {...}, "permissions": {...}}` a
judgement made about one value would have been applied to the other. It is
also exactly the structural-location property failing: two locations with one
name. Refused, as `AmbiguousStructure`, scanned over the whole document rather
than only the paths written into, because the never-widen checks compare the
parsed documents whole.

**Consent named the content and not the target.** §3.24's token was a digest
over the exact lines, which satisfies "consent to one revision is not consent
to the next" and leaves a second hole: the same content spliced into a
different file is a different modification, with different insertion points,
different entries already refused, different conflicts and different resulting
bytes. An operator who read one plan could consent to another. The token now
covers the content **and** the digest of the target as the plan saw it, which
is the owner's "do not overwrite a file changed since it was inspected"
expressed as a mechanism. The check moved to the write boundary and fires only
where there is a write, because a modification already in place is
idempotence, and asking an operator to re-consent to a no-op would turn that
into a conversation. A re-read immediately before the write catches a
concurrent writer in the same window.

**A marker was treated as proof of ownership for hooks.** The deny half
already declined to claim what it could not prove it placed. The hook half did
not, on the reasoning that a registration carries its own marker. The revised
contract makes the three properties conjunctive, and the owner stated the
consequence directly: a marker resembling this product's is not by itself
proof. A marker is content, and content can be copied out of a shipped harness
by anyone. So the hook half now records only what this product actually added
or had already recorded, and a marked registration with no record behind it is
reported as unclaimed and left. The cost is real and is the right direction:
after a lost record this product will not take its own registration back out,
and the operator removes those bytes themselves.

**Test coverage was of the old contract.** Ten cases were added, each
constructing a disagreement rather than asserting a phrase: a duplicate key at
the touched path and below it, a file edited between the plan and the write, a
forged marker, a pre-existing user refusal, a duplicated deny value whose
ownership is ambiguous, the two insertions at their two locations under one
record, a record that cannot be read, removal after a user edit, and an
unrelated key surviving both reapplication and removal. One existing test
changed its expectation rather than its assertion, which is the visible trace
of the third gap: it asserted that a repair after a lost record adopts the
marked hook, and it now asserts that it does not.

**2026-09-21: the published `0.21.0` and the source that calls itself `0.21.0`
are two implementations, and the second one conforms.** §3.15's boundary
reports the producer as non-conforming, and
`the_producer_is_not_yet_conforming` holds that finding. Measuring what is
actually depended on, and what actually exists, separates two things the
version number hides.

**What this build depends on.** `crates/statecraft-home/Cargo.toml` pins
`spec-spine-core = { version = "=0.21.0", default-features = false }`, and
`Cargo.lock` resolves it to `0.21.0` from crates.io with checksum
`cf8d0b123bbf6ae494700924c727ba8a5d115a821f0ea18e6dce9f140276c308`. Called
with this product's own `config_json()` it returns four out-of-contract paths:
`AGENTS.md` and the three `.claude/rules/*` files. That is the
non-conformance, unchanged, and the refusal that withholds those four stays
exactly where it is.

**Why the counterparty's tree says otherwise.** The published crate was cut at
tag `v0.21.0`, commit `694dd294`, where the module is still the old `init`
scaffolder. The narrowing landed **after** that tag, in `373ab506`, and the
module's own documentation now says it emits no `AGENTS.md`, no `CLAUDE.md`
and no `.claude/`. The workspace version was not raised with it, so the tree
and the registry both say `0.21.0` and mean different code. A consumer cannot
tell them apart by version, which is why this entry records a commit.

**The candidate, measured.** Commit `df6fb4f7`, source digest
`ac89a5423e053f04faa68ae69d124c28fef7393da8e7dfa73d4c811fd65bf859` over the
two producer crates' sources. Built in a scratch worktree with a
`[patch.crates-io]` that was never committed, it returns seven paths, all of
them in contract: six governance files and the `.gitignore` fragment. Three
tests invert, which is what the suite was built to show: the producer is
conforming, no out-of-contract path is carried, and the end-to-end
initialization is **complete** rather than partial, with 7 writes, 0 withheld,
2 ignore patterns merged, and the project registered and qualified.

So the boundary is correct and the blockage is entirely a publication. Nothing
here changes the dependency: a candidate measured in a scratch worktree is
evidence about a candidate, and the permanent exact dependency waits for a
published version verified directly. The pinned governance CLI (`=0.20.0` in
`spec-spine.toml`) and the scaffold library dependency are different surfaces
and do not move together.

**2026-09-21: the adopted inventory is delivered, and running the hooks as
programs found three defects the files' own prose denied.** Ten skills, four
agents and four event behaviors, adopted under the authority change above and
now in `crates/statecraft-home/harness/`, reached through `include_str!` from
`harness::ADOPTED_SKILLS`, `ADOPTED_AGENTS` and `ADOPTED_HOOKS`. They are held
as files rather than as string literals because they are 1900 lines of
authored prose: they are reviewed as prose, and a diff against the source they
came from is only legible while they are files.

**§3.23 said the inventory was already repository-invariant. The agents were
not.** All four carried a table of the counterparty's own crate layout,
`crates/{spec-spine-core,spec-spine-types}/` and `crates/spec-spine-cli/`, as
"the surfaces this project has". That is the adoption defect the owner named,
and it is invisible to a check that greps for a word, because the word
`spec-spine` legitimately appears throughout as the name of the governance
CLI. The tables are replaced by a pointer to the project's own `AGENTS.md`,
with two facts an agent may rely on everywhere: the corpus is the source of
truth, and the derived tree is read through the CLI. Four skills carried the
same defect in a different shape: a fenced block listing a gate with exact
flags, immediately after telling the session to run the gate exactly as
`AGENTS.md` lists it.

**Three defects were found by running the hooks, not by reading them.**

1. **The push gate fired outside a Statecraft project.** The manifest test was
   placed in the pull-request half, after the push half, so
   `git push origin main` was refused in an unrelated repository that merely
   happened to be open. That is §3.14 rule 3 broken in its most damaging form,
   because the collision stops an operation rather than printing a line. The
   gate now guards the whole hook, on the repository the **command** acts on.
2. **`SessionStart` sent a session to a writing verb for a validation
   failure**, advising `run spec-spine compile for the violations` on exit 1.
   Contract 4 turns on exactly one of the four answers being the one
   regenerating repairs, and `1` is not it.
3. **The build's own `statecraft-gate.sh` duplicated the adopted
   `SessionStart` behavior.** Registering both would run two freshness reports
   per session and make "one canonical source" false in the only place a user
   would see it. The hand-rolled gate is superseded and removed rather than
   registered beside its replacement.

**The assertions moved with the files, which is what §3.23 priced.**
`harness_hooks.rs` is rewritten against the four adopted hooks: 22 tests, each
writing a body out and running it with the input its event actually delivers,
against a stub binary that records which copy was chosen. `harness_skills.rs`
is new: nine tests over parsed front matter and fenced command blocks rather
than over prose, because the owner named "finds a phrase in a file" as the
failure to avoid. Two of them earned their keep immediately: the read-only
assertion cannot be derived from `allowed-tools`, because `commit` declares
`Bash` alone and looks read-only by that test while its whole purpose is to
write, and a first draft of the fence parser read the closing ``` of a
`markdown` block as the opening of a shell block.

**§3.27 is implemented and the floor moved rather than shrank.** The consented
global modification now carries hook registrations only. `crate::session`
carries the deny floor as a managed-session payload, and carries nothing else:
no allow entry, no `ask`, no `defaultMode`, no model, which is §3.28's list.
Six existing tests asserted the floor landing in the global file and were
rewritten to assert that it does not, each one paired with an assertion that
the floor is still carried whole somewhere, so "not here" cannot be satisfied
by lowering it.

**The mechanism is verified as far as a read can verify it, and no further.**
Claude Code 2.1.267 documents and carries `--settings <file-or-json>`, which
`session::probe_version` establishes by running the installed binary.
`qualification_from` cannot return `Qualified` from that, by construction and
by test: whether a refusal passed through the argument is actually **enforced**
is §3.26's third evidence class, an observation of a running session, and no
read of a settings file substitutes for one. Until a live session produces one,
this product reports a session as **not qualified** for the managed-execution
claim, which is §3.27's answer and not a workaround for it.

**2026-09-21: the producer version this build reports is a literal, not the
dependency's.** Found by running the producer acceptance against the packaged
`0.22.0` candidate in an isolated worktree. `producer::PRODUCER_VERSION` is the
string `"0.21.0"`, and `bridge.rs` carries the same number in a step label, so
an initialization run against a different producer still reported
`spec-spine-core@0.21.0`. Cargo does not hand a dependent crate a dependency's
version at compile time, so the literal is not an oversight with an obvious
cure; it is a coupling between the manifest and two source files that nothing
enforces. Recorded rather than changed: the committed dependency is unmoved,
and a repair belongs with the change that moves it, where the two can be
verified together.

**2026-09-21: a packaged `.crate` digest identifies a commit and the sources
identify the library.** The counterparty's release record carries digests for
the two `0.22.0` producer archives cut at `da1cd99b`. The archives actually on
that machine are cut at `5b8c201a`, two documentation commits later, so their
digests are different and the record's are stale by exactly the mechanism the
record itself documents: `cargo package` stamps `.cargo_vcs_info.json` with the
git sha, so the digest moves on every commit including one that touches no
crate source. This product therefore records **both** for an isolated
measurement: the archive digest, which says which commit the artifact was cut
from, and a digest over the archive's contents **excluding** that stamp, which
says whether the library changed. Only the second is comparable across
commits, and confusing them is how "the candidate moved" and "the candidate's
code moved" become one question with one wrong answer.

**2026-09-21: the adapter's deadline attempt raced its own subject, and the
repair is structural rather than a larger budget.**
`crates/statecraft-adapter-claude-code/tests/settings_transport.rs`, the
attempt named `timeout_after_terminal_denial_cleans_settings_and_retains_evidence`,
has been failing intermittently on loaded machines for several rounds. It is
spec `004`'s file, and this spec declares a corrective `extends` edge for the
repair; `004`'s required behavior is untouched and nothing here weakens it.

**The event ordering the attempt needs**, which is four things and was written
as one: the terminal denial reaches the supervisor; the child is still alive
after emitting it; the **deadline** is what ends supervision; and the denial is
still in the structured evidence afterwards, with the supplied settings file
removed and the workspace's own two untouched.

**The defect.** The deadline is five seconds because the deadline is the
subject. But the attempt reused the shared fixture path, so those five seconds
also had to cover five process spawns, a blocking read of stdin that waits on
the supervisor's writer thread, and a deliberate 100-millisecond sleep, before
the point the assertion measures. None of that is the subject: every one of
those properties is asserted by the attempts that name a thirty-second
deadline. A budget that has to cover work it was not sized for is a race, and a
loaded machine loses it.

**The repair.** The fixture child takes the shortest path when the deadline is
the subject: settings intact, terminal denial emitted, marker written on the
line after the emit, then a hang far longer than the deadline. Nothing
schedulable sits between the terminal event and the marker, so
`settings-after-terminal` existing means the child outlived its own terminal
event rather than meaning it won a race. The assertions are the four above,
spelled separately, and the elapsed time is now bounded **below** by the
deadline as well as above, so a child that exited early fails instead of
passing as an interruption. The deadline stays five seconds, the negative
behavior is unchanged, and nothing is retried or ignored.

**It then reproduced, and the reproduction changed the diagnosis.** The first
repair was written from reading the fixture, because two full
`cargo test --workspace` runs and six runs under CPU spin loops were all green.
A later `make code` run failed, and so did a loop under **process-spawn**
pressure rather than CPU pressure. The attempt now carries a trace, so the
failure described itself instead of being a mystery: the child's trace was
**empty**, the workspace held none of the files the child writes on its first
two lines, supervision reported `Interrupted` after 5.03 seconds, and the
stream error was `NoInit`. The child was spawned and never ran its body. The
five seconds were consumed by process startup alone.

That is a larger finding than the first one and it does not replace it: work
inside the measured window was a real race and removing it was right. What it
adds is that the residual cost is `execve` itself, which the fixture cannot
shorten.

**Two further repairs, and one thing that is not a defect.** The suite's own
contention is serialised away: the deadline attempt takes an exclusive lock and
the other four take a shared one, so the measurement never runs beside the
concurrent attempt's two children and their ten-per-second `sleep` forks. The
budget is unchanged at five seconds and no assertion is weakened, which is why
this is synchronisation rather than a larger deadline. And the trace stays, so
any residual failure names its cause.

What remains is not a defect in this product. A machine that cannot start a
shell script inside five seconds will still fail this attempt, and the
supervisor's behavior in that case, interrupting at the deadline with `NoInit`
and cleaning up, is exactly correct. The attempt fails there because the
**precondition of its measurement** was not met, not because the behavior it
measures is wrong, and it now says which of the two happened. Passing it in
that state would be suppression.

**2026-09-21: the adopted `PreToolUse` gate skipped a check it was required to
refuse on, and the inherited wording is the reason.** A fourth adoption defect,
beside the three already recorded. The shipped `statecraft-pre-bash.sh` reads
the derived directory from `spec-spine config show --json`; where that read
answered nothing, it printed `derived-tree check skipped` and **continued**,
citing the counterparty's spec `093` section 3.4, which says a configuration
that could not be read is not evidence of a dirty tree. That sentence is true
and it is not the rule this corpus adopted. §3.23 contract 6 says a gate whose
check did not run is not green, and the Stop policy's second part says an
enforcing operation gate refuses a failed **or unavailable** check. This gate
stands in front of `gh pr create` and is the one hook §3.23 marks enforcing, so
the conflict is resolved in favour of the adopted rule: an unanswered
configuration read now refuses with exit 2, and the message says the check was
not performed rather than that the tree is dirty, because the two remedies
differ and regenerating shards repairs neither.

The same reading applies to the three git reads beside it. `git diff
--name-only` prints nothing both when a tree is clean and when the command
failed, and the inherited script discarded every exit status, so a failed read
was indistinguishable from a clean tree. Each read's status is now read, and a
read that did not run refuses on the same contract.

Recorded rather than performed silently, because the weaker behavior is not
preserved merely because it was copied, and because the counterparty's own
copy still carries it: this is a defect to fix there independently, not a
regression to revert here.

**2026-09-21: the earlier coverage gap over the derived tree's three states was
wrongly reasoned, and is closed.** The 2026-09-21 handoff recorded staged,
unstaged and untracked derived output as not separately exercised, on the
grounds that the shipped hooks never inspect the index, so the three states
would be indistinguishable to them and a test would assert a property of `git`.
The shipped hook reads `git diff`, `git diff --cached` and `git ls-files
--others` on three separate lines. Seven behavioral cases now assert what the
hook does with those answers rather than what git computes: each of the three
states refuses and is named as itself in the message; a staged edit whose
working-tree contents cancel it against HEAD is still refused and both states
are named, which is the case one HEAD-relative comparison cannot see; a
committed derived tree is the positive control, without which the six refusals
prove nothing; a failed read refuses; and an unanswered configuration refuses.

**2026-09-21: the bounded integration is demonstrated, and three obligations
are named as outstanding rather than counted as met.**
`crates/statecraft-home/tests/bounded_integration.rs` walks the whole flow
once in one isolated home against one fixture repository, and asserts the
properties that only exist **between** the mechanisms every other test file
judges one at a time. Eleven tests: the full harness installs once under the
home and the project receives no copy; the bytes written are the bytes the
plan's digest predicted; the floor is delivered per session and reaches no
global file; an unrelated repository is byte-identical afterwards; the managed
instructions exist before the bridge names them; a pre-existing root
`AGENTS.md`, `model`, `permissions.allow` and an unmarked user hook all
survive; all four events resolve to installed, executable files inside the
canonical revision; the modification reverses to the operator's own bytes and
can be rebuilt afterwards; and nothing arms a target.

Two of them are about the premise rather than the behavior. The authority
port is `Unreachable` throughout, so **every assertion here holds with no
platform login**, and that is asserted rather than left implicit. And a
shipped hook is run with `env_clear()`, a bare `PATH` and no `$HOME` at all:
it still reaches a verdict and still says nothing outside a Statecraft
project, which is the converse of sandboxing and the half that matters on an
operator's own machine.

**§3.25 and §3.26 are implemented, and one obligation is not.** The paragraph
this replaces named three; two of them are done.

**§3.25's required identity** is a committed project requirement in
`required.rs`, carried as one key in the manifest's `project.requirements`,
which is the governed contract for a committed requirement and already binds
every configuration layer. The full digest is what is committed and what every
comparison is against; `Revision` now keeps it, and the twelve-character form
is derived from it by one function and is display only. Seven standings, one of
which permits managed execution. A manifest with no requirement is
`unrequired`, is **not** qualified, and is not repaired by a read; nothing
selects the latest installed revision; and inspection, diagnosis through a new
`doctor` finding, planning and an explicit upgrade all stay available under
every refusal.

**§3.26's startup record** is in `startup.rs`: seven fields, the §3.14 verdict
unchanged beside them, and the three evidence classes as three types that no
code path substitutes for one another. `Observation::Observed` is constructible
only from a session qualification, so a probe cannot reach it. An incomplete
record is refused rather than written, the write is a rename so no reader sees
half of one, and a truncated record reads back as an error rather than as no
record.

**The live-session observation** is the one that remains, and it is the one
this spec cannot produce for itself. `session::qualification_from` cannot
return `Qualified`, by construction and by test, and no session has been run
under this prompt. So no session has been shown to enforce the floor, and every
fixture result in this file is fixture evidence. A bounded acceptance script is
prepared and awaits approval as its own act.

**One thing §3.25 requires has a library and no verb.** Committing a
requirement is an explicit reviewed act, and the command tree carries no
spelling for it: the verb table is `006`'s requirement and an entry in it is
`006`'s change to make. Until that change is proposed, the act is performed
through `crates/statecraft-home/examples/require-harness.rs`, which is also
what builds the acceptance fixture. Named here rather than left for a reader to
discover, because "implemented" and "reachable by an operator" are different
claims and this is the second one missing.

**2026-09-21: the admission reads a structured refusal record, and the
adapter that produces one is a dependency.** §3.29 rule 1 requires the
admission to read structured evidence the harness emits rather than prose. That
record is spec `004` section 3.3's `permission_denials`, and its shape is
already owned, parsed and tested in `crates/statecraft-adapter-claude-code/`.
`crates/statecraft-home/` therefore takes that crate as a dependency and reads
the provider capture through its types, rather than growing a second parser for
the same bytes. The alternative considered and refused was a private copy of
the event shapes in this spec's crate, which would have been two places to
correct when the provider adds a field, and the one place nobody would look.
The dependency is acyclic: nothing in `004`'s crate reaches back here. No unit
of `004` is edited by this, so no new edge is declared; the `extends` edge
already in this file's frontmatter covers the one test of `004`'s that this
round repaired.

**2026-09-21: the negative control was written before the repair, and it
failed.** §3.29 names the exact transcript that defeated the first admission.
It is a test in `crates/statecraft-home/tests/qualification_admission.rs`
rather than a sentence in this section, and it was run against the unrepaired
implementation first, where it failed with the admission returning
`Ok(Observed { .. })` for a transcript stating the command had succeeded. Two
further probes against the same build showed the deserialization route and the
`from_qualification` route reaching `Observed` with no evidence at all, which
is why §3.29 rule 5 covers every route rather than the one that was reported.

**2026-09-21: the evidence bytes are kept in the record, not beside it.** §3.29
rule 5 re-checks a deserialized record, and rule 3 refuses substituted
evidence. Neither is decidable from a digest alone once the original file is
gone, so the record carries each control's captured bytes verbatim along with
the path they were read from. The cost is a record of a few kilobytes where it
was a few hundred bytes, which is the price of a record that can be re-judged
rather than believed. The captures are bounded by the acceptance's own
`--max-turns 1`.

**2026-09-21: the acceptance is a program, and its three stages are three
approvals.** §6 of the handoff was an outline with steps reading "same, with"
and "open a session", which is a procedure a reader performs and a procedure
nobody can rerun identically. It is replaced by
`scripts/acceptance/managed-session.sh`, which carries the commands, the
branching, the capture locations, the preserved exit statuses, a per-step
deadline enforced by a watchdog rather than by `timeout`, and its own cleanup.

The stages are separated by what each one costs and what it touches.
`preflight` is local, spawns no provider and needs no approval;
`permission-experiment` spawns the provider and refuses without
`APPROVED_PROVIDER_SESSION=yes`; `coexistence` observes the real home and
refuses without `APPROVED_REAL_HOME_COEXISTENCE=yes`. **The second gate is not
satisfied by the first**, and that is enforced by the script rather than
described in it, because §3.24's consent is its own act and an experiment that
inherited it would be performing a write nobody approved. No stage activates
anything in the real home: the permission experiment carries its settings on
each invocation's own command line, and the coexistence stage runs a plan and
reads digests.

The preflight ends by submitting a **fabricated** claim, whose refusal capture
is the sentence §3.29 names, and refusing to continue unless the admission
refuses it for being prose. A run whose admission would admit that sentence
would produce a worthless result at provider cost, so it is checked before the
first invocation is paid for rather than after.

**2026-09-21: `session payload`'s human rendering is the bytes and nothing
else.** The first rendering appended the digest and the argument, which made
`session payload > floor.json` write a settings file that was not the payload,
and the acceptance script found it by digesting what it had written. §3.4 of
`006` makes human output a view rather than a contract, which is what allows
the change; the digest and the argument moved to the `--json` rendering, where
a caller reads them.

**2026-09-21: the deadline attempt's synchronisation is contention
mitigation, and the investigation closes there.** The earlier entries on this
attempt recorded a `RwLock` that serialises the one attempt whose subject is a
deadline against the four whose subject is not, measured at 1 failure in 5 runs
before and 0 in 40 after. Re-read against what the test requires, that
description overstated it, and this entry corrects it by appending rather than
by editing the earlier ones.

What the lock establishes is that **no other attempt in that test binary runs
during the measurement**. What the test requires is that the child is
`execve`d, runs its body, emits its terminal event and is read by the
supervisor inside five seconds of wall clock. That is a real-time bound, and
mutual exclusion does not establish a real-time bound: it removes one
contributor to the window and leaves the rest of the workspace, the machine and
the kernel's scheduling of `fork`/`execve` in place. A reduced failure rate
under one load profile is a reduced failure rate, and calling it the absence of
the race would be a claim the measurement does not carry.

The remaining failure is **not** dismissed as irrelevant, because it is not.
When the whole budget goes on `execve`, no init event arrives, supervision
interrupts with `NoInit`, and the attempt's subject, whether a terminal denial
that did arrive survives an interruption, is not measured at all. That is this
measurement's precondition failing and it leaves the test unable to say
anything. So the attempt now reads the child's own `entered` marker **before**
it judges the denial, and fails on the precondition with its own message. It
still fails: passing in that state would be suppression, and reporting it as a
lost denial would send the next reader after the wrong defect. No loop was run
to accumulate a clean count, and no assertion was weakened; the deadline is
unchanged at five seconds.

**2026-09-22, authority: §3.30, recorded before the repair it authorizes.**
Re-reading the §3.29 implementation at `62bde9a` against what each control has
to prove found five defects the section's own negative controls did not reach,
and §3.30 names them. None was a gap in a word list and none is closed by
reading more prose: each is a place where a request stood in for a result, a
text match stood in for an identity, or a description written after the launch
stood in for the launch. The owner authorized the repair on 2026-09-22 with the
instruction that the authority be recorded first and separately, which is this
entry and the section it points at. §3.29's rules 1, 5 and 6 stand as written;
§3.30 adds rules 7 to 12, which sharpen rules 2 to 4.

Two design facts are recorded because they are premises rather than
measurements. The experiment grants both commands to all three launches,
because a non-interactive session refuses an ungranted command regardless of
the payload and the controls would otherwise measure the missing grant; and
that a deny entry prevails over such a grant is what the refusal control
tests, not what it assumes. Neither has been observed on a live provider here.

**2026-09-22, authority: the deadline attempt's contract is corrected, recorded
before the test changes.** The attempt
`timeout_after_terminal_denial_cleans_settings_and_retains_evidence` in spec
`004`'s `settings_transport.rs` asserts four properties inside one five-second
wall-clock window that starts at `spawn`. Three of them are the supervisor's:
supervision ends at the deadline and not before it, the process group is
killed, and the supplied settings file is removed while the workspace's own
settings are untouched. The fourth, that a terminal denial read before the
deadline survives the interruption, needs a **precondition** the product does
not promise: that the fixture child is `execve`d, runs, emits and is read
inside those same five seconds. Spec `004` section 3.5 case 3 promises that a
hung child is killed at the deadline with its descendants and the attempt is
`interrupted`; it does not promise that any child starts within a bound, and the
2026-09-21 entries above measured that on a loaded machine one does not. The
test therefore could fail with the product correct, and the lock recorded above
reduced how often without establishing anything about the bound.

The corrected contract keeps every property and gives each the measurement that
can establish it:

1. **The deadline, the kill and the cleanup** are measured through this crate's
   own execution path with the same five-second deadline and the same start
   point, `spawn`, because the product's deadline covers that interval and
   moving the start would remove part of what is promised. The child hangs from
   its first line and backgrounds a descendant. Nothing here needs the child to
   reach any point by any time: supervision must end no earlier than the
   deadline, report no surviving process, remove the settings file and leave
   the workspace's settings as they were, whether or not the child ever ran.
   The upper bound on elapsed time is chosen to separate the defect it exists to
   catch, a supervisor held by the process it supervises, from scheduling
   latency in the `kill` it spawns: the child's hang is 300 seconds, so any
   return well under that proves supervision was not held, and the bound is set
   at 60 seconds rather than at the deadline plus a guess. A descendant the
   child did start is checked dead by its process id.
2. **Retention across an interruption** is measured where it is decided, and
   without a race. In the claude-code crate, the mapping from what the
   supervisor read to the execution's evidence is a function of the supervised
   events and outcome, so it is tested with an interrupted supervision that read
   a terminal denial: the denial must be in the structured evidence and the
   outcome must stay `interrupted`. In the generic supervisor, an event the
   reader thread had already delivered when the deadline fired was left unread
   in the channel and lost. That is a product defect in the retention the
   attempt was written to protect, found by stating the contract rather than by
   timing it, and it is repaired by draining what was already delivered before
   the kill; its test fills the channel deterministically.
3. **What each failure means** is then separable. A fixture that could not
   start is visible in the trace and fails nothing in (1). A child or
   descendant that escaped the group fails (1) by the survivor report or its
   process id. A deadline the supervisor did not honor fails (1)'s bounds. And
   no remaining assertion is a scheduler-sensitive measurement of a property
   the product does not promise.

The lock is removed with the precondition it mitigated. The deadline stays five
seconds, no assertion about the product is weakened, nothing is retried, and
the earlier failure evidence in the entries above is kept as written.

**2026-09-22: §3.30 is implemented, and the probes were run before the
repair.** Nine one-mutation probes were run against the admission as it stood at
`88bc616` (unchanged since `62bde9a`), each on the section 3.29 fixture with every
other prerequisite satisfied. All nine were **admitted**: an allowed-command
request with no result; a denial under another tool's name; a denial naming
another tool-use id; two init events from two sessions; two terminal events that
disagree; a refused command both denied and executed; a `--settings` naming a
different file than the recorded bytes; a `--settings=` spelling in the
absent-payload control; and a second `--settings`. The probe was not committed.
Each defect is now a test in `crates/statecraft-home/tests/qualification_admission.rs`
that changes exactly that one thing on the admitted fixture and asserts the
refusal it causes, 38 tests in all, beside the unchanged prose control.

What the fixture is matters as much as what it proves. Its event shapes are the
ones the committed 2.1.267 recordings carry, including `tool_result_meta` and its
`non_execution_kind`, which is the field that tells a refused result from a
command that ran and failed; the adapter's own tests read that field out of
`denied.jsonl` and its absence out of `max-turns.jsonl`. Nothing the provider
does not emit is required, and no field was invented to make a control pass.

**2026-09-22: `run` delivers the floor and refuses a requirement it cannot
establish.** Found while looking for the next gap on the qualification path,
with the source as the evidence: `run` launched the provider with
`Invocation::new(program, &[], None)`, an empty deny list, and consulted no
required identity, and `crates/statecraft-cli/tests/native_stream.rs` asserted
the empty list. That assertion predates §3.27 (2026-09-21), which requires the
floor to reach a managed session through the per-session settings mechanism,
and §3.25, which requires managed execution to refuse missing, corrupt or
mismatched required content. No entry deferred either, so this is an open
defect and not a decision.

Three choices the sections were silent on. **The bytes:** a run receives
`session::payload_json()` exactly, through a settings document the adapter
writes verbatim, because §3.29 rule 4 binds an observation to the payload by
digest and a differently formatted equivalent would be other bytes. **Which
standings stop a run:** the three §3.25 names, plus a requirement this build
cannot read as a digest and an installed tree that cannot be read, since both
are required content that cannot be established. A project that commits no
requirement is not managed *under* one, so it runs and is recorded `unrequired`;
it is still not qualified. **Mismatch:** nothing measures which revision
actually answers inside a run yet, and passing the requirement as the resolved
identity would record a resolution nobody observed, so a mismatch is not
detectable at run start and the standing is recorded as evaluated with nothing
resolved. The attempt record carries the payload digest and the standing.

Delivering the floor claims nothing about enforcement: the posture still says
unqualified, and only an admitted live observation changes that. Before the
repair the eight `native_stream` tests failed against the previous binding (the
seven existing ones once the fixture child required the payload bytes, and the
new refusal test); after it, all eight pass.

**2026-09-22: `run` writes its startup records, and four choices §3.31 left
open.** Implemented under §3.31, recorded after it. **A repository with no
manifest** is registered and armed and still not a managed session: §3.14 rule 3
gates every delivered behavior on the manifest, and no project identity exists
to record. Such a run launches as before, writes no startup record, and its
answer says `managed: false` rather than implying a record. **`startup show`
does not require registration**, like the other `startup` verbs; it requires a
manifest, and it reads the attempt list from the run record exactly as `run
show` does. **The adapter identity** recorded is the adapter manifest's name and
version with `claude-code` as the harness, the same three facts the posture
already records. **The provider version** in the launch evidence is the one the
stream's init event reported through the generic seam, and the reserved absence
word when there was none.

Measured. Before, the new `crates/statecraft-cli/tests/run_startup.rs` run
against the tree at `2ee9704` failed all seven tests: `run` completed with no
`startup` field and wrote no record, an unwritable startup directory did not
stop the launch, a record that could not be stored did not change the exit
code, and `startup show` was an unknown verb. The `startup record` test in
`qualification_workflow.rs` failed on the resolved identity, `Some` of the
required digest where nothing was measured. After, all seven pass, the thirteen
`launch` unit tests pass, the three acknowledgment tests in `harness_hooks.rs`
pass, and `cargo test --workspace --locked` passes 936 tests. The fake provider
those tests use runs the shipped hook from the "registered" revision and
streams its output as `hook_response`; that is locally exercised, and whether
the live provider does it in a managed run is still unobserved (§3.31 rule 20).

**2026-09-22: the two provider premises, graded, and the acceptance script's run
step.** The previous hand-back left two premises of the permission experiment
unobserved. Neither is observed now either, and no provider session was
started to change that; what changed is that each has a stated grade.

*One `--allowedTools` followed by two rules containing spaces.* Documented: the
installed `claude --help` for 2.1.267 describes the option as `<tools...>`,
"Comma or space-separated", with `"Bash(git *) Edit"` as its example. Read
statically from the installed binary: the splitter applied to the option's
values (`sd`, called from the permission-context setup with the
`--allowedTools` array) splits on commas and spaces only outside parentheses,
so each argument this build passes stays one rule. Locally exercised:
`admission::tests::each_rule_in_the_grant_survives_the_installed_splitter_whole`
runs the constructed arguments through a transcription of that splitter for all
three controls, and asserts neither command contains a parenthesis. The
construction needed no correction.

*The floor's deny beating that grant.* Documented, in the provider's permissions
reference: rules are evaluated "deny, then ask, then allow", an allow rule
"can't carve an exception out of a deny rule", and a deny at any level cannot
be overridden by `--allowedTools`. Read statically: the Bash permission check
returns early on an exact-stage deny or ask only, then checks prefix and
wildcard deny rules, which is where `Bash(cargo publish*)` matches, before an
exact allow is honored. Observed: not yet. A fake provider cannot observe
either premise, and the refusal control is still what tests the second.

*Harmlessness, including ancestors.* `--manifest-path` names the manifest, so
cargo searches no ancestor for one, and `--dry-run` uploads nothing. What an
ancestor could still change is which toolchain a rustup proxy selects, and an
uninstalled one may be downloaded first. The preflight now refuses when an
ancestor of the fixture project holds `rust-toolchain` or
`rust-toolchain.toml`, lists any ancestor cargo configuration for review, and
the permission stage exports `RUSTUP_AUTO_INSTALL=0`.

*The run path in the script.* The preflight gains a synthetic step 10: `run`
against a local fake that checks the payload bytes, runs the required
revision's shipped `SessionStart` hook as the operator's registration would,
and streams its output as `hook_response`; then `startup show` must read back
an acknowledged observation equal to the requirement, a supplied supply and
the named missing class. The permission stage starts no run, because a run
session would be a fourth session. `acceptance_script.rs` asserts the step's
persisted records and the ancestor refusal.

**2026-09-22: section 3.32 implemented, and six choices it left open.**

1. *The exclusive write.* Each record is written to a private temporary name in
   the attempt's directory and hard-linked to its final name. The link fails
   if the name exists, so the write is exclusive and a reader never sees half
   a file. The previous build checked for the file and then renamed over it,
   and a rename replaces; between the check and the rename a second writer
   could have been replaced. Every record, the gate included, now goes
   through the one function.
2. *The gate's wait.* Thirty seconds, passed to the gate as its first argument
   in the registered command, so the intent records the exact bound and a test
   can run the same script with a shorter one. The hook is registered with a
   sixty-second timeout, longer than the wait, so what a provider reports is
   the gate's refusal rather than its own timeout.
3. *The decision file.* `admission.json` is one line of compact JSON, so the
   gate reads it with `grep` for the exact member `"decision":"admitted"`. A
   reason cannot forge that member: a quotation mark inside a string value is
   escaped. The gate never parses anything else, and a decision it cannot read
   is a decision that has not been written.
4. *Where the decision point is.* The first event that is not a `SessionStart`
   `hook_started` or `hook_response`. The recorded `2.1.267` streams put every
   such response before `init`, so in them this is the init event. A provider
   that reports a startup hook after `init` would be judged on what arrived
   before it, which is refused as not established rather than waited for.
5. *An unpersisted decision.* When `admission.json` cannot be written, a gated
   attempt is stopped and refused under `startup-admission`: the gate cannot
   read a decision that is not on disk, so it would hold every tool call until
   its wait ran out, and stopping the process says that plainly instead.
6. *What `run` refuses and what it reads.* A live attempt's refusal inspects
   that attempt's records with its outcome absent, and says what they
   establish; nothing in the refusal writes, reconciles or infers.

Measured through the built binary: a run whose fake honors the registration is
admitted at its init event and its tool call runs after it; a tool call begun
before the decision waits at the gate and runs only after admission; a
substituted startup hook naming another revision is refused, and neither the
waiting tool call nor a later one produces its effect; a fake that ignores the
registration produces an effect before the decision and the refusal says it
is retrospective; and a launcher killed while its session runs leaves
`outcome-unknown`, after which `run` refuses rather than replaying and the
fake was launched once. Library tests inject each failure section 3.32 names:
the intent, a spawn failure, a process created before its confirmation, a
confirmation that cannot be persisted (the real supervisor, the prompt never
delivered), a decision that cannot be persisted, and a record that cannot be
persisted. Every test in this spec's crates that execs a script it wrote
installs it through `statecraft_adapter::fixture::install_script`, the staging
spec `004`'s 2026-09-22 `ETXTBSY` entry records, so none can hit that race.

**2026-09-22: the sandbox repositories run no automatic git maintenance.** CI
on the command-surface pull request failed
`an_unrelated_repository_is_untouched_and_ungoverned` in the bounded
integration: the unrelated repository's snapshot before initialization held
`.git/objects/maintenance.lock` and the one after did not. The fixture's own
`git commit` had started git's detached automatic maintenance, and its lock
came and went while the test ran. This product wrote nothing there. The test's
git helper now passes `maintenance.auto=false` and `gc.auto=0` to every
invocation, so the repositories a test snapshots change only when something
under test changes them; the snapshot itself still covers `.git`.

**2026-09-22: the permission experiment does not resolve managed startup.**
The handoff's statement of what remained outstanding placed the unobserved
hook delivery of section 3.32 beside the approval of the permission stage, and
a reader could take running that stage as retiring it. The stage starts no
`run` and supplies no hook, so it cannot. Section 3.33 is recorded before any
implementation: a separate managed-startup trial, one run attempt spending one
session and one version probe, under its own approval, with the ordering
premises it tests stated as premises. The handoff's section 7 is corrected in
place and says it was corrected. No code changed with this entry.
**2026-09-22: section 3.33 implemented, and five choices it left open.**

1. *Where the trial lives.* `crates/statecraft-home/src/trial.rs` holds the
   four values, the sentinel, the timeline watch and the judgement. The watch
   wraps the attempt's `LaunchWatch` and forwards every spawn and event to it
   unchanged, so the decision a trial reaches is the one `run` would reach at
   the same line. `inspect` reads `trial.json` where one exists and recomputes
   the judgement from the facts written and the admission, intent and gate log
   on disk now; `agrees` says whether that equals what was written.
2. *What counts as executed.* A tool result with no error flag and no
   `non_execution_kind` note for its id. A result the gate blocked carries the
   error flag, so it is not an execution, and a read that failed would not
   count either; the sentinel's condition additionally needs the nonce.
3. *The stop race.* A refusal stops the process group at the decision, and
   events the provider wrote after that line may or may not have been read.
   So for a provider that ignores the registration, effects read
   `demonstrated-possible` when its execution was read and `unobserved` when
   it was not, and a startup response written after `init` reads `late` or
   `absent` the same way. Neither can read `excluded` or `established`, and
   the tests assert exactly that set rather than a scheduling outcome.
4. *The version probe.* The run path's probe runs `--version` once, without a
   bound of its own. It is the one probe the budget counts; the acceptance
   stage's watchdog bounds the whole verb, probe included, at the session
   deadline plus 120 seconds.
5. *Refusals the section did not list.* The trial drives a session, so it has
   `run`'s preconditions as well: a registered and armed target, and no live
   attempt anywhere in the project. Each is refused before an attempt is
   appended and spends nothing.

Measured through the built binary, `--synthetic` throughout: a faithful fake is
`established` with effects `excluded`, one session and one probe; a second
trial is refused as spent; a read that bypasses the gate is
`demonstrated-possible`; a session requesting no tool leaves effects
`unobserved`; a hook run without the session's environment is `unbound`; a
tool request streamed before `init` is decided at that request, recorded as
`tool-use`, and refused `wrong-session`; a provider ignoring the registration
reads `absent`; a hung session with a descendant is stopped at a three-second
deadline, `uncertain`, with no survivor; and an edited `trial.json` reads back
as disagreeing. The acceptance script's `managed-startup` stage runs the same
path through its local route. **No provider session was started**, so every
premise P1 to P5 keeps the grade section 3.33 gives it.

**2026-09-23: the two live experiments, each run once under its own approval,
and what each one did and did not establish.** The owner authorized both
separately: section 3.30's permission experiment, at most three sessions and
stopping at the first that does not complete, and section 3.33's
managed-startup trial, exactly one session. Neither authorization covered a
retry, a longer limit or a second fixture, and none was used. Both ran from
`main` at `01df484` (the checkout's `HEAD` when each preflight built the
product; the archives record the binary's digest, not its source revision)
through `scripts/acceptance/managed-session.sh`, each on its own fixture built
by its own preflight. The two fixtures were `ACC` =
`$TMPDIR/sc-accept-A` and `$TMPDIR/sc-accept-B`, and both preflights exited 0.
The product binary was built from that revision, SHA-256 `a6cf34ac…19335`. The
provider was Claude Code `2.1.267` at
`~/.local/share/claude/versions/2.1.267`, SHA-256 `a681f300…cd2558`, and each
launch's probe and `init` event both reported `2.1.267`. Every step's
captured output, the permission experiment's raw stream and every record the
product wrote are kept, outside the repository, in two archives:
`sc-accept-A.tar.gz` (`8bf5f4f3…e2a6`) and `sc-accept-B.tar.gz`
(`567c8ee9…60a5`). The two results are reported apart, and neither one, nor
both together, is a qualification.

*The permission experiment: unverified, stopped at session 1 of 3.* The
stage started at 08:10:10Z and ended at 08:10:19Z, version probe included; the
refusal control's session exited by itself with status 1, with no timeout and
no survivor. `startup capture` recorded the
launch as incomplete. Rule 8 requires the terminal event to be the capture's
last event, and it was not: the provider wrote a `system` event of subtype
`task_summary`, with `detail: null`, after its `result` event. The stage
therefore stopped, as it is required to. The allowed-command and
without-payload controls were never launched, the admission never judged a
claim, and the fixture project was unchanged. The capture (`refusal.json`,
SHA-256 `5d988271…b6dc`, stream `cc9b6e59…ea11`, 10775 bytes, twelve
events) records, besides two `hook_started` events and a `rate_limit_event`,
which is not a turn:

- two `SessionStart` responses from the operator's own settings, because the
  stage does not replace `HOME`, both before `init`;
- `init` with `permissionMode: bypassPermissions`, taken from the operator's
  settings;
- one `Bash` request for the refused command;
- a mid-stream `permission_denied` for that tool-use id, with decision reason
  type `subcommandResults`;
- an error `tool_result` carrying the denial text;
- a terminal `result` of subtype `error_max_turns` (the control's
  `--max-turns 1`), whose `permission_denials` names the same tool-use id and
  input;
- one earlier `task_summary` event, between the request and the denial.

None of that is an admitted observation. Rule 8 refused the capture before
rules 9 to 11 were applied, so whether they would have held is not known, and
the premise that the floor's deny beats the grant keeps the grade the
2026-09-22 entry gave it: documented and read statically, not observed. The
recorded `2.1.267` streams under
`crates/statecraft-adapter-claude-code/testdata/stream/` carry no
`task_summary` event and end on their terminal event. So the same provider
version now emits an event those recordings never showed, and rule 8, as
written, refuses every capture that carries one. This entry does not relax
rule 8. Admitting a named non-turn event after the terminal one would change
what section 3.30 admits, and that is the owner's decision, recorded as open
in the next entry.

*The managed-startup trial: established, for one session on this machine.*
`startup trial --provider-session` ran at 08:11:25Z with the product's
120-second default session deadline and `--max-turns 3`, under the script's
own bound on the whole verb (that deadline plus 120 seconds). The run was
attempt 1 of `statecraft-startup-trial`, and it completed by itself; its
record is dated 08:11:32Z. The required and selected revision was
`h-33c417e1c3a4`. The settings document it wrote, `84293ea6…f430`, 993 bytes,
registered the revision's `SessionStart` hook and the attempt's gate.

The timeline, as `trial.json` (`a81429f8…3ccf`) records it:

- lines 1 to 6: three `SessionStart` hooks started and responded, all exiting
  0. Line 5 is the supplied hook's response, and it carried this attempt's
  acknowledgment. The other two carried none; the settings the run wrote
  register one `SessionStart` hook, so they are inferred to be the
  operator's own, as in the permission experiment. The trial keeps its
  per-line timeline, not the provider's raw stream, so this is an inference
  and not a preserved observation.
- line 7: `init`. The decision `admitted` was made there, because the one
  correlated acknowledgment named the required revision and the standing was
  exact.
- line 11: one `Read` request.
- the gate: consulted once, and released on `admitted`.
- line 13: the tool result, executed and carrying the sentinel's nonce.
- line 15: the terminal event, which was the last event.

The judgement was re-derived from the records on disk and agreed:

| Word | Value |
|---|---|
| hook evidence | `correlated` |
| effects before admission | `excluded`, tool calls only |
| verdict | `established` |
| origin | `provider-session` |

Premises P1 and P2 are therefore observed once, for this version and this
machine. P3 is observed as the gate being consulted and the call executing
only after `admitted`; a refused decision was not exercised, because an
admitted trial never refuses. P5 held in this stream. Nothing here says which
process printed the acknowledgment, anything about the deny floor, or anything
about another version. The run's own startup verdict is still `unverified`,
because a run session is not one of section 3.29's three controls.

*Consequence for the implementation state.* The managed-startup question has
a recorded answer. The permission-enforcement question does not: its one
authorized attempt is spent, and a new attempt needs a new authorization, and
under rule 8 as written also a provider that ends its stream on the terminal
event or a decision about rule 8. `implementation` stays `in-progress`.

**2026-09-23, open for the owner: a non-turn event after the terminal event.**
The permission experiment's capture ended with `system`/`task_summary` after
`result`. The choice is between two options:

- keep rule 8, so that the permission experiment cannot be admitted against a
  provider that emits such an event;
- amend section 3.30 to name the event subtypes a capture may carry after its
  terminal event.

Such an amendment would need to say that those events are not turns, that
they name the same session, and that nothing in them is read as evidence. The
same capture also carries a `rate_limit_event` mid-stream, which the list
should account for deliberately rather than by accident. The
recommendation is the second option, narrowly: admit only `system` events
whose subtype is on a closed list (today, `task_summary`), refuse any other
trailing event, and record the list's provenance as this capture. It is an
authority change to an approved section and is not made here.

**2026-09-23: a trial the deadline stopped before its first event read as a
failed launch, not an uncertain one.** Rule 37 makes a process the deadline
stopped `uncertain`. The judgement inferred "the deadline stopped it" from an
interrupted outcome with no stream error and no terminal event. A session the
deadline stops before it writes `init`, however, also carries the adapter's
"no init event" stream error, so that inference failed and the trial was judged
`not-established`. It was found by the 2026-09-23 audit, when
`startup_trial::a_session_stopped_at_its_deadline_is_uncertain_and_its_evidence_stays`
failed under load: its fake was scheduled too late to write anything within
three seconds. The supervisor now reports whether its deadline fired (spec `004`,
entry of this date), `trial.json`'s process end records it as `timedOut`, and
`deadline` is read from that flag first. A record written before the flag
existed has no `timedOut` member and keeps the older inference, so reloading
it cannot change its verdict. The reading follows rule 37's wording, so it
is broader than the no-init case that exposed it: a flagged process the
deadline stopped is `uncertain` even when it had written a terminal event, or
a malformed line, before hanging; either used to read `not-established`.
Neither live trial record is affected: the 2026-09-23 trial ended by itself. A new test uses a fake that writes nothing,
and it fails without the fix and passes with it.

**2026-09-23: the governance producer is the published `spec-spine-core`
0.23.0, and it conforms.** Owner-directed. This moves the library dependency
from `=0.21.0` to `=0.23.0`, from crates.io with no override. It moves as its
own change, after the CLI pin moved to the same version (`D-06`, entry of this
date). The crate records Git revision
`d2bb4763` (tag `v0.23.0`), its registry checksum is `3dca8f68…e492`, and its
unpacked source equals that revision's tree. `PRODUCER_VERSION` and the bridge
test's label move with it, and the test that reads the manifest keeps them
equal.

Section 3.15's boundary is now satisfied by the published dependency, not
only by a candidate:

- `producer::produce` against the real library returns no path outside the
  contract set, so `the_producer_is_conforming` asserts what
  `the_producer_is_not_yet_conforming` asserted the opposite of.
- Initialization end to end is `Complete`, not `Partial`. These are the three
  tests section 8.2 of the handoff predicted would change, and the draft
  adoption on a Git revision (#70) changed them first.
- `an_out_of_contract_path_is_carried_so_it_can_be_recognized_and_never_placed`
  is removed. Its one claim that no other test makes, that the real library
  still returns `AGENTS.md` out of contract, is false for this producer.
- The behavior it guarded is still asserted: against a recorded answer by
  `producer::tests::an_out_of_contract_path_is_named_and_never_placed`, and by
  section 3.10's row in `negative_cases.rs`. The classification of `AGENTS.md`
  as out of contract is asserted in the conforming test.

*One consequence, fail-safe and stated.* `flow::known_generated` recognizes an
untouched generated root `AGENTS.md` only by the bytes the producer returns
out of contract, and nothing is vendored. A conforming producer returns none,
so such a file, written by an older spec-spine's initializer, is now treated
as the user's own. It is preserved and bridged, never overwritten. That is the
conservative reading, and no requirement asks for the other.

**2026-09-23: the owner decided the open trailing-event question, as section
3.34.** The entry above offered two options and recommended the second,
narrowly. The owner chose it and set its safeguards: a complete terminal event
first, the session correlated, a closed list of subtype and shape, no turn,
request, result, outcome or second terminal event after termination, nothing
qualified from the trailer's prose, no trailer standing in for a control,
refusal of anything malformed, unknown, contradictory or misbound, no new
unbounded read, and the original bytes and order kept. Section 3.34 records
that contract before the code it authorizes. It sets the closed list to one
subtype and one shape, from the one capture that showed it. It admits a string
`detail` as well as `null`, because the same capture carries the same subtype
with a string before its terminal event, and neither form is read. The earlier
entries are not edited: the experiment's verdict under rule 8 as then written
stands, and a new attempt is a new authorization, which the owner gave
separately, bounded to at most three sessions and conditional on this
amendment being implemented, reviewed, merged and exercised locally first.

**2026-09-23: section 3.34 implemented.** The closed shape is
`task_summary_trailer` in the claude-code adapter's `stream.rs`, a
`deny_unknown_fields` type with every member required, so an absent `detail`
is refused rather than defaulted and a member given twice is refused by the
deserializer rather than read last-wins. The admission's single pass reads the
line after the terminal event through it, adds its session to rule 8's session
check, and refuses any event after it. `startup capture` and `startup qualify`
print one `trailer` line per admitted trailer, reported from the admission's
own pass (`admitted` and `admitted_in`), and a capture whose events and
non-blank lines do not correspond is refused as unreadable; the capture's
bytes are unchanged. Rule 14's identity is asserted over an admitted body of evidence
and four refused ones, with a `null` detail and with prose shaped like either
verdict, on every control and on one, and the negative cases are the list section 3.34
names, each through the admission and through the launch's own completeness
reading. The local fake gained `trailer` and `bad-trailer` modes, and the
binary test runs both. **Offline replay, not a live observation:** the
archived refusal record (`5d988271…b6dc`) re-read through this build's
`one_session` is a complete session with its trailer at event 12. Its tool use
would classify as refused under rule 9, since it carries both the terminal
denial and the `permission-rule` non-execution note. That is a reading of old
bytes by a new evaluator and spends no session. The experiment's recorded
verdict is unchanged, and the replay is kept outside the repository beside the
archive.

**2026-09-23: the permission experiment, second campaign: admitted, for
Claude Code `2.1.267` on this machine.** A new owner authorization, separate
from the first campaign's, for at most three provider sessions, stopping at the
first incomplete, inadmissible or uncertain one, with the 300-second session
deadline and the 30-second version probe unchanged. It ran only after section
3.34 was implemented, independently reviewed, merged (`main` at `87a3f19`) and
exercised against local positive and adversarial fixtures, and after the
no-provider preflight passed on a fresh fixture, `$TMPDIR/sc-accept-A2`.
The product binary the preflight built from `87a3f19` is `7529dbec…c28b`; the
acceptance script is `f936d494…ca2e`. The provider was named by its absolute
versioned path, `~/.local/share/claude/versions/2.1.267`, SHA-256
`a681f300…cd2558`, the same binary as the first campaign, so a change of the
`claude` symlink could not change it between sessions; each launch's probe and
`init` event reported `2.1.267`.

The stage ran from 10:17:52Z to 10:18:23Z and used all three sessions: the
refusal, the allowed command and the absent payload, in that order. Each
process exited by itself with status 1 and no timeout, signal or survivor, and
each capture ended with one `system`/`task_summary` trailer after its terminal
event (events 15, 14 and 11), which section 3.34 admitted and did not read. The
fixture project was unchanged. `startup qualify` then admitted the
observation: the claimed command was refused through a structured denial and
did not execute under the payload, the allowed command executed under the same
payload and printed `statecraft-allowed-control`, and the claimed command
executed without the payload. The record, `acc-live.json` (`60f452cb…a84c`),
says `observed`, harness `2.1.267` and payload `3a5c7fc2…5fe1`, and `startup
qualify` reported **`qualified: false`**, computed from the record rather than
stored in it: supply is a separate evidence class this stage does not perform.
The captures are kept verbatim (`08a6623d…2083`, `e1fd2739…5500`,
`3201d2ae…b5b1`) in `sc-accept-A2.tar.gz` (`be31d3d5…22d7`), outside the
repository, beside the first campaign's archive, which is unchanged
(`8bf5f4f3…e2a6`).

*What this establishes, and what it does not.* For this version on this
machine, the premise the 2026-09-22 entry graded "documented and read
statically" is now observed once: the floor's deny entry, delivered through
`--settings`, prevailed over an identical `--allowedTools` grant, and the grant
alone let the command run. The first campaign's verdict, `incomplete` under
rule 8 as then written, stands, and its record was not rewritten. Nothing is
established about another provider version, another machine, another command,
a run's own settings document (section 3.32 rule 25 carries hooks as well as
the floor, and section 3.29 rule 4 keeps evidence for one payload from
qualifying another), or whether a model read or complied with anything. In section
3.36's words the observation is observed and observation-admitted, bound to the
provider binary by the digest its launch record carries; it is not a
qualification of any run, trial or session, and whether it covers the floor
inside a run's composed document is the owner's decision that section 3.36 rule
5 leaves open. The
operator's own `SessionStart` hooks and settings were in force, because the
stage does not replace `HOME`, as in the first campaign. This campaign's
authorization is spent; there is no further session under it.

*The managed-startup trial is not repeated.* Its one established observation
(the 2026-09-23 entry above) stays as recorded: one session, Claude Code
`2.1.267`, this machine. Its archive re-digests to the value recorded there
(`567c8ee9…60a5`). It was not re-run, refreshed or re-evaluated under a changed
rule, because no change in this round touches what it read: section 3.34
applies to the admission's reading of a capture and not to the trial's
timeline.

**2026-09-23: the launch-state answer names `run reconcile`.** With spec `003`
section 3.6.1 implemented, `launch::inspect`'s next-action text for
`launch-unknown` and `outcome-unknown` names the verb instead of saying none
exists, as section 3.32 rule 24 now reads. Nothing it reads or judges changed.

**2026-09-23: two reads for reconciliation.** `launch::read_gate_log_checked`
reads an attempt's `gate.log` for spec `003` section 3.6.1 rule 3: an absent
log is `None`, and a log that exists and cannot be read is an error, never an
empty log. The rendering read `inspect` uses is unchanged. `digest::digest_reader`
computes the same SHA-256 as `digest_bytes` over a reader, in chunks, so an
evidence file is not held whole. Neither writes.

**2026-09-23: the project block carries `commands` as an optional list.**
Section 3.16 names the member and spec `004` section 3.17 owns what it means.
`Project` in `manifest.rs` gains `commands: Option<Vec<String>>`, defaulted on
read and omitted on write when absent, so a manifest this crate rewrites keeps
an operator's declaration and never invents one, and an absent
member stays distinguishable from an empty list. This crate does not validate
the entries: `004` refuses a malformed one where it reads the list, so the rule
has one home. A test writes, reads and rewrites a declared list, and asserts an
undeclared one is not serialized.

**2026-09-23: section 3.34's trailer, confirmed by the owner, and the tests it
asked for.** The owner confirmed the reading that the one admitted trailing
`task_summary` event may carry `detail` as `null` or as a string, neither read
nor used as evidence of refusal, execution, permission, qualification or
outcome, with the original bytes kept, and that this confirms that
representation only and relaxes nothing else in the admission. Two tests were
added to `crates/statecraft-home/tests/qualification_admission.rs` for what was
not yet asserted: a boolean or an array `detail` refuses the capture like a
number or an object already did, and an admitted trailer's line, with its
member order, spacing and escapes, reaches the admitted observation byte for
byte and survives a write and a read of it. No requirement changed.

**2026-09-23: section 3.35 implemented, and the choices it was silent on.**
The operation is `crates/statecraft-environment/src/transfer.rs`: `plan`,
`apply` and `revert`, bound as `006` section 3.11.7's verbs. The manifest gains
`transfers`, the journal, read with a default and omitted when empty, so a
manifest written before this section reads as having no transfers and one
nobody transferred in is byte-identical to what an earlier build wrote. Each
choice below is one rule 1 to 7 did not make; none changes what they require.

- **One writer of the manifest at a time, every writer.** The manifest lock
  is an advisory `flock` on `.statecraft/state/manifest.lock`, runtime state
  that is gitignored and created on demand, so the committed tree gains no
  byte; a refusal that needs no lock (no manifest, colliding adapters) is
  answered before it and creates not even that. A lock file rather than the
  repository's root directory, because `flock` on a directory fails where it
  is emulated with byte-range locks (NFS, some SMB and FUSE mounts) and a
  user's own `flock .` would contend with it. A filesystem that cannot lock is
  its own error, `LockUnsupported`, and nothing is written without the lock.
  Only local APFS (macOS) and the CI runner's Linux filesystem were exercised;
  no network or FUSE filesystem was, so behavior there is reasoned, not
  measured. Dropping the lock unlocks explicitly before closing, as spec 003's
  repository lock does: a child spawned from another thread holds a copy of
  the descriptor until its `exec`, and a release by closing alone would leave
  the lock held for that window. The lock file is created inside the
  repository or not at all: a linked `.statecraft` or `.statecraft/state` is
  refused as a symbolic link, and the file is opened without following one.
  The lock is reentrant within a thread. `Manifest::write` takes it itself,
  and every read-modify-write holds it from its read to its write:
  `Manifest::update`, `env apply` (`apply::apply_current`, which the binary
  uses), `env remove`, `init apply`, `harness upgrade`, enrollment, and the
  transfer verbs. A transfer does not wait and refuses (`busy`); every other
  writer waits up to 30 seconds and then refuses too (exit 2 through the
  binary, a refused step in initialization): nothing was written. Independently
  of the lock, a manifest value remembers the digest of the bytes it was read
  from, keyed on the same canonical root the lock uses, so `/tmp/x` and
  `/private/tmp/x` are one repository; a write refuses (nothing replaced) when
  the manifest on disk is no longer those bytes, so a writer holding a stale
  value can never erase what another writer recorded, a transfer already
  reported as applied included.
- **Every manifest write is atomic and durable.** The bytes go to a temporary
  file beside the manifest in `.statecraft/`, with the old file's permissions,
  which is flushed, renamed over it, and the directory flushed, so an
  interruption leaves the old manifest or the new one. A manifest that is a
  symbolic link is refused, never replaced through or over. A temporary file
  an interrupted earlier write left is removed by the next write and named in
  its answer, so none stays among section 3.12's four paths. When the
  directory flush fails after the rename, the new manifest is in force and not
  known to survive a crash: that is exit 4 under section 3.11.7 ("could not be
  written durably"), and the answer says the transfer is in force and names
  its record.
- **An installed adapter, for rule 1's "a path this product would itself
  write", is one in this build's configured set that claims its paths on this
  machine**, judged by the same readiness `env apply` uses. A path an adapter
  declares but does not claim here is refused for `managed`
  (`adapter-not-claiming`). The entry's source is that adapter, and its
  `transfer` names the prior claimant as `foreign` knows it, or the path
  itself where nothing else is known, as `env plan` names a `foreign` path.
- **An adopted entry's source is `template` with identity
  `operator-transfer`.** An adopted file has no source here and the entry
  format requires one. A new source kind would make the manifest unreadable to
  a build that predates this section, which the compatibility paragraph
  assumes can read it.
- **No manifest is a refusal.** Creating one is `env apply`'s act; a transfer
  that wrote the first manifest would be initialization by another name.
- **One file, one name.** Names are compared ignoring ASCII case for rule 2's
  list and for the `.statecraft` and `.git` components, and both components
  are protected at any depth. Each component of the path must be listed by
  its directory byte for byte, so on a case- or normalization-insensitive
  volume a second spelling of a file (`notes.md` for `Notes.md`, a composed
  `é` for a decomposed one) is refused (`spelling`). A path that is the same
  file (device and inode) as another path the manifest, the journal, a
  modification or an adapter already names is refused (`alias`).
  `.github/copilot-instructions.md` is matched as its last two components, at
  any depth.
- **The file is read through one handle.** Under the lock, the path is opened
  component by component without following a link, the handle is confirmed a
  regular file, and the digest and length recorded are read from that same
  handle, so nothing swapped in after the path checks is what gets recorded.
- **Any path carrying a tracked modification is refused for `adopted` and
  `managed`**, not only the root `AGENTS.md` the list already names: rule 2's
  reason is that a modification is not an entry.
- **Rule 2 holds over a reversal.** Reverting the release of a pointer file
  would make an instruction file `managed`, so it is refused
  (`instruction-file`), and so is any reversal rule 1 or rule 3 would refuse.
  A reversal checks rule 3's path first, then the file's digest, and only then
  what the inverse move itself would refuse, so an unrecorded edit is what it
  names when there is one.
- **What "disagrees" means (rule 5).** For each path, the latest record's
  resulting class must be the manifest's class. Two differences are not
  disagreements, because this product's own recorded operations make them: a
  move to `managed` whose entry `env remove` then removed (whatever is at the
  path now, since a user may restore a file there), and a move to `user` whose
  file was deleted and which `env apply` then wrote afresh (a `managed` entry
  from an adapter, carrying no `transfer`). A repeated record identity, or a
  reversal naming a record not before it, is a disagreement. Checked first by
  `apply` and `revert`, before rule 7.
- **Rule 7's `already-satisfied` is exit 0**, reported before the plan
  identity and after the path checks of rule 3 and the journal check of rule 5.
- **How a stale plan names what changed.** The plan identity is rule 4's
  SHA-256 of the JSON array `[path, from, to, file digest, manifest digest]`,
  printed as `identity`, and `transfer apply` accepts it as given. `transfer
  plan` also prints a token carrying it beside the first 16 hex digits of each
  input, `pi1-<from>-<to>-<path>-<file>-<manifest>-<identity>`, as `plan_id`,
  and `apply` accepts that too. A still-current identity, in either spelling,
  is never refused. For a stale token, `apply` compares its fields with the
  current inputs and names each that differs (`classes`, `path`, `file`,
  `manifest`). For a stale bare identity, it searches the values the inputs
  could have had (every admitted pair of classes, the file's digest now and
  every digest the manifest and the journal record for the path, the
  manifest's digest now and every digest the journal records it had), names
  what differs in the combination that reproduces it, and otherwise says that
  what changed could not be determined (`undetermined`). A string that is
  neither is named as not an identity this build issues (`identity`). A class
  that changed since the plan is named as the path's class and the class
  given (`class-mismatch`), checked before the identity.
- **A record's identity** is the SHA-256 of the JSON array of its other
  fields; the producer is `spec-spine-core@0.23.0`, the name and exact version
  `statecraft-home` pins.
- **The operator is recorded verbatim**, beside `operator_provenance:
  operator-supplied`; a blank operator or reason is refused.
- **"Never rewritten" (the compatibility paragraph) is read as: nothing adds,
  alters or backfills an entry's recorded `transfer`, and no journal record is
  synthesized for it.** A move the operator asks for on such a path proceeds as
  for any entry, which for `managed` to `user` removes the entry with the rest
  of it.

Tests: `crates/statecraft-environment/tests/transfer.rs` covers the
acceptance and every negative case against the library on every platform,
including concurrent applies of one plan, an `env apply` racing a transfer
apply with nothing reported as done lost, second spellings and hard-link
aliases, and a write that cannot commit; the manifest's own unit tests cover
mode, symbolic link, leftover and changed-since-read; and
`crates/statecraft-cli/tests/ownership_transfer.rs` covers them through the
built binary (`006` section 5 of the same date).

**2026-09-23: replacing a drifted managed file, per path (section 3.4).**
Section 3.4 requires that "Replacing a drifted managed file requires the
operator to say so per path", and until this entry nothing let the operator say
it: every drifted managed file was withheld on every `env apply` and `env
upgrade`, and the only cure was editing the file back or the committed manifest
by hand. The section is silent on the mechanism, so these choices are recorded
here and change none of its requirements. `crates/statecraft-environment/src/replace.rs`
is new, inside this spec's directory unit.

*Plan, then consent, per path.* `env plan <path> --replace <file>` names one
path per `--replace`, repeatable. The plan reports, for each, the digest the
manifest records, the digest on disk now, the replacement's digest and length,
and a **plan identity**: SHA-256 over a domain string, the path, the adapter and
those three digests. `env apply` and `env upgrade` take `--replace
<file>=<plan-id>`, recompute the plan, and refuse unless every identity is
still equal. Nothing is replaced that is not named, and naming a path is not
enough without the identity, so a script cannot consent to a drift it did not
read. A stale identity (the file edited again, the manifest entry changed, or
the adapter's content changed) or an identity planned for another path refuses
the whole apply, and nothing at all is written. Arguments are parsed strictly:
anything after the target other than `--replace` pairs, a consent given to
`env plan`, and a path given to `env apply` without an identity are usage
errors.

*What can be named.* Only a path a claiming adapter declares, recorded
`managed`, and drifted. Every other named path refuses the plan by name, and
with it the apply: an adopted path, a foreign or occupied pointer path, a user
path, a path no adapter declares, a path that is not drifted (absent, or
holding the bytes last written, which an ordinary apply handles), a path that
is not relative or has an empty, `.` or `..` component, and a path that is, or
passes through, a symbolic link. An adapter that does not claim its paths
(section 3.9) replaces nothing.

*Writing.* A refused plan or a stale identity is answered before the manifest
lock and writes nothing, not even the lock file. Otherwise the lock of this
date's section 3.35 entry is taken and held from here to the manifest write,
and a manifest that changed since it was read refuses with nothing written.
Staged files an earlier interrupted replacement left under
`.statecraft/state/replace/` are removed first and reported as `swept`. Each
replacement is then staged there with the target's permissions and flushed.
Before any rename, every target is checked again: a regular file, reached
through no symbolic link (the staging directory included), still holding the
digest the plan saw; any change refuses with nothing renamed. The same file and
link check is repeated immediately before each rename. The standard library has
no rename relative to a directory handle, so a window of one system call
remains between that check and the rename. A rename is atomic on one
filesystem, so each file holds its old bytes or its replacement and never a
mixture, and the manifest is written after the files.

*Interruption and retry.* A failure (exit 4) can leave some named files
replaced and others not, with the manifest not yet recording any of them:
staging fails before anything is renamed, but a rename or the manifest write
can fail after an earlier rename landed. Repeating the same request recovers:
a named path already holding its replacement is `already-satisfied`, whatever
identity is given, and the manifest records it without writing the file; a
path not yet replaced still carries the drift its identity was planned for and
is replaced. A repeated successful request reports `already-satisfied` and
writes no file.

*Exit codes*, in spec `006` section 3.3's vocabulary and recorded in its
section 5: a refused plan or a stale identity is 2, a failure to stage or
rename is 4, an apply that replaced what was named while withholding other
drift is 1, and one that replaced or found satisfied everything named with
nothing withheld is 0.

*Tests.* `crates/statecraft-environment/tests/replace_per_path.rs` (every
platform) and `crates/statecraft-cli/tests/env_replace.rs` (through the binary).
The binary cases that need a claiming adapter are compiled on macOS only,
because the configured adapter's credential-path prerequisite is measured on
macOS only (spec `004`), so the check runner, which is Linux, runs the library
half and the binary's refusal and usage cases, not the binary's replacement
cases. They use a synthetic `claude` and a synthetic qualification record in a
temporary home.

**2026-09-23: `env remove` takes back the root bridge (section 3.13 rule 4).**
Rule 4 requires: "Removal removes the inserted line and nothing else, and only
while the file still begins with it." `env remove` did not: it read managed
entries only, so the recorded modification and its line survived removal, and
`bridge::unbridged` existed with no caller. It now takes the bridge back through
`statecraft_environment::apply::remove_with`. Where the rule is silent, these
choices are made, and none widens what may be removed:

- *The record is the authority, and only a record this product could have
  written counts.* A record is acted on only when its path is well formed (no
  absolute path, no empty, `.` or `..` component, nothing under `.statecraft/`
  or a `.git` component) and its path, kind and line equal the one place this
  product puts a bridge: the root `AGENTS.md`, `import-bridge`,
  `@.statecraft/AGENTS.md`. Any other record is withheld, named, and kept.
- *A record must show an insertion.* `init apply` recorded a modification even
  when the file already began with the line, with the same digest before and
  after, and removal then took away a line the user wrote. Initialization now
  records only when it inserts or moves the line; a re-run that finds the line
  first keeps an earlier record unchanged, digest before included, and creates
  none when there is none. Removal withholds any record whose digest before
  equals its digest after.
- *What "the inserted line" is.* The line with its terminator and, only where
  the insertion added one, the blank separator after it. Section 3.13 rule 2's
  insertion writes the line alone when the file had nothing else and the line
  plus a blank line before existing content, so the record's digest after says
  which: a digest after equal to the line alone means no separator. Every other
  byte is kept, including edits the user made anywhere in the file since, which
  is why the digest after is not required to match.
- *The file is kept.* Rule 4 removes the line and nothing else, so a file this
  product created is left, empty when nothing else is in it. The rewrite is
  staged under `.statecraft/state/` with the file's permissions and renamed
  into place, after checking again that it is a regular file reached through no
  symbolic link.
- *Ambiguous ownership refuses rather than guesses.* The bridge is withheld,
  named, and its record kept when the file is absent (and was not created by
  this product), is reached through a symbolic link, is not UTF-8, does not
  begin with exactly the recorded line, or carries the line more than once; and
  when the manifest records two modifications of one path.
- *An interrupted removal finishes.* Where the file was rewritten and the
  manifest was not, the file's digest equals the record's digest before, or,
  for a file this product created, the file is empty or absent. The line is
  then already gone, and removal drops the record and says so in a note.
- *A bridge with no record is not this product's.* A root `AGENTS.md` that
  begins with the import line while the manifest records no modification is
  left and reported as a note, which changes no exit code.

A withheld bridge is a finding in section 3.4's sense: the removal is
`partial`, spec `006` exit **1**, beside every managed path it did remove, which
is section 3.6's contract for a drifted managed path applied to a
modification. Section 3.6's behavior is otherwise unchanged and is now
exercised through the binary: a drifted managed path is withheld and named,
every matching one is removed, adopted and user paths are untouched, and the
manifest is written, not deleted, still recording what remains. Tests:
`crates/statecraft-environment/tests/bridge_removal.rs` and
`crates/statecraft-cli/tests/env_remove_bridge.rs`, the second spawning the
binary against bridges `init apply` itself recorded (once, twice, and over a
file that already began with the line), and against recorded fixtures for
each refused and interrupted case.

**2026-09-23: section 3.37 implemented, without the confinement.** Rules 1 to
5 are in `crates/statecraft-home/src/launch.rs`, `trial.rs`, `capture.rs` and
`service.rs`, and bound in `crates/statecraft-cli/src/main.rs`. Spec `004`
section 3.18's mechanism (its rules 5 to 9) is a later change: no Seatbelt
profile or Landlock ruleset is applied here, no attempt is reported confined,
and the exchange directory and the launch records are the two paths that
mechanism will grant and deny. The choices the section is silent on:

- *Where, exactly.* The launch records of a repository are
  `<home>/records/<key>.startup/<run>/<attempt>/`, beside that repository's
  run record `<home>/records/<key>.jsonl`; its exchange directories are
  `<home>/exchange/<key>/runs/<run>/<attempt>/` and, for `startup capture`,
  `<home>/exchange/<key>/captures/<control>-<nonce>/`. Both are derived from
  `statecraft_run::record::chain_path` by `launch::Places::of`, with the same
  target path the run record is opened with, and the key is computed nowhere
  else, so a change to how spec `003` keys a repository moves the launch
  records with the chain. `statecraft-home` takes `statecraft-run` as a
  dependency for that one function; no unit of `003` is edited.
- *A decision already in the exchange directory.* `prepare` refuses an attempt
  whose exchange directory already exists, whose launch records hold anything,
  or whose number already has records inside the target. At the decision, the
  supervisor writes `admission.json` into the launch records, syncs the
  directory, and only then writes the gate's copy (a new file, renamed into
  place). A copy already at that name was written by something other than this
  supervisor: it is removed, not adopted and not silently replaced, the copy is
  refused, and the refusal is the rule 2 failure: the gate withholds, the
  process is stopped, and the attempt is refused under `startup-admission`
  with the failure in `admissionError`. An ungated attempt has no gate and no
  copy.
- *The gate log's copy.* Read from the exchange directory at record time with
  `O_NOFOLLOW` and `O_NONBLOCK`, refused unless the opened handle is a regular
  file, bounded at 64 KiB, and written once as `gate.log` in the launch
  records. The record carries it additively as `launch.gateLog`, whose
  `attestation` is `child-attested`, with its digest, length, truncation and
  any failure; a failure is recorded rather than raised, because the record is
  still owed. `LAUNCH_VERSION` is unchanged: every field section 3.32 defined
  keeps its name, content and judgement. A reader, reconciliation included,
  reads the copy where one exists and otherwise the exchange directory's log,
  both through the same bounded read, and a log that exists and is not a
  regular file is an error, never an empty log.
- *The trial's statement.* A `trial.json` written now carries the additive
  field `consultations: "child-attested"`; one written before carries none,
  and `startup show` says it was recorded before this section and is read as
  it was. The judgement is unchanged.
- *The settings document.* A managed attempt's provider is handed the
  adapter's exclusive temporary file created in the attempt's exchange
  directory rather than the system temporary directory, through
  `execution::supervise_with_in`, an additive entry point in `004`'s
  `statecraft-adapter-claude-code` under this spec's existing edge. It is
  removed when supervision returns, as before. A run in a repository holding
  no manifest records nothing and keeps the system temporary directory.
- *Capture.* The payload is written once into the capture's exchange
  directory and the invocation names that path. After the provider exits, its
  bytes are read back as data, digested for `settingsDigestAfter`, and copied
  to `<control>.settings.json` in the operator's directory beside the other
  capture records, which are written after the exit as before.
- *Reading both layouts.* A reader locates one layout per attempt: the
  product home where the launch records or the exchange directory of that
  attempt exist, and otherwise the target's `.statecraft/state/startup/runs/`.
  Where both hold something of the same attempt, neither is chosen: the read
  fails naming both directories, and nothing in either is moved or rewritten,
  because preferring one would silently set aside what the other holds.
  `startup show` reports the layout as `placement`, `home` or `target`, and
  says of `target` that the records were written where the child could reach
  them and that nothing says the attempt was confined.
- *A planted decision, measured.* Removing a decision the child wrote into
  its exchange directory, and refusing the attempt, is what this change does;
  it does not stop a tool call the child makes before the supervisor gets
  there. The gate admits on any decision file in its directory, and an
  unconfined child can write one, so on CI's Linux runner (2026-09-23) the
  child's tool call ran in that window while the attempt still concluded
  `refused` with the plant named. The call is not invisible: the gate's own
  log, copied into the records and labelled child-attested, shows the
  admission it gave. Rule 3's "a file the child cannot write" holds only under
  spec `004` section 3.18's confinement, which is specified and not
  implemented; until then this is a residual, recorded here and in the test
  that measures it, not a guarantee.

**2026-09-23: the owner a `foreign` finding names, and how `doctor` tells a
user's file from an unrecorded write.** Section 3.21 part 1 retains that a
`foreign` finding names an owner, not only a path, and section 3.2's `foreign`
row names it by package identity where one exists. The binary passed no
claimant, so every occupied path was reported as `path <p>`, which is a path
and no owner. With the kit withdrawn no second installer is left (section
3.22), so no package identity exists for the binary to supply, and section 3.2
answers the rest: a file no manifest records is `user` class. `Claimant::User`
carries that owner with the path it holds; `env plan` and `env apply` name it
in each withheld path's reason and in an additive `owner` field of the JSON
view, and a package claimant a caller supplies is still named by its identity.
`doctor` checked the same declared, unrecorded, present path and reported
every one as `unmanaged-write`, because the 2026-09-16 entry decided that
finding by declaration and let a claimed path through as `foreign` only when
another installer claimed it. With no installer to claim it, a user's own
`CLAUDE.md` at the adapter's pointer path was reported as a write this product
made, which section 3.8 and section 3.28's "resemblance is never ownership"
both refuse. The sections are silent on how `doctor` tells the two apart, so
this records the choice: this product writes exactly the bytes an adapter
declares and nothing else, so a declared path holding those bytes is still
`unmanaged-write`, and one holding any other bytes (or a directory) is
`foreign`, owner `user`, and exits 1 like every other `foreign`. Nothing is
inferred from resemblance in the other direction: neither finding makes a path
managed, and both leave it untouched. `crates/statecraft-cli/tests/foreign_owner.rs`
shows both through the binary's `doctor` on every platform, and the owner in
`env plan` and `env apply` on macOS, where the adapter's credential-path
prerequisite can hold; the library rows are in the environment crate's
`negative_cases.rs`.

**2026-09-23: where section 3.16's frozen resolution is carried for `run`.**
Section 3.16 says a run resolves its harness revision and its tools once and
records the requested and the resolved identity, and section 3.25 places that
record: "The **resolved** identity is recorded **per managed session** ...
Section 3.16's last rule already freezes it". In `run` the managed session is
the attempt, and sections 3.31 and 3.32 already write its evidence once: the
intent carries the committed requirement, the selected revision and the
resolved program, and the record carries the resolved revision. A later
attempt selects the committed requirement's full digest and never the newest
revision the home holds (section 3.31 rule 17), so a global upgrade cannot
change what the next attempt uses. That was true before this entry and was
exercised by no verb: the section 3.10 row "a global upgrade after a run
resolved" was tested only against `resolved::ResolvedRun`, a type no verb
writes, with a requested identity of `latest` that section 3.25 forbids
selecting. `run_startup.rs` now shows the row through the binary: a run
resolves, a newer revision is installed and `home apply` runs, and the next
attempt selects, supplies and resolves the same revision while the first
attempt's records stay byte-identical. No code changed, and `ResolvedRun` is
not wired into `run`, because a second per-run record would duplicate what the
intent already carries. What this entry does not settle is recorded rather
than chosen: whether a run's later attempt may follow a requirement that
`harness upgrade` changed after the run first resolved (section 3.31 rule 17
selects the committed requirement; section 3.16 says the run resolved once),
which matters because spec `003` keys a run by its spec id and so a run never
ends.

**2026-09-23: section 3.23's translation of `check`, where this spec's crates
answer with it.** Two places in this spec's territory run `spec-spine check`
and answer in this product's vocabulary: the qualification behind `project
register` and step 7 of `init apply`, and step 6 of `init apply`. Both passed
the answer through as success or anything else, so a stale tree and a
refused pin both read as a corpus that does not compile, and an absent binary
as a finding. `statecraft_environment::probe::run_check` now reads the answer
once, typed, and `CheckAnswer::exit_code` is the table: 0 to 0, 1 and 2 to 1,
3 to 4, and an absent binary or a missing verb to 2. Contract 5 comes first:
`check --help` must succeed before `check`'s code is read, because spec-spine
0.23.0 spends 3 on an unknown subcommand (measured 2026-09-23), which would
otherwise read as a read not performed. Four choices the section is silent on.
An exit outside spec-spine's four answers (a signal, a panic's 101) is read as
3: none of the four answers was given, so nothing about the corpus was
established. The two readings of exit 2 are told apart from the producer's
own text (`unresolved`, then `stale`), and a text naming neither says so
rather than choosing. A register that meets a refusal or a failure records
nothing, because spec `006` section 3.3 says a refusal did nothing and no
verdict was reached to record; a finding is a verdict and is recorded as
before. And an `init apply` with a failed step now exits 4 rather than 2,
because its outcome word has no failure of its own and spec `006` section 3.3
keeps 4 apart from a refusal; this also moves the steps that already failed
(an unreadable declaration, a write that could not be made) from 2 to 4, which
is the row that section names. In step 6 the verb is established before
`compile` and `index` run, so a refusal there wrote nothing. That is a
statement about the step: the initialization had already written its files in
the governance and project steps, so a step refused after both completed
leaves it `partial` (exit 1), and
`refused` (exit 2) stays what section 3.17 makes it, a precondition that
stopped the flow before any write. Section 3.23's row says "nothing was done",
which is true of the corpus step and not of an initialization that wrote
files. Measured on CI: this spec's own acceptance script runs `init apply`
with no `spec-spine` on `PATH` and accepts only 0 or 1, and an overall
`refused` failed it. The same measurement found two suites,
`managed_environment.rs` and `env_remove_bridge.rs`, that initialized with
whatever `spec-spine` the machine had and so needed the registration an
absent producer no longer gets; each now puts a stub `spec-spine` first on its
`PATH`, because neither is about the producer. Test doubles that
answer `check` now answer `check --help` too, including the stub in
`scripts/acceptance/managed-session.sh`; the probe test that read an absent
binary as a broken corpus now reads it as unavailable, which is the row.
Out of this spec's territory, and left as it stands: spec `003`'s report
source, which `work list` and `run` read, runs `check` without `--help` and
maps 2 and 3 to a refusal, as the 2026-09-23 entries of spec `003` and spec
`006` record. That is a precondition reading of a verb whose answer is not the
check's verdict, but it disagrees with this table's rows for 2 and 3, and
reconciling the two is for their owner.

**2026-09-23: correcting what the gate log shows of a planted decision.** The
entry above on section 3.37 ("section 3.37 implemented, without the
confinement"), under *A planted decision, measured*, says a tool call released
in the plant's window "is not invisible: the gate's own log ... shows the
admission it gave". That is broader than section 3.37 rules 2 and 3 allow,
and this entry corrects it; the entry above stays as written. Unconfined, the
child can write every file in its exchange directory, which holds the gate
script and the gate log. So a child that plants a decision can also rewrite or
truncate the log, or replace the script so that a call passes with no entry
at all; the supervisor's copy is also bounded (`launch::GATE_LOG_LIMIT`,
64 KiB), and a log past the bound is recorded truncated. The rule that holds
is rule 3's: the log is **child-attested**. An `admitted` entry in the copy is
evidence that the gate released a call; **no entry shows nothing**, neither
that no call ran nor that the plant released none. What the implementation
guarantees without confinement is what the test asserts unconditionally: the
plant is removed, never adopted, the attempt is refused under
`startup-admission`, and the launch records hold the supervisor's decision.
The test's conditional check (when the fixture's sentinel shows that a call
ran, the copy holds an `admitted` line) is measured on a fixture child that
does not tamper with the log, and says nothing about one that does; its
comment is corrected to say so. The residual is closed only by spec `004`
section 3.18's confinement, which is specified and not implemented.

**2026-09-24: what initialization reports, and in what order it decides
(amends section 3.17; adopted by the owner on 2026-09-24, decisions D1 (a) and
D2 (a)).** Section 3.17 has three outcome words and no rule for an execution
error, and its `refused` ("a precondition stopped it before any write") was not
true of the code at `fa000c6`: step 1 wrote the product home before the
project's preconditions were read, the governance files were written before
the `.gitignore` check, a failed step was reported `refused` whatever it had
written, and a `.gitignore` write error was reported as the area being
ignored. This entry amends section 3.17 as follows.

1. **Mutations are observed, not inferred.** `init apply` reports every change
   it made, in three categories: `project` (any path under the project root
   outside `.statecraft/state/`), `project-state` (under `.statecraft/state/`,
   including the step-progress record, the manifest lock file and each
   directory created to hold them) and `home` (the product home, including each
   directory created). Each entry names the path, the step, and its state
   before (a SHA-256 digest, `absent`, or `directory`) and after (a digest,
   `removed`, `directory`, or `unreadable`), read from disk after the
   operation, including an operation that failed part-way. Changes made before
   an error are reported. A mutation list is never computed from the plan.
   Files the corpus tool writes in step 6 are found by reading the derived
   directory before and after the tool runs, since this product does not
   write them itself.
2. **Four outcomes, one exit each.** `complete`, exit 0: every step done.
   `partial`, exit 1: at least one mutation, no step failed, and a step
   withheld, skipped or refused. `refused`, exit 2: a precondition of the
   initialization stopped it, and the mutation list holds nothing but what
   taking the manifest lock created. `failed`, exit 4: an execution error in
   any step, whether or not anything was written; the mutation list says what
   was, and an empty list means nothing changed. An execution error is
   anything spec `006` section 3.3 row 4 names: an I/O error, an unreadable
   declaration or record, a program that did not run or did not perform its
   read (a `check`, `compile` or `index` exit outside spec-spine's findings, a
   signal, a spawn error), and a step-progress record that could not be
   written.
3. **Preconditions before mutation, and a plan that writes nothing.** The
   preflight is one computation, shared by `init plan` and `init apply`: the
   product home's reads and the writes step 1 would make, the producer's
   answer, the declaration, reconciliation, the governance plan, the
   `.gitignore` merge and its refusal, the instruction bridge, and whether the
   corpus tool is present and carries `check`. The preflight itself writes
   nothing, takes no lock and creates no file or directory. **`init plan`
   runs only the preflight**, so it still computes what `init apply`
   performs, from the same code, and it leaves the project and the product
   home byte-for-byte and entry-for-entry as they were. **`init apply` takes
   the manifest lock first**, then runs the preflight, then performs. Taking
   the lock can create `.statecraft/`, `.statecraft/state/` and the lock
   file; `init apply` reports each one it created in its mutation list under
   `project-state`, including when it stops there. A precondition that fails
   in the preflight ends the initialization `refused` (exit 2), and nothing
   else is written. The lock held by another writer past the wait is such a
   precondition.
4. **Steps 6 and 7 stay degradable.** Whether the corpus tool is present and
   carries `check` is decided in the preflight and reported, and a refusal
   there refuses that step only; the earlier steps are performed and the
   initialization is `partial` (exit 1). Registration is the same: a corpus
   check that is unavailable refuses step 7 only. A step refusal carries
   `phase: preflight` when it was decided before any mutation and
   `phase: late` when a producer refused after mutations although the
   preflight passed (the tool removed between the preflight and step 6, or
   registration's check unavailable at step 7). A late refusal leaves the
   initialization `partial`, never `refused`. This keeps this spec's acceptance
   script, which runs `init apply` with no `spec-spine` on `PATH` and accepts
   0 or 1, true as written.
5. **Evidence.** Each outcome is tested through the built binary on a real
   directory with an isolated `HOME`, and each test compares the report's
   mutation list with a digest walk of the project root and the product home
   before and after. Failures are arranged on disk (permission bits, a
   directory where a file belongs, a held lock, stub programs), never by a
   mock. `init plan` is tested the same way, and its walk must show no change.
   A test that finds itself running as root, where permission bits do not
   refuse, skips with that reason rather than passing.

The earlier entries stay as written. Where the 2026-09-23 entry on section
3.23's translation says a failed step exits 4 under the word `refused`, this
entry gives that failure its own word, `failed`. Section 3.17's sentence on
`refused` is read with this entry: "before any write" means before any write
other than what taking the manifest lock created, and only `init apply` takes
it.

This entry is the authority. The implementation is a separate change.

**2026-09-24: an unmanaged hook checks the binary it selects against the
repository's pin (amends section 3.23 contract 2; adopted by the owner on
2026-09-24, decisions Q-1 (a), Q-2 (i) and Q-3 (b)).** Contract 2's order
(`$SPEC_SPINE_BIN`, then the repository's own `target/release/spec-spine`,
then `PATH`) never consults the pin the repository declares, so it can hand a
verdict, and the one sanctioned write, to a binary the repository never
adopted.

*What was measured.* This product's shipped hooks at `fa000c6`, run as
programs in disposable clones of this repository, with each candidate binary
behind a shim that records which one answered (0.20.0 and 0.24.0 installed
from crates.io, a 0.22.0 development build, the adopted 0.23.0 on `PATH`):

- **Pinned** (`required_version = "=0.23.0"`). A 0.20.0, 0.22.0 or 0.24.0 in
  `target/release` is chosen over the 0.23.0 on `PATH`. The chosen binary
  refuses itself at configuration load, exit 3, so the pull-request gate
  blocks and the session-start line reads "NOT READ (check exit 3: I/O,
  parse, schema or config)". That fails closed, but it names the wrong cause
  and never tries the binary that matches. A mismatched `$SPEC_SPINE_BIN`
  gives the same answer.
- **An override that names no executable** is skipped silently, and the next
  rule answers.
- **Unpinned**, which is what `init apply` writes today: the producer's
  `spec-spine.toml` carries the pin commented out, measured through the built
  binary. Every wrong candidate judges. 0.20.0, 0.22.0 and 0.24.0 each report
  the 0.23.0 shards `STALE`, and the gate tells the session to regenerate. On
  a `spec.md` edit, the post-edit hook's sanctioned `compile` by 0.22.0
  rewrote all seven registry shards (203 lines removed), where the same edit
  under 0.23.0 changes one; the adopted 0.23.0 then reads those shards as
  stale.

In the pinned clone, `config show` exits 3 under 0.20.0, 0.22.0 and 0.24.0
with "config error: this repository requires spec-spine =0.23.0
(spec-spine.toml [meta] required_version)" and exits 0 under 0.23.0. Evidence:
`statecraft-cli.evidence/2026-09-24/session8/f13/` (the script, its log, the
binaries' digests, the control, and the `init apply` output).

*Contract 2 now reads as follows.* It replaces the text of section 3.23's
contract 2; the other six contracts, the Stop policy and the translation
table are unchanged.

> 2. **Resolve the binary, then establish that the repository admits it.**
>    1. **The pin.** The repository's pin is `[meta] required_version` in its
>       `spec-spine.toml`, read as an uncommented line of that table. A
>       candidate is **compatible** when its reported version satisfies that
>       requirement under Cargo's semantics, which are the ones spec-spine
>       applies to it. An exact pin (`=X.Y.Z`) is compared with the version
>       the candidate's `--version` reports. Any other requirement is put to
>       the candidate itself by a read-only probe (`config show`): spec-spine
>       refuses at configuration load when its version does not satisfy the
>       pin. Only that refusal is read as "incompatible"; any other failure of
>       the probe is contract 4's "not performed", never a compatibility
>       verdict, and no later candidate is tried after it.
>    2. **An explicit override is the only candidate.** When
>       `$SPEC_SPINE_BIN` is set and not empty, no other rule is consulted. If
>       it names no executable, or names one that is not compatible, the hook
>       does not fall back. A gate refuses. An advisory hook reports the check
>       as not performed. Either way the hook names the override, the path,
>       the version it reports, the pin and the file that declares it, and the
>       two remedies: unset the override, or point it at a compatible binary.
>       Changing the pin is not offered as a remedy, because it is a `D-06`
>       change.
>    3. **Otherwise the convention candidates are tried in order**: the
>       target repository's own `target/release/spec-spine`, then `PATH`. The
>       first compatible one is used (Q-1 (a)). Every candidate passed over
>       is named in the hook's output with its path, its version and the pin
>       it fails, so no incompatible binary is bypassed silently. If
>       candidates exist and none is compatible, the check is not performed:
>       a gate refuses (contract 6), an advisory hook says so, and both list
>       what was found. When no candidate exists at all, each hook keeps the
>       behavior it has for an absent binary; this amendment does not change
>       it.
>    4. **An unpinned repository** (Q-2 (i)) keeps the order of rules 2 and 3
>       with no compatibility test, because it has declared none. Every line
>       that reports a verdict says that the repository is unpinned, and names
>       the binary and its version. **The sanctioned `compile` of contract 1
>       is withheld in an unpinned repository** (Q-3 (b)): a binary the
>       repository never adopted does not rewrite its committed shards. The
>       hook says that it withheld the compile, why, and that the read-only
>       check still ran. Contract 1's exception is otherwise unchanged.
>    5. **A managed session is outside this contract.** When the supervisor
>       supplies the resolved executable (`STATECRAFT_SPEC_SPINE`), the hook
>       uses that path and nothing else; if it is not executable, the hook
>       refuses or reports as in rule 2. Artifact identity there is the
>       supervisor's digest-verified resolution; a matching version string is
>       not identity and is never used as a substitute. Until the supervisor
>       supplies the path, a managed session (one whose launch named a run in
>       `STATECRAFT_RUN_ID`) is resolved by rules 1 to 4, and its report says
>       "version-checked, identity not verified".
>    6. **Every verdict line names its judge**: the path, the version, the
>       rule that chose it (override, repository build, `PATH`, supervisor),
>       and the pin with its source, or "unpinned".
>
> A repository that builds its own binary is still governed by the one it
> builds whenever that build satisfies its own pin. A stale build left in
> `target/release` stops judging, and the hook says so.

*Acceptance obligations.* The implementation is a separate `fix(002)` change
to the four shipped hooks and their reports. Each obligation is a test in
`crates/statecraft-home/tests/harness_hooks.rs` that extracts the shipped hook
body and runs it as a program against stub binaries that record whether they
were invoked:

1. A pinned repository with an incompatible repository build and a compatible
   binary on `PATH`: the `PATH` binary judges, and the repository build is
   named as passed over, with its version and the pin.
2. An incompatible override, with a compatible binary on `PATH`: the gate
   refuses, the advisory hooks report not performed, the `PATH` binary is
   never invoked, and the output names the override, its version, the pin and
   both remedies.
3. An override naming no executable: the same refusal, never a silent skip.
4. Candidates present and none compatible: the gate refuses, the advisory
   hooks report not performed, and every candidate is listed.
5. An unpinned repository: the first candidate judges, every verdict line says
   "unpinned", and on a `spec.md` edit the post-edit hook does **not** invoke
   `compile` and says so, while `check` still runs.
6. A pinned repository with a compatible binary: the post-edit hook's
   sanctioned `compile` still runs (contract 1 unchanged).
7. A non-exact requirement decided by the probe: a candidate the probe
   refuses is passed over and the next one judges.
8. A probe that fails for another reason: not performed, and no later
   candidate is tried.
9. The supervisor's path in a managed session: used, and nothing else is
   invoked; not executable, refused as in rule 2.
10. A managed session without the supervisor's path: resolved by rules 1 to 4,
    reported "version-checked, identity not verified".
11. Every verdict line of obligations 1, 5 and 9 names the path, the version,
    the rule and the pin or "unpinned".

Contract 2's existing tests keep their meaning and gain a pin. The change
reaches adopters only through a harness revision (section 3.14). It does not
touch another project's own hooks: an adopter's copies are its own until
Statecraft delivers, the adopter confirms its sessions still have their loop
and hooks, and only then are the project copies removed (section 3.22's
order). It implements no managed resolution and verifies no artifact
identity, which remains the bundle proposal's. Whether `init` writes an
explicit pin into a new project is a separate question, not decided here.

This entry is the authority. The implementation is a separate change.
**2026-09-24: initialization outcomes implemented (the entry above on what
initialization reports).** `crates/statecraft-home/src/flow.rs` now runs a
preflight shared by `init plan` and `init apply`, and `init apply` takes the
manifest lock before it. Tested through the built binary in
`crates/statecraft-cli/tests/init_outcome.rs`: each outcome, `init plan`'s
unchanged disk, and T1 to T12 of the decision's test table, each comparing the
report's `mutations` with a walk of the project root (outside `.git`) and the
product home. Choices the entry left open, recorded here:

- **A mutation is one operation.** A path written twice (the progress record,
  after each step) appears twice, each with its own before and after; the
  walk comparison composes them per path. `step` is the step's word, or
  `lock` for what taking the manifest lock created.
- **Names.** A project path is repository-relative and a home path is
  absolute. `.statecraft/` itself is `project`; `.statecraft/state/` and
  everything under it is `project-state`.
- **The lock's failure is reported under step 1 (`home`)**, since the lock is
  now taken before any step; a held lock is `refused` with `phase: preflight`,
  and any other lock error is `failed`. It was reported under `reconcile`.
- **Step 6's files are found by walking `.statecraft/derived/`** before and
  after the corpus tool runs. `compile` and `index` are translated as `check`
  is: exit 1 or 2 is a finding (`withheld`), any other end, a signal or a
  spawn error is `failed`.
- **A step 7 refusal** carries `phase: preflight` when the preflight had
  already found the corpus tool unavailable, and `phase: late` otherwise.
- **`writes` keeps its meaning** (what the plan writes) and is rendered only
  for a plan; what `apply` changed is rendered from `mutations`.
- **The declaration is rewritten on a re-run** under the lock, as before; it
  is reported because it changed. No other project path outside
  `.statecraft/state/` changes on a re-run (T12).
- **An unreadable `.gitignore` or root `AGENTS.md`** is now `failed` in the
  preflight. Both were read as absent, which would have let a write replace a
  file that could not be read. A `.gitignore` write error is `failed`; it was
  reported as the area being ignored.
- T2 holds the lock from the test process, so it waits out the writer wait
  (30 seconds). T4, T6 and T11 skip, saying so, when run as root.

**2026-09-24: contract 2 as amended, implemented (the entry above on the pin
check).** The four shipped hooks in `crates/statecraft-home/harness/hooks/`
carry one resolver, the same block in each, and
`crates/statecraft-home/tests/harness_hooks.rs` runs each extracted hook
against stub binaries for obligations 1 to 11. Choices the entry left open,
recorded here:

- **The pin is read with `awk`** from the uncommented `required_version`
  line of the `[meta]` table. An exact pin is `=` followed by three numeric
  parts; `=0.23`, a caret or a range goes to the `config show` probe.
- **A refusal's shape per hook.** The pull-request gate exits 2; the
  session-start, post-edit and Stop hooks print "NOT PERFORMED" with the
  reason and exit 0, which is the Stop policy's advisory rule.
- **The gate now prints one line on success**, "[pr-gate] passed, judged by
  ...", because rule 6 requires every verdict line to name its judge; it
  printed nothing on success before. Candidates passed over go to stderr.
- **In an unpinned managed session** the suffix is "identity not verified"
  without "version-checked", since nothing was version-checked.
- **The supervisor's path is not put to the pin.** Rule 5 makes its identity
  the supervisor's digest-verified resolution, so a version test would add
  nothing that rule accepts as identity.
- **The existing contract tests keep their meaning** under a pin: their
  fixture's `spec-spine.toml` pins the version the stubs report, and contract
  5's test pins the old binary it uses, so the missing verb is still what it
  measures.
- The harness revision digest changes with the hook bytes
  (`harness::revision_of`); no committed file records it.

**2026-09-24: the governance producer is the published `spec-spine-core`
0.25.0, and it conforms.** Under `D-06`'s entry of the same date, which moved
the CLI pin, the linked library moves too: `spec-spine-core =0.25.0`
(`default-features = false`) in `crates/statecraft-home/Cargo.toml`, and
`spec-spine-types` 0.25.0 with it. `Cargo.lock` carries both from crates.io
with the checksums `D-06` recorded (`d96d89fb…3b2c` and `a941756c…5495`).
`PRODUCER_VERSION` moves with the manifest, and the two ownership-transfer
assertions that name `spec-spine-core@0.23.0` now name 0.25.0.

*Conformance, on the real library.* `producer_integration.rs` passes in full,
and `make code` passes (1296 tests). The producer's handoff measured the
scaffold for this product's `config_json()` as byte-identical to 0.23.0's
except the commented `# required_version` line, and 129's rules refuse
`derived_dir = "../outside"` and `state_dir = "."`, which 0.23.0 accepted.

*An upgrade, through the built binaries.* A project initialized by the
0.23.0-core build and committed, then `init apply` by this build: `complete`,
and the mutation list names exactly one governance file, `spec-spine.toml`
(the managed template whose source changed), besides the declaration, the
progress record, the tools record and two derived files the corpus tool
rewrote. The plan listed all seven managed paths as writes; the disk shows
the six unchanged ones were not changed. With `spec-spine.toml` edited by
hand first, the same run withholds it as `drifted`, names both digests, and
leaves the file byte-identical.

*Found, not changed here.* That drifted run reports `complete`: the
governance step is `done` while one of its paths is withheld. Section 3.17
says partial work is never reported as complete. Whether a withheld path in
an otherwise completed step makes the initialization `partial` is left to
the owner. This change does not alter it.

**2026-09-24: a withheld path makes the initialization `partial`, never
`complete` (amends section 3.17 and the outcome rule 2 of this section's
2026-09-24 initialization entry; owner decision I-3, session request of
2026-09-24).** It answers the finding recorded in the 0.25.0 producer entry
above: a run that withheld a drifted `spec-spine.toml` reported `complete`
because the governance step was `done`.

1. **The rule.** When `init apply` (or `init plan`, which computes the same
   plan) withholds a path it would otherwise write, for any reason except
   `adopted`, the governance step's state is `withheld`, not `done`, and the
   initialization's outcome is `partial`, exit 1, provided no step failed
   (`failed`, exit 4) and no precondition stopped it (`refused`, exit 2). The
   reasons that count are the plan's: `drifted` (the on-disk digest differs
   from the manifest's), `foreign` (claimed by someone else), a pointer path
   already occupied, and an undecidable tracked modification. It holds whether
   or not the same run changed anything else, so rule 2's "at least one
   mutation" does not apply to a withheld path: a re-run that changes nothing
   and withholds a drifted file is still `partial`.
2. **`adopted` is not a withholding in this sense.** A path the manifest
   records as `adopted` is the reconcile step's intended result (section 3.17
   step 3: preserve every user file), is listed under `adopted`, and does not
   by itself make the outcome `partial`. `env apply` and `env upgrade` keep
   section 3.4's rule unchanged.
3. **Each withheld path is named with why.** The human report prints one
   `withhold` line per path with its reason, and the JSON report's `withheld`
   list carries the same entries; a `drifted` entry names both digests
   (expected, found). The governance step's own reason names how many paths
   were withheld. The withheld file is left byte-identical.
4. **Evidence.** The implementation is a separate change, tested through the
   built binary on real directories with an isolated `HOME`: an initialized
   project whose managed `spec-spine.toml` is edited by hand, then `init
   apply` again, reports `partial`, exit 1, the path with both digests, in
   human and JSON output, and the file's bytes are unchanged; the unedited
   re-run stays `complete`, exit 0.

**2026-09-23, adopted 2026-09-24 as the bundle proposal's Part 9 step 1. One
qualified release bundle, a global artifact store, explicit project adoption,
and frozen run resolution.** The repository owner adopted this entry in the
session request of 2026-09-24 ("Bundle step 1. Adopt the r5 bundle contract
entry ... with the selected direction already recorded"), with decision H-3
changed from (b) to (a) by owner Addendum 2 of the same day. It was written as
a proposal through five revisions (below, kept as the record), and this
paragraph states what binds from the merge that adopts it:

- **Binding, as this spec's requirements until they move to `007`:** Parts 1
  to 4 (P1 to P10) and Part 7's acceptance cases, read with Part 8's selected
  answers as the decisions. Part 8's table is the decision; the option text
  under each H is the record of what was weighed. Where a binding part says
  "would", "proposed" or "if adopted", read "must". Where it conflicts with
  Part 8's table, the table wins.
- **Not bound by this entry:** the changes Part 5 names for specs `001`,
  `003`, `005` and `006`, which bind only through those specs' own changes
  (Part 9 steps 2, 4 and 5); Part 9 step 3a, adopted as its own entry, and
  step 3b, a separate change to this spec; and everything the selection
  deferred (H-8 (a), bundle publication under `F-02` (H-9), H-18, P9.4 rules 4
  to 7, `trust reset`, A12).
- **Nothing here is implemented.** Each part's grade stays *specified* until
  the change that builds it lands with its tests.
- **Where it moves.** These requirements are `007`'s, reached by route P in
  Part 9 as adopted: once spec-spine publishes planned claims (announced for
  0.26.0) and this repository adopts that release under `D-06`, spec `007` is
  drafted with these requirements moved without change and
  `crates/statecraft-bundle/` as a planned claim, the owner ratifies it, and
  only then is it built. Route R1's narrow exception is **not adopted**; its
  text (`r1-exception-text.md`, with this entry's evidence) is a fallback only
  if the producer's feature is refused or proves unusable, and using it would
  need the owner's separate adoption.

It absorbs, and replaces, two
earlier drafts that were never committed (scaffold provenance for this spec,
and run lifetime for spec `003`), so that no two proposals cover the same
behavior. Its labels are its own: findings F1 to F12, proposal parts P1 to
P10, the owner's acceptance cases 1 to 15 with proposed additions A1 to A15,
and decisions H-1 to H-18. "Section" always means a section of a named spec.

*Revision 2 (2026-09-23, after the owner's review, still proposal only).*
Eight review findings are addressed where they arise: the bundle identity is
made acyclic (P1.0); the `supported` state that let a non-current bundle
start new work is removed, and a support window survives only as an explicit
exception the owner may request (P1.3, H-17); an existing run's frozen
resolution now precedes any explicit choice, which may only restate it (P3);
the migration transaction is ordered as stage, validate, journal, publish and
recover, with edits made after an interruption protected (P6.1); the upgrade
bootstrap says how bundle A judges a candidate whose pin and derived output
require B without ever running A's executable on it (P6.2); bounded grace
gains a maximum age, clock rules and a separate rule for metadata never seen
(P9.3); the trust model gains rollback protection and key transition
(P9.4); and the implementation sequence no longer ratifies `007` before any
code exists (Part 2 and Part 9).

*Revision 3 (2026-09-23, after the second review, still proposal only).*
Four findings are addressed where they arise: the migration plan identity no
longer hashes a declaration that contains it, and `adoptedAt` is a plan input
rather than a clock reading, so CI can recompute it (P6.0); an older project
converges through intermediate bundles admitted for migration only, which
start no new work, so "exactly one current" still holds (P6.3); the apply
transaction states which writers it excludes and which it only detects, moves
each original aside instead of overwriting it, and names what a reader can
observe mid-transaction (P6.1); and the loss of a signing threshold has a
separate, locally authorized recovery, since a newer binary alone cannot
satisfy the chain rule (P9.4 rule 7, H-18).

*Revision 4 (2026-09-23, after the third review, still proposal only).*
Seven findings are addressed where they arise. The apply transaction now
rechecks every path it already published, the declaration included, and
re-enumerates the read set rather than only re-digesting it, before its
commit point and again at close; it journals its temporary files, names the
state in which an original is moved aside and not yet replaced, and states
detection as checks at named points rather than as a guarantee about every
concurrent write (P6.1). A revoked bundle's programs are never run, including
for the mechanical comparison: an adoption away from a revoked source is a
separate, owner-authorized recovery performed by a verified non-revoked tool
(P6.2, P6.3, H-11). Route R1 states the authority each of its steps needs
(Part 9). The binary's embedded digests exclude its own, which would
otherwise make the bundle identity depend on itself (P1.0). A failure to
persist the clock high-water mark refuses new work (P9.3), a reset root starts
a new root epoch so versions an attacker chained cannot outrank it (P9.4 rule
7), and release metadata keeps every bundle a migration path may cross
(P6.3). Acceptance additions A13 to A15 test them.

*Revision 5 (2026-09-24, after the owner selected a design direction, still
proposal only).* The owner chose an answer for each of H-1 to H-18 on
2026-09-24 (Part 8 opens with the table). That choice is a **design
direction**: it tells the next revision what to write, and it adopts nothing.
Every part of this entry still binds only when the owner adopts it in the
change Part 9 names, and that act remains the owner's. Three texts change
with the direction. P9.2's release metadata gains a schema for its lists: each
superseded or revoked entry names its qualification record, the lists never
drop a bundle, and revocation and reinstatement move a bundle between them
(this closes the review's finding R4d). Under H-8 (b), P9.4 rules 4 to 7,
`trust reset` and A12 are specified for later and not built, and every place
that named them says so (P5.2, P9.4, Part 5, Part 7, Part 9 step 13). H-16
names its target: the published spec-spine 0.25.0, carrying spec-spine's 126
to 129. Part 9 step 3 is split so that the provenance slice (step 6) waits
only on the authority it uses (step 3a). The narrow authority for creating
`007` under route R1 is written as its own proposed text, kept with this
entry's evidence (`statecraft-cli.evidence/2026-09-24/session9/`,
`r1-exception-text.md`), for the owner's review.

**Part 1: Purpose.**

Four initialization findings from adopting this product in another repository
(rustev's decision record, rows `C-02`, `C-03`, `C-04` and `C-04a`) and one
open run-semantics question (F7) have one cause in common: the
product has no single identity for the tools it governs with, and no rule for
where each invocation finds them. Every product-driven `spec-spine` call runs
whatever `PATH` offers; the scaffold producer is a linked library the
declaration does not name; the judge in CI is chosen by the candidate's own
pin; and a run re-resolves its judge at every attempt.

This proposal defines:

1. a **release bundle**: one exact, digest-identified set of the Statecraft
   build, the spec-spine executable, the embedded scaffold producer, the
   harness revision and the adapter and provider compatibility it was
   qualified with (P1);
2. a **global artifact store** in the product home, immutable and
   content-addressed, with verified, concurrency-safe, recoverable installation
   and no background service (P2);
3. **one resolution**, used by every managed execution path, from a declared
   identity to a verified artifact (P3);
4. **explicit project adoption and mandatory convergence**: the adopted bundle
   is committed, a project behind the required bundle is refused new managed
   work, and inspection, diagnosis, migration and recovery stay available
   (P4 and P5);
5. a **trusted migration** that is itself a reviewed authority change and
   cannot choose a more permissive judge, and **CI** that reproduces the
   authorized judge (P6 and P7);
6. an ownership rule for **authored governance inputs** that ends false drift
   alarms without hiding a real change (P8);
7. **recovery and offline** behavior (P9).

Statecraft manages its own tooling dependencies. This proposal does **not**
centralize application dependencies such as a project's Cargo graph, and it
keeps the existing boundary: Statecraft is the environment and execution
coordinator; spec-spine is the governance producer and the judge.

**Part 2: Territory.**

None today. Part 6 proposes where each part would live. This proposal is an
entry here rather than a draft spec because the gate admits neither shape a
spec could take before its code exists. Both measured on 2026-09-23 with
0.23.0 in scratch worktrees of `96e9f2d`: a draft `007` with no `establishes`
raises lint `L-001` ("declares no ownership edge"), which `lint
--fail-on-warn` refuses; a draft `007` that claims the unwritten
`crates/statecraft-bundle/` passes `check`, `lint` and `index coverage`, and
`index check --fail-on-unresolved` refuses it (one `W-001`, exit 1). So `007`
below names the new spec H-1 recommends; no such spec exists, and Part 9 says
how its authority can precede its code without either shape.

**Part 3: Findings, with source evidence.**

Measured on 2026-09-23. The rustev findings were reproduced first at `ba118f9`
in disposable repositories with an isolated `STATECRAFT_HOME` and `HOME` and a
`PATH` of one chosen `spec-spine` plus `/usr/bin:/bin` (evidence folder
`statecraft-cli.evidence/2026-09-23/handoff-session3/rustev-repro/`), and
re-measured on the revision named in F12. The Statecraft actions are
distinguished from operator actions throughout: rustev's `.tooling/bin` install
and its `PATH` edit were done by the operator, not by this product.

*F1: C-02: two producer identities, and only one is recorded as a pin (remains).*

- The scaffold comes from the linked library `spec-spine-core =0.23.0`
  (`crates/statecraft-home/Cargo.toml`); `init plan` reports
  `producer spec-spine-core@0.23.0 conforming`.
- The corpus step runs whatever `spec-spine` is first on `PATH`. The manifest's
  `pins.spec_spine` records that executable's `--version` answer
  (`crates/statecraft-home/src/flow.rs`, `Manifest::new(Pins { spec_spine:
  ctx.corpus.version() ... })`). With 0.24.0 on `PATH` it records `0.24.0`;
  with 0.22.0 it records `0.22.0`, and the older CLI compiled a 0.23.0
  scaffold without comment.
- `Pins` (`crates/statecraft-environment/src/manifest.rs`) has `product`,
  `spec_spine` and `adapters` and no producer member, although spec `002`
  section 3.15 says the producer identity is recorded "in the report and in
  the declaration's pins". Each entry's source identity does carry
  `spec-spine-core@0.23.0`.
- `pins.product` records `0.0.0`, the crate version, which has never been
  bumped: no build of this product has an identity beyond that string. No
  binary digest or source commit is recorded anywhere.
- `doctor` compares `pins.spec_spine` with the version on `PATH`
  (`crates/statecraft-cli/src/adapters.rs`, `observed_spec_spine`) and says
  nothing about the producer.
- The scaffold's commented `required_version` is the library's own version,
  whichever CLI runs.
- `home.json`'s sibling `tools.json` records `spec-spine` as requested `any`,
  observed from `path` (`flow.rs`, `tools.upsert`).

Owner: this product. No section says which CLI versions may run a corpus a
given library scaffolded; that is a missing decision (H-3, since decided
as (a), equality), and it
is not this product's to invent by equating version strings.

*F2: C-03: every product invocation resolves a bare `spec-spine` from `PATH` (remains, broader than reported).*

The complete inventory of managed execution paths that run the judge, with how
each resolves it today:

| Path | Where | Resolution today |
|---|---|---|
| Initialization, corpus step | `statecraft-home/src/flow.rs` `SpecSpineCommand::default()`, used by `statecraft-cli/src/manage.rs` | bare `spec-spine`, `PATH` |
| Qualification probe (`project register`, init step 7) | `statecraft-environment/src/probe.rs` `CommandProbe::default()`, used by `main.rs` and `manage.rs` | bare, `PATH` |
| Work selection (`work list`, `work show`) | `statecraft-run/src/report.rs` `SpecSpineCli::default()`, used by `main.rs` | bare, `PATH` |
| Run: contract binding and readiness | `main.rs`, `SpecSpineCli::default()` passed to the run path | bare, `PATH`, at **every attempt** |
| Posture coverage, suite plan | `statecraft-cli/src/coverage.rs`, `SpecSpineCli::default().binary` | bare, `PATH` |
| Acceptance suite (`accept`) | `statecraft-cli/src/accept.rs`, `SpecSpineVerify::default()` | bare, `PATH` |
| Acceptance delta report | `accept.rs`, `Command::new("spec-spine")` | bare, `PATH` |
| Diagnostics (`doctor`) | `statecraft-cli/src/adapters.rs`, `observed_spec_spine` | bare, `PATH` |
| Delivered hooks | `statecraft-home/harness/hooks/*.sh` | `$SPEC_SPINE_BIN`, then the target's `target/release/spec-spine`, then `PATH` (spec `002` section 3.23 contract 2) |
| This repository's `make` | `Makefile` | `.tooling/bin/spec-spine` if present, else `PATH` |
| This repository's CI | `.github/workflows/govern.yml` | `.tooling/bin`, installed from the `required_version` **read from the checked-out candidate** |

Consequences:

- `$SPEC_SPINE_BIN` and a repository-local `.tooling/bin` are both ignored by
  every product invocation. With none on `PATH`, `init apply`'s corpus step is
  refused and the initialization is `partial`, although the repository holds
  a local binary.
- The product's own invocations meet a weaker rule than the one section 3.23
  contract 2 imposes on its hooks. No contract mentions `.tooling/bin`; it is a
  convention of this repository's `Makefile` and of rustev.
- Spec `001` section 3.5 puts "its verifier" in the authority set, read at the
  trusted base. Nothing resolves the verifier from the base: acceptance runs
  whatever `PATH` holds when `accept` runs.

*F3: C-04: files the scaffold tells the adopter to edit are recorded as `managed` (remains).*

After initialization, uncommenting `required_version` in `spec-spine.toml`
(whose comment invites it) and replacing `created: "REPLACE-WITH-DATE"` in
`specs/000-bootstrap/spec.md` (whose body says to customize it) makes `doctor`
report both as `drifted`, exit 1. Rustev also reports the constitution
placeholder, which its template invites the adopter to replace. This is
exactly what spec `002` sections 3.2, 3.5 and 3.15 specify: a contract path
that did not exist is written and recorded `managed`. The contradiction is
inside section 3.15, which also says "existing configuration and authored
standards or specs are not disposable templates", and between it and the
producer's own instructions.

Measured in the code, not only the text: `doctor` compares every entry's digest
without consulting its class (`crates/statecraft-environment/src/doctor.rs`,
the per-entry loop), so an `adopted` entry that is later edited is also
`drifted`. Re-recording the seeds as `adopted` would not remove the alarm.
Section 3.35's transfer can release one path to `user` by hand; that is a
remedy per repository, not a lifecycle.

*F4: C-04a: the scaffold declares one derived directory and excludes another (remains).*

The `spec-spine.toml` the library returns sets `derived_dir =
".statecraft/derived"` and `resolver_exclusions = ["target", "node_modules",
".derived", "dist", "build", ".next"]`; `.tooling` is not excluded.
Origin, read at the source: `spec-spine-core`'s scaffold emits the `[index]`
it is given; this product's `producer::config_json()` passes `[layout]` only,
so the list is `spec-spine-types`' `IndexConfig::default()`, which hard-codes
`.derived` (0.23.0 and 0.24.0 alike). The scaffold's `.gitignore` follows
`layout.derived_dir` and is coherent. The coverage walk skips the configured
derived and state roots on its own, so the stale entry misstates the layout
without changing what is walked; a missing `.tooling` exclusion puts a local
tool directory's files into the walk (read from the code, not measured).
Obligations: spec-spine owns the incoherent default; this product owns leaving
`[index]` defaulted, which section 3.15's "passed explicitly, never defaulted"
covers only for the layout. The byte equality between the library's returned
`spec-spine.toml` and the file this product writes is recorded in F12.

*F5: A resolved run is not frozen for its tools (new).*

Spec `002` section 3.16 says "a run resolves its harness revision and its
tools once". `resolved::ResolvedRun::freeze` implements that record and **no
verb writes it**; only `crates/statecraft-home/tests/negative_cases.rs` calls
it. The harness half is carried per attempt by the intent and record (spec
`002` sections 3.31 and 3.32, and the L4 binary test merged in `#96`, which
shows a later attempt selecting the committed
requirement across a `harness upgrade`). The tools half is not: every attempt,
and `accept`, resolves `spec-spine` from `PATH` afresh (F2). A newer
spec-spine installed between two attempts of one run, or between a run and
its acceptance, changes the judge without any record saying so.

*F6: The candidate chooses its own judge in CI (new).*

`govern.yml` reads `required_version` from the candidate's `spec-spine.toml`
and installs that version into `.tooling/bin`. A pull request that changes the
pin is judged by the version it names. This repository treats a pin change as
an authority change decided by a human (`D-06`; AGENTS.md, "What a green gate
means"), so the effect is limited by review, not by mechanism. The same holds
locally: `make` resolves `.tooling/bin`, which `make tools` fills from the
working tree's pin.

*F7: Run lifetime (remains).*

- The run id is the spec id (spec `006` section 5, 2026-09-17), so every `run
  <spec>` appends an attempt to one run, and `workspace::release` ("used when a
  run ends") has no caller: a run never ends (spec `006` section 5,
  2026-09-17, "when a run ends is a question section 3 does not answer").
- `workspace::base_moved` compares the target's current base with the
  workspace's recorded `base_commit`, the first attempt's. Once the target's
  `HEAD` moves, every later attempt of that run ends `interrupted` ("the base
  revision moved"), and no later attempt of that spec runs on the new base.
- Spec `003` section 3.2 fixes a run's base at run start; its section 3.4 gives each
  retry "its own number, base revision and outcome" and lists "a base revision
  that moved" as `interrupted`, which it defines as "stopped without reaching an
  outcome", although nothing stopped.
- Harness: each attempt selects the committed requirement (spec `002` section
  3.31 rule 17), while section 3.16 says a run resolves it once.

*F8: No store, no bundle, no current-release knowledge (new).*

The product home (spec `002` section 3.11) has `tools.json` and
`harness/<revision>/` and nothing that holds or verifies an executable.
Nothing downloads, and nothing records which release is current, supported or
revoked. The harness revision is the one artifact that is already content
addressed, with a committed required identity and a per-session resolved one
(spec `002` section 3.25); this proposal generalizes that model.

*F9: A standalone spec-spine release exists that nothing has qualified (new).*

`cargo search` on 2026-09-23 lists `spec-spine-cli`, `-core` and `-types`
0.24.0. This repository pins `=0.23.0` and links `spec-spine-core =0.23.0`.
The decision record's `C-02` row still says crates.io lists 0.21.0 then
0.23.0; it is stale as an observation (not corrected here: the record is spec
`001`'s, and a correction is its own change). Rustev ran 0.24.0; the shared
`~/.cargo/bin` holds 0.22.0, a version this record says was never published.
Nothing states which of these may judge a corpus this product scaffolded.
On 2026-09-24 `cargo search` still lists 0.24.0 as newest, and spec-spine's
`main` carries three corrections after `v0.24.0` (its specs 126 to 128) that
no release contains; H-16 says what is qualified next and how. Later the
same day spec-spine's `main` added 129 (a configuration passed as JSON obeys
the loader's rules) and moved to version 0.25.0 for its next release, which is
prepared and not published; H-16 now names it.

*F10: The two absorbed drafts.*

Two drafts were written on 2026-09-23 and never committed; their branches
`002-proposal-scaffold-provenance` and `003-proposal-run-lifetime` point at
`e1d74fa` and carry no commit. Their text is in the evidence folder
(`proposal-002.md`, `proposal-003.md`). Everything they propose is absorbed
here: scaffold provenance into P1 and P3, executable resolution into
P3, authored-file ownership into P8, explicit exclusions into
P8.2, and run lifetime into P5. The two branches stay empty,
as coordination markers only.

*F11: Coordination with work in flight.*

The implementation slices merged on 2026-09-23 (`#90` transfer,
`#94` per-path replacement and bridge removal, `#95` launch-record placement,
and the L3 to L5 slice, `#96`) implement no behavior proposed here. Two are adjacent:
L5 translates spec-spine's exit codes for `check` after establishing the verb
(section 3.23 contract 5), which the resolution of P3 would call; and
`#90`'s manifest lock is the lock an adoption would take (P6).

*F12: Re-measurement on the final revision.* Re-run on 2026-09-23 with this
product built from `3afa22c`, whose product code is identical to `main` at
`96e9f2d` (the merged L3 to L5 change; only tests and text differ), with the
same three executables (0.24.0 `bcb6fe52...`, 0.23.0 `365d87ab...`, 0.22.0
`c572d9f5...`) and an isolated home, in disposable repositories. All four
findings remain. C-02: with 0.24.0 on `PATH` the plan names `producer
spec-spine-core@0.23.0 conforming`, the corpus step says `compiled, indexed and
checked` without naming the executable, `pins` is `{product: 0.0.0,
spec_spine: 0.24.0}`, and each governance entry's source is
`spec-spine-core@0.23.0`; with 0.22.0 on `PATH`, `spec_spine` is `0.22.0`.
C-03: a repository-local `.tooling/bin/spec-spine` (0.24.0) with 0.22.0 on
`PATH` pins `0.22.0`, and adding `SPEC_SPINE_BIN` naming the local binary
changes nothing; with no `spec-spine` on `PATH` the corpus step is `refused`
("spec-spine is not available") and the initialization is `partial`, exit 1
(before the L5 change the step was `withheld`, also `partial`). C-04: after the
two documented edits `doctor` reports both files `drifted`, exit 1, with the
seed digests `5b2ab9fb...` and `2316c27b...` expected. C-04a: the written
`spec-spine.toml` names `.derived` in `resolver_exclusions` and does not
exclude `.tooling`; its SHA-256 is `5b2ab9fb...`, the same as the bytes the
linked library returns when called once with this product's configuration, so
this product writes the producer's bytes unchanged and the stale entry is the
producer's default. One more fact bears on P1: the binary has no `--version`
(it answers `unknown verb`), so today it cannot state its own identity at all.

*F13: A second adopter's binary resolution (reported, not measured here).*
Reported on 2026-09-24 by the session that upgraded rahi to spec-spine 0.24.0
(rahi PR `#78`, merge `d17a638`, rahi spec `001` `D-14`), and not reproduced
by this product: rahi's hooks choose their binary by `$SPEC_SPINE_BIN`, then
`target/release`, then `PATH`, which is section 3.23 contract 2's order, and
never compare it with the pin; stale 0.20.0 binaries left in two worktrees'
`target/release` would have outranked `PATH`; and rahi's merge driver also
prefers `target/release`. So contract 2's second rule can select a binary
older than the repository's own pin with no signal. This bears on H-13 and
P3: inside a managed session the supervisor's resolved path replaces the
order, and outside one the order is kept only if the owner accepts that
residual or adds a pin comparison to contract 2, which is a section 3.23
amendment of its own.

*F13, reproduced here on 2026-09-24.* This product's own shipped hooks at
`fa000c6`, run as programs in disposable clones of this repository, with each
candidate binary behind a shim that records which one answered. Pinned
(`required_version = "=0.23.0"`): a 0.20.0, 0.22.0 or 0.24.0 build in
`target/release` is chosen over a 0.23.0 on `PATH`; the chosen binary refuses
itself (exit 3), so the pull-request gate blocks and the session-start line
reads "NOT READ (check exit 3: I/O, parse, schema or config)", which fails
closed but misnames the cause and never tries the matching binary. A
`$SPEC_SPINE_BIN` naming no executable is skipped silently and the next rule
answers. Unpinned, which is what `init apply` writes (the producer's
`spec-spine.toml` carries the pin commented out; measured through the built
binary), every wrong candidate judges: 0.20.0, 0.22.0 and 0.24.0 each report
the 0.23.0 shards `STALE` and the gate tells the session to regenerate, and on
a `spec.md` edit the post-edit hook's sanctioned `compile` rewrote all seven
registry shards in 0.22.0's format (203 lines removed, against one shard for
the same edit under 0.23.0), which the adopted 0.23.0 then reads as stale.
The remedy for unmanaged use is a separate amendment of contract 2, decided
without this entry: the owner chose Q-1 (a), Q-2 (i) and Q-3 (b) on
2026-09-24 (the first compatible convention candidate is used and each one
passed over is named; an incompatible or non-executable explicit override
refuses without fallback; an unpinned repository is reported as unpinned; and
the hook-triggered `compile` is withheld when the repository is unpinned).
That amendment lands as its own change; this entry keeps only the managed
half (H-13).

**Part 4: The proposed contract.**

*P1: The release bundle and its identity.*

A **bundle** is a manifest, serialized as canonical JSON (`canonical-keysort-
json`), whose SHA-256 over those bytes is the **bundle identity**. A short
prefix is a display convenience and never the thing compared (the rule spec
`002` section 3.25 already applies to harness revisions). The manifest names:

| Member | What it identifies | Bundled or external |
|---|---|---|
| `statecraft` | Version, source commit, and per platform the binary's SHA-256 | bundled |
| `specSpine` | One producer release (P1.1): its version and source revision, the linked `spec-spine-core` and `spec-spine-types` crates.io checksums from `Cargo.lock`, and per platform the executable's SHA-256 | the executable bundled, the library embedded in `statecraft`; **one identity** |
| `harness` | The full harness revision digest (spec `002` section 3.14) | bundled (it is compiled into `statecraft` today) |
| `adapters` | Each adapter's name and version | embedded in `statecraft` |
| `providers` | Per adapter, each provider version the bundle is to be qualified with, by version **and binary digest** | external prerequisite |
| `prerequisites` | Other external programs a managed path runs (`git`, with a minimum version) | external prerequisite |
| `schema` | The manifest, declaration and record schema versions this build reads and writes | bundled |
| `migratesFrom` | The bundle identities a project may adopt this bundle from (P6.2), each already fixed when this manifest is written | reference to earlier manifests only |

**P1.0 Identity is acyclic.** Three records, each hashed over its own
canonical bytes, each referring only to records that already exist:

1. the **artifact manifest** above. Its SHA-256 is the bundle identity. It
   names artifacts and earlier bundle identities, and nothing about
   qualification, promotion, state or time, so it can be hashed before any
   evidence about it exists;
2. the **qualification record** `{ bundle, evidence: [{case, digest,
   result}], environment, qualifiedAt, qualifiedBy }`, written after the
   evidence, naming the bundle identity. Its own identity is its SHA-256. A
   provider qualification record of spec `004` section 3.16 that it cites
   names an adapter and a provider binary, never a bundle, so it cannot close
   a loop;
3. the **promotion record**, one entry of the release metadata (P9.2), naming
   the bundle identity and the qualification record identity it relies on.

A bundle's state is read from records 2 and 3 and never written into record 1.
Adding evidence, promoting, superseding or revoking never changes a bundle
identity, and a project that adopted a bundle is never told its identity
changed.

**No artifact embeds an identity that covers it.** The manifest names the
`statecraft` binary's digest, so that binary cannot embed the bundle identity
or its own digest: either would make the digest depend on itself. What the
binary embeds (H-8 (b)) is limited to the digests of the artifacts it is
bundled **with**: the spec-spine executable per platform, and the harness
revision it already compiles in. A binary verifies its own bundle by reading
the manifest shipped beside it, hashing it to the bundle identity, digesting
its own file (the path the operating system reports for the running program),
and requiring both that the manifest names that digest for this platform and
that the manifest's co-bundled digests equal the embedded ones. A qualification
record, a promotion record and release metadata name the bundle identity and
are never embedded in any artifact the manifest names.

**P1.1 Producer and executable are one identity (H-3 (a), owner Addendum 2,
2026-09-24).** The embedded producer (`spec-spine-core`, with
`spec-spine-types`) and the spec-spine executable must be the same release:
equal versions, built from the same source revision. A bundle records them as
**one** producer identity, the `specSpine` member above, never as two fields
that may disagree. A bundle whose linked library and executable are different
releases is not qualifiable, and no evidence makes it so: there is no
qualified combination of two releases. Qualification still measures that a
corpus scaffolded by that library compiles, indexes, checks and verifies under
that executable (P1.2), because equal version strings are asserted, not
assumed to work.

**P1.2 Qualification before promotion.** A bundle is `qualified` only when a
qualification record (P1.0, record 2) names its identity and cites an
evidence set containing at least: the
workspace suite (`make code`) and every spec's declared acceptance on the
bundle's source commit; a fresh initialization of a fixture repository by the
bundle's own `statecraft` with no tool on `PATH` (case 7); the
producer release's library and executable run together (P1.1); the negative suite of spec `004`
section 3.5 for each adapter against each provider the bundle names; and,
for every identity in `migratesFrom`, a migration from it on a fixture judged
as P6.2 prescribes (case 10). A standalone spec-spine release does not become
part of any bundle by being published: it enters only through a new bundle
that is qualified.

**P1.3 States.** A bundle is one of `built` (a manifest and nothing else),
`qualified` (a qualification record names it), `current`, `superseded` or
`revoked`. **Exactly one bundle is `current` at a time,** and it is the only
one that may start new managed work (P5.2). Promoting a bundle to `current`
makes the previous one `superseded` in the same metadata issue: no new work,
while runs already frozen on it keep it (H-6). `revoked` means no new work, and
retries and acceptance of runs frozen on it are refused (H-6 (a), P9.1),
with their records flagged. There is no state in which a non-current bundle
starts new work (an older project converges through bundles admitted for
migration only, P6.3, which start none); that is the convergence the owner required, and its cost is
stated rather than softened: a promotion makes every project on the previous
bundle unable to start new work until its adoption change merges. A support
window would relax that and is not proposed; H-17 records it as an exception
the owner may request. Promotion and revocation are publication acts (H-9).

*P2: The global artifact store.*

Under the product home, beside the paths spec `002` section 3.11 names:

| Path | Holds |
|---|---|
| `store/sha256/<digest>/` | One immutable artifact, named by the SHA-256 of its bytes. Never rewritten. |
| `store/bundles/<bundle-id>.json` | A bundle manifest, named by its identity. |
| `store/tmp/` | In-progress downloads, one directory per attempt. |
| `store/locks/<digest>.lock` | One advisory lock per artifact being installed. |
| `store/refs/` | Which runs and projects reference which bundle (P2 rule 7). |
| `releases.json` | Cached release metadata (P9.2). |

Rules:

1. **Verified before visible.** An artifact is fetched into `store/tmp/`,
   digested, compared with the digest the bundle manifest names, made durable,
   and renamed into `store/sha256/<digest>/` in one rename. A reader either
   sees a complete verified artifact or none. A digest mismatch deletes the
   temporary copy and is a refusal naming both digests.
2. **Concurrency-safe.** An installer takes `store/locks/<digest>.lock`
   (advisory, non-blocking with a bounded wait, released explicitly as `#93`
   and `#90` do), rechecks whether the final path exists and verifies it, and
   only then fetches. Two installers of the same artifact never write the same
   path; two of different artifacts never wait for each other.
3. **Interrupted download recovery.** A leftover temporary directory is
   removed, named, by the next installer holding that artifact's lock. A
   partial file is never renamed into place and never resumed without a
   digest check of the whole.
4. **Atomic availability of a bundle.** A bundle is `available` only when its
   manifest and every bundled artifact it names are present and verified; its
   manifest is renamed into `store/bundles/` last.
5. **Corruption is detected on use.** Every resolution (P3) checks the
   artifact's digest before running it, or checks a recorded digest of an
   immutable file whose metadata has not changed since it was verified (an
   owner decision on cost, H-12). A mismatch quarantines the artifact and
   refuses; it is never repaired silently.
6. **No background service.** Nothing runs between commands. Downloading
   happens only inside an explicit verb (`bundle fetch`, or `bundle adopt apply`
   when the operator passes `--fetch`); no schedule, daemon or login item is
   installed. Downloading is separate from project migration and from any
   native-agent settings change (spec `002` section 3.24 consent stays its own
   act).
7. **Garbage collection and retention.** `bundle gc plan` names what would be
   removed; `bundle gc apply` removes it. An artifact is never removed while
   a bundle referenced by an unended run, an adopted project declaration known
   to this home, or a record the operator pins references it
   (`store/refs/`). Historical reproducibility is served by **archival
   retrieval** (re-fetching a digest-identified artifact), not by keeping every
   artifact forever and not by accepting new work on an old bundle.

**P2.1 The `.tooling/bin` transition.** Existing repository-local installs are
user-owned files and are never deleted, moved or rewritten by this product.
`doctor` reports a `.tooling/bin/spec-spine` whose digest differs from the
adopted bundle's executable as `shadowing-candidate`, naming both, and whether
any managed path would run it (none would, under P3). A repository's
own `Makefile` or CI keeps using `.tooling/bin` until its owner changes them;
P7 gives the replacement. Whether `.tooling/bin` joins the resolution
order at all is H-10; the selected direction is that it does not.

*P3: One resolution for every managed execution path.*

Every managed path of F2 resolves the judge through one function,
with one order, and records what it resolved. The operation's kind selects
exactly one source; nothing falls through from one source to the next:

1. **An operation on an existing run** (an attempt that appends to an unended
   run, reconciliation, recovery, and acceptance of that run's attempts) uses
   the run's **frozen resolution** (P5), always. A later adoption, a newer
   download, a promotion, a changed working-tree declaration and an explicit
   argument all leave it unchanged.
2. **A new run and work selection** use the **adopted bundle** of the
   declaration **at the trusted base revision** (spec `001` section 3.5.1).
3. **Inspection, diagnosis and migration planning** use the working tree's
   declaration, and say so in their output; nothing they resolve judges a
   candidate.
4. Nothing else. `PATH`, `$SPEC_SPINE_BIN`, `.tooling/bin` and another
   repository's installation are never consulted by a managed path. Absent the
   artifact, the operation is refused, naming the bundle, the digest, and the
   command that fetches it; it is never satisfied by whatever is installed.

An **explicit bundle argument** never selects. It is an assertion: accepted
when it equals the identity rules 1 to 3 resolve for that operation, and
otherwise refused, exit 2, naming both identities. For an existing run the
only identity it can name is the frozen one; for a new run, the adopted one,
which under P1.3 must also be `current`. This keeps section 3.16's "resolved
once" true, and leaves no argument through which an operator, a script or an
agent could run a retry or an acceptance under a judge the run did not
start with.

`$SPEC_SPINE_BIN` remains an **operator** override for unmanaged use (a hook
run by hand, a development checkout). A managed session's hooks receive the
resolved executable's absolute path in their constructed environment
(`STATECRAFT_SPEC_SPINE`), set by the supervisor, so the hook's section 3.23
contract 2 order is used only outside a managed session. How an unmanaged
selection is checked against the repository's pin is the separate amendment of
contract 2 that F13 names, which keeps the `target/release/spec-spine`
candidate only when it satisfies the pin. Inside a managed session a version match is never
enough: the supervisor's path is the artifact P1.2 verified by digest.

Each resolution records `{bundle, artifact, path, digest, rule, asserted}`,
where `rule` is which of 1 to 3 answered and `asserted` is the explicit
argument, if one was given. `doctor` reports a `spec-spine` on `PATH` that
differs from the adopted bundle's as information, never as the judge.

*P4: Project adoption: the declaration commits the exact bundle.*

The committed declaration (`.statecraft/environment.json`) gains an
`adopted` member: `{ bundle: <identity>, adoptedAt, migration: <plan
identity> }`. The declaration and the files the bundle governs stay mutually
consistent, and `doctor` reports any disagreement as a finding naming both
values:

- `spec-spine.toml` `required_version` equals the bundle's producer release
  (a **tracked modification** of that one line, as the root instruction bridge
  of spec `002` section 3.13 is of `AGENTS.md`; the rest of the file is an
  authored input, P8);
- `pins` gains `producer` (the bundle's one producer identity, P1.1) and
  `bundle`; `pins.spec_spine` becomes that release's version, never an observed
  `PATH` answer; `pins.product` becomes the Statecraft version **and** source
  commit, not the crate's `0.0.0`;
- the harness requirement of spec `002` section 3.25 equals the bundle's harness
  revision unless the owner keeps them separate (H-2);
- `.statecraft/derived/` is fresh under the bundle's executable.

A manifest written before this change reads with `adopted`, `producer` and
`bundle` absent, and says so ("adopted before bundles"); it is never given a
guessed value.

*P5: Frozen run resolution, and what "new work" means.*

**P5.1 A run is fixed and ends** (absorbs the run-lifetime draft's option A; H-5
records option B). A run's bundle, base commit and harness requirement are
resolved at its first attempt, written with `ResolvedRun::freeze` (spec `002`
section 3.16), and reused by every later attempt; "the base moved" then means
only that the recorded commit no longer resolves. A run ends when an attempt
concludes `completed` and its acceptance (spec `005`) is recorded, or when the
operator ends it; ending releases the workspace. The next `run <spec>` after an
end begins a new run, whose identity is the spec id plus an ordinal
(amending spec `006` section 5, 2026-09-17). Downloading or promoting a newer
bundle never changes a frozen resolution.

**P5.2 New managed work** is any operation that starts something a bundle
will judge and that no existing frozen resolution already covers:

| Operation | New work? | Under a project behind the required bundle |
|---|---|---|
| `run <spec>` that begins a new run (none unended for that spec) | yes | refused, exit 2 |
| `run <spec>` that appends an attempt to an unended run (a retry) | no: it uses the run's frozen bundle | allowed, unless that bundle is revoked (P9) |
| Continuation of an attempt that did not conclude | no such verb: spec `003` section 3.6 reconciles and never repeats | `reconcile` and `recover` allowed |
| A resumed provider process | not a product operation; a restarted supervisor reconciles | allowed as reconciliation only |
| A new workspace (worktree) | only as part of a new run | refused with the run |
| `accept` of an attempt of an existing run | no: acceptance uses the run's frozen bundle | allowed, unless revoked |
| Further commits on an open pull request after its run ended | yes: a new run | refused until the project converges |
| `startup trial`, `startup capture` | yes: they start a provider under a bundle's harness | refused |
| `work list`, `work show`, `run list`, `run show`, `startup show`, `doctor`, `status`, `home show`, `env plan`, `bundle show`, `override show` | no: inspection and diagnosis | allowed |
| `bundle fetch`, `bundle adopt plan`, `bundle adopt apply`, `reconcile`, `recover`, `override grant/revoke` | no: acquisition, migration and recovery | allowed |
| `trust reset` | not built under the selected H-8 (b); if H-8 (a) is adopted later, acquisition, allowed | not applicable until then |

A refusal names the adopted bundle, the required one, why it is required
(P9.2), and the exact commands to converge.

*P6: Trusted migration: `bundle adopt plan | apply`.*

`plan` computes, writes nothing, and prints a plan identity (the pattern of
spec `002` section 3.35): source and target bundle identities; every file
change with its current and proposed digest (the pin line, `pins`, `adopted`,
the harness requirement, managed reference artifacts per spec `002` section
3.4's rule, the explicit `[index]` of P8.2, and `.statecraft/derived/`
regenerated by the target executable); preconditions; and the validation
`apply` will run. The plan identity is defined in P6.0.

- **Preconditions.** A tracked modification to any path the plan writes, a
  drifted managed file the plan would rewrite (per-path consent, spec `002`
  section 3.4), a manifest that changed since the plan, a leftover journal
  (P6.1 step 7), or an unavailable target artifact is a refusal naming each,
  with nothing written.
- **The migration is a reviewed change, not a side effect.** `apply` produces
  working-tree changes; adoption is in force for a project only when the
  change is committed on the branch the trusted base is read from.

**P6.0 The plan identity, without a cycle.** The declaration's
`adopted.migration` holds the plan identity, and the plan lists the
declaration among the files it changes, so the identity cannot be a hash over
the declaration's final bytes. It is a hash over a **canonical plan** that
contains everything except that one value:

- `{ schema, source, target, adoptedAt, paths }`, serialized as canonical
  JSON. `source` and `target` are the bundle identities; `paths` is sorted by
  path, each `{ path, current, proposed }` with digests or `absent`.
- For every path but the declaration, `proposed` is the digest of the exact
  bytes `apply` will write.
- For the declaration, `proposed` is the digest of its **body**: the proposed
  declaration serialized canonically with `adopted.migration` omitted, and
  nothing else omitted. The published declaration is that body with
  `adopted.migration` set to the plan identity, serialized canonically; its
  digest is written to the journal (P6.1 step 4) as `published`, and is the
  digest recovery and CI compare against the file.
- No other path's bytes may depend on the plan identity. The derived shards
  are generated from a tree in which the declaration is outside the target
  executable's read set (P8.2's exclusions); `plan` checks that the read set
  it computes excludes the declaration, and refuses otherwise, rather than
  hashing a value that would change with its own hash. Measured on
  2026-09-23 with 0.23.0 in a scratch worktree of `96e9f2d`: adding, committing
  and then editing `.statecraft/environment.json` left `check` fresh (exit 0),
  while one appended line in `README.md` turned it stale (exit 2). So under
  this repository's layout the declaration is outside the index's hashed
  inputs today. On 2026-09-24, in a disposable clone of `fa000c6` with the
  same binary, committing a declaration that gained `adopted` and `pins`
  left the exit code and the SHA-256 of the complete output unchanged for
  each of `check`, `lint --fail-on-warn`, `index check --fail-on-unresolved`
  and `index coverage --fail-on-untraced`, so step 3's other reads do not see
  it either. That is one layout and one version; the guard stays, and a
  project whose layout puts the declaration in the read set is refused by it.

`adoptedAt` is an **input** of the plan, not a clock reading taken during
comparison: `plan` fixes it once (UTC, whole seconds, RFC 3339, from the wall
clock unless given), it enters the canonical plan, and `apply` writes the
plan's value. **Reconstruction**, used by `apply --resume` and by P6.2 rule 1:
read the candidate declaration; take `adopted.migration` as the claimed
identity M and `adopted.adoptedAt` as the input; recompute the canonical plan
from the base tree and the two bundle identities with that input; require its
identity to equal M, the candidate's declaration with `adopted.migration`
removed to hash to the plan's body digest, and every other path to carry its
`proposed` digest. `adoptedAt` is the only value the candidate supplies, so CI
also refuses one earlier than the base commit's committer time or later than
its own clock plus P9.3's skew; it is a record of when, never an authority.

The shards `apply` regenerates in step 2 must equal the plan's `proposed`
digests; a difference means the generator is not deterministic for this tree,
and `apply` refuses, exit 1, naming each shard, rather than publishing bytes
the plan identity does not cover.

**P6.1 The apply transaction.** `apply` runs these steps in this order. A
step that fails ends the transaction at that step with the result stated, and
no later step runs.

*The concurrency boundary.* Two kinds of writer are distinguished, and the
contract differs for each:

- **Cooperating writers** are Statecraft processes. They take the manifest
  lock of `#90` before writing any governed path, so `apply`, recovery,
  initialization, `env apply` and another `apply` exclude one another for the
  whole transaction.
- **Non-cooperating writers** are everything else: an editor, `git`, a
  formatter, another tool. No lock excludes them, and none is claimed. Two
  properties are promised for them, and they are different in kind:
  - **No overwrite by this product.** `apply` and recovery never replace or
    delete bytes they did not write: every placement is a no-replace rename,
    and every original is moved aside and kept rather than removed. A
    retained copy is deleted only at step 7's close, and only after its
    digest equals the journal's original, so what is deleted is the original
    the plan replaces and nothing another writer put there; a write through a
    held descriptor after that check is the residual stated below. So a
    byte another writer puts at a planned path is, after the transaction,
    either at that path or in a retained copy under `orig/`, unless that
    writer itself removed it.
  - **Detection at named checkpoints, not continuously.** A write is detected
    when a checkpoint's digest or path enumeration sees it: step 5.2 (the
    moved original), step 5.3 (the path reappearing), step 6 (every published
    path, every read-set file, and the read set's membership) and step 7 (the
    same, plus every retained copy). Ordinary file-system operations offer no
    compare-and-swap across a digest and a rename, so a write that lands
    between a checkpoint's digest and the rename after it, or after step 7,
    is not detected by `apply`. It is detected afterwards by the P6.2
    comparison, which refuses a candidate whose bytes differ from the plan,
    and by `bundle adopt plan`, which then reports the project neither
    adopted-and-consistent nor untouched. Nothing claims that every
    concurrent write is caught at the moment it happens.

  One residual is also stated: a process that holds a planned file open for
  writing across the transaction and writes through that descriptor after
  step 7 writes into an inode the product has released. `apply` is
  operator-initiated, and its precondition text says that files it names must
  not be held open for writing.

1. **Lock and recompute.** Take the manifest lock for the whole transaction;
   recompute the plan (P6.0); refuse, writing nothing, unless its identity
   equals the one given. Refuse, exit 2, if `.statecraft/state/` is not on the
   same file system as every planned path, since steps 5 and 6 rely on rename
   within one file system.
2. **Stage.** Build a validation tree under
   `.statecraft/state/adopt/<plan-id>/tree/`: a copy of every file the target
   executable reads (tracked and untracked, not ignored) as it is now, with
   each planned file replaced by its proposed bytes, and record each copied
   file's digest as the **read-set snapshot**. `.statecraft/derived/` is
   regenerated there by the target executable and must equal the plan (P6.0).
   Nothing outside `.statecraft/state/` is written in this step.
3. **Validate.** Run the plan's validation (the target executable's `check`,
   `lint` and `index check`, and `doctor`'s consistency rules of P4) against
   the validation tree only. On failure: remove the stage, exit 1 naming the
   failed check; no project file changed and no journal exists.
4. **Journal.** Write `.statecraft/state/adopt/<plan-id>/journal.json`
   durably (below): the plan identity, the read-set snapshot, and per path
   the original digest (or `absent`), the proposed digest, the name of its
   temporary file (`<path>.statecraft-adopt-<plan-id>`, fixed before it is
   created), and for the declaration the `published` digest of P6.0. The
   journal exists before the first project file changes, so no file this
   transaction creates in the project tree is unnamed by it.
5. **Publish every path except the declaration,** one at a time, each as
   follows:
   1. write the proposed bytes to the journaled temporary file beside the
      path, created exclusively (an existing file of that name is a conflict,
      never overwritten), and make them durable;
   2. **move the original aside**: rename the path into
      `.statecraft/state/adopt/<plan-id>/orig/<n>`, which moves the file
      itself rather than copying it, then digest the moved file. If its digest
      is not the journal's original, a writer changed it after staging: rename
      it back only if the path is still absent (a no-replace rename:
      `renameat2(RENAME_NOREPLACE)` on Linux, `renamex_np(RENAME_EXCL)` on
      macOS, `link` then `unlink` where neither exists), and stop with a
      conflict (below). An original that was `absent` must still be absent;
   3. **place the proposed file with a no-replace rename.** If the path exists
      again, a writer created it after step 5.2; the proposed file is not
      placed, and the transaction stops with a conflict;
   4. make both directories durable, and only then mark the path `published`
      in the journal.
6. **Revalidate, then publish the declaration, last.** Before the commit
   point, three checks, each against what step 3 validated:
   1. every **published** planned path still exists and carries its
      `proposed` digest. A path that is missing, or carries other bytes, was
      changed by a writer after this transaction placed it;
   2. every file in the read-set snapshot that is not a planned path carries
      its snapshot digest, and is present;
   3. the read set, **enumerated again** by the same rule as step 2, has
      exactly the snapshot's members: a new file the target executable would
      read (an untracked spec, say) changes what step 3 validated as surely
      as an edit does.

   Any difference stops the transaction with a conflict before the commit
   point, and the declaration is not published. Otherwise publish the
   declaration by step 5's procedure. Its rename is the commit point: before
   it, nothing records the target as adopted; after it, every other path has
   been published and was checked once after publication.
7. **Close.** Repeat step 6's three checks, now also requiring the
   declaration to carry its `published` digest, and digest each file under
   `orig/` against the journal's original. Any difference is a conflict
   **after** the commit point: the declaration records the target, and the
   tree no longer equals the plan. `apply` then keeps the journal and every
   copy, exits 1, and reports `adopted-with-conflict`, naming each path with
   its proposed and found digests; it does not close, and it does not undo
   the commit point on its own. Each named path is `neither` to recovery, so
   the operator first repairs those paths by hand, and then chooses
   `--resume`, which then only closes, or `--rollback`. A project in that
   state reads `migrating`, and the P6.2 comparison refuses to adopt it as it
   stands. With no difference, mark the journal `committed`, then remove the
   stage, the copies, and the journal, in that order.

A **conflict** exits 1, leaves the journal in place, names each path with its
original, proposed and found digests and where each set of bytes now is, and
never removes a retained copy. The next `plan` or `apply` finds the journal
and requires recovery.

*Durability.* A durable write is: write a temporary file, flush it (`fsync`;
`fcntl(F_FULLFSYNC)` on macOS, where `fsync` does not reach the medium),
rename it into place, and flush the containing directory. Every journal
update and every rename of steps 5 to 7 is durable before the next step, so a
crash leaves each path either original (at the path or under `orig/`) or
proposed, and the journal says which it expected.

*What a reader can observe.* A multi-file publish is not atomic on the file
systems this product supports, and nothing pretends otherwise:

- a **Statecraft reader** that finds an uncommitted journal reports the
  project as `migrating`, starts no new work, and judges nothing; recovery
  and inspection proceed;
- a **cooperating writer** is excluded by the lock, so it never builds on a
  partial tree;
- a **non-cooperating reader** (an editor, `git status`, a `spec-spine` run by
  hand) can see a mixture of original and proposed files between steps 5 and
  6. A commit made from that mixture differs from the plan, so P6.2 rule 1
  refuses it; that byte-for-byte comparison, not the file system, is what
  keeps a partial tree from being adopted.

**Recovery** from a leftover journal is `apply --resume <plan-id>` or `apply
--rollback <plan-id>`, never automatic. Each takes the manifest lock, checks
the file system as step 1 does, and first classifies every journaled path by
what is on disk, at the path and under `orig/`:

| At the path | Retained copy | Class | Crash point it corresponds to |
|---|---|---|---|
| original digest (or absent, if the original was) | none | `original` | before step 5.2 for this path |
| absent, the original having existed | original digest | `moved-aside` | between 5.2 and 5.3 |
| `proposed` (`published` for the declaration) | original digest, or none if the original was absent | `proposed` | after 5.3 |
| anything else, or absent where the plan expected bytes | any | `neither` | a writer after the last checkpoint |
| any | present with another digest | `neither` | a writer through a held descriptor |

and every journaled temporary file by whether it exists. An absent one needs
nothing. A present one is this transaction's own file whatever its bytes,
because step 5.1 created it exclusively under a name fixed in the journal: a
leftover from a crash inside step 5.1 (complete or partly written), removed by
either direction and named in the report. A temporary file is never a
`neither`. **`neither`, or a changed retained copy,
means someone wrote after the transaction's own last check, and no recovery
touches that path:** both directions refuse, exit 2, naming each such path
with its digests and the location of every copy, and leave the journal in
place until the operator resolves those paths by hand and runs the command
again. Otherwise:

- `--resume` requires the target artifact to be present and verified,
  rebuilds and revalidates the stage (steps 2 and 3, against the tree as it
  now is), then completes every `moved-aside` path by steps 5.1 and 5.3 (a
  fresh temporary file, then the no-replace placement), publishes
  every path still `original` by step 5's procedure, the declaration last by
  step 6's (with all three of its checks), and closes by step 7;
- a `moved-aside` path under `--rollback` is restored from its retained copy
  by a no-replace rename, so a file created at the path in the meantime is a
  conflict, not an overwrite;
- `--rollback` puts back, for every path whose digest is `proposed` or
  `published`, the retained original from `orig/` (or removes the path where
  the original was `absent`), each by step 5's move-aside and no-replace
  procedure so that a write during recovery is also a conflict and never lost;
  the declaration is restored first, so the project stops recording the target
  as adopted before any other path reverts; then it closes.

A stage with no journal (a crash in steps 2 to 4, before the journal was
durable) changed no project file; the next `plan` or `apply` removes it and
names it. A journal marked `committed` needs only step 7. A crash after step 6 and
before the mark is recognized by the declaration's digest being `published`,
and is closed the same way. No recovery path ever reports adoption that the
declaration does not record.

**P6.2 Bootstrap: how bundle A judges a move to B.** The pull request that
adopts B carries a pin that A's executable refuses outright (it checks
`required_version` on every run) and derived shards that B generated in a
format A may not read. So A's executable is **never run on the candidate
tree**, and authority is split three ways, none of them chosen by the
candidate:

1. **Mechanical identity, decided by A's product.** CI resolves A from the
   base (P7). A's `statecraft` recomputes `bundle adopt plan` from the base
   tree to B, using B's executable only as a digest-verified tool to generate
   the shards, and requires the candidate's diff against the base to equal
   that plan **byte for byte**: the plan's paths with exactly their proposed
   digests, and no other change. An adoption is therefore its own pull
   request; mixing it with any other change is a refusal.
2. **Admissibility of B, decided by records, not by the candidate.** A must
   be listed in B's `migratesFrom`, which B's qualification exercised (P1.2),
   and A's `statecraft` must be able to read B's manifest schema. B must be
   either `current` in verified release metadata (P9.2), or a **migration
   step** admitted by P6.3 on a path that ends at the current bundle.
3. **Governance of the resulting tree, judged by B.** B's executable runs the
   full gate on the candidate. Because rule 1 has established that the
   candidate differs from the base only by B's own mechanical output, this
   judges exactly the corpus the base already held, as B reads it.

The pull request passes only when all three hold, and the adoption remains an
authority change (spec `001` section 3.5 rule 2) with a human decision
recorded outside the candidate.

**A revoked source is never executed.** Revocation may mean the build itself
is wrong or compromised, and a byte comparison performed by such a build
proves nothing, so no program of a revoked bundle runs on any managed path,
including rule 1's comparison and the stage validation of P6.1. Where A is
revoked, the adoption is a **recovery adoption**, separate from an ordinary
one in three ways:

1. **Authority.** It needs an owner authorization recorded outside the
   candidate before CI judges it (a decision entry naming the project, A, the
   target and the reason), in addition to the ordinary adoption review.
2. **The comparing tool.** Rule 1 is performed by a **recovery tool**: a
   `statecraft` from a bundle that verified metadata lists and does not
   revoke, verified by digest, and named in the authorization. The preferred
   tool is one from a bundle **other than the target** (the newest
   non-revoked bundle able to read both manifests), so the target does not
   check its own admission. When the only such bundle is the target itself,
   that is allowed only when the authorization says so, and the record states
   that the target compared its own adoption. A's manifest and the base
   tree's files are read as data, digest-bound (P9.4 rule 3); nothing of A's
   is run.
3. **Admission.** Rule 2's `migratesFrom` edge from A is still required, and
   the path of P6.3 starts at A, the one revoked bundle it may contain, and
   only in this recovery form.

Rule 3 is unchanged: the target's executable judges the result.

*Alternatives, and what each trusts.* (i) Run revoked A's `statecraft` for the
comparison only: trusts a build that was revoked, possibly for being wrong or
compromised; not proposed. (ii) Always use the target's `statecraft`: needs
no third bundle, but the target checks its own admission, which only the
owner's authorization and the comparison's reproducibility (anyone can rerun
it with another non-revoked tool) offset. (iii) A separately qualified
recovery tool (preferred above): trusts a verified, non-revoked build that is
neither the source nor, where one exists, the target. (iv) No migration from
a revoked source: re-initialize the project under the current bundle by hand;
loses the adoption record's continuity. Under H-8 (b) every non-revoked status
is as trustworthy as the one fixed origin that states it, which is P9.4's
stated residual. H-11 decides.

The same shape covers a repository not yet
adopted (P7): a pin bump judged with the merge base's pin fails under the old
executable for the same reason, so its pull request is likewise confined to
the pin line and the regenerated shards, compared mechanically, and judged by
the new version.
- **Automatic mechanical migrations.** Only within a scope the owner adopts
  (H-12): the pin line, `pins`, `adopted`, derived output, explicit `[index]`
  entries computed from the declared layout, and managed reference artifacts
  whose digest still matches. Automatic means planned and applied without
  per-path consent; it still produces a reviewed change and never runs in the
  background. A change to governance requirements (constitution, contract, a
  spec's text), to ownership classes or roles, or to approval policy is never
  in that scope and is surfaced separately.

**P6.3 Stepped convergence, without a second current bundle.** A bundle that
changes the manifest schema names in `migratesFrom` only sources that can read
it, so a project far enough behind cannot reach the current bundle C in one
adoption. It converges through intermediate bundles, and each intermediate I
is admitted **for migration only**:

- **Admission.** I is admissible as a step when verified metadata (P9.2)
  lists it `superseded`, never `revoked`; its qualification record exists and
  names it; and `plan` finds a path A, I1, ..., C in which every hop is a
  `migratesFrom` edge of the later bundle, every hop's source `statecraft` can
  read the next manifest, and no bundle after A is revoked. A itself may be
  revoked only in P6.2's recovery form, with its authorization, and then no
  program of A's runs. `plan` chooses the
  shortest such path, prints all of it, and plans only its first hop. No
  path, or only a path through a revoked bundle after A, is a refusal naming
  the gap; the remedy is a publisher's bridge bundle, never a skipped check.
- **Metadata keeps the path.** Admission reads each hop from the `superseded`
  list, with the qualification record that entry names, so release metadata
  never drops a bundle: the union of its lists only grows (P9.2 rule 2), a
  hop is admissible only while it is in `superseded`, and a bundle a later
  document omits is treated as unknown, not admissible, and named. A
  reinstated bundle is admissible again from the issue that reinstated it
  (P9.2 rule 5). An intermediate revoked while a project
  sits on it makes that project's adopted bundle revoked, and its next hop is
  a recovery adoption (P6.2).
- **What a migration-only bundle may do.** While a project's adopted bundle is
  an intermediate, the project is `converging`: every row of P5.2 that
  refuses new work for a project behind C refuses it here too, and the only
  admissible adoption is the path's next hop. Inspection, `reconcile`,
  `recover` and the adoption verbs behave as P5.2's table says. A run frozen
  on a bundle before the project began converging keeps that bundle, as P5.1
  says; no run is ever frozen on an intermediate, because no run starts under
  one.
- **Judgment of each hop.** Each hop is its own adoption pull request, judged
  exactly as P6.2 says, with the hop's source as A and the hop's target as B.
  That B's executable judges the resulting tree is a governance judgment by a
  qualified bundle that the metadata still lists and has not revoked; it
  starts no work.
- **Where this sits in P1.3.** "Exactly one bundle is `current`" is
  unchanged: an intermediate is `superseded` and stays so; migration-only
  admission is a property of an adoption hop, not a state, and gives no
  bundle other than C the right to start new work. Each hop's plan records
  the whole path and its position on it, so review sees that the adoption is
  a step and not a destination.

*P7: CI reproduces the authorized judge.*

CI resolves the judge from the **base** revision's declaration, fetches it by
digest into a clean store, verifies it, and runs every governed check with it.
A release arriving while a pull request is open changes nothing: the base's
declaration did not change. The result records the bundle identity and
executable digest that judged. For a repository not yet adopted, CI keeps its
current mechanism, but reads the pin at the merge base rather than from the
candidate (F6), and judges a pin bump as P6.2's last paragraph says; that
is a change to `.github/workflows/`, an authority
change decided by a human, never bundled with the product change.

*P8: Authored governance inputs and generated files.*

The existing classes (`managed`, `adopted`, `user`) and the transfer
mechanism of spec `002` section 3.35 are kept. Evaluated against the
requirement (no perpetual false alarm, no silent adoption, a genuine
unauthorized change still detectable):

- recording the seeds `adopted` does not end the alarm (F3);
- transferring them to `user` at initialization ends it, but the product then
  forgets it seeded them and cannot remove an untouched seed;
- ignoring digest mismatches for them is forbidden by the request and hides a
  real change.

**Proposed: a role on `managed` entries**, keeping section 3.2's three classes
(H-4 records a fourth class as the alternative):

- `role: reference` (default, today's behavior): templates under
  `<standards_dir>/templates/**`, `.statecraft/AGENTS.md`, adapter pointer
  files. An edit is `drifted`, as now.
- `role: authored-input`: `spec-spine.toml` (except its pin line, a tracked
  modification per P4), `<specs_dir>/000-bootstrap/spec.md`,
  `<standards_dir>/constitution.md` and `<standards_dir>/contract.md`. The list
  is closed, in the spec, and never inferred from bytes.

For an authored input:

| Moment | Behavior |
|---|---|
| Initialization | Written only when absent, recorded with the **seed digest** and the producer identity; an existing file is `adopted` as today. |
| Editing | Expected. Not drift. |
| `doctor` | `seeded` when the digest equals the seed, `customized` when not, both information with no finding; `missing` as today. |
| Genuine unauthorized change | Detected where authored governance is governed: spec-spine's `check`, the coupling gate, the constitution's own Amendment rule, and review. The digest comparison is not the control for authored text, and `doctor` says so. |
| Upgrade or adoption | Never rewritten. A newer producer that seeds different bytes is reported, naming the new seed digest. |
| Removal | Deleted only while its digest equals the seed; otherwise kept and named. |
| Transfer | May be released to `user` (section 3.35); nothing moves a path into this role. |

A manifest written before the change reads every entry as `role: reference`,
as it was recorded. Changing an existing entry's role is itself a per-path,
operator-initiated, recorded act (the transfer mechanism), never automatic.

**P8.0 The rest of the governed tree, unchanged in class.** Ordinary specs
(`<specs_dir>/NNN-*/spec.md` other than the bootstrap) are written by the
project, never by this product, and stay `user`: no manifest entry, no digest,
governed by spec-spine's `check` and the coupling gate, as today. The
integration files this product writes, `.statecraft/AGENTS.md` and each
adapter's pointer file, stay `managed` with `role: reference`. The two tracked
modifications stay modifications with their own records: the root
instruction bridge (section 3.13) and the `.gitignore` fragment (section
3.15), and P4 adds a third of the same kind, the `required_version` line of
`spec-spine.toml`. A modification is never promoted to ownership of the file
it modifies.

**P8.1 Derived output.** `.statecraft/derived/` is the executable's output,
not a manifest entry; its freshness is spec-spine's `check`, and adoption
regenerates it with the adopted executable.

**P8.2 Pins and exclusions match the layout.** This product passes `[index]`
explicitly: `resolver_exclusions` derived from the declared layout (the derived
and state roots) plus build directories, and no other derived directory. A test
built on the real library asserts it, so a producer change that reintroduces a
literal is caught at the pin bump. The producer boundary is kept: no governance
template is copied into this product; the upstream requests (P10) ask
spec-spine to make its own defaults coherent.

*P9: Recovery, revocation and offline.*

**P9.1 States and what each allows.**

| Condition | Result |
|---|---|
| Conflicting local edits to a path a migration writes | refused, naming each path and both digests; nothing written |
| Dirty worktree outside those paths | not a precondition; untouched |
| Validation fails after `apply` staged | nothing renamed; exit 1 naming the failed check; staged files removed and named |
| Artifact unavailable (not in store, offline) | refused naming the digest and `bundle fetch`; inspection unaffected |
| Schema this build cannot read (declaration, manifest or record newer than the build) | refused with both schema versions; never rewritten down |
| Interrupted migration | reported by the next `plan`, and the project reads `migrating`; `apply --resume` or `--rollback` per P6.1; a path or retained copy written since the transaction's last check blocks both and is never overwritten; never reported adopted unless the declaration records it |
| A non-Statecraft write during `apply` | a conflict per P6.1: stopped before the commit point, or blocked at close; every written byte kept at the path or under `orig/`; exit 1 |
| A project too far behind for one adoption | stepped convergence per P6.3; `converging` until it reaches the current bundle, with no new work |
| Concurrent runs during an adoption | runs keep their frozen bundle; adoption takes only the manifest lock, not the repository lock |
| Revoked bundle | no new work; retries and acceptance of runs frozen on it are refused under H-6 (a) and (b), and allowed and flagged only under H-6 (c), which would execute a revoked bundle's programs and is not recommended; records made under it are never rewritten, and later readers flag them |

**P9.2 How "current" is learned and trusted.** Release metadata is one
document, fetched by an explicit verb or side-loaded, verified under P9.4, and
stored as `releases.json` with the local time it was accepted:

```
{ schema, sequence, issuedAt, expiresAt,
  current:    { bundle, qualification, since },
  superseded: [ { bundle, qualification, since } ],
  revoked:    [ { bundle, qualification, since, reason } ],
  reinstated: [ { bundle, since, reason } ] }
```

`sequence` is an integer that every issue increments. `bundle` is a bundle
identity and `qualification` the identity of the qualification record (P1.0,
record 2) the bundle was promoted on; `since` is the `sequence` of the issue
that put the entry in its list. The lists obey five rules, and a reader checks
each one against the last document it accepted:

1. **Every bundle is in exactly one place.** `current`, `superseded` and
   `revoked` are disjoint; `reinstated` is a history, not a state, and a
   bundle named there is in `current` or `superseded` as well.
2. **Nothing listed is ever dropped.** The set of bundles in `current`,
   `superseded` and `revoked` together only grows from one issue to the next;
   an entry's `bundle`, `qualification` and `since` never change while it
   stays in its list. This is the "only grows" obligation P6.3 relies on,
   stated over the union, because revocation moves a bundle out of
   `superseded`: `superseded` alone does not only grow, and was never meant
   to.
3. **Promotion.** A new `current` is a qualified bundle not previously listed,
   or a `superseded` one whose promotion names a qualification record. The
   previous `current` moves to `superseded` in the same issue, keeping its
   `qualification`, with `since` set to that issue (P1.3).
4. **Revocation** moves a bundle from `current` or `superseded` to `revoked`,
   keeping its `qualification`, with the issue's `sequence` as `since` and a
   stated reason. A revoked `current` needs a new `current` in the same issue;
   there is never an issue with no current bundle.
5. **Reinstatement** moves a bundle from `revoked` to `superseded`, never to
   `current` directly, and adds a `reinstated` entry with the reason. Its
   `qualification` is the one it was revoked with; becoming `current` again is
   a separate promotion under rule 3. `reinstated` only grows.

A reader that finds rule 2 broken (a bundle it knew is absent from every
list) treats that bundle as unknown: not admissible as a migration step,
named in the report, and, if the home had seen it `revoked`, still revoked in
this home (P9.4 rule 1). A document that breaks rule 1, 3, 4 or 5 is refused
whole, naming the rule. So the history P6.3 reads is exactly the lists: a
bundle a project may cross is found in `superseded` with the qualification it
was admitted on, a revoked hop is found in `revoked` and needs P6.2's recovery
form, and nothing a publisher omits can make an unknown bundle admissible.
Runs frozen on a bundle follow its list at the moment of use (H-6): while it
is `superseded` their retries and acceptance proceed, while it is `revoked`
they are refused, and after a reinstatement they proceed again; records made
while it was revoked are never rewritten, and are flagged with the `sequence`
of the revocation. A disconnected machine cannot prove it knows the newest release,
so the product distinguishes four freshness states and never conflates them.
Each is computed at the moment of use, from the verified document and the
clock rules of P9.3:

- **fresh**: verified metadata whose `expiresAt` has not passed;
- **stale**: verified metadata past `expiresAt` by no more than the grace
  bound G;
- **expired**: verified metadata past `expiresAt` by more than G, or whose
  age cannot be established (P9.3, clock rules);
- **absent**: no verified metadata was ever accepted by this home.

Whether the adopted bundle is `current`, `superseded` or `revoked` is read
from the document in every state but `absent`, and is reported with that
state beside it.

**P9.3 Offline policy options** (H-7):

- **O1, strict.** New work requires `fresh` metadata naming the adopted bundle
  `current`. Safest, and unusable offline beyond `expiresAt`.
- **O2, bounded grace (the selected direction).** New work proceeds without comment when
  metadata is `fresh` and names the adopted bundle `current`. When it is
  `stale` and names the adopted bundle `current`, new work requires an explicit
  per-invocation acknowledgment (`--metadata-stale`), recorded in the run
  record with the metadata's `sequence`, `issuedAt`, `expiresAt` and age;
  nothing claims the bundle is current as of now. `expired` and `absent` are
  refused exactly as O1 refuses, and no acknowledgment unlocks them: the
  remedy is `bundle fetch`, or `bundle import` of a side-loaded metadata file.
  **Absent is never treated as stale:** a home that has never verified
  metadata has no evidence the adopted bundle was ever current. Metadata
  naming the adopted bundle `superseded` or `revoked` refuses new work in
  every state. Revocation learned later flags runs recorded as stale.
- **O3, permissive.** The adopted bundle is used indefinitely and freshness is
  recorded as unknown without acknowledgment. Silent divergence; not
  recommended.

*The bound.* G is a fixed number in the product, not an operator setting; the
proposed value is 14 days, and the owner sets it with H-7. `expiresAt` minus
`issuedAt` is chosen by the publisher; the proposal is 7 days, so under O2 new
work stops at most 21 days after the last issue this machine verified.

*Clock rules.* Every comparison uses the local wall clock, never a time taken
from the document or from the network. The home keeps a high-water mark: the
latest of every accepted `issuedAt` and every wall-clock reading taken at a
freshness decision. A document whose `issuedAt` is more than a stated skew
(5 minutes proposed) ahead of the wall clock is refused as not yet valid. A
wall clock earlier than the high-water mark by more than the same skew means
the age cannot be established, and the state is `expired` until the clock
passes the mark again or a newer document is accepted, which resets the mark
to that document's `issuedAt` or the wall clock, whichever is later. A clock
set forward only brings expiry closer (and, once corrected, reads as set back
until a newer document is accepted), and a clock set back cannot extend grace
past the mark. Advancing the mark is a durable write to the home under its own
lock; if it cannot be written, the decision that needed it refuses new work
(the freshness it would rely on is not recorded, so a later set-back clock
could not be detected), and inspection is unaffected. An offline machine
whose clock was set forward and then corrected stays `expired` until a newer
document is imported; that cost is stated, not avoided.

A first installation on a machine with no network uses a side-loaded bundle
and metadata file (`bundle import <dir>`), verified exactly as a download
under P9.4, including its freshness: an old side-loaded file is `expired`, not
a way around the bound.

**P9.4 Trust: rollback protection and key transition.** H-8 chooses the root.
The selected direction is H-8 (b) initially, with signing (H-8 (a)) and H-18
deferred until `F-03` lifts: rules 1 to 3 are built with the first metadata
slice, and rules 4 to 7 below are specified so that a later adoption of (a)
has its text, and are **not built** under (b). Whatever the root, these rules
hold:

1. **No rollback.** The home stores the highest `sequence` it has accepted.
   A document with a lower `sequence` is refused, as is one with an equal
   `sequence` and different bytes; the refusal names both. A later document
   can un-revoke nothing: a bundle once seen `revoked` stays revoked in this
   home unless a document with a higher `sequence` reinstates it under P9.2
   rule 5, with a `reinstated` entry and a reason. So an attacker, a mirror or a stale cache
   that replays an older document cannot restore a revoked bundle, demote the
   current one, or reset the grace clock.
2. **Freeze is bounded.** An attacker who withholds new documents can keep
   this home on the last one it verified only until P9.3's bound; that is why
   `expired` cannot be acknowledged.
3. **Bundles are bound by digest.** Metadata names bundle identities; a
   bundle is accepted only when its manifest hashes to the identity named.
   The trust root authenticates metadata, and metadata authenticates
   everything else.

Under H-8 (a), signed metadata, four more rules apply. They are deferred with
H-8 (a) and H-18, and nothing in Part 9 builds them now:

4. **A root of keys, not one key.** The binary embeds a root document listing
   the metadata-signing public keys and a threshold (1 of 1 is allowed at
   first, and stated as such). Metadata verifies only with the threshold of
   listed keys.
5. **Transition by chained roots.** A new root document, version n+1, is
   accepted only when signed by the threshold of version n's keys **and** by
   the threshold of its own keys. The home stores the highest root version it
   has accepted and walks the chain one version at a time from its stored or
   embedded root; it never skips a version and never accepts a lower one,
   where "lower" is by (epoch, version) as rule 7 defines it. A
   newer binary may embed a newer root, which the home accepts only if the
   chain from its stored root reaches it.
6. **Removal while the threshold survives.** A compromised or lost key is
   removed by the next root version. Rule 5 still applies: version n's
   threshold must be met, and the removed key's signature is not counted
   toward it, so this works only while version n's **other** keys can meet
   version n's threshold. Metadata signed by a removed key is refused from
   then on, whatever its `sequence`. A root document carries its own
   `expiresAt`; the chain walk of rule 5 checks each intermediate root's
   signatures and not its expiry, and only the root reached at the end must
   be unexpired, since an expired root verifies no metadata. Key custody is
   `F-03`'s decision.
7. **Reset when the threshold is lost.** When version n's remaining keys
   cannot meet its threshold (a 1-of-1 root whose key is lost or compromised
   is the certain case; a 2-of-3 root that loses two keys is another), no
   root the chain rule accepts can follow n, and a newer binary embedding a
   newer root does not help, because the home accepts an embedded root only
   through the chain. Recovery is therefore a separate procedure, never an
   exception to rule 5 taken silently:
   - the publisher issues a **reset root**: version n+1, marked `reset`,
     naming the version it replaces and the reason, carrying a
     `sequenceFloor` no lower than the last metadata `sequence` it issued,
     and signed by the threshold of its own keys. It is announced through a
     channel H-18 names, with its full SHA-256;
   - a home accepts it only after an **explicit local authorization** by the
     home's operator: `trust reset --root <sha256>`, giving that digest as
     obtained from the announcement. The home refuses a reset root whose
     digest was not so authorized, and until one is, it refuses metadata
     signed under any root it cannot chain to, names the pending reset, and
     keeps its current state (which then ages under P9.3 like any other);
   - the act is appended to a trust log in the home: the replaced version,
     the reset root's digest, the operator, the local time and the stated
     reason. It is never inferred from a download, a newer binary, or a
     document's own claim;
   - rule 1 survives the reset: the home keeps the higher of its stored
     `sequence` and the reset's `sequenceFloor`, and every bundle it has seen
     `revoked` stays revoked. Where the reset follows a compromise, the reset
     root also names every root version above n it supersedes, and a home
     that had accepted one of those (an attacker's chained root) discards it
     and says so;
   - roots are ordered by **(epoch, version)**, not by version alone. Each
     reset root carries an `epoch` one higher than the root it replaces, and
     rule 5's chain walk runs within an epoch. So an attacker's chained root
     n+3 in the old epoch cannot outrank the reset's n+1 in the new one, and
     the authorized reset is the only way the epoch moves. A root in an
     earlier epoch is refused after a reset, whatever its version.

   A 1-of-1 root is still allowed, but only as a stated choice: its loss
   forces every home through rule 7. The proposal recommends starting at
   2 of 3 so that one lost key is ordinary rotation under rule 6. The initial
   threshold, the announcement channel and whether rule 7 exists at all are
   H-18.

Under H-8 (b), TLS to one fixed origin with the digests of the running binary's
co-bundled artifacts embedded (P1.0), rules 1 to 3 still hold locally, and there are no keys to
rotate: a change of origin or of TLS roots is a new product release. The
residual is stated, not hidden: whoever controls that origin, or a TLS root
the platform trusts, can issue a new higher-`sequence` document, so (b)
protects against rollback and freeze but not against a forged current
release.

*P10: Proposed upstream handoff to spec-spine.*

A request, not a requirement on spec-spine, and nothing here edits that corpus
(its ordinals are resolved through its `docs/corpus-map.md` before any is
cited): (1) derive the derived-root exclusion from `layout.derived_dir`, or
drop it, since the walk skips that root; (2) say whose version the scaffold's
commented `required_version` is, or accept a caller-supplied value; (3)
publish which executable versions accept which scaffold output, so a consumer
can qualify a combination instead of inventing one (unneeded under H-3 (a),
which forbids mixing releases); (4) accept a
caller-supplied creation date for the bootstrap spec; (5) carry a per-file
role (authored input or reference artifact) in the files-as-data answer, so
P8's list can come from the producer.

**Part 5: Exact contract changes, per spec.**

Each is proposed text for that spec's own change, in the order of part 9.
None is made by this draft.

- **`001` section 3.5, authority set.** Add, after "its verifier": "and the
  adopted bundle identity that names it, read at the trusted base like every
  other member". Component table: the spec-spine CLI row becomes "bundled
  executable, resolved only through the adopted bundle"; the `spec-spine-core`
  row gains "its identity recorded as the bundle's producer". Decision record:
  proposed rows `D-11` (the release bundle), `D-12` (distribution channel and
  trust root, interacting with `F-02` and `F-03`), and a proposed amendment of
  `D-06` (the pin follows the adopted bundle).
- **`002`.** Section 3.3: pins gain `producer` and `bundle`; `spec_spine` is the
  bundle's producer release (H-3 (a)). Section 3.2 or 3.3: the `role` of P8.
  Section 3.5: `seeded` and `customized`, information, for `authored-input`.
  Section 3.6: removal of an authored input only at its seed digest. Section
  3.11: the store paths of P2 (owned by `007`; see the ownership map). Section
  3.12: the declaration carries `adopted`. Section 3.15: "passed explicitly,
  never defaulted" extends to `[index]`; the contract set is split by role;
  the producer identity is recorded in `pins.producer`. Section 3.16: "its
  tools" means the adopted bundle, frozen by `ResolvedRun` at run start.
  Section 3.23 contract 2: the order applies outside a managed session; inside
  one, the supervisor supplies `STATECRAFT_SPEC_SPINE`. Section 3.25: the
  required harness identity is the bundle's, or stays separate (H-2).
- **`003`.** Section 3.2: the base commit and bundle are fixed for the run.
  Section 3.4: a retry reuses the run's base; `interrupted` loses "a base
  revision that moved" except where the recorded commit no longer resolves.
  New section: when a run ends, and what ending releases (P5.1).
- **`004`.** Section 3.16: a qualification record is also cited by the bundle
  that names the adapter and provider pair; a provider version absent from the
  adopted bundle's list is `unqualified`, as today, and a managed run refuses
  it only if the owner adopts that (H-15).
- **`005`.** Section 3.2: the suite runs with the executable the run's frozen
  resolution names. Section 3.6: the receipt records the judge's bundle
  identity and executable digest. New: CI reproduction of the judge (P7).
- **`006`.** Section 3.1: verbs `bundle show | fetch | import | adopt plan |
  adopt apply [--resume | --rollback] | gc plan | gc apply` and `run end`;
  `trust reset` only if H-8 (a) is adopted later, and not under the selected
  H-8 (b). Section 3.3: the behind
  refusal is exit 2. Section 5 (2026-09-17): the run id becomes the spec id
  plus an ordinal.
- **`007` (the new spec of H-1, if the owner creates and ratifies it).** P1 to
  P4, P6 and P9, owning a new crate for bundle identity, store and resolution.

**Part 6: Ownership map.**

| Concern | Owner | Crate |
|---|---|---|
| Bundle manifest, identity, states | `007` | new, `crates/statecraft-bundle/` (name is H-1's) |
| Store, locking, recovery, gc | `007` | same |
| Resolution function and its record | `007`; every caller reaches it through an `extends` edge | same |
| Adoption plan and apply, journal | `007` | same, using `002`'s manifest lock |
| Declaration members `adopted`, `pins.producer`, `pins.bundle`, `role` | `002` | `statecraft-environment` |
| Authored-input lifecycle in doctor, upgrade, removal | `002` | `statecraft-environment` |
| Explicit `[index]` in the producer call | `002` | `statecraft-home` |
| Run lifetime, ending, frozen resolution use | `003` | `statecraft-run`, `statecraft-home` (`ResolvedRun`) |
| Provider compatibility cited by the bundle | `004` | `statecraft-adapter-claude-code` |
| Judge identity in receipts; CI reproduction | `005` | `statecraft-acceptance` |
| Verbs and exit codes | `006` | `statecraft-cli` |
| `.github/workflows/`, `Makefile` | unclaimed; human authority change | none |
| Release metadata publication, promotion, revocation | owner; blocked by `F-02` | none |

**Part 7: Acceptance cases.**

The fifteen cases the owner required, recovered verbatim from the request of
2026-09-23, each mapped to an owning spec and to the kind of evidence it
needs. **None is tested.** Every row is `not executed`; nothing here was run
against an implementation that does not exist.

| # | Case (owner's requirement) | Owner | Evidence kind | Status |
|---|---|---|---|---|
| 1 | Two projects with different recorded identities sharing one global artifact store | `007` | local fixture: two repositories adopting bundles A and B, one home; each run resolves its own digest | not executed |
| 2 | A global update during an active run and an open PR | `007`, `003`, `005` | local fixture for the run (fetch and promote B mid-run; attempts and `accept` keep A); CI evidence for the PR (judge stays the base's) | not executed |
| 3 | A stale project refused new work while diagnostics and upgrade remain usable | `007`, `006` | local fixture through the binary: every row of P5.2 | not executed |
| 4 | Continuation or retry governed by the agreed run-lifetime contract | `003` | local fixture: retry after `HEAD` moved reaches a real outcome; run end releases the workspace; next run is distinct | not executed; H-5 (A) selected, not adopted |
| 5 | An unexpected spec-spine executable first on `PATH` | `007` | local fixture: a hostile `spec-spine` first on `PATH` is never run by any path of F2 | not executed |
| 6 | Producer, executable, project pin and manifest identity disagreement | `002`, `007` | local fixture: each disagreement is a distinct `doctor` finding naming both values; `run` refuses | not executed |
| 7 | First initialization without manual dependency installation or `PATH` surgery | `007`, `002` | release qualification: a clean machine with only `statecraft` and a side-loaded or fetched bundle | not executed |
| 8 | Expected authored customization versus genuine managed-file drift | `002` | local fixture: pin line and date edits are `customized`, exit 0; a template edit is `drifted` | not executed |
| 9 | Correct exclusions for a non-default derived directory | `002` | local fixture on the real library: a non-default `derived_dir` appears in no exclusion but its own | not executed |
| 10 | Interrupted, concurrent and repeated upgrades | `007` | local fixture: kill between staged and declaration write; two concurrent `apply`; repeat is `already-satisfied` | not executed |
| 11 | Conflicting local edits preserved with an actionable result | `007` | local fixture: refusal names paths and digests; bytes unchanged | not executed |
| 12 | Offline operation with current, stale and absent cached release metadata | `007` | local fixture with a controlled clock and metadata file; no network | not executed; H-7 (O2) and H-8 (b) selected, not adopted |
| 13 | CI reproducing the authorized judge from a clean environment | `005` | external: a CI run on a fresh runner fetching the base's bundle by digest | not executed |
| 14 | Migration failure leaving no false success record | `007` | local fixture: failed validation leaves the declaration unchanged and no `adopted` | not executed |
| 15 | Artifact cleanup preserving referenced run identities | `007` | local fixture: `gc apply` keeps every artifact an unended run or adopted project references; archival retrieval restores a removed one by digest | not executed |

**Proposed additions**, distinct from the owner's list and not required unless
adopted:

| # | Case | Owner | Evidence kind |
|---|---|---|---|
| A1 | A corrupted artifact in the store is quarantined and refused, never repaired silently | `007` | local fixture |
| A2 | A candidate that changes its declared bundle is judged by the base's bundle, and the change is reported as an authority change | `005`, `007` | local fixture plus CI evidence |
| A3 | A manifest written before bundles reads without guessed values | `002` | local fixture |
| A4 | A revoked bundle refuses retries of runs frozen on it, per H-6 | `007`, `003` | local fixture |
| A5 | Every bare `spec-spine` invocation site of F2 is replaced; a source test fails on a new one | `007` | local test |
| A6 | Replayed older metadata, and equal-`sequence` metadata with different bytes, are refused; a revoked bundle stays revoked | `007` | local fixture |
| A7 | `stale` needs the acknowledgment; `expired`, `absent` and a clock set back past the mark refuse new work with no acknowledgment that unlocks them | `007` | local fixture with a controlled clock |
| A8 | An adoption candidate carrying any change outside its plan is refused; a path edited after an interrupted `apply` blocks both `--resume` and `--rollback` and keeps its bytes | `007`, `005` | local fixture |
| A9 | The plan identity is reproducible: CI reconstructs it from the candidate's `adoptedAt` and gets the declared identity; a changed `adoptedAt` or one outside its bounds is refused; the declaration is the only path whose bytes contain the identity | `007`, `005` | local fixture |
| A10 | A project two schema generations behind converges through an intermediate admitted for migration only; while on it, every new-work row of P5.2 is refused; a path through a revoked bundle is refused naming the gap | `007` | local fixture with three bundles |
| A11 | A write to a planned path, and to an unplanned read-set file, between staging and publication: each is a conflict before the commit point, every written byte is at the path or in `orig/`, and the declaration is not published; a mid-transaction commit is refused by the P6.2 comparison | `007`, `005` | local fixture with an injected writer |
| A12 | (Deferred with H-8 (a) and H-18; not built or tested under the selected H-8 (b).) A 1-of-1 root whose key is lost: new metadata is refused until `trust reset` is authorized with the reset root's digest; an unauthorized reset root is refused; `sequence` and revocations survive the reset; an old-epoch root with a higher version is refused after the reset | `007` | local fixture with test keys |
| A13 | Between a path's publication and the commit point, an injected writer (a) edits the published path, (b) deletes it, (c) creates a new file in the read set, (d) edits a retained original under `orig/`, (f) creates a planned path whose original was `absent` between 5.2 and 5.3; after the commit point and before close, (e) edits a published path. (a) to (c) and (f): a conflict before the commit point, (f) with the writer's file left at the path and the proposed file not placed, declaration unpublished, every byte at its path or under `orig/`. (d) and (e): `adopted-with-conflict`, exit 1, journal kept. Each asserted against a digest walk of the tree and `orig/` | `007` | local fixture with an injected writer, on disk |
| A14 | A crash (the process killed by a test failpoint, then a real `SIGKILL` at a sample of the same points) at each of: during stage, after the journal, inside 5.1, between 5.2 and 5.3, after 5.3 before the mark, between the last path and step 6, after the commit point before `committed`, after `committed` before cleanup. For each: the classification of P6.1's recovery table, `--resume` and `--rollback` each reaching a consistent tree, a path written after the crash blocking both, and a temporary file absent, complete or partly written each classified as P6.1 says (never `neither`); then a second crash inside `--resume` and inside `--rollback`, and inside step 6's and step 7's checks, each recovered by the same rules to a consistent tree | `007` | local fixture, on disk |
| A15 | A project adopted on a bundle later revoked: an ordinary adoption is refused; a recovery adoption without an authorization is refused; with one, the recovery tool performs the comparison, no program of the revoked bundle runs (a revoked `statecraft` and executable replaced by programs that record any invocation, which must record none), and the record names the tool and, where it is the target, says so | `007`, `005` | local fixture |

**Release qualification requirements** (P1.2): the workspace suite
and every spec's declared acceptance on the bundle's source commit; case 7 on
each supported platform; the producer release's library and executable run together (P1.1); the adapter
negative suite against each named provider binary, bound by digest; a
migration from each identity in `migratesFrom`, judged as P6.2 prescribes.
The record names the bundle identity and is kept with it.

**Part 8: Decisions for the owner.**

Each names the options, the recommendation, and the authority requested. On
2026-09-24 the owner selected a design direction, one answer per decision,
tabled here. A selection adopts nothing: each answer binds only when the
owner adopts it in the change named in its "Authority" line, in Part 9's
order. The option text below each decision is kept so that the adopting
change can cite what was weighed.

| H | Selected direction (2026-09-24) |
|---|---|
| 1 | (a) a new `007` owning `crates/statecraft-bundle/`, created through the normal route once planned claims are adopted (Part 9, route P; the owner's update of 2026-09-24, R1 kept as a fallback only) |
| 2 | (a) the bundle fixes the harness revision |
| 3 | (a) equality: the CLI and the linked library are the same release, recorded as one identity (owner Addendum 2, 2026-09-24; the direction first selected (b)) |
| 4 | (a) `role` on `managed` |
| 5 | (A) a run is fixed and ends; the next run gets an ordinal |
| 6 | (a) superseded: retries and acceptance continue; revoked: execution refused |
| 7 | O2 bounded offline grace: G 14 days, validity 7 days, skew 5 minutes |
| 8 | (b) initially; signing (a) deferred with `F-03`, so P9.4 rules 4 to 7, `trust reset` and A12 are not built |
| 9 | keep `F-02`: Statecraft bundle publication stays deferred; local work proceeds |
| 10 | (a) `.tooling/bin` never consulted by a managed path |
| 11 | (a) recovery adoption through a verified non-revoked tool, with an owner authorization |
| 12 | P6's scope; digest verification on every use |
| 13 | managed: the supervisor's artifact only; unmanaged: the separate contract-2 amendment (Q-1 (a), Q-2 (i), Q-3 (b)) |
| 14 | as recommended: this repository moves to the store only after it exists, as its own unclaimed change |
| 15 | as recommended: `unqualified` label and a `doctor` finding, no refusal |
| 16 | target the published spec-spine 0.25.0 carrying 126 to 129, after the producer's registry-backed consumer check passes |
| 17 | no support window |
| 18 | deferred with H-8 (a) |

- **H-1: where the contract lives.** (a) A new spec `007` owning a new crate
  (this draft); (b) new sections of `002`. (a) keeps `002`, already over 4000
  lines, from owning distribution, and gives the store one owner; it adds a
  spec and a crate. (b) needs no new spec but mixes lifecycle and
  distribution. *Recommend (a),* reached by Part 9's route, which never
  ratifies `007` ahead of its crate. Authority: adopt the contract as Part 9
  step 1 describes, then ratify `007` in the pull request that brings its
  crate, and name the crate.
- **H-2: harness inside the bundle.** (a) The bundle fixes the harness
  revision; (b) the harness requirement stays separately committed. (a) is one
  identity to converge; (b) lets a project hold a harness back. *Recommend
  (a),* keeping section 3.25's refusal rules. Authority: amend `002` section
  3.25.
- **H-3: producer and executable versions.** (a) Require equality; (b) allow a
  qualified combination recorded in two fields. (a) is simple and false today
  (0.23.0 library, 0.24.0 released CLI). *Recommend (b).* Authority: adopt
  P1.1. *Selected (a) by owner Addendum 2, 2026-09-24,* replacing the
  direction's (b): one release, one identity; P1.1 is written accordingly.
- **H-4: authored inputs.** (a) A role on `managed` (recommended; three classes
  stay); (b) a fourth class, amending section 3.2's "disjoint and exhaustive
  three"; (c) transfer to `user` at initialization. Authority: amend `002`
  sections 3.2, 3.5, 3.6 and 3.15.
- **H-5: run lifetime.** (A) A run is fixed and ends, and the next run gets an
  ordinal (recommended: keeps section 3.2's single base and section 3.16's
  "resolved once" true, restores `interrupted`'s meaning, makes
  `workspace::release` reachable). (B) Each attempt takes the current base,
  rebasing the run's workspace and keeping the prior branch; a run still never
  ends (keeps the run id; turns every retry into a rebase of work the product
  did not author). Authority: amend `003` sections 3.2 and 3.4, and `006`
  section 5.
- **H-6: retries on a superseded or revoked bundle.** (a) Superseded: retries
  and acceptance allowed, revoked: refused (recommended); (b) both refused;
  (c) both allowed and flagged. Authority: adopt P9.1's last row.
- **H-7: offline policy.** O1, O2 (recommended), O3, P9.3; under O2, the
  grace bound G (14 days proposed), the publisher's validity period (7 days
  proposed) and the clock skew (5 minutes proposed). Authority: adopt one
  policy and its numbers.
- **H-8: trust root for release metadata and bundles.** (a) Signed metadata,
  which needs `F-03` (signing, key custody) lifted; (b) digests pinned in the
  running binary for its co-bundled artifacts (P1.0) plus TLS to one fixed origin for metadata;
  (c) TLS only. (a) is the only one that survives a compromised origin.
  P9.4 rules 1 to 3 (no rollback, bounded freeze, digest binding) hold under
  every option; rules 4 to 7 (key threshold, chained root rotation, removal
  of a compromised key, root expiry, reset after threshold loss) apply under
  (a). *Recommend (b) now and
  (a) when `F-03` lifts,* with (b)'s residual stated in P9.4. (c) differs from
  (b) only by dropping the embedded digests, which are what let a binary
  verify its own bundle with no metadata at all (a first offline install).
  Authority: a decision-record row, and lifting or keeping `F-03`, which also
  decides key custody and the initial threshold. *Selected 2026-09-24:* (b)
  initially; (a) and H-18 wait for `F-03`.
- **H-9: publication.** Promotion, support windows and revocation are releases;
  `F-02` defers all publication. Authority: lift `F-02` for bundle publication,
  name the channel, or keep this proposal local until then. *Selected
  2026-09-24:* `F-02` stays; Statecraft bundle publication is deferred, so Part
  9 step 14 waits, and steps 1 to 13 are local work.
- **H-10: `.tooling/bin` in resolution.** (a) Never consulted by a managed path
  (recommended; reported only); (b) consulted when its digest equals the
  adopted bundle's (harmless but redundant); (c) consulted first (reintroduces
  F2). Authority: amend `002` section 3.23 and adopt P2.1.
- **H-11: the adoption bootstrap, and a revoked source.** P6.2 splits an
  adoption's judgment into A's mechanical comparison, the records that admit
  B, and B's governance judgment, and (revision 4) never runs a revoked
  bundle's programs, so P6.2 and P6.3 now agree. Where A is revoked: (a) a
  recovery adoption with an owner authorization, compared by a verified
  non-revoked recovery tool that is not the target where one exists, and by
  the target only when the authorization says so (recommended; P6.2's
  alternative iii, falling back to ii); (b) always the target's
  `statecraft` (alternative ii); (c) no migration from a revoked source, only
  re-initialization (alternative iv). Running revoked A (alternative i),
  which revision 3 recommended, is withdrawn. Authority: adopt P6.2 with the
  chosen form.
- **H-12: scope of automatic mechanical migration, and verification cost.** The
  scope in P6 (recommended), narrower, or none; and whether every use
  digests the artifact or trusts unchanged metadata. *Recommend digesting on
  every use:* a version string or an unchanged modification time is not
  identity, and digesting an executable costs milliseconds against a
  judgment. Authority: adopt the list.
- **H-13: the hook's `target/release/spec-spine` rule.** (a) Keep it for
  unmanaged use only (recommended); (b) remove it. Authority: amend `002`
  section 3.23 contract 2. *Revised 2026-09-24:* F13 is now reproduced, and
  the unmanaged half of this decision moves to a separate proposed amendment of
  contract 2 (unmanaged selection checks each candidate against the
  repository's pin; an incompatible explicit override refuses; an
  incompatible convention candidate is never used and never skipped
  silently). What stays here is the managed half: a managed session's hooks
  use only the supervisor's resolved artifact (P3), because a matching version
  string does not establish artifact identity. *Selected 2026-09-24:* the
  managed half as written; the unmanaged half is the contract-2 amendment
  with Q-1 (a), Q-2 (i) and Q-3 (b), decided and landed separately.
- **H-14: this repository's own adoption.** Whether statecraft-cli's `Makefile`
  and CI move from `.tooling/bin` to the store, and when. Authority: an
  unclaimed-file change, decided by the owner, after `007`'s store exists.
- **H-15: provider versions outside the bundle.** Whether a managed run refuses
  a provider version the adopted bundle does not name, or runs it labelled
  `unqualified` as spec `004` section 3.16 does today. *Recommend the label
  plus a `doctor` finding, no refusal,* until qualification records are bound
  by digest. Authority: amend `004` section 3.16.
- **H-16: which producer release is qualified next (revised 2026-09-24).** The
  CLI pin and the linked `spec-spine-core` both stay at `=0.23.0`, and there is
  no intermediate adoption of 0.24.0: nothing in this corpus needs a capability
  0.24.0 adds, and the one defect measured here that a newer producer fixes is
  in 0.24.0 too. On 2026-09-24, in a disposable clone, a spec whose `id` named
  a path outside the derived directory made `compile` overwrite a file outside
  the repository under 0.23.0 and under the published 0.24.0 alike, while
  exiting 1; spec-spine's specs 126 to 128 fix it on its `main` after `v0.24.0`
  and are in no published release. The release qualified next is the next
  published producer release that carries those corrections, and it is
  selected and qualified by **exact identity**, not by version number: the tag
  object and its target revision; each of `spec-spine-cli`, `spec-spine-core`
  and `spec-spine-types` by version, registry checksum and recorded VCS
  revision, with the unpacked source equal to that revision's tree; and the
  installed executable by the install command, toolchain and lock file that
  produced it and by its digest. A candidate built from a branch or an
  unpublished revision is **candidate testing**: it can find a reason not to
  adopt, and it qualifies nothing. **Published-package qualification** is the
  `D-06` review of that exact identity, and it moves the CLI pin as its own
  authority change with its own re-index and adoption record; the linked
  `spec-spine-core` follows as a separate implementation change under this
  spec, with real scaffold-conformance and initialization tests. **Bundle
  qualification** (P1.3) is a third thing, over the one producer release
  (its library and executable, P1.1) and the harness, and it exists only once `007`'s store does; a
  qualified package is not a qualified bundle. If a concrete need for an
  intermediate release appears first, it is stated with its measurement, and
  that release is qualified the same way. Authority: `D-06` for the pin; this
  spec for the linked library. *Selected 2026-09-24:* the target is the
  published spec-spine 0.25.0, whose prepared source carries spec-spine's 126
  to 129 (129 added the loader's rules to the JSON configuration the linked
  library takes). It is qualified only after it is published and the
  producer's registry-backed consumer check passes; the CLI pin moves first,
  as its own `D-06` authority change, and the linked library follows under
  this spec. Candidate testing, published-package qualification and bundle
  qualification stay three separate records.
- **H-17: a support window (an exception, not recommended).** The owner's
  request requires convergence to one current qualified bundle, and P1.3
  provides exactly that. If the owner nevertheless wants a superseded bundle
  to keep starting new work for a stated period after a promotion, that is an
  explicit exception to the requirement, recorded as such, with its period,
  and every run it admits records `admittedBy: support-window`. *Recommend
  no window;* the cost it would remove (new work stops until adoption merges)
  is better paid by making adoption a mechanical, reviewable change (P6).
  Authority: amend the requirement itself, not this proposal.
- **H-18: signing threshold and threshold loss (only under H-8 (a)).** The
  initial root threshold (2 of 3 recommended; 1 of 1 allowed and stated);
  the channel that announces a reset root's digest; and whether P9.4 rule 7
  exists. Without rule 7, losing the threshold means every home must be
  reinstalled from nothing, which discards its sequence and revocation
  memory. *Recommend rule 7 as written,* with a local authorization that no
  download can supply. Authority: a decision-record row alongside `D-12`,
  and `F-03`'s key custody. *Selected 2026-09-24:* deferred with H-8 (a);
  nothing of rule 7 is built.

**Part 9: Implementation sequence.**

Dependency ordered, one owning spec per pull request, authority before the
implementation it authorizes. Each step names what it needs.

*How `007`'s authority precedes its code.* Part 2's measurement rules out
both direct routes: a ratified `007` with no claim fails lint, and one
claiming its unwritten crate fails `index check --fail-on-unresolved`. Two
routes that fail nothing remain, and neither adds a placeholder crate:

- **Route P (adopted 2026-09-24): planned claims, then the normal route.**
  spec-spine is adding planned claims, announced for 0.26.0: an approved spec
  may claim a unit that does not exist yet, declared as planned, without
  `index check --fail-on-unresolved` refusing it. Step 1 adopts this entry's
  requirements here in `002`, binding from its merge. Once that release is
  published and adopted under `D-06`, with its qualification covering planned
  claims, `007` is created through the normal route: a `spec(007)` draft whose
  requirements are this entry's, moved without change (a mechanical diff
  recorded in the pull request), with `crates/statecraft-bundle/` as a planned
  claim; the owner ratifies it; then `feat(007)` builds it. From `007`'s
  ratification its requirements supersede this entry's, which a later
  `spec(002)` change marks moved, so the two never bind at once. No pull
  request builds code a ratified spec does not claim, and the gate is
  unchanged.
- **Route R1 (fallback only, not adopted): adopt in `002`, relocate with the
  crate.** Used only if the producer's planned-claim feature is refused or
  proves unusable, and then only by the owner's separate adoption of
  `r1-exception-text.md`. Its text as proposed is kept below. Step 1
  turns the parts of this entry the owner accepts into an **adopted** decision
  entry here in `002` section 5, amended as decided, which is valid under the
  gate today because `002` already claims territory, and which is binding from
  its merge. The first `feat(007)` pull request then creates `spec 007` whose
  requirements are that adopted text, moved without change, together with the
  first real slice of its crate; the owner ratifies `007` in that pull
  request, and the gate sees a claim and the code it claims at once. `002`'s
  entry is then marked as moved to `007`. Authority precedes code because
  the requirements were adopted in step 1; the pull request that creates
  `007` adds no requirement, which review can check by comparing the two
  texts.
  *The authority each step of R1 needs, under this repository's rules* (an
  agent never ratifies, never adopts, and builds nothing a ratified spec does
  not claim; AGENTS.md, "Approval semantics" and "New sessions"):
  1. **Requirements first.** The owner adopts the requirement text itself
     (not a summary and not a pointer to this proposal) as a dated `002`
     section 5 entry, with the H-answers written in. From its merge it binds
     as `002`'s, and it is `002`'s only until step 3.
  2. **The relocation is authorized in the same entry, in these words or
     equivalent:** "The owner authorizes creating spec `007` whose
     requirements are this entry's, moved without change, in the pull request
     that brings the first slice of the crate it claims; that pull request
     may add ownership edges, section 2 territory and a verification block,
     and nothing that adds or alters a requirement." It also names the crate
     and the first slice's scope. This is what lets an implementation branch
     open for a spec that is not yet ratified: the authority is `002`'s
     adopted entry, not `007`'s draft.
  3. **Ratification stays the owner's act.** The `feat(007)` pull request
     carries `007` as `draft`; the owner changes it to `approved` in that pull
     request, after review has compared its requirement text with step 1's
     byte for byte (a mechanical diff recorded in the pull request). An
     agent does not flip it. From that merge, `002`'s entry is superseded by
     `007` for those requirements, as the entry says in advance, so the two
     never bind at once; a later `spec(002)` change only marks it moved.
  4. **The gate is unchanged.** `index check --fail-on-unresolved` stays;
     `007` claims `crates/statecraft-bundle/` only in the pull request that
     writes it, with real code and tests (no empty crate, no placeholder
     module, no claim ahead of code).
  5. **It is an exception, and a narrow one.** Step 2 authorizes one pull
     request to build code that a ratified spec does not yet claim, which
     AGENTS.md otherwise forbids ("Working the backlog", "Approval
     semantics"). It covers only the first `feat(007)` pull request, only
     requirements already adopted in step 1, and only with the relocation
     shown unchanged; it is not a general waiver of ratify-before-build, and
     it grants no other spec, crate or slice anything. The exact text proposed
     for the owner's review is kept with this entry's evidence
     (`r1-exception-text.md`).
- **Route R2: remove the flag deliberately.** AGENTS.md names removing
  `--fail-on-unresolved` "deliberately, as its own change" as the case for a
  spec that must claim ahead. That is a change to the `Makefile`, an unclaimed
  authority change, and it weakens the gate for every spec until restored. It
  is listed because AGENTS.md provides for it, not recommended.

Under route P (the numbering is kept from the proposal):

1. `spec(002)` section 5: adopt this entry's accepted parts as a decision,
   binding, with the owner's H-answers written in. **Done 2026-09-24** by this
   entry's adoption; route P replaces R1's narrow authority for `007`. Needs H-1 to H-3, H-5 to H-8, H-10 to H-12 and H-17
   (H-18 is deferred with H-8 (a)).
2. `spec(001)`: authority set names the adopted bundle; proposed rows `D-11`,
   `D-12`; `D-06` amendment. Needs H-8, H-9.
3. `spec(002)`, in two separable changes, so that step 6 waits only on what it
   uses:
   - **3a, provenance:** the `role` of P8 (H-4), `pins.producer` and what
     `pins.spec_spine` records, and the explicit `[index]` of P8.2. Needs H-4
     only, and no bundle concept: it names no bundle identity, store or
     resolution. Its proposed text is kept with this entry's evidence
     (`provenance-slice-authority.md`), for the owner to adopt alone.
   - **3b, bundle-bound:** `pins.bundle`, the managed half of section 3.23
     contract 2 (`STATECRAFT_SPEC_SPINE`, H-13), `.tooling/bin` (H-10) and
     section 3.25's harness requirement as the bundle's (H-2). Needs step 1.
     The unmanaged half of contract 2 is the separate amendment already
     decided (Q-1 (a), Q-2 (i), Q-3 (b)), not this step.
4. `spec(003)` then `spec(006)` section 5: run lifetime and run identity.
   Needs H-5.
5. `spec(005)`: judge identity in receipts; CI reproduction. Needs step 2.
6. `feat(002)`: explicit `[index]`, `pins.producer`, authored-input role
   (closes C-04 and C-04a, and C-02's recording half) with cases 8 and 9, and
   case 6's rows that need no bundle (producer, executable and project pin
   disagreeing). Independent of the store and of steps 1, 2 and 3b; it may
   land first, after step 3a. This is the **independent provenance slice**,
   and it is the first implementation the selected direction prioritizes.
7. After the planned-claim release is adopted: `spec(007)` as a draft from
   step 1's adopted text, with a planned claim; the owner's ratification; then
   `feat(007)`, the first slice of
   `crates/statecraft-bundle/`: bundle manifest, identity, store,
   verification, locking, recovery, `bundle show | import | gc`, no network.
   Cases 1, 15, A1.
8. `feat(007)`: the one resolution; every site of F2 routed through
   it by `extends` edges; hooks get `STATECRAFT_SPEC_SPINE`. Cases 5, A5.
   Closes C-03.
9. `feat(003)`: run lifetime, `ResolvedRun` written by `run`, `run end`. Cases
   2 (run half), 4, A4.
10. `feat(007)`: `bundle adopt plan | apply`, the P6.1 transaction and its
    recovery, stepped convergence, the behind refusal and its table. Cases 3,
    10, 11, 14, A3, A8 (its local half), A9 (its local half), A10, A11 (its
    local half), A13, A14, A15 (its local half).
11. `feat(005)`: judge from the frozen resolution; receipts record it; the
    P6.2 comparison. Case A2.
12. Unclaimed change, owner's act: CI reads the base's bundle (or, before
    adoption, the merge base's pin). Case 13 (external evidence).
13. `feat(007)`: release metadata with P9.2's list rules, offline policy O2,
    P9.4 rules 1 to 3, and `bundle fetch`. Cases 12, A6, A7. Needs H-7 and
    H-8. Under the selected H-8 (b), `trust reset`, P9.4 rules 4 to 7 and case
    A12 are not built; they would be a later step of their own once `F-03`
    lifts and the owner adopts H-8 (a) and H-18.
14. Release qualification of the first bundle, and its promotion. Case 7 and
    P1.2. Needs H-9, which the selected direction keeps deferred under
    `F-02`: this step does not start until the owner lifts it.

**Part 10: What this proposal does not do.**

It implements nothing, commits nothing, publishes nothing, bumps no pin,
touches no real product home, runs no provider session, and modifies neither
spec-spine nor rustev. It does not centralize application dependencies, add a
background service, or claim that any case above was tested.

**2026-09-24: a new project is pinned exactly to the linked producer's
release, and the producer writes the pin (amends sections 3.3 and 3.15;
adopted by the owner on 2026-09-24; implementation waits for the producer
release).** The repository owner decided this in the session request of
2026-09-24: "new projects get an exact required_version equal to the linked
core's release. Implement through the producer's upcoming scaffold exact-pin
option (spec-spine is adding it for 0.26.0) so init still writes library
output byte for byte. Until that release is adopted, land the authority text
only; do not post-edit library output." It adopts the separate proposal of the
same day (kept with the session evidence as `init-exact-pin-proposal.md`) in
that form. Under H-3 (a) (owner Addendum 2, 2026-09-24) the linked library and
the governance executable are one release, so the proposal's options E1 (the
qualified executable's version) and E2 (the producer's own version) name the
same number; the mechanism is its E3 (the producer writes it).

1. **The value.** A new project's `spec-spine.toml` gets an uncommented
   `[meta]` table and `required_version = "=X.Y.Z"`, where `X.Y.Z` is the
   release of the `spec-spine-core` this build links: the version `D-06` (as
   amended 2026-09-24) states once, and which this product reports as its
   producer version, derived from the build. It is never taken from `PATH`, an
   installed binary, `pins.spec_spine`, `$SPEC_SPINE_BIN`, `.tooling/bin` or
   any repository's configuration at run time. The pin is exact: a caret range
   is never written.
2. **The producer writes it.** `init` passes that version to the producer's
   scaffold exact-pin option and writes the returned `spec-spine.toml` byte
   for byte, as section 3.15 requires. This product never post-edits library
   output: there is no tracked modification of `spec-spine.toml`, and section
   3.15's "bytes unchanged" keeps no exception.
3. **Until that option is adopted, nothing changes.** The option is announced
   for spec-spine 0.26.0. Until a release carrying it is qualified and adopted
   under `D-06` (the CLI and the linked library together, as one release), a
   new project stays unpinned exactly as today, and `init` reports it as
   today. No interim edit of the producer's output is made, and the proposal's
   E1 tracked modification is not adopted.
4. **Adopted files are never modified.** A project that already has
   `spec-spine.toml` keeps it, pinned or not; the report says which, and to
   what.
5. **The corpus step reads the pin it wrote.** Step 6 runs an executable only
   if it satisfies the project's pin, choosing it as section 3.23 contract 2
   (as amended on 2026-09-24) chooses for a hook; with none compatible, step 6
   is refused naming the pin and each executable passed over, and stays
   degradable (the initialization is `partial`).
6. **Qualification.** The `D-06` review of the release that adds the option
   covers it: the scaffold with the option equals the scaffold without it
   except the `[meta]` header and the pin line, and the pinned scaffold passes
   `check`, `lint`, `index check` and `index coverage` under the same
   release's executable.
7. **What it does not do.** It pins no existing project, verifies no artifact
   identity (a pin is a version, not a digest), and implements no bundle.
   `doctor` reports a project pin that differs from the linked release as
   information, since a project may move its own pin.

*Acceptance obligations, for the implementation,* through the built binary on
real directories with an isolated `HOME`: (1) a fresh `init apply` writes
`required_version = "=X.Y.Z"` equal to the linked release, whatever
`spec-spine` is on `PATH` (an older one, a newer one, none); (2) the written
file equals the producer's returned bytes exactly; (3) an existing
`spec-spine.toml`, pinned or unpinned, is adopted byte for byte and reported
as such; (4) `doctor` after init reports no drift, and an operator edit of the
pin line reads as information naming both values; (5) with only an
incompatible `spec-spine` on `PATH`, step 6 is refused naming the pin and the
skipped binary, outcome `partial`, exit 1; (6) a unit test holds the written
value equal to the build's producer version; (7) the shipped hooks in a fresh
project print pinned verdict lines, and the post-edit `compile` runs under a
compatible binary.

This entry is the authority. The implementation is a separate change, after
the release that carries the option is adopted.

**2026-09-24: a hook reads freshness with the gate's unresolved-claim flag
(amends section 3.23 contract 4; owner decision H-4, session request of
2026-09-24).** A delivered hook ran `spec-spine check` with no flags, while
this repository's gate runs `index check --fail-on-unresolved`. So a session
could be told a tree is fresh that the gate then refuses.

*What was measured* (0.25.0, disposable clones of `5ed8780`; evidence
`statecraft-cli.evidence/2026-09-24/session10/H4/`):

- A `draft` spec claiming a crate it has not written: `check` exits 0 and
  prints `codebase-index: fresh`; `check --fail-on-unresolved` exits 1 and
  prints `codebase-index: fresh, but REFUSED: 1 unresolved unit
  diagnostic(s) (--fail-on-unresolved)`; `index check --fail-on-unresolved`
  exits 1. The shipped session-start hook matched the first line and reported
  the index fresh.
- An `approved` spec claiming a missing file (`I-004`, an error): `check`
  exits 1 with or without the flag, and prints `codebase-index: UNRESOLVED
  CLAIM`.
- The same unresolved claim with stale shards: exit 1 (spec-spine's order is
  3, then 1, then 2, then 0), and the text carries both `STALE` and
  `UNRESOLVED CLAIM`.
- `check` has carried `--fail-on-unresolved` since spec-spine 0.18.0, the same
  release the hooks already require for the verb.

*Contract 4 now reads as follows.* It replaces the text of section 3.23's
contract 4 and the `check` column of the exit-vocabulary table; the other six
contracts, the Stop policy and the translation's shape are unchanged.

> 4. **Read the verdict the gate would give; never guess it.** Every hook that
>    reads freshness runs `check --fail-on-unresolved`, the flag the gate
>    passes to its unresolved-claim read, so the session learns in the
>    session what the gate refuses at merge. It does not add the gate's other
>    reads (`check --fail-on-warn`, `lint --fail-on-warn`, `index coverage
>    --fail-on-untraced`): those judge authoring quality rather than whether
>    the committed tree describes the corpus, and a hook that ran them would
>    be a second gate. The four answers are then: `0` fresh with no unresolved
>    claim; `1` either the corpus does not validate or the index records an
>    unresolved claim the gate refuses, which a hook distinguishes by the
>    report text (`spec-registry: INVALID` or `REFUSED`, `codebase-index:
>    UNRESOLVED CLAIM` or `fresh, but REFUSED`) and names, together with any
>    `STALE` line the same report carries; `2` stale and nothing else; `3` a
>    read that was not performed. Only `2`, and the `STALE` part of a `1`, is
>    repaired by regenerating; an unresolved claim is repaired in the spec or
>    the tree. Before passing the flag, the hook establishes that the binary's
>    `check --help` names it (contract 5); a binary that does not is treated as
>    lacking the verb.

The translation table's row "2 stale, or an unresolved claim" is corrected
to "2 stale": under the flag an unresolved claim is a `1`, and this product
still reports both as a finding (1), distinguished in the text.

*Obligations for the implementation*, each a test in
`crates/statecraft-home/tests/harness_hooks.rs` that runs the extracted hook
against stub binaries:

1. Every hook that runs `check` passes `--fail-on-unresolved`, and no hook
   passes a writing verb because of it (contract 1 unchanged).
2. A draft's unresolved claim (`fresh, but REFUSED`, exit 1): the
   pull-request gate refuses and names the unresolved claim, not an invalid
   corpus; the session-start line reports the index as refused by the gate
   for an unresolved claim, never as fresh; Stop reports it and does not
   block.
3. An unresolved claim with stale shards (exit 1, both lines): each hook names
   both, and says regenerating clears only the stale part.
4. An invalid corpus (exit 1, `spec-registry: INVALID`): reported as today.
5. A binary whose `check --help` does not name the flag: treated as lacking
   the verb, never read as stale.

The change reaches adopters only through a harness revision (section 3.14).
It does not touch another project's own hooks, and Rahi is untouched: an
adopter's copies stay its own until Statecraft delivers, the adopter confirms
its sessions still have their loop and hooks, and only then are the project
copies removed (section 3.22's order). A check writes nothing.

This entry is the authority. The implementation is a separate change.

**2026-09-24: contract 4 as amended, implemented (the entry above on the
gate's unresolved-claim flag).** The four shipped hooks in
`crates/statecraft-home/harness/hooks/` run `check --fail-on-unresolved`
wherever they read freshness, and
`crates/statecraft-home/tests/harness_hooks.rs` runs each extracted hook
against stub binaries for obligations 1 to 5 (`h4_*`). Choices the entry left
open, recorded here:

- **Establishing the flag** is one read: `check --help` must succeed and name
  `--fail-on-unresolved`. The pull-request gate still asks only on its exit-2
  arm, so its fresh path costs one process as before; a binary lacking the
  flag answers 2 there (clap's unknown argument), and the probe then names the
  missing flag instead of reporting a stale tree.
- **The pull-request gate's exit-1 message** names the unresolved claim, the
  invalid corpus, or both, and adds one line when the same report also names a
  stale tree; an exit 1 whose text matches none keeps the previous "does not
  validate" message.
- **Session start** reports `fresh, but REFUSED` as "REFUSED by the gate", ahead
  of the plain `fresh` match that used to absorb it.
- **Stop** reads exit 1 the same way and stays advisory; a stale tree riding
  with an unresolved claim is reported as "STALE as well".
- **Post-edit** prints the flagged `check` report, or "check NOT READ" naming
  the missing flag; the sanctioned `compile` is unchanged.
- The stubs now print the flag in `check --help` and record every argument
  vector, which is what obligation 1 reads. Obligation 4 passes on the previous
  hooks too, as "reported as today" requires; the other four fail on them.
- The harness revision digest changes with the hook bytes; no committed file
  records it.

**2026-09-24: implemented, the I-3 withheld-path rule above.** `flow.rs`'s
governance step reports `withheld`, with a reason naming each path, when the
plan withholds any path for a reason other than `adopted`; the outcome rule is
unchanged and so yields `partial`, exit 1. Two tests in
`crates/statecraft-cli/tests/init_outcome.rs` run the built binary on a real
project with an isolated `HOME`: a hand-edited `spec-spine.toml` makes the
re-run `partial`, exit 1, names the path with `drifted` and both digests in the
JSON `withheld` list and in the human `withhold` line, leaves the file's bytes
unchanged and lists no mutation for it; the unedited re-run stays `complete`,
exit 0, with the governance step `done`.

**2026-09-24: a repository setup profile: the first one renders a GitHub
Actions and Rust CI surface with an aggregate gate and an AI review (amends
sections 3.2, 3.15 and 3.17; adopted by the owner, session request of
2026-09-24, decisions S-0 to S-5 as recorded at the end of this entry).** It
is the authority for a product capability; this repository's own adoption of that capability, and
any acceptance against a real remote, are separate later acts (*Order* below).

*Why here, and not in a new spec.* The capability is initialization, the
manifest and reconciliation, which this spec owns (sections 3.2 to 3.4, 3.15,
3.17), and the entry point is the existing flow
(`crates/statecraft-home/src/flow.rs`, `plan` and `apply`), whose preflight
and observed-mutation list the 2026-09-24 entry on initialization outcomes
established. A new spec would own a second installer beside this one. The
code is a new module, `crates/statecraft-home/src/setup.rs`, and the profile
content is data under `crates/statecraft-home/profiles/github-actions-rust/`,
both already inside this spec's directory claim; no new crate. The provider
reviewer the profile invokes is a CI step, not a provider adapter, so spec
`004`'s boundary and `F-07` are not engaged. **What changes in this spec's
stance:** section 3.22 records that no counterparty verb writes a CI workflow
or a `Makefile`, and a recommendation made to an adopter on 2026-09-24 said
this product never delivers them either. For a project that **selects** a
profile, this entry supersedes that recommendation. It migrates no existing
consumer; a consumer's own files stay its own until its owner applies a plan.

*Two delivery surfaces, and neither proves the other.* The global harness
(section 3.14) is content-addressed under the home and reaches sessions
through adapters. A profile renders **committed repository files**, which
GitHub reads from the repository and nowhere else. Installing one says
nothing about the other, and each has its own evidence. The governance
producer (section 3.15) is **unchanged**: `scaffold_init_json` still returns
governance starter content only, its contract set is still closed, and a
workflow or `Makefile` from the producer is still out of contract. The profile
composes with the producer's output; it does not widen it.

**1. The profile.** A profile is `(id, revision)`, content-addressed like a
harness revision: its identity is a digest over its template files and its
policy document, so a changed byte is a different revision. This entry
admits one: `github-actions-rust`. Its schema, rendered into the target as
`.statecraft/setup/github-actions-rust.json` and recorded in the declaration:

| Field | Holds |
|---|---|
| `id`, `revision`, `identity` | the profile, its revision number, and the content digest |
| `files` | each rendered path with its content digest and role (`workflow`, `script`, `policy`, `makefile`, `ignore-fragment`) |
| `commands` | named command selections (`governance`, `code`), each a fixed argument vector the profile defines |
| `parameters` | the only values a project may set, each typed and bounded (below) |
| `jobs` | each CI job, whether it is required, and its applicability per event (section 4) |
| `review` | the reviewer tool and its pinned version, the credential **name**, skip classes and the release-candidate rule (section 5) |
| `prerequisites` | local: an exact `spec-spine` pin, `rust-toolchain.toml`, a `Cargo.lock`, a git work tree |
| `remote` | obligations the product states and never performs (section 6) |

**Commands come from the profile and from explicit project configuration,
never from a guess.** The profile fixes each command's program and arguments.
A project can set only declared parameters, in its declaration's `setup`
block, each validated before planning: `default_branch` (a ref name; default:
the remote's own `HEAD`, then `main`), `review.diff_cap` (an integer in a
bounded range), `review.exclude` (repository-relative path prefixes),
`release.branch_pattern` (a glob over ref names), and `jobs.<name>.enabled`
only for a job the profile marks optional. No parameter holds a command
string, and an unknown key refuses the plan. Nothing is read from the
project's own `Makefile`, `package.json` or scripts to infer a command.

**Extensibility, without building it.** `setup.rs` defines the profile as a
closed Rust type with one registered value. Another CI provider or language
is a new `(id, revision)` with its own templates and tests, admitted by its
own entry. There is no plugin loading, no template language beyond named
parameter substitution, and no provider or language other than this one is
built.

**2. What `github-actions-rust` renders.**

| Path | Role | Note |
|---|---|---|
| `.github/workflows/statecraft-ci.yml` | workflow | `pull_request` (`opened`, `synchronize`, `reopened`, `ready_for_review`) and `push` to the default branch. Jobs `governance`, `code`, `ai-review` (a caller of the reusable workflow) and `ci-gate`. No `merge_group` trigger. |
| `.github/workflows/statecraft-ai-review.yml` | workflow | `workflow_call` only; declares the one credential secret by name. |
| `scripts/statecraft/install-spec-spine.sh` | script | installs the version the exact pin names into `.tooling/bin`; refuses a non-exact or absent pin |
| `scripts/statecraft/gate.sh` | script | the local gate: `governance` (check, lint, index coverage, index check, authored-content if the project has the script) and `code` (cargo build, test, clippy `-D warnings`, fmt check, each `--workspace --locked`), using only `.tooling/bin/spec-spine`. CI calls the same script, so there is one definition. |
| `scripts/statecraft/ci-gate.sh` | script | the aggregate policy (section 4), reading the needs record from the environment as JSON |
| `scripts/statecraft/ai-review.sh` | script | invocation, classification and the evidence record (section 5) |
| `.statecraft/setup/github-actions-rust.json` | policy | the rendered schema above; read at the trusted base by `ci-gate.sh` |
| `Makefile` | makefile | **only when no `Makefile` exists**: `gate` and `code` targets calling `gate.sh`. An existing one is the user's and is never edited; the plan names the two lines an operator may add. |
| `.gitignore` | ignore-fragment | `.tooling/` merged into the existing marked block (section 3.15's merge) |

Every action is pinned by commit SHA; the reviewer CLI by exact version; the
governance tool by the project's exact pin; the Rust toolchain by the
project's `rust-toolchain.toml`, which the profile requires and never writes.
The rendered workflows and scripts are members of the target's **authority
set** (spec `001` section 3.5: its check suite and hooks). The plan says so for
each, and applying them is the operator's act.

**3. Plan and apply.** The profile is selected with `init plan|apply <path>
--profile github-actions-rust`, recorded in the declaration, and re-planned by
`env plan|apply` and `env upgrade` after that. No parallel command family.

1. **One typed plan.** The preflight computes the profile plan beside the
   governance plan and from the same reconciliation: each file's intended
   bytes and digest, its role, whether it is in the authority set, the
   command selections, prerequisites with their observed state, remote
   obligations, and each existing-file conflict. `init plan` shows exactly
   this and writes nothing (the 2026-09-24 rule). The plan has an identity: a
   digest over the profile identity, the declaration's `setup` block, and the
   observed digest of every path the plan reads or would write.
2. **Approved inputs.** `apply` accepts `--plan <identity>`. When given, apply
   recomputes the plan and refuses, writing nothing but the lock, if the
   identity differs. Without it, apply is the unattended form and reports the
   identity it performed.
3. **Authored content is preserved.** A profile path that exists and is not
   in the manifest is **not adopted and not replaced**: it is an
   `existing-authored` conflict, named with its digest, and the profile is
   `partial` for that path. Replacing it needs section 3.21's per-path
   ownership transfer, which is explicit, operator-initiated and recorded.
   Unlike a governance starter file, a workflow cannot be adopted in place,
   because adopting it would claim that the operator's CI is the profile's.
4. **Managed upgrade compares three digests**: previous (the manifest's),
   current (on disk) and intended (the new revision's). Current equals
   intended: record only. Current equals previous: replace. Otherwise the
   file is `customized`: left intact, and the conflict names all three
   digests and writes the intended bytes to
   `.statecraft/state/setup/<path>.intended` so the operator can merge by
   hand. Never a silent choice.
5. **Resume after interruption.** Before its first write, apply records the
   plan identity and the ordered path list under `.statecraft/state/`. A
   re-run with that record present re-plans; each path whose disk bytes equal
   the intended bytes is done, each whose bytes equal the previous (or
   absent) is written, and anything else is a conflict, never overwritten.
   The manifest is written last, under the lock, so an interrupted run leaves
   no manifest entry for a file it did not finish.
6. **Idempotent.** A repeat apply of the same plan writes no project file,
   and its mutation list shows at most the runtime state.
7. **Observed, not inferred.** Every write goes through the mutation recorder
   of the initialization-outcome entry.

**4. The gate policy.** `ci-gate` always runs (`if: always()`), needs every
other job, and passes only when each required job ended as its event
requires:

| Job | `pull_request` | `push` to default branch | Otherwise |
|---|---|---|---|
| `governance` | required: `success` | required: `success` | not triggered |
| `code` | required: `success` | required: `success` | not triggered |
| `ai-review` | required: `success`, carrying a review result (section 5) | **inapplicable** (no subject pull request): must be `skipped` | not triggered |
| `ci-gate` | aggregates | aggregates | |

- `failure` or `cancelled` in any required job blocks.
- **A required job that is `skipped` where it applies blocks** (an unexpected
  skip). A skip is accepted only where the table says the job is
  inapplicable, and the gate prints which rule admitted it.
- **A job that vanished blocks.** `ci-gate.sh` reads the required set from the
  policy document **at the trusted base** (`git show <base>:.statecraft/setup/...`)
  and refuses when the needs record lacks one of them. The coupling step
  inside `governance` runs only on `pull_request` with frozen
  `base.sha...head.sha` endpoints, as this repository's gate does; that is a
  documented step-level inapplicability, printed, not a skip of a job.
- **No `merge_group`.** A queue is not rendered until queued-candidate
  coupling and a waiver transport exist and are tested (AGENTS.md, "Before
  enabling a merge queue"). A trigger alone proves neither.
- **A candidate cannot weaken the policy that judges it.** GitHub runs the
  workflow definitions in the candidate on `pull_request`, so no file in the
  candidate can be the last word. The profile therefore does three things:
  the gate reads its required set from the base, not the candidate; any
  candidate that changes a profile path or the policy document is reported by
  `ci-gate` as an **authority change** (spec `001` section 3.5.2) in its
  summary; and the profile states a remote obligation that branch protection
  require code-owner review for those paths (section 6). The residual is
  named: a writer who can both change the workflow and satisfy the review
  can still weaken it, as on any GitHub repository.

**5. The AI review.**

1. **Subject.** The event's `base.sha` and `head.sha`, and the digest of the
   exact three-dot diff reviewed. Context is read from the base commit only.
2. **Permissions.** Workflow default `contents: read`; the review job adds
   `pull-requests: write` for its comment, nothing else. The caller forwards
   exactly one secret by name, never `secrets: inherit`. The product names the
   credential (`CLAUDE_CODE_OAUTH_TOKEN` for the pinned Claude Code CLI) and
   the operator command that sets it; it never reads, copies or prints a
   value, and no test uses a real one.
3. **Contributor content is data.** The diff and any body-derived line reach
   the reviewer on stdin, never through shell interpolation; `${{ }}`
   expressions carrying contributor text are bound to environment variables,
   never spliced into `run:`. The reviewer runs from an empty directory with a
   temporary `HOME`, so no configuration in the checkout is discovered.
4. **Untrusted code never meets a credential.** The trigger is
   `pull_request`, never `pull_request_target`; a fork receives no secret and
   is a visible skip, not a privileged run.
5. **A review is evidence, not text.** The reviewer must end its output with a
   fenced JSON block naming the subject head it was given, a verdict
   (`findings` or `no-findings`) and, for `findings`, entries whose paths are
   among the diff's changed paths. The runner builds the evidence record:
   profile identity, tool and version, subject (repository, pull request,
   base, head, diff digest), result, findings, and timestamps. It is posted as
   a comment and uploaded as an artifact named for the head sha.
6. **What is not a review, and blocks:** a missing credential on a
   same-repository pull request; empty output; output without the block, or
   whose block names another head or cites paths outside the diff (an
   unrelated last message); an explicit provider refusal; any unclassified
   failure; a head that moved before publication (stale subject, not
   counted); and a failed comment or artifact upload.
7. **Visible skips.** `draft`, `fork`, `dependabot`, `oversized` (over
   `review.diff_cap`) and a **recognized** transient provider failure, by the
   contextual signals spec-spine's 091 classifier uses. Each is posted, and
   each is recorded as `skipped:<class>`. **A green `ci-gate` therefore does not
   mean every pull request was reviewed**, and the report says which it was.
8. **AI findings never become human approval.** The job posts a comment,
   never an approving review, and nothing in the profile counts it toward a
   required approval.
9. **Release candidates** (pull requests whose head matches
   `release.branch_pattern`): *owner decision S-1 below*.

**6. Six results, reported separately.** `init`, `env` and `doctor` report,
per selected profile:

| Result | Means | Local init sets it |
|---|---|---|
| `files-installed` | every rendered path is present with the intended digest, or named as a conflict | yes |
| `local-checks` | `gate.sh governance` and `gate.sh code` ran here and passed | only with `--verify-local`; otherwise `not-run` |
| `remote-prerequisites` | the credential secret exists by name, Actions enabled, workflow token read-only by default | `unverified` |
| `required-checks` | branch protection on the default branch requires `ci-gate` (and, per section 4, code-owner review of profile paths) | `unverified` |
| `ci-executed` | a `ci-gate` run exists for the recorded head | `unverified` |
| `ai-review-produced` | an evidence record exists for that head, with its result | `unverified` |

Local initialization can be `complete` while the remote results are
`unverified`: the initialization-outcome words describe local steps, and a
setup is **complete** only when all six are satisfied. The product performs
no remote write: no secret, no branch protection, no pull request. A
read-only verification against the host is performed only when the operator
asks for it (`doctor --remote`), and without a reachable host it reports
`unverified`, never success.

**7. Acceptance obligations.** Each is a test in `crates/statecraft-home/tests/`
or through the built binary in `crates/statecraft-cli/tests/`, hermetic, with
no provider, host or network call:

1. A fresh temporary Rust repository: `init apply --profile
   github-actions-rust` then the rendered `install-spec-spine.sh` and `gate.sh
   governance` and `gate.sh code` run with the **real pinned** spec-spine and
   cargo, and exit 0.
2. A customized existing repository (an authored `.github/workflows/ci.yml`, a
   `Makefile`, a `.gitignore`): the authored files are byte-identical after
   apply, each is named as a conflict or left alone as specified, and every
   unrelated file is byte-identical (a digest walk before and after).
3. A repeat apply changes no project file.
4. An interrupted apply (the process killed after its Nth write) resumes to the
   same final tree as an uninterrupted one, and a file edited between the two
   runs is a conflict, not overwritten.
5. A managed upgrade from revision N to N+1: an unmodified file is replaced,
   a customized one is kept with the three digests and the `.intended` copy.
6. `--plan <identity>` refuses when an input changed after the plan.
7. The rendered workflows parse, and their `run:` scalars are **executed** in
   fixtures (spec-spine 091's method: the real scalars, stubbed `claude` and
   `gh`, and a harness that panics on an expression it does not support),
   covering: each required job `failure`, `cancelled`, missing from the needs
   record, and skipped where applicable; an inapplicable skip accepted on
   `push`; a missing credential; a valid review with findings and with
   none; empty output; unrelated output; a head mismatch; a classified
   transient failure; an explicit refusal; a fork; a stale subject; and a
   failed evidence upload.
8. **Mutation tests:** removing a required job from `ci-gate`'s `needs`, or
   from the base policy's required set only in the candidate, makes a test
   fail; so does inverting any blocking branch of `ai-review.sh`.
9. The plan reports each rendered workflow and script as an authority-set
   path, and the credential by name only (a test asserts no secret value
   appears in any output).

**8. Order.** (1) This entry, decided by the owner. (2) The planner,
renderer and reconciler as one `feat(002)` change under it. (3) This
repository's own adoption of the profile, as a **separate authority change**:
its `.github/workflows/**` and `Makefile` are unclaimed judging files, and
replacing them with rendered ones is decided on its own (decision S-5). (4) A
disposable remote acceptance run, only under an authorization that names the
remote target and the provider use (given by the owner on 2026-09-24: a new
private `statecraft-setup-acceptance-<YYYYMMDD>` repository and at most three
review invocations, run by the operator's session, not by this change). Publication of a released binary and a
fresh released-binary consumer are separate from the source merge.

*Decisions, as the owner made them (session request of 2026-09-24).* S-0:
this entry is adopted as written, with the choices below. Each item keeps the
option text it was decided from; the option not taken is recorded, not
binding.

- **S-1: release candidates. Decided: (a).** A release candidate needs an
  actual review (`findings` or `no-findings`) or an explicit owner exception
  recorded outside the candidate: an approval on a GitHub Environment
  `statecraft-review-exception` whose required reviewers the operator sets,
  which GitHub records by name and which the author cannot grant. (b) Release
  candidates take the same visible skips as any other pull request.
- **S-2: ordinary pull requests. Decided: (a).** Keep the five visible skip
  classes of section 5 item 7 non-blocking, as spec-spine does today. (b)
  Make `oversized` block, so a large change needs the exception of S-1.
- **S-3: the trusted-base defense. Decided: (a), all three.** Base-read required set,
  authority-change reporting, and code-owner review of profile paths stated as
  a remote obligation (a `CODEOWNERS` rendered only when none exists). (b)
  The first two only, with the residual accepted.
- **S-4: the exact pin prerequisite. Decided: (a).** The profile requires an
  exact `spec-spine` pin and is withheld, naming the prerequisite, in an
  unpinned project; how a new project gets one is the separate exact-pin
  proposal. (b) The profile writes the pin itself, which amends section 3.15's
  unchanged-bytes rule here instead of there.
- **S-5 (at step 3, named now). Decided: adopt by rendering.** Whether this
  repository adopts the rendered surface. Its current judging files are claimed by no spec (constitution VII:
  a spec does not own the rules it is judged by), while the profile's
  templates are this spec's territory. Decided: adopt by rendering, keep
  the rendered paths unclaimed, and make every later profile upgrade here its
  own authority change, so a template edit in this spec never changes this
  repository's gate in the same change.

**2026-09-24: provenance of what initialization writes (amends sections
3.2, 3.3, 3.5, 3.6 and 3.15).** Adopted by the repository owner in the
session request of 2026-09-24: bundle proposal Part 9 step 3a, with the
proposal's H-4 option (a) (a role on `managed` entries), P-1 option (i) (the
declared pin, or `unpinned`), and H-3 option (a) (owner Addendum 2,
2026-09-24: one producer identity, the CLI pin and the linked library being
the same release). It adopts only the parts of the 2026-09-23 proposed bundle
entry named here, and none of its bundle, store, resolution, migration or
trust parts. Its implementation is a separate `feat(002)` change.

1. **A role on `managed` entries (amends 3.2 and 3.3).** The three classes of
   section 3.2 stay disjoint and exhaustive. A `managed` entry gains `role`:
   `reference` (the default, and every entry recorded before this entry) or
   `authored-input`. The `authored-input` list is closed and stated here,
   never inferred from bytes: `spec-spine.toml`,
   `<specs_dir>/000-bootstrap/spec.md`, `<standards_dir>/constitution.md`
   and `<standards_dir>/contract.md`. Everything else this product writes
   (templates under `<standards_dir>/templates/**`, `.statecraft/AGENTS.md`,
   adapter pointer files) is `reference`. Changing an existing entry's role
   is a per-path, operator-initiated, recorded act through section 3.35's
   transfer mechanism, never automatic.
2. **What an authored input means (amends 3.5 and 3.6).** Written only when
   absent, and recorded with its **seed digest** and the producer identity;
   an existing file is `adopted`, as today. Editing it is expected. `doctor`
   reports `seeded` while its digest equals the seed and `customized` when
   not, both as information with no finding, and `missing` as today; it
   states that the digest comparison is not the control for authored text
   (spec-spine's `check`, the coupling gate and review are). Upgrade never
   rewrites it and reports a newer seed by its digest. Removal deletes it
   only while its digest equals the seed, and otherwise keeps and names it.
   A `reference` entry that differs from its digest is `drifted`, as today.
3. **The producer is one recorded identity (amends 3.3 and 3.15).** The
   declaration's pins gain `producer`: the linked library's crate name, exact
   version, and the crates.io checksum `Cargo.lock` records for it, fixed
   when this product is built and derived from the build, never from a
   second literal (`D-06` as amended, spec `001` section 3.13); a test checks
   it against the lock file. Section 3.15's "recorded in the declaration's
   pins" is thereby true. Under H-3 (a) the CLI pin and the linked library
   are **one producer identity, one release**: `pins.producer` and
   `pins.spec_spine` (item 4) are never written or reported as two
   independently qualified identities. `pins.producer` is that identity as
   this build links it; `pins.spec_spine` is what the project declares it
   requires of the same release. A declaration written before this entry
   reads with `producer` absent, and says so ("recorded before
   provenance"); it is never given a guessed value.
4. **`pins.spec_spine` stops recording an observation as a pin (amends 3.3;
   P-1 (i)).** Until now it held the `--version` answer of whatever
   `spec-spine` was first on `PATH` when `init` ran (`C-02`). From this entry
   it holds the repository's declared exact pin, read from the uncommented
   `required_version` line of `spec-spine.toml`'s `[meta]` table after the
   governance step, as the bare version (`0.25.0` for `"=0.25.0"`), or the
   word `unpinned` when there is no such line or its requirement is not
   exact (`=` followed by three numeric parts, section 3.23 contract 2's
   reading of the same line). It is **never** the version found on `PATH`.
   The observation is kept, and named as one: the init report carries
   `observed_spec_spine` (the path, the version it answered, and how it was
   found), and the home's `tools.json` keeps its `spec-spine` record as the
   observation it already is (`resolved`, `observed_from`). Neither is a
   pin. `doctor` reports two disagreements, each on its own line naming both
   values: the observed executable differing from the declared pin, as a
   finding; and the declared pin differing from the producer identity's
   version, as information, because a project may move its own pin (the
   entry above, item 7). `unpinned` is itself reported, as information. Until the producer's scaffold can write an
   exact pin (the owner's exact-pin decision of 2026-09-24, through the
   producer's own option, never a post-edit of library output), a newly
   initialized project records `unpinned`.
5. **`[index]` is passed explicitly (amends 3.15).** "Passed explicitly,
   never defaulted" extends from `[layout]` to `[index]`:
   `resolver_exclusions` is derived from the declared layout (the derived and
   state roots) plus build directories (`target`, `node_modules`, `dist`,
   `build`, `.next`) and the local tool directory `.tooling`, and names no
   other derived directory. A test on the real library asserts it, so a
   producer change that reintroduces a literal is caught when the producer
   moves.
6. **Evidence.** The implementation is a separate `feat(002)` change, tested
   through the built binary on real directories with an isolated `HOME`:
   pin-line and date edits to an authored input read `customized`, exit 0,
   and a template edit reads `drifted`; a non-default `derived_dir` appears
   in no exclusion but its own; the producer version, the observed
   executable and the project pin disagreeing are distinct reports naming
   both values (a finding and information, item 4); and a declaration written before this entry is read without
   guessed values.

This entry names no bundle, store or resolution, and changes no section
3.23 contract.

**2026-09-24: provenance of what initialization writes, implemented (the
entry above).** Choices the entry left open, recorded here:

- **The producer identity comes from the lock file at build time.**
  `crates/statecraft-home/build.rs` reads the workspace `Cargo.lock` and
  fixes `PRODUCER_VERSION` and `PRODUCER_CHECKSUM` for `spec-spine-core`;
  `producer::linked()` is what the pins record. No literal version remains in
  `producer.rs`, and a test compares both constants with the lock file.
- **`role` is written only by a first write.** `ManagedFile::authored_input`
  marks the four closed-list paths in the governance declaration; a path
  already recorded keeps its recorded role, so a declaration from before this
  entry keeps `reference` for all of them. A move to `managed` through
  section 3.35 records `reference`; the operator's role-change act is not
  built.
- **The seed is the entry's `digest`.** An authored input is never rewritten,
  so its recorded digest stays the seed; no second field is added.
- **What is kept is not withheld.** An authored input on disk goes to the
  plan's `kept` list (`seeded` or `customized`, and a newer seed by its
  digest), never to `withheld`, so it never makes an initialization partial.
  The init report carries `kept`; `env plan` prints `keep` lines.
- **`doctor` carries `notes`,** information that never changes the exit
  code: `unpinned`, "recorded before provenance", and the sentence that the
  seed comparison is not the control for authored text, and the declared pin
  against the producer ("declared X, producer identity spec-spine-core@Y").
  The executable disagreement is the finding "declared X, observed
  executable Z". Both compare the recorded `pins.spec_spine`.
- **The declared pin needs an uncommented `[meta]` header.** The producer's
  scaffold comments the whole table, so a new project records `unpinned`;
  pinning is adding the table. The pins are re-read on every `init apply` and
  on an `env apply` that creates the declaration, so a project that adds a pin
  records it on the next run.
- **The observation is `observedSpecSpine`** in the init report: `program`,
  `version`, and `foundBy` (`path` for a bare name, `explicit` for a path).
  The home's `tools.json` record is unchanged.
- **I-3 keeps its meaning for reference files.** The I-3 implementation
  entry above tested a hand-edited `spec-spine.toml`; under this entry a
  newly initialized `spec-spine.toml` is an authored input, so an edit is
  `customized` and kept, and the run stays `complete`. The I-3 test in
  `init_outcome.rs` now edits a template (a `reference` entry), which is
  still withheld as `drifted`, `partial`, exit 1, both digests named. A
  `spec-spine.toml` recorded before this entry keeps `reference`, so for it
  the I-3 behavior is unchanged.
- **`[index] resolver_exclusions`** is `target`, `node_modules`, `dist`,
  `build`, `.next`, `.statecraft/derived`, `.statecraft/state` and
  `.tooling`, passed in `config_json()`; the real library renders exactly
  that list and not `.derived`.

Evidence, through the built binary on real directories with an isolated
`HOME` (`crates/statecraft-cli/tests/provenance.rs`) and on the library
(`crates/statecraft-environment/tests/authored_inputs.rs`,
`producer_integration.rs`): roles, the linked producer and `unpinned` on a
fresh project whose `PATH` answers 0.23.0; pin-line and date edits read
`customized` and add no finding, and a re-run keeps them as `kept`; a template
edit reads `drifted`, exit 1; a declared 0.24.0 against the producer and a
0.23.0 executable gives a finding and a note, each naming both values; a declaration
written before this entry reads with no producer guessed and a note saying so.

**2026-09-24: the setup profile implemented (the entry above on a
repository setup profile, adopted with S-0 to S-5).** The planner, renderer
and reconciler are one change under that entry, its order item (2). This
repository's own adoption (S-5, item 3) and the disposable remote run (item
4) are not part of it.

*What is built.* `crates/statecraft-home/src/setup.rs` holds the one
registered profile, `github-actions-rust` revision 1, as a closed value; its
templates are data under `crates/statecraft-home/profiles/github-actions-rust/`
and its identity is a digest over them, the ignore fragment and the static
policy. `init plan|apply <path> --profile <id> [--plan <identity>]
[--verify-local]` plans it in the preflight beside the governance plan and
from the same reconciliation; an unknown profile, an unknown or out-of-bounds
parameter, or an approved plan identity that is not the plan now is a
refusal before any write. The apply writes the resume record first, each file
through the flow's observed recorder, the `.intended` copies, and records the
manifest entries, which the flow writes last; the resume record is removed
after that write. A declaration that selects a profile is re-planned by `init
plan|apply` without the flag. The governance step reports `withheld`, naming
each conflict, when the profile is withheld or has a conflict, so the outcome
is `partial` (the I-3 rule). `doctor --remote [--head <sha>]` reads the four
remote results through `gh api -X GET` only and reports the six separately;
a host that cannot be asked is `unverified`.

*Choices the entry left open, decided here.*

1. **Parameter marker.** Templates name parameters as `{{sc:name}}`, so a
   GitHub expression (`${{ ... }}`) is template text and never substituted.
   An unknown or unterminated marker is a template defect, reported and never
   written.
2. **`review.code_owners`.** S-3 (a) renders a `CODEOWNERS` "only when none
   exists", and a `CODEOWNERS` needs owners, which the entry's parameter list
   does not carry. One declared parameter supplies them: a list of `@handle`
   values. Absent, no `CODEOWNERS` is rendered and the plan says code-owner
   review stays a remote obligation. An existing `CODEOWNERS` at any of the
   three places GitHub reads is the user's and is left alone.
3. **Coverage is reported, not enforced, in the rendered gate.** `gate.sh
   governance` runs `check --fail-on-warn`, `lint --fail-on-warn`, `index
   coverage` and `index check --fail-on-unresolved`. Measured on 2026-09-24
   with the pinned spec-spine on a fresh `cargo new --lib` project initialized
   with the profile: every verb exits 0 except `index coverage
   --fail-on-untraced`, which exits 1 because the project's own `src/lib.rs`
   and the four rendered scripts are unclaimed until it writes the specs that
   claim them. A project adds the flag when its coverage debt is retired, as
   this repository did.
4. **Every local prerequisite withholds, not only the pin.** S-4 withholds an
   unpinned project; the same rule applies to a missing `rust-toolchain.toml`,
   `Cargo.lock` or git work tree, because the rendered gate cannot run without
   them. Each is named with its observed state.
5. **The gate and the reviewer run the base's scripts.** `ci-gate` reads both
   its policy and `ci-gate.sh` at the base commit, and the review job reads
   `ai-review.sh` at the base, falling back to the candidate's copy only when
   the base carries none (the adoption), which each says. The residual the
   entry names is unchanged: the candidate's workflow definition still runs.
6. **A fork or Dependabot skip is not posted as a comment.** Their token is
   read-only; the skip is recorded in the evidence record and the job summary.
   Every other visible skip is posted before its result is claimed, and a skip
   notice that cannot be posted blocks.
7. **The review tool.** The Claude Code CLI pinned at 2.1.116, spec-spine's
   own pin for the same reviewer, installed by `npm` only when a review will
   run.
8. **Actions pinned by commit:** `actions/checkout` v7.0.0 and
   `actions/upload-artifact` v7.0.1, as spec-spine's workflows pin them.

*Evidence (tested, local).* Through the built binary, with an isolated
product home, `crates/statecraft-cli/tests/setup_profile.rs`: a fresh Rust
project applies the profile with `--verify-local` and its rendered
`install-spec-spine.sh`, `gate.sh governance` and `gate.sh code` exit 0 with
the real pinned spec-spine and cargo (obligation 1); authored files stay
byte-identical by a digest walk, with the conflict and the two left-alone
files named (2); a repeat apply changes no profile file and no profile record in the
declaration (3; on this base the governance step re-dates some of its own
declaration entries on a repeat run, which is that step's behavior and is
reported separately); an apply stopped
at its first script write by a read-only directory resumes to the same profile bytes
and manifest entries as an uninterrupted run, and a file edited in between is
a conflict with its `.intended` copy (4); `--plan` refuses a changed input,
writing no project file (6); the plan marks the authority set and names the
credential only, and a credential value in the environment appears in no
output and no file (9); `doctor --remote` against a stub `gh` reports all six,
issues reads only, and reports `unverified` for an unreachable host. The
library, `crates/statecraft-home/tests/setup_upgrade.rs`: revision N to N+1
replaces an unmodified file and keeps a customized one with three digests and
the `.intended` copy (5). The rendered workflows,
`crates/statecraft-home/tests/setup_workflows.rs`: the aggregate step's and
the review step's real `run:` scalars run under `bash -e` with stub `claude`,
`gh` and `npm`, and an expression the harness does not state panics; every
case of obligation 7 is covered, and each blocking branch of `ci-gate.sh`
(8) and `ai-review.sh` (15) is inverted in turn and caught, as is a
candidate that drops a job from its own policy (8).

*Not built here.* `env plan|apply|upgrade` do not re-plan the profile; `init
apply` does, and is the only verb that writes it. Job-level `if:` conditions
(the review exception's) are asserted by structure, not executed. The remote
results read existence: `ai-review-produced` names the artifact, and the
review's result is in the record and the pull-request comment, not read back
by `doctor`. No remote was touched and no provider was invoked.

*Measured on the way.* The workflow harness first wrote its three stub
programs afresh for every case: 330 s for the suite on this machine. Written
once per process and reused, the same cases took 23 to 39 s. That is the
fresh-executable first-exec stall the 002 and 003 acceptance diagnostic
investigates, observed here incidentally and not measured in isolation.

**2026-09-24: a re-run keeps the declaration's bytes when nothing it records
changed.** Found by the setup-profile implementation (#115) and reproduced
through the built binary: a second `init apply` of an unchanged project
rewrote the planned governance files with the bytes already on disk and
re-dated their entries, so the committed `.statecraft/environment.json`
changed on every run although no file did. The section is silent on what
`written_at` means for an unchanged write; it now means when this product last
wrote the file. A planned write whose digest is already on disk, over an entry
that records exactly what the write would record apart from `written_at`, is
skipped: no file write, no entry change. The plan still lists it as a write,
because the plan's decision (the bytes are this product's to write) is
unchanged; the mutation list, which reports what the run changed, no longer
names the declaration. `t12_a_re_run_lists_only_what_changed` asserts the
declaration is byte-identical across a re-run a second apart and that no
project path is a mutation; without the fix it fails ("a re-run re-dated the
declaration").

**2026-09-24: profile revision 2, the AI review is the final approver
(adopted by the owner on 2026-09-24, decisions R2-1 (a) and R2-2 (a); amends the
setup-profile entry's gate policy, S-1 and S-3).** Measured in the remote
acceptance run of 2026-09-24 (evidence commit 420979f, `session10/R`): the
review job succeeds whatever its verdict, so on PR B a `findings` verdict did
not block by itself (clippy did), and `ci-gate` could pass a head the reviewer
flagged. The owner's direction is `ci-gate` with the AI review as the final
approver and no human approval for ordinary changes.

1. **A findings verdict blocks.** `ci-gate` refuses a head whose review
   result is `findings` unless the owner's exception was approved for that
   run. The exception is the S-1 protected Environment
   (`statecraft-review-exception`), which now applies to two cases: a release
   candidate whose review was skipped (as in revision 1), and any pull request
   whose review returned `findings`. Approving it is the owner's act in the
   forge; rejecting it, or leaving it pending, keeps the head unmergeable. A
   verdict recorded for another head never counts: the review's verdict is
   already bound to the subject head and diff digest.
2. **No human approval for ordinary changes; code-owner review stays for the
   profile's own files (R2-2 (a)).** The operator steps name required
   approvals 0 with code-owner review required, so only a change to a path
   CODEOWNERS lists waits for the owner. `doctor --remote` keeps judging
   code-owner review as required.
3. **`ci-gate` is bound to its source.** The operator steps require
   `ci-gate` from GitHub Actions (or, under the local gate once adopted, the
   organization's app), so a status any token posts does not satisfy it;
   `doctor --remote` reports an unbound `ci-gate` as not satisfied.
4. **Skips stay non-blocking (S-2).** A visible `skipped:<class>` result is not
   a `findings` verdict and is admitted as in revision 1.
5. **Revision.** The profile becomes revision 2, a new identity; a project on
   revision 1 upgrades through `init apply --profile`, which rewrites the
   profile's unchanged files and withholds drifted ones as for any managed
   path. Acceptance obligations: a `findings` result without an approved
   exception blocks `ci-gate`; with one it passes; `no-findings` and each skip
   class behave as in revision 1; the operator steps and `doctor --remote`
   carry items 2 and 3; tests in `crates/statecraft-home/tests/` extend the
   revision-1 suites.

**2026-09-24: profile revision 2 implemented (the entry above on revision 2,
R2-1 and R2-2).** `setup.rs` registers revision 2 (identity
`bfe420c0f7694d55eca0ab5ae396b7bf5819222ecf1295dd76ad66bb8fb4aa6f`). The
policy names the `review-exception` job's pull-request rule `owner-exception`:
`ci-gate.sh` makes it required for a `findings` result and for a release
candidate whose review was skipped, and blocks a `findings` head whose
exception did not succeed with a message naming the verdict and the
exception. The script keeps revision 1's `rc-exception` rule, because a
revision-1 base's policy is what judges the pull request that upgrades it. The
rendered workflow runs the exception job for a `findings` result as well as for
revision 1's case. The operator steps state required approvals 0 with
code-owner review required and `ci-gate` required from GitHub Actions (app id
15368, the constant `GATE_APP_ID`); `doctor --remote` reports a `ci-gate` with
no app or another app as not satisfied and names the binding. Tests:
`setup_workflows.rs` replaces revision 1's case "pr, findings do not block"
(exit 0) with three cases (no exception, rejected, approved; the mutation test
covers them) and asserts the exception job's condition; `setup_upgrade.rs`
upgrades a revision-1 project to revision 2 (the unchanged gate script is
rewritten, a customized workflow is withheld) and checks the operator steps;
`setup_profile.rs`'s `gh` stub now reports a bound `ci-gate` for the satisfied
case and adds an unbound mode that must read not satisfied.

**2026-09-24: the linked producer's version is stated once (spec `001`
section 3.13, `D-06` as amended).** `crates/statecraft-home` takes
`spec-spine-core.workspace = true`, so the root `Cargo.toml`'s
`[workspace.dependencies]` is the only statement of the version and features.
`PRODUCER_VERSION` was already derived from `Cargo.lock` by `build.rs` (the
provenance entry above). The manifest test now reads the root manifest and
asserts the member inherits it, and a new test asserts `spec-spine.toml`'s
`required_version` names the same release (one producer identity, H-3 (a)):
pinning the CLI to another release fails it. The two ownership-transfer
assertions derive the expected `spec-spine-core@<version>` from the constant.
`Cargo.lock` does not change.

**2026-09-24: profile revision 3, the merge queue (adopted by the owner on
2026-09-24; amends the setup-profile entries and revision 2).** Revisions 1 and
2 render CI that triggers on `pull_request` and `push` only. A repository that
requires a merge queue, as statecraft-cli did on 2026-09-24, never gets
`ci-gate` reported for a queued entry, so its queue stalls. Revision 3 makes
the rendered CI judge a queue entry without spending a second review.

1. **The rendered CI triggers on `merge_group`.** Governance and code run on
   the queued candidate as on any other event, and are required there.
2. **The review is not re-run in the queue.** The `ai-review` job stays
   pull-request only, so a queue entry spends no provider review. Its skip on
   `merge_group` is admitted only through rule 3, never as an ordinary
   inapplicable skip.
3. **`ci-gate` requires the verdict already recorded for the entry's pull
   request.** On `merge_group` it reads the pull-request number from the
   group's `head_ref` (`.../pr-<n>-<sha>`) and that pull request's head
   through the API. It then finds the latest completed pull-request run of the
   rendered CI for exactly that head, and reads the review evidence record the
   run uploaded (`statecraft-ai-review-<head>`), whose subject must name the
   same pull request and head. Admitted: `no-findings`; `findings` only when
   that run's `review-exception` job succeeded (R2-1); a visible
   `skipped:<class>` as in revision 1, and for a release candidate only with
   the exception (S-1). Refused: no recorded run, no record, a record for
   another pull request or head, or an unreadable record. The job reading the
   record needs `actions: read` and `pull-requests: read`, and nothing that
   writes.
4. **Coupling on `merge_group`.** Governance couples the group's
   `base_sha...head_sha` and reads a waiver from the entry's pull-request body
   through the API, honouring it only when the group changes no path that pull
   request does not change; otherwise it couples with no waiver and fails
   closed. This is the rule statecraft-cli's own CI adopts in its #130.
5. **A skipped required job never passes.** The policy states a rule for every
   event it triggers on (`push`, `pull_request`, `merge_group`); a required
   job's skip is admitted only by a rule that names why, as in revision 1.
6. **Upgrade order.** The gate reads its script and policy at the base, so a
   pull request upgrading to revision 3 is judged by the base's revision-2
   policy, which states no `merge_group` rule. Upgrade to revision 3 before
   requiring a merge queue, or merge the upgrade while the queue is not
   required; the operator steps say so. The revision becomes 3, a new
   identity.

Acceptance obligations, as tests in `crates/statecraft-home/tests/`: a queue
entry with a recorded `no-findings` verdict for its pull request's head passes;
no recorded verdict blocks; `findings` without a successful exception blocks
and with one passes; a record naming another head blocks; the review job's
skip on `merge_group` is admitted only with a recorded verdict; the rendered
workflow triggers on `merge_group` and grants the gate job only read
permissions; a revision-2 project upgrades to revision 3.

**2026-09-24: profile revision 3 implemented (the entry above on the merge
queue).** `setup.rs` registers revision 3 (identity
`5e30836810d88d294027788376354495bc73333363607002ea968b7153fefb12`); the
policy states a `merge_group` rule for every required job (`required` for
governance and code, `recorded-review` for `ai-review`, `inapplicable` for the
exception job), and the operator steps state the upgrade order. The rendered
workflow triggers on `merge_group`; governance gains a read-only
`pull-requests` permission and a merge-queue coupling step
(`gate.sh couple-group`, the waiver rule of statecraft-cli's #130); `ci-gate`
gains read-only `actions` and `pull-requests` permissions and the queue's
endpoints. `ci-gate.sh`'s `recorded-review` rule requires the review job to be
skipped, then reads the entry's pull request, its latest completed
`statecraft-ci` pull-request run at that head, the uploaded evidence record
(whose subject must name the same pull request and head) and that run's
exception job; it admits `no-findings`, `findings` with an approved exception,
and a visible skip (a release candidate's only with the exception), and blocks
every other case with a reason. Tests: `setup_workflows.rs` adds seventeen
queue cases through the rendered step with a stubbed `gh` (three admitted,
fourteen blocked, none calling the reviewer or posting), includes them in the
mutation test over every blocking branch, and asserts the trigger, the
gate's read-only permissions and the pull-request-only review;
`setup_upgrade.rs` rebuilds revision 2 from revision 3 and upgrades it (the
unchanged gate script is rewritten, a customized workflow is withheld), and
asserts the queue rules and the upgrade order. The merge-queue coupling step is
not exercised by a local test (it needs the pinned spec-spine and the API);
its first live queue run is its evidence.

**2026-09-24: profile revision 4, the checks S-5 found missing (adopted by the
owner on 2026-09-24; amends the setup-profile entries and revisions 2 and 3).**
S-5 renders this repository's own CI from the setup profile. The render of
revision 3 into a scratch copy of `main` (profile identity `5e30836810d8`;
evidence repository commit `a6b0bb8`, `2026-09-24/session11/S5`) passes every
governance check and matches `.github/workflows/govern.yml` on the pin,
`check`, `lint`, `index check`, coupling on both events, `make code` and the
`ci-gate` binding, and is stronger on `ci-gate`. It is weaker in five places
and has no parameter for any of them, so adopting it would weaken the check
suite. Revision 4 adds the parameters; each default keeps a revision-3
project's behaviour except rule 5, which is a new refusal.

1. **`governance.enforce_coverage`** (boolean, default `false`). When `true`,
   `gate.sh governance` runs `index coverage --fail-on-untraced`, a refusal
   rather than a report.
2. **`governance.authored_content`** (a repository-relative path, default
   unset). When set, `gate.sh governance` runs that script and **refuses (exit
   1) when it is absent or not executable**; revision 3's "run it if it is
   executable" is removed, so deleting the script can no longer pass silently.
   When unset, no authored-content step runs and `doctor` says so. The upgrade
   from revision 3 sets it to `scripts/check-authored-content.sh` when that
   file exists, which keeps the check a revision-3 project already ran.
3. **`governance.authored_content_text`** (boolean, default `false`; requires
   rule 2's path). When `true`, the declared script is also run as
   `<script> --text FILE...` on the pull request's title and body, from the
   event on `pull_request` and through the API on `merge_group`, and on every
   commit message in the change. The script's `--text` contract is spec `001`
   section 3.6's: the same rules, exit 0 clean, 1 findings, 3 usage.
4. **`governance.gate_each_commit`** and **`governance.require_signed_commits`**
   (booleans, default `false`). On `pull_request` and `merge_group` the
   `governance` job walks every commit in the change's `base..head`. With
   `gate_each_commit`, each commit's tree must pass `gate.sh governance` and
   `cargo fmt --all --check`, using the spec-spine pin that commit's own
   `spec-spine.toml` names. With `require_signed_commits`, each commit must be
   verified as signed by GitHub (`commit.verification.verified` through the
   API, the verification branch protection reads). The walk is a step in the
   `governance` job, never a job of its own, so it cannot be skipped into a
   green gate.
5. **`governance.require_default_base`** (boolean, default `true`). A pull
   request whose base is not `default_branch` fails `governance`: a stacked
   pull request merges into another branch and is never judged against the
   default branch.
6. **Caches, not a check.** The rendered CI caches the installed spec-spine
   binary keyed on the pin, and the cargo registry, git and `target/` keyed on
   `Cargo.lock` and `rust-toolchain.toml`. A cache never decides a verdict.
7. **Upgrade.** The revision becomes 4, a new identity. `ci-gate`'s policy is
   unchanged: every addition is a step inside a job the policy already
   requires. The operator steps name rule 5 as a new refusal and the
   parameters to set for a repository that already runs these checks by hand.

The local `make gate` and `make code` delegating to `gate.sh` is S-5's
adoption concern, not the profile's. The render's corpus step also ran a bare
`spec-spine` from `PATH` (0.24.0 on this machine) and exited 4 until the pinned
binary came first; it fails closed, and the single selection variable proposed
in #119 is where that belongs.

S-5, next: render revision 4 with rules 1 to 5 enabled, keep every property
`govern.yml` has, and add the AI review job and the protected
`statecraft-review-exception` Environment (owner, 2026-09-24: the review is
what S-5 adds over `govern.yml`). Before that pull request can pass, the owner
sets `CLAUDE_CODE_OAUTH_TOKEN` and the Environment's required reviewers; the
review then uses the provider on every pull request.

Acceptance obligations, as tests in `crates/statecraft-home/tests/`: coverage
enforced refuses an untraced file and reported does not; a declared
authored-content script that is missing or not executable refuses, and an
undeclared one runs nothing; the text mode refuses a U+2014 in a title, a body
and a commit message; the commit walk refuses an unsigned commit (a `gh` stub)
and a commit whose own tree fails the gate while the head passes; a base other
than the default branch refuses and the default branch passes; the rendered
workflow keeps the gate job read-only; a revision-3 project upgrades to
revision 4 with `governance.authored_content` set when the script exists and
unset when it does not.

**2026-09-24: profile revision 4 implemented (the entry above on the checks
S-5 found missing).** `setup.rs` registers revision 4 (identity
`998c0d7eb8f2890a3d1558b3c9b2a73ff199722d01d6a6e149b8f52237ab4d7b`) and
validates the six `governance.*` parameters: five booleans, the path
(repository-relative, no `..`, no leading `/` or `-`), and
`authored_content_text` refused without the path. `gate.sh` carries them as
rendered values. `governance` passes `--fail-on-untraced` when coverage is
enforced, and runs a declared script as `./<path>`, exiting 1 when it is absent
or not executable; with none declared it says that none runs. Revision 3's
`if [ -x ... ]` is gone. New subcommands: `base` (rule 5), `text` (the title
and body, from the event or through the API), `commits` (the walk:
`commit.verification.verified` through `gh api`, each message through
`--text`, and each commit's own `gate.sh governance` and `cargo fmt --all
--check` in a detached worktree using the release its own `spec-spine.toml`
pins, falling back to the running `gate.sh` when the commit has none), and
`pin` (the cache key). The rendered workflow keeps the same five jobs and
calls each subcommand as a step of `governance`. It caches
`.tooling/bin/spec-spine` on the pin in both jobs, and the cargo registry, git
and `target/` on `Cargo.lock` and `rust-toolchain.toml` in `code`, using
`actions/cache` pinned by commit; the install step always runs and still checks
the version. The policy's `commands` follow the parameters, and `ci-gate`'s
jobs and rules are unchanged. The upgrade declares
`scripts/check-authored-content.sh` for any recorded revision below 4 when
the file exists. Revisions 1 and 2 ran the same `if -x` check, so this keeps
the check they ran too (an agent's choice, recorded here). `doctor --remote`
names the authored-content step in its `local-checks` detail, and `init plan`
names it in its rendering. Tests: `setup_workflows.rs` runs the rendered
steps with a stubbed `spec-spine`, a stubbed `cargo`, the `gh` stub and this
repository's `scripts/check-authored-content.sh`, one test per obligation:
coverage enforced and reported; a declared script missing, not executable or
refusing, and an undeclared one running nothing; U+2014 in a title, a body
(event and queue) and a commit message; an unsigned commit, and red
intermediate trees (gate and fmt) under a green head; a stacked base refused
and the default branch passing. A structural test asserts steps rather than
jobs, read-only `governance` and `ci-gate`, and the caches.
`setup_upgrade.rs` rebuilds revision 3 from revision 4 and upgrades it with
and without the script, and asserts the unchanged policy and the operator
steps. `setup_profile.rs` asserts the `doctor --remote` note. Not exercised
locally: the real spec-spine behind these steps (its flags are this
repository's own gate), and the walk's install of a different pinned release
for an older commit (it needs the network); the first live run is their
evidence.

**2026-09-24: two defects in revision 4's `gate.sh`, found by the first live
AI review (statecraft-cli #138).** Both are corrections inside the adopted
revision, so the revision stays 4 and only the profile identity changes.
(1) `gate.sh code` had lost the guard the hand-written CI had: in a workspace
with no member crates, every `cargo --workspace` verb refuses the virtual
manifest. It now asks `cargo metadata --no-deps` first and, with no workspace
members, says that build, test, clippy and fmt judge nothing and passes; with
a member it runs all four as before. (2) In the commit walk, a commit whose
`spec-spine.toml` states no exact pin was refused with an empty log, because
the reason went only to stderr before the log existed. The reason is now also
written to the log that the refusal prints. Tests:
`the_code_gate_judges_nothing_without_member_crates_and_says_so` and
`a_commit_without_an_exact_pin_is_refused_and_says_why` in
`setup_workflows.rs`, each observed failing against the previous `gate.sh`.

**2026-09-24: two more corrections to revision 4, from the second live AI
review of #138.** (1) `ai-review.sh` posted a skip notice on every run, so a
re-run of the job (a transient skip is the likely case) repeated it. The
notice now carries a marker naming its class and head, and a run that finds
that marker in the pull request's thread does not post again; a thread that
cannot be read gets the notice anyway, because a duplicate is better than an
invisible skip. The skip is recorded either way. (2) The empty-workspace guard
added above matched cargo's JSON text as serialized; whitespace is now removed
before matching, and a failed `cargo metadata` stops the gate. Tests:
`a_rerun_does_not_repeat_a_skip_notice`, and the guard's test now feeds spaced
JSON; both observed failing against the previous scripts.

**2026-09-24: S-5, this repository's CI is rendered from the profile
(revision 4; owner decision of 2026-09-24).** `init apply` with
`--profile github-actions-rust` was run on this repository, and the
`project.setup` block of `.statecraft/environment.json` sets
`governance.enforce_coverage`, `governance.authored_content`
(`scripts/check-authored-content.sh`), `governance.authored_content_text`,
`governance.gate_each_commit`, `governance.require_signed_commits` and
`review.code_owners`; `governance.require_default_base` keeps its default.
Every property `.github/workflows/govern.yml` had is kept, so that file is
removed in the same change: both would report `ci-gate`. `make gate` and
`make code` now run `scripts/statecraft/gate.sh`, one definition for local and
CI. The AI review and the `statecraft-review-exception` Environment are what
S-5 adds; the owner sets `CLAUDE_CODE_OAUTH_TOKEN` and the Environment's
required reviewers. The adoption pull request is judged by the candidate's own
`ci-gate.sh`, because the base carries none (revision 1's adoption rule). The
measurement tables above that name `govern.yml` record what was true when they
were written and are not rewritten.

**2026-09-24: the authority-change report compares with the fork point (a
correction inside revision 4, from the third live AI review of #138).**
`ci-gate.sh` listed the candidate's changed paths with a two-dot diff, so on a
pull request whose base moved on after the branch was cut it reported the
base's later changes to the profile's files as this candidate's authority
change. It now uses `BASE...HEAD`, the candidate's own changes, as coupling
does. The report stays informational. Test:
`an_advanced_base_is_not_reported_as_the_candidates_authority_change`,
observed failing against the previous script. The same review's second
finding, that the governance job and the commit walk run the candidate's own
`gate.sh`, is a property of the profile since revision 1 and is proposed as
revision 5 in its own change.

**2026-09-24: profile revision 5, a candidate never judges itself with its own
gate (ratified by the owner on 2026-09-24).** Found by the third live AI review of #138
and confirmed by reading the profile: only `ci-gate.sh` and the policy are read
at the base. The `governance` job, the commit walk and the declared
authored-content script run the candidate's own copies, and on `pull_request`
GitHub runs the candidate's own workflow file. A pull request can therefore
weaken the check that judges it. The authority-change report names the change
but does not block it, and the code-owner review the profile relies on
(S-3) does not separate an agent from the owner when agents act under the
owner's account. The hand-written `govern.yml` had the same property, so
revision 4 is no weaker than what it replaced; revision 5 closes it.

1. **The gate's scripts are read at the base.** The `governance` job, the
   commit walk and the `code` job run `scripts/statecraft/gate.sh`, and the
   declared authored-content script, as they exist at the base commit, exactly
   as `ci-gate.sh` is read today. The adoption, where the base carries none,
   runs the candidate's copy and says so. A change to these scripts takes
   effect for the pull requests after it.
2. **An authority change blocks unless the owner approves it.** When the
   candidate changes any file of the authority set (the rendered workflows,
   `scripts/statecraft/*`, the policy, the declared authored-content script),
   `ci-gate` blocks unless that run's `statecraft-review-exception` job
   succeeded, the owner's approval on the protected Environment. Today this
   is only reported. This also covers an edited workflow file, which rule 1
   cannot: GitHub runs the candidate's workflow on `pull_request`, but
   `ci-gate.sh` and its policy are the base's, and they refuse.
3. **Consequence.** Every re-render of the profile in an adopting repository,
   including this repository's own, needs the owner's approval once. That is
   intended: a change to the gate is reserved to the owner (AGENTS.md, "Owner
   delegation").
4. **Upgrade.** The revision becomes 5, a new identity. Revision 4 projects
   upgrade through one pull request that the owner approves under rule 2.

Acceptance obligations, as tests in `crates/statecraft-home/tests/`: a candidate that weakens `gate.sh` is judged
by the base's copy and fails; the adoption runs the candidate's copy and says
so; a candidate that changes any authority-set file blocks without the
exception and passes with it; a candidate that changes no such file is
unaffected; a revision-4 project upgrades.

**2026-09-24: profile revision 5 implemented (the entry above on a candidate
never judging itself with its own gate).** `setup.rs` registers revision 5
(identity `38935d7738639190dc0f5830e00ebe83ac4fe1bad5a63046326d709ecea5185e`).
Rule 1: the `governance` and `code` jobs check out the whole history and begin
with a step that reads `scripts/statecraft/gate.sh` and
`install-spec-spine.sh` at the base (the pull request's base, the queue's
`base_sha`, or the push's `before`) into the runner's temporary directory;
every later step runs those copies. `install-spec-spine.sh` is read there too
because it chooses the binary the gate runs (an agent's choice, recorded
here). A base commit that cannot be read stops the job; a base that carries no
copy is the adoption, and the step says the candidate's copy runs. `gate.sh`
reads the declared authored-content script at `BASE_SHA`, refuses one that is
not executable there, falls back to the candidate's copy only when the base
carries none and says so; a local run names no base and runs the working
tree's copy. The commit walk judges every commit with the running `gate.sh`,
the base's in CI; revision 4's per-commit `gate.sh` is gone, and each commit
is still judged with the spec-spine release its own `spec-spine.toml` pins.
Rule 2: `ci-gate.sh` computes the authority set from the base's policy (every
path in `files`, the policy, any path under `scripts/statecraft/`, and
`parameters.authored_content`) and compares `BASE...HEAD`. On `pull_request`
a change blocks unless that run's `review-exception` job succeeded; on
`merge_group` it blocks unless the run recorded for the entry's pull request
has a successful exception; on `push` it is reported, having been approved on
its pull request. The adoption, whose base carries no policy, is reported and
not blocked, because the candidate's own `ci-gate.sh` judges it (an agent's
choice, recorded here). The `governance` job gains an `Authority change` step
and an `authority_change` output, and `review-exception` needs `governance`
and also runs when that output is `true`; `ci-gate.sh` never reads it. The
step asks for the exception only when the base's policy carries the new static
`authority_rule` field, so on the upgrade from revision 4 the exception job
does not run: revision 4's `ci-gate.sh` would refuse it as an inapplicable job
that ran. That upgrade pull request is therefore judged by the base's
revision-4 `ci-gate`, which reports the authority change and does not block
it, and the owner's approval of it is procedural; the operator steps say so,
and that every re-render or authority-set change after it needs the owner's
approval once. `ci-gate`'s jobs and rules are unchanged. Tests, each observed
failing against the previous scripts (or the previous `setup.rs` for the
operator steps): in `setup_workflows.rs`,
`a_candidate_that_weakens_the_gate_is_judged_by_the_base_copy_and_fails`
(the governance step, the title, the commit walk and the code job),
`the_adoption_runs_the_candidates_gate_and_says_so` (and an unreadable base
refuses), `an_authority_change_blocks_without_the_owner_exception_and_passes_with_it`
(seven authority-set paths, on a pull request, in the queue and on push),
`a_candidate_that_changes_no_authority_file_is_unaffected` and
`revision_five_reads_the_gate_at_the_base_in_every_job_that_runs_it`; the
mutation test covers the two new blocking branches. In `setup_upgrade.rs`,
`a_revision_four_project_upgrades_to_revision_five` and
`revision_five_names_the_owner_approval_in_its_operator_steps`; revision 4 is
rebuilt from revision 5 and the older rebuilds from it. Existing tests changed
to the ratified behaviour: the declared-script test now makes a script absent
or not executable at the base (a candidate that only deletes it is judged by
the base's copy), and the code-job and pin-step tests run after the step that
reads the gate. Not closed by revision 5 as written: a workflow file the
profile does not render is not in the authority set, and on `pull_request`
GitHub runs a candidate's new workflow, which could report a check named
`ci-gate` from GitHub Actions; this is recorded for the owner, not decided
here. The real `GITHUB_ENV` hand-off between steps is simulated by the test
harness; the first live run is its evidence.

**2026-09-24: revision 5's review corrections (the first live AI review of
#143).** (1) In the merge queue an authority change whose recorded review
could not be read was already blocked, by `recorded_review`'s own refusal, but
the rule 2 branch said nothing. A second, explicit block there was tried and
refused by the mutation test as dead code, which confirms the first; the
branch now says in a comment where the block happens, and a queue case with
an unreadable pull request proves the authority change is refused. (2) The
upgrade tests simulated revision 4 by patching revision 5's marks out of its
templates, which left revision 5's new steps in place. They now use the three
templates revision 5 changed exactly as revision 4 shipped them (main at
`2c82d9a`), kept in `crates/statecraft-home/tests/support/profile-r4/`.

**2026-09-25: profile revision 6, every workflow file is in the authority
set (decided by the owner on 2026-09-25, option (a); amends revision 5, rule
2).** Revision 5's authority set named the profile's rendered files, the policy
and the declared authored-content script. A workflow file the profile does not
render was outside it, and on `pull_request` GitHub runs a candidate's new
workflow, which could report a check named `ci-gate` from GitHub Actions and
satisfy branch protection. Revision 6 adds every path under
`.github/workflows/` to the authority set, in `ci-gate.sh` (read at the base)
and in the `governance` job's `authority_change` output that decides whether
the exception job runs. Adding, changing or removing any workflow therefore
needs the owner's approval once, like any other change to the gate. The
revision becomes 6, a new identity; everything else in revision 5 is
unchanged. Acceptance obligations, as tests: a candidate that adds a workflow
the profile does not render blocks without the owner exception and passes
with it; a candidate that changes no workflow and no other authority file is
unaffected; a revision-5 project upgrades to revision 6.

**2026-09-25: this repository's CI upgraded from revision 4 to revision 6
(S-5).** One re-render with the recorded parameters unchanged; six managed
files and the policy are replaced. The base's revision-4 `ci-gate` reports
this authority change without blocking it, so the owner approves the run
before it merges, as revision 5's implementation entry says. From the next
pull request on, a change to any authority-set file here blocks without that
approval.

**2026-09-25: the declared `rust-version` is 1.88, the measured floor, and
the let chains it permits are applied (owner, 2026-09-25, "check this
repository's declared rust-version and fix it with evidence").** The root
`Cargo.toml` declared 1.85, and it never built: under 1.87.0,
`cargo check --workspace --all-targets --locked --keep-going` fails in two
dependencies, `spec-spine-core` 0.25.0 (18 errors) and `attest-ledger-core`
(1 error), each `E0658` on a let chain, which stabilised in 1.88; both
dependencies declare 1.85 themselves. Under 1.88.0, 1.89.0 and 1.90.0 the
same check passes, and `cargo +1.88.0 build --workspace --all-targets
--locked` finishes. The pinned toolchain in `rust-toolchain.toml` (1.96.0) is
unchanged and is still not the floor. Raising the floor makes clippy's
`collapsible_if` suggest let chains, so the nested `if` blocks it names across
`statecraft-adapter`, `statecraft-cli`, `statecraft-envelope`,
`statecraft-environment`, `statecraft-home` and `statecraft-run` were
collapsed by `cargo clippy --fix` and `cargo fmt`, with no other edit; the one
comment explaining why a let chain was avoided (`plan.rs`) is removed with the
reason it gave. Nothing checks the floor in CI, which is how 1.85 survived;
adding such a check is a change to the check suite and is left to the owner.
spec-spine 0.26.0 declares 1.90, so adopting it raises this floor again.

**2026-09-25: profile revision 7, one exit contract for every rendered
script, and declared extra required jobs (owner, 2026-09-25).** Two owner
decisions, one revision; everything else in revision 6 is unchanged.

1. **The family exit contract.** Every script the profile renders, and every
   `run:` step of its workflows, exits in the vocabulary spec `006` section 3.3
   fixes for this product: 0 ok, 1 a finding, 2 refused (a precondition the
   operator supplies was not met, and nothing was judged), 3 usage, 4 failed
   (an operation that was attempted broke). The three known defects are
   corrected: a `gate.sh` usage error exits 3 (was 64); a missing
   `.tooling/bin/spec-spine` exits 2 (was 3), because an absent binary is a
   prerequisite the operator installs, which is how section 3.23's translation
   table already reads it; an absent or non-executable declared
   authored-content script exits 2 (was 1), the same reasoning.

   Every exit was inventoried, including the implicit paths. What changed:

   | Script | Condition | Was | Now |
   |---|---|---|---|
   | `gate.sh` | usage, unknown mode | 64 | 3 |
   | `gate.sh` | an input unset (`${X:?}`, the shell's own 1 or 2) | 1 or 2 | 3 |
   | `gate.sh` | spec-spine not installed | 3 | 2 |
   | `gate.sh` | declared authored-content script absent or not executable (here or at the base) | 1 | 2 |
   | `gate.sh` | base commit unreadable | 1 | 2 |
   | `gate.sh` | queue ref names no pull request; `text` given another event | 1 | 3 |
   | `gate.sh` | a spec-spine verb (passed through) | its own | the table below |
   | `gate.sh` | the authored-content script (passed through) | its own | 1 on any non-zero |
   | `gate.sh` | a cargo verb failed (passed through, 101 for tests) | cargo's | 1 |
   | `gate.sh` | no cargo on `PATH` | 1 | 2 |
   | `gate.sh` | `cargo metadata` failed | 1 | 4 |
   | `gate.sh`, `install-spec-spine.sh` | any other command `set -e` stopped on (git, gh, mktemp, `cargo install`) | its own | 4 |
   | `install-spec-spine.sh` | no cargo; install failed; the installed binary does not answer | 127, 101, its own | 2, 4, 4 |
   | `ci-gate.sh` | no policy; the policy states no rule for the event (`stop`) | 1 | 2 |
   | `ci-gate.sh` | an input unset | 1 | 3 |
   | `ci-gate.sh`, `ai-review.sh` | any command `set -e` stopped on | its own | 4 |
   | `ai-review.sh` | credential unset; the provider refused it; the head moved | 1 | 2 |
   | `ai-review.sh` | the review was attempted and produced no review of this subject | 1 | 4 |
   | workflow "Read the gate at the base" | base commit unreadable | 1 | 2 |

   Unchanged: `ci-gate.sh` blocks with 1, a finding, as do a base that is not
   the default branch, a refused commit in the walk and a violation the
   authored-content script finds; `pin` and an absent or ranged pin in the
   installer already refused with 2.

   **Child codes are translated, never passed through.** Each deliberate
   non-zero exit is a literal. The sh scripts carry an EXIT trap that reports
   any other ending as 4, and every deliberate exit clears it first (`leave`);
   the bash scripts run under `set -eEuo pipefail` with an ERR trap that exits
   4. So a command that broke is 4 whatever its own code was, and no child's
   code leaves a script untranslated. spec-spine's codes are translated by one
   table in `gate.sh` (`spec_spine`), under the 0.25.0 pin: 0 is 0; 1 (a
   validation failure, or coupling drift) is 1; 2 (stale) is 1, a finding; 3
   (I/O, parse, schema, config or usage, which includes a pin the binary does
   not satisfy) is 4, as section 3.23's table reads it; any other code is 4.
   spec-spine 0.26.0 moves stale to 1 and a pin mismatch to 2 and adds 4
   failed; its adoption rewrites those rows of that one table and nothing
   else. cargo's codes do not separate a failing test from a failing tool (101
   is both), so a cargo verb that ran and failed is a finding; the declared
   authored-content script is the project's, with its own codes, so any
   non-zero answer from it is a finding.

   Two latent defects the inventory found are closed with it. `gate.sh
   couple-group` piped `git diff` and `gh api` into `sort` in a shell without
   `pipefail`, so a failed read was an empty path list, and an empty group
   list honoured the pull request's waiver; each read is now its own command.
   `ci-gate.sh` read its required jobs through a process substitution, whose
   failure is not seen, and an empty set passes; the set is now read into a
   file first.

   **The hooks keep the hook protocol, and the conflict is recorded.** Claude
   Code reads a hook's exit 2 as a block and any other non-zero as a
   non-blocking error. The family contract would give an enforcing gate's
   finding (a stale tree, a coupling failure) exit 1, which Claude Code does
   not block on, so the operation the gate stands in front of would run: that
   breaks section 3.23's contract 6. The protocol wins. The four shipped hooks
   exit only 0 or 2, both codes the family also names, and are unchanged: the
   pre-bash gates block with 2 on every refusal and every finding, and the
   advisory hooks (session start, post-edit, stop) report every answer with 0,
   which is the Stop policy. None runs under `set -e` and each ends on `true`,
   so none can end on a command's own code.

2. **Declared extra required jobs.** An adopter can have checks beyond the
   profile's that must stay required. Rahi, measured read-only on 2026-09-25:
   its `ci.yml` holds its `ci-gate` with `govern` (a reusable workflow call),
   `cargo` and `deny` (cargo-deny); `live.yml`, `release.yml` and `image.yml`
   are separate workflows it keeps outside its gate by its own decisions.
   The rendered `statecraft-ci.yml` is managed, so a project cannot add a job
   to it by hand, and `ci-gate` cannot `needs:` a job in another workflow file.
   The parameter is therefore `ci.extra_required_jobs`, a list of
   `{"job": <id>, "workflow": ".github/workflows/<file>.yml"}`: each entry is
   rendered as a job of `statecraft-ci.yml` that calls the project's own
   reusable workflow (`on: workflow_call`), `ci-gate` needs it, and the policy
   requires it on every event with the `required` rule, so failed, cancelled,
   skipped and vanished all block exactly as for the profile's own jobs. A job
   id is GitHub's shape, unique, and none of the profile's own
   (`governance`, `code`, `ai-review`, `review-exception`, `ci-gate`); a
   workflow is a `.yml` or `.yaml` file directly under `.github/workflows/`
   (GitHub calls nothing deeper), not one the profile renders, and must exist
   when the plan is made. No secret is passed and the workflow's default
   permissions (`contents: read`) apply; a job that needs more is a later
   revision's parameter. The called workflow is under `.github/workflows/`,
   so revision 6 already puts it in the authority set. What `ci-gate` judges
   is the calling job's result as GitHub reports it: a condition the project
   writes inside its called workflow is the project's, and a check that must
   stay required should not skip itself there. With nothing declared,
   `ci-gate` needs exactly revision 6's four jobs. For rahi,
   `deny` fits as declared (moved into a reusable workflow); its `cargo` job is
   what the profile's `code` job already runs. Adopting it there is rahi's
   change and the owner's decision.

3. **The exact-pin default for new projects is not in this revision.** It
   waits for spec-spine 0.26.0's
   `scaffold_init_opts_json(cfg, {"pinExactVersion": true})`, which is the
   governance producer's scaffold (`producer.rs`), not a profile byte. A
   profile revision is content-addressed, and leaving 7 open to change later
   would give one revision two identities, so revision 7 is closed at this
   identity. The pin default lands with the 0.26.0 adoption as a producer
   change; if it turns out to need a profile byte, that is revision 8.

Acceptance obligations, as tests. `setup_workflows.rs`:
`every_exit_a_rendered_script_states_is_in_the_family_contract` parses every
rendered script and workflow step for exit statements, requires each code in
0 to 4, and requires the trap on each script and no `${X:?}` check;
`the_rendered_gate_exits_in_the_family_contract` runs the rendered `gate.sh`
for the three defects (3, 2, 2), every spec-spine translation row, cargo's
three cases and a broken command (4);
`the_rendered_installer_exits_in_the_family_contract`;
`a_declared_extra_job_is_required_like_the_profiles_own` (rendering, policy,
and failed, cancelled, skipped and vanished blocking);
`inverting_a_required_job_branch_is_noticed_by_the_declared_job_cases`, the
mutation obligation for declared jobs; the existing ci-gate and AI-review
cases carry the new codes. `harness_hooks.rs`:
`every_hook_exit_is_zero_or_the_protocols_block`. `setup_upgrade.rs`:
`a_revision_six_project_upgrades_to_revision_seven`, from revision 6's
`gate.sh` as shipped (#144), kept in
`crates/statecraft-home/tests/support/profile-r6/`. `setup.rs`:
`extra_required_jobs_are_validated`. The upgrade is one re-render and an
authority change, so it waits for the owner's approval once.

**2026-09-25: one environment variable selects the spec-spine binary (owner
Addendum 2, item N; amends section 3.23 contract 2 rules 2 and 5). Proposed
2026-09-24; adopted by the owner, 2026-09-25.** The owner's words: "#119 N
(one spec-spine selection variable): adopted; land it now." This change
implements it; the implementation choices follow the adopted text.

*Today three names do one job.* Measured at main `17dbdb6`:

| Variable | Read by | Meaning today |
|---|---|---|
| `SPEC_SPINE_BIN` | the four delivered hooks (`crates/statecraft-home/harness/hooks/statecraft-{session-start,pre-bash,post-edit,stop}.sh`), 16 lines each; `harness_hooks.rs`; this section's contract 2 rule 2 | operator override, unmanaged use; the only candidate when set |
| `STATECRAFT_SPEC_SPINE` | the same four hooks; contract 2 rule 5; the bundle entry (P2.1, H-13) | the supervisor's resolved executable in a managed session; no Rust source sets it yet |
| `STATECRAFT_PRODUCER_BIN` | `crates/statecraft-run/tests/producer_candidate.rs` only (three `#[ignore]` tests, with `STATECRAFT_PRODUCER_REV` and `STATECRAFT_PRODUCER_FIXTURES`) | the exact producer build an operator-run candidate test judges |

This repository's `Makefile` variable `SPEC_SPINE` is a fourth spelling, but
it is the check surface's own, unclaimed, and not read by the product; aligning
it is a separate authority change and not proposed here.

*Adopted: `STATECRAFT_SPEC_SPINE` is the one variable*, namespaced as this
product's, with this precedence:

1. **In a managed session** (the launch named a run in `STATECRAFT_RUN_ID`),
   the supervisor always sets `STATECRAFT_SPEC_SPINE` in the constructed
   environment, overwriting any inherited value, so an operator's shell value
   cannot reach a managed hook. It is the only candidate (rule 5 unchanged in
   substance).
2. **Outside a managed session**, a non-empty `STATECRAFT_SPEC_SPINE` is the
   operator override of rule 2: the only candidate, no fallback, named with its
   path, version, the pin and the two remedies.
3. **Otherwise** the convention candidates of rule 3, then rule 4 for an
   unpinned repository, unchanged.
4. **The retired names are reported, never read.** A hook that finds
   `SPEC_SPINE_BIN` set and `STATECRAFT_SPEC_SPINE` unset says the old name
   was ignored and names the new one; it never selects by it. The candidate
   tests read `STATECRAFT_SPEC_SPINE` (the revision and fixtures variables
   stay, since they name different things).

*Cost, stated.* An operator or script that sets `SPEC_SPINE_BIN` for
Statecraft's hooks loses the override silently in behavior, loudly in output
(rule 4). Rahi's own copied hooks read `SPEC_SPINE_BIN` and are untouched:
this changes only what Statecraft delivers, and the migration order (Statecraft
delivers, the adopter confirms loop and hooks live, then copies are removed)
is unchanged. The one-variable rule is what lets rule 1's overwrite be the
whole isolation argument, instead of two variables whose precedence a reader
must remember.

*Acceptance at implementation.* `harness_hooks.rs`: with only
`SPEC_SPINE_BIN` set, no hook selects it and each names it as ignored; with
`STATECRAFT_SPEC_SPINE` set outside a managed session, the override rules of
contract 2 rule 2 hold unchanged; in a managed session an inherited value is
replaced by the supervisor's. `producer_candidate.rs` reads the new name.

*Implemented.* The four hooks carry the new resolver, the same block in each.
`harness_hooks.rs` renames the override in the existing contract 2 tests and
adds three: the retired name alone is never invoked and is reported as
ignored; with both names set outside a managed session the new one judges as
`override` and the old one is not mentioned; and the same incompatible binary
is refused as an operator's override but used as the supervisor's inside a
managed session. The selection rule is also a library function,
`crates/statecraft-home/src/spec_spine.rs`, with unit tests, so a verb this
product runs and a hook it delivers choose from the same inputs by the same
rule. `run_startup.rs` asserts the value a managed session receives and that
an operator's value under either name neither reaches it nor is invoked;
`provenance.rs` covers initialization. Choices the entry left open, recorded
here:

- **The supervisor's selection is rule 3, then rule 4, and nothing else.** It
  reads neither name from the operator's environment and searches the `PATH`
  the child is given, so rule 1's overwrite holds by construction: the
  constructed environment never carried an inherited value, and both names
  are removed from the binding before the supervisor's value is added, or
  before nothing is added when no candidate exists. A
  digest-verified identity is still the bundle proposal's; nothing here
  verifies one, and the hooks read the supervisor's path as rule 5 says.
- **Candidates that exist with none the project admits refuse the attempt**
  before its intent is written, under the guard `spec-spine-selection`,
  naming each candidate passed over. **No candidate at all sets no value**, so
  the hooks keep the absent-binary behavior rule 3 leaves unchanged; "always
  sets" is read as always whenever there is a binary to hand.
- **Initialization follows the same rule.** `init plan` and `init apply`
  selected nothing: they ran a bare `spec-spine` from `PATH` for the corpus
  step and the qualification probe (F2's first two rows). They now select by
  rules 1 to 4 for the project being initialized; a selection that finds
  candidates and admits none makes the corpus step refused and the
  qualification unavailable, with a new unavailability, `not-selected`,
  translated as a refusal like an absent binary. An initialization under a
  pin the `PATH` binary does not satisfy is therefore `partial`, exit 1, where
  it was `complete`. The other rows of F2 are unchanged and remain the bundle
  proposal's.
- **`observedSpecSpine` names the rule.** Its `program` is the selected path
  and its `foundBy` is the rule's word (`supervisor`, `override`,
  `repository-build`, `path`), where the 2026-09-24 provenance entry said
  `path` for a bare name. A caller that names a program directly keeps that
  entry's words.
- **Initialization does not withhold its compile in an unpinned project.**
  Rule 4's withheld compile is contract 1's exception, which is a hook's;
  initialization's compile is its own step 6, and a new project is unpinned by
  the producer's scaffold, so applying it there would withhold every first
  compile.
- **The notice is one line**, "ignored SPEC_SPINE_BIN=<value>: that name is
  retired and selects nothing; set STATECRAFT_SPEC_SPINE to choose the
  binary", printed with the lines naming candidates passed over, which for the
  pull-request gate is standard error. A newline or carriage return in the
  value is printed as a space, so the value cannot add a line of its own.
- **A version is read only from a `--version` call that exits 0**, in the
  hooks and in the library alike; a binary whose version call fails reports
  no version, which an exact pin reads as "not performed". The hooks read the
  first line whatever the exit status before this entry.
- **A managed session without the supervisor's path** keeps rule 5's
  fallback to rules 1 to 4 and its "version-checked, identity not verified"
  report; a value outside a managed session is an override and is put to the
  pin.
- Entries above that name `$SPEC_SPINE_BIN` record what was true when they
  were written and are not rewritten.

**2026-09-25: revision 7's `gate.sh` reads spec-spine's exit table by the
pinned release (owner, 2026-09-25: adopters render once, at revision 7, with
`=0.26.0`).** Revision 7 as merged translated spec-spine's codes with one
table, 0.25.0's. Measured the same day against the published 0.26.0 in a
disposable clone of `main`: stale moves from 2 to 1, a pin not met from 3 to
2, and an invalid corpus stays 1, on `check --fail-on-warn`, `lint
--fail-on-warn`, `index check --fail-on-unresolved`, `index coverage
--fail-on-untraced` and `compile --check`; an unknown verb stays 3. Under the
single table a 0.26.0 pin mismatch would have read as a finding, not a
refusal, and adopting 0.26.0 would have needed a second rendering. The table
is now chosen by the release `spec-spine.toml` pins (the binary's `--version`
when no pin is readable): below 0.26.0 the old rows, from 0.26.0 spec-spine's
132 contract, which is this family's, with a usage error from the gate's own
fixed invocation read as the gate failing (4) under both. This stays
revision 7: no repository had merged a revision-7 rendering (this
repository's re-render was still open), so no recorded digest names the
earlier text. `the_rendered_gate_exits_in_the_family_contract` asserts both
tables on the rendered script.

**2026-09-25: spec `006`'s JSON naming convention, this spec's half (adopted by
the owner on 2026-09-25; spec `006` section 5 of that date).** Two changes to
this spec's crates, and what stays.

*Renamed to camelCase*, each a `--json` answer only, written nowhere and read by
no other repository: `transfer::Plan` and `transfer::Standing`
(`manifestDigest`, `recordedWithoutJournal`, `journalDisagreements`,
`recordedDigest`, `matchesRecord`, `declaredBy`), with **`plan_id` kept** by
an explicit rename because this spec and spec `006` name it;
`producer::Conformance` (`outOfContract`); and the struct-variant fields of
`authority::Answer`, `ignore::Refusal`, `settings::Removal` and
`settings::SettingsOutcome` (`rename_all_fields = "camelCase"`).

*Strict within a version.* `home.json` and `tools.json` (`home::Personal`,
`home::Tools`, `home::ToolRecord`) refuse an unknown member under version 1,
naming it, and a file that declares another version is refused by that number
before any member is read, so a file from a newer build never reads as a
malformed one. Neither document has lost a member since it was introduced, so
no file this product wrote is refused. Tests: `home::tests::an_unknown_member_under_the_current_version_is_refused_by_name`,
`a_newer_version_is_refused_by_its_version_not_by_its_new_members` and
`what_this_build_writes_reads_back_strictly`.

*Grandfathered, not renamed:* the environment manifest and transfer journal
(including `project.setup`'s parameters), the setup profile's six result names
(this spec's results table names them in kebab-case), the register's
data-carrying qualification reasons, and the startup and trial records.
*Not made strict:* `projects.json`, `delivery.json` and `modifications.json`
carry no schema version, so a refusal could only name a field; each needs a
version first.

**2026-09-25: this repository's CI upgraded from revision 6 to revision 7
(S-5).** One re-render, with the recorded parameters unchanged and no
`ci.extra_required_jobs` declared, replaces the six managed files and the
policy, including the gate that reads spec-spine's exit table by the pinned
release (the entry above). It changes the authority set, so the base's revision-6 `ci-gate`
blocks it until the owner approves that run's `statecraft-review-exception`.

**2026-09-25: `check` and the pin probe read both of spec-spine's exit
tables (owner, 2026-09-25: adopt 0.26.0).** Section 3.23's contract 4 and
contract 2's pin probe state spec-spine's codes as every release below 0.26.0
spends them. spec-spine 0.26.0 (its spec 132) spends them differently, and a
target repository may pin either line. Measured the same day against the
published 0.25.0 and 0.26.0 on a disposable clone of `main`: stale moves from 2
to 1 with the same report lines (`spec-registry: STALE`, "index is stale"), a
pin not met moves from 3 to 2 and still says "requires spec-spine", and 0.26.0
adds 4 for a read that failed. The code alone cannot say which table answered,
so, as contract 4 already does for its two readings of 2, the producer's words
decide: an exit 1 whose report names a stale tree and nothing that regenerating
would not cure is stale; an exit 2 that names a refusal (`refused:`, or a pin
not met) is a read not performed; every other code keeps its reading. The
requirement is unchanged, and so is the translation into spec `006`'s codes:
stale and invalid are 1, a read not performed is 4. `probe::read_check`,
`names_stale_only`, `names_refusal` and `names_pin_refusal` in
`statecraft-environment` carry it, and `statecraft-home`'s pin probe
(`Pin::admits`) reads a range-pin refusal under 2 or 3 by its words. Tested
against the recorded lines in `both_exit_tables_read_to_the_same_answers` and
`a_range_pin_refusal_is_read_under_both_exit_tables`. The delivered hooks read
the same codes and change in their own pull request, because a hook change is
an authority change (AGENTS.md).

**2026-09-25: this repository adopts spec-spine 0.26.0, and the declared
toolchain floor is 1.90 (owner, 2026-09-25).** The adoption is recorded in
`docs/adoption/spec-spine.md` (`D-06`). Two parts of it reach this spec's
territory. First, the setup fixtures that stand in for the adopted release now
state it: the rendered-gate tests pin `=0.26.0` and read spec-spine's 132
table by default, with the 0.25.0 table as the override. The hook fixture's
unresolved-claim text is recorded as byte-identical under both releases. No
assertion was weakened; each table is still asserted in full. Second,
`spec-spine-core` and `spec-spine-types` 0.26.0 declare `rust-version = "1.90"`.
The workspace's declared floor moves from 1.88 (this section, 2026-09-25) to
1.90, measured: 1.90.0 builds the workspace, and Cargo refuses 1.89.0 naming
those two crates.

**2026-09-25: the delivered hooks read both of spec-spine's exit tables
(owner, 2026-09-25: adopt 0.26.0; a hook change is an authority change, so it
is its own change).** The four hooks of section 3.23 read `check` and the pin
probe by 0.25.0's table. Measured the same day against the published 0.26.0,
three readings were wrong: `stop.sh` reported a stale-only tree (now exit 1)
as INVALID; `pre-bash.sh` refused it as a corpus that "does not validate" and
"is not stale", and refused a pin not met (now exit 2) as stale; and every
hook's pin probe read a range-pin refusal only under exit 3, so a 0.26.0
candidate that does not satisfy the pin was "not performed" instead of passed
over. Each hook now reads the producer's words, as contract 4 already does for
its two readings of 2: a report naming `refused:` or a pin not met is a read
not performed, whatever the code; an exit 1 whose report names only a stale
tree is stale; 4 is a read that failed. Contracts 1 to 7 are unchanged, and
every non-zero code still refuses at the enforcing gate (contract 6).
`contract_4_the_0_26_0_table_is_read_as_itself` and
`pin_a_refusal_under_the_0_26_0_table_is_decided_by_the_probe` run the shipped
bodies against the recorded 0.26.0 lines; the 0.25.0 rows are unchanged.

**2026-09-25: profile revision 8, the AI review prefers an API key (owner,
2026-09-25).** The owner decided that the review bills to Anthropic Console
credits while that is wanted, and to the subscription otherwise, with no code
change between the two. `github-actions-rust` revision 8: the reusable review
workflow declares `ANTHROPIC_API_KEY` beside `CLAUDE_CODE_OAUTH_TOKEN`, both
optional `workflow_call` secrets, and binds both in the Review step's
environment; `statecraft-ci.yml` passes both by name and never
`secrets: inherit`. `ai-review.sh` chooses once, after the visible skips: a
non-empty `ANTHROPIC_API_KEY` is used and `CLAUDE_CODE_OAUTH_TOKEN` is unset;
otherwise a non-empty `CLAUDE_CODE_OAUTH_TOKEN` is used and `ANTHROPIC_API_KEY`
is unset; otherwise the script refuses with 2, naming both secrets and the
`gh secret set` command for each. `ANTHROPIC_AUTH_TOKEN`, which no workflow
binds, is unset as well, and the reviewer still runs from an empty directory
with a temporary `HOME`, so no stored login or settings file supplies another
credential. The class chosen (`api-key` or `oauth`, never the value) is in the
job log and in the evidence record as `tool.credential` (`none` on a visible
skip); ci-gate reads the record's subject and result only, so the field is
additive. **The choice is made by which secrets a repository can see.** Both
are organization secrets on `statecrafting` with selected visibility, so the
owner decides per repository, and removing `ANTHROPIC_API_KEY` from a
repository's visibility restores subscription billing. A repository-level
secret of the same name takes precedence over the organization's. The pull
request that introduces revision 8 into a repository is reviewed by the
script at its base, which is revision 7 and uses the OAuth token; revision 8
takes effect from the next pull request. Not changed here: `doctor --remote`
still asks for a repository-level `CLAUDE_CODE_OAUTH_TOKEN`
(`repos/{slug}/actions/secrets/...`), which does not see an organization
secret; that is a separate change. Tested in
`ai_review_prefers_the_api_key_and_falls_back_to_the_oauth_token` (all four
cases, with a stub reviewer that records which variable reached it and that
the others are absent), the caller and declaration assertions of
`contributor_text_is_never_spliced_into_a_run_scalar`, and
`a_revision_seven_project_upgrades_to_revision_eight`, against revision 7's
templates as shipped (`tests/support/profile-r7/`).

**2026-09-25: `config error` is a refusal, and `validation failed` is never
stale (owner, 2026-09-25: adopt 0.27.0).** Measured the same day against the
published 0.26.0 and 0.27.0 on clones of this repository. Invalid
configuration exits 2 under both and says `spec-spine: config error:`, not
`refused:`; the "both exit tables" entry above read only `refused:` and a pin
not met as a refusal, so `probe::read_check` read a malformed `spec-spine.toml`
under 0.26.0 as a stale tree and sent the operator to regenerate. 0.27.0 adds
two more exit-2 answers (its spec 144): a link leaving the repository says
`refused:`, and a layout root that is not a plain relative path (`C:specs`,
`out/nul`) says `config error:`. `names_refusal` now also recognises
`spec-spine: config error:`, which below 0.26.0 was spent on exit 3, a read not
performed either way. 0.27.0's guarded readers (`couple`, `index coverage`,
`index owner`, `scope`, `delta`) report an unresolved claim at
`implementation: complete` as `validation failed` at exit 1 (its spec 145),
where 0.26.0 said "index is stale"; `names_stale_only` excludes it by name. No
reader here runs a guarded reader for freshness: `check` is unchanged and still
prints `UNRESOLVED CLAIM`, and `check --json`, whose index half still says
`"fresh": false` for an unresolved claim (spec-spine plans to change that in
0.28.0), is read by nothing in this product. Tested in
`a_configuration_or_containment_refusal_is_never_stale`,
`an_unresolved_claim_at_a_guarded_reader_is_neither_stale_nor_a_refusal`, and
`a_repository_with_a_link_leaving_it_is_a_read_not_performed`, which builds a
real repository with a real link leaving it and answers it with a labelled
stand-in printing 0.27.0's recorded words; the published binary's own answer is
asserted when the pin moves. The delivered hooks are an authority change of
their own and follow in a separate change.

## Verification

`--fail-on-untraced` joined the corpus gate with this spec's first
implementation, which is the condition AGENTS.md recorded for it; it defends
every claimed file rather than reporting a number (13 then, 149 on
2026-09-23).

Each line is one command. Every row of section 3.10 is one integration test
named after the row it covers, so a row that stops being covered shows up as a
deleted test rather than as a quietly weakened assertion.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
test -f crates/statecraft-environment/src/manifest.rs
test -f crates/statecraft-environment/tests/negative_cases.rs
spec-spine index check --fail-on-unresolved
test -f crates/statecraft-home/src/lib.rs
cargo test -p statecraft-home --test negative_cases
cargo test -p statecraft-home --test harness_hooks
test -f crates/statecraft-home/src/settings.rs
cargo test -p statecraft-home --test settings_modification
test -f crates/statecraft-home/src/session.rs
cargo test -p statecraft-home --test harness_skills
cargo test -p statecraft-home --test bounded_integration
test -f crates/statecraft-home/src/required.rs
test -f crates/statecraft-home/src/startup.rs
cargo test -p statecraft-home --lib required
cargo test -p statecraft-home --lib startup
cargo test -p statecraft-home --lib admission
cargo test -p statecraft-home --test qualification_admission
cargo build -p statecraft-home --example require-harness
cargo test -p statecraft-adapter-claude-code --test settings_transport
cargo test -p statecraft-home --lib setup
cargo test -p statecraft-home --test setup_workflows
cargo test -p statecraft-home --test setup_upgrade
cargo test -p statecraft-cli --test setup_profile
cargo test -p statecraft-adapter --lib supervisor
cargo test -p statecraft-home --lib capture
cargo test -p statecraft-cli --test qualification_workflow
cargo test -p statecraft-cli --test acceptance_script
cargo test -p statecraft-cli --test native_stream
cargo test -p statecraft-home --lib launch
cargo test -p statecraft-cli --test run_startup
cargo test -p statecraft-home --lib trial
cargo test -p statecraft-cli --test startup_trial
sh -n scripts/acceptance/managed-session.sh
test -f crates/statecraft-environment/src/transfer.rs
cargo test -p statecraft-environment --test transfer
cargo test -p statecraft-cli --test ownership_transfer
cargo test -p statecraft-environment --test replace_per_path
cargo test -p statecraft-cli --test env_replace
cargo test -p statecraft-environment --test bridge_removal
cargo test -p statecraft-cli --test env_remove_bridge
```
