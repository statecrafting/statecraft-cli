//! Spec 045 (one YAML reader), R-2: every structural verdict the two reads
//! of `statecraft-home` reached under the previous reader are unchanged under the
//! producer's one reader, `spec_spine_core::types::yaml`, for every workflow
//! the setup and external-code suites read (V-1).
//!
//! The three verdicts are spec 032's two, on a project's reusable code
//! workflow (`workflow_call` declared as a string, a sequence entry or a
//! mapping key; `jobs` a nonempty mapping), and spec 012's one, on the
//! default branch's `statecraft-ci.yml` (the aggregate job's `needs`, a
//! string or a sequence, names a required job). Each is restated here with
//! the shape the product's two sites use, and asserted against the verdict
//! the old reader gave, which is recorded per fixture from the fixture's own
//! content: the inline reusable workflows the suites write, the rendered
//! profile workflows of the registered profile and of every retained earlier
//! revision under `tests/support/`, and the `statecraft-ci.yml` texts the
//! remote-state suite serves. Only the new reader runs here; the point of
//! the change is that the old one is no longer linked.

use spec_spine_core::types::yaml::{self, YamlValue};

/// Spec 032 through `setup.rs`: `on` declares `workflow_call`.
fn declares_workflow_call(doc: &YamlValue) -> bool {
    match doc.get("on") {
        Some(YamlValue::String(name)) => name == "workflow_call",
        Some(YamlValue::Sequence(names)) => names
            .iter()
            .any(|name| name.as_str() == Some("workflow_call")),
        Some(on @ YamlValue::Mapping(_)) => on.get("workflow_call").is_some(),
        _ => false,
    }
}

/// Spec 032 through `setup.rs`: `jobs` is a nonempty mapping.
fn has_jobs(doc: &YamlValue) -> bool {
    doc.get("jobs")
        .and_then(YamlValue::as_mapping)
        .is_some_and(|jobs| !jobs.is_empty())
}

/// Spec 012 through `remote_state.rs`: `jobs.<aggregate>.needs` names `job`.
fn aggregated_by(doc: &YamlValue, aggregate: &str, job: &str) -> bool {
    match doc
        .get("jobs")
        .and_then(|jobs| jobs.get(aggregate))
        .and_then(|a| a.get("needs"))
    {
        Some(YamlValue::String(s)) => s == job,
        Some(YamlValue::Sequence(list)) => list.iter().any(|x| x.as_str() == Some(job)),
        _ => false,
    }
}

/// The reusable workflows the suites write for a project's `ci.code`, with
/// the verdict spec 032's read gave each: accepted, or refused as needing
/// `on: workflow_call` and nonempty `jobs`.
const REUSABLE: [(&str, bool); 8] = [
    // setup_workflows.rs `REUSABLE`.
    (
        "on:\n  workflow_call:\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - run: true\n",
        true,
    ),
    // setup_workflows.rs, the extra and project-code workflows written inline.
    (
        "on:\n  workflow_call:\njobs:\n  x:\n    runs-on: ubuntu-latest\n    steps:\n      - run: true\n",
        true,
    ),
    // external_code.rs `project()`.
    (
        "on:\n  workflow_call:\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - run: scripts/project-code.sh\n",
        true,
    ),
    // external_code.rs, the changed workflow: `workflow_call` as a string.
    (
        "on: workflow_call\njobs:\n  code:\n    runs-on: ubuntu-latest\n    steps:\n      - run: false\n",
        true,
    ),
    // external_code.rs, the three the read refuses: a comment, a string
    // literal, and an empty `jobs`.
    ("# on: workflow_call\non: push\njobs: {}\n", false),
    ("on: push\ncomment: 'on: workflow_call'\njobs: {}\n", false),
    ("on: workflow_call\njobs: {}\n", false),
    // Section 3.2's third form, a sequence entry; no suite fixture writes
    // it, and the read recognises it as it did.
    (
        "on: [push, workflow_call]\njobs:\n  check:\n    runs-on: ubuntu-latest\n",
        true,
    ),
];

#[test]
fn reusable_workflows_keep_their_spec_032_verdict() {
    for (text, accepted) in REUSABLE {
        let doc = yaml::parse(text).unwrap();
        assert_eq!(
            declares_workflow_call(&doc) && has_jobs(&doc),
            accepted,
            "{text}"
        );
    }
}

#[test]
fn an_unparseable_workflow_is_still_refused_as_invalid_yaml() {
    // external_code.rs, the fourth refused text, which the old reader could
    // not parse either; the product reports the reader's message under the
    // same `invalid YAML` refusal (section 3.2).
    assert!(yaml::parse("not: [valid: yaml").is_err());
    // An empty document is `Null`, which declares nothing and has no jobs.
    let empty = yaml::parse("").unwrap();
    assert!(empty.is_null());
    assert!(!declares_workflow_call(&empty));
    assert!(!has_jobs(&empty));
    // A document that is not a mapping indexes to nothing, as the previous
    // reader's `Value["on"]` was `Null` on a sequence.
    let list = yaml::parse("- workflow_call\n").unwrap();
    assert!(!declares_workflow_call(&list));
    assert!(!has_jobs(&list));
}

/// The `statecraft-ci.yml` texts the remote-state suite serves as the
/// default branch's workflow, and what spec 012's read says of `platform`.
#[test]
fn the_remote_state_workflows_keep_their_spec_012_verdict() {
    let aggregated = "jobs:\n  platform:\n    uses: ./.github/workflows/platform.yml\n  ci-gate:\n    needs: [governance, code, platform]\n";
    let not_aggregated = "jobs:\n  platform:\n    uses: ./.github/workflows/platform.yml\n  ci-gate:\n    needs: [governance, code]\n";
    let doc = yaml::parse(aggregated).unwrap();
    assert!(aggregated_by(&doc, "ci-gate", "platform"));
    assert!(aggregated_by(&doc, "ci-gate", "code"));
    assert!(!aggregated_by(&doc, "ci-gate", "ai-review"));
    assert!(!aggregated_by(&doc, "platform", "platform"));
    let doc = yaml::parse(not_aggregated).unwrap();
    assert!(!aggregated_by(&doc, "ci-gate", "platform"));
    // A `needs` written as one string, which the read accepts as before.
    let single = yaml::parse("jobs:\n  ci-gate:\n    needs: governance\n").unwrap();
    assert!(aggregated_by(&single, "ci-gate", "governance"));
    assert!(!aggregated_by(&single, "ci-gate", "code"));
    // No document at all: the read's `unwrap_or` equivalent, `Null`.
    assert!(!aggregated_by(&YamlValue::Null, "ci-gate", "governance"));
}

/// Render a profile template as `setup.rs` renders it: every `{{sc:name}}`
/// replaced, and nothing else touched. A GitHub expression (`${{ ... }}`)
/// is template text. The values are stand-ins of each parameter's shape;
/// `extras` renders one declared extra job, `extra`, as `extra_job_text`
/// renders it, or none.
fn render(body: &str, extras: bool) -> String {
    const MARK: &str = "{{sc:";
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find(MARK) {
        out.push_str(&rest[..start]);
        let after = &rest[start + MARK.len()..];
        let end = after.find("}}").expect("a terminated marker");
        let value = match &after[..end] {
            "ci.extra_jobs" if extras => {
                "\n  extra:\n    name: extra\n    needs: [preflight]\n    uses: ./.github/workflows/extra.yml # zizmor: ignore[self-repository]\n"
            }
            "ci.extra_needs" if extras => ", extra",
            "ci.extra_jobs" | "ci.extra_needs" => "",
            "default_branch" => "main",
            "profile.revision" => "22",
            "profile.identity" => "0123456789abcdef",
            "release.branch_pattern" => "release/*",
            "review.exclude" => "docs/** vendor/**",
            "review.tool_version" => "1.0.0",
            "review.diff_cap"
            | "review.context_tokens"
            | "review.max_calls"
            | "review.deletion_cap" => "1",
            other => panic!("a template names a parameter this test does not render: {other}"),
        };
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Every `statecraft-ci.yml` the suites read: the registered profile's and
/// each retained revision's, with the jobs its `ci-gate` needs.
const CI_WORKFLOWS: [(&str, &str, &[&str]); 10] = [
    (
        "profiles/github-actions-rust/workflows/statecraft-ci.yml",
        include_str!("../profiles/github-actions-rust/workflows/statecraft-ci.yml"),
        &[
            "governance",
            "preflight",
            "code",
            "supply-chain",
            "workflow-lint",
            "sg-scan",
            "secret-scan",
            "ai-review",
            "review-exception",
        ],
    ),
    (
        "tests/support/profile-r4/statecraft-ci.yml",
        include_str!("support/profile-r4/statecraft-ci.yml"),
        &["governance", "code", "ai-review", "review-exception"],
    ),
    (
        "tests/support/profile-r5/statecraft-ci.yml",
        include_str!("support/profile-r5/statecraft-ci.yml"),
        &["governance", "code", "ai-review", "review-exception"],
    ),
    (
        "tests/support/profile-r7/statecraft-ci.yml",
        include_str!("support/profile-r7/statecraft-ci.yml"),
        &["governance", "code", "ai-review", "review-exception"],
    ),
    (
        "tests/support/profile-r11/statecraft-ci.yml",
        include_str!("support/profile-r11/statecraft-ci.yml"),
        &["governance", "code", "ai-review", "review-exception"],
    ),
    (
        "tests/support/profile-r12/statecraft-ci.yml",
        include_str!("support/profile-r12/statecraft-ci.yml"),
        &["governance", "code", "ai-review", "review-exception"],
    ),
    (
        "tests/support/profile-r13/statecraft-ci.yml",
        include_str!("support/profile-r13/statecraft-ci.yml"),
        &["governance", "code", "ai-review", "review-exception"],
    ),
    (
        "tests/support/profile-r17/statecraft-ci.yml",
        include_str!("support/profile-r17/statecraft-ci.yml"),
        &["governance", "code", "ai-review", "review-exception"],
    ),
    (
        "tests/support/profile-r18/statecraft-ci.yml",
        include_str!("support/profile-r18/statecraft-ci.yml"),
        &[
            "governance",
            "preflight",
            "code",
            "ai-review",
            "review-exception",
        ],
    ),
    (
        "tests/support/profile-r21/statecraft-ci.yml",
        include_str!("support/profile-r21/statecraft-ci.yml"),
        &[
            "governance",
            "preflight",
            "code",
            "ai-review",
            "review-exception",
        ],
    ),
];

#[test]
fn every_rendered_ci_workflow_keeps_its_verdicts() {
    for (name, template, needs) in CI_WORKFLOWS {
        let plain = yaml::parse(&render(template, false)).unwrap_or_else(|e| panic!("{name}: {e}"));
        // Spec 032's read on the caller: no `workflow_call`, nonempty jobs.
        assert!(!declares_workflow_call(&plain), "{name}");
        assert!(has_jobs(&plain), "{name}");
        // The `on:` key stays the string `on` under YAML 1.2 booleans
        // (section 3.2), so the triggers are reachable by name.
        let on = plain
            .get("on")
            .and_then(YamlValue::as_mapping)
            .unwrap_or_else(|| panic!("{name}: on"));
        for trigger in ["pull_request", "push", "merge_group"] {
            assert!(
                on.iter().any(|(k, _)| k.as_str() == Some(trigger)),
                "{name}: {trigger}"
            );
        }
        // Spec 012's read: each job the fixture lists is needed by ci-gate,
        // and a job it does not list is not.
        for job in needs {
            assert!(aggregated_by(&plain, "ci-gate", job), "{name}: {job}");
        }
        assert!(!aggregated_by(&plain, "ci-gate", "extra"), "{name}: extra");
        assert!(
            !aggregated_by(&plain, "ci-gate", "ci-gate"),
            "{name}: itself"
        );
        assert!(
            !aggregated_by(&plain, "governance", "code"),
            "{name}: not the aggregate"
        );
        // With one declared extra job rendered, where the revision renders
        // extras at all, ci-gate needs it too and nothing else changes.
        if template.contains("{{sc:ci.extra_needs}}") {
            let with =
                yaml::parse(&render(template, true)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(aggregated_by(&with, "ci-gate", "extra"), "{name}: extra");
            assert!(
                aggregated_by(&with, "extra", "preflight"),
                "{name}: extra needs preflight"
            );
            for job in needs {
                assert!(aggregated_by(&with, "ci-gate", job), "{name}: {job}");
            }
            assert!(!declares_workflow_call(&with), "{name}");
        }
    }
}

/// Every `statecraft-ai-review.yml` the suites read: the registered
/// profile's and each retained revision's, each a reusable workflow with
/// one job, `review`.
const REVIEW_WORKFLOWS: [(&str, &str); 5] = [
    (
        "profiles/github-actions-rust/workflows/statecraft-ai-review.yml",
        include_str!("../profiles/github-actions-rust/workflows/statecraft-ai-review.yml"),
    ),
    (
        "tests/support/profile-r7/statecraft-ai-review.yml",
        include_str!("support/profile-r7/statecraft-ai-review.yml"),
    ),
    (
        "tests/support/profile-r11/statecraft-ai-review.yml",
        include_str!("support/profile-r11/statecraft-ai-review.yml"),
    ),
    (
        "tests/support/profile-r19/statecraft-ai-review.yml",
        include_str!("support/profile-r19/statecraft-ai-review.yml"),
    ),
    (
        "tests/support/profile-r20/statecraft-ai-review.yml",
        include_str!("support/profile-r20/statecraft-ai-review.yml"),
    ),
];

#[test]
fn every_rendered_review_workflow_keeps_its_verdicts() {
    for (name, template) in REVIEW_WORKFLOWS {
        let doc = yaml::parse(&render(template, false)).unwrap_or_else(|e| panic!("{name}: {e}"));
        // A reusable workflow: `workflow_call` as a mapping key, under which
        // the secrets are declared, and one job.
        assert!(declares_workflow_call(&doc), "{name}");
        assert!(has_jobs(&doc), "{name}");
        let jobs = doc.get("jobs").and_then(YamlValue::as_mapping).unwrap();
        assert_eq!(jobs.len(), 1, "{name}");
        assert_eq!(jobs[0].0.as_str(), Some("review"), "{name}");
        assert!(!aggregated_by(&doc, "review", "governance"), "{name}");
        assert!(
            doc.get("on")
                .and_then(|on| on.get("workflow_call"))
                .and_then(|call| call.get("secrets"))
                .and_then(YamlValue::as_mapping)
                .is_some_and(|secrets| !secrets.is_empty()),
            "{name}: declared secrets"
        );
    }
}

/// Spec 045 R-1: the crate depends on no YAML parser but the producer's.
/// The manifest names none, and the workspace lockfile carries neither
/// `serde_yaml` nor its `unsafe-libyaml`, so RUSTSEC-2024-0449 no longer
/// applies to this workspace.
#[test]
fn no_yaml_parser_but_the_producers_is_linked() {
    let manifest =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    assert!(
        !manifest.contains("serde_yaml") && !manifest.contains("serde-saphyr"),
        "statecraft-home names a YAML parser directly"
    );
    let lock =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).unwrap();
    for package in ["serde_yaml", "unsafe-libyaml"] {
        assert!(
            !lock.contains(&format!("name = \"{package}\"")),
            "the lockfile still carries {package}"
        );
    }
}
