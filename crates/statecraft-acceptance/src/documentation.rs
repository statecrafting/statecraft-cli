//! The inputs a later documentation acceptance contract will read: spec 020
//! section 3.9, through its additive `extends` edge on this crate (spec 005
//! owns it).
//!
//! [`inputs`] decodes the attempt's recorded task, the delivery fold and the
//! retained raw result, and says why the result is ineligible when it is. It
//! judges nothing: until spec 021's contract exists, documentation acceptance
//! is [`Acceptance::NotDeclared`], which is Statecraft's command-level word
//! and never a value of the provider result's `acceptanceResult`. The two are
//! kept apart and labelled by source.

use std::path::Path;

use serde::Serialize;

use statecraft_run::context::read_retained;
use statecraft_run::documentation::delivery::{Delivery, recorded_delivery};
use statecraft_run::documentation::{
    self, DocumentationResult, DocumentationTask, Recorded, ResultProjection, Validation,
};
use statecraft_run::record::Chain;

/// Statecraft's documentation acceptance, while no contract is declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Acceptance {
    /// No documentation acceptance contract exists yet (spec 021).
    NotDeclared,
}

/// The provider result's own sentinels, labelled as the provider's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSentinels {
    /// Always `provider-result`: where these words come from.
    pub source: &'static str,
    /// The result's `reviewDisposition`, as decoded.
    pub review_disposition: statecraft_run::documentation::NotReviewed,
    /// The result's `acceptanceResult`, as decoded.
    pub acceptance_result: statecraft_run::documentation::NotAttempted,
}

/// One attempt's documentation inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Declared {
    /// The task, exactly as the intent recorded it.
    pub task: DocumentationTask,
    /// The delivery fold, if one was recorded.
    pub delivery: Option<Delivery>,
    /// What the run record holds about the result.
    pub result: ResultProjection,
    /// The decoded result, read again from the retained bytes, when it is
    /// bound and its bytes are still under their custody digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decoded: Option<Box<DocumentationResult>>,
    /// The provider's sentinels, when there is a decoded result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderSentinels>,
    /// Why the result could not be considered by a later contract.
    pub ineligible: Vec<String>,
    /// Statecraft's acceptance: `not-declared`.
    pub acceptance: Acceptance,
}

/// What `accept` shows under `documentation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Inputs {
    /// The attempt's intent carries no task; a historical attempt reads so.
    NotRecorded,
    /// The intent carries a value that binds nothing.
    Unreadable {
        /// Why.
        reason: String,
    },
    /// The inputs, and acceptance `not-declared`.
    Declared(Box<Declared>),
}

/// Read one attempt's documentation inputs from the chain and the product
/// home's custody store. Nothing is repaired and no absence reads as success.
pub fn inputs(chain: &Chain, home: &Path, run_id: &str, attempt: u32) -> Inputs {
    let task = match documentation::recorded_task(chain, run_id, attempt) {
        Ok(None) => return Inputs::NotRecorded,
        Ok(Some(t)) => t,
        Err(reason) => return Inputs::Unreadable { reason },
    };
    let delivery = recorded_delivery(chain, &task);
    let result = match documentation::project(chain, run_id, attempt) {
        documentation::Projection::Recorded { result, .. } => result,
        _ => ResultProjection::NotRecorded,
    };
    let mut ineligible = Vec::new();
    match &delivery {
        None => ineligible.push("delivery-not-recorded".to_string()),
        Some(d) if !d.eligible => {
            ineligible.extend(d.findings.iter().map(|f| format!("delivery {f}")))
        }
        Some(_) => {}
    }
    let mut decoded = None;
    match &result {
        ResultProjection::NotRecorded => ineligible.push("result-not-recorded".into()),
        ResultProjection::Duplicate { .. } => ineligible.push("result-duplicate".into()),
        ResultProjection::Unreadable { reason } => {
            ineligible.push(format!("result-unreadable: {reason}"));
        }
        ResultProjection::Recorded(Recorded { raw, validation }) => match validation {
            Validation::SchemaInvalid { reason } => {
                ineligible.push(format!("result-schema-invalid: {reason}"));
            }
            Validation::Unbound { field } => ineligible.push(format!("result-unbound: {field}")),
            Validation::Bound { .. } => match read_retained(home, &raw.custody_digest) {
                Ok(Some(bytes)) => match DocumentationResult::decode(&bytes, &task) {
                    Ok(r) => decoded = Some(Box::new(r)),
                    Err(e) => ineligible.push(format!("result-schema-invalid: {e}")),
                },
                Ok(None) => ineligible.push("result-bytes-not-retained".into()),
                Err(e) => ineligible.push(format!("result-bytes-unreadable: {e}")),
            },
        },
    }
    // Read from the decoded result itself: the closed schema admits only
    // these literals, and whatever the bytes carry is what is shown.
    let provider = decoded.as_ref().map(|r| ProviderSentinels {
        source: "provider-result",
        review_disposition: r.review_disposition,
        acceptance_result: r.acceptance_result,
    });
    Inputs::Declared(Box::new(Declared {
        task,
        delivery,
        result,
        decoded,
        provider,
        ineligible,
        acceptance: Acceptance::NotDeclared,
    }))
}
