---
id: "006-command-surface"
title: "The command surface: one binary, the verbs the other specs name, and what an exit code means"
status: approved
implementation: complete
created: "2026-09-16"
summary: >
  The binary. Specs 002 to 005 each describe operator verbs and then own only a
  library crate, so nothing they specify is runnable. This spec owns
  crates/statecraft-cli/ and binds those verbs to a process: the command tree,
  the rule that a command is a thin binding and never a second implementation,
  the closed exit-code vocabulary that distinguishes a refusal from a failure
  from a finding, human and --json output as two renderings of one value, and
  the binary's name being a recorded decision rather than a Cargo default.
establishes:
  - { kind: directory, path: "crates/statecraft-cli/" }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
---

# 006: The command surface

## 1. Purpose

Spec 002 names `project register`, `env plan`, `env apply`, `env upgrade`,
`env remove` and `doctor`. Specs 003 to 005 name more. Every one of those specs
owns a library crate and stops there, which is correct for each of them and
leaves the product in a state where nothing it specifies can be run.

This spec exists so that gap is closed **once, deliberately, by a spec that owns
a binary**, rather than by whichever implementing change first finds the absence
inconvenient. A binary that appears as a side effect of a feature is a binary
nobody designed: its exit codes are whatever that feature needed, and the next
feature inherits them.

## 2. Territory

`crates/statecraft-cli/` (forward claim; unresolved until implemented).

Not this spec's territory: every behavior the commands expose. Those belong to
`002` to `005`, and section 3.2 is the rule that keeps them there.

## 3. Behavior

### 3.1 The command tree

The binary exposes the verbs its dependency specs name, grouped as those specs
group them:

| Command | Owning spec | What it does |
|---|---|---|
| `project register <path>` | `002` | Records a target and prints its verdict with reasons. |
| `project list` | `002` | Every registered target, its verdict and whether it is armed. |
| `project arm <path>` / `project disarm <path>` | `002` | Consent to being driven, separately from registration. |
| `env plan` | `002` | Prints what `env apply` would do. Writes nothing. |
| `env apply` | `002` | Performs the plan. |
| `env upgrade` | `002` | Re-plans against a newer product or adapter version. |
| `env remove` | `002` | Removes managed paths whose digest still matches. |
| `doctor` | `002` | Diagnoses. Repairs nothing. |

Verbs owned by `003`, `004` and `005` join this tree as each is implemented,
each by an `extends` edge from this spec naming that spec. A verb is added by
the change that implements the behavior behind it, never ahead of it: a command
that prints "not implemented" is a worse answer than a command that does not
exist, because only one of them is discoverable as absent.

### 3.2 A command is a binding, never a second implementation

A command parses arguments, calls exactly one library operation, renders the
value it returns, and maps it to an exit code. It contains no rule the owning
spec did not state.

This is enforceable rather than aspirational: `crates/statecraft-cli/` is
claimed by this spec, and the coupling gate refuses a change to it that does not
edit this spec. A rule that leaked into a command would therefore have to be
written down here, where it visibly does not belong, instead of accumulating
where nobody looks for it.

### 3.3 The exit-code vocabulary

Closed, and the same for every command:

| Code | Meaning |
|---|---|
| 0 | The operation did what was asked, and found nothing wrong. |
| 1 | The operation ran and reports a **finding**: a diagnostic state, a withheld write, a verdict that is not `qualified`. Nothing failed. |
| 2 | The operation **refused**: a precondition was not met and nothing was done. |
| 3 | Usage error: the arguments do not name an operation this binary has. |
| 4 | The operation **failed**: something went wrong that neither the operator nor the target asked for. |

The distinction between 1, 2 and 4 is the whole point and is the one a caller
scripts against. A `partial` apply is 1: every withheld path was named and the
contract held. A removal with no manifest is 2: it declined. An unreadable
manifest is 4.

`doctor` exits 1 on any `drifted`, `missing`, `foreign`, `shadowed` or
`unmanaged-write`, which is `002` section 3.10's requirement expressed in this
vocabulary rather than restated.

### 3.4 Two renderings of one value

Every command supports `--json`. Human output and JSON output are two renderings
of the **same** returned value, produced from it by this crate, never two code
paths that compute their own answers.

JSON output is a contract: adding a field is compatible, removing or retyping
one is a change to this spec. Human output is not a contract and may be
reshaped freely, which is exactly why a caller is given `--json` to use instead.

### 3.5 The binary's name is recorded, not defaulted

The package is `statecraft-cli`, which is `D-02` as adopted. Cargo would then
name the executable `statecraft-cli` by default, and `D-01` recommends
`statecraft` and **is not adopted for naming**.

So the name is stated here rather than inherited: the executable is
`statecraft-cli`, matching the package, until `D-01`'s naming half is decided.
Adopting `statecraft` later is a change to this section and a rename in one
`[[bin]]` stanza. What this section refuses is the name being settled by a
build-tool default that nobody recorded agreeing to.

### 3.6 What the binary reads, and what it does not

It reads its arguments, the target repository, and the product home. It does not
read a configuration file that could change a rule an owning spec fixed: an
option that would alter behavior is an argument, visible in the invocation that
produced a run record, rather than ambient state.

It never reads `.derived/` directly and never answers a specification question
itself. `spec-spine` is asked, which is `001` section 3.2.

### 3.7 Observable negative cases

| Case | Required behavior |
|---|---|
| An unknown verb | Exit 3, naming the verb and listing the ones that exist. Nothing else happens. |
| `env apply` against an unregistered target | Exit 2, refused, naming the path as unregistered. No write. |
| `env apply` that withholds a path | Exit 1, every withheld path named with its reason, and the paths that did apply reported as applied. |
| `env remove` with no manifest | Exit 2, with the reason from `002` section 3.6. No deletion. |
| `doctor` on a clean environment | Exit 0. |
| `doctor` on any finding | Exit 1, every state reported, nothing repaired. |
| A command given `--json` | Identical facts to the human rendering, from the same value. |
| A library operation returning an i/o error | Exit 4, naming the path, distinguishable from a refusal. |

## 4. Out of scope

Installing the binary; publishing it; a shell installer; per-target
configuration files; an interactive mode; and any read-only observation surface
(`F-04`). Distribution is deferred by `F-02`, and `D-01`'s packaging half is not
adopted.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what it requires.

**2026-09-16: the JSON contract has its own view types.** §3.4 makes this
crate's JSON a contract. The library types behind the verbs are not that
contract, and deriving `Serialize` onto spec 002's `Outcome` and `Report` from
here would have been a change to 002's territory made by 006's change, which the
coupling gate refuses and should. So the wire shapes are declared here, where
the contract is, and the bindings map onto them. The two reasons point the same
way, which is usually the sign a boundary is in the right place.

**2026-09-16: the environment verbs refuse, and that is not a stub.** §3.1 says a
verb joins the tree as the behavior behind it lands, and `env plan`, `env
apply`, `env upgrade`, `env remove` and `doctor` all need a configured adapter
set. No spec ratifies a provider adapter, so there is nothing for them to plan
against. They exit 2 with the reason, which is the correct answer to a
precondition that is not met, rather than exit 0 having done nothing or a
"not implemented" message that reads like a defect. The verbs that need no
adapter, the four `project` verbs, work.

**2026-09-16: the product home is overridable by `STATECRAFT_HOME`.** §3.6 says
the binary reads its arguments, the target and the product home, and that no
configuration file may change a rule. An environment variable naming *where the
product's own state lives* changes no rule: it relocates the register. It earns
its place by making the end-to-end tests possible at all, since they must not
write into the developer's real home to check that registration writes nothing
into a target.

**2026-09-16: a relative path is resolved, not refused.** Spec 002 refuses a
relative path, and that refusal is about what the register stores. An operator
typing `project register .` has not made an error, so the binding makes the path
absolute against the working directory first. It does not canonicalize:
resolving symlinks would record a path the operator did not name.

**2026-09-16: the last gate flag joined with this change.**
`index check --fail-on-unresolved` was withheld while specs claimed crates they
had not written. `006` built the last one, so it is now enforced in `make gate`
and in CI. The cost is recorded in AGENTS.md: a new spec claiming a crate ahead
of its implementation now fails the gate, and removing the flag again would be
its own deliberate change.

## Verification

Each line is one command. §3.7's rows are integration tests that **spawn the
built binary**: an exit code is a property of a process, and a test that called
a function and inspected a returned enum would check the mapping without ever
checking that the binary uses it.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
spec-spine index check --fail-on-unresolved
cargo test -p statecraft-cli --test negative_cases
```
