---
id: "057-release-delivery"
title: "Releasing this repository's binaries is repository delivery: cargo-dist archives, installers, SBOM and build provenance"
status: approved
implementation: pending
created: "2026-10-21"
summary: >
  Specifies how this repository delivers its own binaries, under the F-02
  narrowing of 2026-10-20: a tag the owner pushes builds macOS and Linux
  archives for aarch64 and x86_64 with cargo-dist at an exact version, a shell
  installer and a Homebrew formula, a CycloneDX SBOM per archive that refuses
  to ship empty, and a keyless build-provenance attestation per archive. It
  adds no verb, and no verb in this corpus publishes. The binary's name, the
  Homebrew tap and the version scheme are the owner's decisions.
# Amends 006, whose section 4 puts installing and publishing the binary and a
# shell installer out of scope "deferred by F-02"; the 2026-10-20 F-02
# narrowing (spec 001 section 5, already recorded) moves that delivery here.
# 001 is only used, not amended.
amends:
  - "006-command-surface"
# No `extends`, and no claim on the release workflow or the dist
# configuration: they are unclaimed authority files under AGENTS.md (sections
# 2 and 5, O-4). The scripts the workflow runs are this spec's (section 5,
# 2026-11-01), under the `scripts/**` rule of AGENTS.md's ownership table.
establishes:
  - "scripts/release/check-config.sh"
  - "scripts/release/install-dist.sh"
  - "scripts/release/harden-installer.sh"
  - "scripts/release/verify-set.sh"
depends_on:
  - "001-boundaries-and-authority"
  - "006-command-surface"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "I-1"
    kind: invariant
    text: "No verb of this product publishes, releases or uploads anything; a release is started only by the owner pushing a signed version tag, and the workflow it starts is repository delivery, not product behaviour."
    anchor: "3-1-what-starts-a-release"
  - id: "R-1"
    kind: requirement
    text: "Every archive is built by cargo-dist at an exact version for aarch64 and x86_64 on macOS and Linux, from the tagged commit with the committed lockfile, and every action the workflow uses is pinned by commit digest."
    anchor: "3-2-the-archives"
  - id: "R-2"
    kind: requirement
    text: "Each archive carries a CycloneDX SBOM from the committed lockfile and a keyless build-provenance attestation naming it as subject; a release with an SBOM of zero components, or an archive without an attestation, is not published."
    anchor: "3-4-sbom-and-provenance"
  - id: "V-1"
    kind: verification
    text: "A dry run of the release workflow on a test tag produces four archives, a shell installer, a formula, four non-empty SBOMs and four attestations that verify with gh attestation verify, and publishes nothing."
    anchor: "verification"
    inputs:
      - "docs/decisions/00-founding-decisions.md"
---

# 057: Release delivery

## 1. Purpose

`F-02` deferred publication of every kind. On 2026-10-20 the owner narrowed
it (`docs/decisions/00-founding-decisions.md`, section 5, "F-02 narrowed to
product verbs"): distributing this repository's own binaries is repository
delivery, specified in its own spec and performed by the owner's release
process, while every product verb that publishes stays deferred and `F-03`
(signing and key custody for this product's records) is unchanged. Today the
only way to run this product is to build it. This spec is the delivery the
narrowing allows and nothing more.

## 2. Territory

A release workflow under `.github/workflows/`, the cargo-dist configuration
in the workspace manifest, and the shell installer and Homebrew formula
cargo-dist generates. The workflow is this repository's own, not a profile
file: no target releases this product. No crate changes and no verb is added.

This spec claims none of those files. `AGENTS.md`'s source-ownership table
keeps `.github/workflows/**` and the root `Cargo.toml` claimed by no spec,
because a spec that owned them would own the rules it is judged by
(constitution VII), so each lands as its own authority change, decided by the
owner. Section 5 resolves O-4: the table holds.

## 3. Behavior

### 3.1 What starts a release

The owner pushes a signed tag `v<version>` on a commit of `main`. The workflow
runs on that tag only. Nothing in the product, no CI job on a branch and no
merge starts it, and a tag whose commit is not on `main` or whose signature
does not verify is refused before any build.

### 3.2 The archives

cargo-dist at an exact version builds `statecraft-cli` for
`aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu` and
`x86_64-unknown-linux-gnu`, with `--locked`, from the tagged commit, each
archive with a SHA-256 sidecar. Every `uses:` in the workflow is pinned by
commit digest, which spec 044's action-pin verifier (on main since pull
request 296; ratification in pull request 306) would refuse otherwise.

### 3.3 Installers

- A shell installer that downloads the archive for the host, checks its
  SHA-256 and installs with `install -m 755`, never `cp` (the macOS
  code-signature cache; the `make install-local` target of pull request 305
  records the same reason).
- A Homebrew formula, published to a tap once the owner names it (section 5,
  O-2).

### 3.4 SBOM and provenance

Each archive gets a CycloneDX SBOM generated from the committed `Cargo.lock`,
and the release refuses to publish an SBOM with zero components, as
spec-spine's release workflow does. Each archive gets a keyless
build-provenance attestation (GitHub's `attest-build-provenance`) with the
archive as subject. This is the CI provider's attestation of how an artifact
was built; it is not a key this product holds and changes nothing `F-03`
defers.

### 3.5 What is not published

No crate is published to crates.io (`publish = false` stays), no package to
any other registry, and nothing is pushed back to the repository.

## 4. Out of scope

- Any product verb that publishes (`F-02` as narrowed).
- Signing this product's records, key custody, trust-root enrollment (`F-03`).
- Windows targets.

## 5. Resolved decisions

**2026-10-21: the workflow is the repository's, not the profile's.** The
profile renders what every target runs; only this repository ships this
binary.

**2026-10-31: the four questions, resolved within row `AG`.** Row `AG` of the
founding decisions keeps the binary's name, trust and publication with the
owner; each answer below stays inside it. O-1: the installed binary is
`statecraft-cli`, as built and installed today. O-2: the formula is generated
with every release but published to no tap until the owner names its
repository; the archives and the shell installer are the release until then.
O-3: tags are `vMAJOR.MINOR.PATCH`, the first is `v0.1.0`, the release commit
sets the workspace version to the tag's, and the workflow refuses a tag whose
version the manifest does not carry. O-4: the source-ownership table holds;
the release workflow and the dist configuration stay unclaimed authority files,
each landed as its own change the owner decides, and this spec claims none.

**2026-11-01: the scripts the release workflow runs are this spec's.** The
workflow and the dist table stay unclaimed (O-4); the checks they run live in
`scripts/release/`, so the coupling gate holds them to this spec and each runs
offline under `--self-test`. `check-config.sh` judges the dist table and the
workflow from their text, and the workflow's admission job runs it on the
tagged tree before any build.

**2026-11-01: the V-1 test tag is `dry-run/vMAJOR.MINOR.PATCH`.** It runs the
same workflow under the same section 3.1 refusals (signed, annotated, on
`main`, the workspace's version), builds, attests and verifies the full set,
and skips only the publish job. The attestations it makes are V-1's evidence,
not a release.

**2026-11-01: cargo-dist 0.32.0, from its release archive.** The workflow
downloads the cargo-dist archive for the runner's host at the version
`cargo-dist-version` names and refuses it unless its SHA-256 equals the digest
the workflow pins. cargo-dist does not pass `--locked` to cargo, so each build
runs `cargo fetch --locked`, then builds offline, then fails on any change to
`Cargo.lock`. The workflow is written for this repository, not generated, so
the dist table sets `allow-dirty = ["ci"]`.

**2026-11-01: the generated installer is hardened, and fails closed on drift.**
cargo-dist 0.32.0's shell installer places the binary with `mv` and `chmod`
and skips the SHA-256 check when `sha256sum` is absent or no checksum is
embedded. `harden-installer.sh` rewrites those lines: the binary is written by
`install -m 755`, SHA-256 falls back to `shasum -a 256`, and a missing tool,
checksum or style refuses the install. Every rewrite must match exactly once,
so a template change under a newer cargo-dist stops the release.

**2026-11-01: the signature check is GitHub's tag verification.** The
admission job refuses a lightweight tag, a tag object that does not name the
built commit, and one whose `verification.verified` is not true, so the owner's
signing key must be registered on the tagger's GitHub account.

**2026-11-01: the SBOM is syft's CycloneDX over the committed lockfile.** It
uses `anchore/sbom-action`, as spec-spine's release does, and so lists every
package `Cargo.lock` names, not only the binary's dependency closure; each
archive gets its own copy under its own name.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is declared by the
implementing change, which will run the workflow against a test tag in dry-run
mode and find four archives with sidecars, a shell installer, a formula, four
SBOMs with components and four attestations that `gh attestation verify`
accepts, with nothing published.

The implementing change also declares the offline checks of the
configuration. Each line is one command; none needs the network, cargo or
cargo-dist. The dry run above stays the acceptance of V-1.

```verify:cli
scripts/release/check-config.sh
scripts/release/check-config.sh --self-test
scripts/release/harden-installer.sh --self-test
scripts/release/verify-set.sh --self-test
```
