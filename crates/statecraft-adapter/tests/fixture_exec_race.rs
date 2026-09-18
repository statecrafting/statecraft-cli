//! The fixture adapter is exec'd by many suite rows at once, and must never be
//! refused for being open for writing.
//!
//! Spec 004 section 3.5's negative suite runs its rows as threads in one test
//! binary, and each row ends by exec'ing the script
//! [`statecraft_adapter::fixture::write`] just produced. A writer that opened
//! the executed path in this process would let a sibling thread's fork inherit
//! that descriptor, and `execve` refuses a file any process holds open for
//! writing: `ETXTBSY`, "text file busy". It was an intermittent Linux CI
//! failure that landed on a different row each time, which is what an
//! interleaving-dependent race looks like from the outside.
//!
//! This reproduces the interleaving deliberately instead of waiting for CI to
//! find it: writer threads writing and exec'ing their own fixture while other
//! threads fork children that outlive the fork, which is the same shape as the
//! suite and far denser. Any `ETXTBSY` is a failure.
//!
//! **Linux only, because that is where it was ever observed.** Measured against
//! the original in-place writer on Debian bookworm: 2 of 240 execs refused with
//! `ExecutableFileBusy`, on three consecutive runs, and none after the repair.
//! The same reproduction on darwin produced no refusal in any run and cost 35
//! seconds rather than 0.2, so running it there would buy a slow test and no
//! signal. This is a statement about where the failure was reproduced, not a
//! claim that no other platform can raise `ETXTBSY`.

#![cfg(target_os = "linux")]

use statecraft_adapter::fixture::{self, Behavior};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

const WRITERS: usize = 6;
const HOLDERS: usize = 6;
const ROUNDS: usize = 40;

/// One round: write a fixture and exec it, reporting any refusal.
fn write_and_exec(thread: usize, round: usize) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    let adapter = fixture::write(dir.path(), Behavior::CompletedWithNoCost).unwrap();
    // Spawned directly rather than through `supervise`: the race is in the
    // exec, and this keeps the reproduction free of the supervisor's deadline
    // and reader threads.
    match Command::new(&adapter)
        .current_dir(dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            child.wait().unwrap();
            None
        }
        Err(e) => Some(format!("writer {thread} round {round}: {:?} {e}", e.kind())),
    }
}

#[test]
fn a_freshly_written_fixture_is_never_refused_as_busy_by_a_concurrent_exec() {
    let stop = AtomicBool::new(false);
    let failures: Vec<String> = std::thread::scope(|scope| {
        // The other half of the race. Each of these forks a child that outlives
        // the fork by a little, which is precisely what holds an inherited
        // descriptor open past the writer's `drop`. The suite's own rows do this
        // to each other; here it is deliberate and dense.
        for _ in 0..HOLDERS {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let mut child = Command::new("/bin/sleep")
                        .arg("0.02")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .unwrap();
                    child.wait().unwrap();
                }
            });
        }

        let writers: Vec<_> = (0..WRITERS)
            .map(|thread| {
                scope.spawn(move || {
                    (0..ROUNDS)
                        .filter_map(|round| write_and_exec(thread, round))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let failures: Vec<String> = writers
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();
        stop.store(true, Ordering::Relaxed);
        failures
    });

    assert!(
        failures.is_empty(),
        "{} of {} execs were refused: {failures:#?}",
        failures.len(),
        WRITERS * ROUNDS
    );
}
