//! Spec 006 section 5, 2026-09-25: the JSON naming convention, enforced on
//! what the binary prints.
//!
//! Keys this product authors are camelCase, `^[a-z][a-zA-Z0-9]*$`; string enum
//! values are kebab-case. Every binary test that parses `--json` output parses
//! it through [`from_output`] or [`from_text`], which walk the whole value and
//! fail on a key outside the rule unless [`GRANDFATHERED`] names it. The
//! source-level half is `tests/json_naming.rs`, which reads the same list.
#![allow(dead_code)]

use serde_json::Value;

/// A document whose names predate the convention, or are someone else's.
pub struct Grandfathered {
    /// What the document is.
    pub document: &'static str,
    /// Why its names do not move in this change.
    pub why: &'static str,
    /// Declarations the source scan excuses, as `crate/src/file.rs::Type`.
    pub types: &'static [&'static str],
    /// Keys the walker excuses wherever they appear.
    pub keys: &'static [&'static str],
}

/// The one exemption list. **It only shrinks**: an entry leaves when its
/// document moves to the convention with its owning spec's `schemaVersion`
/// bump, and `every_grandfathered_type_still_needs_its_exemption` refuses an
/// entry whose type no longer needs it. Adding one is a spec 006 section 5
/// decision, not a way to make a test pass.
pub const GRANDFATHERED: &[Grandfathered] = &[
    Grandfathered {
        document: "the environment manifest and its transfer journal (spec 002)",
        why: "committed in every adopting repository as `.statecraft/environment.json`, \
              including the `project.setup` parameters; a rename is a schema change for \
              every existing file and moves only with a manifest `schemaVersion` bump",
        types: &[
            "statecraft-environment/src/manifest.rs::Transfer",
            "statecraft-environment/src/manifest.rs::Entry",
            "statecraft-environment/src/manifest.rs::Pins",
            "statecraft-environment/src/manifest.rs::Project",
            "statecraft-environment/src/manifest.rs::Modification",
            "statecraft-environment/src/manifest.rs::Written",
            "statecraft-environment/src/transfer.rs::TransferRecord",
            "statecraft-home/src/setup.rs::Parameters",
        ],
        keys: &[
            "digest_at_transfer",
            "evaluated_against",
            "written_at",
            "spec_spine",
            "shared_approval_required",
            "digest_before",
            "digest_after",
            "removed_leftovers",
            "operator_provenance",
            "manifest_before",
            "default_branch",
            "diff_cap",
            "release_branch_pattern",
            "code_owners",
            "extra_required_jobs",
            "enforce_coverage",
            "authored_content",
            "authored_content_text",
            "gate_each_commit",
            "require_signed_commits",
            "require_default_base",
            "fail_on_unresolved",
        ],
    },
    Grandfathered {
        document: "the transfer plan's identity (spec 002, spec 006 section 5)",
        why: "`plan_id` is named by both specs as the field `transfer apply` is given; \
              the rest of the plan is camelCase",
        types: &["statecraft-environment/src/transfer.rs::Plan"],
        keys: &["plan_id"],
    },
    Grandfathered {
        document: "the project register, `projects.json` (spec 002)",
        why: "persisted in the product home; a qualification reason that carries data \
              is written with its kebab-case variant name as the key",
        types: &[],
        keys: &[
            "corpus-does-not-compile",
            "corpus-check-unavailable",
            "corpus-check-not-performed",
        ],
    },
    Grandfathered {
        document: "the setup profile's six results (spec 002)",
        why: "the result names are specified as kebab-case identifiers in spec 002's \
              results table; renaming the keys is an amendment of that table",
        types: &["statecraft-home/src/setup.rs::Results"],
        keys: &[
            "files-installed",
            "local-checks",
            "remote-prerequisites",
            "required-checks",
            "ci-executed",
            "ai-review-produced",
        ],
    },
    Grandfathered {
        document: "the startup and trial records (spec 002)",
        why: "recorded under `.statecraft/state/` and read back; `session_id`, \
              `hook_name` and `exit_code` copy the provider's hook-event names",
        types: &[
            "statecraft-home/src/launch.rs::HarnessObservation",
            "statecraft-home/src/trial.rs::Entry",
            "statecraft-home/src/required.rs::Finding",
        ],
        keys: &["session_id", "hook_name", "exit_code", "rel_path"],
    },
    Grandfathered {
        document: "the run record and journal (spec 003)",
        why: "hash-chained and never rewritten; an old binary must read a run a newer \
              one wrote, so a rename moves only with the record's `schemaVersion`",
        types: &[
            "statecraft-run/src/attempt.rs::Attempt",
            "statecraft-run/src/policy.rs::PolicySource",
            "statecraft-run/src/policy.rs::Policy",
            "statecraft-run/src/policy.rs::Override",
            "statecraft-run/src/record.rs::Entry",
            "statecraft-run/src/refusal.rs::Accounting",
            "statecraft-run/src/session.rs::Concluded",
            "statecraft-run/src/work.rs::WorkItem",
            "statecraft-run/src/work.rs::WorkList",
            "statecraft-run/src/workspace.rs::Workspace",
        ],
        keys: &[
            "run_id",
            "base_commit",
            "admitted_by_override",
            "policy_was_declared",
            "as_written",
            "schedulable_statuses",
            "spec_id",
            "operator_provenance",
            "granted_at",
            "grant_line",
            "idempotency_key",
            "tamper_attempts",
            "adapter_claimed",
            "base_moved",
            "workspace_retained",
            "from_field",
            "status_from",
            "policy_disagreement",
            "spec_spine_version",
        ],
    },
    Grandfathered {
        document: "the adapter protocol, posture and qualification records (spec 004)",
        why: "the protocol is the wire contract an adapter binary speaks; the posture \
              is recorded with every attempt; qualification records are persisted in \
              the home's `qualifications.json`",
        types: &[
            "statecraft-adapter/src/protocol.rs::Request",
            "statecraft-adapter/src/protocol.rs::AttemptIdentity",
            "statecraft-adapter/src/protocol.rs::AdapterResult",
            "statecraft-adapter/src/posture.rs::Posture",
            "statecraft-adapter/src/manifest.rs::QualificationRecord",
            "statecraft-adapter-claude-code/src/qualification.rs::ProviderPair",
            "statecraft-adapter-claude-code/src/qualification.rs::PairedRecord",
        ],
        keys: &[
            "base_commit",
            "deadline_seconds",
            "run_id",
            "adapter_version",
            "provider_version",
            "unverifiable_refusal_account",
            "surviving_processes",
            "binary_version",
            "suite_version",
        ],
    },
    Grandfathered {
        document: "the provider stream mirror (spec 004)",
        why: "the provider's names, kept as the provider spells them",
        types: &["statecraft-adapter-claude-code/src/stream.rs::PermissionDenial"],
        keys: &["tool_name", "tool_use_id", "tool_input"],
    },
    Grandfathered {
        document: "acceptance and portable evidence (spec 005)",
        why: "receipts, outcomes and envelopes are portable and read by verifiers; \
              section 3.4 makes an added field compatible, so a renamed one is a \
              breaking change that moves only with a spec 005 schema version",
        types: &[
            "statecraft-acceptance/src/authority.rs::DeclaredMember",
            "statecraft-acceptance/src/authority.rs::Verdict",
            "statecraft-acceptance/src/independence.rs::Check",
            "statecraft-acceptance/src/independence.rs::SuiteResult",
            "statecraft-acceptance/src/judged.rs::Candidate",
            "statecraft-acceptance/src/judged.rs::NoAcceptance",
            "statecraft-acceptance/src/outcome.rs::Acceptance",
            "statecraft-acceptance/src/outcome.rs::Sourced",
            "statecraft-acceptance/src/outcome.rs::ReviewableOutcome",
            "statecraft-acceptance/src/receipt.rs::Receipt",
            "statecraft-acceptance/src/receipt.rs::SuiteEntry",
            "statecraft-acceptance/src/receipt.rs::NoReceipt",
            "statecraft-acceptance/src/trust.rs::Anchor",
            "statecraft-acceptance/src/trust.rs::VerifierRecord",
            "statecraft-envelope/src/dimensions.rs::Dimensions",
            "statecraft-envelope/src/dimensions.rs::AdmissionPolicy",
            "statecraft-envelope/src/reference.rs::Reference",
            "statecraft-envelope/src/roots.rs::Root",
            "statecraft-envelope/src/roots.rs::RootSet",
            "statecraft-envelope/src/verdict.rs::EvidenceVerdict",
            "statecraft-envelope/src/verdict.rs::VerifiedUnder",
        ],
        keys: &[
            "path_prefix",
            "repository_members_touched",
            "environment_manifest_touched",
            "corpus_members",
            "corpus_classes",
            "prior_policy_required",
            "authority_change",
            "may_accept_on_own_suite",
            "exit_code",
            "structured_pass",
            "agent_claim",
            "work_tree_clean",
            "head_stable",
            "dirty_paths",
            "unrun_checks",
            "refusal_count",
            "from_record",
            "run_id",
            "independent_result",
            "receipt_freshness",
            "policy_digest",
            "product_version",
            "spec_spine_version",
            "adapter_version",
            "harness_revision",
            "authority_paths_touched",
            "root_id",
            "root_set",
            "stopped_early",
            "issuer_trust",
            "subject_binding",
            "require_integrity",
            "require_signature",
            "require_issuer_trust",
            "policy_version",
            "require_artifacts",
            "require_subject_binding",
            "min_approvals",
            "approvers_distinct",
            "approver_may_not_be_submitter",
            "evidence_type",
            "schema_version",
            "producer_digest",
            "key_id",
            "public_key",
            "not_before",
            "not_after",
            "root_set_version",
            "signed_by",
            "verdict_version",
            "verified_under",
            "incomplete-evidence",
        ],
    },
];

/// The key rule.
pub fn is_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_alphanumeric())
}

/// The enum-value rule: kebab-case, one lowercase word included.
pub fn is_value(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|w| {
            let mut c = w.chars();
            matches!(c.next(), Some(f) if f.is_ascii_lowercase())
                && c.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn exempt(key: &str) -> bool {
    GRANDFATHERED.iter().any(|g| g.keys.contains(&key))
}

/// Every key in `value` that breaks the rule and is not grandfathered, with
/// the path it was found at.
pub fn violations(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk(value, "$", &mut out);
    out
}

fn walk(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if !is_key(k) && !exempt(k) {
                    out.push(format!("{path}: {k:?}"));
                }
                walk(v, &format!("{path}.{k}"), out);
            }
        }
        Value::Array(items) => {
            for v in items {
                walk(v, &format!("{path}[]"), out);
            }
        }
        _ => {}
    }
}

/// Fail when `value` carries a key outside the convention.
pub fn assert_conforms(value: &Value) {
    let v = violations(value);
    assert!(
        v.is_empty(),
        "--json output breaks spec 006's JSON naming convention (camelCase \
         keys); rename the field, or grandfather a persisted document in \
         tests/support/json_naming.rs with its reason:\n{}",
        v.join("\n")
    );
}

/// Parse `--json` output from bytes, and hold it to the convention.
pub fn from_output(bytes: &[u8]) -> serde_json::Result<Value> {
    let v: Value = serde_json::from_slice(bytes)?;
    assert_conforms(&v);
    assert_envelope(&v);
    Ok(v)
}

/// The family `error.kind` tokens (spec 007 section 3.2), spelled out here
/// rather than read from the crate, so a token the crate adds fails a test.
pub const ERROR_KINDS: [&str; 10] = [
    "validation",
    "stale",
    "not-found",
    "drift",
    "refused",
    "config",
    "io",
    "schema",
    "usage",
    "internal",
];

/// Spec 007 section 3.1, held on every answer a binary test parses: the
/// header, `outcome` and `exitCode` naming one exit, and exactly one of
/// `report` (0 or 1) and `error` (2, 3 or 4) with a kind from the closed set.
pub fn assert_envelope(v: &Value) {
    let words = ["ok", "finding", "refused", "usage", "failed"];
    assert_eq!(v["schemaVersion"], "1.0.0", "not the family envelope: {v}");
    assert_eq!(v["tool"], "statecraft-cli", "{v}");
    assert!(v["verb"].as_str().is_some_and(|s| !s.is_empty()), "{v}");
    assert!(v["summary"].is_string(), "{v}");
    let code = v["exitCode"]
        .as_u64()
        .unwrap_or_else(|| panic!("no exitCode: {v}")) as usize;
    assert_eq!(
        v["outcome"], words[code],
        "outcome and exitCode disagree: {v}"
    );
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    for k in &keys {
        assert!(
            [
                "schemaVersion",
                "tool",
                "verb",
                "outcome",
                "exitCode",
                "summary",
                "report",
                "error"
            ]
            .contains(k),
            "a member outside the envelope, {k}: {v}"
        );
    }
    if code <= 1 {
        assert!(v.get("report").is_some() && v.get("error").is_none(), "{v}");
    } else {
        assert!(v.get("report").is_none(), "{v}");
        let kind = v["error"]["kind"]
            .as_str()
            .unwrap_or_else(|| panic!("no error.kind: {v}"));
        assert!(
            ERROR_KINDS.contains(&kind),
            "error.kind outside the set: {v}"
        );
        assert!(v["error"]["message"].is_string(), "{v}");
    }
}

/// What the answer carries beside its header: `report` for 0 and 1, and
/// `error.details` for 2, 3 and 4, for a helper that serves both.
pub fn payload(v: &Value) -> &Value {
    match v.get("report") {
        Some(report) => report,
        None => &v["error"]["details"],
    }
}

/// Parse `--json` output from text, and hold it to the convention.
pub fn from_text(text: &str) -> serde_json::Result<Value> {
    let v: Value = serde_json::from_str(text)?;
    assert_conforms(&v);
    assert_envelope(&v);
    Ok(v)
}
