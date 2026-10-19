//! Wire-evidence admission, the ordered procedure: spec 014 section 3.6,
//! through its additive `extends` edge on this crate (spec 005 owns it; spec
//! 014 owns no code).
//!
//! [`admit`] joins the two halves. The steps that need only the reference,
//! the manifest bytes and the recorded policy are `statecraft-envelope::wire`'s;
//! the steps that read the run record are `statecraft-run::wire`'s. Nothing
//! here re-decides either: it orders them, computes the four dimensions and
//! appends one record.
//!
//! [`eligibility`] is section 3.5: whether a policy requires the artifact for
//! an attempt, and if it does, whether acceptance is eligible and why not. It
//! reads the attempt outcome's `witnessRequired` (spec 013 section 7,
//! 2026-10-17) and the attempt's recorded requirement, and **it never
//! rewrites the attempt outcome** or says that the work was wrong.
//!
//! A completed judgment is exactly `admit` or `refuse` with one reason. An
//! operational failure that prevents judgment is [`Unjudged`] and records
//! nothing: it is never converted into an admission or a refusal.

use serde::Serialize;
use serde_json::Value;

use statecraft_envelope::dimensions::{Dimensions, Integrity, SubjectBinding};
use statecraft_envelope::wire::bounded::{self, Json};
use statecraft_envelope::wire::manifest::{Facts, Redaction, Retention, SCHEMA};
use statecraft_envelope::wire::policy::{DimensionName, PermittedAbsence, WirePolicy};
use statecraft_envelope::wire::record::{
    AdmissionRecord, Artifact, Binding, Custody, RECORD_SCHEMA, consistent,
};
use statecraft_envelope::wire::reference::WireReference;
use statecraft_envelope::wire::screen::{SecretDetector, screen};
use statecraft_envelope::wire::{EVIDENCE_TYPE, Outcome, Reason, Stated};
use statecraft_run::record::{Chain, Kind};
use statecraft_run::wire::{
    Earlier, RecordedPolicy, Target, admissions, append_admission, bind, prior, recorded_policy,
};

/// The policy identity recorded when the attempt's policy is ambiguous: there
/// is no one identity to name, and none is chosen.
pub const AMBIGUOUS_POLICY: &str = "ambiguous";

/// The candidate one judgment reads: immutable bytes and their custody.
#[derive(Debug, Clone)]
pub struct Candidate<'a> {
    /// The producer's evidence reference, as it wrote it.
    pub reference: &'a [u8],
    /// The manifest bytes the reference names, held in quarantined memory.
    pub manifest: &'a [u8],
    /// Where the bytes came from and who handled them.
    pub custody: Custody,
}

/// An operational failure that prevented judgment. Nothing was recorded, and
/// the read surface reports the attempt's evidence as `unavailable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unjudged(pub String);

/// Judge `candidate` for `target` and append the one record (section 3.6).
///
/// `detector` is the secret detector to run; the screen refuses unless its
/// identity is exactly the one the attempt's policy records.
///
/// # Errors
///
/// [`Unjudged`] when the record could not be appended. No admission exists.
pub fn admit(
    chain: &mut Chain,
    target: &Target,
    candidate: &Candidate<'_>,
    detector: Option<&dyn SecretDetector>,
    timestamp: &str,
) -> Result<AdmissionRecord, Unjudged> {
    let record = judge(chain, target, candidate, detector);
    append_admission(chain, target, &record, timestamp).map_err(|e| Unjudged(e.to_string()))?;
    Ok(record)
}

/// Steps 1 to 10 over immutable input; step 11 is [`admit`]'s append.
pub fn judge(
    chain: &Chain,
    target: &Target,
    candidate: &Candidate<'_>,
    detector: Option<&dyn SecretDetector>,
) -> AdmissionRecord {
    let recorded = recorded_policy(chain, target);
    let policy = recorded.policy();
    let policy_identity = policy
        .as_ref()
        .map_or_else(|| AMBIGUOUS_POLICY.to_string(), WirePolicy::identity);
    let mut record = AdmissionRecord {
        schema: RECORD_SCHEMA.to_string(),
        artifact: Stated::Unavailable,
        binding: Binding::NotEvaluated,
        facts: Stated::NotRecorded,
        custody: candidate.custody.clone(),
        dimensions: Dimensions::unsigned_today(Integrity::Unknown, SubjectBinding::Unknown),
        outcome: Outcome::Refuse,
        reason: Reason::Malformed,
        policy: policy_identity.clone(),
        prior: None,
        findings: Vec::new(),
        malformed: None,
    };
    let refuse = |mut record: AdmissionRecord, reason: Reason| {
        record.outcome = Outcome::Refuse;
        record.reason = reason;
        record
    };

    // 1. Bounded, strict decoding of the reference.
    let reference = match WireReference::decode(candidate.reference) {
        Ok(r) => r,
        Err(e) => {
            record.malformed = Some(format!("reference: {}", e.detail()));
            return refuse(record, Reason::Malformed);
        }
    };
    record.artifact = Stated::known(Artifact::from(&reference));
    // A manifest the reference says is larger than the bound is refused here,
    // so no caller reads more than `MAX_BYTES` of it.
    if reference.byte_length > bounded::MAX_BYTES as u64 {
        record.malformed = Some("reference: byte_length over the bound".to_string());
        return refuse(record, Reason::Malformed);
    }

    // 2. One supported artifact type, schema version and construction.
    if !reference.supported() {
        return refuse(record, Reason::UnsupportedArtifact);
    }

    // 3. The manifest bytes, their length and digest; then their own bounded
    //    decoding, which holds them only in this function's memory.
    if !reference.as_reference().matches(candidate.manifest) {
        record.dimensions.integrity = Integrity::Fail;
        return refuse(record, Reason::DigestMismatch);
    }
    record.dimensions.integrity = Integrity::Pass;
    let manifest = match bounded::decode(candidate.manifest) {
        Ok(json) => json,
        Err(e) => {
            record.malformed = Some(format!("manifest: {}", e.word()));
            return refuse(record, Reason::Malformed);
        }
    };

    // 4. Exact producer and subject binding, against the run record.
    record.binding = bind(
        chain,
        target,
        &reference.subject,
        &reference.producer,
        &reference.digest,
    );

    // 5. The four dimensions. Nothing here is signed, and no issuer is judged.
    //    `bind` judges every candidate and never answers `not-evaluated`;
    //    were it to, no binding is established, and the record says so in
    //    its binding, its dimension and its reason alike.
    if record.binding == Binding::NotEvaluated {
        record.binding = Binding::SubjectMismatch;
    }
    let refusal = match record.binding {
        Binding::Bound => None,
        Binding::SubjectMismatch | Binding::NotEvaluated => Some(Reason::SubjectMismatch),
        Binding::ProducerMismatch => Some(Reason::ProducerMismatch),
        Binding::Stale => Some(Reason::Stale),
    };
    record.dimensions.subject_binding = if refusal.is_none() {
        SubjectBinding::Pass
    } else {
        SubjectBinding::Fail
    };
    if let Some(reason) = refusal {
        return refuse(record, reason);
    }

    // 6. The durable record: an ambiguous policy, an earlier judgment of the
    //    same identities, or of the same bytes under another policy.
    let Some(policy) = policy else {
        return refuse(record, Reason::PolicyMismatch);
    };
    match prior(
        chain,
        target,
        &reference.digest,
        &policy_identity,
        &reference.subject,
    ) {
        Earlier::None => {}
        Earlier::Duplicate(p) => {
            record.prior = Some(p);
            return refuse(record, Reason::Duplicate);
        }
        Earlier::OtherPolicy(p) => {
            record.prior = Some(p);
            return refuse(record, Reason::PolicyMismatch);
        }
    }

    // 7. The prohibited-content screen, bound to the policy's detector.
    let findings = screen(
        &manifest,
        &SCHEMA,
        policy.secret_detector.as_deref(),
        detector,
    );
    if !findings.is_empty() {
        record.findings = findings;
        return refuse(record, Reason::ProhibitedContent);
    }

    // 8. The retention gate. No separately ratified policy permits
    //    `encrypted-content` in this version, so it is always forbidden.
    let retention = manifest
        .get("retention")
        .and_then(|r| r.get("result"))
        .and_then(Json::as_str);
    if retention == Some("encrypted-content") {
        return refuse(record, Reason::RetentionForbidden);
    }

    // 9. Required facts and cross-field consistency.
    let facts = match Facts::read(&manifest) {
        Ok(f) => f,
        Err(e) => {
            record.malformed = Some(format!("manifest: {}", e.0));
            return refuse(record, Reason::Malformed);
        }
    };
    if let Err(reason) = consistent(&facts, &reference) {
        return refuse(record, reason);
    }
    let verdict = requirement_verdict(&policy, &facts, &record.dimensions);
    record.facts = Stated::known(facts);

    // 10. The exact recorded policy.
    match verdict {
        Some(reason) => refuse(record, reason),
        None => {
            record.outcome = Outcome::Admit;
            record.reason = Reason::Satisfied;
            record
        }
    }
}

/// Step 10: the reason the policy's requirement refuses, or `None`. A policy
/// with no requirement requires nothing; its detector was step 7's.
fn requirement_verdict(
    policy: &WirePolicy,
    facts: &Facts,
    dimensions: &Dimensions,
) -> Option<Reason> {
    let requirement = policy.requirement.as_ref()?;
    if requirement.artifact_type != EVIDENCE_TYPE {
        return Some(Reason::PolicyUnsatisfied);
    }
    if facts.completeness.state < requirement.minimum_completeness {
        return Some(Reason::Incomplete);
    }
    if matches!(
        facts.redaction.result,
        Redaction::Failed | Redaction::Unknown
    ) {
        return Some(Reason::RedactionFailed);
    }
    if matches!(
        facts.retention.result,
        Retention::Failed | Retention::Unknown
    ) {
        return Some(Reason::PolicyUnsatisfied);
    }
    let passes = |d: &DimensionName| match d {
        DimensionName::Integrity => dimensions.integrity == Integrity::Pass,
        DimensionName::Signature => {
            dimensions.signature == statecraft_envelope::dimensions::Signature::Pass
        }
        DimensionName::IssuerTrust => {
            dimensions.issuer_trust == statecraft_envelope::dimensions::IssuerTrust::Pass
        }
        DimensionName::SubjectBinding => dimensions.subject_binding == SubjectBinding::Pass,
    };
    if !requirement.required_dimensions.iter().all(passes) {
        return Some(Reason::PolicyUnsatisfied);
    }
    None
}

/// Section 3.5 for one attempt: whether the artifact is required, the state
/// of the attempt's evidence, and whether acceptance is eligible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireEligibility {
    /// Whether the attempt outcome or the recorded policy requires it.
    pub required: bool,
    /// `admitted`, `refused`, or the absence state: `not-recorded`,
    /// `unavailable` or `unknown`.
    pub state: &'static str,
    /// The reason acceptance is ineligible, when it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ineligible: Option<Reason>,
}

impl WireEligibility {
    /// Whether acceptance may proceed as far as wire evidence is concerned.
    pub fn eligible(&self) -> bool {
        self.ineligible.is_none()
    }
}

/// Section 3.5: read the attempt's requirement and evidence from the chain.
///
/// Without a requirement, absence changes nothing and a refusal is shown but
/// does not gate acceptance. With one, anything but an admitted artifact, or
/// an absence the requirement permits, is ineligible with its specific reason.
pub fn eligibility(chain: &Chain, target: &Target) -> WireEligibility {
    let outcome = attempt_outcome(chain, target);
    let required_by_run = outcome
        .as_ref()
        .and_then(|d| d.get("witnessRequired"))
        .and_then(Value::as_bool)
        == Some(true);
    let policy = recorded_policy(chain, target);
    let requirement = match &policy {
        RecordedPolicy::One(p) => p.requirement.clone(),
        RecordedPolicy::NotRecorded | RecordedPolicy::Ambiguous => None,
    };
    let required = required_by_run || requirement.is_some();
    let mine: Vec<AdmissionRecord> = admissions(chain, &target.run_id)
        .into_iter()
        .filter(|p| p.attempt == target.attempt)
        .filter_map(|p| p.record)
        .collect();

    let (state, reason) = if mine.iter().any(|r| r.outcome == Outcome::Admit) {
        ("admitted", None)
    } else if let Some(last) = mine.last() {
        ("refused", Some(last.reason.clone()))
    } else {
        // A judgment the run path could not make (its manifest unreadable)
        // is `unavailable`, never `not-recorded` (section 3.6).
        let unjudged = outcome
            .as_ref()
            .and_then(|d| d.get("witness"))
            .and_then(|w| w.get("wireEvidence"))
            .and_then(|w| w.get("unavailable"))
            .is_some();
        let absence = match outcome
            .as_ref()
            .and_then(|d| d.get("witnessPosture"))
            .and_then(Value::as_str)
        {
            _ if unjudged => PermittedAbsence::Unavailable,
            Some("witness-unavailable") => PermittedAbsence::Unavailable,
            Some("witness-failed") => PermittedAbsence::Unknown,
            _ => PermittedAbsence::NotRecorded,
        };
        let permitted = requirement
            .as_ref()
            .is_some_and(|r| r.permitted_absence.contains(&absence));
        let word = match absence {
            PermittedAbsence::NotRecorded => "not-recorded",
            PermittedAbsence::Unavailable => "unavailable",
            PermittedAbsence::Unknown => "unknown",
        };
        (word, (!permitted).then_some(Reason::PolicyUnsatisfied))
    };
    let ineligible = if !required {
        None
    } else if matches!(policy, RecordedPolicy::Ambiguous) {
        Some(Reason::PolicyMismatch)
    } else {
        reason
    };
    WireEligibility {
        required,
        state,
        ineligible,
    }
}

/// The attempt outcome's detail, written once by spec 003's session.
fn attempt_outcome(chain: &Chain, target: &Target) -> Option<Value> {
    chain
        .entries()
        .into_iter()
        .find(|e| {
            e.kind == Kind::Outcome
                && e.subject == statecraft_run::session::OUTCOME_SUBJECT
                && e.run_id == target.run_id
                && e.attempt == target.attempt
        })
        .map(|e| e.detail)
}
