// Spec 125: the credential fence (doc 04 §11, and the limit 122 recorded in
// B-7, §5 and D-4). Spec 122 moved publishing to the broker so every push,
// pull request and merge is journaled as intent and outcome over a receipt
// and a lease. The value of that boundary is the journal, and the journal is
// only complete if the broker is the only path that works.
//
// 122 scrubbed two token names from the candidate's environment and recorded
// what a name list cannot reach: a `gh` authenticated through the system
// keyring. The reach is wider still, because a repository whose origin is an
// ssh URL is pushed to from `SSH_AUTH_SOCK` or a key under `~/.ssh`. Neither
// is environment-variable shaped, so `CHILD_ENV_DENY` cannot close either
// however many names it grows.
//
// The fence therefore *constructs* the child's credential environment rather
// than filtering it: refusing shims first on PATH, git redirected to a
// config with no credential helper and no agent, and gh pointed at an empty
// config directory. The property bought is integrity, not confidentiality:
// the obvious move fails and names the broker, so a capable session cannot
// casually publish around the boundary and leave no record. A session that
// means to escape is not contained by this, and spec 125 §6 says so.

import * as fs from "fs";
import { join } from "path";

// What a shim exits with. 127 is "command not found", which is what a caller
// that shells out reads as "this tool is not here" without special casing.
export const FENCE_REFUSAL_EXIT = 127;

const GH_MESSAGE =
  "gh is fenced in a driven session; publishing goes through the broker (spec 122). Write the PR text to the proposal drop box.";
const SSH_MESSAGE =
  "ssh is fenced in a driven session; publishing goes through the broker (spec 122). The engine pushes on a receipt and a lease.";

// --- layout (B-2) ------------------------------------------------------------

// The fence sits beside the candidate, never inside it: a directory under the
// worktree would dirty the tree that 121 B-4 requires clean before a receipt.
export function fencePath(homeDir: string, project: string, branch: string): string {
  return join(homeDir, "fences", project, branch);
}

export function fenceBinDir(fenceDir: string): string {
  return join(fenceDir, "bin");
}

export function fenceRefusalLog(fenceDir: string): string {
  return join(fenceDir, "refusals.log");
}

// The shim source is a pure function of (fence directory, tool), so a stale
// fence from a crashed run is overwritten rather than trusted (B-2).
export function shimSource(fenceDir: string, tool: string, message: string): string {
  const log = JSON.stringify(fenceRefusalLog(fenceDir));
  return [
    "#!/bin/sh",
    "# Spec 125: the credential fence. Generated on every openCandidate;",
    "# hand edits are overwritten. See specs/125-credential-fence/spec.md.",
    `printf '%s %s\\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" ${tool} >> ${log} 2>/dev/null`,
    `printf '%s\\n' ${JSON.stringify(message)} >&2`,
    `exit ${FENCE_REFUSAL_EXIT}`,
    "",
  ].join("\n");
}

// An empty `credential.helper` resets the inherited helper chain rather than
// appending to it, which is the one git spelling that unsets a helper the
// operator configured globally or in a system file.
export function gitConfigSource(fenceDir: string): string {
  return [
    "# Spec 125: the credential fence. Generated; hand edits are overwritten.",
    "[credential]",
    "\thelper =",
    "[core]",
    `\tsshCommand = ${JSON.stringify(join(fenceBinDir(fenceDir), "ssh"))}`,
    "",
  ].join("\n");
}

// --- construction (B-2) ------------------------------------------------------

// Idempotent: every call rewrites the shims and the config from the constants
// above and starts a fresh refusal tally for the round about to run.
export function buildFence(fenceDir: string): string {
  const bin = fenceBinDir(fenceDir);
  fs.mkdirSync(bin, { recursive: true });
  // gh reads `hosts.yml` from its config directory; an empty one that exists
  // is the difference between "no credential" and "fall back to the default".
  fs.mkdirSync(join(fenceDir, "gh"), { recursive: true });
  for (const [tool, message] of [
    ["gh", GH_MESSAGE],
    ["ssh", SSH_MESSAGE],
  ] as const) {
    const path = join(bin, tool);
    fs.writeFileSync(path, shimSource(fenceDir, tool, message), { mode: 0o755 });
    // writeFileSync's mode applies on create only; an existing file keeps its
    // bits, and a fence reused from a crashed run must still be executable.
    fs.chmodSync(path, 0o755);
  }
  fs.writeFileSync(join(fenceDir, "gitconfig"), gitConfigSource(fenceDir));
  fs.writeFileSync(fenceRefusalLog(fenceDir), "");
  return fenceDir;
}

// --- the overlay (B-3) -------------------------------------------------------

// Applied over the deny-list scrub, never instead of it: the scrub is the
// subtractive half and keeps its contract role (121 B-3), this is the
// additive half. HOME is deliberately absent from the table; both providers
// authenticate from it (`~/.claude`, `~/.codex/auth.json`) and 014 B-2
// depends on that. Spec 125 §6 records what leaving it in place means.
export function fenceOverlay(fenceDir: string, inheritedPath: string | undefined): Record<string, string> {
  const bin = fenceBinDir(fenceDir);
  return {
    PATH: inheritedPath === undefined || inheritedPath.length === 0 ? bin : `${bin}:${inheritedPath}`,
    GH_CONFIG_DIR: join(fenceDir, "gh"),
    GIT_CONFIG_GLOBAL: join(fenceDir, "gitconfig"),
    GIT_CONFIG_SYSTEM: "/dev/null",
    GIT_TERMINAL_PROMPT: "0",
    GIT_ASKPASS: join(bin, "gh"),
    SSH_ASKPASS: join(bin, "gh"),
  };
}

// The environment a fenced session receives: a scrubbed base with the overlay
// applied. `scrubbed` comes from candidate.ts so the deny list stays one list.
export function applyFence(scrubbed: Record<string, string>, fenceDir: string | null): Record<string, string> {
  if (fenceDir === null) return scrubbed;
  return { ...scrubbed, ...fenceOverlay(fenceDir, scrubbed.PATH) };
}

// --- the tally (B-7) ---------------------------------------------------------

// How many times a shim refused during the round. A non-zero count is a
// session that tried to publish around the broker, which is worth journaling:
// it is the signal that a prompt still asks for something the boundary
// forbids. A missing log is zero, not an error; the fence may not be built.
export function readFenceRefusals(fenceDir: string | null): number {
  if (fenceDir === null) return 0;
  let text: string;
  try {
    text = fs.readFileSync(fenceRefusalLog(fenceDir), "utf8");
  } catch {
    return 0;
  }
  return text.split("\n").filter((line) => line.trim().length > 0).length;
}
