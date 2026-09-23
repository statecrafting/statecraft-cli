//! Reconciling an unresolved attempt, as an operator's act.
//!
//! Spec 003 section 3.6.1. An attempt whose intent has no outcome holds the
//! repository's one live slot (section 3.7) until it is reconciled. This module
//! records one reconciliation: the operator's **declaration** of whether the
//! attempt's governed work took effect, beside this product's **observation** of
//! what its own launch records say. It never launches, signals, replays or
//! re-runs anything to find out; the observation is read by the caller from
//! spec 002's launch records and passed in as facts.
//!
//! What each finding does (rule 4): `unknown` leaves the attempt live; `confirmed`
//! and `absent` resolve it, and [`crate::session::runs`] then reads its outcome
//! as `interrupted`, with the reconciliation beside it.

use crate::record::{Chain, Entry, Identity, Kind};
use crate::recovery::Verdict;
use crate::session::{INTENT_SUBJECT, runs};
use serde::{Deserialize, Serialize};

/// The word the record carries for a declaration.
pub const BASIS: &str = "operator-declared";

/// The word every reconciliation carries beside the operator's name.
pub const OPERATOR_PROVENANCE: &str = "operator-supplied";

/// The launch state the observation read (spec 002 section 3.32 rule 23), or
/// `unrecorded` for a repository that keeps no launch records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchState {
    /// No `intent.json`: no spawn was attempted.
    NotLaunched,
    /// An `intent.json` and nothing after it.
    LaunchUnknown,
    /// A confirmed spawn and no completion.
    OutcomeUnknown,
    /// The spawn call reported no process was created.
    SpawnFailed,
    /// A process created and stopped before its prompt, or one that did not
    /// end as one readable session.
    Interrupted,
    /// A completed launch record, with the verdict word it carries.
    Completed(String),
    /// A repository with no manifest keeps no launch records.
    Unrecorded,
}

impl LaunchState {
    /// Read a state word as `startup show` prints it.
    pub fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "not-launched" => LaunchState::NotLaunched,
            "launch-unknown" => LaunchState::LaunchUnknown,
            "outcome-unknown" => LaunchState::OutcomeUnknown,
            "spawn-failed" => LaunchState::SpawnFailed,
            "interrupted" => LaunchState::Interrupted,
            "unrecorded" => LaunchState::Unrecorded,
            "mismatched" | "not-admitted" | "unverified" | "qualified" => {
                LaunchState::Completed(word.to_string())
            }
            _ => return None,
        })
    }

    /// The word.
    pub fn word(&self) -> &str {
        match self {
            LaunchState::NotLaunched => "not-launched",
            LaunchState::LaunchUnknown => "launch-unknown",
            LaunchState::OutcomeUnknown => "outcome-unknown",
            LaunchState::SpawnFailed => "spawn-failed",
            LaunchState::Interrupted => "interrupted",
            LaunchState::Completed(w) => w,
            LaunchState::Unrecorded => "unrecorded",
        }
    }
}

/// What this product observed, read-only, at the moment of writing (rule 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// The launch state.
    pub launch_state: LaunchState,
    /// Whether the attempt's `gate.log` records a released tool call. An absent
    /// or empty log proves nothing and reads `false`.
    pub gate_released_tool_call: bool,
    /// The process id `launched.json` confirmed, where it did.
    pub confirmed_pid: Option<u32>,
    /// Whether a process with that id exists now. An observation, not an
    /// identification.
    pub pid_exists: Option<bool>,
    /// The files the observation was read from.
    pub files: Vec<String>,
}

/// One file the operator offered as supporting evidence. Its content is not
/// copied and not read for a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceFile {
    /// The path, as given.
    pub path: String,
    /// Its length in bytes.
    pub bytes: u64,
    /// Its SHA-256.
    pub sha256: String,
}

/// What the operator asked for.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The run.
    pub run_id: &'a str,
    /// The attempt.
    pub attempt: u32,
    /// The finding.
    pub finding: Verdict,
    /// The launch state the operator inspected, as a word.
    pub inspected: &'a str,
    /// Who, as supplied.
    pub operator: &'a str,
    /// Why.
    pub reason: &'a str,
    /// Supporting files.
    pub evidence: Vec<EvidenceFile>,
    /// Now, RFC 3339 UTC.
    pub at: &'a str,
}

/// A reconciliation as the record carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reconciliation {
    /// The finding, under the key the existing reader renders.
    pub verdict: Verdict,
    /// Always [`BASIS`].
    pub basis: String,
    /// Whether this product's own write order establishes the finding
    /// independently (rule 2): only `absent` against `not-launched` or
    /// `spawn-failed`.
    pub corroborated: bool,
    /// Who, as supplied.
    pub operator: String,
    /// Always [`OPERATOR_PROVENANCE`].
    pub operator_provenance: String,
    /// Why.
    pub reason: String,
    /// Supporting files, by digest.
    pub evidence: Vec<EvidenceFile>,
    /// What this product observed.
    pub observed: Observation,
    /// Whether a later `run` is permitted (rule 4).
    pub retry_allowed: bool,
    /// Whether the effect is idempotent by a key (section 3.6).
    pub idempotent_by_key: bool,
    /// The intent's idempotency key, where it had one.
    pub idempotency_key: Option<String>,
    /// The chain position of the `unknown` reconciliation this replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<usize>,
    /// When it was written.
    pub at: String,
}

impl Reconciliation {
    /// Whether this resolves the attempt.
    pub fn conclusive(&self) -> bool {
        matches!(self.verdict, Verdict::Confirmed | Verdict::Absent)
    }

    /// One line an operator reads.
    pub fn describe(&self) -> String {
        format!(
            "reconciled {} by {} ({}, {}{}), observed {}: {}",
            match self.verdict {
                Verdict::Confirmed => "confirmed",
                Verdict::Absent => "absent",
                Verdict::Unknown => "unknown",
            },
            self.operator,
            self.operator_provenance,
            self.basis,
            if self.corroborated {
                ", corroborated by this product's launch records"
            } else {
                ", not corroborated"
            },
            self.observed.launch_state.word(),
            self.reason
        )
    }
}

/// Read a reconciliation written under section 3.6.1 from a chain entry.
///
/// An older-shape entry (no `basis`) returns `None`: it carries no operator,
/// reason or observation, and never releases an attempt (rule 6).
pub fn read(entry: &Entry) -> Option<Reconciliation> {
    if entry.kind != Kind::Reconciliation {
        return None;
    }
    let r: Reconciliation = serde_json::from_value(entry.detail.clone()).ok()?;
    (r.basis == BASIS).then_some(r)
}

/// Why a reconciliation was not recorded.
#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
    /// A precondition failed; nothing was written.
    #[error("{0}")]
    Refused(String),
    /// The record could not be written.
    #[error(transparent)]
    Record(#[from] crate::record::RecordError),
}

/// Record one reconciliation (rules 1 to 5). The caller holds the repository
/// lock and read `observation` just before calling.
pub fn reconcile(
    chain: &mut Chain,
    request: &Request<'_>,
    observation: Observation,
) -> Result<Reconciliation, ReconcileError> {
    let refuse = |why: String| {
        Err(ReconcileError::Refused(format!(
            "{why}; nothing was written"
        )))
    };
    let operator = request.operator.trim();
    let reason = request.reason.trim();
    if operator.is_empty() || reason.is_empty() {
        return refuse("a reconciliation needs a non-empty operator and reason".into());
    }

    // Rule 1: the repository's live attempt, and no other.
    let all = runs(chain);
    let Some(run) = all.iter().find(|r| r.id == request.run_id) else {
        return refuse(format!("the record carries no run `{}`", request.run_id));
    };
    let Some(attempt) = run.attempts.iter().find(|a| a.number == request.attempt) else {
        return refuse(format!(
            "run {} has no attempt {}",
            request.run_id, request.attempt
        ));
    };
    if !attempt.live() {
        return refuse(format!(
            "run {} attempt {} is not live: it has an outcome or a conclusive reconciliation",
            request.run_id, request.attempt
        ));
    }

    // Rule 3: stale, then conflicting.
    if request.inspected != observation.launch_state.word() {
        return refuse(format!(
            "stale: the launch state inspected was `{}`, and it reads `{}` now",
            request.inspected,
            observation.launch_state.word()
        ));
    }
    if request.finding == Verdict::Absent && observation.gate_released_tool_call {
        return refuse(
            "conflicting: the attempt's gate log records a released tool call, so this \
             product's own record says governed work ran, and `absent` contradicts it"
                .into(),
        );
    }

    let entries = chain.entries();
    let intent = entries.iter().find(|e| {
        e.kind == Kind::Intent
            && e.run_id == request.run_id
            && e.attempt == request.attempt
            && e.subject == INTENT_SUBJECT
    });
    let idempotency_key = intent.and_then(|e| e.idempotency_key.clone());
    let replaces = entries.iter().enumerate().rev().find_map(|(i, e)| {
        (e.run_id == request.run_id && e.attempt == request.attempt)
            .then(|| read(e))
            .flatten()
            .map(|_| i)
    });

    let corroborated = request.finding == Verdict::Absent
        && matches!(
            observation.launch_state,
            LaunchState::NotLaunched | LaunchState::SpawnFailed
        );
    let reconciliation = Reconciliation {
        verdict: request.finding,
        basis: BASIS.to_string(),
        corroborated,
        operator: operator.to_string(),
        operator_provenance: OPERATOR_PROVENANCE.to_string(),
        reason: reason.to_string(),
        evidence: request.evidence.clone(),
        observed: observation,
        retry_allowed: request.finding.retry_allowed(),
        idempotent_by_key: idempotency_key.is_some(),
        idempotency_key: idempotency_key.clone(),
        replaces,
        at: request.at.to_string(),
    };
    chain.append(
        &format!(
            "{}/{}/reconciliation/{}",
            request.run_id,
            request.attempt,
            entries.len()
        ),
        request.at,
        &Entry {
            kind: Kind::Reconciliation,
            run_id: request.run_id.to_string(),
            attempt: request.attempt,
            subject: INTENT_SUBJECT.to_string(),
            effect_id: Identity::Absent,
            idempotency_key,
            detail: serde_json::to_value(&reconciliation).unwrap_or_default(),
        },
    )?;
    Ok(reconciliation)
}

/// The attempts reconciled conclusively since the last intent in the record,
/// which the next intent names (rule 4), whichever run they belong to.
pub fn since_last_intent(chain: &Chain) -> Vec<serde_json::Value> {
    let entries = chain.entries();
    let start = entries
        .iter()
        .rposition(|e| e.kind == Kind::Intent)
        .map_or(0, |i| i + 1);
    entries[start..]
        .iter()
        .filter_map(|e| {
            read(e).filter(Reconciliation::conclusive).map(|r| {
                serde_json::json!({
                    "runId": e.run_id,
                    "attempt": e.attempt,
                    "finding": r.verdict,
                })
            })
        })
        .collect()
}

/// Whether a process with this id exists now. An observation: a reused id
/// reads as existing, and nothing is identified by it.
pub fn process_exists(pid: u32) -> bool {
    match rustix::process::Pid::from_raw(pid as i32) {
        Some(p) => !matches!(
            rustix::process::test_kill_process(p),
            Err(rustix::io::Errno::SRCH)
        ),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Chain;
    use statecraft_environment::time::FixedClock;

    fn observed(state: LaunchState) -> Observation {
        Observation {
            launch_state: state,
            gate_released_tool_call: false,
            confirmed_pid: None,
            pid_exists: None,
            files: vec![],
        }
    }

    fn live() -> (tempfile::TempDir, tempfile::TempDir, Chain) {
        let home = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "--quiet"],
            vec![
                "-c",
                "user.email=t@e",
                "-c",
                "user.name=t",
                "commit",
                "--allow-empty",
                "-qm",
                "x",
            ],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(&args)
                    .current_dir(target.path())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        crate::session::begin(&mut chain, target.path(), "r", "HEAD", &FixedClock(0)).unwrap();
        (home, target, chain)
    }

    fn request(finding: Verdict, state: &'static str) -> Request<'static> {
        Request {
            run_id: "r",
            attempt: 1,
            finding,
            inspected: state,
            operator: "alice",
            reason: "looked",
            evidence: vec![],
            at: "2026-09-23T00:00:00Z",
        }
    }

    #[test]
    fn an_older_shape_reconciliation_releases_nothing() {
        let (_h, _t, mut chain) = live();
        chain
            .append(
                "r/1/legacy",
                "2026-09-23T00:00:00Z",
                &crate::recovery::reconciliation_entry(&crate::recovery::Reconciled {
                    intent: crate::recovery::unmatched_intents(&chain).remove(0),
                    verdict: Verdict::Absent,
                }),
            )
            .unwrap();
        assert!(runs(&chain)[0].live_attempt().is_some());
    }

    #[test]
    fn unrecorded_absent_is_a_declaration_and_empty_names_are_refused() {
        let (_h, _t, mut chain) = live();
        let mut r = request(Verdict::Absent, "unrecorded");
        r.operator = " ";
        assert!(matches!(
            reconcile(&mut chain, &r, observed(LaunchState::Unrecorded)),
            Err(ReconcileError::Refused(_))
        ));
        let done = reconcile(
            &mut chain,
            &request(Verdict::Absent, "unrecorded"),
            observed(LaunchState::Unrecorded),
        )
        .unwrap();
        assert!(!done.corroborated);
        assert!(runs(&chain)[0].live_attempt().is_none());
        assert_eq!(since_last_intent(&chain).len(), 1);
    }

    #[test]
    fn every_state_word_reads_back() {
        for w in [
            "not-launched",
            "launch-unknown",
            "outcome-unknown",
            "spawn-failed",
            "interrupted",
            "unrecorded",
            "mismatched",
            "not-admitted",
            "unverified",
            "qualified",
        ] {
            assert_eq!(LaunchState::from_word(w).unwrap().word(), w);
        }
        assert!(LaunchState::from_word("done").is_none());
    }
}
