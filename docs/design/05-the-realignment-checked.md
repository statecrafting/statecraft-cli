# 05: The realignment, checked against the tree

The fifth design record. Doc 04 built the governed substrate (specs 119 to
125) and left five things open in its §11. On 2026-09-11 an external
realignment of the whole family arrived as three documents: a plan that
extends spec-spine into a compiler of authority snapshots, work scopes and
context closures; a register of 45 proposals (A01 to A10, B01 to B35), each
with a disposition; and a handoff packet addressed to this repository. The
packet asks this repository to finish local distribution and evidence, to
consume the records spec-spine is drafting, and to define its half of a
hosted connection.

This document checks each of the packet's claims against the tree at
`6f49d9a`, records what the check found that the packet did not, and turns
what survives into proposed decisions and draft specs. The method is doc
04's: one question per section, each decision numbered so a spec can cite
it, the evidence named. Decisions continue doc 04's numbering at D58.

**Status: proposed.** Unlike doc 04, nothing here is born approved. Every
decision below (D58 to D73) is a proposal until the owner ratifies this
document. The five specs it introduces (128 to 132) are born `draft`; the
two drafts it revises (126, 127) stay `draft`. The packet is evidence and a
recommendation. It amends no approved spec, and neither does this document.
Where the packet quotes an instruction from an earlier date (a September 10
baseline, a warning about uncommitted changes), that instruction is history,
checked below, and not a work order.

## 1. What the realignment asks of this repository

The realignment's ownership table gives this repository local policy
application, permit enforcement adapters, provider negotiation, candidates,
leases, event attribution, receipts, local explanations and the packaging
of an independent bundle verifier. It excludes two things explicitly:
claiming a protection a provider or the operating system cannot enforce, and
hosted tenant authority. Both exclusions are already this repository's
habits (120 D45's refusal of an unsupported required token; doc 04 D52's
"the hosted plane is already a broker"), so the realignment extends the
repository's direction rather than reversing it.

It names five records this repository would consume or produce. None exists
yet as an agreed contract:

| Record | Producer | State on 2026-09-11 |
|---|---|---|
| AuthoritySnapshot | spec-spine | spec-spine draft 087, uncommitted in that repository's working tree |
| WorkScope, ContextClosure | spec-spine | a design note in the same working tree, "proposed P1, not filed" |
| WorkPermit | a local operator or Statecraft | defined nowhere |
| ExecutionEvidence, AcceptanceReceipt | this repository | `acceptance.receipt` schema version 1 exists (121); nothing else |
| VerificationReport | a neutral verifier | defined nowhere; this repository's journal crate verifies bundles today |

So the consumption half of the realignment (scope, closure, permit) cannot
be specified to the field yet, and §8 records it as intent with the
constraints the packet fixed. The production half (receipts, fixtures, a
verifier, the explanation of a run) can, and §7 does.

## 2. The packet's findings, checked

Each claim read against the tree, the registry and a run of the suites it
cites.

| Packet claim | Verdict | Evidence |
|---|---|---|
| HEAD `6f49d9a`, registry and index fresh, tree clean | confirmed | `git status` clean; the session hook reports both fresh |
| 125 complete; 126 and 127 draft and pending | confirmed | `spec-spine registry list`: 71 specs, 68 approved (66 complete, 010 `n-a`, 000 with no implementation field), 100 superseded, 2 draft |
| Fence, receipt and broker suites: 26 tests, 133 assertions | confirmed | rerun: 26 pass, 0 fail, 133 `expect()`; the candidate, export, handoff, API server and static suites add 95 pass, 667 `expect()`. Fixtures and a local bare remote only; not live-provider or hosted evidence |
| The earlier baseline (`bf2f0fd`, 18 tests, uncommitted 109 edits) | superseded | 109 was ratified (#35) and kit 082 adopted (#38); nothing is uncommitted |
| Engine is Bun/TypeScript; Rust contracts, journal, sensors and both drivers exist; a full engine port is unresolved | confirmed | `crates/`; doc 02 D30; doc 04 §11 |
| The UI exposes standby, run, DAG, quota, economics, decisions and history | confirmed, with a gap | `members/web/src/views/`. No view shows a receipt, a broker action, a denial, a fence refusal, or a capability applied or degraded; the only verdict shown is the registry's project qualification (025) |
| Candidates, receipts, brokered publication, lifecycle policy, handoff capsules and capability negotiation exist | confirmed | 120 to 125 |
| The release packages only the umbrella | confirmed | `release.yml:71-82` builds and archives `statecraft` alone; `install.sh` mentions no member |
| Member CI builds members and the UI, but does not distribute them | confirmed | `members.yml:57-65`; 042 §6 and 108 §10 put installing and publishing members out of scope, for "a later spec" |
| UI asset lookup is source-relative | confirmed, and worse than stated | `static.ts:19-21` resolves from `import.meta.dir`. A compiled engine run outside the checkout answers `/` with 503 "The web UI has not been built", expecting assets at `/web/dist` (measured, §3 F4) |
| The daemon refuses a non-loopback bind because it has no auth layer | confirmed | `server.ts:93-105`; 022 "no auth in v1 (loopback trust)" |
| Login asks for a pasted browser session token | confirmed | `auth.rs:203-221`. Echo is left on deliberately (`auth.rs:198-201`, a recorded v1 deferral), so that part is a known limit, not a finding |
| Rahi supplies an audience-bound bearer contract | confirmed, with limits | rahi 025 is approved and complete: an RS256 access token whose `aud` must contain the resource URL, RFC 9728 discovery, a 15-minute maximum lifetime, a `jti` deny-list for revocation. It defines no refresh for a bearer client and no tenant binding, and the chassis implements no native flow itself (B-8 names rauthy's device grant) |
| A source receipt alone does not identify a deployed artifact | confirmed | `receipt.ts:63-74` binds a candidate sha. The release's SLSA provenance names the umbrella archive and is not linked to any receipt |

Two corrections to the packet's framing, both small. The packet says the
fence suite passed "today" with "fixture/local Git scope"; that remains the
whole of the evidence, and nothing in this document upgrades it. And it
lists 126 as proposing "a credential-focused deny-list"; that is accurate,
and 126 already says so in its own D-1 and §6. What 126 lacks is not honesty
about the deny-list but a stated threat model and the gate (§4, D61).

## 3. What the check found that the packet did not

- **F1. Any web page can drive the daemon.** The API checks neither `Origin`
  nor `Host`. `X-Api-Version` is optional (`server.ts:284-292`), and a body
  is parsed as JSON whatever its `Content-Type` (`server.ts:314-324`). A POST
  with a `text/plain` body and no custom header is a CORS "simple request",
  which a browser sends without a preflight; the page cannot read the answer,
  but the effect happens. Measured against a real server with the test
  fixtures: a POST to `/api/projects/alpha/disarm` carrying
  `Origin: https://attacker.example` and `Content-Type: text/plain` returned
  200 and journaled `project.disarmed`; a GET carrying a foreign `Host` (the
  DNS-rebinding shape) returned 200. Reachable this way: registering a path,
  arming, the execution profile, the gate contract, the lifecycle policy, the
  cost ceiling, `approve` and `force-human-gate`. Loopback binding keeps other
  machines out; it does not keep out a browser on the operator's own machine.
  Some browsers now gate public-to-local requests behind a permission, but
  that is a property of a browser, not of this server.
- **F2. The gate runs outside the fence.** `runGate` spawns each command with
  no `env` (`stages/build.ts:135-136, 296-303`), so the gate inherits the
  daemon's whole environment: the model keys, `GH_TOKEN`, `SSH_AUTH_SOCK`, the
  real `gh` on `PATH` and the operator's git credential helper. The gate runs
  the project's own commands (`make gate`, `cargo test`, `bun test`) in the
  candidate, and the candidate can edit every file those commands execute. So
  the fence (125) and the scrub (121 B-3) hold for the session and not for the
  code the session wrote. 125's claim is "the broker is the only path that
  works, so the journal is complete", and a test that pushes during the gate
  is an effect the journal never sees. 125 §6 lists three residuals; this is a
  fourth it does not list, and draft 126 as it stood at `6f49d9a` (B-7: "the
  sandbox wraps the member the driver spawns and nothing else") would have
  left it open as well; D61 revises that. spec-spine's request R7 names the
  same boundary from the other side.
- **F3. A credential in the remote URL is journaled and exported.**
  `originUrl` returns `git remote get-url origin` verbatim
  (`candidate.ts:143-146`). With an origin of the common CI shape
  `https://x-access-token:<token>@github.com/...`, a minted receipt carries the
  token in `repo.origin`, and the export policy passes it (measured:
  `withheldFields: []`), because `origin` is not a stripped field and the
  value is not a path. The API serves the same string as a project's
  `origin`, and the handoff capsule carries it into every remediation prompt
  (`stages/build.ts:1122-1126`, `handoff.ts:172`), which sends it to the model
  provider. A journal is hash-chained, so a token that reached one cannot be
  removed without breaking it.
- **F4. A packaged engine is not a working engine.** Built with
  `bun run build:member:engine` and run from a directory outside the checkout:
  `daemon run` serves the API, but `/` answers 503 with assets expected at
  `/web/dist`, and `daemon start` fails. Its re-spawn passes
  `join(PROJECT_DIR, "src/index.ts")` to `process.execPath`
  (`commands/orchestrator.ts:296-298`), which a compiled binary reads as an
  unknown command, so the child prints usage, the lock is never acquired, and
  the start exits 1 after 15 s. The default data directory has the same
  source-relative shape (`paths.ts:11-12`), as do the sensor's `daemon start`
  and its launchd plist (`commands/daemon.ts:38, 70-71`). 042 D-10 recorded the
  cause and deferred the fix to "the spec that makes a member binary the
  primary way its verbs are reached". No such spec exists.
- **F5. No original-bytes receipt exists to hand anyone.** The one committed
  bundle (`docs/evidence/journal-bundle.json`) is export policy version 1,
  predates 039's attestation block, and contains no `acceptance.receipt` and
  no `broker.action`. And what bundle verification establishes is internal
  consistency: `verifyBundle` checks each chain from the `anchorHash` the
  bundle declares for itself (`export.ts:625-674`), and `journal verify
  --bundle` reports `verified: true` (`commands/orchestrator.ts:1844`). A
  chain fabricated end to end, with a fresh anchor, verifies. That is correct
  for what the code claims, and it is exactly the "self-selected root" the
  packet warns a verifier against trusting. Separately, `parseReceipt`
  (`receipt.ts:136-179`) accepts a payload with `passing: true` beside a
  non-zero exit code and never recomputes the suite or policy digest; a
  minted receipt cannot have either defect, and a forged one can.
- **F6. The review note cannot be posted from a governed session.** Draft 127
  has the session post a pull request comment through the GitHub API. In a
  driven run that session is fenced (125): `gh` refuses and git has no
  credential. 127 B-5 makes a failed post non-fatal, so in every driven run the
  note would silently never appear, and if it did appear through some other
  credential it would be an effect with no receipt and no journal record,
  which is what 122 and 125 exist to prevent.
- **Two of spec-spine's requests describe this tree accurately, and a third
  only half does.** R1: the export and the adoption holdback match
  `attestationHash:` in prose (`export.ts:248`, `adopt/holdback.ts:325`). R5:
  `spec-spine.toml` pins `required_version = "0.18.0"` as a caret range and
  both workflows install from `main`'s `install.sh` unpinned. R6: the engine's
  floor (`gate-contract.ts:121-125`) does omit `--fail-on-unresolved`, but the
  request's premise that this repository's own floor carries it is wrong: the
  Makefile's gate omits it too, deliberately (spec 050's opt-in; the comment
  above `gate:` says to add it once every approved spec is implemented).

## 4. Local protections

- **D58 (proposed; the daemon answers only its own origin).** Before any
  route runs, the server refuses a request whose `Host` is not one of the
  loopback names at the bound port, and a request that carries an `Origin`
  other than the daemon's own. A state-changing request must also carry a
  header a cross-origin page cannot send without a preflight
  (`X-Api-Version`, which both first-party clients already send), and the
  server never answers a preflight with permission. A request with no
  `Origin` (the CLI's client, `curl`) is admitted as before; loopback remains
  the trust boundary for local processes, and nothing here is an auth layer
  or a reason to bind anywhere else. Spec 128.
- **D59 (proposed; a credential never leaves through the API, a receipt or an
  export).** The origin URL is reduced to scheme, host and path before it is
  journaled or served: userinfo in an `http` or `https` URL is dropped whole,
  because a token can sit in the user field as well as the password. The
  export policy treats a URL with userinfo as a value to strip, so a record
  minted before the fix exports as redacted rather than leaking, and its bytes
  in the journal are never rewritten. Spec 128.
- **D60 (proposed; the gate runs behind the fence).** Gate and bracket
  commands spawn with the same scrubbed, fenced environment the session
  receives, from the fence of the candidate they run in. A gate-time refusal
  is tallied apart from the session's, so a reader can tell the session
  reaching for `gh` from a test doing it. The broker, which publishes, keeps
  the daemon's environment (125 B-6). The verify stage, which runs a merged
  spec's declared acceptance after publication, is left as it is and recorded
  as a residual, because some declared acceptance is live by design. Spec 129,
  small enough to land before 126.
- **D61 (proposed; 126 is a credential deny-list and says so).** Draft 126
  gains an explicit threat model: a capable session, not a hostile one, and an
  integrity property (a complete journal), not confinement of source authority.
  Three corrections follow. Its `required` policy guarantees that the named
  deny set is in force, not that "containment is in force", and the text says
  exactly that. Its blast radius includes the gate (after 129, the gate is
  fenced; 126 adds the profile to the same spawn), because the gate executes
  candidate-authored code. And it claims no capability token: a read and
  exec deny-list confines no writes, so it neither lets the Claude driver
  claim `workspace-write` nor stands in for any future WorkScope enforcement.
  `sandboxMode` names the deny set it applied by digest, so "declared" and
  "enforced" are two fields, not one inference.

## 5. The review note

- **D62 (proposed; the session proposes a note, a publisher posts it).** The
  note keeps 127's one-marked-comment shape and its honesty rules, and changes
  who posts. In a driven run the ship session adds the review's findings and
  dispositions, as a `review` field, to the proposal it already writes to the
  drop box (122 D-6), and the broker posts the note as a fourth action,
  `note`, admitted on the same receipt and lease as `openPr` and journaled as
  intent and outcome with the body's digest. In an interactive session the
  operator's own credentials post it, as the first draft said. Either way the
  note has two sections that never mix: **machine results**, copied from the
  receipt the publication consumed and labelled `receipted` (or, where no
  receipt exists, labelled `reported by the session`), and **review
  findings**, the model's own account, labelled as narrative. It names the
  head sha and the receipt hash; a note whose head is not the pull request's
  head says it is stale.

## 6. Distribution

- **D63 (proposed; a complete install is one release).** A tag builds, beside
  the umbrella's five archives, a member archive per supported triple
  (macOS and Linux, both architectures) holding the engine, the native sensor
  and driver members, the built web assets, the provider qualification
  records, and a `members.json` manifest that names every file with its
  SHA-256, its member-contract version and the release tag. The archive carries the same checksum sidecar, SBOM and SLSA
  provenance as the umbrella's (107). `install.sh` installs it into the
  managed member directory (108 §4) when asked, verifies every digest before
  anything is moved into place, and replaces the previous set atomically.
  `statecraft members doctor` reports each expected member as present, missing,
  refused, digest-mismatched or shadowed, and each provider prerequisite (the `claude`
  and `codex` binaries, `spec-spine` at the required floor, `git`, `gh` for
  the broker) as found or absent, with its version and qualification record.
  Nothing installs a provider or signs in to one. Spec 130.
- **D64 (proposed; a packaged member finds its files at run time).** 042
  D-10's successor. The engine stops resolving locations from
  `import.meta.dir`. Its web assets and qualification records resolve from an
  explicit flag or variable first, then the directory beside the installed
  binary, then the checkout for the source path; a packaged engine's default
  daemon home sits under the platform data directory 108 §4 already uses,
  while the source checkout keeps its own so no operator's registry moves.
  `daemon start` re-spawns `process.execPath` with the verb arguments when
  compiled. Acceptance is a clean machine: a CI job that
  never checks the repository out installs from the release artifacts, starts
  the daemon, loads the UI and registers a repository. Spec 130.

## 7. Evidence

- **D65 (proposed; one view explains a change).** A read-only view, reached by
  a verb, a route and a UI panel, joins the records a run already writes into
  one account per spec: the spec's lifecycle, the candidate and its base,
  every round's gate commands and exit codes, the receipt with its suite and
  policy digests, `acceptance.unstable` and `acceptance.sensitive`, denials,
  fence refusals, the driver's capabilities applied, degraded and refused, the
  provider's qualification, approvals and forced gates, the broker's intents,
  outcomes and refusals, the merge sha and verify's result. Each value is
  labelled with its source record, and four labels are first-class: `none`
  (the run recorded that nothing happened), `not recorded` (no record of this
  kind exists for the run, including every field a future contract will add:
  scope, closure, permit), `stale` (a receipt whose candidate is no longer
  the branch head), and `unknown` (a fact no record can establish: provider
  tool-event coverage is shown this way, because neither driver proves it saw
  every call). A "declared versus enforced" section sets the profile's
  posture and required tokens beside what `session.init` says was applied,
  and a model's narrative is labelled apart from what a machine observed.
  Spec 131.
- **D66 (proposed; compatibility is original bytes plus a verdict manifest).**
  A fixture directory holds bytes minted by the real code paths: a journal and
  its export at policy version 5 containing `acceptance.receipt` version 1,
  `broker.action` intent and outcome, `broker.refused` and `fence.refused`,
  with and without an attestation; the committed policy-1 bundle stays as the
  oldest fixture. Beside them, the negative set: truncation, a tampered
  payload, a broken link, a reordered sequence, a withheld record that carries
  a payload, an unknown format, an unsupported version, malformed JSON, an
  attestation mismatch, a forged receipt (`passing: true` beside a red exit
  code, or a digest that does not recompute), a broker action naming a receipt
  the chain does not hold, and a re-anchored chain that is internally perfect.
  A manifest states each fixture's expected outcome per dimension, and both
  existing verifiers (the TypeScript `verifyBundle` and the Rust journal
  crate's `verify_bundle`) run it. JSON Schemas describe version 1 exactly as
  the code parses it. This is what Statecraft and hqgit receive, before either
  writes a parser. Spec 132.
- **D67 (proposed; a verification report has four outcomes).** Integrity (do
  the bytes and links recompute), subject binding (does the receipt name the
  revision and repository asked about), issuer trust (was it produced by a key
  or an anchor the reader trusts, given out of band), and policy (does it meet
  the reader's rules). Each is `pass`, `fail`, `unknown` or `not-applicable`,
  never folded into one boolean. A verifier never executes anything a bundle
  carries, and never accepts the bundle's own anchor as its trust root. Today
  every bundle's issuer trust is `unknown`, because nothing is signed; saying
  so is the point. Where the neutral verifier is packaged is an open decision
  (§16): the Apache-2.0 `statecraft-journal` crate already parses and verifies
  bundles and is the obvious seed, and no AGPL code may enter it.
- **D68 (proposed; receipt version 2 is additive).** A later receipt revision
  adds, without changing version 1's bytes or meaning: the merge base `couple`
  diffed from; per gate command, the verdict envelope's schema version and the
  SHA-256 of its bytes (spec-spine R2); an `authority` block that holds the
  corpus attestation hash and the spec's attestation until spec-spine 087
  provides a snapshot hash to replace them; and an `execution` block naming
  the driver, its binary version and qualification, the capabilities applied
  and degraded, the fence and sandbox modes, and the denial and refusal counts
  (these exist today in separate records). A reader of version 1 keeps reading
  version 1. Not specified until spec-spine's snapshot shape is approved.

## 8. Scope, context and permits: the consumer side

Intent and constraints, not specifications. Each waits on a record another
repository has not yet agreed.

- **D69 (proposed; what reached the provider is recorded).** Every session
  records what was actually submitted to it: the digest of the prompt, the
  capsule it carried (123 D55) by digest, the spec text by digest, and, once
  spec-spine files ContextClosure, the closure it was built from, with each
  required item marked included, omitted or truncated. Anything retrieved
  beyond the closure is listed separately as optional. A truncated or omitted
  required item invalidates any completeness claim and is shown, not hidden.
  What the provider retained, and whether it read it, is `unknown`.
- **D70 (proposed; a permit is enforced by the executor, not the prompt).**
  When a WorkPermit exists, the engine maps each protection it requires onto
  what can actually enforce it: a capability token (120), the fence (125), the
  sandbox profile (126), the gate fence (129). A required protection with no
  enforcer refuses before spawn, as 120 D45 already does for tokens. A scope
  that names symbols is checked after the change (the coupling verdict, or
  spec-spine 088's delta), because an OS rule fences a file and not a symbol.
  Mutable reservations stay in the engine and extend the existing lease (122
  D51); conflict sets come from spec-spine. Compiling a scope is not issuing a
  permit: the actor, run, audience, validity and fencing come from the local
  operator or from Statecraft.

## 9. The hosted seam: the client half

- **D71 (proposed; this repository states constraints, not the protocol).**
  Runner enrollment, the outbound connection, jobs, cancellation and fencing,
  evidence upload and hosted action authorization are Statecraft's to lead,
  and Statecraft's corpus defines none of them yet. This repository commits to
  what its side needs and to the negative cases a first wire contract must
  carry before either end is built (§14). It will not draft both ends.
- **D72 (proposed; login moves to an authorization flow, once agreed).** The
  token paste gives way to rauthy's device authorization grant (or, where a
  browser is at hand, authorization code with PKCE on a loopback redirect),
  requesting the plane's resource URL as audience and the scopes the plane
  advertises. A credential is stored per plane, issuer and audience, not per
  base URL alone, with its expiry; an expired token is renewed where the flow
  allows and otherwise prompts a fresh login, never retried blindly. A
  revoked token (the plane's 401) is discarded. This waits on Statecraft
  choosing its client registration and on rahi 025's silence about renewal
  being resolved.

## 10. The journey

- **D73 (proposed; one path from a repository to verifiable publication).**
  Install (130), register an existing repository, approve or name a spec,
  watch the run, open the explanation (131), export and verify (132), with no
  account. The engine's verbs flatten from `statecraft engine orchestrator
  <verb>` once a packaged member is the primary way they are reached, which is
  042 D-7's condition, and every old path stays as an alias for at least one
  minor release. Two supervisions stay two concepts in every name and screen:
  this repository supervises agents working a repository; a Rahi-hosted plane
  may supervise running applications. A command or view never mixes them.

## 11. Replay and shadow policy

Register B27 and B28 are later work (the register's P4). One constraint is
worth fixing now, because 132's frozen fixtures will be replay's first
inputs: a replay evaluates recorded inputs under a named policy and verifier
version, reports a missing fact as `unknown`, and never re-runs an effect or
guesses what another provider would have written. A shadow policy's verdicts
are recorded apart from enforcement and can authorize nothing.

## 12. Declared versus enforced, today

What a reader of this repository's evidence can rely on at `6f49d9a`.

| Protection | Declared where | Enforced by | Holds against |
|---|---|---|---|
| Publication only through the broker | 122, 125 | fence shims, git config, deny list | a capable session; not the gate (F2), not an absolute path, `HOME` reads or `~/.ssh` (125 §6) |
| Required capability tokens | 120 D45 | refusal before spawn | an unsupporting driver |
| Writes confined to the candidate | the `workspace-write` token | Codex's `--sandbox workspace-write`, under `guarded` only (`bypass` disables it); Claude does not claim the token | nothing, under Claude or a `bypass` posture |
| Candidate isolation | 121 D47 | a git worktree | accidental edits to the operator's checkout; it is not a security boundary |
| Acceptance | 121 D49 | a stable, passing gate over one revision | a moving head or dirty tree; not a candidate that edits its own gate (recorded as `acceptance.sensitive`, not refused) |
| Merge of the checked head | 119 D43 | GitHub's `sha` parameter | a push between check and merge |
| Control API reachable only locally | 022 | loopback bind | other machines; not a web page (F1) |
| Bundle integrity | 031, 039 | link and payload recomputation | edits to a bundle; not a fabricated bundle (F5) |
| Provider tool-event coverage | none | none | unknown |

## 13. Responses to spec-spine's requests

spec-spine's working tree carries seven requests to this repository (its
design note 04 §7). They are proposals from a draft; each is answered here so
that its authors can plan around the answer.

| Request | Response |
|---|---|
| R1: parse `attest --json`, not prose | accepted; a small change to 031/039's export and 036's holdback. Waits on spec-spine confirming the `--json` report shape it wants consumed |
| R2: receipt carries spec-spine envelopes, an authority block, the merge base | accepted as D68, after spec-spine 087 is approved |
| R3: read acceptance from `verify --plan --json` at the base revision | accepted in principle; the verify stage's browser path (019) keeps working because spec-spine reports `verify:browser` blocks as skipped. A later spec |
| R4: hash stored attestation bytes, refuse unknown members | the first half holds today (`export.ts:284`); the member check joins R1's change |
| R5: pin the spec-spine installer | accepted; a harness change under 109 when a spec-spine release carries 083 |
| R6: `--fail-on-unresolved` on the floor | deferred to the owner: it changes what every governed project must satisfy, and this repository's own Makefile omits it by spec 050's design, so the request's comparison does not hold (§3). Distinguishing exit codes 1, 2 and 3 in evidence is accepted with D68 |
| R7: acceptance only behind the fence | accepted for the build gate as D60 (spec 129); the verify stage stays a recorded residual |

## 14. Requests to other repositories

Proposals for each repository's own governance; none changes its contract
from here.

**Statecraft.** (1) Lead the first runner and evidence wire contract, and
include before either end is built these negative cases: a token for the
wrong audience, a token for another tenant, the same evidence uploaded twice,
a job whose lease has expired or been superseded (stale fencing token), a
runner reconnecting mid-job, a candidate sha that does not match the job's,
a receipt for a revision the job did not name, and an upload whose bundle
verifies but whose issuer is not trusted. (2) Say which client registration a
native CLI uses (a public client with device grant, or dynamic registration)
and what it may request as scopes. (3) State that the plane never executes a
spec's declared verification (spec-spine's S1). (4) Consume 132's fixtures
and schemas as the definition of receipt version 1 and bundle version 1.

**spec-spine.** (1) Commit and review drafts 085 to 088 and the design note;
this document cites them as uncommitted. (2) Name the JSON report fields a
consumer should read from `attest --json` (R1). (3) When ContextClosure is
filed, include the `omitted` and `truncated` states D69 needs.

**hqgit.** Reference receipts and bundles by the digests 132 defines rather
than defining a second receipt format; review D67's four outcomes against its
own verifier (hqgit 034, 064). hqgit is AGPL-3.0: this repository will not
depend on its code, and the neutral verifier stays Apache-2.0.

**Rahi.** Say whether rahi 025's 15-minute access token is renewed for a
bearer client, and how, since a CLI session outlives one token. Review D72
before Statecraft adopts it.

## 15. The spec plan

| Step | Spec | Lands | Depends on |
|---|---|---|---|
| The origin guard and credential hygiene | 128 | D58, D59 | 022, 027, 031, 121 |
| The gate fence | 129 | D60 | 016, 121, 125 |
| The sandboxed executor, revised | 126 | D61 | 121, 122, 125, 129 |
| The review note, revised | 127 | D62 | 017, 018, 109, 122 |
| Distribution | 130 | D63, D64 | 024, 042, 107, 108, 124 |
| The evidence view | 131 | D65 | 027, 120 to 122, 124, 125, 129 |
| Compatibility fixtures | 132 | D66, D67 (fixtures and manifest only) | 031, 039, 113, 121, 122, 125 |
| Receipt version 2 | later | D68 | spec-spine 087 |
| The context submission record | later | D69 | spec-spine ContextClosure |
| Permit enforcement | later | D70 | a permit format |
| The hosted client and login | later | D71, D72 | Statecraft's wire contract, rahi's renewal answer |

The recommended order is 128, 129, then 132 and 130 in either order, then
131 (which reads more once 129's tallies exist), then 126 and 127. 128 and 129
are small, verified and security-relevant; either can be approved on its own.

What the register's proposals assigned to this repository come to here:

| Register | Where it lands | State |
|---|---|---|
| B01 WorkPermits | D70 | waits on a permit format; compiling a scope is not issuing one |
| B02 OS enforcement | 126 (revised), 129 | drafts; a credential deny-list, not source-authority confinement |
| B03 Leases | D70, 122 D51 | the run lease exists; reservations extend it once conflict sets exist |
| B04, B05 Context closure | D69 | waits on spec-spine ContextClosure; omission and truncation are first-class |
| B06 Spec-addressable events | 131 | draft; every value names its record; attribution the records lack is `not recorded` |
| B07 Flight recorder | 131 | draft; narrative apart from machine outcomes; no invented complete trace |
| B17 Waiver half-life | none here | the waiver is a human instrument in a PR body (109) and no session writes one; a scoped, expiring waiver whose use is consumed atomically belongs to a broker or the plane, not an authored counter |
| B18 Constraint budgets | 033, unchanged | spend is a known-cost floor with unknown sessions counted beside it (033 B-5); a hard budget would need a reservation before a turn, which nothing proposes yet |
| B27 Replay, B28 Shadow policy | §11 | later; constraints fixed |
| B32 Audit bundles | 132 | draft; original bytes, explicit coverage, an input cap, no execution |
| B33 Proof protocol | §14, D67 | Statecraft leads the envelope; this repository supplies receipt and bundle version 1 |
| B34 Verifiers first | 132, D67 | draft fixtures and manifest; the package home is open (§16) |
| B35 Policy compiler, not runtime | D70, 132 B-4 | pure checks of explicit inputs; keys, clocks, leases and approvals stay in the engine or the plane |

## 16. Not decided here

- The neutral verifier's home: extend `statecraft-journal`, add a sibling
  crate in this workspace, or a separate repository. The realignment calls
  this workspace a proposed initial home, not a settled one.
- Which member builds ship: the Rust sensor and drivers (112, 114, 115, 116)
  or the TypeScript builds of the same names, which no spec has retired. 130
  D-1 recommends the Rust builds; approving 130 confirms or changes it.
- Whether web assets ship beside the engine (130 D-2's recommendation) or
  embedded in it.
- The first issuer and trust model for a signed receipt, and whether the
  daemon holds a signing key at all (126's brokered signing is a candidate).
- `--fail-on-unresolved` on every governed project's floor (R6).
- Whether a failure to post the review note in a driven run is a stage
  failure (127 says no; D62 keeps that).
- Doc 04 §11's remaining items: a custom agent loop, per-stage driver routing,
  and the engine port (doc 02 D30).

## 17. Sources

The handoff packet `04-statecraft-cli.md` and its index, the realignment plan
and the feature disposition register, all dated 2026-09-11. This repository
at `6f49d9a`: `members/src/orchestrator/{api/server.ts, api/static.ts,
api/types.ts, stages/build.ts, candidate.ts, receipt.ts, export.ts, broker.ts,
gate-contract.ts, adopt/holdback.ts}`, `members/src/{paths.ts,
commands/orchestrator.ts, commands/daemon.ts}`, `members/web/src/`,
`src/{auth,members}.rs`, `.github/workflows/{release,members,spec-spine}.yml`,
`install.sh`, `spec-spine.toml`, `docs/evidence/journal-bundle.json`, and
specs 022, 024, 042, 107, 108, 112, 114, 121, 122, 125, 126, 127. The
measurements in §3 were taken on this machine (macOS, Bun 1.3.11) with
throwaway probes against the test fixtures and a compiled engine in a scratch
directory; nothing touched a real remote or the operator's running daemon.
spec-spine's working tree at `75181a5` plus uncommitted drafts 085 to 088,
`docs/design/04-authority-evidence-extension.md` and
`docs/authority-evidence.md`. rahi specs 021, 022 and 025 at their approved
text. The statecraft and hqgit corpora, searched for runner, job, evidence,
receipt, permit and verifier contracts.
