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
may have landed. This build has no verb that reconciles an attempt; the
limitation is stated in that answer rather than worked around, and nothing
here infers an outcome to free the lock.

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

## 4. Out of scope

Installing the product itself; provider authentication; hosted registration;
multi-machine environment sync; and any write to a target outside the manifested
set. Publication, release and distribution are deferred by `F-02`; how this
product is itself packaged is recommendation `D-01`.

Implementing a hosted platform, a platform protocol or a platform client; any
user interface, which `F-04` defers; publication, release and distribution,
which `F-02` defers; multi-machine environment sync; a package registry or a
plugin marketplace; migrating sibling repositories or any real home directory;
adding a provider adapter, which is `008`'s boundary and `F-07`'s deferral; and
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

## Verification

Each line is one command. They run the acceptance this spec's behavior declares:
every row of §3.10 is one integration test, named after the row it covers, so a
row that stops being covered shows up as a deleted test rather than as a
quietly weakened assertion.

`--fail-on-untraced` joins the corpus gate with this change, which is the
condition AGENTS.md recorded for it: coverage is 13/13 specifically claimed, so
the flag now defends that number instead of reporting it.

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
cargo test -p statecraft-adapter --lib supervisor
cargo test -p statecraft-home --lib capture
cargo test -p statecraft-home --lib launch
sh -n scripts/acceptance/managed-session.sh
```
