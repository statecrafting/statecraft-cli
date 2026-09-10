---
id: "126-sandboxed-executor"
title: "The sandboxed executor: the OS refuses what a shim could only discourage"
status: draft
created: "2026-09-10"
implementation: pending
depends_on:
  - "125-credential-fence"
  - "122-action-broker"
  - "121-candidate-and-receipt"
establishes:
  - "members/src/orchestrator/sandbox.ts"
  - "members/src/orchestrator/sandbox.test.ts"
extends:
  # 043 owns the driver seam; the sandbox wraps argv at the same spawn where
  # 125 overlays the fence on the environment.
  - { spec: "043-driver-seam", unit: "members/src/orchestrator/driver.ts", nature: additive }
  # 121 owns the candidate; the profile is written beside the fence, on open.
  - { spec: "121-candidate-and-receipt", unit: "members/src/orchestrator/candidate.ts", nature: additive }
  # 014 owns the session; evidence gains the denial tally, as 125 gave it
  # `fenceRefusals`.
  - { spec: "014-session-driver", unit: "members/src/orchestrator/session.ts", nature: additive }
  # The three stages that open a candidate and spawn a session.
  - { spec: "016-stage-build", unit: "members/src/orchestrator/stages/build.ts", nature: additive }
  - { spec: "017-stage-ship", unit: "members/src/orchestrator/stages/ship.ts", nature: additive }
  - { spec: "018-stage-shepherd", unit: "members/src/orchestrator/stages/shepherd.ts", nature: additive }
references:
  - { unit: { kind: file, path: "docs/design/04-the-governed-substrate.md" }, role: context }
summary: >
  Spec 125 fenced the credential path a driven session would reach for
  first, and recorded three residuals it could not close: HOME stays
  readable, an absolute path bypasses PATH, and ~/.ssh keys stay on disk.
  All three go around PATH, so nothing built out of shims and environment
  variables reaches them; what refuses a read of a path is the operating
  system. This spec makes the engine apply an OS confinement at the one
  place it already constructs the child's world, the session spawn in
  driver.ts, with a deny-list profile written beside the candidate. It
  turns 125's integrity property into a containment property for the
  reaches 125 enumerated, and says plainly which reaches remain.
---

# 126: The sandboxed executor

## 1. Purpose

Spec 122 made the broker the only *sanctioned* path to publishing. Spec 125
made it the only *ergonomic* one: refusing `gh` and `ssh` shims first on
`PATH`, git redirected off every inherited credential helper. The value of
that boundary is the journal, and the journal is complete only if the broker
is the path that works. 125 bought integrity, and §6 recorded three
residuals rather than hiding them.

The three share a shape the fence could not reach, because a shim is a file
on `PATH` and every residual goes around `PATH`:

- `HOME` stays readable, so `~/.config/gh/hosts.yml` answers a session that
  reads it, and the platform keyring answers one that asks it;
- a binary invoked by absolute path never consults `PATH`, so
  `/opt/homebrew/bin/gh` still runs;
- `~/.ssh` is a directory, not a command, and no shim stands in front of it.

No list of names closes any of these, however long it grows. What refuses a
read of a path is the operating system.

Neither provider can be asked to do it. The Claude driver's permission modes
gate prompts and do not fence the filesystem, which is exactly why it does
not claim `workspace-write` (120 D-2). Codex does confine, but
`--sandbox workspace-write` confines *writes* to the workspace, and every
residual here is a read or an exec. A confinement that only one of the two
providers has, and that covers the wrong direction, is not a boundary the
engine can build on. So the engine applies its own, at the one place it
already constructs the child's world: the spawn in `driver.ts` where 125
overlays the fence.

This spec closes 125 §6's three residuals. It does not claim to close every
reach, and §6 below says which remain.

## 2. Territory

- `members/src/orchestrator/sandbox.ts`: the profile builder, the argv
  wrapper, the platform probe and the denial tally.
- `members/src/orchestrator/sandbox.test.ts`: the suite.
- `members/src/orchestrator/candidate.ts` (extends 121): the profile is
  written on `openCandidate`, beside the fence.
- `members/src/orchestrator/driver.ts` (extends 043): argv is wrapped at the
  session spawn, where the fence is already overlaid on the environment.
- `members/src/orchestrator/session.ts` (extends 014): `SessionEvidence`
  gains `sandboxDenials`, as 125 gave it `fenceRefusals`.
- `members/src/orchestrator/stages/{build,ship,shepherd}.ts` (extends
  016, 017, 018): each carries the count to its result, as it does the
  fence's.

## 3. Behavior

### B-1. The profile sits beside the candidate, never inside it

`buildSandbox(homeDir, project, branch)` writes to
`<home>/sandboxes/<project>/<branch>/profile.sb`, the sibling shape 125 B-2
chose for the fence and for the same reason: a file under the worktree
would dirty the tree that 121 B-4 requires clean before a receipt. The
profile is a pure function of its inputs, so a stale profile from a crashed
run is overwritten rather than trusted.

### B-2. The profile is a deny-list, not an allow-list

The profile opens `(allow default)` and then denies, by name, the credential
material 125 §6 enumerated. It does not enumerate what a session may read.

This is a deliberate weakening of the obvious design, and D-1 records why:
both providers authenticate out of `HOME` (`~/.claude`, `~/.codex`), the
session's own toolchain is spread across the machine, and an allow-list
tight enough to be worth calling containment breaks the session it is
protecting. 125 §6 already named that trade ("Closing that needs a different
`HOME` per session, which would break both providers' own authentication").
A deny-list that provably closes the four enumerated reaches is worth more
than an allow-list that has to be widened until it closes none of them.

### B-3. The deny set

Denied for reading:

- `~/.ssh` (subpath): the keys, `known_hosts` and any `config` naming them;
- `~/.config/gh` (subpath): where `gh` stores a token in plaintext.
  125 already points the session's `GH_CONFIG_DIR` at an empty directory;
  this denies the real one, which is what an absolute read would have
  reached around that;
- `~/.git-credentials` and `~/.config/git/credentials` (literal).

Denied for execution:

- every `gh` and `ssh` binary discoverable on the host at profile-build
  time, by absolute path, plus the conventional locations
  (`/usr/bin/ssh`, `/opt/homebrew/bin/gh`, `/usr/local/bin/gh`);
- `/usr/bin/security` (macOS), the keyring CLI 125 §6 names.

Denied for service lookup:

- `com.apple.SecurityServer` (macOS), so a process linking the Security
  framework directly does not reach the keychain the CLI was denied.

The fence (125) stays on top of all of this. It is not made redundant: the
fence is what produces a *legible refusal* naming the broker, and the
sandbox is what makes the refusal unavoidable. A session that runs `gh`
from `PATH` should meet 125's message, not an opaque OS denial.

### B-4. The wrapper is applied at the session spawn

On a supported platform, `wrapArgv(profile, argv)` returns
`["sandbox-exec", "-f", <profile>, "--", ...argv]`. `driver.ts` applies it
to the member spawn that already receives `applyFence(env, fenceDir)`, so
the provider inherits the confinement the same way it inherits the fence,
without the member knowing about it.

### B-5. The candidate commits unsigned

Denying `~/.ssh` denies the SSH signing key this repository's
`commit.gpgsign` + `gpg.format=ssh` configuration uses, so a session inside
the sandbox cannot sign. The candidate's git configuration therefore sets
`commit.gpgsign=false` for the candidate only.

This costs nothing that is currently being paid for. Every commit on `main`
is created by GitHub's squash merge and signed with GitHub's key; the local
key never signs anything that lands. D-3 records the evidence and the
condition under which this stops being true.

### B-6. An unsupported platform degrades and is journaled, never refuses

`sandboxSupport()` reports `seatbelt` where `/usr/bin/sandbox-exec` is
executable, `bwrap` where `bwrap` is on `PATH`, and `none` otherwise. On
`none` the session runs unsandboxed and the engine journals
`sandbox.unavailable` with the platform and the reason, exactly as 124
journals an unqualified binary and 120 journals a degraded capability. A
run is not refused for want of a sandbox: the engine's job is to record
what protection was actually in force, not to stop work on a machine that
cannot offer it.

`bwrap` support is declared here and built here only to the extent the
Linux path is exercised by the suite; the live evidence in §5 is macOS,
and D-4 says so rather than implying a confinement nobody has run.

### B-7. The blast radius is the provider session only

The sandbox wraps the member the driver spawns and nothing else. The
daemon's own processes are untouched, which is what keeps the broker
working: the broker pushes, opens and merges from the daemon (122), and it
must keep the credentials the candidate is denied. This is 125 B-6's
property restated for the OS boundary, and it is asserted in
`sandbox.test.ts` for the same reason 125 D-4 put that case beside the
fence's own suite.

### B-8. A denial is counted and reaches the evidence

Seatbelt denials are reported on the child's stderr. The driver tallies
them into `sandboxDenials` on `SessionEvidence`, required with an explicit
zero, which is 125 D-3's shape and for its reason: a missing tally must not
read as "nothing was denied" to a consumer downstream.

## 4. Functional requirements

- **FR-001.** `sandbox.test.ts` asserts the profile text is a pure function
  of its inputs, and that a hand-edited profile is overwritten on the next
  `openCandidate`.
- **FR-002.** Deny-set tests: each B-3 entry is present in the built
  profile, and the profile parses (`sandbox-exec` accepts it) on a machine
  reporting `seatbelt`.
- **FR-003.** Wrapper tests: `wrapArgv` returns argv unchanged when support
  is `none`, and the `sandbox-exec` form otherwise.
- **FR-004.** Blast-radius test: a process the daemon spawns itself is
  unwrapped (B-7), asserted beside the broker's own publish path.
- **FR-005.** Evidence tests: `sandboxDenials` is present with an explicit
  zero on a clean run and reaches each of the three stage results.
- **FR-006.** Degradation test: with support forced to `none`, a session
  runs and `sandbox.unavailable` is journaled once.
- **FR-007.** Signing test: a candidate opened under a supported sandbox has
  `commit.gpgsign=false` in its candidate-local configuration (B-5).

## 5. Acceptance

- `bun test` in `members/` is green, including the new suite.
- `make gate` exits 0 and `make members` exits 0.
- A live round on this repository, on a machine reporting `seatbelt`, with
  the four reaches confirmed open on the host beforehand: inside the
  sandbox, reading `~/.ssh` fails, reading `~/.config/gh/hosts.yml` fails,
  executing `gh` by absolute path fails, and executing `/usr/bin/security`
  fails, while ordinary work in the worktree succeeds.
- The same round's ship stage publishes through the broker, and the journal
  shows `broker.action` for `push` and `openPr`: the boundary confined the
  session without disarming the engine (B-7).

## Verification

```sh
cd members && bun test src/orchestrator/sandbox.test.ts
cd members && bun test src/orchestrator/candidate.test.ts
cd members && bun test src/orchestrator/driver.test.ts
make gate
```

## 6. Out of scope

The other half of doc 04 §11. This spec is the sandboxed executor; the
**brokered file service** is not built here. The profile denies, it does not
mediate: there is no path by which a session asks the engine for a file it
was refused, and no record of such a request. That remains open.

Also out of scope, and still open after this spec:

- **Network confinement.** The session keeps general network access, which
  it needs to be a coding agent at all. A session that means to exfiltrate
  or to publish over HTTPS with a token it obtained some other way is not
  stopped by a filesystem profile.
- **A token already in the session's context.** The sandbox denies reads of
  credential material on disk. It cannot un-know a secret that reached the
  session another way.
- **Linux as a qualified platform.** See B-6 and D-4.
- **`sandbox-exec` being a supported Apple interface.** It is deprecated and
  present; D-2 records the bet and the fallback.
- Signing the action record (122 §6), a permit format for the hosted plane,
  and per-stage driver routing.

## 7. Resolved decisions

D-1 (2026-09-10). Deny-list, not allow-list. An allow-list is the stronger
shape and was rejected on evidence: both providers authenticate out of
`HOME`, the session's toolchain is spread across the machine, and the
allow-list would have to be widened on every provider upgrade until it
enumerated most of the filesystem. 125 §6 had already identified the same
trade from the other side. The deny-list's weakness is stated rather than
finessed: it closes the reaches it names, and a reach nobody enumerated
stays open. The four in B-3 are the four 125 §6 enumerated, which is the
scope this spec claims and no more.

D-2 (2026-09-10). Seatbelt (`/usr/bin/sandbox-exec`) on macOS, not a
container. A container changes the developer's loop, the worktree's
identity and the provider's own installation, for a spec whose entire job is
to deny four reads. Seatbelt is deprecated by Apple and still shipped
(present on macOS 26.5.1, the machine this was written on) and is what
Codex's own sandbox uses on macOS, so the bet is one the ecosystem is
already making. If Apple removes it, `sandboxSupport()` reports `none` and
B-6's degradation is the fallback, which is why that path exists rather than
a refusal.

D-3 (2026-09-10). The candidate commits unsigned (B-5). Denying `~/.ssh`
denies the signing key, so signing inside the sandbox is impossible, and the
question is only whether anything depends on it. Nothing does: every commit
on `main` is committed by `GitHub <noreply@github.com>` and signed with
GitHub's key, because the squash merge is created server-side. The local
key's signature on a feature-branch commit is never what satisfies branch
protection. This stops being true if the protection rule is changed to
require every commit in a pull request to be signed, or if merges stop being
squashes; either change makes B-5 wrong and this spec must be revisited
rather than worked around.

D-4 (2026-09-10). macOS is qualified, Linux is declared. The `bwrap` branch
is written and unit-tested, but §5's live criterion is macOS only, and this
spec does not claim a Linux confinement it has not run. That mirrors 124's
distinction between a claimed capability and a qualified one: a claim
nobody exercised is a claim, and the record should say which it is.

D-5 (2026-09-10). The fence (125) stays, and is not folded into the
profile. They do different jobs: the fence produces a legible refusal that
names the broker and is tallied as `fence.refused`, and the sandbox makes
the refusal unavoidable. Removing the fence would replace a message a
session can act on with an OS denial it cannot interpret, and would lose the
journal kind 125 added to the export policy at version 5.

## Status (2026-09-10)

Authored `draft`, `implementation: pending`. It comes from doc 04 §11's
first open item and from the three residuals 125 §6 recorded.

The four reaches in B-3 were probed on the authoring machine (macOS 26.5.1)
before this spec was written, to establish that the mechanism closes them
rather than to assume it: under a profile of the B-2 shape, reading `~/.ssh`
was denied, reading `~/.config/gh/hosts.yml` was denied, executing `gh` by
absolute path was denied, and `/usr/bin/security` exited 71 (denied) against
44 (permitted, item not found) outside the profile, while an ordinary
command in the worktree still ran. That probe is evidence for feasibility,
not the §5 acceptance round, which runs against a real candidate.

Approval is a human flip.
