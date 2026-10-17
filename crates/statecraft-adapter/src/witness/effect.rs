//! The sidecar effect's bracket in the run record, spec 013 section 3.2.
//!
//! [`open`] writes and durably acknowledges one intent carrying the run, the
//! attempt and a fresh effect id **before** the sidecar starts. [`close`]
//! writes the one closing outcome repeating that effect id. The chain's
//! append is fsynced before it returns, so a successful [`open`] is the
//! durable acknowledgement section 3.2 requires.
//!
//! When no closing outcome is durable, spec 003's identity fold
//! ([`statecraft_run::fold_effects`]) reports the intent as open, and
//! reconciliation owns it. Nothing here closes an effect it did not observe.

use crate::witness::ledger::VerifiedProducer;
use crate::witness::protocol::{AttemptBinding, Closing};
use statecraft_run::record::RecordError;
use statecraft_run::{Chain, EffectId, Entry, Identity, Kind};

/// The subject both halves of the bracket carry.
pub const SUBJECT: &str = "witness-sidecar";

/// Mint the fresh effect id for one attempt's sidecar.
///
/// Unique per run because the attempt number is, and a retry is a new
/// attempt: an old effect is never reused.
#[must_use]
pub fn mint(run_id: &str, attempt: u32) -> Option<AttemptBinding> {
    AttemptBinding::new(
        run_id,
        attempt,
        &format!("{run_id}/{attempt}/witness-sidecar"),
    )
}

/// Write the intent. The sidecar may start only after this returns `Ok`.
///
/// # Errors
///
/// The chain's own error; the sidecar must then not start.
pub fn open(
    chain: &mut Chain,
    binding: &AttemptBinding,
    producer: &VerifiedProducer,
    timestamp: &str,
) -> Result<(), RecordError> {
    chain.append(
        &format!("{}/intent", binding.effect_id),
        timestamp,
        &Entry {
            kind: Kind::Intent,
            run_id: binding.run_id.clone(),
            attempt: binding.attempt,
            subject: SUBJECT.to_string(),
            idempotency_key: None,
            detail: serde_json::json!({
                "binding": binding,
                "producer": producer,
            }),
            effect_id: identity(binding),
        },
    )
}

/// Write the one closing outcome.
///
/// # Errors
///
/// The chain's own error; the intent then stays open for reconciliation.
pub fn close(
    chain: &mut Chain,
    binding: &AttemptBinding,
    closing: &Closing,
    timestamp: &str,
) -> Result<(), RecordError> {
    chain.append(
        &format!("{}/outcome", binding.effect_id),
        timestamp,
        &Entry {
            kind: Kind::Outcome,
            run_id: binding.run_id.clone(),
            attempt: binding.attempt,
            subject: SUBJECT.to_string(),
            idempotency_key: None,
            detail: serde_json::to_value(closing)
                .unwrap_or_else(|e| serde_json::json!({ "unserializable": e.to_string() })),
            effect_id: identity(binding),
        },
    )
}

fn identity(binding: &AttemptBinding) -> Identity {
    EffectId::new(&binding.effect_id).map_or(Identity::Absent, Identity::Valid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::witness::WitnessPosture;
    use crate::witness::protocol::{Integrity, Reader, Termination};

    fn producer() -> VerifiedProducer {
        VerifiedProducer {
            name: "wire-witness".into(),
            version: "0.1.0".into(),
            interface: "wire-witness.sidecar/1".into(),
            target: "test-target".into(),
            executable_digest: "sha256:00".into(),
            qualification: "qualification/test.md".into(),
            path: "/bin/true".into(),
        }
    }

    #[test]
    fn an_intent_without_its_outcome_stays_open_and_a_closed_one_is_closed() {
        let home = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        let first = mint("013-run", 1).unwrap();
        open(&mut chain, &first, &producer(), "2026-10-17T00:00:00Z").unwrap();
        let fold = statecraft_run::fold_effects(&chain);
        assert_eq!(fold.open().len(), 1);
        assert_eq!(fold.open()[0].key.effect_id.as_str(), first.effect_id);

        let reader = Reader::new(first.clone());
        let closing = Closing::assemble(
            &reader,
            Termination::Failed,
            Integrity::Missing,
            "wire-witness@0.1.0",
            "wire-witness.sidecar/1",
            WitnessPosture::WitnessFailed,
        );
        close(&mut chain, &first, &closing, "2026-10-17T00:00:01Z").unwrap();
        let fold = statecraft_run::fold_effects(&chain);
        assert!(fold.open().is_empty());
        assert_eq!(fold.closed().len(), 1);
        assert!(fold.is_clean());

        // A retry is a new attempt and a new effect.
        let retry = mint("013-run", 2).unwrap();
        assert_ne!(retry.effect_id, first.effect_id);
        open(&mut chain, &retry, &producer(), "2026-10-17T00:00:02Z").unwrap();
        let fold = statecraft_run::fold_effects(&chain);
        assert_eq!(fold.open().len(), 1);
        assert!(fold.ambiguous().is_empty());
    }

    #[test]
    fn the_sidecar_effect_is_not_an_attempt() {
        let home = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        open(&mut chain, &mint("013-run", 1).unwrap(), &producer(), "t").unwrap();
        let runs = statecraft_run::session::runs(&chain);
        assert!(runs.iter().all(|r| r.attempts.is_empty()));
    }
}
