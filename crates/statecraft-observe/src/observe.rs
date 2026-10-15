//! One observation of one commit (spec 027 sections 3.1 to 3.5).
//!
//! The operation requires the observation lock to be held by its caller and
//! never takes it (section 3.8), so admission's fresh observation runs under
//! admission's own hold. Its three seams (section 3.10) are a commit source,
//! spec 003's report and contract sources, and the store.

use crate::projection::{Contract, Eligibility, Projections, canonical_digest};
use crate::proposal::{Entry, Ledger, Observed, plan};
use crate::store::{Held, Store, StoreError};
use crate::subscription::{Candidate, Kind, Scope, matches};
use crate::transition::{Transition, compute};
use serde::Serialize;
use serde_json::{Value, json};
use statecraft_run::contract::{ContractSource, State as Bound};
use statecraft_run::report::{ReportError, ReportSource};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Resolves refs and materializes commits, never touching the operator's
/// checkout (section 3.1 rule 1).
pub trait CommitSource {
    /// The default ref: the branch `HEAD` names, or its commit id when `HEAD`
    /// is detached.
    fn default_ref(&self, root: &Path) -> Result<String, String>;
    /// The commit a ref resolves to, once.
    fn resolve(&self, root: &Path, git_ref: &str) -> Result<String, String>;
    /// A read-only tree of the commit's tracked bytes.
    fn materialize(&self, root: &Path, commit: &str) -> Result<PathBuf, String>;
}

/// The production commit source: spec 003's trusted Git against the
/// operator's repository, exporting each commit once into the product home.
pub struct Git {
    /// Where exported trees are kept: one directory per commit.
    pub trees: PathBuf,
}

impl CommitSource for Git {
    fn default_ref(&self, root: &Path) -> Result<String, String> {
        use statecraft_run::trusted_git::output;
        if let Ok(out) = output(root, &["symbolic-ref", "-q", "--short", "HEAD"]) {
            let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !branch.is_empty() {
                return Ok(branch);
            }
        }
        self.resolve(root, "HEAD")
    }

    fn resolve(&self, root: &Path, git_ref: &str) -> Result<String, String> {
        if git_ref.is_empty() || git_ref.starts_with('-') {
            return Err(format!("`{git_ref}` is not a ref"));
        }
        let spec = format!("{git_ref}^{{commit}}");
        statecraft_run::trusted_git::output(root, &["rev-parse", "--verify", "-q", &spec])
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .map_err(|_| format!("the ref `{git_ref}` does not resolve to a commit"))
    }

    fn materialize(&self, root: &Path, commit: &str) -> Result<PathBuf, String> {
        let key = statecraft_run::repository::key(root);
        let tree = self.trees.join(&key).join(commit);
        if tree.is_dir() {
            return Ok(tree);
        }
        let parent = self.trees.join(&key);
        std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        // Exported beside its final name and renamed into place, so a tree
        // that exists is a whole one.
        let partial = tempfile::Builder::new()
            .prefix(".export-")
            .tempdir_in(&parent)
            .map_err(|e| e.to_string())?;
        statecraft_run::trusted_git::export(root, commit, partial.path())
            .map_err(|e| e.to_string())?;
        let kept = partial.keep();
        match std::fs::rename(&kept, &tree) {
            Ok(()) => Ok(tree),
            Err(e) => {
                // Never leave a partial export behind, whether another
                // observation won the race or the rename failed.
                let _ = std::fs::remove_dir_all(&kept);
                if tree.is_dir() {
                    Ok(tree)
                } else {
                    Err(e.to_string())
                }
            }
        }
    }
}

/// What one observation is given.
pub struct Request<'a> {
    /// The product home.
    pub home: &'a Path,
    /// The registered root, as stored.
    pub root: &'a Path,
    /// The ref, or `None` for the default.
    pub git_ref: Option<&'a str>,
    /// The time entries are stamped with.
    pub at: &'a str,
}

/// The seams an observation reads through.
pub struct Sources<'a> {
    /// Refs and trees.
    pub commits: &'a dyn CommitSource,
    /// spec 003's report reader.
    pub reports: &'a dyn ReportSource,
    /// spec 003's contract binding.
    pub contracts: &'a dyn ContractSource,
}

/// Why an observation recorded nothing.
#[derive(Debug)]
pub enum Refusal {
    /// The ref did not resolve (section 3.11).
    Ref(String),
    /// The commit could not be materialized.
    Materialize(String),
    /// The override journal could not be read: overrides are part of the
    /// input identity, so no identity exists without them.
    Overrides(String),
    /// spec 003's run record could not be read, so a moved contract's bound
    /// attempts are unknown.
    RunRecord(String),
    /// spec 003's reader refused in a way that is not spec-spine declining
    /// to answer for the commit (a missing field, an unrunnable producer).
    Report(ReportError),
    /// The chain could not be read or written.
    Store(StoreError),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Ref(d)
            | Refusal::Materialize(d)
            | Refusal::Overrides(d)
            | Refusal::RunRecord(d) => f.write_str(d),
            Refusal::Report(e) => write!(f, "{e}"),
            Refusal::Store(e) => write!(f, "{e}"),
        }
    }
}

/// One proposal an observation made or caused again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Touched {
    /// The proposal.
    pub proposal: String,
    /// `run` or `notice`.
    pub kind: Kind,
    /// What it is about.
    pub scope: Scope,
    /// `proposed` or `recurred`.
    pub entry: &'static str,
}

/// What one observation found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// `answered`, `refused` or `unchanged`.
    pub result: &'static str,
    /// The observation id: the recorded one, or for `unchanged` the last
    /// recorded one it equals.
    pub observation: String,
    /// The ref as named.
    #[serde(rename = "ref")]
    pub git_ref: String,
    /// The commit it resolved to.
    pub commit: String,
    /// spec-spine's class and message, for `refused`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Value>,
    /// The transitions.
    pub transitions: Vec<Transition>,
    /// Each proposal made or caused again, in the order written.
    pub proposals: Vec<Touched>,
    /// Each superseded proposal, with the one that superseded it.
    pub superseded: Vec<(String, String)>,
}

/// spec-spine declining to answer for the observed commit: recorded as
/// `refused` with its class (section 3.2). Every other reader failure records
/// nothing.
fn declined(e: &ReportError) -> Option<(String, String, String)> {
    match e {
        ReportError::LedgerStale { version, .. } => {
            Some(("ledger-stale".to_string(), version.clone(), e.to_string()))
        }
        ReportError::ProducerRefused {
            version, status, ..
        } => Some((status.clone(), version.clone(), e.to_string())),
        ReportError::CorpusDoesNotCompile { .. } => Some((
            "corpus-does-not-compile".to_string(),
            "not-recorded".to_string(),
            e.to_string(),
        )),
        _ => None,
    }
}

/// What a new proposal names beside its identity: a spec-scoped notice names
/// every open `run` proposal for its spec; a `corpus-refusal` names
/// spec-spine's class and message.
fn detail(
    candidate: &Candidate,
    ledger: &Ledger,
    refusal: Option<&Value>,
    bound: &[BoundAttempt],
) -> Value {
    match (&candidate.kind, &candidate.scope) {
        (Kind::Notice, Scope::Spec { .. }) => {
            let open: Vec<&str> = ledger
                .open()
                .filter(|p| p.kind == Kind::Run && p.scope == candidate.scope)
                .map(|p| p.id.as_str())
                .collect();
            let mut detail = json!({ "openRunProposals": open });
            // `moved-contract` also names every attempt bound to the earlier
            // digest. No acceptance is recorded (spec 005 judges on demand),
            // so each is unaccepted; whether it is live is said beside it.
            if candidate.subscription == crate::subscription::Subscription::MovedContract
                && let Some(earlier) = candidate.row["before"]["digest"].as_str()
            {
                let attempts: Vec<Value> = bound
                    .iter()
                    .filter(|a| a.digest == earlier)
                    .map(|a| json!({ "runId": a.run_id, "attempt": a.attempt, "live": a.live }))
                    .collect();
                detail["boundAttempts"] = Value::Array(attempts);
            }
            detail
        }
        (Kind::Notice, Scope::Repository) => refusal.cloned().unwrap_or(Value::Null),
        (Kind::Run, _) => Value::Null,
    }
}

/// Each proposal made or caused again, and each supersession (old, by).
type Appended = (Vec<Touched>, Vec<(String, String)>);

/// Plan and append what `candidates` call for under `observation`, returning
/// each proposal made or caused again and each supersession.
#[allow(clippy::too_many_arguments)]
fn append_planned(
    held: &Held,
    store: &mut Store,
    ledger: &mut Ledger,
    request: &Request<'_>,
    observation: &str,
    git_ref: &str,
    candidates: &[Candidate],
    refusal: Option<&Value>,
) -> Result<Appended, Refusal> {
    let bound = if candidates
        .iter()
        .any(|c| c.subscription == crate::subscription::Subscription::MovedContract)
    {
        bound_attempts(request.home, request.root)?
    } else {
        Vec::new()
    };
    let entries = plan(ledger, observation, git_ref, candidates, |c, l| {
        detail(c, l, refusal, &bound)
    });
    let mut proposals = Vec::new();
    let mut superseded = Vec::new();
    for entry in &entries {
        store
            .append(held, request.at, entry)
            .map_err(Refusal::Store)?;
        ledger.apply(entry.clone());
        match entry {
            Entry::Proposed { proposal, .. } | Entry::Recurred { proposal, .. } => {
                let p = ledger.proposal(proposal).expect("applied above");
                proposals.push(Touched {
                    proposal: proposal.clone(),
                    kind: p.kind,
                    scope: p.scope.clone(),
                    entry: if matches!(entry, Entry::Proposed { .. }) {
                        "proposed"
                    } else {
                        "recurred"
                    },
                });
            }
            Entry::Disposed {
                proposal,
                disposition: crate::proposal::Disposition::Superseded { by },
            } => superseded.push((proposal.clone(), by.clone())),
            _ => {}
        }
    }
    Ok((proposals, superseded))
}

/// An attempt and the closure digest its intent bound it to (spec 003
/// section 3.1.3).
struct BoundAttempt {
    run_id: String,
    attempt: u32,
    digest: String,
    live: bool,
}

/// Every attempt in the run record with a bound closure digest.
fn bound_attempts(home: &Path, root: &Path) -> Result<Vec<BoundAttempt>, Refusal> {
    let (chain, _) = statecraft_run::record::Chain::open(home, root)
        .map_err(|e| Refusal::RunRecord(format!("the run record could not be read: {e}")))?;
    let runs = statecraft_run::session::runs(&chain);
    Ok(chain
        .entries()
        .into_iter()
        .filter(|e| e.kind == statecraft_run::record::Kind::Intent)
        .filter_map(|e| {
            let digest = e.detail["contract"]["digest"].as_str()?.to_string();
            let live = runs
                .iter()
                .find(|r| r.id == e.run_id)
                .and_then(|r| r.attempts.iter().find(|a| a.number == e.attempt))
                .is_some_and(|a| a.live());
            Some(BoundAttempt {
                run_id: e.run_id,
                attempt: e.attempt,
                digest,
                live,
            })
        })
        .collect())
}

/// One observation, under the caller's hold of the observation lock.
pub fn observe(
    held: &Held,
    store: &mut Store,
    request: &Request<'_>,
    sources: &Sources<'_>,
) -> Result<Observation, Refusal> {
    let git_ref = match request.git_ref {
        Some(r) => r.to_string(),
        None => sources
            .commits
            .default_ref(request.root)
            .map_err(Refusal::Ref)?,
    };
    let commit = sources
        .commits
        .resolve(request.root, &git_ref)
        .map_err(Refusal::Ref)?;
    let tree = sources
        .commits
        .materialize(request.root, &commit)
        .map_err(Refusal::Materialize)?;

    // The policy as read at the commit, and the overrides as they stand now:
    // both change eligibility, so both are in the input identity.
    let (policy, disagreement) =
        statecraft_run::policy::resolve(&tree, &statecraft_run::policy::NoDeclarationFiled, None);
    let overrides = statecraft_run::overrides::read(request.home, request.root)
        .map_err(|e| Refusal::Overrides(e.to_string()))?
        .overrides();

    let (projections, version, refusal) = match sources.reports.corpus_report(&tree) {
        Ok(report) => {
            let work =
                statecraft_run::work::select(&report, &policy, &overrides, disagreement.clone());
            let mut rows = BTreeMap::new();
            for item in &work.eligible {
                rows.insert(item.id.clone(), Eligibility::Eligible);
            }
            for e in &work.excluded {
                rows.insert(
                    e.id.clone(),
                    Eligibility::Excluded {
                        reason: e.reason.clone(),
                    },
                );
            }
            let projections = Projections::answered(rows, |id| {
                let binding = statecraft_run::contract::bind(
                    sources.contracts,
                    &tree,
                    id,
                    report.lifecycle_of(id),
                );
                match (binding.state, binding.digest) {
                    (Bound::Bound, Some(digest)) => Contract::Digest { digest },
                    (state, _) => Contract::Unavailable {
                        reason: binding.detail.unwrap_or_else(|| {
                            serde_json::to_value(state)
                                .ok()
                                .and_then(|v| v.as_str().map(str::to_string))
                                .unwrap_or_default()
                        }),
                    },
                }
            });
            (projections, report.spec_spine_version.clone(), None)
        }
        Err(e) => match declined(&e) {
            Some((class, version, message)) => (
                Projections::refused(&class),
                version,
                Some(json!({ "class": class, "message": message })),
            ),
            None => return Err(Refusal::Report(e)),
        },
    };

    let input_identity = canonical_digest(&json!({
        "repository": statecraft_run::repository::key(request.root),
        "commit": commit,
        "specSpine": version,
        "policy": policy,
        "overrides": overrides,
    }));
    let mut ledger = Ledger::fold(store.entries().to_vec());
    // Unchanged: the same input identity as the last recorded observation
    // writes no second `observed` entry. If the process that wrote that entry
    // died before every proposal it called for was appended, those are
    // appended now, under the same observation, so a crash never silences a
    // proposal (section 3.8: the chain holds no unknown state of its own).
    if let Some(last) = ledger.previous().cloned()
        && last.input_identity == input_identity
    {
        let position = ledger.observations.last().map_or(0, |(p, _)| *p);
        let written: std::collections::BTreeSet<&str> = store.entries()[position + 1..]
            .iter()
            .filter_map(|e| match e {
                Entry::Proposed {
                    proposal,
                    observation,
                    ..
                }
                | Entry::Recurred {
                    proposal,
                    observation,
                    ..
                } if observation == &last.observation => Some(proposal.as_str()),
                _ => None,
            })
            .collect();
        let missing: Vec<Candidate> = matches(&last.transitions, &last.projections)
            .into_iter()
            .filter(|c| !written.contains(c.id().as_str()))
            .collect();
        let (proposals, superseded) = append_planned(
            held,
            store,
            &mut ledger,
            request,
            &last.observation,
            &last.git_ref,
            &missing,
            None,
        )?;
        return Ok(Observation {
            result: "unchanged",
            observation: last.observation.clone(),
            git_ref,
            commit,
            refusal: None,
            transitions: Vec::new(),
            proposals,
            superseded,
        });
    }

    let transitions = compute(
        ledger.previous().map(|o| &o.projections),
        ledger.last_answered().map(|o| &o.projections),
        &projections,
    );
    let candidates = matches(&transitions, &projections);
    let observation = canonical_digest(&json!({
        "inputIdentity": input_identity,
        "position": ledger.len(),
    }));
    let observed = Entry::Observed(Box::new(Observed {
        observation: observation.clone(),
        input_identity,
        git_ref: git_ref.clone(),
        commit: commit.clone(),
        message: refusal
            .as_ref()
            .and_then(|r| r["message"].as_str().map(str::to_string)),
        digests: projections.digests(),
        projections: projections.clone(),
        transitions: transitions.clone(),
    }));
    store
        .append(held, request.at, &observed)
        .map_err(Refusal::Store)?;
    ledger.apply(observed);

    let (proposals, superseded) = append_planned(
        held,
        store,
        &mut ledger,
        request,
        &observation,
        &git_ref,
        &candidates,
        refusal.as_ref(),
    )?;
    Ok(Observation {
        result: if projections.corpus_state.answered() {
            "answered"
        } else {
            "refused"
        },
        observation,
        git_ref,
        commit,
        refusal,
        transitions,
        proposals,
        superseded,
    })
}

/// What `observe show` answers (section 3.9): the last answered observation,
/// and whether the ref now resolves to a commit the chain has no `observed`
/// entry for. Reads without the lock and writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    /// The ref as named, or the default.
    #[serde(rename = "ref")]
    pub git_ref: String,
    /// The commit it resolves to now.
    pub commit: String,
    /// The last answered observation, or `None` when none is recorded.
    pub last_answered: Option<Observed>,
    /// Whether no `observed` entry names `commit`.
    pub unobserved: bool,
    /// The chain's incomplete tail, reported and not interpreted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torn_tail: Option<u64>,
}

/// `observe show`.
pub fn show(
    store: &Store,
    root: &Path,
    git_ref: Option<&str>,
    commits: &dyn CommitSource,
) -> Result<Shown, Refusal> {
    let git_ref = match git_ref {
        Some(r) => r.to_string(),
        None => commits.default_ref(root).map_err(Refusal::Ref)?,
    };
    let commit = commits.resolve(root, &git_ref).map_err(Refusal::Ref)?;
    let ledger = Ledger::fold(store.entries().to_vec());
    Ok(Shown {
        unobserved: !ledger.observations.iter().any(|(_, o)| o.commit == commit),
        last_answered: ledger.last_answered().cloned(),
        git_ref,
        commit,
        torn_tail: store.torn_tail().map(|t| t.bytes),
    })
}
