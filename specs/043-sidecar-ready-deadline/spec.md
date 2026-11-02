---
id: "043-sidecar-ready-deadline"
title: "The witness sidecar's ready deadline starts when its process starts"
status: approved
implementation: pending
created: "2026-10-21"
summary: >
  Amends 013 so the supervisor's ready deadline measures the producer's own
  startup, not the operating system's assessment of a newly placed
  executable. A bounded start deadline covers the time from spawn until the
  producer's code is observed running; the existing ready deadline runs from
  that observation. Closes the residual 013 records for a ready-timeout under
  syspolicyd load, and measures the same route on Linux.
amends:
  - "013-supervised-wire-witness"
extends:
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "004-execution-adapter"
  - "013-supervised-wire-witness"
obligations:
  - id: "R-1"
    kind: requirement
    text: "The ready deadline is measured from an observation that the producer's own code is running, never from spawn; the time before that observation is bounded by a separate start deadline."
    anchor: "3-1-two-deadlines"
  - id: "R-2"
    kind: requirement
    text: "A sidecar that never starts, starts but never reports ready, or exits first is still refused before the child spawns, under a closing reason that names which deadline ran out."
    anchor: "3-2-what-each-outcome-records"
  - id: "R-3"
    kind: requirement
    text: "The start observation reads only what the supervisor already holds or the operating system reports about the process; it adds no protocol message, trusts no producer claim about time, and widens no confinement."
    anchor: "3-3-the-start-observation"
---

# 043: The witness sidecar's ready deadline starts when its process starts

## 1. Purpose

Spec 013 gives a witness sidecar 20 seconds from spawn to report `ready`
(section 7, 2026-10-21, owner decision). On macOS the first execution of a
newly placed executable is assessed by the operating system before the
executable's own code runs. The released-producer test copies the producer to
a fresh path each run, so every run pays that assessment, and under
`syspolicyd` or XProtect load it has taken longer than the whole deadline: the
test failed twice with `ready-timeout` and passed when run alone. The posture
it recorded, `witness-failed`, was accurate; the deadline was measuring the
wrong thing.

Measured on 2026-10-21 on macOS 26.5.1 (`aarch64-apple-darwin`), with the
released wire-witness 0.1.0, unconfined, and the machine otherwise lightly
loaded: the first `wire-witness identity` of each of five fresh copies took
0.27 to 0.32 seconds wall time, and the second 0.05 to 0.06 seconds. The cost is per newly placed file and is paid before
the producer runs. The owner decided on 2026-10-20 that the deadline starts at
process start rather than at spawn, and that this spec measures the Linux route
too.

## 2. Territory

This spec owns no new unit. Its implementing change touches the sidecar
supervision in `crates/statecraft-adapter/src/witness/sidecar.rs` (spec 004's
directory, extended by 013) and the deadlines the run path passes from
`crates/statecraft-cli/src/witness.rs` (spec 006's directory, extended by
013), and declares those `extends` edges with the code. It changes nothing in
the producer, its protocol, either confinement, or the posture words of 013
section 3.7.

## 3. Behavior

### 3.1 Two deadlines

The supervisor runs two deadlines in sequence:

1. **Start deadline.** From spawn until the start observation of section 3.3.
   It bounds the operating system's work before the producer runs, including
   the assessment of a newly placed executable, and is longer than the ready
   deadline.
2. **Ready deadline.** From the start observation until a bound `ready` that
   validates. It keeps 013's value and meaning: the producer's own startup.

The two values are constants named in the run path, as `READY_TIMEOUT` is
now. Their values are set from the measurements of section 5 and recorded in
this spec's decisions with those measurements; this draft does not choose
them.

### 3.2 What each outcome records

Every outcome 013 section 4 names keeps its posture. The closing outcome's
termination reason distinguishes the deadline:

| Outcome | Reason | Posture |
|---|---|---|
| No start observation before the start deadline | `start-timeout` | `witness-failed` |
| Started, no `ready` before the ready deadline | `ready-timeout` | `witness-failed` |
| Exits before `ready`, either phase | `sidecar-exited-before-ready` | `witness-failed` |

`start-timeout` is a new reason word in the sidecar effect's closing outcome.
A reader that does not know it renders the posture, which it already reads,
and never infers success. No path admits a child whose sidecar has not
reported a validated `ready`.

### 3.3 The start observation

The observation is a fact about the process that the supervisor can read
without the producer's cooperation, because a claim from the producer about
when it started is testimony (013 section 3.4) and wire-witness 0.1.0 sends no
message before `ready`. It must not be satisfied by spawn alone, by the
confinement wrapper running, or by the kernel creating the process. Candidates
the implementation measures, and records the one it adopts:

- **The start line is consumed.** The supervisor writes the start message to
  the producer's stdin before waiting. The producer reads it only once its
  code runs. Whether the supervisor can observe the pipe draining, on both
  platforms, without a protocol change, is open.
- **The process image is the producer's and has left the loader.** The
  operating system reports the executing image and its state; whether it can
  distinguish "assessed and running" from "blocked in assessment" on macOS is
  open.
- **The assessment is made before the attempt.** Not a start observation: the
  producer is executed once, confined, when `locate` verifies it, so the
  attempt's spawn finds it already assessed. This keeps one deadline. It is
  listed because it may be the only route that needs no new observation; it
  changes when the operator's machine assesses the file, and the owner chooses
  between it and an observation.

If no observation satisfies this section on a platform, that platform keeps
013's single deadline from spawn, and the residual stays recorded for it.

## 4. Observable negative cases

| Case | Required result |
|---|---|
| Producer blocked before its code runs, past the ready deadline but inside the start deadline | Reaches `ready`; the child is admitted. |
| Producer blocked past the start deadline | `start-timeout`; `witness-failed`; the child never spawns. |
| Producer runs but never sends `ready` | `ready-timeout` after the ready deadline from the start observation. |
| A wrapper or loader activity alone | Does not count as the start observation. |
| A producer message claiming a start time | Ignored for timing. |
| Platform with no adopted observation | 013's single deadline from spawn; the residual stays recorded. |

## 5. Measurements this spec requires

- **M-1, macOS.** The time from spawn to the start observation and from it to
  `ready`, for a freshly placed and a previously executed producer, idle and
  under assessment load, confined as 013 section 3.6.2 confines the sidecar.
- **M-2, Linux.** The same two intervals on `aarch64-unknown-linux-gnu` and
  `x86_64-unknown-linux-gnu`. Linux has no comparable assessment of a newly
  placed file; the measurement establishes whether the start deadline is ever
  material there and whether the observation works under Landlock.
- **M-3, the observation itself.** That the adopted observation is not
  satisfied before the producer's code runs, shown with a fixture producer
  that blocks before reading its input.

## 6. Out of scope

- Changing the producer, its protocol, or its release.
- Preventing or shortening the operating system's assessment, or allowing any
  service the sidecar's confinement refuses.
- Retrying a sidecar, or running a child unwitnessed after a requested witness
  failed to start (013 section 7, 2026-10-17).

## 7. Resolved decisions

**2026-10-31: an observation first, the pre-assessed producer as the fallback,
per platform.** The first two candidates of section 3.3 keep the two deadlines
and leave when the operator's machine assesses the file unchanged. The
implementation measures them (M-3) and adopts the first that is not satisfied
before the producer's code runs. Where neither can be shown on a platform, that
platform adopts the confined execution at `locate`, keeps 013's single deadline
from spawn, and records that here with the measurement.

**2026-10-31: `start-timeout` needs no reader change.** `run show` renders the
posture it already reads and shows a reason word it does not know as recorded;
section 3.2 already forbids inferring success from either.

**2026-10-31: the adopted observation, on macOS and Linux, is the start line
consumed, read from a retained read end.** The supervisor makes the producer's
stdin pipe itself, keeps a duplicate of its read end through the start phase,
writes the start line, and asks the kernel how many of its bytes remain unread
(`FIONREAD` on that end); fewer than it wrote is the observation. The write
end's own count is not used, because on macOS it reports that end's own empty
buffer. Nothing that runs before the producer's code (the kernel creating the
process, the confinement wrapper, the loader, the assessment of a newly placed
file) reads that pipe, which M-3's fixture, a producer that sleeps before
reading its input, shows in `witness_sidecar` on both platforms. The second
candidate of section 3.3 is not adopted: the fixture producer is a script run
by `/bin/sh`, whose process image never becomes the producer's, so M-3 cannot
show it, and the first candidate suffices. A kernel that declines the query
leaves that attempt on 013's single deadline from spawn, and the supervisor
records the observation as unavailable; neither platform has shown that.

**2026-10-31: the deadline word is the closing outcome's no-digest reason.**
The run path writes `start-timeout` or `ready-timeout` as the sidecar
effect's `noDigestReason` when the sidecar was refused for that deadline, in
place of `sidecar-exited-before-ready`; every other refusal keeps 013's words,
and the attempt's `witness.failure` carries the same word as before. The
protocol module is unchanged.

**2026-10-31: provisional values until M-1 and M-2 are recorded here.**
`START_TIMEOUT` is 60 seconds and `READY_TIMEOUT` keeps 013's 20 seconds, in
`crates/statecraft-cli/src/witness.rs`. The supervisor exposes both intervals
of M-1 and M-2 (spawn to the observation, the observation to `ready`) as the
sidecar's `Timing`, so the measurements read them rather than the wall clock.
This spec stays `implementation: pending` until those measurements and the
values they set are entered here.

**2026-11-01: M-1 and the aarch64 half of M-2, measured; x86_64 is not.**
`crates/statecraft-cli/tests/witness_start_measurement.rs` (ignored; run with
`--ignored --nocapture` and `STATECRAFT_M_PRODUCER` naming the producer) runs
the wire-witness 0.1.0 release through the product's own route: placed at its
home path and digest-checked by `locate`, confined by `admit_sidecar`, started
by `start_confined`, with both intervals read from the sidecar's `Timing`.
"Fresh" places a new copy in a new home every attempt; "warm" reuses one
already executed copy after one discarded attempt; "loaded" adds eight
busy-loop processes and two that keep copying and executing the producer. Each
case is 20 attempts; every attempt reached `ready`, none was refused and the
observation was available in every one. Milliseconds, median / p95 / max:

| Platform | Case | Spawn to observation | Observation to `ready` |
|---|---|---|---|
| macOS 26.5.1, aarch64 (M1 Max, 10 cores) | fresh, idle | 235 / 243 / 245 | 3.1 / 11.6 / 29.6 |
| | warm, idle | 41 / 44 / 44 | 3.0 / 8.3 / 9.5 |
| | fresh, loaded | 458 / 497 / 619 | 16.8 / 29.6 / 52.5 |
| | warm, loaded | 54 / 70 / 72 | 4.5 / 15.1 / 17.7 |
| Linux 7.0.14, aarch64 (Docker, 6 CPUs), Landlock | fresh, idle | 31 / 33 / 34 | 3.2 / 5.2 / 12.5 |
| | warm, idle | 31 / 33 / 34 | 2.3 / 3.5 / 3.6 |
| | fresh, loaded | 72 / 104 / 106 | 17.5 / 56.5 / 104 |
| | warm, loaded | 73 / 122 / 134 | 19.6 / 47.6 / 49.3 |

The macOS assessment of a newly placed producer is visible (fresh exceeds
warm by about 200 ms idle and 400 ms loaded) and is two orders of magnitude
inside `START_TIMEOUT`; Linux shows no such difference, and the observation
works under Landlock. The measuring found one defect, fixed with it: a
producer that read its start line and answered between two start-phase polls
reached `ready` with no observation recorded and its ready interval counted
from spawn; the retained read end is now also asked when a line arrives in
that phase (`a_producer_that_answers_between_two_polls_is_still_observed_started`).
The values stay 60 and 20 seconds: the largest observed interval is 0.62
seconds. `x86_64-unknown-linux-gnu` is not measured: this host can run it
only emulated, which is not that platform, so `implementation` stays
`pending` until a native x86_64 run is entered here.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is run through
`make verify SPEC=043`, with M-3's fixture producer under the platform it runs
on and no provider; the measurements of section 5 are recorded in section 7
with the change that sets the deadlines.

```verify:cli
cargo test -p statecraft-adapter --test witness_sidecar
cargo test -p statecraft-cli --lib witness
cargo test -p statecraft-cli --test witness_admission
cargo test -p statecraft-cli --test witness_sidecar_confinement
cargo test -p statecraft-cli --test witness_run
```

These show that a producer blocked before reading its input is not observed
started and still reaches `ready` past the ready deadline, that each deadline
refuses under its own word with the posture `witness-failed`, that a
producer's claim about its start is ignored for timing, that the start
deadline is the longer and the ready deadline keeps 013's value, and that the
sidecar suites of 013 still pass under the two deadlines, the confined ones
included.
