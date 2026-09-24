<!-- Governed by D-06 in docs/decisions/00-founding-decisions.md; claimed by no spec. -->

# Producer adoption ledger: spec-spine

One entry per adopted spec-spine release, newest last. `D-06` in
`docs/decisions/00-founding-decisions.md` is the stable decision this ledger
serves; this file is where each application of it is recorded. Spec `001`
section 3.13 states why the two are separate files.

**The pin itself is not stated here.** `required_version` in
`spec-spine.toml` is the single stated source of the CLI pin, and the linked
library's exact version is `[workspace.dependencies]` in the root
`Cargo.toml`. An entry records what was adopted and why; it never replaces
reading those two files.

## How an entry is written

- **Heading:** `### YYYY-MM-DD: <release>`, and whether it moves the CLI pin,
  the linked core, or both. `D-06` as amended on 2026-09-24 lets one change move
  both when they come from the same release.
- **Identities, recorded separately.** The CLI binary and each library crate
  get their own identity block (tag and target revision, registry checksum,
  `.cargo_vcs_info.json` revision and `dirty` flag, source-tree comparison,
  install digest with toolchain). Moving both in one change never merges their
  identities into one line.
- **The review `D-06` requires:** the bypass floor compared, coupling over
  merged ranges, exit codes, the hook-read report text, the re-index shape.
- **Evidence kinds kept apart:** the producer's evidence, this repository's
  qualification, and what neither qualifies.
- **What the entry does not do,** and the consequence if the adoption is
  rejected.

An adoption change edits this file, `spec-spine.toml`, the root `Cargo.toml`
and `Cargo.lock` when the library moves, and the regenerated shards. None of
those is claimed by a spec, so the change couples without a spec edit or a
waiver. A change that must also alter code or a spec's requirements because the
release changed behavior is not only an adoption, and carries that spec's
authoring edit as usual.

## Constraint history relocated from the decision record

Rows `C-01` and `C-02` of the decision record's section 2 were restated on
2026-09-24 as rules that do not name a release. Their dated history, as it
stood, is kept here verbatim.

| ID | Constraint history (verbatim, as of 2026-09-24) |
|---|---|
| C-01 | **2026-09-24:** the binary is **0.25.0**, at the same repository-local path, adopted under `D-06`'s entry of that date. **2026-09-23:** the binary is **0.23.0**, at the same repository-local path, adopted under `D-06`'s entry of that date; the rest of this row is unchanged. **2026-09-17:** the binary this corpus is compiled, linted and pinned against is **0.20.0**, installed at the repository-local `.tooling/bin/spec-spine`. The shared `~/.cargo/bin/spec-spine` is no longer this repository's binary and is not consulted by `make`. Until 2026-09-16 the row read 0.18.0 at the shared path. |
| C-02 | **2026-09-24:** 0.24.0 and 0.25.0 are released and 0.25.0 is adopted (`D-06`); 0.24.0 never was. **2026-09-23:** 0.21.0 and 0.23.0 are released and 0.23.0 is adopted (`D-06`); 0.21.0 never was, and 0.22.0 was prepared but never tagged or published (crates.io lists 0.21.0 then 0.23.0). The rule below still holds: unreleased producer work is never described here as available. **2026-09-17:** 0.19.0 and 0.20.0 are both released and 0.20.0 is adopted (`D-06`). The development checkout at `~/DevWork/spec-spine` is ahead of both and sits on an unmerged branch; **no feature of that tree may be described here as available**, which is the part of this row that did not change. The rule is about unreleased work, not about 0.19.0 in particular. |

## Entries

The three entries below were written under `D-06` in the decision record and
were relocated here verbatim on 2026-09-24. A relative phrase in them such as
"the 0.23.0 entry above" still refers to the entry above it in this file.

**2026-09-17: the pin moves to `=0.20.0`.** The floor was reviewed before the pin
moved, because this row is the reason to review it.

- `DEFAULT_BYPASS_PREFIXES` is **byte-identical** between `v0.18.0` and
  `v0.20.0`: the same thirteen entries in the same order.
- `spec-spine config show`, which prints the merged and attributed floor the gate
  actually matches on, differs between the two versions only by the pin line
  itself and a new, empty `[coverage] governed_scope` block.
- The one coupling change that widens what the gate asks about is spec 078's
  governed scope, and it is **inert while `governed_scope` is empty**, which it
  is here. The code takes the empty-scope path, which is the 0.18.0 behaviour.
- Spec 092 makes the gate **stricter**, not looser: diff membership is completed
  from `git diff --name-status`, so a mode-only or binary change is judged rather
  than silently dropped. A floor review is about paths escaping judgement; this
  is a path that stops escaping.
- Measured, not only read: `couple` over four merged ranges of this repository
  returns the same verdict and the same checked-path count under both versions.

The re-index the pin change requires is small and worth stating exactly, because
it is the shape a future pin move will take too. Eight spec-registry shards moved
one field, `specVersion` 1.2.0 to 1.3.0, and their `shardHash` did not move at
all: the compiled content is identical and only the schema label advanced.
Fourteen codebase-index shards moved one field, `shardHash`, because
`spec-spine.toml` and the root documents are in the global-inputs hash and this
change edits both. No shard said anything different about the corpus. That
containment is a consequence of reading `.derived/` only through `spec-spine`
subcommands: a schema label a consumer never parses cannot break the consumer.

**2026-09-23: the pin moves to `=0.23.0`.** Owner-directed, as the scoped
adoption of spec-spine's published expansion line. It moves the CLI pin only.
The library dependency `spec-spine-core` moves in its own implementation
change, because `D-02` gives that crate an owning spec and this pin has none.
0.21.0 and 0.22.0 were never adopted here.

*What was adopted, by identity.* `spec-spine-cli`, `spec-spine-core` and
`spec-spine-types` 0.23.0 were published to crates.io at about 08:40Z on
2026-09-23 by the producer's owner account. Each downloaded `.crate` matches
the registry checksum:

| Crate | Checksum |
|---|---|
| `spec-spine-cli` | `6b0e7800…ca91` |
| `spec-spine-core` | `3dca8f68…e492` |
| `spec-spine-types` | `dcd35073…9dc4` |

Each records Git revision `d2bb4763`, the target of tag `v0.23.0`, in
`.cargo_vcs_info.json`, and each unpacked source equals that revision's tree
with no differing path. Between the candidate `97f82ee5` that the producer's
preparation record measured and `d2bb4763`, nothing under `crates/` changed:
the release adds only the ratification of its specs and documentation. A build
is identified by version and source revision, not by digest. The same
`cargo install --locked` gave `599d724d…f23a` under an empty `CARGO_HOME` and
`365d87ab…02c0` under `make tools`.

*The floor.*

- `DEFAULT_BYPASS_PREFIXES` is byte-identical between `v0.20.0` and `v0.23.0`:
  the same declaration, the same thirteen entries.
- `config show` differs only by the pin line and one built-in floor entry,
  `.statecraft/derived/`. From 0.23.0 the configured derived directory joins
  the floor the gate applies, not only the default `.derived/`.
- That is the one place the gate asks about fewer paths. It is compiler output
  that no spec authors, and `check` judges its freshness on every gate run.
  Spec `001` section 3.1 already forbids reading it except through the
  producer.
- The other coupling changes since 0.20.0 are narrower or stricter, not looser:
  - a deleted path is judged where it lived (its spec 100);
  - citations the renumber could not see (098);
  - a waiver's lifecycle lines, inert without them (113).
- Measured, not only read: `couple` over six merged ranges of this repository
  gives `OK` under both versions: `01df484..4d6a9d5`, `4d6a9d5..ea2076b`,
  `ea2076b..f49ac76`, `f49ac76..323ac62`, `1739131..50a5269` and
  `1739131..01df484`. (A first draft of this entry listed the fifth range
  backwards, which is an empty diff and proves nothing; it was re-measured.)

The checked-path counts differ by exactly the derived paths in each range:

| Range | 0.20.0 | 0.23.0 | Derived paths |
|---|---|---|---|
| `01df484..4d6a9d5` | 16 | 1 | 15 |
| `4d6a9d5..ea2076b` | 6 | 4 | 2 |
| `ea2076b..f49ac76` | 14 | 8 | 6 |
| `f49ac76..323ac62` | 10 | 6 | 4 |
| `1739131..50a5269` | 3 | 1 | 2 |
| `1739131..01df484` | 35 | 17 | 18 |

- One behavior is new and matters here. 0.23.0's `couple` refuses with exit 2
  when the working tree's ledger is stale, before judging. That is stricter,
  and it is why a pin move must be committed with its re-index.

*The re-index.* The move regenerates 22 shards:

- Seven spec-registry shards: registry `specVersion` 1.3.0 to 1.6.0, with each
  record gaining `sectionDigests`, spec-spine's 106.
- Fifteen codebase-index shards: `shardHash` only, because the tool version,
  `spec-spine.toml` and the root documents are global inputs.

Read through the CLI, `registry plan --json` is read schema 0.7.0 and each
ready row carries `status`, and `registry closure` resolves: for
`002-environment-lifecycle` alone it is one member, digest `419cc963…8dbd`. No
shard says anything different about the corpus's specs, and the ready set is
unchanged: `002`, `approved`.

*What this does not do.* It does not move `spec-spine-core`, invert the
producer-conformance tests, record new producer fixtures, or change any code.
It adopts no new producer capability as a runtime feature. Work scopes,
impacts, interface references and waiver lifecycles are available and not
consumed, because no requirement here calls for them.

**Consequence if rejected.** The pin returns to `=0.18.0` and `make tools`
installs that instead, since the version is read from the pin. The rows corrected
under C-16 would have to go back to naming those seven specs unreleased, which
would then be false: their release is a fact about spec-spine, not about this
pin.

**2026-09-24: the pin moves to `=0.25.0`.** Owner-directed on 2026-09-24:
qualify the published 0.25.0 carrying spec-spine's 126 to 129, after the
producer's registry-backed consumer check. It moves the CLI pin only;
`spec-spine-core` moves in its own implementation change, as in the 0.23.0
entry above. 0.24.0 was never adopted here: nothing needed a 0.24.0 capability,
and 0.24.0 carried the defect below.

*Why this release.* Measured on 2026-09-24 in a disposable clone: a spec `id`
naming a path outside the repository (`../../../../../outside/victim`) made
`compile` overwrite a file outside the repository under both 0.23.0 and
published 0.24.0, while exiting 1. 126 (a derived file stays in its
directory), 127 (a derived file is where its path says), 128 (a derived tree
stays in its repository) and 129 (a configuration passed as JSON obeys the
loader's rules) are the producer's fixes. The installed 0.25.0 refuses that
case with exit 3, and the outside file's digest is unchanged.

*What was adopted, by identity.* Tag `v0.25.0` is an annotated tag with a good
signature (ED25519, the producer owner's key), and it targets `25d46b9f`,
which is on spec-spine `main` and contains `f6afdc61` (126), `212995fe` (127),
`8446e773` (128) and `34d0d0df` (129). Its release run succeeded. The three
crates were published to crates.io at about 10:42Z on 2026-09-24 by the
producer's owner account, none yanked. Each downloaded `.crate` matches the
registry checksum:

| Crate | Checksum |
|---|---|
| `spec-spine-cli` | `1e7e7eda…8688` |
| `spec-spine-core` | `d96d89fb…3b2c` |
| `spec-spine-types` | `a941756c…5495` |

Each records Git revision `25d46b9f` in `.cargo_vcs_info.json`, with no
`dirty` flag, and each unpacked source equals that revision's
`crates/<crate>` with no differing path (the normalized `Cargo.toml` differs
as Cargo writes it; `Cargo.toml.orig` equals the tree's). `cargo install
--locked` with rustc 1.96.0 gave `35e5cc20…69dd`, and `make tools` gave
`fb29901f…8d14`; as before, a build is identified by version and source
revision, not by digest.

*The floor.* The five source files that name `DEFAULT_BYPASS_PREFIXES` are
byte-identical between `v0.23.0` and `v0.25.0`, and `config show` differs only
by the pin line.

*Coupling.* `couple` gives the same verdict and the same checked-path count
under both versions over fourteen merged ranges: the six in the 0.23.0 entry
above (1, 4, 8, 6, 1 and 17 paths, as recorded) and `6d02de4..bad7136`,
`bad7136..411234c`, `411234c..96e9f2d`, `96e9f2d..fa000c6`,
`fa000c6..fee508a`, `fee508a..2b15987`, `2b15987..fa8229d` and
`fa8229d..b2d80c9`. Four synthetic commits agree as well: a
`docs/decisions`-only change, a `src`-only change and a deleted file each
exit 1 with `C-001` naming the same owner, and a derived-only change exits 0
with no path checked. Against shards a 0.23.0 wrote, 0.25.0 refuses every
range with exit 2, "index is stale": the ledger refusal 0.23.0 already has,
and the reason this move is committed with its re-index.

*Exit codes and the text the hooks read.* `check`, `lint --fail-on-warn`,
`index check --fail-on-unresolved`, `index coverage --fail-on-untraced` and
`compile --check` give the same code under both versions on a fresh, a stale,
an invalid and a mismatched-pin tree and on an unknown verb. The report lines
the shipped hooks match (`spec-registry:` and `codebase-index:`) are
byte-identical, and the pin refusal differs only by its two version numbers,
so section 3.23 contract 2's probe still reads it.

*The re-index.* The move regenerates 22 shards, one line each:

- Seven spec-registry shards: `specVersion` 1.6.0 to 1.8.0, with every
  `shardHash` unchanged.
- Fifteen codebase-index shards: `shardHash` only.

Read through the CLI, `registry plan --json` is read schema 0.8.0 with the
same keys, the ready set is unchanged (`002`, `approved`), and every
`registry closure` digest and member count is unchanged. No shard says
anything different about the corpus.

*Evidence kinds, kept apart.* The producer's candidate testing (local archives
at `e6c5186c`) and its registry-backed consumer check (the five tests of its
0.25.0 handoff, all passing, including `statecraft-home`'s suite against the
published core in a scratch clone) are the producer's evidence. The identity,
floor, coupling, exit, hook-text and re-index measurements above are this
repository's published-package qualification of the CLI. Neither qualifies a
bundle, which is not adopted.

*What this does not do.* It does not move `spec-spine-core` or
`PRODUCER_VERSION`, change any code, or adopt a new producer capability as a
runtime feature.

**Consequence if rejected.** The pin returns to `=0.23.0`, `make tools`
installs that, and the 22 shards are regenerated back. The crafted-id defect
stays in the governing binary.
