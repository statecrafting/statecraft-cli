//! Portable fixtures for spec 014's offline acceptance (its section 4).
//!
//! The manifest and the reference are built in exactly the shape wire-witness
//! 0.1.0 writes them (`wire_exchange.rs` and `sidecar_protocol.rs` at tag
//! `v0.1.0`), so the tests in this crate and the crates that build on it judge
//! the producer's real member set without running the producer. Nothing here
//! is a producer: it writes no file and contacts nothing, and every value is
//! an obviously synthetic placeholder.

use serde_json::{Value, json};

use super::Subject;
use crate::hash::sha256_hex;

/// The producer revision the fixtures name.
pub const REVISION: &str = "0.1.0";

/// The producer content digest the fixtures name.
pub fn producer_digest() -> String {
    format!("sha256:{}", "ab".repeat(32))
}

/// A complete, metadata-only manifest for `subject` with one exchange.
pub fn manifest(subject: &Subject) -> Value {
    json!({
        "artifactType": super::EVIDENCE_TYPE,
        "attestation": { "state": "unknown" },
        "capture": { "endDisposition": "supervisor-closed", "startDisposition": "listening" },
        "completeness": { "gaps": [], "state": "complete" },
        "counts": {
            "droppedExchanges": 0,
            "findings": 0,
            "observedExchanges": 1,
            "recordedExchanges": 1
        },
        "exchanges": [{
            "completeness": "complete",
            "estimatedCost": { "state": "not-estimated" },
            "recordByteLength": 10,
            "recordDigest": format!("sha256:{}", "cd".repeat(32)),
            "requested": {
                "destination": "api.provider.example:443",
                "model": { "state": "known", "value": "model-requested" },
                "protocol": "http/1.1"
            },
            "sequence": 1,
            "served": {
                "destination": "api.provider.example:443",
                "model": { "state": "known", "value": "model-served" },
                "protocol": "http/1.1"
            },
            "usage": {
                "label": "provider-testimony",
                "values": [{
                    "location": "$.usage",
                    "providerField": "input_tokens",
                    "unit": "tokens",
                    "value": 3
                }]
            }
        }],
        "hostPolicyId": "policy-1",
        "producer": {
            "contentDigest": producer_digest(),
            "name": "wire-witness",
            "revision": REVISION
        },
        "recordDigestConstruction": super::CONSTRUCTION,
        "redaction": { "fieldClasses": ["sensitive-header"], "reason": null, "result": "applied" },
        "retention": { "reason": null, "requested": "metadata-only", "result": "metadata-only" },
        "schemaVersion": super::SCHEMA_VERSION,
        "subject": {
            "attempt": subject.attempt,
            "runId": subject.run_id,
            "sidecarEffectId": subject.sidecar_effect_id
        }
    })
}

/// The bytes of a manifest value.
pub fn bytes(manifest: &Value) -> Vec<u8> {
    serde_json::to_vec(manifest).expect("a fixture serializes")
}

/// The producer's reference over `manifest_bytes`, in its snake_case form.
pub fn reference(manifest_bytes: &[u8], subject: &Subject) -> Value {
    json!({
        "byte_length": manifest_bytes.len(),
        "completeness": "complete",
        "construction": super::CONSTRUCTION,
        "evidence_type": super::EVIDENCE_TYPE,
        "producer_digest": producer_digest(),
        "producer_identity": format!("wire-witness@{REVISION}"),
        "schema_version": super::SCHEMA_VERSION,
        "sha256": sha256_hex(manifest_bytes),
        "subject": {
            "attempt": subject.attempt,
            "effect_id": subject.sidecar_effect_id,
            "run_id": subject.run_id
        }
    })
}
