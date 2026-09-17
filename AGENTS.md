# AGENTS.md

The cross-agent authority for this repository, read by Claude Code, Codex CLI and
any other agent through the `AGENTS.md` convention. Edit this file to evolve the
protocol.

This repository holds six crates and one binary, each crate claimed by the spec
whose boundary it is. Do not add code, a crate or a test runner without a spec
that claims it: `crates/**` is claimed by the spec whose boundary the crate is,
and the coverage gate refuses an unclaimed source file.

## Where authority lives

1. `specs/000-bootstrap/spec.md`: what a spec is. Its `unamendable` anchors
   cannot be contradicted.
2. `standards/spec/constitution.md`: the durable principles. I to V are
   spec-spine's; VI to XIII are this product's and were **ratified on
   2026-09-16**, three of them frozen as spec `000` anchors.
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
make tools                                # install the pinned spec-spine locally
.tooling/bin/spec-spine --version         # must satisfy the =0.20.0 pin
make gate                                 # the whole read-only corpus surface
make status                               # version, lifecycle counts, schedulable set
git log --oneline -10
```

Run `spec-spine` through `make`, or as `.tooling/bin/spec-spine`. A bare
`spec-spine` resolves to the shared `~/.cargo/bin` copy, which any project on
this machine replaces, and this repository has been governed by the wrong
version that way more than once. `make` prefers the local binary automatically.

**Exit 2 means stale, and only stale, under the pinned 0.20.0.** A claim that
cannot be resolved exits **1**, the validation code, because it is a corpus that
does not describe its tree rather than a ledger that has fallen behind:
re-indexing cannot cure it, and `refresh` against it produces shards that say the
same thing. spec-spine's specs 098 and 101 separate the two readings, and 0.20.0
carries both (`C-16`). Under the previous 0.18.0 pin both conditions exited 2 and
the message was the only way to tell them apart.

Read the message anyway. The codes are now distinct, so exit 1 from `check` sends
you to the spec and exit 2 sends you to `make refresh`, but neither code says
which spec or which shard.

A genuinely stale tree is reported and then fixed as committed work. Do **not**
substitute a writing `compile` or `index` for a check: a read that repairs the
tree hides the fact that the *committed* copy was stale, so the drift then reads
as a local edit rather than as a defect on the branch.

**`registry plan` offers a `draft` spec as ready, and that is not permission to
build it.** Verified against 0.18.0 on 2026-09-16, and re-verified against
0.20.0 on 2026-09-17 by forcing one spec to `draft` plus `pending` in a scratch
worktree: the lifecycle table makes that combination schedulable, so `plan`
names a spec the owner has not agreed to exactly as it names one the owner has.
`plan --json` still carries only `id` and `title`, which is why spec `003`
section 3.1.1 joins it with `registry list --json` to read `status`. What `draft` withholds is
*ratification*, which is why an unratified spec's unresolved units warn instead
of refusing.

`000` to `007` are ratified, so what `plan` offers from that range is a real work
order. Today it offers nothing: all eight are `approved` and `complete`, and
`plan` reports 0 ready, 0 blocked. The next `draft` written here will be offered
as ready anyway. Check the `status` field, not the plan output.

spec-spine does not enforce the difference, so this repository does. Until the
owner ratifies a spec (see Approval semantics), `plan` naming it is a reading
suggestion, not a work order. Do not open an implementation branch for a `draft`.

## The gate

Two surfaces, not one. `make gate` judges the **corpus** and is meaningful with
no code at all. `make code` judges the **workspace**, which is six crates today.
CI runs them as separate jobs and requires both through `ci-gate`, the single
status context branch protection names.

```sh
make gate
spec-spine check --fail-on-warn
spec-spine lint --fail-on-warn
spec-spine index coverage --fail-on-untraced
spec-spine index check --fail-on-unresolved
scripts/check-authored-content.sh

make code
cargo build  --workspace --locked
cargo test   --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt    --all --check
```

CI caches `~/.cargo/registry`, `~/.cargo/git` and `target/` for the `code` job,
keyed on `rust-toolchain.toml` plus `Cargo.lock`: those two are exactly what
invalidates a build. A dependency or toolchain bump misses the cache and is
meant to; an ordinary source edit hits it and recompiles only this workspace's
crates. Nothing in the governance job is cached beyond the spec-spine binary,
because `check` must judge the committed tree and not a restored one.

Each `make code` target is guarded on `crates/*/Cargo.toml` existing, because
every `cargo --workspace` verb refuses a virtual manifest with no members. The
guard is a file test rather than a flag, so the job went live with the first
crate and nobody had to remember to enable it. It no longer fires, and it stays:
the reason it was written, deciding the check surface before the work it judges,
is what Approval semantics requires.

**Both flags the generic kit carries are now in the gate**, and each arrived on
the condition recorded here when it was left out, not on a whim:

- `index coverage --fail-on-untraced` joined with the first source file. On a
  code-free tree it refused an empty universe rather than passing vacuously.
- `index check --fail-on-unresolved` joined when `006` built the last forward
  claim. Until then it would have refused specs that were correctly claiming
  crates they had not written yet.

The second has a cost worth knowing before you pay it: **a new spec that claims
a crate it has not written yet now fails the gate.** That is intended. This
corpus builds what it claims within one pull request, so an unresolved unit
reaching the default branch is a claim nobody wrote. A spec that genuinely needs
to claim ahead of its implementation is the case for removing the flag again,
deliberately, as its own change.

## What a green gate means

The invariant: **the exact integration candidate must pass every required
governance check before it lands.** Two properties are needed for that, and they
are independent. Conflating them is easy and was done here once already.

**Freshness**: the checks ran against a candidate reconciled with the current
`main`. Enforced by branch protection, which requires a branch to be up to date
before merging. How a branch becomes up to date is not prescribed: rebase,
merge, or the forge's own update button all satisfy it, and the one to prefer is
whichever keeps history legible for the change at hand.

**Comparison scope**: the check evaluated *this* candidate's changes and no
others. Enforced by which two commits the check is given, not by freshness.
Re-running a stale check against a moved base makes it fresh and still wrong.

The coupling gate is where scope bites. `spec-spine couple` takes a **three-dot**
diff, so its merge base is derived from the two endpoints it is handed:

- `base.sha...head.sha`, both frozen event SHAs, has this pull request's own fork
  point as its merge base. It evaluates exactly this pull request's changes and
  is stable across re-runs. **This is what CI uses.**
- `base.sha...HEAD`, where `HEAD` is the checked-out `refs/pull/N/merge`, has
  `base.sha` as its merge base and folds in everything merged after the event
  fired. Measured here: 15 changed paths where the pull request changed 1.

The second form is not merely noisy. A waiver is scoped to the diff the gate
evaluated, so a `Spec-Drift-Waiver:` in the body would have covered all 15.

## Before enabling a merge queue

A queue is an optimization for when concurrent work causes repeated
update-and-retest cycles. It must preserve the same guarantees, and today it
would not. Three things it needs first:

1. **Coupling evaluated against the queued integration candidate**, not only
   against the pull request in isolation. The queue's whole value is testing the
   speculative merged tree, and a coupling verdict from PR time does not cover
   it.
2. **Waivers bound to the specific changes and to the authority that approved
   them.** A `merge_group` event carries no pull-request body, which is where a
   waiver lives today. That gap needs a deliberate waiver contract: something
   the queue can read, scoped to the change it was granted for. It is not solved
   by skipping coupling in the queue, and not by refusing every waived change.
3. **`ci-gate` refusing success when a required governance check was skipped.**
   It currently treats a skipped job as a pass, which is correct while the only
   event-gated check is one that cannot apply. Under a queue that rule would let
   an absent coupling verdict read as a green one.

Until those hold, an up-to-date branch is the mechanism, and it is sufficient
for sequential merges.

`spec-spine couple` is **CI-only, and deliberately not in `make gate`**. It
compares two commits, so it cannot see a change being staged and is useless as a
pre-commit check (`C-18`). CI runs it against the pull request's merge base. It
became meaningful with the first crate and judges every `crates/**` change
today. `make couple` exists for reproducing a CI verdict locally, against a
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
   every commit. The codebase index hashes the authored tree, not only the specs:
   editing a root document such as this one or `README.md` turns `check` stale
   with no structural change, and the fix is the same refresh in the same commit.
   The shards do change, and visibly: each one's `shardHash` moves, because that
   hash covers the global inputs. What does not change is what the shard says
   about the corpus.
   **`make couple` before committing proves nothing.** The gate compares two
   commits, so with `HEAD` still at `BASE` it judges an empty diff and reports
   "0 path(s) checked, no drift", which reads exactly like a pass. Commit, then
   couple. The same reasoning applies after a merge: a verdict on merged `main`
   cannot establish that the pull request was correctly coupled, because the
   endpoints it judged are gone. The verdict that counts is the one CI recorded
   against that pull request's own frozen endpoints.
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
