//! Spec 047 section 3.4: a fixture adapter with a different declaration
//! proves the seam.
//!
//! It declares no stream witness, no print-mode hooks and no sidecar. Through
//! [`Needs::launch`] and [`fit`], the code `run` reads for every adapter, a
//! witnessed run with it is refused naming `stream_witness`, and a plain run
//! is admitted. It runs no provider, and nothing here asks which adapter it is.

use statecraft_adapter::capabilities::{
    AdapterCapabilities, Field, Needs, REASON, Unregistered, fit, register,
};
use statecraft_adapter::witness::posture::Requirement;

/// The fixture's declaration, supplied as data, so registration is exercised.
fn fixture() -> AdapterCapabilities {
    register(&serde_json::json!({
        "streamWitness": false,
        "printModeHooks": false,
        "wireSidecar": false,
        "perRunConfigDir": true,
        "settingsFileTransport": true,
        "versionField": null,
    }))
    .expect("the fixture sets every field")
}

const REQUIRED: Requirement = Requirement {
    requested: true,
    required: true,
};

const REQUESTED: Requirement = Requirement {
    requested: true,
    required: false,
};

#[test]
fn a_witnessed_run_is_refused_naming_stream_witness() {
    let fitted = fit(&fixture(), &Needs::launch(REQUIRED, false));
    let refusal = fitted.refusal("fixture-adapter").expect("refused");
    assert_eq!(refusal.fields, [Field::StreamWitness, Field::WireSidecar]);
    let text = refusal.to_string();
    assert!(text.starts_with(REASON), "{text}");
    assert!(text.contains("stream_witness"), "{text}");
    assert!(text.contains("before spawn"), "{text}");
}

#[test]
fn a_plain_run_is_admitted() {
    let fitted = fit(&fixture(), &Needs::launch(Requirement::DEFAULT, false));
    assert!(!fitted.refuses(), "{fitted:?}");
    assert!(fitted.degraded.is_empty());
    assert_eq!(
        fitted.witness_requirement(Requirement::DEFAULT),
        Requirement::DEFAULT
    );
}

#[test]
fn a_run_that_only_requests_a_witness_runs_degraded_and_unwitnessed() {
    let fitted = fit(&fixture(), &Needs::launch(REQUESTED, false));
    assert!(!fitted.refuses());
    assert_eq!(fitted.degraded, [Field::StreamWitness, Field::WireSidecar]);
    assert_eq!(fitted.witness_requirement(REQUESTED), Requirement::DEFAULT);
}

#[test]
fn a_managed_run_is_refused_naming_print_mode_hooks() {
    let fitted = fit(&fixture(), &Needs::launch(Requirement::DEFAULT, true));
    let refusal = fitted.refusal("fixture-adapter").expect("refused");
    assert_eq!(refusal.fields, [Field::PrintModeHooks]);
}

#[test]
fn a_declaration_with_a_field_unset_is_refused_at_registration() {
    let refused = register(&serde_json::json!({
        "printModeHooks": false,
        "wireSidecar": false,
        "perRunConfigDir": true,
        "settingsFileTransport": true,
        "versionField": null,
    }));
    assert_eq!(
        refused,
        Err(Unregistered::Unset {
            field: "stream_witness"
        })
    );
}
