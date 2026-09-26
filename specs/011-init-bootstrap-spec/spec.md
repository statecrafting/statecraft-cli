---
id: "011-init-bootstrap-spec"
title: "Initialization never scaffolds a second 000 spec and never writes an approved one"
status: approved
implementation: complete
created: "2026-09-25"
summary: >
  Amends 002 section 3.15's reconciliation of the contract path
  <specs_dir>/000-bootstrap/spec.md. When the corpus already has another
  spec holding the ordinal 000, init does not scaffold the bootstrap spec and
  drops a stale managed record of one that is not on disk. When init does
  write the bootstrap spec, it writes it with status draft, because
  ratification is the owner's act and never a tool's. An existing
  000-bootstrap spec is adopted and never rewritten, as before. A defect
  found by the travel-memory program in hiqlite #47 and rahi #90.
amends:
  # Section 3.15: "a contract path that does not exist is written", for the
  # bootstrap spec. 002 is not edited to record it.
  - "002-environment-lifecycle"
extends:
  # The initialization flow and the producer boundary are code in 002's crate
  # (src/flow.rs, src/producer.rs). `amends` does not make this spec an owner
  # of that code (001 section 5, the amendment model, Part 1 item 1), so the
  # edge is declared here, in the same change.
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: corrective }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
---

# 011: Initialization never scaffolds a second 000 spec and never writes an approved one

## 1. Purpose

Spec `002` section 3.15 makes `<specs_dir>/000-bootstrap/spec.md` a contract
path: when it does not exist, `init` writes the producer's bytes and records
the file `managed`. Two defects follow, both seen while five repositories
adopted setup profile revision 7:

1. **A collision.** A corpus with its own `000` spec (hiqlite's
   `000-hiqlite-ownership-bootstrap`, Rahi's `000-rahi-bootstrap`) has no
   `000-bootstrap`, so `init` writes one beside it, and the compiler refuses
   the pair (`V-004`, two specs with one ordinal). hiqlite #47 and rahi #90
   left the file uncommitted, so `.statecraft/environment.json` records a file
   that does not exist and every later `init apply` restores it or reports
   `partial`.
2. **A ratification by tooling.** The producer's bootstrap spec says `status:
   approved`, so a repository initialized by this product starts with an
   approved spec nobody approved. `AGENTS.md` ("Approval semantics") and
   constitution principle VII make ratification the owner's act.

Under the amendment model (spec `001` section 5, adopted 2026-09-25) a change
to what an approved spec requires is a new spec with an `amends` edge, and
this is it.

## 2. Territory

None of its own. The code it changes is `002`'s, reached through the
`extends` edge above: the governance step of the initialization flow in
`crates/statecraft-home/src/flow.rs`, three helpers beside the contract set in
`crates/statecraft-home/src/producer.rs`, and the tests in
`crates/statecraft-home/`.

## 3. Behavior

### 3.1 No second `000` spec

Before the governance step reconciles, `init` looks for **another `000`
spec**: a directory `<specs_dir>/000-*` other than `000-bootstrap` that holds a
`spec.md`. When there is one:

- `<specs_dir>/000-bootstrap/spec.md` is not in the governance declaration:
  it is not written, not adopted and not recorded;
- the report's `kept` list names it, with the other `000` spec's path and the
  reason (`not scaffolded, ... already holds the ordinal 000`);
- nothing about it is withheld, so it never makes the outcome `partial`;
- the other `000` spec is never read beyond its existence and never written.

When there is none, section 3.15 of `002` applies unchanged to the path, with
section 3.2 below for its bytes. An existing `000-bootstrap/spec.md` is still
**adopted** and never rewritten, whatever its status.

### 3.2 A bootstrap spec this product writes is a draft

When `init` writes `<specs_dir>/000-bootstrap/spec.md`, its bytes are the
producer's with one change in the frontmatter: a `status` whose value is not
`draft` becomes `status: draft`, under a two-line comment saying that
ratifying a spec is the owner's act and never a tool's. Nothing else in the
file changes, and a body line that happens to start with `status:` is not
frontmatter and is untouched. The recorded digest is of the bytes written, so
`doctor` compares against them. Ratifying it is the owner's edit, after which
it is a modification the manifest tracks, like any other edit to a file the
producer's own body invites the adopter to customize.

### 3.3 A stale record is dropped

When section 3.1's condition holds, and the declaration records
`<specs_dir>/000-bootstrap/spec.md` as `managed` while the file is absent,
the record is removed in the same apply and the report's `kept` list says so
(`its managed record is removed, the file is absent and ... is this corpus's
000 spec`). A present file is never removed: two `000` specs on disk are the
operator's to resolve, and the compiler already names them.

### 3.4 Observable negative cases

| Case | Required behavior |
|---|---|
| A fresh repository | The bootstrap spec is written with `status: draft`; the outcome is `complete`. |
| A corpus with `000-rahi-bootstrap/spec.md` | No `000-bootstrap` is written or recorded; `kept` names why; the outcome is `complete`. |
| The same corpus, with a `managed` record of an absent `000-bootstrap` | The record is removed and named; the file is not restored; the outcome is `complete`. |
| An existing `000-bootstrap/spec.md` with `status: approved` | Adopted and byte-identical afterwards. |
| An empty `specs/000-x/` directory | Not a spec; the bootstrap spec is scaffolded. |

## 4. Out of scope

- The producer's own bytes. spec-spine's `scaffold_init_json` still returns
  `status: approved`; asking it to return a draft is a request to that corpus,
  not a requirement here. The test in section 5 fails the day it stops, so
  this spec is revisited then.
- Other placeholders in the bootstrap spec (`created: "REPLACE-WITH-DATE"`),
  which `002` section 5 (F3) already records.
- A `specs_dir` other than the one this product declares (`specs`), which the
  producer is always asked for.
- Consumer repositories. hiqlite and Rahi re-render with a build carrying this
  change; that is their change.

## 5. Decisions recorded during implementation

**2026-09-25: "another `000` spec" is read from the directory name.**
spec-spine requires a spec's directory to equal its `id`, and the ordinal is
the id's first three digits, so `<specs_dir>/000-*/spec.md` is the set the
compiler would number `000`. Reading frontmatter would parse a file this
product does not own to learn what its path already says.

**2026-09-25: the notes use `kept`, not a new report field.** `kept` is
"authored inputs on disk, left alone"; the other `000` spec is exactly that,
and a new field would change the `init` answer's shape for one sentence.

**2026-09-25: the tests use the real compiler.** `tests/bootstrap_spec.rs` runs
`init apply` with the pinned spec-spine, so a written draft bootstrap spec and
a corpus holding only its own `000` spec are each judged by the compiler that
governs them, not by a stub. `a_scaffolded_bootstrap_spec_is_a_draft` also
asserts that the linked producer still answers `approved`, so it fails when
section 3.2 stops having anything to do.

**2026-09-25: the number `011`.** `007` is the draft in #165, `008` and `009`
are named by spec `001` section 5's proposal for the split of `002`, and
`010` is the draft in #166.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --lib producer::tests::as_draft_changes_only_the_frontmatter_status
cargo test -p statecraft-home --lib producer::tests::other_zero_spec_names_a_colliding_000_spec_only
cargo test -p statecraft-home --test bootstrap_spec
```
