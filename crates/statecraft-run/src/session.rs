//! One run, from intent to outcome, and the fold that reads runs back.
//!
//! Added by spec `009`'s additive `extends` edge on this crate. Spec 006
//! section 3.2 forbids a command from being a second implementation, and spec
//! 009 section 3.5 gives that its practical form: when a verb needs something
//! the owning library does not expose, the entry point is added **here** and the
//! binding calls it.
//!
//! # Why this is two halves and not one call
//!
//! The adapter is spec 004's, in a crate that depends on this one, so this crate
//! cannot call it. [`begin`] does everything before the child exists (resolve
//! the base, refuse a second live attempt, prepare the workspace, make the
//! intent durable) and [`conclude`] does everything after it is gone (decide the
//! outcome from the supervisor's own accounting and record it). The supervision
//! between them is the caller's, and it is the only thing the caller does.
//!
//! # Concluding an attempt does not release the workspace
//!
//! [`crate::workspace::release`] exists and its own words are "used when a run
//! ends; never during one". Concluding an **attempt** is not a run ending: spec
//! 003 section 3.4 makes a retry an appended attempt of the same run, and
//! [`crate::workspace::prepare`] is idempotent by run identity precisely so the
//! retry finds the workspace it left. Releasing here would break that, and it
//! was measured doing so: the worktree went away, the branch `release` created
//! did not, and the next attempt could not prepare. So the workspace is
//! retained and the outcome record says so.
//!
//! Nothing here reads memory as an authority: [`runs`] folds the record, which
//! is spec 003 section 3.3's rule, and [`begin`] reads the fold to find a live
//! attempt rather than trusting the process that is asking.

use crate::attempt::{Attempt, Outcome, Run};
use crate::record::{Chain, Entry, Identity, Kind};
use crate::refusal::{Accounting, decide};
use crate::workspace::{self, Workspace, WorkspaceError};
use serde::{Deserialize, Serialize};
use statecraft_environment::time::{Clock, rfc3339_utc};
use std::path::Path;

/// Fold the record into every run it describes, oldest first.
///
/// Spec 009 section 3.3: inspection reads **nothing else**. Not the filesystem,
/// not the prepared worktree, not the adapter's transcript, and not memory
/// carried from an earlier command in the same process. The parameter is a
/// chain and there is no other one, which is how that is structural here rather
/// than a promise in prose.
pub fn runs(chain: &Chain) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for entry in chain.entries() {
        let run = match out.iter_mut().find(|r| r.id == entry.run_id) {
            Some(r) => r,
            None => {
                out.push(Run::new(&entry.run_id));
                out.last_mut().expect("just pushed")
            }
        };
        match entry.kind {
            Kind::Intent if entry.subject == INTENT_SUBJECT => {
                let base = entry
                    .detail
                    .get("baseCommit")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                // An intent for an attempt number already folded is not a
                // second attempt: the record is append-only and a replayed
                // intent must not multiply what it describes.
                if !run.attempts.iter().any(|a| a.number == entry.attempt) {
                    run.append_attempt(base);
                }
            }
            Kind::Outcome if entry.subject == OUTCOME_SUBJECT => {
                if let Some(word) = entry.detail.get("outcome").and_then(|v| v.as_str()) {
                    if let Some(outcome) = outcome_from_word(word) {
                        if let Some(a) = run.attempts.iter_mut().find(|a| a.number == entry.attempt)
                        {
                            // Written once. A second outcome record for a
                            // concluded attempt is ignored rather than allowed
                            // to rewrite one.
                            if a.outcome.is_none() {
                                a.outcome = Some(outcome);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The subject an intent record carries.
pub const INTENT_SUBJECT: &str = "prepare-workspace";

/// The subject an outcome record carries.
pub const OUTCOME_SUBJECT: &str = "attempt";

/// The subject the supervisor's own accounting record carries.
pub const ACCOUNTING_SUBJECT: &str = "refusals";

fn outcome_from_word(word: &str) -> Option<Outcome> {
    Outcome::all().into_iter().find(|o| o.word() == word)
}

/// Why a run could not begin or conclude.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// The repository already has a live attempt.
    ///
    /// Spec 003 section 3.7: one live attempt per registered repository, with
    /// the workspace as the lock. The refusal **names** the live attempt,
    /// because "something else is running" is not something an operator can act
    /// on.
    #[error(
        "run {run_id} attempt {attempt} is live in this repository; \
         one live attempt per repository, so this is refused and nothing was prepared"
    )]
    LiveAttempt {
        /// The run that holds the lock.
        run_id: String,
        /// Its live attempt's number.
        attempt: u32,
    },
    /// The workspace could not be prepared or released.
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    /// The record could not be written.
    ///
    /// Spec 009 section 3.4: a supervisor that could not write the record is a
    /// **failure**, not a refusal. Neither the operator nor the target asked
    /// for it.
    #[error(transparent)]
    Record(#[from] crate::record::RecordError),
}

/// A run that has begun: its workspace exists and its intent is durable.
#[derive(Debug, Clone)]
pub struct Session {
    /// The run.
    pub run_id: String,
    /// Which attempt this is within it.
    pub attempt: u32,
    /// The prepared workspace.
    pub workspace: Workspace,
    /// The base revision as it was named.
    pub base_revision: String,
}

/// Begin a run: refuse a second live attempt, prepare, and record the intent.
///
/// The order is load-bearing. The intent is written and fsynced **before** the
/// workspace exists, which is spec 003 section 3.6's rule that intent is durable
/// before the effect; a crash between the two leaves an intent with no outcome,
/// which is exactly what reconciliation is for.
pub fn begin(
    chain: &mut Chain,
    target: &Path,
    run_id: &str,
    base_revision: &str,
    clock: &dyn Clock,
) -> Result<Session, SessionError> {
    for run in runs(chain) {
        if let Some(live) = run.live_attempt() {
            return Err(SessionError::LiveAttempt {
                run_id: run.id.clone(),
                attempt: live.number,
            });
        }
    }

    let base_commit = workspace::resolve_base(target, base_revision)?;
    let existing = runs(chain).into_iter().find(|r| r.id == run_id);
    let number = existing.map(|r| r.attempts.len() as u32 + 1).unwrap_or(1);
    let timestamp = rfc3339_utc(clock.now_unix());

    chain.append(
        &format!("{run_id}/{number}/intent"),
        &timestamp,
        &Entry {
            kind: Kind::Intent,
            run_id: run_id.to_string(),
            attempt: number,
            subject: INTENT_SUBJECT.to_string(),
            // Section 3.3.1 clause 9: this section writes no identity. The
            // prepare-workspace intent is folded by the legacy pairing.
            effect_id: Identity::Absent,
            // The workspace path is the idempotency key: preparing twice for
            // one run prepares once, which is what makes the key real rather
            // than declared.
            idempotency_key: Some(
                workspace::workspace_path(target, run_id)
                    .display()
                    .to_string(),
            ),
            detail: serde_json::json!({
                "baseRevision": base_revision,
                "baseCommit": base_commit,
            }),
        },
    )?;

    let workspace = workspace::prepare(target, run_id, base_revision)?;
    Ok(Session {
        run_id: run_id.to_string(),
        attempt: number,
        workspace,
        base_revision: base_revision.to_string(),
    })
}

/// What concluding a run decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Concluded {
    /// The run.
    pub run_id: String,
    /// The attempt.
    pub attempt: u32,
    /// The outcome the supervisor decided, which no exit code overruled.
    pub outcome: Outcome,
    /// What the adapter classified its own termination as.
    ///
    /// Retained beside the decision, in a field named for a claim. Spec 003
    /// section 3.5 gives the count to the supervisor precisely so this cannot
    /// be the answer.
    pub adapter_claimed: Outcome,
    /// How many refusals the supervisor counted.
    pub refusals: u32,
    /// Whether the base revision moved while the attempt ran.
    pub base_moved: bool,
    /// The workspace this attempt used, retained for the next attempt.
    pub workspace_retained: String,
}

/// The mapped termination beside the adapter's own claim about it.
///
/// These are distinct inputs: a provider's error flag need not mean the work
/// failed, and a completed claim need not survive the supervisor's accounting.
#[derive(Debug, Clone, Copy)]
pub struct Termination {
    /// The termination observed through the adapter's protocol mapping.
    pub observed: Outcome,
    /// The adapter's claim, retained in the existing `adapterClaimed` field.
    pub adapter_claimed: Outcome,
}

impl From<Outcome> for Termination {
    fn from(outcome: Outcome) -> Self {
        Self {
            observed: outcome,
            adapter_claimed: outcome,
        }
    }
}

/// Conclude an attempt: decide, record the accounting and the outcome.
///
/// `detail` is whatever the caller wants the outcome record to carry for a later
/// fold; spec 005 section 3.9's account is read back from it, so a caller that
/// recorded less would produce an account with more absences, never a wrong one.
pub fn conclude(
    chain: &mut Chain,
    target: &Path,
    session: &Session,
    adapter_said: Outcome,
    accounting: &Accounting,
    detail: serde_json::Value,
    clock: &dyn Clock,
) -> Result<Concluded, SessionError> {
    conclude_observed(
        chain,
        target,
        session,
        adapter_said.into(),
        accounting,
        detail,
        clock,
    )
}

/// Conclude with a mapped termination independent of the adapter's claim.
///
/// Uses the same refusal accounting, base-movement rule and record shape as
/// [`conclude`], whose callers supply the same value for both inputs.
pub fn conclude_observed(
    chain: &mut Chain,
    target: &Path,
    session: &Session,
    termination: Termination,
    accounting: &Accounting,
    detail: serde_json::Value,
    clock: &dyn Clock,
) -> Result<Concluded, SessionError> {
    let timestamp = rfc3339_utc(clock.now_unix());

    // The supervisor's own accounting, recorded as its own entry: spec 003
    // section 3.5 puts it where the supervised process cannot reach it, and a
    // field inside the outcome record would be a field the outcome could be
    // rewritten without.
    chain.append(
        &format!("{}/{}/accounting", session.run_id, session.attempt),
        &timestamp,
        &Entry {
            kind: Kind::Accounting,
            run_id: session.run_id.clone(),
            attempt: session.attempt,
            subject: ACCOUNTING_SUBJECT.to_string(),
            effect_id: Identity::Absent,
            idempotency_key: None,
            detail: serde_json::to_value(accounting).unwrap_or(serde_json::Value::Null),
        },
    )?;

    // Section 3.8: a base revision that moves during an attempt makes the
    // outcome `interrupted`. Observed here, before the workspace is released,
    // because releasing it destroys the evidence.
    let moved = workspace::base_moved(target, &session.workspace, &session.base_revision);
    let decided = if moved {
        Outcome::Interrupted
    } else {
        decide(termination.observed, accounting)
    };

    chain.append(
        &format!("{}/{}/outcome", session.run_id, session.attempt),
        &timestamp,
        &Entry {
            kind: Kind::Outcome,
            run_id: session.run_id.clone(),
            attempt: session.attempt,
            subject: OUTCOME_SUBJECT.to_string(),
            effect_id: Identity::Absent,
            idempotency_key: None,
            detail: merge(
                serde_json::json!({
                    "outcome": decided.word(),
                    "adapterClaimed": termination.adapter_claimed.word(),
                    "refusals": accounting.count,
                    "baseMoved": moved,
                    "baseCommit": session.workspace.base_commit,
                    "workspaceRetained": session.workspace.path.display().to_string(),
                }),
                detail,
            ),
        },
    )?;

    Ok(Concluded {
        run_id: session.run_id.clone(),
        attempt: session.attempt,
        outcome: decided,
        adapter_claimed: termination.adapter_claimed,
        refusals: accounting.count,
        base_moved: moved,
        workspace_retained: session.workspace.path.display().to_string(),
    })
}

fn merge(mut base: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
    if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

/// The attempt a run's fold says is live, if any.
///
/// A convenience over [`runs`], and deliberately not a second source of truth:
/// it folds the same record.
pub fn live_attempt(chain: &Chain) -> Option<Attempt> {
    runs(chain)
        .into_iter()
        .find_map(|r| r.live_attempt().cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_environment::time::FixedClock;

    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "--quiet"]);
        git(dir.path(), &["config", "user.email", "t@example.com"]);
        git(dir.path(), &["config", "user.name", "t"]);
        std::fs::write(dir.path().join("a.txt"), b"a").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "--quiet", "-m", "one"]);
        dir
    }

    #[test]
    fn a_run_that_begins_and_concludes_folds_back_out_of_the_record() {
        let home = tempfile::tempdir().unwrap();
        let target = repo();
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();

        let session = begin(&mut chain, target.path(), "r1", "HEAD", &FixedClock(0)).unwrap();
        assert_eq!(session.attempt, 1);
        // The intent is durable before the workspace exists, so the fold sees a
        // live attempt while the run is still in flight.
        assert!(live_attempt(&chain).is_some());

        let concluded = conclude(
            &mut chain,
            target.path(),
            &session,
            Outcome::Completed,
            &Accounting::default(),
            serde_json::json!({}),
            &FixedClock(1),
        )
        .unwrap();
        assert_eq!(concluded.outcome, Outcome::Completed);
        assert!(live_attempt(&chain).is_none());

        let folded = runs(&chain);
        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].attempts.len(), 1);
        assert_eq!(folded[0].attempts[0].outcome, Some(Outcome::Completed));
    }

    #[test]
    fn a_second_run_while_one_is_live_is_refused_naming_the_live_attempt() {
        let home = tempfile::tempdir().unwrap();
        let target = repo();
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        begin(&mut chain, target.path(), "r1", "HEAD", &FixedClock(0)).unwrap();

        match begin(&mut chain, target.path(), "r2", "HEAD", &FixedClock(1)) {
            Err(SessionError::LiveAttempt { run_id, attempt }) => {
                assert_eq!(run_id, "r1");
                assert_eq!(attempt, 1);
            }
            other => panic!("expected a refusal naming the live attempt, got {other:?}"),
        }
        // And nothing was prepared for the second run.
        assert!(!workspace::workspace_path(target.path(), "r2").exists());
    }

    #[test]
    fn a_retry_appends_a_second_attempt_rather_than_rewriting_the_first() {
        let home = tempfile::tempdir().unwrap();
        let target = repo();
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();

        let first = begin(&mut chain, target.path(), "r1", "HEAD", &FixedClock(0)).unwrap();
        conclude(
            &mut chain,
            target.path(),
            &first,
            Outcome::Failed,
            &Accounting::default(),
            serde_json::json!({}),
            &FixedClock(1),
        )
        .unwrap();

        let second = begin(&mut chain, target.path(), "r1", "HEAD", &FixedClock(2)).unwrap();
        assert_eq!(second.attempt, 2);
        // The same workspace, found rather than rebuilt, which is what
        // `prepare`'s idempotence by run identity is for.
        assert_eq!(second.workspace.path, first.workspace.path);
        let folded = runs(&chain);
        assert_eq!(folded[0].attempts.len(), 2);
        // The first attempt's outcome is untouched.
        assert_eq!(folded[0].attempts[0].outcome, Some(Outcome::Failed));
    }

    #[test]
    fn a_refusal_counted_by_the_supervisor_overrules_a_completed_adapter_claim() {
        let home = tempfile::tempdir().unwrap();
        let target = repo();
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        let session = begin(&mut chain, target.path(), "r1", "HEAD", &FixedClock(0)).unwrap();

        let mut accounting = Accounting::default();
        accounting.observe(crate::refusal::RefusalEvent {
            guard: "permission-deny-rule/Bash".into(),
            detail: "denied".into(),
        });

        let concluded = conclude(
            &mut chain,
            target.path(),
            &session,
            Outcome::Completed,
            &accounting,
            serde_json::json!({}),
            &FixedClock(1),
        )
        .unwrap();
        assert_eq!(concluded.outcome, Outcome::Refused);
        // And the adapter's claim survives beside the decision.
        assert_eq!(concluded.adapter_claimed, Outcome::Completed);
        assert_eq!(concluded.refusals, 1);
    }

    #[test]
    fn an_intent_with_no_outcome_folds_as_a_live_attempt_and_not_as_an_outcome() {
        let home = tempfile::tempdir().unwrap();
        let target = repo();
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        begin(&mut chain, target.path(), "r1", "HEAD", &FixedClock(0)).unwrap();

        let folded = runs(&chain);
        assert_eq!(folded[0].attempts[0].outcome, None);
        assert!(folded[0].attempts[0].live());
        // Reconciliation, not inference, is what resolves it.
        assert_eq!(crate::recovery::unmatched_intents(&chain).len(), 1);
    }

    #[test]
    fn the_fold_reads_the_record_and_nothing_else() {
        let home = tempfile::tempdir().unwrap();
        let target = repo();
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        let session = begin(&mut chain, target.path(), "r1", "HEAD", &FixedClock(0)).unwrap();
        conclude(
            &mut chain,
            target.path(),
            &session,
            Outcome::Completed,
            &Accounting::default(),
            serde_json::json!({}),
            &FixedClock(1),
        )
        .unwrap();

        // Re-opened from bytes, with no memory of the process that wrote them.
        let (reopened, report) = Chain::open(home.path(), target.path()).unwrap();
        assert!(report.records >= 3);
        assert_eq!(runs(&reopened), runs(&chain));
    }
}
