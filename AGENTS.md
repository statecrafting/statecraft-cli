# AGENTS.md

The cross-agent authority for this repository, read by Claude Code, Codex CLI and
any other agent through the `AGENTS.md` convention. Edit this file to evolve the
protocol.

This repository is **specification only**. There is no code, no binary and no
test suite. Do not add one without a spec that claims it.

## Where authority lives

1. `specs/000-bootstrap/spec.md`: what a spec is. Its `unamendable` anchors
   cannot be contradicted.
2. `standards/spec/constitution.md`: the durable principles. I to V are
   spec-spine's; VI to XIII are this product's and are **draft**.
3. `standards/spec/contract.md`: the normative summary, including the lifecycle
   table that decides what is schedulable.
4. `specs/NNN-slug/spec.md`: ordinary specs.
5. `docs/decisions/00-founding-decisions.md`: what is intent, what is inherited,
   what is proposed. Nothing in its section 3 is adopted.

`.derived/` is compiler output. Never hand-edit it, and never parse it with
`jq`, `sed` or `awk`: read it through `spec-spine` subcommands, which fail at the
deserializer instead of silently encoding a stale assumption.

## New sessions

Read before working. None of these writes.

```sh
spec-spine --version                      # must satisfy the =0.18.0 pin
spec-spine check                          # both freshness reads; see the exit-2 note below
spec-spine lint
spec-spine registry status-report --nonzero-only
spec-spine registry plan                  # what spec-spine offers as ready
git log --oneline -10
```

**Exit 2 does not only mean stale.** Under the pinned 0.18.0 it also covers a
claim that cannot be resolved, which re-indexing cannot cure: running `refresh`
against it produces identical shards and the check still exits 2. So read the
message, not the code. If `refresh` leaves `check` at 2, the cause is a claim, not
a stale shard, and the fix is in the spec. spec-spine's spec 098 separates the two
readings; it is not in any release (`C-16` in the decision record).

A genuinely stale tree is reported and then fixed as committed work. Do **not**
substitute a writing `compile` or `index` for a check: a read that repairs the
tree hides the fact that the *committed* copy was stale, so the drift then reads
as a local edit rather than as a defect on the branch.

**`registry plan` offers a `draft` spec as ready, and that is not permission to
build it.** Verified against 0.18.0 on 2026-09-16: the lifecycle table makes
`draft` plus `pending` schedulable, so `plan` names a spec the owner has not
agreed to exactly as it names one the owner has. What `draft` withholds is
*ratification*, which is why an unratified spec's unresolved units warn instead
of refusing.

`002-environment-lifecycle` is the one `plan` names ready today, and it is
ratified (`approved` plus `pending`), so it is a real work order. The next spec
`plan` offers, `003`, is `draft` and will not be. Check the `status` field, not
the plan output.

spec-spine does not enforce the difference, so this repository does. Until the
owner ratifies a spec (see Approval semantics), `plan` naming it is a reading
suggestion, not a work order. Do not open an implementation branch for a `draft`.

## The gate

Two surfaces, not one. `make gate` judges the **corpus** and is meaningful with
no code at all. `make code` judges the **workspace** and is inert until a crate
exists. CI runs them as separate jobs and requires both through `ci-gate`, the
single status context branch protection names.

```sh
make gate
spec-spine check --fail-on-warn
spec-spine lint --fail-on-warn
scripts/check-authored-content.sh

make code
cargo build  --workspace --locked
cargo test   --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt    --all --check
```

Each `make code` target is guarded on `crates/*/Cargo.toml` existing, because
every `cargo --workspace` verb refuses a virtual manifest with no members. The
guard is a file test rather than a flag, so the job goes live with the first
crate and nobody has to remember to enable it. Until then it passes having
judged nothing, and says so in the log: that is the price of deciding the check
surface before the work it judges, which is what Approval semantics requires.

Two flags the generic kit uses are **deliberately absent**, and adding them would
refuse this repository's own correct state:

- `index coverage --fail-on-untraced` refuses on a code-free tree rather than
  passing vacuously. It joins the gate with the first source file.
- `index check --fail-on-unresolved` refuses a forward claim. Specs `002` to
  `005` claim crates that do not exist yet, which is what a `draft` or a
  `pending` spec is for. It joins the gate when this repository builds what it
  claims within one pull request.

`spec-spine couple` is **CI-only, and deliberately not in `make gate`**. It
compares two commits, so it cannot see a change being staged and is useless as a
pre-commit check (`C-18`). CI runs it against the pull request's merge base. On a
tree with no code it has nothing to refuse; it becomes meaningful with the first
crate. `make couple` exists for reproducing a CI verdict locally, against a
commit, not for gating a commit you are about to make.

## Working the backlog

One spec per pull request, then stop.

1. **Pick the spec.** `spec-spine registry plan --next` names a candidate.
   Confirm it is `approved` before working it: a `draft` is offered as ready and is
   not ratified (see New sessions). If nothing is ratified, ask; never ratify.
2. **Branch.** A feature branch named after the spec id. Never commit to `main`.
3. **Re-read the design before coding.** If the design is imprecise, record the
   choice in the spec. If it is wrong, stop and report. Never rewrite an approved
   spec to match code you just wrote.
4. **Implement within the territory.** Claim every new file in the spec's
   ownership edges. Touching a unit another spec owns is an `extends` edge naming
   that spec and unit.
5. **Refresh and gate.** `make refresh` after editing any `spec.md`, and commit
   the regenerated shards with the change that made them stale. `make gate` before
   every commit.
6. **Verify.** `make verify SPEC=<id>` runs the spec's declared acceptance. A spec
   with no `## Verification` block declares none, which is honest for an
   unimplemented spec and is not a passing acceptance.

## Source ownership

| Path | Owner | Rule |
|---|---|---|
| `specs/**/spec.md` | authored | The source of truth. One directory per spec, name equal to the frontmatter `id`. |
| `standards/spec/**` | authored | Constitution, contract, templates. The constitution is changed by an `approved` spec claiming the affected heading as a section unit. |
| `docs/decisions/**`, `docs/design/**` | authored, claimed by spec `001` | Editing either is a change to spec `001`'s territory. |
| `scripts/**` | authored, claimed by the spec that adds it | `check-authored-content.sh` is spec `001`'s. |
| `.derived/**` | **compiler** | Regenerated by `make refresh` only. Committed. Never hand-edited. `build-meta.json` is the one gitignored file. |
| `.statecraft/state/**` | runtime | Declared as `state_dir`: ungoverned, gitignored, and never claimed by a spec. |
| `spec-spine.toml` | authored | The version pin and the layout. Changing the pin is its own change with its own re-index. |
| `Cargo.toml`, `rust-toolchain.toml`, `Makefile`, `.github/workflows/**` | authored, claimed by no spec | The check surface and the workspace root it needs. Deliberately unclaimed: they build and judge the whole corpus rather than any one spec's territory, and a spec that owned them would be a spec that owns the rules it is judged by (constitution VII). Changing any of them is an **authority change**, decided by a human on its own. |
| `crates/**` | authored, claimed by the spec whose boundary the crate is | D-02: one crate per spec. A crate's files are claimed by that spec's directory unit, which is how the coupling gate holds a real compile unit per spec. |
| `LICENSE` | preserved | Never edited. Apache-2.0. |

## Approval semantics

- **An agent never ratifies a spec.** Flipping `status: draft` to `approved` is
  the repository owner's act. An agent may draft, amend a draft, and report that a
  spec is ready to read.
- **An agent never writes a `Spec-Drift-Waiver:` line on its own authority.** It
  needs explicit human approval, and it is cited in the pull-request body.
- **An agent never adopts a row of `docs/decisions/00-founding-decisions.md`.** It
  may add a row marked proposed, with its reason.
- **An authority change is separated from implementation.** A change touching
  policy, the check suite, the verifier, hooks, the acceptance instructions or
  the environment manifest is proposed on its own and decided by a human. It is
  never bundled with the work it would authorize.
- **Publication is not authorized by default.** No push to `main`, pull request,
  merge, release or deploy happens without the owner asking for it in the request
  at hand. Deferral `F-02` in the decision record holds until separately lifted.

## Authored-content rules

Checked mechanically by `scripts/check-authored-content.sh`, which is part of
`make gate`. Both rules are spec `001` section 3.6.

1. **No U+2014.** Use a colon, semicolon, comma, parentheses or two sentences.
   U+2013 is permitted only for numeric or section ranges.
2. **No agent-session URL and no session-tracking trailer**, anywhere: files,
   commit messages, pull-request bodies, issues, reviews, release notes. Do not
   substitute another tracking link. No agent attribution line in a commit message
   or a pull-request description.

## Claims

Four grades, stated separately and never inferred from one another: **specified**,
**implemented**, **tested**, **released**. A spec being `approved` grants no grade
above *specified*. Any document here that calls a behavior implemented, tested or
released names the evidence in the same sentence.
