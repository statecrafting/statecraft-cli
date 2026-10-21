//! Spec 019's command bindings: `run --context`, and the `contextManifest`
//! member of `run show` and `accept` (section 3.9), through spec 019's
//! additive edge on this crate.
//!
//! Every operation is a library's. The packet reads are
//! `statecraft-home::context`'s, the manifest, custody and delivery fold are
//! `statecraft-run::context`'s, and the checks are
//! `statecraft-acceptance::context`'s. This module reads the operator's two
//! files, joins those calls, and renders their typed values.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use statecraft_acceptance::context::{
    Citation, ClosureAnswer, ContextResult, CurrentAnswer, EvidenceResult, Inputs, MemberAnswer,
    Producer, Validated,
};
use statecraft_home::context::{Closure, Decoded, Member, Source};
use statecraft_run::context::{
    AcceptedRepository, ContextPolicy, DeliveryPolicy, Intended, Manifest, OptionalOmission,
    Projection, Requirement, Task,
};
use statecraft_run::record::Chain;

/// One repository in a `run --context` task file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskRepository {
    /// The stable local repository id.
    pub id: String,
    /// Its role in the task.
    pub role: String,
    /// Whether the task requires it.
    pub requirement: Requirement,
    /// The checkout, relative to the task file.
    pub path: String,
    /// The opaque repository identity carried into the packet.
    pub repository: String,
    /// The revision to export; absent binds the clean working tree at HEAD.
    #[serde(default)]
    pub revision: Option<String>,
    /// Whether its pages are delivered.
    pub delivery: Intended,
    /// The producer's context-packet request, passed through unread.
    pub request: Value,
}

/// A `run --context` task file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskFile {
    /// The later task contract's identity, when there is one.
    #[serde(default)]
    pub task: Option<String>,
    /// What an incomplete optional repository does to eligibility.
    pub optional_omission: OptionalOmission,
    /// The acceptance policy, recorded with the intent.
    pub policy: ContextPolicy,
    /// The repositories, in order.
    pub repositories: Vec<TaskRepository>,
}

/// The manifest and policy prepared for one attempt.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// The manifest, built for the attempt it will be recorded in.
    pub manifest: Manifest,
    /// The policy.
    pub policy: ContextPolicy,
}

fn read_json<T: serde::de::DeserializeOwned>(file: &Path) -> Result<T, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", file.display()))
}

/// Assemble, retain and compose the manifest a task file names, for
/// `attempt`, and refuse one that may not execute (section 3.2). Nothing is
/// recorded in the run's chain here; every page's bytes are retained in the
/// home, write-once, under their custody digest.
pub fn prepare(
    root: &Path,
    home: &Path,
    run_id: &str,
    attempt: u32,
    file: &Path,
) -> Result<Prepared, String> {
    let task: TaskFile = read_json(file)?;
    if task.policy.schema != statecraft_run::context::POLICY_SCHEMA {
        return Err(format!("unsupported policy schema {}", task.policy.schema));
    }
    let program = PathBuf::from(crate::judge::program(root)?);
    let version = crate::judge::selection(root)
        .judge()
        .ok()
        .and_then(|s| s.version.clone())
        .ok_or("the selected spec-spine reports no version")?;
    let base = file.parent().unwrap_or(Path::new("."));
    let mut repositories = Vec::with_capacity(task.repositories.len());
    for r in &task.repositories {
        let repo = statecraft_home::context::operator_path(base, &r.path);
        let pages = statecraft_home::context::assemble(
            &Source {
                program: &program,
                repo: &repo,
                repository: &r.repository,
                revision: r.revision.as_deref(),
            },
            &r.request,
            r.delivery,
        )
        .map_err(|e| format!("repository {}: {e}", r.id))?;
        for page in &pages {
            statecraft_run::context::retain(home, &page.bytes).map_err(|e| e.to_string())?;
        }
        repositories.push(AcceptedRepository {
            id: r.id.clone(),
            role: r.role.clone(),
            requirement: r.requirement,
            pages,
        });
    }
    let manifest = Manifest::build(
        run_id,
        attempt,
        task.task.map_or(Task::NotRecorded, Task::Id),
        &statecraft_run::context::Producer {
            package: "spec-spine".into(),
            version,
            packet_interface: spec_spine_interface(),
        },
        &repositories,
        DeliveryPolicy::new(task.optional_omission),
    )
    .map_err(|e| e.to_string())?;
    manifest.eligibility().map_err(|e| e.to_string())?;
    Ok(Prepared {
        manifest,
        policy: task.policy,
    })
}

/// The packet schema the adopted producer serves, which is the interface
/// this consumer reads.
fn spec_spine_interface() -> String {
    statecraft_home::context::SCHEMA_VERSION.to_string()
}

/// One checkout an `accept --context` file names.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkout {
    /// The checkout, relative to the file.
    pub path: String,
    /// The repository identity the caller supplies for it.
    pub repository: String,
}

/// An `accept --context` file: the current checkouts, the claimed citations
/// and the spec 005 evidence admissions.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptFile {
    /// By manifest repository id.
    #[serde(default)]
    pub checkouts: BTreeMap<String, Checkout>,
    /// The claimed citations, in order.
    #[serde(default)]
    pub citations: Vec<Citation>,
    /// The evidence admissions, in order.
    #[serde(default)]
    pub evidence: Vec<EvidenceResult>,
}

/// Read an `accept --context` file, resolving its checkout paths.
pub fn read_accept_file(file: &Path) -> Result<AcceptFile, String> {
    let mut parsed: AcceptFile = read_json(file)?;
    let base = file.parent().unwrap_or(Path::new("."));
    for c in parsed.checkouts.values_mut() {
        c.path = statecraft_home::context::operator_path(base, &c.path)
            .display()
            .to_string();
    }
    Ok(parsed)
}

/// The code word of a producer refusal: its verdict message is
/// `refused: <code>: <detail>`, and the code is the token between the first
/// two colons, read exactly. A message of any other shape has none.
fn refusal_code(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("refused: ")?;
    let (code, _) = rest.split_once(": ")?;
    code.bytes()
        .all(|b| b.is_ascii_lowercase() || b == b'-')
        .then_some(code)
        .filter(|c| !c.is_empty())
}

#[cfg(test)]
mod tests {
    use super::refusal_code;

    #[test]
    fn a_refusal_code_is_the_exact_token_after_refused() {
        assert_eq!(
            refusal_code("refused: unsupported-schema: consumer schema 9.0.0 is not served"),
            Some("unsupported-schema")
        );
        assert_eq!(
            refusal_code("refused: dirty-tree: not clean"),
            Some("dirty-tree")
        );
        // A code word elsewhere in the text is not the refusal's code.
        assert_eq!(refusal_code("stale: see unsupported-schema: here"), None);
        assert_eq!(refusal_code("refused: Not A Code: x"), None);
        assert_eq!(refusal_code("refused: unsupported-schema"), None);
    }
}

/// The adopted producer, as acceptance asks it.
struct Adopted<'a> {
    program: PathBuf,
    checkouts: &'a BTreeMap<String, Checkout>,
    manifest: &'a Manifest,
    retained: &'a dyn Fn(&str) -> Option<Vec<u8>>,
}

impl Adopted<'_> {
    fn first_page(&self, id: &str) -> Option<Vec<u8>> {
        let repo = self.manifest.repositories.iter().find(|r| r.id == id)?;
        (self.retained)(&repo.pages.first()?.custody_digest)
    }
}

impl Producer for Adopted<'_> {
    fn validate_page(&self, bytes: &[u8]) -> Validated {
        match statecraft_home::context::decode(bytes) {
            Decoded::Valid(f) => Validated::Valid(f),
            Decoded::Invalid(r) => Validated::Invalid(r),
            Decoded::Unsupported(r) => Validated::Unsupported(r),
            Decoded::Unreadable(r) => Validated::Unreadable(r),
        }
    }

    fn closure(&self, id: &str, page: &[u8]) -> ClosureAnswer {
        let Some(checkout) = self.checkouts.get(id) else {
            return ClosureAnswer::Missing(format!("no checkout supplied for {id}"));
        };
        match statecraft_home::context::closure(&self.program, Path::new(&checkout.path), page) {
            Closure::Answered {
                digest,
                differences,
            } => ClosureAnswer::Answered {
                digest,
                differences,
            },
            Closure::Refused(r) => {
                let why = r.to_string();
                match (r.kind.as_str(), refusal_code(&r.message)) {
                    ("not-found" | "io", _) => ClosureAnswer::Missing(why),
                    ("stale", _) => ClosureAnswer::Stale(why),
                    ("refused", Some("unsupported-schema")) => ClosureAnswer::Unsupported(why),
                    // Every other refusal keeps its code in the reason and is
                    // a state no check can pass, never a success.
                    _ => ClosureAnswer::Unreadable(why),
                }
            }
        }
    }

    fn current(&self, id: &str) -> CurrentAnswer {
        let Some(checkout) = self.checkouts.get(id) else {
            return CurrentAnswer::Unverified(format!("no current checkout supplied for {id}"));
        };
        let Some(page) = self.first_page(id) else {
            return CurrentAnswer::Unverified("no retained page to bind under".into());
        };
        match statecraft_home::context::current(
            &self.program,
            Path::new(&checkout.path),
            &checkout.repository,
            &page,
        ) {
            Ok(snapshot) => CurrentAnswer::Resolved(snapshot),
            Err(why) => CurrentAnswer::Unverified(why),
        }
    }

    fn member(&self, page: &[u8], identity: &str) -> MemberAnswer {
        match statecraft_home::context::member(page, identity) {
            Member::Found {
                content_digest,
                start_line,
                end_line,
            } => MemberAnswer::Found {
                content_digest,
                start_line,
                end_line,
            },
            Member::Omitted => MemberAnswer::Omitted,
            Member::Absent => MemberAnswer::Absent,
            Member::Unreadable(r) => MemberAnswer::Unreadable(r),
        }
    }
}

/// What `accept` shows under `contextManifest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Judgment {
    /// The attempt's intent carries no manifest.
    NotRecorded,
    /// The intent carries a manifest or policy that binds nothing, or none
    /// was recorded beside a manifest.
    Unreadable {
        /// Why.
        reason: String,
    },
    /// Every result, and eligibility under the recorded policy.
    Judged {
        /// The results.
        #[serde(flatten)]
        result: Box<ContextResult>,
    },
}

impl Judgment {
    /// Why acceptance is ineligible on this judgment, when it is.
    pub fn ineligible(&self) -> Option<Vec<String>> {
        match self {
            Judgment::NotRecorded => None,
            Judgment::Unreadable { reason } => Some(vec![format!("context-unreadable: {reason}")]),
            Judgment::Judged { result } if !result.eligible => {
                Some(result.reasons.iter().map(ToString::to_string).collect())
            }
            Judgment::Judged { .. } => None,
        }
    }
}

/// Section 3.7 for one attempt: read the recorded manifest, policy and
/// delivery from the chain, and judge them through the adopted producer.
pub fn judge(
    root: &Path,
    home: &Path,
    chain: &Chain,
    run_id: &str,
    attempt: u32,
    supplied: &AcceptFile,
) -> Judgment {
    let manifest = match statecraft_run::context::recorded_manifest(chain, run_id, attempt) {
        Ok(None) => return Judgment::NotRecorded,
        Ok(Some(m)) => m,
        Err(reason) => return Judgment::Unreadable { reason },
    };
    let policy = match statecraft_run::context::recorded_policy(chain, run_id, attempt) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return Judgment::Unreadable {
                reason: "the intent records a manifest and no policy".into(),
            };
        }
        Err(reason) => return Judgment::Unreadable { reason },
    };
    let program = match crate::judge::program(root) {
        Ok(p) => PathBuf::from(p),
        Err(reason) => return Judgment::Unreadable { reason },
    };
    let observations = statecraft_run::context::recorded_deliveries(chain, &manifest);
    let delivery = statecraft_run::context::fold_delivery(&manifest, &observations);
    let retained = |custody: &str| {
        statecraft_run::context::read_retained(home, custody)
            .ok()
            .flatten()
    };
    let producer = Adopted {
        program,
        checkouts: &supplied.checkouts,
        manifest: &manifest,
        retained: &retained,
    };
    let result = statecraft_acceptance::context::judge(
        &Inputs {
            manifest: &manifest,
            policy: &policy,
            delivery: &delivery,
            retained: &retained,
            citations: &supplied.citations,
            evidence: &supplied.evidence,
        },
        &producer,
    );
    Judgment::Judged {
        result: Box::new(result),
    }
}

/// One attempt's context, as `run show` renders it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptContextView {
    /// The attempt.
    pub attempt: u32,
    /// The record's projection: `not-recorded` for a historical attempt.
    #[serde(flatten)]
    pub projection: Projection,
}

/// Every attempt's context for `run show`, folded from the chain alone.
pub fn of_run(chain: &Chain, run: &statecraft_run::Run) -> Vec<AttemptContextView> {
    run.attempts
        .iter()
        .map(|a| AttemptContextView {
            attempt: a.number,
            projection: statecraft_run::context::project(chain, &run.id, a.number),
        })
        .collect()
}

/// The human line for one attempt's context.
pub fn describe(views: &[AttemptContextView]) -> String {
    let mut out = String::new();
    for v in views {
        match &v.projection {
            Projection::NotRecorded => {}
            Projection::Unreadable { reason } => {
                out.push_str(&format!(
                    "attempt {} context unreadable: {reason}\n",
                    v.attempt
                ));
            }
            Projection::Recorded { manifest, delivery } => {
                out.push_str(&format!(
                    "attempt {} context manifest {} ({} repositories)\n",
                    v.attempt,
                    manifest.manifest_digest,
                    manifest.repositories.len()
                ));
                for r in &manifest.repositories {
                    out.push_str(&format!(
                        "  {} {} at {} tree {}\n",
                        r.id, r.snapshot.repository, r.snapshot.revision, r.snapshot.tree
                    ));
                }
                for (i, o) in delivery.intended.iter().zip(&delivery.observed) {
                    out.push_str(&format!(
                        "  page {} intended {} observed {}\n",
                        i.page_digest,
                        serde_json::to_value(i.delivery)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_string))
                            .unwrap_or_default(),
                        serde_json::to_value(o.state)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_string))
                            .unwrap_or_default()
                    ));
                }
                for f in &delivery.findings {
                    out.push_str(&format!("  finding {f}\n"));
                }
            }
        }
    }
    out
}
