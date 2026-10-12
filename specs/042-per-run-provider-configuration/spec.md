---
id: "042-per-run-provider-configuration"
title: "Per-run provider configuration and a measured Mach-service allowlist close spec 004's two open items against IX"
status: draft
implementation: pending
created: "2026-10-12"
summary: >
  Amends 004 section 3.18 to close the two items its rule 12 names as open
  against constitution IX. A confined provider process gets its own
  configuration directory inside the attempt, discarded after it, and is
  authenticated by a provider token the operator provisions once, so the
  child no longer writes any file that an unconfined process later loads. On
  macOS every Mach service is refused except an allowlist measured against a
  confined provider session. Both rest on live measurements this spec names,
  and the first confined provider session is the activation rule 12 defers.
amends:
  - "004-execution-adapter"
# No `extends` yet: the implementing change adds edges on
# crates/statecraft-home/ (002) and crates/statecraft-adapter/ (004) with the
# code, because the gate counts an extender as an owner.
depends_on:
  - "002-environment-lifecycle"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Every supervised provider process runs with a fresh per-run configuration directory inside the attempt as its only provider-configuration writable root, removed after the process tree exits and never read by an unconfined process; the operator's provider configuration is not writable by the child."
    anchor: "3-1-the-per-run-configuration-directory"
  - id: "R-2"
    kind: requirement
    text: "The provider is authenticated by an operator-provisioned token held under the product home, passed only in the constructed environment, never written to a record, log, argument or file the child can read, and its absence refuses the launch with exit 2."
    anchor: "3-2-the-provider-token"
  - id: "R-3"
    kind: requirement
    text: "On macOS the confinement refuses every Mach service lookup except a fixed allowlist, each entry justified by a recorded measurement against a confined provider session."
    anchor: "3-4-the-mach-service-allowlist"
  - id: "R-4"
    kind: requirement
    text: "Rule 12's two open items are reported closed only for the provider version and platform their live measurements cover, and the attempt's posture records that coverage."
    anchor: "3-5-what-closes-and-what-is-measured"
---

# 042: Per-run provider configuration and a measured Mach-service allowlist

## 1. Purpose

Spec 004 section 3.18 rule 12 names two gaps against constitution IX and says
they keep every spec that accounts for IX from `complete`:

1. **The route through a later unconfined process.** Rule 3 grants the child
   writes to the operator's provider configuration (`~/.claude/` and
   `~/.claude.json`), which the operator's own later sessions load and act
   on. A hook placed there runs outside the confinement and can reach the
   protected set.
2. **Unmeasured Mach services on macOS.** The profile refuses three named
   services and allows the rest, so whether another one starts or instructs a
   process outside the confinement is unknown.

Rule 12 says closing the first needs "a separate operating-system principal
for the child, or a per-run provider configuration directory whose effect on
the provider's authentication has been measured live", and that both are the
owner's. On 2026-10-12 the owner chose the per-run directory and authorized
the live provider sessions it needs, on the Claude Code subscription.

## 2. Territory

At draft this spec owns no code. The implementing change adds additive
`extends` edges on spec 002's `crates/statecraft-home/` (the confinement
roots and the launch) and spec 004's `crates/statecraft-adapter/` (the Seatbelt
profile and the constructed environment), with the code.

## 3. Behavior

### 3.1 The per-run configuration directory

1. **Fresh, inside the attempt.** Before each supervised provider process
   (spec 004 section 3.18 rule 1: `run`, `startup trial`, `startup capture`),
   the supervisor creates an empty directory, mode `0700`, inside that
   process's scratch root, and names it to the provider in the constructed
   environment (for Claude Code, `CLAUDE_CONFIG_DIR`). Nothing else from the
   operator's provider configuration is copied into it.
2. **The only provider-configuration writable root.** Rule 3's grant of the
   operator's provider configuration directory and its configuration file is
   withdrawn on both platforms. Those paths stay readable, as every path
   outside the protected set is, and are no longer writable.
3. **Discarded.** When the process tree has exited, the supervisor removes the
   directory. No verb of this product reads from it, before or after, and no
   process this product starts outside the confinement is given it.
4. **What a managed session still receives.** Harness delivery for a managed
   session is the launch configuration of spec 008 section 3.27, passed on the
   command line from the canonical harness, and is unchanged. The operator's
   user-global settings, plugins and servers are not loaded in a confined
   attempt; that is the point, and it is stated in the attempt's posture.

### 3.2 The provider token

1. **Provisioned by the operator, once.** The operator creates a long-lived
   provider token with the provider's own command (for Claude Code,
   `claude setup-token`) and stores it with
   `statecraft-cli provider token set`, which reads it from standard input and
   writes `credentials/claude-code-token` under the product home, mode `0600`,
   by a no-replace rename. `provider token clear` removes it. Neither verb
   prints the token.
2. **Passed only in the environment.** At launch the supervisor reads the file
   and adds exactly one name to the constructed environment (for Claude Code,
   `CLAUDE_CODE_OAUTH_TOKEN`). The token never appears in an argument, a
   record, a log, a `--json` answer, the per-run directory or any other file;
   a test scans every output and every file the attempt leaves for its bytes.
3. **Refused when absent.** A confined launch with no token refuses with exit
   2, names `statecraft-cli provider token set` and starts nothing. The
   product never falls back to the operator's own login, which a per-run
   directory cannot reach and which a refresh by the child could rotate.
4. **Its residual, stated.** The token is in the child's environment for the
   life of the attempt, and a child that means to can send it over the network
   the confinement allows. That is the residual spec 004 section 3.14 already
   declares for a keychain credential, made concrete for a token the operator
   can revoke on its own; section 3.6's statement that this product claims
   nothing about credentials is unchanged.

### 3.3 The constructed environment

Spec 004 section 3.14's allowed set grows from `PATH` and `USER` to those two
plus the configuration-directory name and the token name of section 3.2. It
is still constructed, not filtered. `USER` stays: it is cheap and the measured
keychain path that needed it is the fallback this spec refuses.

### 3.4 The Mach-service allowlist

1. **Refuse by default.** On macOS the Seatbelt profile refuses every
   `mach-lookup` and then allows a fixed allowlist of global names. The three
   services rule 6 refuses by name stay refused, and the test that proves each
   refusal stays.
2. **Each entry is measured.** An entry joins the allowlist only with a
   recorded measurement: a confined provider session of the qualified version,
   run with the refuse-all profile, whose sandbox violation log names the
   service and whose failure the entry cures. The measurement record names the
   provider version, the macOS version, the denial lines and the session's
   outcome before and after. An entry that only silences a log line without
   changing the outcome is not admitted.
3. **What an entry may not be.** No entry is a service that starts, opens or
   instructs a process outside the confinement: LaunchServices, Apple events,
   `launchd` job submission, or any service whose documented role is one of
   those. The allowlist is a constant in the profile, and changing it is an
   amendment with its own measurement.

### 3.5 What closes, and what is measured

1. **Measurements.** Three live measurements, each on the Claude Code
   subscription and each recorded under `docs/adoption/` with its date,
   provider version and platform:
   - **M-1**: a fresh per-run directory plus the provisioned token
     authenticates and completes a one-turn session, unconfined.
   - **M-2**: the first confined provider session (rule 12's activation)
     completes a one-turn session under the refuse-all Mach profile plus the
     allowlist; the allowlist is what this measurement produces.
   - **M-3**: the same confined session's child cannot write the operator's
     provider configuration: the hostile fixtures of spec 004's acceptance
     run through the real launch path and assert refusal and unchanged bytes
     for `~/.claude/` and `~/.claude.json`.
2. **Coverage.** Rule 12's two open items are closed for the platform and
   provider version M-1 to M-3 cover, and only there. On a platform without
   them (Linux until measured) the attempt's posture records each item as
   open, exactly as rule 11 requires today.
3. **Rule 12's other bullets** (operator-executed content, the
   administrator, the child-attested gate log, Linux UDP and port-443
   listeners) are unchanged and remain residuals, not open items.

### 3.6 Observable negative cases

| Case | Required result |
|---|---|
| A confined launch with no provisioned token | Exit 2 naming `provider token set`; no process started. |
| The child writes `~/.claude/settings.json` or `~/.claude.json` | Refused; bytes unchanged. |
| The child writes inside its per-run directory | Allowed; the directory is gone after the attempt. |
| Any output, record or remaining file of an attempt | Contains no token bytes. |
| A Mach lookup outside the allowlist (macOS) | Refused by the profile. |
| The three services rule 6 names | Still refused. |
| A platform without M-1 to M-3 | Posture records both items open. |

## 4. Out of scope

- A separate operating-system principal for the child (the other route rule
  12 names).
- Any credential beyond the provider token: publication credentials stay out
  of the child (spec 004 section 3.6).
- Keychain custody or rotation of the operator's own provider login.
- A second provider. The configuration-directory and token names are this
  provider's; another adapter states its own.

## 5. Open questions for ratification

- **O-1. Linux.** M-1 to M-3 are planned on macOS, the platform this machine
  runs. Linux closes only after its own M-2 and M-3 (a Linux host or
  container with the provider installed). Ratification should confirm that
  rule 12 may read closed per platform.
- **O-2. The token's home.** A `0600` file under the product home, as spec 038
  settled for the hosted credential, or the macOS Keychain.

## Verification

Implementation acceptance is not run while this spec is a `draft` with
`implementation: pending`.
