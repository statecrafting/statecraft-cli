---
id: "129-gate-fence"
title: "The gate fence: acceptance runs in the world the session worked in, not in the daemon's"
status: draft
created: "2026-09-11"
implementation: pending
risk: medium
depends_on:
  - "125-credential-fence"
  - "121-candidate-and-receipt"
  - "016-stage-build"
establishes:
  - "members/src/orchestrator/gate-fence.test.ts"
extends:
  # 016 owns the build runner, whose runGate spawns the gate with no env today.
  - { spec: "016-stage-build", unit: "members/src/orchestrator/stages/build.ts", nature: additive }
  - { spec: "016-stage-build", unit: "members/src/orchestrator/stages/build.test.ts", nature: additive }
  # 021 and 026 own fixture runners and evidence literals that gain the field,
  # as they did for 125's tally.
  - { spec: "021-orchestrator-daemon", unit: "members/src/orchestrator/daemon.test.ts", nature: additive }
  - { spec: "026-standby-daemon", unit: "members/src/orchestrator/standby.test.ts", nature: additive }
  # Doc 05 is the record this spec is born from (D60).
  - { spec: "110-corpus-merge", unit: { kind: directory, path: "docs/design/" }, nature: additive }
references:
  - { unit: { kind: file, path: "docs/design/05-the-realignment-checked.md" }, role: context }
  - { unit: { kind: file, path: "specs/125-credential-fence/spec.md" }, role: context }
summary: >
  Spec 125 fenced the driven session so the broker is the only path that
  works and the journal is complete. The fence stops at the session. The
  build runner's runGate spawns every gate and bracket command with no env,
  so the gate inherits the daemon's whole environment: the model keys, the
  GitHub tokens, the SSH agent, the real gh on PATH and the operator's git
  credential helper. The gate runs the project's own commands in the
  candidate, and the candidate can edit every file those commands execute, so
  a test that publishes during the gate is an effect no receipt covers and no
  journal records. This spec runs the gate with exactly the environment the
  session receives, scrubbed and fenced, tallies a gate-time refusal apart from
  a session's, and records the fence on the build evidence. The broker keeps
  the daemon's environment, as 125 B-6 requires.
---

# 129: The gate fence

## 1. Purpose

125's claim is precise: "the broker is the only path that works, so the
journal is complete." It holds for the process 125 fenced, the provider
session. It does not hold for the process that judges the session's work.

`createProcessRunner` in `stages/build.ts` runs every gate command through
`runProcessSync(workDir, cmd)`, which calls `Bun.spawnSync(cmd, { cwd })` with
no `env`. The child inherits the daemon's environment unchanged. That
environment holds everything 121 B-3 and 125 took away from the session: the
two model keys, `GH_TOKEN` and `GITHUB_TOKEN`, `SSH_AUTH_SOCK`, the real `gh`
first on `PATH`, and a git that reads the operator's credential helper.

The gate is not the engine's own code. It is the project's gate contract
(041), typically `make gate`, `cargo test`, `bun test`, run in the candidate
after the session ends. The session can edit the Makefile, the test files and
the build scripts those commands execute. 121 records such an edit as
`acceptance.sensitive`; it does not refuse it, and it should not have to,
because editing tests is ordinary work.

So a capable session, meeting 125's refusal for `gh pr create`, has one more
obvious move: a test, or a Makefile target, that does what it wanted. No
malice is required for the same effect: a project whose own integration tests
push to a remote does it on every gate. Either way the effect happens with
the operator's credential, outside the broker, with no receipt, no lease and
no journal record. 125 §6 lists three residuals and this is a fourth it does
not name. Draft 126's B-7 wraps only the session as well.

This spec closes the gap the way 125 closed it for the session. It is an
integrity property, not containment, for the same reasons 125 gives.

## 2. Territory

- `members/src/orchestrator/stages/build.ts` (extends 016): `runGate` spawns
  with the session's environment; the gate evidence gains the fence record.
- `members/src/orchestrator/gate-fence.test.ts`: the negative suite, run from
  a real fenced candidate, the shape of 125's `fence.test.ts`.
- `members/src/orchestrator/stages/build.test.ts` (extends 016), and the
  fixture literals in `daemon.test.ts` (021) and `standby.test.ts` (026): the
  evidence field.

## 3. Behavior

### B-1. The gate receives the session's environment

`runGate` spawns each command with `applyFence(scrubEnv(process.env),
fenceDir)`, the same expression `driver.ts` uses for the session, where
`fenceDir` is the fence of the open candidate (125 D-1). The bracket commands
(`spec-spine compile` and `index`, 016 D-8) run through `runGate` and are
fenced with it. The broker is untouched: it publishes from the daemon with the
daemon's environment (125 B-6), and a test asserts a broker push still
succeeds beside a fenced gate.

### B-2. In-place mode keeps its shape

A runner built without a candidate home has no fence (125 D-1). Its gate
receives the scrub without the overlay, which is exactly what its session
already receives. No path runs the gate with more than the session had.

### B-3. A gate-time refusal is tallied apart from the session's

The fence's refusal log is reset per round (125 B-7) and appended to by every
shim. The build reads the tally immediately before and after the gate suite,
and the difference is the gate's. `GateEvidence` gains `fence: { applied:
boolean; refusals: number }`, required with an explicit zero (125 D-3's rule:
a missing tally must not read as "nothing was refused"), and the session's
`fenceRefusals` no longer includes a gate's refusals.

### B-4. A refusal in the gate is a red gate, not a bypass

A shim exits 127, so a gate command that reached for `gh` or `ssh` fails and
the suite is red. No receipt is minted over a red suite (121). The build's
remediation prompt already carries the gate's tail, so the next session sees
the shim's message naming the broker.

### B-5. The receipt is unchanged

`acceptance.receipt` stays at schema version 1. The fence is recorded on the
build evidence beside it, not inside it, so no existing receipt reader changes
and no receipt bytes change meaning. Doc 05 D68 puts the execution context into
a later receipt revision.

## 4. Functional requirements

- **FR-001.** From a real fenced candidate (`gate-fence.test.ts`), a gate
  command that prints its environment shows none of the six `CHILD_ENV_DENY`
  names and shows the fence's `PATH`, `GH_CONFIG_DIR` and `GIT_CONFIG_GLOBAL`.
- **FR-002.** A gate command running `gh` exits 127, the suite is red, and
  `fence.refusals` on the gate evidence is 1 while the session's
  `fenceRefusals` is 0.
- **FR-003.** A gate command pushing to an `ssh` remote fails at the fenced
  `ssh`; the same command pushing to a local bare remote over a `file://` URL
  succeeds, which proves the fence closes credentials and not git.
- **FR-004.** A clean gate over the governance floor passes and a receipt is
  minted, with `fence: { applied: true, refusals: 0 }` on the evidence.
- **FR-005.** In-place mode (no candidate home): the gate environment is the
  scrub, `fence.applied` is false, and every existing build test passes.
- **FR-006.** A broker push with a receipt and a lease succeeds in the same
  world whose gate was fenced (B-1).

## 5. Acceptance

- `bun test` in `members/` is green; `make gate` exits 0; `make members` exits 0.
- A live round on a governed fixture project whose gate is this repository's
  shape (`make gate` plus a language gate): the build passes under the fence
  and mints a receipt. Any gate that needs a credential to pass is found here,
  named in the status section, and resolved by changing the gate, not by
  unfencing it.

## Verification

```sh
cd members && bun test src/orchestrator/gate-fence.test.ts
cd members && bun test src/orchestrator/stages/build.test.ts
cd members && bun test src/orchestrator/fence.test.ts
make gate
```

## 6. Out of scope, and what stays open

- **The verify stage.** `stages/verify.ts` runs a merged spec's declared
  acceptance in a detached worktree of the merged head, with the daemon's
  environment. It runs after publication, on accepted code, and some declared
  acceptance is live by design (a qualification round, a check against a real
  plane). It is a recorded residual, not an oversight; spec-spine's request R7
  asks for it too, and doc 05 §13 defers it to the owner.
- **Everything 125 §6 names.** `HOME` reads, absolute paths and `~/.ssh` stay
  open for the gate as for the session. Draft 126 wraps both.
- **A private dependency fetched over SSH during the gate** fails, as it does
  in the session under 125. A project that needs one fetches over HTTPS or
  vendors it.

## 7. Resolved decisions

D-1 (2026-09-11). The session's expression, not a gate-specific environment.
Two environments would drift, and the property worth having is that the gate
can do nothing the session could not. Reusing `applyFence(scrubEnv(...))`
makes that a fact of the code rather than of a test.

D-2 (2026-09-11). Scrub in in-place mode too. The session is scrubbed in every
mode; leaving the gate unscrubbed where there is no fence would keep a
difference with no reason behind it. The cost is that a fixture world's gate
no longer sees `GH_TOKEN`, which no existing gate reads.

D-3 (2026-09-11). Attribution by difference, not a second log. The shims are
125's and stay a pure function of the fence directory; reading the tally
before and after a sequential suite attributes a refusal exactly without
changing them.

D-4 (2026-09-11). The fence goes on the evidence, not the receipt (B-5). A
receipt field would be a schema change for a fact the evidence already
carries, and receipt revisions are doc 05 D68's to make once.

## Status (2026-09-11)

Authored `draft`, `implementation: pending`, from doc 05 §3 F2. The finding is
a read of `stages/build.ts` (`runProcessSync` at the top of the file and
`runGate` in `createProcessRunner`) against 121 B-3 and 125 B-1, both of which
scope the scrub and the fence to the session. Approval is a human flip.
