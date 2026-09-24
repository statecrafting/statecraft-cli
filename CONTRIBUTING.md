# Contributing

This repository is governed by its specifications, and the rules for changing
it live in [AGENTS.md](AGENTS.md). Read that file first; this page only points
into it and does not restate its rules.

- **Where authority lives, and what a green gate means:** AGENTS.md, "Where
  authority lives", "The gate" and "What a green gate means". Run `make tools`,
  then `make gate` (the corpus) and `make code` (the workspace) before every
  commit; CI requires both through the `ci-gate` check.
- **One spec per pull request:** AGENTS.md, "Working the backlog". A change to
  a crate is claimed by, and coupled to, its one owning spec.
- **Authority changes are separate:** AGENTS.md, "Approval semantics". Policy,
  the check suite, hooks and acceptance instructions change in their own pull
  request, decided by the owner.
- **Authored-content rules:** AGENTS.md, "Authored-content rules", checked by
  `make gate`: no U+2014 in any authored file, commit message or pull-request
  body, no agent-session links or session-tracking trailers, and no agent
  attribution lines.
- **Security issues** go through private reporting, never a public issue: see
  [SECURITY.md](SECURITY.md).

Contributions are accepted under the repository's license (Apache-2.0, see
`LICENSE`).
