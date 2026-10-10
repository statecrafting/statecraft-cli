---
id: "038-hosted-client"
title: "The hosted client: device-grant login, submit and decision against one Statecraft cell"
status: draft
implementation: pending
created: "2026-10-10"
summary: >
  Amends 006 with the CLI's half of the hosted thin path that the Statecraft
  platform's spec 005 section 3.2 names and its 004 B-3 leaves open. The
  operator logs in with the OAuth device authorization grant (RFC 8628)
  against the cell's own issuer, as the public native client that the cell's
  manifest declares. `hosted submit` registers the revision this product
  accepted locally and submits its receipt and artifact bytes idempotently.
  `hosted decision` retrieves the revision's decision bundle and verifies it
  offline against a root set the operator pinned independently, never against
  the bundle's own. Tokens never reach a log, a receipt, a repository or a
  JSON answer. Nothing here dispatches work, records a reviewer's decision, or
  acts on an approval.
amends:
  - "006-command-surface"
# No `extends` yet. The implementing change adds the two edges it needs, on
# crates/statecraft-cli/ (006) and crates/statecraft-envelope/ (005). The gate
# counts an extender as an owner, so a draft that extends a directory refuses
# every pull request touching it until the draft is ratified.
depends_on:
  - "001-boundaries-and-authority"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "007-family-envelope"
  - "035-offline-issuer-contract"
  - "036-offline-issuer-verbs"
---

# 038: The hosted client

## 1. Purpose

The Statecraft platform's recovery record of 2026-10-03
(`statecraft/docs/design/06-project-recovery-and-blockers.md`, the row for its
004 B-3 and 005 section 3.2) names the blocker this spec answers: "the latest
local Statecraft CLI still has no hosted login/submission/retrieval verbs.
Rahi grant support alone does not implement this consumer path." The work it
asks for is "the CLI consumer seam against the released native-client/device
grants". Direct authenticated HTTP can exercise the cell, but it cannot prove
the specified CLI thin path.

On 2026-10-10 the owner chose that row as the decision basis for this draft
(decision 2). This draft therefore proposes lifting `F-01`, the deferral of
any hosted control plane, for exactly the three acts below, against exactly
one cell that the operator names. It lifts nothing else in `F-01` and nothing
in `F-02` or `F-03`; section 5 lists what ratification must still settle. A
draft authorizes no implementation.

The server half exists. Rahi 038 (`approved`, `implementation: complete`)
supplies the public native client, the device grant through the cell's
proxy, the CSRF exemption for a bearer write, token lifetimes and
revocation. Statecraft's 004 B-3 correction of 2026-09-17 puts it plainly:
before Rahi 038, "a token can be checked and cannot be obtained". This spec is
the half that obtains, holds and presents a token.

## 2. Territory

At draft this spec owns no code and declares no `extends` edge, because the
gate counts an extender as an owner and would refuse every change to a draft's
extended directory. The implementing change adds the edges with the code. It
reaches spec 006's binary to add the `hosted` verb
group, and spec 005's envelope crate to add the offline decision-bundle reader
(section 3.6). The proposed implementation adds one crate,
`crates/statecraft-hosted/`, the only crate allowed to speak HTTP to a cell,
in the way `statecraft-adapter-claude-code` is the only crate allowed to name
a provider. The implementing change claims it in this spec's `establishes`,
in the same change that writes it, so the gate never sees a forward claim.

## 3. Behavior

### 3.1 The cell, and what this product learns from it

1. **One named cell.** The operator names a cell by its origin
   (`https://<host>`, no path, no userinfo). Every hosted verb takes the
   origin from the project's committed declaration or from `--cell <origin>`.
   The origin is never inferred from a Git remote, an environment variable or
   a response.
2. **Discovery is the cell's.** The client reads
   `<origin>/.well-known/oauth-protected-resource` (RFC 9728), and then the
   authorization server's discovery document. The issuer must equal
   `<origin>/auth/v1/`, trailing slash included. A different issuer, or a
   resource whose identifier is not the origin, refuses before any grant
   starts.
3. **The client id and the scopes are declared, never negotiated.** The cell's
   manifest declares the public native client under `[[auth.native_clients]]`
   and the scopes its bearer routes require (Rahi consumer contract section
   8, item 2). The CLI presents that client id and requests exactly those
   scopes, no wider. Where they come from on the CLI's side is open question
   O-1. Until it is settled, the implementation reads both from the operator's
   declaration and refuses when either is absent.

### 3.2 Login: the device authorization grant

1. `hosted login` posts to the device authorization endpoint that discovery
   names, prints the `verification_uri` (or `verification_uri_complete`) and
   the `user_code`, and polls the token endpoint at the `interval` the
   response carries. It adds 5 seconds on `slow_down` and stops at
   `expires_in`. Rauthy's defaults are a 5-second interval and a 300-second
   code; the client never hard-codes either.
2. `access_denied` and `expired_token` end the login with a refusal (exit 2)
   that names which one. Neither is retried.
3. A token response is accepted only when its `token_type` is `Bearer` and
   its access token is a JWT whose header names `RS256` and whose `aud`
   contains the origin. The cell validates every token it receives; these
   checks only keep this product from storing a token the cell will refuse.
   The product reads the token's claims, never verifies its signature, and
   asserts nothing about it beyond the two checks.
4. Login performs no other request and writes nothing into any repository.

### 3.3 Holding a credential

1. **Where.** The credential is held under the product home, keyed by a
   digest of the origin, readable by the owner only (mode `0600`), and never
   inside a repository or a worktree. Open question O-2 asks whether the
   platform keychain replaces the file on macOS.
2. **Never disclosed.** No access token, refresh token or device code appears
   in stdout, stderr, a `--json` answer, a log, a run record, a receipt, an
   envelope, an evidence bundle or a commit message. A test scans every
   output of every hosted verb for each token's bytes.
3. **Late renewal only.** The access token is renewed with the
   `refresh_token` grant at the issuer's token endpoint, on the `expires_in`
   that the token response carried, and never on a number read from the cell.
   Rauthy sets a refresh token's `nbf` to `issued_at + lifetime - 60`.
   Presenting it earlier invalidates the refresh token and every linked
   session (Rahi 038 D-13). The client therefore refuses locally to present a
   refresh token before that instant. It reports "log in again, or wait until
   <instant>" rather than risk logging the person out.
4. **Logout revokes first.** `hosted logout` posts the access token to
   `<origin>/session/token/revoke`, then deletes the local credential. Its
   answer states whether the cell confirmed the revocation. When the cell
   cannot be reached, the answer says the token stays valid until its `exp`
   plus 60 seconds, and the credential is deleted anyway.

### 3.4 Request rules every hosted verb follows

1. A request carries `Authorization: Bearer <access token>` and never a
   cookie. It carries no CSRF pair: a bearer write on a declared bearer route
   is exempt (Rahi 038 B-1), and a pair of the client's choosing is not a
   mechanism the chassis supports (038 D-2).
2. A `401` with `WWW-Authenticate: Bearer resource_metadata=...` is the
   bootstrap. The verb refuses with "log in to <origin>" (exit 2) and does
   not retry. A `403` with `insufficient_scope` is final and names the scope.
3. No verb retries a `4xx`. A transport failure or a `5xx` exits 4 (failed),
   naming the request by method and route template, never by a URL that
   carries a credential.
4. Every hosted answer conforms to spec 007's family envelope. The routes,
   identifiers, attestation ids and states are data. A token is never part
   of the answer.

### 3.5 `hosted submit`: register the revision and submit its evidence

1. **Precondition.** The revision is one this product accepted locally: a
   receipt (spec 005 section 3.4) exists for the named commit in the current
   project. A revision without a receipt refuses (exit 2), and nothing is
   sent.
2. **Plan first.** `hosted submit --plan` prints, with no network request,
   every request it would make: the repository id, the commit, tree and base
   it would register, and each artifact's declared reference, digest and
   length. Without `--plan`, the verb runs only on the operator's explicit
   invocation. No other verb submits, and acceptance never submits on its own.
3. **Register.** It posts `{ commit, tree, base }` to
   `POST /api/repositories/{id}/revisions`. Registering a commit that is
   already registered returns the existing `RevisionId` (platform 005 B-1).
   The client treats that as success and records which of the two it was.
4. **Submit.** It posts the receipt bytes and each artifact's bytes, with a
   submission manifest listing each artifact's declared reference, digest and
   length, to `POST /api/evidence/submissions` (platform 003 B-21). The
   `Idempotency-Key` is a digest over the repository id, the `RevisionId` and
   the manifest bytes. A repeat of the same submission returns the first
   outcome unchanged, a refusal included (003 B-22). A body that differs
   under the same key cannot arise from this client.
5. **Answer.** The answer names every attestation id, every verdict's four
   dimensions shown separately, and the admission result, as the platform
   returned them. The submission is recorded in the project's runtime state,
   which is not committed: the cell, the `RevisionId`, the idempotency key
   and the response digest. It is not recorded in the receipt, which the
   submission does not change. Exit 0 means the platform accepted the
   submission. It says nothing about admission, which `hosted decision`
   reports.

### 3.6 `hosted decision`: retrieve and verify offline

1. **Retrieve.** `GET /api/repositories/{id}/revisions/{revisionId}/decision`
   (platform 005 B-4). A `404` is reported as "not registered" (exit 2). A
   registered revision with no decision is reported with the word `none`
   (platform 005 B-6), and `none` never reads as success.
2. **Verify against an independent root set.** The bundle is verified against
   the operator's pinned `RootSetV2`, read through spec 035's reader with its
   version and digest pin (spec 036 holds the operator's copy). It is never
   verified against a root set carried by the bundle, the cell or a
   repository. A bundle whose `rootSet` digest is not the pinned one is a
   finding that names both digests.
3. **The reader is clean-room and pure.** It is added to the envelope crate,
   which stays free of file, clock, entropy and network access, and it is
   implemented from the platform's format description (005 B-4 and B-5). The
   platform's AGPL bundle code is never copied (statecraft register `P-03`).
   For each entry it recomputes the hash and the signature, resolves every
   claim, distinguishes tombstoned objects from missing ones, re-evaluates
   admission under the pinned policy where it can, and reports agreement or
   the first divergence.
4. **Exit.** Exit 0 only when the bundle verifies and the reproduced
   admission is a pass. A verified bundle whose admission is `none` or not a
   pass exits 1 (finding), with each decision and its reviewer named.
   Divergence, a root-set mismatch and an admission the reader cannot
   reproduce also exit 1, and each says which. The platform's answer is
   never taken on trust.

### 3.7 Observable negative cases

| Case | Required result |
|---|---|
| Discovery names another issuer | Refused before any grant; nothing stored. |
| Device code expires, or the person denies it | Exit 2 naming `expired_token` or `access_denied`; no retry. |
| Token is not RS256, or its `aud` lacks the origin | Not stored; exit 2. |
| A refresh would be early | Refused locally; the refresh token is never presented. |
| No credential, or `401` with resource metadata | Exit 2: "log in to <origin>"; no retry. |
| `403 insufficient_scope` | Exit 2 naming the scope; final. |
| Submit without a local receipt | Exit 2; no request sent. |
| The same submission repeated | The first outcome, unchanged. |
| Decision for an unregistered revision | Exit 2: not registered. |
| A registered revision with no decision | Exit 1; reported as `none`. |
| Bundle signed under a root set other than the pinned one | Exit 1, naming both digests. |
| Bundle entry hash or signature does not recompute | Exit 1, naming the first divergent entry. |
| Any output of any hosted verb | Contains no token bytes. |
| Operator asks to approve, merge or dispatch | Usage refusal: no such verb. |

## 4. Out of scope

- Recording a reviewer's decision. `POST .../decisions` is a browser-session
  act for a person with the `reviewer` role (platform 005 section 3.2, step
  4), and this product does not send it.
- Repository registration and policy changes, which are an admin's acts
  (platform 005 B-1, B-2).
- Hosted dispatch, runners, leases and service principals (platform `P-08`;
  Rahi 038 D-3). A runner identity is not settled here.
- Offline authoring, local ledgers and sync (statecraft amendment `A-01`).
- Any effect that follows from an approval: push, merge, comment, deploy or
  permit (platform 005 B-8; this corpus's `F-02`).
- Signing by this product. The CLI submits unsigned evidence as before
  (`F-03`). The platform's issuer signs its own entries.
- More than one cell per project, cell discovery, and federation.

## 5. Open questions for ratification

None of these is resolved by this draft. Each needs the owner's answer before
or at ratification.

- **O-1. The client id and the scopes on the CLI's side.** The cell's manifest
  declares them, and the statecraft cell declares no native client yet. The
  candidates are a project-committed declaration, an operator-held setting in
  the product home, or values published by the cell in its resource metadata
  (`scopes_supported`), with the client id still declared. Statecraft and the
  CLI jointly own this answer (Rahi consumer contract section 8,
  "Unresolved").
- **O-2. Credential storage.** A `0600` file under the product home, or the
  macOS Keychain with the file elsewhere. The Keychain matches how Claude Code
  holds its credential on this machine, at the cost of a platform branch.
- **O-3. `F-02` and submission.** `F-02` says that no verb in this corpus
  publishes, and it lists push, pull request, merge, release and deploy. This
  draft reads submitting evidence to the operator's own cell as outside that
  list, because it changes no repository and no external system's state on
  the repository's behalf. Ratification should either confirm that reading
  in the decision record, or extend `F-02`'s list and keep `hosted submit`
  behind its own owner decision.
- **O-4. Lifting `F-01`.** Ratifying this spec records, in the founding
  decision record, that `F-01` is lifted for these three acts against one
  operator-named cell, citing decision 2. That entry belongs to spec 001's
  record and lands with ratification, not with this draft.
- **O-5. The crate.** Whether the HTTP client lives in a new
  `statecraft-hosted` crate, as section 2 proposes, or behind a feature in an
  existing one. It also asks which HTTP and TLS dependencies are admitted.

## Verification

Implementation acceptance is not run while this spec is a `draft` with
`implementation: pending`. Draft review uses the repository gate and the
frontmatter relationship report, which establish corpus consistency only.
