---
id: "128-api-origin-guard"
title: "The origin guard: the daemon answers its own page and its own clients, and never serves a credential"
status: draft
created: "2026-09-11"
implementation: pending
risk: medium
depends_on:
  - "022-http-api-and-events"
  - "027-api-projects"
  - "121-candidate-and-receipt"
  - "031-journal-export"
establishes:
  - "members/src/orchestrator/api/origin-guard.ts"
  - "members/src/orchestrator/api/origin-guard.test.ts"
extends:
  # 022 owns the API directory: the guard runs first in the router, and the
  # envelope's error vocabulary gains `forbidden` (additive; no shape changes).
  - { spec: "022-http-api-and-events", unit: "members/src/orchestrator/api/server.ts", nature: additive }
  - { spec: "022-http-api-and-events", unit: "members/src/orchestrator/api/types.ts", nature: additive }
  - { spec: "022-http-api-and-events", unit: "members/src/orchestrator/api/server.test.ts", nature: additive }
  # 121 owns the candidate, where the origin URL is read before it is
  # journaled in a receipt or served as a project's origin.
  - { spec: "121-candidate-and-receipt", unit: "members/src/orchestrator/candidate.ts", nature: additive }
  - { spec: "121-candidate-and-receipt", unit: "members/src/orchestrator/candidate.test.ts", nature: additive }
  # 031 owns the export policy; the value scan learns credential-shaped URLs
  # and the policy moves to version 6.
  - { spec: "031-journal-export", unit: "members/src/orchestrator/export.ts", nature: additive }
  - { spec: "031-journal-export", unit: "members/src/orchestrator/export.test.ts", nature: additive }
  # 113 owns the Rust journal crate, which carries the policy's second copy;
  # parity is byte-for-byte (039 FR-003), so the bump lands on both or neither.
  - { spec: "113-journal-port", unit: { kind: directory, path: "crates/statecraft-journal/" }, nature: additive }
  # Doc 05 is the record this spec is born from (D58, D59).
  - { spec: "110-corpus-merge", unit: { kind: directory, path: "docs/design/" }, nature: additive }
references:
  - { unit: { kind: file, path: "docs/design/05-the-realignment-checked.md" }, role: context }
summary: >
  The daemon binds loopback because it has no auth layer (022), and loopback
  keeps other machines out. It does not keep out a web page open in the
  operator's own browser: the API checks neither Origin nor Host, the version
  header is optional, and a text/plain body is parsed as JSON, so any page can
  send a preflight-free POST that arms a project, changes its posture or gate,
  or approves a human gate, and a DNS-rebinding page can read every answer.
  Measured on 2026-09-11, a foreign-origin POST disarmed a fixture project. This
  spec refuses, before any route runs, a request whose Host is not a loopback
  name at the bound port or whose Origin is not the daemon's own, and requires
  on every state-changing request a header a cross-origin page cannot send
  without a preflight the server never grants. It also stops a credential in
  the origin remote URL from being journaled, served or exported, which was
  measured passing through a minted receipt and the export policy.
---

# 128: The origin guard

## 1. Purpose

Spec 022 made loopback the trust boundary: "no auth in v1 (loopback trust),
designed so an auth layer slots in front without shape changes." The
reasoning holds for processes. It does not hold for a browser, because a
browser on the operator's machine will send a request to `127.0.0.1` on
behalf of any page the operator has open.

Three facts in `server.ts` make that request effective:

- nothing reads `Origin` or `Host`;
- `X-Api-Version` is checked only when present (`versionMismatch`);
- `readJsonBody` parses the body whatever its `Content-Type`.

A POST with a `text/plain` body and no custom header is a CORS "simple
request", which a browser sends without asking the server first. The page
cannot read the answer; the effect still happens. Measured on 2026-09-11
against a real server built from the API test fixtures: a POST to
`/api/projects/alpha/disarm` with `Origin: https://attacker.example` and
`Content-Type: text/plain` returned 200 and journaled `project.disarmed`. A
GET with a foreign `Host`, the shape a DNS-rebinding page produces once its
name resolves to `127.0.0.1`, returned 200, and a rebinding page is
same-origin with itself, so it reads what it gets.

Every control 027 and 021 expose is reachable that way: registering a path,
arming, the execution profile, the gate contract, the lifecycle policy, the
cost ceiling, `approve` and `force-human-gate`. Some browsers now ask before
letting a public page reach a local address. That is a browser's policy, not
this server's, and the daemon is not allowed to depend on it.

The second half is smaller and was found the same day. `originUrl` returns
`git remote get-url origin` verbatim, and a remote of the common CI shape
`https://x-access-token:<token>@github.com/...` puts the token into every
receipt's `repo.origin` and into the API's project view. The export passes it
(measured: `withheldFields: []`), because `origin` is not a stripped field and
the value is not a path. Worse, the handoff capsule (123) renders the same
string into every remediation prompt (`stages/build.ts:1122-1126`,
`handoff.ts:172`), so the token is sent to the model provider. A journal is
hash-chained, so a token that reaches one stays there.

This spec is not an auth layer, and it is not a step toward binding anywhere
but loopback. It makes loopback trust mean what 022 meant by it.

## 2. Territory

- `members/src/orchestrator/api/origin-guard.ts`: the admission check, a pure
  function of the request's method and headers and the server's bound
  address.
- `members/src/orchestrator/api/origin-guard.test.ts`: its table.
- `members/src/orchestrator/api/server.ts` (extends 022): the guard runs first
  in `route`, before the version check, the `authorize` seam and every route,
  static assets included.
- `members/src/orchestrator/api/types.ts` (extends 022): `forbidden` joins the
  error kinds, answered as 403.
- `members/src/orchestrator/candidate.ts` (extends 121): `originUrl` returns the
  URL without userinfo.
- `members/src/orchestrator/export.ts` (extends 031) and the Rust journal crate
  (extends 113): the value scan withholds a URL that carries userinfo, and the
  policy moves to version 6 on both sides.

## 3. Behavior

### B-1. Host names the daemon

The guard admits a request only when its `Host` header is one of
`127.0.0.1:<port>`, `localhost:<port>` or `[::1]:<port>`, where `<port>` is the
port the server actually bound. A missing `Host` is refused. This is the whole
defense against DNS rebinding: the rebinding page's requests carry its own
name, and no name but a loopback one is served.

### B-2. Origin, when present, is the daemon's own

A request that carries `Origin` is admitted only when the value is
`http://127.0.0.1:<port>`, `http://localhost:<port>` or `http://[::1]:<port>`.
`Origin: null` (a sandboxed frame, a `file:` page) is refused. A request with
no `Origin` is admitted: the CLI's client and `curl` send none, and a browser
sends one on every cross-origin request that could change state.

### B-3. A state-changing request carries a header a page cannot forge quietly

Every method other than `GET` and `HEAD` must carry `X-Api-Version`. Both
first-party clients already send it on every request (`api-client.ts`, which
the web UI reuses). A custom header makes any cross-origin browser request a
preflighted one, and B-4 guarantees the preflight fails. This is defense in
depth behind B-2, for a client that omits `Origin`.

### B-4. The server never grants cross-origin access

No response carries an `Access-Control-Allow-*` header, and an `OPTIONS`
request is answered by the guard with `forbidden` rather than routed. The
server serves exactly one origin, which is 024's same-origin design as
`static.ts` states it: "one origin, no CORS, no second port".

### B-5. A refusal is the envelope, and says which rule

A refused request is answered `{ ok: false, error: { kind: "forbidden",
message } }` with status 403, the message naming the rule (`host`, `origin`,
`header` or `preflight`) and the value it saw. Refusals are not journaled: a
hostile page could otherwise write to the journal at will. They are counted,
and `/api/meta` reports the count since start.

### B-6. The origin URL loses its userinfo before anyone reads it

`originUrl` parses an `http` or `https` remote and returns it with the whole
userinfo removed (user and password alike, because a token can sit in either),
so a receipt, the capsule (and with it every remediation prompt) and the API
all carry `https://github.com/...`. An `ssh://` or scp-style remote
(`git@github.com:org/repo.git`) and a local path are returned unchanged: an
SSH user part is not a secret, and 031 already withholds a private path on
export. A remote that names `http` or `https` but does not parse as a URL is
returned as `null`, never guessed.

### B-7. The export withholds a credential-shaped URL it did not mint

The export's value scan treats a string that parses as a URL with userinfo the
way it treats a private path: the field holding it is stripped and named in
`withheldFields`. A receipt minted before B-6, whose bytes stay in the journal
unchanged, therefore exports as redacted rather than leaking. The redaction
policy moves to version 6 on the TypeScript and Rust sides in the same change,
and a version-6 bundle verifies under both.

## 4. Functional requirements

- **FR-001.** Guard table (`origin-guard.test.ts`): each B-1 to B-4 rule
  admits and refuses the cases named there, at a port other than the default,
  for IPv4, `localhost` and IPv6.
- **FR-002.** Over real HTTP (`server.test.ts`): a foreign-origin
  `text/plain` POST to a control route is refused with 403 and journals
  nothing; a foreign `Host` GET is refused; the same POST with no `Origin` and
  with `X-Api-Version` is applied; the typed client passes every existing
  route test unchanged.
- **FR-003.** No response in the server suite carries an
  `Access-Control-Allow-*` header.
- **FR-004.** `/api/meta` reports the refusal count, and a refusal appends no
  journal record.
- **FR-005.** `originUrl` over a temporary repository: an `https` remote with a
  token user, with `user:password`, and without userinfo all return the bare
  URL; an scp-style remote is unchanged.
- **FR-006.** Export: a receipt payload whose `repo.origin` carries userinfo
  exports with `repo` named in `withheldFields` and without the token anywhere
  in the serialized bundle; the TypeScript and Rust exports of the same chains
  are byte-identical at policy version 6.

## 5. Acceptance

- `bun test` in `members/` is green; `cargo test -p statecraft-journal` is
  green; `make gate` exits 0.
- A live round in a real browser: the daemon's own UI loads and a control
  applied from it is journaled, and a page served from another loopback port
  that posts to the same control is refused, with the refusal count on
  `/api/meta` incremented and no journal record.
- The committed evidence bundle and every bundle 132 fixes still verify.

## Verification

```sh
cd members && bun test src/orchestrator/api/origin-guard.test.ts
cd members && bun test src/orchestrator/api/server.test.ts
cd members && bun test src/orchestrator/candidate.test.ts
cd members && bun test src/orchestrator/export.test.ts
cargo test -p statecraft-journal
make gate
```

## 6. Out of scope

- **An auth layer.** 022's `authorize` seam stays unwired. The guard answers
  "which page and which name", not "which person".
- **Binding anything but loopback.** Unchanged, and this spec is no argument
  for changing it.
- **What the API serves to a local reader.** Gate output tails and session
  evidence are served as they are to same-origin clients, which after this
  spec means local processes and the daemon's own page.
- **The umbrella's token paste.** `statecraft login` echoes a pasted token on
  a terminal, a deliberate v1 deferral recorded in `auth.rs`; doc 05 D72
  replaces the paste altogether.
- **Rewriting a journal.** A credential already chained stays in its record;
  B-7 keeps it out of every export instead.

## 7. Resolved decisions

D-1 (2026-09-11). Refuse by name and port, not by resolving. B-1 compares the
`Host` header's text with the three loopback names at the bound port, and never
resolves a name. Resolution is what a rebinding attack controls, so it cannot
be part of the defense.

D-2 (2026-09-11). `X-Api-Version` on every state-changing request, not
`Content-Type: application/json`. Both make a cross-origin request preflighted,
but the first-party client sends no body, and therefore no content type, on
controls such as `pause`; it sends the version header on every request. The
cost is that a raw `curl -X POST` now needs `-H 'X-Api-Version: 2'`, and the
refusal message says so.

D-3 (2026-09-11). `forbidden` is a new kind, not `bad-request`. The envelope's
`kind` is the token a client branches on (022 B-2), and "this request was
well-formed and is not allowed from where it came" is a different branch from
"this request was malformed". The addition is additive: no existing kind
changes meaning.

D-4 (2026-09-11). Refusals are counted, not journaled. A journal record per
refusal would hand the thing being refused a way to write to the journal.

D-5 (2026-09-11). Userinfo is dropped whole, not just the password. GitHub
accepts a token as the user part of an `https` URL with no password at all.

## Status (2026-09-11)

Authored `draft`, `implementation: pending`, from doc 05 §3 F1 and F3. The
measurements it rests on were taken with throwaway probes against the API test
fixtures and a temporary repository; nothing touched the operator's running
daemon or a real remote. Approval is a human flip.
