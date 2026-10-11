//! Admission is policy (spec 004 B-10 to B-12): a pure function over verdicts
//! and decisions, returning the CLI's `Admission` with the reasons named, and
//! the claim of the `statecraft/policy-eval/v1` attestation that records it.
//!
//! The combinator is action-gate's closed evaluation mode (spec 037): one gate
//! per evaluation, every check required, every deny reported in order. What
//! a policy requires, what a refusal is called and how it serializes stay here.

use std::collections::BTreeSet;
use std::sync::Arc;

use action_gate_core::{ActionContext, Check, Decision as GateDecision, Gate, Outcome};

use crate::attestation::{AttestationId, Principal};
use crate::dimensions::{
    Admission, AdmissionPolicy, Integrity, IssuerTrust, RefusalCode, Signature,
    SignatureRequirement, SubjectBinding,
};
use crate::hash::Hash;
use crate::value::Value;
use crate::verdict::{Evidence, EvidenceVerdict};

/// A decision the policy counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The attestation.
    pub id: AttestationId,
    /// Approval or change request.
    pub approves: bool,
    /// Who.
    pub reviewer: Principal,
}

/// The inputs.
#[derive(Debug, Clone)]
pub struct AdmissionInput<'a> {
    /// Every artifact verdict for the revision, with its attestation id.
    pub verdicts: &'a [(AttestationId, EvidenceVerdict)],
    /// Every decision recorded for the revision.
    pub decisions: &'a [Decision],
    /// Who submitted.
    pub submitter: &'a Principal,
}

/// What a policy requires of a signature, from the CLI booleans plus the extension.
fn signature_requirement(p: &AdmissionPolicy) -> SignatureRequirement {
    if p.require_issuer_trust {
        SignatureRequirement::Trusted
    } else if p.require_signature {
        SignatureRequirement::UnsignedOk
    } else {
        SignatureRequirement::Any
    }
}

/// The action every admission check is evaluated against. The checks own their
/// inputs, so the context carries nothing else.
const ACTION: &str = "statecraft.admission";

/// The reason an admission check allows with.
const SATISFIED: &str = "statecraft:allow:admission:satisfied";

/// One requirement of a policy over one input, as an action-gate check.
///
/// Each check owns the part of the input it judges and answers with at most
/// one refusal. A requirement the policy does not set is satisfied, so a
/// permissive policy admits.
enum Requirement {
    /// `require_artifacts`: an artifact verdict of this type is present.
    Artifact {
        evidence_type: String,
        present: Arc<BTreeSet<String>>,
    },
    /// `require_integrity`, for one verdict.
    Integrity { required: bool, status: Integrity },
    /// The signature, for one verdict, under the policy's requirement.
    Signature {
        requirement: SignatureRequirement,
        status: Signature,
    },
    /// Issuer trust, for one verdict; required only under `Trusted`.
    IssuerTrust {
        requirement: SignatureRequirement,
        status: IssuerTrust,
    },
    /// `require_subject_binding`, for one verdict.
    SubjectBinding {
        required: bool,
        status: SubjectBinding,
    },
    /// `approver_may_not_be_submitter`.
    ApproverIsSubmitter {
        forbidden: bool,
        approvers: Arc<Vec<Principal>>,
        submitter: Principal,
    },
    /// `min_approvals`, counting distinct principals when `approvers_distinct`,
    /// and never the submitter when `approver_may_not_be_submitter`.
    ApprovalCount {
        need: u32,
        distinct: bool,
        exclude_submitter: bool,
        approvers: Arc<Vec<Principal>>,
        submitter: Principal,
    },
}

fn not_passed(dimension: &str, value: &str) -> Option<RefusalCode> {
    Some(RefusalCode::RequiredDimensionNotPassed {
        dimension: dimension.into(),
        value: value.into(),
    })
}

fn unknown(dimension: &str) -> Option<RefusalCode> {
    Some(RefusalCode::DimensionUnknown {
        dimension: dimension.into(),
    })
}

impl Requirement {
    /// The refusal this requirement makes of its input, if any.
    fn refusal(&self) -> Option<RefusalCode> {
        match self {
            Requirement::Artifact {
                evidence_type,
                present,
            } => (!present.contains(evidence_type)).then(|| RefusalCode::MissingRequiredArtifact {
                evidence_type: evidence_type.clone(),
            }),
            Requirement::Integrity { required, status } => match (required, status) {
                (false, _) | (true, Integrity::Pass) => None,
                (true, Integrity::Fail) => not_passed("integrity", "fail"),
                (true, Integrity::Unknown) => unknown("integrity"),
            },
            Requirement::Signature {
                requirement,
                status,
            } => match (requirement, status) {
                (SignatureRequirement::Any, _) => None,
                (SignatureRequirement::UnsignedOk, Signature::Pass | Signature::Unsigned) => None,
                (SignatureRequirement::UnsignedOk, Signature::Fail) => {
                    not_passed("signature", "fail")
                }
                (SignatureRequirement::UnsignedOk, Signature::Unknown) => unknown("signature"),
                (SignatureRequirement::Trusted, Signature::Pass) => None,
                (SignatureRequirement::Trusted, Signature::Unsigned) => {
                    not_passed("signature", "unsigned")
                }
                (SignatureRequirement::Trusted, Signature::Fail) => not_passed("signature", "fail"),
                (SignatureRequirement::Trusted, Signature::Unknown) => {
                    not_passed("signature", "unknown")
                }
            },
            Requirement::IssuerTrust {
                requirement,
                status,
            } => match (requirement, status) {
                (SignatureRequirement::Trusted, IssuerTrust::Fail) => {
                    not_passed("issuerTrust", "fail")
                }
                (SignatureRequirement::Trusted, IssuerTrust::Unknown) => {
                    Some(RefusalCode::IssuerTrustRequired)
                }
                _ => None,
            },
            Requirement::SubjectBinding { required, status } => match (required, status) {
                (false, _) | (true, SubjectBinding::Pass | SubjectBinding::NotApplicable) => None,
                (true, SubjectBinding::Fail) => Some(RefusalCode::SubjectMismatch),
                (true, SubjectBinding::Unknown) => unknown("subjectBinding"),
            },
            Requirement::ApproverIsSubmitter {
                forbidden,
                approvers,
                submitter,
            } => (*forbidden && approvers.contains(submitter))
                .then_some(RefusalCode::ApproverIsSubmitter),
            Requirement::ApprovalCount {
                need,
                distinct,
                exclude_submitter,
                approvers,
                submitter,
            } => {
                let counted = approvers
                    .iter()
                    .filter(|p| !(*exclude_submitter && *p == submitter));
                let have = if *distinct {
                    counted.collect::<BTreeSet<_>>().len() as u32
                } else {
                    counted.count() as u32
                };
                (have < *need).then_some(RefusalCode::ApprovalsInsufficient { have, need: *need })
            }
        }
    }
}

/// A [`Requirement`] under its gate id.
struct AdmissionCheck {
    id: String,
    requirement: Requirement,
}

impl Check for AdmissionCheck {
    fn id(&self) -> &str {
        &self.id
    }

    /// Always decides: an allow when satisfied, otherwise a blocking deny whose
    /// reason is the refusal's canonical JSON, which [`refusal_of`] reads back.
    fn evaluate(&self, _ctx: &ActionContext) -> Option<GateDecision> {
        Some(match self.requirement.refusal() {
            None => GateDecision {
                outcome: Outcome::Allow,
                reason: SATISFIED.into(),
                check_ids: vec![self.id.clone()],
                blocking: false,
            },
            Some(refusal) => GateDecision::deny(
                serde_json::to_string(&refusal).expect("a refusal code serializes"),
                vec![self.id.clone()],
            )
            .blocking(),
        })
    }
}

/// The closed gate for one evaluation (spec 037): one required check per
/// requirement, registered in the order the refusals are reported.
fn gate(policy: &AdmissionPolicy, input: &AdmissionInput<'_>) -> Gate {
    let mut checks = Vec::new();
    let mut add =
        |id: String, requirement: Requirement| checks.push(AdmissionCheck { id, requirement });

    let present: Arc<BTreeSet<String>> = Arc::new(
        input
            .verdicts
            .iter()
            .filter_map(|(_, v)| match &v.evidence {
                Evidence::Artifact { reference } => Some(reference.evidence_type.clone()),
                Evidence::Attestation { .. } => None,
            })
            .collect(),
    );
    for (i, evidence_type) in policy.require_artifacts.iter().enumerate() {
        add(
            format!("artifact/{i}"),
            Requirement::Artifact {
                evidence_type: evidence_type.clone(),
                present: present.clone(),
            },
        );
    }

    let requirement = signature_requirement(policy);
    for (i, (_, v)) in input.verdicts.iter().enumerate() {
        add(
            format!("verdict/{i}/integrity"),
            Requirement::Integrity {
                required: policy.require_integrity,
                status: v.integrity.status,
            },
        );
        add(
            format!("verdict/{i}/signature"),
            Requirement::Signature {
                requirement,
                status: v.signature.status,
            },
        );
        add(
            format!("verdict/{i}/issuer-trust"),
            Requirement::IssuerTrust {
                requirement,
                status: v.issuer_trust.status,
            },
        );
        add(
            format!("verdict/{i}/subject-binding"),
            Requirement::SubjectBinding {
                required: policy.require_subject_binding,
                status: v.subject_binding.status,
            },
        );
    }

    let approvers: Arc<Vec<Principal>> = Arc::new(
        input
            .decisions
            .iter()
            .filter(|d| d.approves)
            .map(|d| d.reviewer.clone())
            .collect(),
    );
    add(
        "approver-is-submitter".into(),
        Requirement::ApproverIsSubmitter {
            forbidden: policy.approver_may_not_be_submitter,
            approvers: approvers.clone(),
            submitter: input.submitter.clone(),
        },
    );
    add(
        "approval-count".into(),
        Requirement::ApprovalCount {
            need: policy.min_approvals,
            distinct: policy.approvers_distinct,
            exclude_submitter: policy.approver_may_not_be_submitter,
            approvers,
            submitter: input.submitter.clone(),
        },
    );

    let ids: Vec<String> = checks.iter().map(|c| c.id.clone()).collect();
    checks
        .into_iter()
        .fold(Gate::builder(), |b, c| b.check(c))
        .require_all(ids)
        .build()
}

/// The refusal a deny names. Every check here always decides and writes its
/// refusal as the reason, so a deny the gate made itself cannot occur; if one
/// ever did, it is kept verbatim as an unknown code, and admission still refuses.
fn refusal_of(deny: &GateDecision) -> RefusalCode {
    serde_json::from_str(&deny.reason)
        .unwrap_or_else(|_| RefusalCode::Unknown(serde_json::Value::String(deny.reason.clone())))
}

/// Evaluate. Deterministic; reads nothing but its arguments.
///
/// One closed action-gate gate (action-gate spec 004, `action-gate-core`
/// 0.3.0) with every check required; every deny, in registration order, is one
/// refusal. Admits exactly when nothing denies.
pub fn evaluate(policy: &AdmissionPolicy, input: &AdmissionInput<'_>) -> Admission {
    let evaluation = gate(policy, input).evaluate_exhaustive(&ActionContext::new(ACTION));
    if evaluation.denials.is_empty() {
        Admission::Admit
    } else {
        Admission::refuse(evaluation.denials.iter().map(refusal_of).collect())
    }
}

/// The claim of the `statecraft/policy-eval/v1` attestation recording an evaluation.
pub fn policy_eval_claim(
    policy_digest: Hash,
    inputs: &[AttestationId],
    admission: &Admission,
) -> Value {
    let (decision, reasons): (&str, Vec<Value>) = match admission {
        Admission::Admit => ("admit", Vec::new()),
        Admission::Refuse { reasons, reason } => {
            let list: Vec<&RefusalCode> = if reasons.is_empty() {
                vec![reason]
            } else {
                reasons.iter().collect()
            };
            (
                "refuse",
                list.iter()
                    .map(|r| {
                        Value::from_json(&serde_json::to_value(r).expect("reason serializes"))
                            .expect("portable")
                    })
                    .collect(),
            )
        }
    };
    Value::map()
        .with("policy", Value::text(policy_digest.to_hex()))
        .unwrap()
        .with(
            "inputs",
            Value::Array(inputs.iter().map(|i| Value::text(i.0.to_hex())).collect()),
        )
        .unwrap()
        .with("decision", Value::text(decision))
        .unwrap()
        .with("reasons", Value::Array(reasons))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::PrincipalKind;
    use action_gate_core::{Mode, closed};

    #[test]
    fn the_gate_is_closed_and_requires_every_check_it_registers() {
        let sam = Principal::new(PrincipalKind::Human, "sam");
        let policy = AdmissionPolicy {
            require_artifacts: vec!["a".into(), "b".into()],
            ..AdmissionPolicy::strict()
        };
        let verdict = crate::verdict::verify_artifact(
            &crate::reference::Reference::over_file_bytes("a", "1", b"bytes"),
            Some(b"bytes"),
            None,
            &crate::roots::RootSet::empty(),
        );
        let verdicts = [
            (AttestationId(Hash::of(b"v0")), verdict.clone()),
            (AttestationId(Hash::of(b"v1")), verdict),
        ];
        let input = AdmissionInput {
            verdicts: &verdicts,
            decisions: &[],
            submitter: &sam,
        };
        let gate = gate(&policy, &input);
        assert_eq!(gate.mode(), Mode::Closed);
        let mut registered = gate.check_ids();
        assert_eq!(
            registered,
            [
                "artifact/0",
                "artifact/1",
                "verdict/0/integrity",
                "verdict/0/signature",
                "verdict/0/issuer-trust",
                "verdict/0/subject-binding",
                "verdict/1/integrity",
                "verdict/1/signature",
                "verdict/1/issuer-trust",
                "verdict/1/subject-binding",
                "approver-is-submitter",
                "approval-count"
            ]
        );
        registered.sort_unstable();
        assert_eq!(gate.required_ids(), registered);
        assert!(gate.unregistered_required().is_empty());
    }

    #[test]
    fn a_deny_the_gate_made_itself_still_refuses() {
        let deny = GateDecision::deny(closed::NO_CHECK_DECIDED, vec![]).blocking();
        assert_eq!(
            refusal_of(&deny),
            RefusalCode::Unknown(serde_json::Value::String(closed::NO_CHECK_DECIDED.into()))
        );
    }

    /// Spec 037 section 3.4 (R-3), measured on the resolved graph rather than
    /// restated: the pin is exact with no default features, the three crates it
    /// brings are the only ones it adds to the envelope's normal closure, that
    /// closure carries no `regex`, and each of the three is Apache-2.0,
    /// forbids unsafe code, has no build script and names no clock,
    /// environment, file system, network, process or thread API.
    #[test]
    fn admission_dependency_is_exact_pure_and_apache() {
        use serde_json::Value as J;
        use std::collections::{BTreeMap, BTreeSet};
        use std::path::Path;

        // The host's graph only: an unfiltered resolve needs the sources of
        // every platform's dependencies, which an offline host never fetched.
        let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
        let version = std::process::Command::new(rustc)
            .arg("-vV")
            .output()
            .expect("rustc -vV runs");
        let version = String::from_utf8(version.stdout).expect("rustc -vV is UTF-8");
        let host = version
            .lines()
            .find_map(|l| l.strip_prefix("host: "))
            .expect("rustc -vV names the host");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let out = std::process::Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--offline", "--locked"])
            .args(["--filter-platform", host])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("cargo metadata runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let meta: J = serde_json::from_slice(&out.stdout).expect("metadata is JSON");
        let packages: BTreeMap<&str, &J> = meta["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p["id"].as_str().unwrap(), p))
            .collect();
        let normal: BTreeMap<&str, Vec<&str>> = meta["resolve"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                let deps = n["deps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|d| {
                        d["dep_kinds"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|k| k["kind"].is_null())
                    })
                    .map(|d| d["pkg"].as_str().unwrap())
                    .collect();
                (n["id"].as_str().unwrap(), deps)
            })
            .collect();
        fn closure<'a>(
            normal: &BTreeMap<&'a str, Vec<&'a str>>,
            roots: Vec<&'a str>,
        ) -> BTreeSet<&'a str> {
            let mut seen = BTreeSet::new();
            let mut todo = roots;
            while let Some(id) = todo.pop() {
                if seen.insert(id) {
                    todo.extend(normal[id].iter().copied());
                }
            }
            seen
        }
        let name_version = |id: &str| {
            let p = packages[id];
            format!(
                "{} {}",
                p["name"].as_str().unwrap(),
                p["version"].as_str().unwrap()
            )
        };

        let (envelope_id, envelope) = packages
            .iter()
            .find(|(_, p)| p["name"] == "statecraft-envelope")
            .unwrap();
        let pin: Vec<&J> = envelope["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["name"] == "action-gate-core")
            .collect();
        assert_eq!(pin.len(), 1, "one action-gate-core dependency");
        assert_eq!(pin[0]["req"], "=0.3.0", "the pin is exact");
        assert!(pin[0]["kind"].is_null(), "a normal dependency");
        assert_eq!(pin[0]["uses_default_features"], false);
        assert_eq!(pin[0]["features"], J::Array(vec![]));

        let direct = &normal[envelope_id];
        let core = *direct
            .iter()
            .find(|id| packages[**id]["name"] == "action-gate-core")
            .unwrap();
        let with = closure(&normal, direct.clone());
        let without = closure(
            &normal,
            direct.iter().copied().filter(|id| *id != core).collect(),
        );
        let added: BTreeSet<String> = with
            .difference(&without)
            .map(|id| name_version(id))
            .collect();
        assert_eq!(
            added,
            BTreeSet::from(
                [
                    "action-gate-core 0.3.0",
                    "action-gate-types 0.1.0",
                    "canonical-keysort-json 0.1.0"
                ]
                .map(String::from)
            ),
            "the crates action-gate-core adds to the envelope's normal closure"
        );
        assert!(
            with.iter().all(|id| packages[id]["name"] != "regex"),
            "no regex in the envelope's closure"
        );

        for id in with.difference(&without) {
            let p = packages[id];
            let what = name_version(id);
            assert_eq!(p["license"], "Apache-2.0", "{what} licence");
            assert!(
                p["targets"].as_array().unwrap().iter().all(|t| {
                    t["kind"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|k| k != "custom-build")
                }),
                "{what} has a build script"
            );
            let root = Path::new(p["manifest_path"].as_str().unwrap())
                .parent()
                .unwrap();
            let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
            let mut sources = Vec::new();
            let mut dirs = vec![root.join("src")];
            while let Some(dir) = dirs.pop() {
                for entry in std::fs::read_dir(&dir).unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.extension().is_some_and(|e| e == "rs") {
                        let text = std::fs::read_to_string(&path).unwrap();
                        sources.push((path, text));
                    }
                }
            }
            let code: Vec<(&std::path::PathBuf, String)> = sources
                .iter()
                .map(|(path, text)| (path, without_comments(text)))
                .collect();
            assert!(
                manifest
                    .lines()
                    .any(|l| l.trim() == "unsafe_code = \"forbid\"")
                    || code.iter().any(|(path, text)| {
                        path.ends_with("src/lib.rs")
                            && text.lines().any(|l| l.trim() == "#![forbid(unsafe_code)]")
                    }),
                "{what} does not forbid unsafe code"
            );
            // Identifiers, not substrings, outside comments: `use std::{fs,
            // env}` is caught and `lifetime::` is not. A match anywhere else,
            // a string literal included, fails closed, to be read and reviewed.
            for (path, text) in &code {
                for word in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                    assert!(
                        ![
                            "env",
                            "fs",
                            "net",
                            "time",
                            "process",
                            "thread",
                            "SystemTime",
                            "Instant",
                        ]
                        .contains(&word),
                        "{what}: {} names `{word}`",
                        path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn admission_purity_scan_reads_code_not_comments() {
        let src =
            "a // env\n/* fs\n /* net */ time\n*/ b \"// x\" c '\"' d '\\'' e\nuse std::{fs, env};";
        let code = without_comments(src);
        assert_eq!(
            code,
            "a \n\n\n b \"// x\" c '\"' d '\\'' e\nuse std::{fs, env};"
        );
        let raw = "x r#\"q\" // fs\"# y br\"/*\" z // t";
        assert_eq!(without_comments(raw), "x r#\"q\" // fs\"# y br\"/*\" z ");
    }

    /// The index just past a raw string literal (`r"..."`, `r#"..."#`, with
    /// an optional `b`) that starts at `i`, if one does.
    fn raw_string_end(c: &[char], i: usize) -> Option<usize> {
        let ident = |j: usize| c[j].is_alphanumeric() || c[j] == '_';
        let r = match c[i] {
            'r' => i,
            'b' if c.get(i + 1) == Some(&'r') => i + 1,
            _ => return None,
        };
        if i > 0 && ident(i - 1) {
            return None;
        }
        let hashes = c[r + 1..].iter().take_while(|&&h| h == '#').count();
        if c.get(r + 1 + hashes) != Some(&'"') {
            return None;
        }
        let close: Vec<char> = std::iter::once('"')
            .chain(std::iter::repeat_n('#', hashes))
            .collect();
        let body = r + 2 + hashes;
        let at = (body..c.len()).find(|&j| c[j..].starts_with(&close))?;
        Some(at + close.len())
    }

    /// Rust source with its line and (nested) block comments blanked. String
    /// literals are kept, so a comment marker inside one is not a comment; a
    /// char literal is stepped over so a quote inside it opens no string.
    fn without_comments(text: &str) -> String {
        let c: Vec<char> = text.chars().collect();
        let mut out = String::with_capacity(text.len());
        let (mut i, mut depth, mut in_str) = (0, 0usize, false);
        while i < c.len() {
            let next = c.get(i + 1).copied();
            if depth > 0 {
                if c[i] == '*' && next == Some('/') {
                    depth -= 1;
                    i += 2;
                } else if c[i] == '/' && next == Some('*') {
                    depth += 1;
                    i += 2;
                } else {
                    if c[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            } else if in_str {
                out.push(c[i]);
                if c[i] == '\\' {
                    if let Some(n) = next {
                        out.push(n);
                    }
                    i += 2;
                    continue;
                }
                in_str = c[i] != '"';
                i += 1;
            } else if let Some(end) = raw_string_end(&c, i) {
                out.extend(&c[i..end]);
                i = end;
            } else if c[i] == '/' && next == Some('/') {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            } else if c[i] == '/' && next == Some('*') {
                depth = 1;
                i += 2;
            } else if c[i] == '\'' && (c.get(i + 2) == Some(&'\'') || next == Some('\\')) {
                // A char literal: 'x' or an escape such as '\'' or '\u{..}'.
                let end = (i + 2..c.len())
                    .find(|&j| c[j] == '\'' && j > i + 1 + usize::from(next == Some('\\')));
                let end = end.unwrap_or(c.len() - 1);
                out.extend(&c[i..=end]);
                i = end + 1;
            } else {
                in_str = c[i] == '"';
                out.push(c[i]);
                i += 1;
            }
        }
        out
    }
}
