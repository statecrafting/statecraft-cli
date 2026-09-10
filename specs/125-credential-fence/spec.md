---
id: "125-credential-fence"
title: "The credential fence: the broker is the only path that works, so the journal is complete"
status: approved
created: "2026-09-10"
implementation: in-progress
risk: medium
depends_on:
  - "122-action-broker"
  - "121-candidate-and-receipt"
  - "124-provider-conformance"
establishes:
  - "members/src/orchestrator/fence.ts"
  - "members/src/orchestrator/fence.test.ts"
extends:
  # 121 owns the candidate and its environment scrub; the fence is built where
  # the scrub is, and replaces "remove four names" with "construct the child's
  # credential environment".
  - { spec: "121-candidate-and-receipt", unit: "members/src/orchestrator/candidate.ts", nature: additive }
  - { spec: "121-candidate-and-receipt", unit: "members/src/orchestrator/candidate.test.ts", nature: additive }
  # 111 owns the contract crate; CHILD_ENV_DENY grows and the fixture with it.
  - { spec: "111-contract-crate", unit: { kind: directory, path: "crates/statecraft-contract/" }, nature: additive }
  # 114 and 116 own the providers, which read the contract's list on both sides.
  - { spec: "114-driver-port", unit: { kind: directory, path: "crates/statecraft-driver-core/" }, nature: additive }
  - { spec: "116-codex-driver", unit: "members/src/members/driver-codex.test.ts", nature: additive }
  # 122 owns the broker, which keeps the daemon's unfenced environment.
  - { spec: "122-action-broker", unit: "members/src/orchestrator/broker.ts", nature: additive }
  - { spec: "122-action-broker", unit: "members/src/orchestrator/broker.test.ts", nature: additive }
  # The three publishing stages pass the fence to the session they spawn.
  - { spec: "016-stage-build", unit: "members/src/orchestrator/stages/build.ts", nature: additive }
  - { spec: "017-stage-ship", unit: "members/src/orchestrator/stages/ship.ts", nature: additive }
  - { spec: "018-stage-shepherd", unit: "members/src/orchestrator/stages/shepherd.ts", nature: additive }
summary: >
  Spec 122 moved publishing to the broker so every push, pull request and
  merge is journaled as intent and outcome over a receipt and a lease. It
  scrubbed four names from the candidate's environment and recorded a known
  limit: a `gh` authenticated through a keyring still answers, so a session
  can publish around the broker and leave no record. The limit is wider than
  122 named, because the remote is reached over SSH as readily as over a
  token. This spec constructs the candidate's credential environment instead
  of filtering it: a fence directory first on `PATH` whose `gh` and `ssh`
  refuse and name the broker, `git` redirected to a candidate-owned config
  with no credential helper and no agent, and the agent socket added to the
  deny list. The property bought is integrity, not confidentiality: the
  broker becomes the only path that works, so 122's journal is complete.
  Containment against a session that means to escape stays doc 04 §11's
  sandboxed executor.
---

# 125: The credential fence

## 1. Purpose

Spec 122 established the action boundary: the engine publishes on a receipt
and a lease, journaling `broker.action` intent and outcome, and the session
proposes PR text into the drop box rather than running `gh` itself. The
boundary's value is the journal. If every publish goes through the broker,
the journal is a complete record of what this machine did to a remote.

122 B-7 scrubbed `GH_TOKEN` and `GITHUB_TOKEN` from the candidate and
recorded what the scrub could not reach: a `gh` authenticated through a
keyring. On the machine this spec was written on, `gh auth status` reports
`Logged in to github.com account (keyring)` with `repo` and `workflow`
scopes, and `git remote -v` is `git@github.com:...`. So the candidate can
reach the remote two ways the deny list does not touch:

1. `gh` resolves its token from the system keyring, with no environment
   variable involved.
2. `git push` over SSH authenticates from `SSH_AUTH_SOCK` or a key under
   `~/.ssh`, neither of which is denied.

The threat model is not a malicious session. It is a capable one. A coding
agent told to ship will run `gh pr create` when `gh` works, because that is
the obvious move, and 122's prompt asking it not to is a request rather
than a boundary. Every such publish is invisible to the journal, unreceipted
and unleased. The fence makes the obvious move fail with a message naming
the broker, so the only path that works is the one that is recorded.

This is an integrity property. It is not containment, and §6 says so.

## 2. Territory

- `members/src/orchestrator/fence.ts`: builds the fence for a candidate
  (the directory, the shims, the git config) and returns the environment
  overlay a spawned session receives.
- `members/src/orchestrator/fence.test.ts`: the negative suite, run from
  inside a real fenced candidate.

Extended, each owned elsewhere and named in the frontmatter: `candidate.ts`
(the scrub becomes the fence's first half), the contract crate (the deny
list and its fixture), the two providers (which read the list), the broker
(which keeps the daemon's environment), and the three publishing stages
(which pass the fence to the session).

## 3. Behavior

### B-1: the fence is constructed, not filtered

`scrubEnv` today copies the parent environment minus four names. That shape
can only remove what is named, and the reach that matters is not named by
any variable. `fenceEnv(candidate)` returns the same scrub plus an overlay
that redirects every credential resolution a child could attempt. The deny
list stays the subtractive half and keeps its contract role; the overlay is
the new additive half.

### B-2: the fence directory

A fence lives at `<homeDir>/fences/<project>/<branch>/`, beside the
candidate and never inside it: a directory under `<candidate>/` would dirty
the tree that 121 B-4 requires clean before a receipt. It holds:

- `bin/gh`: an executable that writes to stderr "gh is fenced in a driven
  session; publishing goes through the broker (spec 122). Write the PR text
  to the proposal drop box." and exits 127.
- `bin/ssh`: the same shape, naming the broker, exit 127.
- `gitconfig`: `credential.helper` set to the empty string (which resets the
  inherited helper chain rather than adding to it), `core.sshCommand` set to
  the fenced `ssh`, and `url.*.insteadOf` absent.

The shims are written on every `openCandidate` and their content is a
constant, so a stale fence from a crashed run is overwritten rather than
trusted.

### B-3: the overlay

The environment a session receives gains, over the scrub:

| Name | Value | What it closes |
|---|---|---|
| `PATH` | `<fence>/bin:` + inherited | `gh` and `ssh` resolve to the shims |
| `GH_CONFIG_DIR` | `<fence>/gh` (empty) | `gh` reads no `hosts.yml` |
| `GIT_CONFIG_GLOBAL` | `<fence>/gitconfig` | no inherited credential helper |
| `GIT_CONFIG_SYSTEM` | `/dev/null` | no system helper |
| `GIT_TERMINAL_PROMPT` | `0` | no interactive credential prompt |
| `GIT_ASKPASS`, `SSH_ASKPASS` | `<fence>/bin/gh` | no askpass fallback |

`HOME` is deliberately untouched: both providers authenticate from it
(`~/.claude`, `~/.codex/auth.json`), and 014 B-2 already turns on the rule
that an empty or unset key must not shadow OAuth. §6 records what leaving
`HOME` in place means.

### B-4: the deny list grows, and stays one list

`CHILD_ENV_DENY` becomes six: the four it has plus `SSH_AUTH_SOCK` and
`SSH_AGENT_PID`. The contract crate is the single definition, `candidate.ts`
carries the same six, and `child-env-deny.json` is what both sides assert
against, exactly as 121 B-3 and 122 B-7 established. The two Rust providers
pick the change up from the constant; their length assertions move from four
to six.

### B-5: the fence is proven from inside, not asserted from outside

In 124's idiom, a negative suite. Each case spawns a process with the fenced
environment in a real candidate and asserts the refusal:

- `gh auth token` exits non-zero and prints the broker message.
- `gh api user` exits non-zero.
- `git credential fill` over `protocol=https\nhost=github.com` returns no
  `password` line.
- `git -c protocol.version=2 ls-remote <local bare remote>` over an SSH-style
  URL fails at the fenced `ssh`.
- `git push` to a local bare remote over a `file://` URL still succeeds,
  which proves the fence closes credentials and not git itself.

No case touches the real `github.com`. The bare remote on disk is 122's
existing live-smoke shape.

### B-6: the broker is not fenced

The broker's own `gh` and `git push` run with the daemon's environment,
unscrubbed and unfenced, exactly as 122 B-7 says. A test asserts that a
broker holding a receipt and a lease still pushes and opens a PR while the
session beside it is fenced. The fence is a property of the child's
environment, not of the process tree.

### B-7: a fenced refusal is journaled

When a shim runs, it appends a line to `<fence>/refusals.log`, and the stage
folds a non-empty log into the session result as `fenceRefusals: n`. A
session that tried to publish around the broker is a fact worth having in
the journal, and it is the signal that a prompt still tells a session to do
something the boundary forbids. `fence.refused` joins the export allowlist
with `detail` stripped, as `broker.refused` does.

## 4. Functional requirements

- **FR-001.** `fence.test.ts` covers every B-5 case, each asserting the exit
  code and the stream the message lands on.
- **FR-002.** Deny-list tests: the list is six names on both sides, and
  `child-env-deny.json` matches the Rust constant and the TypeScript one.
- **FR-003.** Overlay tests: each B-3 row is present in the returned
  environment, `PATH` has the fence first, and `HOME` is unchanged from the
  parent.
- **FR-004.** Idempotence: `openCandidate` twice over the same branch leaves
  one fence with the constant shim content, and a hand-edited shim is
  overwritten.
- **FR-005.** Broker tests: B-6's case, the broker publishing beside a
  fenced session.
- **FR-006.** Stage tests: build, ship and shepherd each spawn with the
  fenced environment, and a `fenceRefusals` count reaches the result.

## 5. Acceptance

- `bun test` in `members/` is green, including the new suite.
- `cargo test` is green; the contract fixture asserts six names.
- `make gate` exits 0 and `make members` exits 0.
- A live round on this repository: a build session in a fenced candidate,
  with `gh auth status` confirmed keyring-backed on the host beforehand,
  cannot obtain a token; the ship round publishes through the broker and the
  journal shows `broker.action` for `push` and `openPr`.

## Verification

```sh
cd members && bun test src/orchestrator/fence.test.ts
cd members && bun test src/orchestrator/candidate.test.ts
cargo test -p statecraft-contract
make gate
```

## 6. Out of scope

Containment. The fence closes the ergonomic path, not the determined one,
and this is the honest limit to record rather than a gap to hide:

- `HOME` stays readable, so a session can read `~/.config/gh/hosts.yml`
  where `gh` stores a token in plaintext, or call the platform keyring
  directly (`security find-generic-password` on macOS). Closing that needs
  a different `HOME` per session, which would break both providers' own
  authentication, or a real sandbox.
- A binary invoked by absolute path bypasses `PATH`, so `/opt/homebrew/bin/gh`
  still runs.
- `~/.ssh` keys remain on disk and readable; only the agent and the `ssh`
  on `PATH` are fenced.

Each of these is closed by doc 04 §11's sandboxed executor with a brokered
file service, which is a larger sequence and not this spec. Also out of
scope: signing the action record (122 §6), a permit format for the hosted
plane, and removing `gh` from the operator's machine.

## 7. Resolved decisions

None yet; this spec is authored ahead of its build. Decisions taken during
the build are recorded here with their date, per `AGENTS.md` step 3.

## Status (2026-09-10)

Authored `draft`, `implementation: pending`. It comes from doc 04 §11's
open limits and from the known limit 122 recorded three times (B-7, §5, D-4).

Approved by the owner on 2026-09-10, as written, and started the same day.
