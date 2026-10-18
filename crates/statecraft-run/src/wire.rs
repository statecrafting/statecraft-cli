//! Wire-exchange evidence in the run record: spec 014, through its additive
//! `extends` edge on this crate (spec 003 owns it; spec 014 owns no code).
//!
//! Three things live here, each read from the chain and nothing else:
//!
//! - [`bind`] is step 4 of spec 014 section 3.6: the artifact's subject must
//!   name the one sidecar effect of its attempt, closed exactly once, under the
//!   producer identity that attempt recorded. The effect is located by spec
//!   003's correlation key `(runId, effectId)`; the attempt is a binding fact
//!   checked beside it, never a replacement key.
//! - The attempt's wire-evidence policy, recorded once ([`record_policy`],
//!   [`recorded_policy`]), and its admission records, appended and read back
//!   with their chain position and record hash ([`append_admission`],
//!   [`admissions`], [`prior`]).
//! - [`project`], the additive per-attempt fold `run show` renders (section
//!   3.7). It performs no admission and reads nothing but the chain.
//!
//! The sidecar bracket is spec 013's. It is read here by its wire shape (the
//! subject `witness-sidecar`, an intent whose detail carries `binding` and
//! `producer`, an outcome whose detail may carry `captureDigest`), so this
//! crate depends on no adapter type.
//!
//! Both record kinds this module writes are [`Kind::Evidence`], which no
//! earlier fold reads: an older reader skips them, and every existing view of
//! a run is unchanged by their presence.

use serde::Serialize;
use serde_json::Value;

use statecraft_envelope::wire::policy::WirePolicy;
use statecraft_envelope::wire::record::{AdmissionRecord, Binding, Prior};
use statecraft_envelope::wire::{Producer, Stated, Subject, bare_digest};

use crate::record::{Chain, Entry, Identity, Kind, RecordError};
use crate::recovery::{EffectKey, FoldDefect, fold_effects};

/// The subject of spec 013's sidecar bracket.
pub const SIDECAR_SUBJECT: &str = "witness-sidecar";
/// The subject of an attempt's wire-evidence policy record.
pub const POLICY_SUBJECT: &str = "wire-evidence-policy";
/// The subject of an admission record.
pub const ADMISSION_SUBJECT: &str = "wire-evidence-admission";

/// The attempt an admission judges, and files its record under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The run.
    pub run_id: String,
    /// The attempt.
    pub attempt: u32,
}

fn sidecar_entries<'c>(
    entries: &'c [(usize, Entry)],
    run_id: &'c str,
    kind: Kind,
) -> impl Iterator<Item = &'c Entry> + 'c {
    entries
        .iter()
        .map(|(_, e)| e)
        .filter(move |e| e.kind == kind && e.subject == SIDECAR_SUBJECT && e.run_id == run_id)
}

fn text<'v>(v: &'v Value, path: &[&str]) -> Option<&'v str> {
    path.iter()
        .try_fold(v, |v, k| v.get(k))
        .and_then(Value::as_str)
}

/// Step 4: bind `subject` and `producer` to the attempt judged, and
/// `manifest_digest` to the effect's closing outcome.
///
/// Returns [`Binding::Bound`], or the one failure: `subject-mismatch` for an
/// absent, invalid, orphaned, ambiguous, unclosed or mismatched effect,
/// `stale` for an effect of an earlier attempt, and `producer-mismatch` for a
/// missing or unequal producer identity.
pub fn bind(
    chain: &Chain,
    target: &Target,
    subject: &Stated<Subject>,
    producer: &Producer,
    manifest_digest: &str,
) -> Binding {
    let Some(subject) = subject.value() else {
        return Binding::SubjectMismatch;
    };
    if subject.run_id != target.run_id || subject.attempt == 0 {
        return Binding::SubjectMismatch;
    }
    if !closed_once(chain, &subject.run_id, &subject.sidecar_effect_id) {
        return Binding::SubjectMismatch;
    }
    let entries = chain.positioned_entries();
    let carries =
        |e: &&Entry| e.effect_id.valid().map(|id| id.as_str()) == Some(&subject.sidecar_effect_id);
    let intents: Vec<&Entry> = sidecar_entries(&entries, &subject.run_id, Kind::Intent)
        .filter(carries)
        .collect();
    let [intent] = intents.as_slice() else {
        return Binding::SubjectMismatch;
    };
    // The one sidecar intent of its attempt, repeating the binding exactly.
    let per_attempt = sidecar_entries(&entries, &subject.run_id, Kind::Intent)
        .filter(|e| e.attempt == intent.attempt)
        .count();
    let binding = intent.detail.get("binding");
    let repeats = binding
        .and_then(|b| b.get("attempt"))
        .and_then(Value::as_u64)
        == Some(u64::from(subject.attempt))
        && binding.and_then(|b| text(b, &["runId"])) == Some(subject.run_id.as_str())
        && binding.and_then(|b| text(b, &["effectId"])) == Some(subject.sidecar_effect_id.as_str());
    if intent.attempt != subject.attempt || per_attempt != 1 || !repeats {
        return Binding::SubjectMismatch;
    }
    // The closing outcome names the capture it closed, when it names one.
    let outcome = sidecar_entries(&entries, &subject.run_id, Kind::Outcome).find(carries);
    let closes = match outcome.map(|o| o.detail.get("captureDigest")) {
        Some(Some(Value::String(d))) => bare_digest(d) == bare_digest(manifest_digest),
        Some(Some(Value::Null) | None) => true,
        _ => false,
    };
    if outcome.is_none_or(|o| o.attempt != subject.attempt) || !closes {
        return Binding::SubjectMismatch;
    }
    if subject.attempt < target.attempt {
        return Binding::Stale;
    }
    if subject.attempt > target.attempt {
        return Binding::SubjectMismatch;
    }
    let recorded = |path: &[&str]| text(&intent.detail, path);
    let same = |stated: &Stated<String>, path: &[&str]| match (stated.value(), recorded(path)) {
        (Some(a), Some(b)) => bare_digest(a) == bare_digest(b),
        _ => false,
    };
    if same(&producer.name, &["producer", "name"])
        && same(&producer.revision, &["producer", "version"])
        && same(&producer.content_digest, &["producer", "executableDigest"])
    {
        Binding::Bound
    } else {
        Binding::ProducerMismatch
    }
}

/// Whether spec 003's identity fold has `(run_id, effect_id)` closed exactly
/// once, by one intent, with no defect naming it.
fn closed_once(chain: &Chain, run_id: &str, effect_id: &str) -> bool {
    let fold = fold_effects(chain);
    let is = |k: &EffectKey| k.run_id == run_id && k.effect_id.as_str() == effect_id;
    fold.closed().iter().any(is)
        && !fold.ambiguous().iter().any(is)
        && !fold.defects().iter().any(|d| match d {
            FoldDefect::DuplicateIntent { key, .. } | FoldDefect::DoubleClose { key, .. } => {
                is(key)
            }
            _ => false,
        })
}

/// Record the attempt's wire-evidence policy. Written once, before the
/// attempt's sidecar starts; a second record for the attempt makes the
/// policy ambiguous rather than replacing the first.
///
/// # Errors
///
/// The chain's own error.
pub fn record_policy(
    chain: &mut Chain,
    target: &Target,
    policy: &WirePolicy,
    timestamp: &str,
) -> Result<(), RecordError> {
    let detail = serde_json::json!({
        "identity": policy.identity(),
        "policy": policy,
    });
    append(chain, target, POLICY_SUBJECT, detail, timestamp)
}

/// The attempt's recorded policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedPolicy {
    /// None recorded: the historical, empty policy applies.
    NotRecorded,
    /// Exactly one.
    One(WirePolicy),
    /// More than one, or one that does not decode.
    Ambiguous,
}

impl RecordedPolicy {
    /// The policy admission judges under, when there is one.
    pub fn policy(&self) -> Option<WirePolicy> {
        match self {
            RecordedPolicy::NotRecorded => Some(WirePolicy::default()),
            RecordedPolicy::One(p) => Some(p.clone()),
            RecordedPolicy::Ambiguous => None,
        }
    }
}

/// Read the attempt's recorded policy.
pub fn recorded_policy(chain: &Chain, target: &Target) -> RecordedPolicy {
    let found: Vec<Option<WirePolicy>> = chain
        .entries()
        .into_iter()
        .filter(|e| {
            e.kind == Kind::Evidence
                && e.subject == POLICY_SUBJECT
                && e.run_id == target.run_id
                && e.attempt == target.attempt
        })
        .map(|e| {
            e.detail
                .get("policy")
                .and_then(|p| serde_json::from_value(p.clone()).ok())
        })
        .collect();
    match found.as_slice() {
        [] => RecordedPolicy::NotRecorded,
        [Some(p)] => RecordedPolicy::One(p.clone()),
        _ => RecordedPolicy::Ambiguous,
    }
}

fn append(
    chain: &mut Chain,
    target: &Target,
    subject: &str,
    detail: Value,
    timestamp: &str,
) -> Result<(), RecordError> {
    let id = format!(
        "{}/{}/{subject}/{}",
        target.run_id,
        target.attempt,
        chain.records().len()
    );
    chain.append(
        &id,
        timestamp,
        &Entry {
            kind: Kind::Evidence,
            run_id: target.run_id.clone(),
            attempt: target.attempt,
            subject: subject.to_string(),
            idempotency_key: None,
            detail,
            effect_id: Identity::Absent,
        },
    )
}

/// Append one admission record, changing no prior record (step 11).
///
/// # Errors
///
/// The chain's own error; nothing is then recorded.
pub fn append_admission(
    chain: &mut Chain,
    target: &Target,
    record: &AdmissionRecord,
    timestamp: &str,
) -> Result<(), RecordError> {
    let detail = serde_json::to_value(record).map_err(|e| RecordError::Unreadable {
        path: String::new(),
        line: 0,
        detail: e.to_string(),
    })?;
    append(chain, target, ADMISSION_SUBJECT, detail, timestamp)
}

/// One admission record as the chain holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Positioned {
    /// Its position in the chain.
    pub position: usize,
    /// Its record hash in the chain.
    pub record_digest: String,
    /// The attempt it is filed under.
    pub attempt: u32,
    /// The record, or `null` when it does not decode.
    pub record: Option<AdmissionRecord>,
}

/// Every admission record of `run_id`, in chain order, readable or not.
pub fn admissions(chain: &Chain, run_id: &str) -> Vec<Positioned> {
    chain
        .positioned_entries()
        .into_iter()
        .filter(|(_, e)| {
            e.kind == Kind::Evidence && e.subject == ADMISSION_SUBJECT && e.run_id == run_id
        })
        .map(|(position, e)| Positioned {
            position,
            record_digest: chain.records()[position].record_hash.clone(),
            attempt: e.attempt,
            record: serde_json::from_value(e.detail).ok(),
        })
        .collect()
}

/// What step 6 finds in the durable record for one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Earlier {
    /// Nothing for this effect: judge it.
    None,
    /// The same manifest, policy and subject were judged: `duplicate`.
    Duplicate(Prior),
    /// The same manifest and subject were judged under another policy:
    /// `policy-mismatch`, and no replacement begins.
    OtherPolicy(Prior),
}

/// Step 6: an earlier admission record of the same manifest and subject.
pub fn prior(
    chain: &Chain,
    target: &Target,
    manifest_digest: &str,
    policy_identity: &str,
    subject: &Stated<Subject>,
) -> Earlier {
    let mut other = None;
    for p in admissions(chain, &target.run_id) {
        let Some(r) = &p.record else { continue };
        let Some(a) = r.artifact.value() else {
            continue;
        };
        if p.attempt != target.attempt
            || a.manifest_digest != manifest_digest
            || &a.subject != subject
        {
            continue;
        }
        let prior = Prior {
            record_digest: p.record_digest.clone(),
            reason: r.reason.clone(),
        };
        if r.policy == policy_identity {
            return Earlier::Duplicate(prior);
        }
        other.get_or_insert(prior);
    }
    other.map_or(Earlier::None, Earlier::OtherPolicy)
}

/// One attempt's wire evidence, as `run show` renders it (section 3.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptEvidence {
    /// The attempt.
    pub attempt: u32,
    /// `recorded` when an admission record exists, else `not-recorded`.
    pub state: &'static str,
    /// Every readable admission record, in chain order.
    pub admissions: Vec<Positioned>,
}

/// A record that did not decode, with its position. It suppresses nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MalformedRecord {
    /// Its position in the chain.
    pub position: usize,
    /// The attempt it is filed under.
    pub attempt: u32,
}

/// The run's wire evidence: every attempt, and every unreadable record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Projection {
    /// One entry per attempt, `not-recorded` where nothing was admitted.
    pub attempts: Vec<AttemptEvidence>,
    /// Admission records that did not decode.
    pub malformed: Vec<MalformedRecord>,
}

/// Fold `run_id`'s wire evidence from the chain alone.
pub fn project(chain: &Chain, run_id: &str) -> Projection {
    let all = admissions(chain, run_id);
    let mut numbers: Vec<u32> = crate::session::runs(chain)
        .into_iter()
        .filter(|r| r.id == run_id)
        .flat_map(|r| r.attempts.into_iter().map(|a| a.number))
        .chain(all.iter().map(|p| p.attempt))
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    let attempts = numbers
        .into_iter()
        .map(|attempt| {
            let admissions: Vec<Positioned> = all
                .iter()
                .filter(|p| p.attempt == attempt && p.record.is_some())
                .cloned()
                .collect();
            AttemptEvidence {
                attempt,
                state: if admissions.is_empty() {
                    "not-recorded"
                } else {
                    "recorded"
                },
                admissions,
            }
        })
        .collect();
    let malformed = all
        .iter()
        .filter(|p| p.record.is_none())
        .map(|p| MalformedRecord {
            position: p.position,
            attempt: p.attempt,
        })
        .collect();
    Projection {
        attempts,
        malformed,
    }
}
