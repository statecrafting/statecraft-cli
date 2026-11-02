//! The admission dry-run (spec 049): `admission dry-run --bundle <path>
//! --policy <path>` evaluates a decision bundle's recorded verdicts and
//! decisions under a policy and answers what admission would decide.
//!
//! The decision is `admission::evaluate`'s, through
//! [`statecraft_envelope::admission::evaluate_traced`], so the refusal codes,
//! their order and their serialized form are exactly admission's (section
//! 3.2); beside it, every registered check's own result (section 3.4). The
//! verb reads two files and nothing else: no attestation, record, journal or
//! file is written, no lock is taken and no cell is reached (section 3.3).
//! The bundle is read as `bundle::verify` reads it, without a pinned root set,
//! so no signature is verified here; `hosted decision` is the verification.

use serde_json::{Value, json};
use statecraft_envelope::admission::{AdmissionInput, Decision, evaluate_traced};
use statecraft_envelope::attestation::{AttestationId, Principal, PrincipalKind};
use statecraft_envelope::bundle::{DecisionBundle, admission_inputs, policy_digest};
use statecraft_envelope::dimensions::AdmissionPolicy;
use std::path::Path;

use crate::exit::Exit;
use crate::render::{Answer, ErrorKind};
use crate::trust::{envelope_refusal, parse_opts, read_input, refused, require};

/// What the verb returns: an answer, or a usage message.
pub type Outcome = Result<Answer<Value>, String>;

/// `admission dry-run`'s usage line.
pub const DRY_RUN_USAGE: &str =
    "usage: admission dry-run --bundle <decision-bundle.json> --policy <admission-policy.json>";

/// An input file: a path that names nothing is `not-found`; anything else
/// that stops the read is the trust verbs' refusal.
fn input(path: &Path, what: &str) -> Result<Vec<u8>, Answer<Value>> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(refused(
            ErrorKind::NotFound,
            format!("{what} {} does not exist", path.display()),
            Value::Null,
        )),
        _ => read_input(path, what),
    }
}

/// `admission dry-run --bundle <file> --policy <file>`.
pub fn dry_run(rest: &[String]) -> Outcome {
    let o = parse_opts(rest, &["bundle", "policy"], &[], DRY_RUN_USAGE)?;
    require(&o, &["bundle", "policy"], DRY_RUN_USAGE)?;
    let bundle = match input(&o.path("bundle").expect("given"), "--bundle")
        .and_then(|b| DecisionBundle::parse(&b).map_err(|e| envelope_refusal("--bundle", &e)))
    {
        Ok(b) => b,
        Err(a) => return Ok(a),
    };
    let policy: AdmissionPolicy = match input(&o.path("policy").expect("given"), "--policy")
        .and_then(|b| {
            serde_json::from_slice::<AdmissionPolicy>(&b).map_err(|e| {
                refused(
                    ErrorKind::Schema,
                    format!("--policy is not an admission policy: {e}"),
                    Value::Null,
                )
            })
        }) {
        Ok(p) => p,
        Err(a) => return Ok(a),
    };
    let inputs = match admission_inputs(&bundle) {
        Ok(i) => i,
        Err(why) => {
            return Ok(refused(
                ErrorKind::Validation,
                format!("--bundle does not resolve: {why}"),
                Value::Null,
            ));
        }
    };
    let Some(submitter) = inputs.submitter.as_deref() else {
        return Ok(refused(
            ErrorKind::Validation,
            "--bundle does not resolve: the revision names no submitter",
            Value::Null,
        ));
    };
    let submitter = Principal::new(PrincipalKind::Human, submitter);
    let counted: Vec<Decision> = inputs
        .decisions
        .iter()
        .map(|d| Decision {
            id: AttestationId(d.id),
            approves: d.approves,
            reviewer: d.reviewer.clone(),
        })
        .collect();
    let traced = evaluate_traced(
        &policy,
        &AdmissionInput {
            verdicts: &inputs.verdicts,
            decisions: &counted,
            submitter: &submitter,
        },
    );

    // Section 3.1: the evaluated policy by its digest, and whether it is the
    // one the bundle's recorded admission names.
    let digest = policy_digest(&policy).to_hex();
    let recorded_policy = inputs.recorded.as_ref().and_then(|r| r.policy.clone());
    let relation = match &inputs.recorded {
        None => "none",
        Some(r) if r.policy.as_deref() == Some(digest.as_str()) => "recorded",
        Some(_) => "different",
    };
    let decision = if traced.admission.is_admit() {
        "admit"
    } else {
        "refuse"
    };
    let reasons = traced.admission.reasons();
    let check_count = traced.checks.len();
    let overruled = traced
        .checks
        .iter()
        .filter(|c| {
            matches!(
                c.outcome,
                statecraft_envelope::admission::CheckOutcome::Overruled
            )
        })
        .count();
    let report = json!({
        "revision": bundle.revision.to_hex(),
        "policy": {
            "digest": digest,
            "relation": relation,
            "recordedPolicy": recorded_policy,
        },
        "inputs": {
            "submitter": submitter.id,
            "verdicts": inputs.verdicts.len(),
            "decisions": inputs.decisions.len(),
        },
        "decision": decision,
        "reasons": &reasons,
        "checks": &traced.checks,
    });
    let mut summary = format!(
        "dry-run: {decision} under policy {digest} ({relation}); {} checks, {} denied, {overruled} overruled; nothing was written\n",
        check_count,
        reasons.len()
    );
    for r in &reasons {
        summary.push_str(&format!(
            "refusal: {}\n",
            serde_json::to_string(r).expect("a refusal code serializes")
        ));
    }
    let exit = if traced.admission.is_admit() {
        Exit::Ok
    } else {
        Exit::Finding
    };
    Ok(Answer::new(report, exit, summary))
}
