//! Spec 040 section 3.2: each verb and its typed operation, given the same
//! logical inputs, answer byte-identical family envelopes and leave identical
//! effects. Each case builds one fixture, keeps a pristine copy, runs the verb
//! against it, records the envelope and every byte under the fixture, restores
//! the pristine copy at the same path, runs the operation through
//! `render::Answer`, and compares both. Nothing is normalized: the clock is
//! fixed on both sides (the binary's through `STATECRAFT_TEST_CLOCK`, read only
//! by a debug build), and the paths are the same paths.

#![cfg(unix)]

use statecraft_cli::application::{
    Context, ReconcileOperands, RecoverChoice, Response, TrialOperands, accept, override_recover,
    run, run_reconcile, startup_trial,
};
use statecraft_cli::commands::Verb;
use statecraft_cli::render::Format;
use statecraft_environment::registry::Registry;
use statecraft_environment::time::FixedClock;
use statecraft_home::trial::Origin;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const AT: i64 = 1_790_000_000;

/// One fixture: a product home and a registered (or not) Git target, under
/// one root so a snapshot covers both.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new(register: bool) -> Fixture {
        let f = Fixture {
            dir: tempfile::tempdir().unwrap(),
        };
        let target = f.target();
        std::fs::create_dir_all(&target).unwrap();
        for args in [
            &["init", "--quiet"][..],
            &["config", "user.name", "fixture"],
            &["config", "user.email", "fixture@example.com"],
            // No background maintenance: its transient lock would land in
            // one side's snapshot and not the other's.
            &["config", "maintenance.auto", "false"],
            &["config", "gc.auto", "0"],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(&target)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(target.join("a.txt"), "a").unwrap();
        for args in [&["add", "a.txt"][..], &["commit", "--quiet", "-m", "base"]] {
            assert!(
                Command::new("git")
                    .current_dir(&target)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::create_dir_all(f.home()).unwrap();
        if register {
            let out = f.verb(&["project", "register", target.to_str().unwrap()]);
            assert!(out.0 <= 1, "{}", out.1);
        }
        f
    }
    fn root(&self) -> &Path {
        self.dir.path()
    }
    fn home(&self) -> PathBuf {
        self.root().join("home")
    }
    fn target(&self) -> PathBuf {
        self.root().join("target")
    }

    /// The verb, with `--json`, against this fixture: (exit, stdout).
    fn verb(&self, args: &[&str]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("HOME", self.root())
            .env("PATH", "/usr/bin:/bin")
            .env("STATECRAFT_TEST_CLOCK", AT.to_string())
            .current_dir(self.root())
            .output()
            .unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap(),
        )
    }

    /// The verb, with `--json`, in this test process's own environment, so a
    /// judge selection that reads the environment reads the same one the
    /// in-process operation does: (exit, stdout).
    fn verb_inheriting(&self, args: &[&str]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_TEST_CLOCK", AT.to_string())
            .current_dir(self.root())
            .output()
            .unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap(),
        )
    }

    fn arm(&self) {
        let out = self.verb(&["project", "arm", self.target().to_str().unwrap()]);
        assert_eq!(out.0, 0, "{}", out.1);
    }

    /// Pin the target to a spec-spine no binary reports, so no judge is
    /// selected whatever this machine has installed.
    fn pin_unresolvable(&self) {
        std::fs::write(
            self.target().join("spec-spine.toml"),
            "[meta]\nrequired_version = \"=0.0.1\"\n",
        )
        .unwrap();
    }

    fn registry(&self) -> Registry {
        Registry::read(&self.home()).unwrap()
    }

    /// Every file under the fixture, by path, with its bytes; `.git`'s index
    /// and logs are included because a verb could touch them too.
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![self.root().to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                let m = std::fs::symlink_metadata(&p).unwrap();
                if m.is_dir() {
                    stack.push(p);
                } else {
                    out.insert(
                        p.strip_prefix(self.root()).unwrap().to_path_buf(),
                        std::fs::read(&p).unwrap_or_default(),
                    );
                }
            }
        }
        out
    }

    /// A pristine copy, restorable at the same path.
    fn keep(&self) -> tempfile::TempDir {
        let copy = tempfile::tempdir().unwrap();
        assert!(
            Command::new("cp")
                .args([
                    "-Rp",
                    &format!("{}/.", self.root().display()),
                    copy.path().to_str().unwrap()
                ])
                .status()
                .unwrap()
                .success()
        );
        copy
    }
    fn restore(&self, copy: &tempfile::TempDir) {
        for e in std::fs::read_dir(self.root()).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                std::fs::remove_dir_all(&p).unwrap();
            } else {
                std::fs::remove_file(&p).unwrap();
            }
        }
        assert!(
            Command::new("cp")
                .args([
                    "-Rp",
                    &format!("{}/.", copy.path().display()),
                    self.root().to_str().unwrap()
                ])
                .status()
                .unwrap()
                .success()
        );
    }
}

/// Run the verb, restore, run the operation; both envelopes and both
/// resulting trees must be equal.
fn equivalent(
    f: &Fixture,
    verb: Verb,
    args: &[&str],
    operation: impl Fn(&Context<'_>) -> Result<Response, String>,
) {
    compare(f, verb, args, Setting::Clean, operation);
}

/// What each side runs under.
#[derive(Clone, Copy, PartialEq)]
enum Setting {
    /// The verb in a cleared environment.
    Clean,
    /// The verb in this process's environment.
    Inherited,
    /// As `Inherited`, with the target's repository lock held by this
    /// process throughout each side.
    Busy,
}

fn compare(
    f: &Fixture,
    verb: Verb,
    args: &[&str],
    setting: Setting,
    operation: impl Fn(&Context<'_>) -> Result<Response, String>,
) {
    let pristine = f.keep();
    let mut full: Vec<&str> = args.to_vec();
    full.push("--json");
    let hold = || {
        (setting == Setting::Busy)
            .then(|| statecraft_run::lock::try_acquire(&f.home(), &f.target()).unwrap())
    };
    let held = hold();
    let (code, by_verb) = match setting {
        Setting::Clean => f.verb(&full),
        Setting::Inherited | Setting::Busy => f.verb_inheriting(&full),
    };
    drop(held);
    let after_verb = f.snapshot();
    f.restore(&pristine);
    let _held = hold();
    let registry = f.registry();
    let clock = FixedClock(AT);
    let home = f.home();
    let ctx = Context {
        home: &home,
        registry: &registry,
        clock: &clock,
    };
    let by_operation = match operation(&ctx) {
        Ok(answer) => {
            assert_eq!(answer.exit.code(), code, "{args:?}");
            answer.render(Format::Json(verb))
        }
        Err(usage) => {
            assert_eq!(code, 3, "{args:?}: the operation said usage: {usage}");
            statecraft_cli::render::usage_envelope(&verb.dotted(), &usage)
        }
    };
    assert_eq!(by_verb, by_operation, "{args:?}: the envelopes differ");
    let after_operation = f.snapshot();
    assert_eq!(
        after_verb.keys().collect::<Vec<_>>(),
        after_operation.keys().collect::<Vec<_>>(),
        "{args:?}: the files written differ"
    );
    for (path, bytes) in &after_verb {
        assert_eq!(
            Some(bytes),
            after_operation.get(path),
            "{args:?}: {} differs",
            path.display()
        );
    }
}

fn usage_of(r: Result<Response, statecraft_cli::application::Usage>) -> Result<Response, String> {
    r.map_err(|u| u.0)
}

/// A-EQ-ACCEPT: refusals before any judgement are the verb's, in its order.
#[test]
fn accept_answers_as_the_verb_does() {
    // Unregistered, with and without a run id: registration is judged first.
    let f = Fixture::new(false);
    let t = f.target();
    let target = t.to_str().unwrap();
    equivalent(&f, Verb::Accept, &["accept", target, "replay"], |ctx| {
        usage_of(accept(ctx, &t, Some("replay")))
    });
    equivalent(&f, Verb::Accept, &["accept", target], |ctx| {
        usage_of(accept(ctx, &t, None))
    });
    // Registered: a missing run id is usage; an unknown run is its answer.
    let f = Fixture::new(true);
    let t = f.target();
    let target = t.to_str().unwrap();
    equivalent(&f, Verb::Accept, &["accept", target], |ctx| {
        usage_of(accept(ctx, &t, None))
    });
    equivalent(&f, Verb::Accept, &["accept", target, "replay"], |ctx| {
        usage_of(accept(ctx, &t, Some("replay")))
    });
}

/// A-EQ-RECOVER: the report and a recorded choice, and an inadmissible one.
#[test]
fn override_recover_answers_and_records_as_the_verb_does() {
    let f = Fixture::new(true);
    let t = f.target();
    let target = t.to_str().unwrap();
    equivalent(
        &f,
        Verb::OverrideRecover,
        &["override", "recover", target],
        |ctx| Ok(override_recover(ctx, &t, None)),
    );
    for (choice, journal, authority) in [
        ("adopt-empty", "absent", "absent"),
        ("adopt-as-read", "absent", "absent"),
        ("discard-pending", "0000", "absent"),
    ] {
        let args = [
            "override", "recover", target, choice, journal, authority, "sam", "a", "reason",
        ];
        equivalent(&f, Verb::OverrideRecover, &args, |ctx| {
            Ok(override_recover(
                ctx,
                &t,
                Some(RecoverChoice {
                    choice: statecraft_run::overrides::Choice::parse(choice).unwrap(),
                    journal,
                    authority,
                    operator: "sam",
                    reason: "a reason",
                }),
            ))
        });
    }
    // A-ADMISSION: a malformed journal is reported, and each choice made
    // against it, including one with nothing to settle, keeps its outcome.
    let journal_file = statecraft_run::overrides::journal_path(&f.home(), &t);
    std::fs::create_dir_all(journal_file.parent().unwrap()).unwrap();
    std::fs::write(&journal_file, "{ not a journal line\n").unwrap();
    equivalent(
        &f,
        Verb::OverrideRecover,
        &["override", "recover", target],
        |ctx| Ok(override_recover(ctx, &t, None)),
    );
    let digest = statecraft_environment::digest::digest_bytes(b"{ not a journal line\n");
    for (choice, journal) in [
        ("complete-pending", digest.as_str()),
        ("adopt-prefix", digest.as_str()),
        ("adopt-empty", "absent"),
        ("adopt-empty", digest.as_str()),
    ] {
        let args = [
            "override", "recover", target, choice, journal, "absent", "sam", "a", "reason",
        ];
        equivalent(&f, Verb::OverrideRecover, &args, |ctx| {
            Ok(override_recover(
                ctx,
                &t,
                Some(RecoverChoice {
                    choice: statecraft_run::overrides::Choice::parse(choice).unwrap(),
                    journal,
                    authority: "absent",
                    operator: "sam",
                    reason: "a reason",
                }),
            ))
        });
    }
    // Unregistered.
    let f = Fixture::new(false);
    let t = f.target();
    equivalent(
        &f,
        Verb::OverrideRecover,
        &["override", "recover", t.to_str().unwrap()],
        |ctx| Ok(override_recover(ctx, &t, None)),
    );
}

/// A-EQ-RECONCILE: refusals before and after resolution, and a missing
/// attempt, invent no success.
#[test]
fn run_reconcile_refuses_as_the_verb_does() {
    for registered in [false, true] {
        let f = Fixture::new(registered);
        let t = f.target();
        let target = t.to_str().unwrap().to_string();
        for (attempt, finding, state) in [
            ("1", "absent", "not-launched"),
            ("x", "absent", "not-launched"),
            ("1", "maybe", "not-launched"),
            ("1", "absent", "nonsense"),
        ] {
            let args = [
                "run",
                "reconcile",
                target.as_str(),
                "replay",
                attempt,
                finding,
                state,
                "sam",
                "why",
            ];
            equivalent(&f, Verb::RunReconcile, &args, |ctx| {
                usage_of(run_reconcile(
                    ctx,
                    &ReconcileOperands {
                        path: &t,
                        run_id: "replay",
                        attempt,
                        finding,
                        inspected: state,
                        operator: "sam",
                        reason: "why",
                        evidence: &[],
                    },
                ))
            });
        }
    }
}

/// A-EQ-RUN and A-ADMISSION: every admission `run` performs before a launch,
/// in the verb's order: registration, arming, the lock, the judge, the spec
/// id. None of them appends an attempt, on either side.
#[test]
fn run_admits_and_refuses_as_the_verb_does() {
    // Unregistered: judged before the missing spec id.
    let f = Fixture::new(false);
    let t = f.target();
    let target = t.to_str().unwrap();
    equivalent(&f, Verb::Run, &["run", target, "001-x"], |ctx| {
        usage_of(run(ctx, &t, Some("001-x")))
    });
    equivalent(&f, Verb::Run, &["run", target], |ctx| {
        usage_of(run(ctx, &t, None))
    });
    // Registered and not armed: consent is judged before the lock.
    let f = Fixture::new(true);
    let t = f.target();
    let target = t.to_str().unwrap();
    compare(
        &f,
        Verb::Run,
        &["run", target, "001-x"],
        Setting::Busy,
        |ctx| usage_of(run(ctx, &t, Some("001-x"))),
    );
    // Armed: a held lock refuses before the judge is consulted.
    f.arm();
    compare(
        &f,
        Verb::Run,
        &["run", target, "001-x"],
        Setting::Busy,
        |ctx| usage_of(run(ctx, &t, Some("001-x"))),
    );
    // Armed with no judge: the report refusal comes before the spec id.
    f.pin_unresolvable();
    for args in [&["run", target, "001-x"][..], &["run", target]] {
        let id = args.get(2).copied();
        compare(&f, Verb::Run, args, Setting::Inherited, |ctx| {
            usage_of(run(ctx, &t, id))
        });
    }
}

/// A-EQ-TRIAL and A-ADMISSION: the trial's admission, and each refusal that
/// precedes its attempt, in the verb's order.
#[test]
fn startup_trial_admits_and_refuses_as_the_verb_does() {
    let synthetic = || {
        Some(TrialOperands {
            stated: vec![Origin::Synthetic],
            deadline: statecraft_home::trial::DEFAULT_DEADLINE_SECONDS,
        })
    };
    // Unregistered, with operands and with unreadable ones.
    let f = Fixture::new(false);
    let t = f.target();
    let target = t.to_str().unwrap();
    equivalent(
        &f,
        Verb::StartupTrial,
        &["startup", "trial", target, "--synthetic"],
        |ctx| usage_of(startup_trial(ctx, &t, synthetic())),
    );
    equivalent(
        &f,
        Verb::StartupTrial,
        &["startup", "trial", target, "--bogus"],
        |ctx| usage_of(startup_trial(ctx, &t, None)),
    );
    // Registered, not armed.
    let f = Fixture::new(true);
    let t = f.target();
    let target = t.to_str().unwrap();
    equivalent(
        &f,
        Verb::StartupTrial,
        &["startup", "trial", target, "--synthetic"],
        |ctx| usage_of(startup_trial(ctx, &t, synthetic())),
    );
    // Armed: unreadable operands, no origin, two origins, a refused
    // deadline, and a target with no manifest.
    f.arm();
    let cases: [(&[&str], Option<TrialOperands>); 5] = [
        (&["--deadline", "x"], None),
        (
            &[],
            Some(TrialOperands {
                stated: vec![],
                deadline: 60,
            }),
        ),
        (
            &["--synthetic", "--provider-session"],
            Some(TrialOperands {
                stated: vec![Origin::Synthetic, Origin::ProviderSession],
                deadline: 60,
            }),
        ),
        (
            &["--synthetic", "--deadline", "0"],
            Some(TrialOperands {
                stated: vec![Origin::Synthetic],
                deadline: 0,
            }),
        ),
        (&["--synthetic"], synthetic()),
    ];
    for (flags, operands) in cases {
        let mut args = vec!["startup", "trial", target];
        args.extend_from_slice(flags);
        let operands = std::cell::RefCell::new(operands);
        equivalent(&f, Verb::StartupTrial, &args, |ctx| {
            usage_of(startup_trial(ctx, &t, operands.borrow_mut().take()))
        });
    }
}

/// A-ADAPTER, first half: the five adapters spec 040 thinned call their one
/// operation and render it. None of them resolves a registration, reads
/// arming, takes a lock, opens the chain or the journal, selects a report or
/// launches; the dispatch arms that reach them check only that a path was
/// typed.
#[test]
fn the_adapters_hold_no_admission_or_orchestration() {
    let source = include_str!("../../src/main.rs");
    let body = |name: &str| -> &str {
        let start = source
            .find(&format!("\nfn {name}("))
            .unwrap_or_else(|| panic!("main.rs defines {name}"));
        let rest = &source[start + 1..];
        &rest[..rest.find("\n}\n").expect("the function ends") + 3]
    };
    for (adapter, operation) in [
        ("run_verb", "run_witnessed(&ctx"),
        ("trial_verb", "startup_trial(&ctx"),
        ("accept_verb", "accept(&ctx"),
        ("recover_verb", "override_recover("),
        ("reconcile_verb", "run_reconcile("),
    ] {
        let body = body(adapter);
        assert!(body.contains(operation), "{adapter} calls {operation}");
        for admission in [
            "registered(",
            "stored_or_typed(",
            "registry.get",
            ".armed",
            "try_acquire",
            "Chain::open",
            "overrides::read",
            "overrides::inspect",
            "report_source",
            "corpus_report",
            "launch_attempt",
            "Manifest::read",
            "print!",
        ] {
            assert!(
                !body.contains(admission),
                "{adapter} performs {admission} itself"
            );
        }
    }
}

/// A-ADAPTER, second half: the format a caller selects changes the rendering
/// and nothing else. The verb with and without `--json`, and the operation,
/// each from the same state, leave the same bytes; the human rendering is the
/// operation's answer rendered for a human, and the exit is the same.
#[test]
fn format_selection_changes_no_result_or_effect() {
    let f = Fixture::new(true);
    let t = f.target();
    let target = t.to_str().unwrap();
    let args = [
        "override",
        "recover",
        target,
        "adopt-empty",
        "absent",
        "absent",
        "sam",
        "a",
        "reason",
    ];
    let before = f.snapshot();
    let pristine = f.keep();
    let (json_code, _) = f.verb(&[&args[..], &["--json"]].concat());
    let after_json = f.snapshot();
    f.restore(&pristine);
    let (human_code, human) = f.verb(&args);
    let after_human = f.snapshot();
    f.restore(&pristine);
    let registry = f.registry();
    let clock = FixedClock(AT);
    let home = f.home();
    let ctx = Context {
        home: &home,
        registry: &registry,
        clock: &clock,
    };
    let answer = override_recover(
        &ctx,
        &t,
        Some(RecoverChoice {
            choice: statecraft_run::overrides::Choice::parse("adopt-empty").unwrap(),
            journal: "absent",
            authority: "absent",
            operator: "sam",
            reason: "a reason",
        }),
    );
    let after_operation = f.snapshot();
    assert_eq!(json_code, human_code);
    assert_eq!(answer.exit.code(), human_code);
    assert_eq!(human, answer.render(Format::Human));
    assert_ne!(after_json, before, "the choice changed nothing");
    assert_eq!(after_json, after_human, "the format changed an effect");
    assert_eq!(after_json, after_operation, "the caller changed an effect");
}
