---
id: "051-host-held-provider-credentials"
title: "The supervisor holds the provider token behind a loopback gateway; the child holds only a per-attempt token"
status: approved
implementation: pending
created: "2026-10-21"
summary: >
  Replaces spec 042's delivery of the operator's provider token in the
  child's environment with a loopback authentication gateway the supervisor
  owns. The child receives the gateway's address and a short-lived token
  minted for its attempt, through a 0600 file in the per-run configuration
  directory; the gateway checks that token and forwards with the real one,
  and revokes it when the attempt ends. The provider token never appears in
  the child's environment or files. Claude Code's honouring of a base-URL
  override with an OAuth token in print mode is measured, unconfined; the
  confined measurement is part of acceptance.
amends:
  - "004-execution-adapter"
  - "042-per-run-provider-configuration"
# No `extends` yet: the implementing change adds the edges on
# crates/statecraft-home/ (002), crates/statecraft-adapter-claude-code/ (004)
# and crates/statecraft-cli/ (006) with the code.
depends_on:
  - "004-execution-adapter"
  - "013-supervised-wire-witness"
  - "042-per-run-provider-configuration"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
# Anchors keep the section number's dashes ("3-1-the-gateway" for "### 3.1
# The gateway"), the corpus convention: approved 041 uses
# "3-1-context-selection-and-provenance" and 047 on main "3-1-the-declaration".
# The anchor lint runs inside `make gate` with --fail-on-warn and reports 0
# warnings for every anchor below.
obligations:
  - id: "I-1"
    kind: invariant
    text: "For an unwitnessed confined attempt, the operator's provider token never appears in the child's environment, arguments, per-run directory or any file the attempt can read or leaves behind."
    anchor: "3-1-the-gateway"
  - id: "R-1"
    kind: requirement
    text: "Each attempt gets its own gateway token, written to a 0600 no-replace file in its per-run configuration directory and given to the provider only through the adapter's declared route, accepted by the gateway only for that attempt, and refused by it once the attempt has an outcome."
    anchor: "3-2-the-per-attempt-token"
  - id: "R-2"
    kind: requirement
    text: "The gateway listens on loopback only, forwards only to the provider authority its adapter names, and refuses any request without the attempt's token."
    anchor: "3-1-the-gateway"
  - id: "R-3"
    kind: requirement
    text: "A gateway that cannot start, or an adapter that cannot be routed through one, refuses the launch with exit 2; it never falls back to placing the provider token in the child's environment."
    anchor: "3-4-refusal-not-fallback"
  - id: "V-1"
    kind: verification
    text: "A scan of the child's environment, every file in its per-run directory and every file the attempt leaves finds no byte of the provider token; a request with a revoked or another attempt's token is refused by the gateway."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/src/provider_token.rs"
---

# 051: Host-held provider credentials

## 1. Purpose

Spec 042 section 3.2 rule 4 states the residual plainly: the provider token is
in the child's environment for the life of the attempt, and a child that means
to can send it anywhere its confinement lets it reach. The token outlives the
attempt and is the operator's, so leaking it leaks every future run. This spec
moves the token out of the child. The child gets something that works only
through a gateway the supervisor holds and only until its attempt ends, so a
leak is bounded to one attempt and to the one provider route.

**Drafts this one cites:** spec 047, the capability seam, on main as a draft since pull request 299. Each is cited by its proposed number so the reference survives ratification.

## 2. Territory

The gateway and the per-attempt token are the supervisor's: they live with
spec 042's token store in `statecraft-home`
(`crates/statecraft-home/src/provider_token.rs`) and its launch. The Claude
Code adapter names the variables its provider reads. `statecraft-cli` starts
and stops the gateway around an attempt. No new verb.

## 3. Behavior

### 3.1 The gateway

For each confined attempt the supervisor binds a gateway on a loopback port it
reserves, before the child starts. The gateway accepts plain HTTP from the
child, checks the per-attempt token, replaces it with the operator's token
read from spec 042's store, and forwards over TLS to the one provider
authority the adapter names (`api.anthropic.com:443` for Claude Code). It
forwards nothing else, holds no general proxy behaviour, and answers a request
without the attempt's token with 401 and no forward. The provider token is
held in the supervisor's memory only and is never written by the gateway.

### 3.2 The per-attempt token

The supervisor mints a random token per attempt and writes it to a file in
the attempt's per-run configuration directory (spec 042 section 3.1), mode
0600, created with no-replace. The adapter delivers the gateway's address and
that file to the provider. When the attempt has an outcome the gateway drops
the token, so a copy the child made is useless after its attempt ends.

For Claude Code, the measured route (section 5) is `ANTHROPIC_BASE_URL` set
to the gateway and `CLAUDE_CODE_OAUTH_TOKEN` set to the per-attempt token.
Spec 042 rule 2 forbids the operator's token in the environment; this spec
keeps that and puts only the per-attempt token there, which is exactly what a
leak can be allowed to carry. Whether Claude Code can read the token from the
file directly (a helper command, not the environment) is an implementation
measurement; if it can, the environment carries the file path instead.

### 3.3 Confinement

The child's network grant gains exactly the gateway's loopback endpoint, the
same kind of single reserved-endpoint grant spec 013 gives the witness
sidecar. The gateway runs in the supervisor, outside the child's confinement.

### 3.4 Refusal, not fallback

If the gateway cannot bind, cannot read the operator's token, or the adapter
declares no gateway route, the launch refuses with exit 2 and starts nothing.
No path places the operator's token in the child's environment instead: for
an unwitnessed attempt, spec 042 rule 2's delivery is withdrawn, not kept as a
fallback. A witnessed attempt keeps spec 042's delivery, as section 5
resolves, and its record says which delivery it used.

## 4. Out of scope

- Providers other than Claude Code; a second adapter declares its own route
  (spec 047).
- Rotating or refreshing the operator's token (spec 042 section 4 keeps it out
  of scope).
- Composition with spec 013's wire witness: a later amendment (section 5).

## 5. Resolved decisions

**2026-10-21: the precondition is measured, unconfined.** Claude Code 2.1.307,
`claude -p` on macOS with a fresh `CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_OAUTH_TOKEN`
set to a sample token and `ANTHROPIC_BASE_URL` set to a loopback listener that
recorded header names only and answered 401. The listener received a
`HEAD /api/hello`, then `POST /v1/messages` twice, each with
`Authorization: Bearer` carrying exactly the sample token and
`anthropic-beta` including `oauth-2025-04-20`; no `x-api-key`. So an OAuth
token in print mode follows the base-URL override, and the gateway sees the
token it must swap. The same measurement under this product's confinement, on
macOS and Linux, is part of acceptance, and the gateway must answer the
`HEAD /api/hello` preflight.

**2026-10-31: a witnessed attempt keeps spec 042's delivery.** With both the
gateway and spec 013's witness, the child's traffic reaches the gateway in
plain HTTP and leaves it over TLS, so the two compose only if the gateway
routes through the sidecar and the capture sits between gateway and provider.
That composition is a later amendment to both specs; until it lands, a run
with `--witness` is delivered as 042 rule 2 delivers it, and its record says
so.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is declared by the
implementing change, which will scan the child's environment, its per-run
directory and every file the attempt leaves for the provider token's bytes,
replay a request with a revoked token and with another attempt's token and see
both refused, refuse a launch whose gateway cannot bind, and repeat the
section 5 measurement under confinement on macOS and Linux.
