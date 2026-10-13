# Spec 042: live provider measurements

The measurements spec 042 section 3.5 requires, recorded with their date,
provider version and platform. Each session was a single turn on the owner's
Claude Code subscription, authorized on 2026-10-12. No token appears here.

Platform: macOS 26.5.1 (Apple silicon). Provider: Claude Code 2.1.296.

## Before 042 (motivating evidence)

| Probe | Result |
|---|---|
| A fresh `CLAUDE_CONFIG_DIR`, no token | `Not logged in`: the macOS Keychain item is keyed per configuration directory. |
| A fresh `CLAUDE_CONFIG_DIR` holding a `.credentials.json` | The file is read (a fabricated token failed at refresh), so the directory is where the provider looks. |

## M-1: per-run directory plus provisioned token, unconfined

The constructed environment only (`PATH`, `USER`, `CLAUDE_CONFIG_DIR` at a
fresh `0700` directory, `CLAUDE_CODE_OAUTH_TOKEN` from the provisioned token).
Result: `completed`, `is_error: false`, answer `ok`. No token bytes in the
output or standard error. **Pass.**

## M-2: the first confined provider session

`sandbox-exec` with the product's profile, writes confined to the per-run
directory, a scratch root and the working directory, the product home
unreadable, and `(deny mach-lookup)` plus an allowlist. Denials were read from
the unified log's sandbox reports for the provider's signing identity
(`com.anthropic.claude-code`).

| Round | Allowlist | Session | Mach lookups refused |
|---|---|---|---|
| 0 | none | exits before its first turn: `uv_os_homedir` ENOENT | `com.apple.system.opendirectoryd.libinfo` |
| 1 | `opendirectoryd.libinfo` | `completed`, answer `ok` | `notification_center`, `logd` (outcome unchanged) |
| 1b | same | `completed`, answer `ok` | `notification_center`, `logd`, `securityd.xpc` (outcome unchanged) |
| tool | same | the Bash tool fails: `EPERM` creating its directory under `/private/tmp/claude-501/` | `notification_center`, `logd` |
| tool 2 | same, plus `CLAUDE_CODE_TMPDIR` at the scratch root | `completed`, answer `done`; the tool wrote the workspace file | `notification_center`, `logd` |

Outcome: the allowlist is one entry, `com.apple.system.opendirectoryd.libinfo`
(account lookup, needed because the constructed environment has no `HOME`).
The three refusals that changed no outcome are not admitted (section 3.4 rule
2). The provider's own temporary area is pointed at the launch's scratch root
with `CLAUDE_CODE_TMPDIR`: granting `/private/tmp/claude-<uid>` instead would
let the child write files the operator's later sessions read, the route
section 3.1 closes. The Keychain service is refused and the session still
authenticates, by token. **Pass.**

## Through the product's own launch path

`scripts/acceptance/managed-session.sh`, built from this change, with the
operator's provisioned token copied into the fixture home (2026-10-12, local
time; records stamped 2026-10-13T00:13Z to 00:14Z):

| Stage | Sessions | Result |
|---|---|---|
| `preflight` | none (synthetic fake) | every precondition holds |
| `managed-startup` | one, through `run` | **ESTABLISHED**: `outcome completed`, the hook correlated, harness `h-0ce987c8c560` reached, the read admitted only after the gate |
| `permission-experiment` | three (refusal, allowed-command, without-payload) | **ADMITTED**: each launch confined (macOS), program digest `c9b53416…`, probed 2.1.296; the observation is recorded, launcher-attested and version specific |

No token bytes were found anywhere under the run's root except the owner-only
credential copy. These are spec 015's first admitted live permission
experiment and spec 004 rule 12's activation, on macOS.

## M-3: the operator's provider configuration is not writable

`per_run_provider_configuration` runs a hostile child through the real `run`
path and the real confinement: writes to `~/.claude/settings.json` and
`~/.claude.json` and a new file under `~/.claude/` are refused and the bytes
are unchanged, the child writes its own per-run directory, which is gone after
the attempt, and no output, record or remaining file carries the token. Run on
macOS locally and on Linux in CI (#253). **Pass.**

## Linux

Docker Desktop's Linux VM (kernel 7.0.14-linuxkit, Landlock among the active
security modules), the product built from `main` at the time inside a
`rust:1.96-bookworm` container, Claude Code 2.1.296 installed there, the
operator's token mounted read-only and copied into the fixture home. The
container ran with its own seccomp profile disabled so the product's filter is
the one in force; the product installs Landlock and seccomp itself.

| Stage | Sessions | Result |
|---|---|---|
| `preflight` | none (synthetic fake) | every precondition holds |
| `managed-startup` | one, through `run` | **ESTABLISHED** |
| `permission-experiment` | three | **ADMITTED**; each launch recorded `landlock-and-seccomp` on `linux` |

No token bytes were found under the run's root except the owner-only
credential copy. With M-3 already run on Linux in CI (#253), M-1 to M-3 are
covered on Linux. **Pass.**

## Coverage

Rule 12's two IX items are closed on macOS, and its later-unconfined-consumer
item on Linux (the Mach-service item does not arise there), for Claude Code
2.1.296. Another provider version is a new measurement.
