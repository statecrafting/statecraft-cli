//! Spec 027 section 3.11's negative cases at the crate's seams (section
//! 3.10): a commit source, spec 003's report and contract sources, and the
//! store, with no git and no spec-spine. The cases that need the binary are in
//! `statecraft-cli`'s `observation` suite.

use statecraft_observe::observe::{CommitSource, Refusal, Request, Sources, observe};
use statecraft_observe::proposal::{Disposition, Entry, Ledger, State};
use statecraft_observe::proposals::{Admission, AdmitError, admit, recover};
use statecraft_observe::store::{self, Store};
use statecraft_run::contract::{ContractSource, Request as ClosureRequest, Resolution};
use statecraft_run::report::{
    CorpusReport, ReadySpec, ReportError, ReportSource, SpecLifecycle, StatusSource,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What spec-spine answers for one commit.
#[derive(Clone)]
enum Answer {
    /// Each ready spec: status, and its closure digest when one resolves.
    Corpus(Vec<(&'static str, &'static str, Option<&'static str>)>),
    Stale,
    MissingField,
}

struct World {
    home: tempfile::TempDir,
    root: PathBuf,
    head: RefCell<String>,
    answers: RefCell<BTreeMap<String, Answer>>,
}

impl World {
    fn new() -> World {
        let home = tempfile::tempdir().unwrap();
        World {
            root: home.path().join("target"),
            home,
            head: RefCell::new(String::new()),
            answers: RefCell::new(BTreeMap::new()),
        }
    }

    /// Make `commit` what `main` resolves to, with this answer.
    fn at(&self, commit: &str, answer: Answer) {
        *self.head.borrow_mut() = commit.to_string();
        self.answers.borrow_mut().insert(commit.to_string(), answer);
    }

    fn answer(&self, tree: &Path) -> Answer {
        let commit = tree.file_name().unwrap().to_str().unwrap();
        self.answers.borrow()[commit].clone()
    }

    fn observe(&self) -> Result<statecraft_observe::observe::Observation, Refusal> {
        let held = store::lock(self.home.path(), &self.root).unwrap();
        let mut chain = Store::open(self.home.path(), &self.root).unwrap();
        observe(&held, &mut chain, &self.request(), &self.sources())
    }

    fn request(&self) -> Request<'_> {
        Request {
            home: self.home.path(),
            root: &self.root,
            git_ref: None,
            at: "2026-10-13T00:00:00Z",
        }
    }

    fn sources(&self) -> Sources<'_> {
        Sources {
            commits: self,
            reports: self,
            contracts: self,
        }
    }

    fn ledger(&self) -> Ledger {
        Ledger::fold(
            Store::open(self.home.path(), &self.root)
                .unwrap()
                .entries()
                .to_vec(),
        )
    }

    fn admit(&self, id: &str, enter: impl FnOnce(&str, &str)) -> Result<Admission<()>, AdmitError> {
        let held = store::lock(self.home.path(), &self.root).unwrap();
        let mut chain = Store::open(self.home.path(), &self.root).unwrap();
        admit(
            &held,
            &mut chain,
            &self.request(),
            &self.sources(),
            id,
            "sam",
            "go",
            enter,
        )
    }

    fn open_run(&self) -> String {
        self.ledger()
            .open()
            .find(|p| p.kind == statecraft_observe::subscription::Kind::Run)
            .unwrap()
            .id
            .clone()
    }
}

impl CommitSource for World {
    fn default_ref(&self, _: &Path) -> Result<String, String> {
        Ok("main".to_string())
    }
    fn resolve(&self, _: &Path, git_ref: &str) -> Result<String, String> {
        match git_ref {
            "main" => Ok(self.head.borrow().clone()),
            other if self.answers.borrow().contains_key(other) => Ok(other.to_string()),
            other => Err(format!("the ref `{other}` does not resolve to a commit")),
        }
    }
    fn materialize(&self, _: &Path, commit: &str) -> Result<PathBuf, String> {
        let tree = self.home.path().join("trees").join(commit);
        std::fs::create_dir_all(&tree).map_err(|e| e.to_string())?;
        Ok(tree)
    }
}

impl ReportSource for World {
    fn corpus_report(&self, tree: &Path) -> Result<CorpusReport, ReportError> {
        match self.answer(tree) {
            Answer::Corpus(specs) => Ok(CorpusReport {
                spec_spine_version: "0.29.0".to_string(),
                ready: specs
                    .iter()
                    .map(|(id, status, _)| ReadySpec {
                        id: id.to_string(),
                        title: id.to_string(),
                        status: Some(status.to_string()),
                    })
                    .collect(),
                lifecycle: specs
                    .iter()
                    .map(|(id, status, _)| SpecLifecycle {
                        id: id.to_string(),
                        status: status.to_string(),
                        implementation: Some("pending".to_string()),
                        obligations: Vec::new(),
                    })
                    .collect(),
                status_source: StatusSource::ListAgreeingWithPlan,
            }),
            Answer::Stale => Err(ReportError::LedgerStale {
                path: tree.display().to_string(),
                version: "0.29.0".to_string(),
                detail: "spec-registry: STALE".to_string(),
            }),
            Answer::MissingField => Err(ReportError::MissingField {
                command: "registry plan --json".to_string(),
                field: "ready".to_string(),
                version: "0.29.0".to_string(),
            }),
        }
    }
}

impl ContractSource for World {
    fn producer_version(&self, _: &Path) -> String {
        "0.29.0".to_string()
    }
    fn resolve(&self, tree: &Path, request: &ClosureRequest) -> Resolution {
        let Answer::Corpus(specs) = self.answer(tree) else {
            unreachable!("a closure is asked only of an answered corpus")
        };
        match specs
            .iter()
            .find(|s| request.specs.iter().any(|r| r == s.0))
            .and_then(|s| s.2)
        {
            Some(digest) => Resolution::Resolved {
                digest: digest.to_string(),
                members: Vec::new(),
            },
            None => Resolution::Unresolved {
                detail: "no closure".to_string(),
            },
        }
    }
}

fn kinds(o: &statecraft_observe::observe::Observation) -> Vec<&'static str> {
    o.transitions.iter().map(|t| t.kind()).collect()
}

#[test]
fn a_report_missing_a_field_records_nothing_and_an_unresolved_ref_records_nothing() {
    let w = World::new();
    w.at("c0", Answer::MissingField);
    assert!(matches!(
        w.observe(),
        Err(Refusal::Report(ReportError::MissingField { .. }))
    ));
    assert!(w.ledger().is_empty());
    let held = store::lock(w.home.path(), &w.root).unwrap();
    let mut chain = Store::open(w.home.path(), &w.root).unwrap();
    let r = observe(
        &held,
        &mut chain,
        &Request {
            git_ref: Some("nope"),
            ..w.request()
        },
        &w.sources(),
    );
    assert!(matches!(r, Err(Refusal::Ref(_))));
    assert!(chain.entries().is_empty());
}

#[test]
fn a_first_refusal_is_recorded_without_a_notice_and_a_second_with_the_same_class_is_no_transition()
{
    let w = World::new();
    w.at("c0", Answer::Stale);
    let first = w.observe().unwrap();
    assert_eq!(first.result, "refused");
    assert!(first.transitions.is_empty() && first.proposals.is_empty());
    w.at("c1", Answer::Stale);
    let second = w.observe().unwrap();
    assert!(second.transitions.is_empty() && second.proposals.is_empty());
}

#[test]
fn admission_is_stale_on_exclusion_and_on_a_refused_fresh_observation() {
    let w = World::new();
    w.at(
        "c0",
        Answer::Corpus(vec![("001-a", "approved", Some("d1"))]),
    );
    w.observe().unwrap();
    let id = w.open_run();
    w.at("c1", Answer::Corpus(vec![("001-a", "draft", None)]));
    let entered = std::cell::Cell::new(false);
    match w.admit(&id, |_, _| entered.set(true)).unwrap() {
        Admission::Stale { differing, .. } => {
            assert!(differing.contains("excluded"), "{differing}")
        }
        other => panic!("{other:?}"),
    }
    assert!(!entered.get());

    let w = World::new();
    w.at(
        "c0",
        Answer::Corpus(vec![("001-a", "approved", Some("d1"))]),
    );
    w.observe().unwrap();
    let id = w.open_run();
    w.at("c1", Answer::Stale);
    match w.admit(&id, |_, _| panic!("no run")).unwrap() {
        Admission::Stale { differing, .. } => {
            assert!(differing.contains("refused(ledger-stale)"), "{differing}")
        }
        other => panic!("{other:?}"),
    }
    // The fresh observation's refusal is recorded too.
    assert!(
        !w.ledger()
            .previous()
            .unwrap()
            .projections
            .corpus_state
            .answered()
    );
}

#[test]
fn a_contract_that_becomes_unavailable_leaves_the_run_open_until_a_digest_appears() {
    let w = World::new();
    w.at(
        "c0",
        Answer::Corpus(vec![("001-a", "approved", Some("d1"))]),
    );
    w.observe().unwrap();
    let first = w.open_run();
    w.at("c1", Answer::Corpus(vec![("001-a", "approved", None)]));
    let moved = w.observe().unwrap();
    assert_eq!(kinds(&moved), ["contract-moved"]);
    // A moved-contract notice, and the run proposal stays open.
    assert_eq!(moved.proposals.len(), 1);
    assert_eq!(w.ledger().proposal(&first).unwrap().state, State::Open);
    // Admitting it now is stale.
    w.at("c2", Answer::Corpus(vec![("001-a", "approved", None)]));
    assert!(matches!(
        w.admit(&first, |_, _| panic!("no run")).unwrap(),
        Admission::Stale { .. }
    ));
    // A digest appears: a new run proposal; nothing else is open for 001-a.
    w.at(
        "c3",
        Answer::Corpus(vec![("001-a", "approved", Some("d3"))]),
    );
    w.observe().unwrap();
    let open: Vec<_> = w
        .ledger()
        .open()
        .filter(|p| p.kind == statecraft_observe::subscription::Kind::Run)
        .map(|p| p.contract().unwrap().to_string())
        .collect();
    assert_eq!(open, ["d3"]);
}

#[test]
fn a_process_that_died_after_the_attempt_intent_is_recovered_as_admitted() {
    let w = World::new();
    w.at(
        "c0",
        Answer::Corpus(vec![("001-a", "approved", Some("d1"))]),
    );
    w.observe().unwrap();
    let id = w.open_run();
    let held = store::lock(w.home.path(), &w.root).unwrap();
    let mut chain = Store::open(w.home.path(), &w.root).unwrap();
    chain
        .append(
            &held,
            "t",
            &Entry::Admitting {
                proposal: id.clone(),
                observation: "o".to_string(),
                operator: "sam".to_string(),
                reason: "go".to_string(),
            },
        )
        .unwrap();
    // spec 003's intent, naming the proposal (section 3.7), then the crash.
    let (mut record, _) = statecraft_run::record::Chain::open(w.home.path(), &w.root).unwrap();
    record
        .append(
            "intent-1",
            "t",
            &statecraft_run::record::Entry {
                kind: statecraft_run::record::Kind::Intent,
                run_id: "001-a".to_string(),
                attempt: 1,
                subject: "attempt".to_string(),
                idempotency_key: None,
                detail: serde_json::json!({ "proposal": id }),
                effect_id: Default::default(),
            },
        )
        .unwrap();
    let written = recover(&held, &mut chain, w.home.path(), &w.root, "t").unwrap();
    assert_eq!(
        written,
        [Entry::Disposed {
            proposal: id.clone(),
            disposition: Disposition::Admitted {
                run_id: "001-a".to_string(),
                attempt: 1
            }
        }]
    );
}

#[test]
fn a_subscription_file_in_the_target_is_not_a_source_and_derived_output_is_never_opened() {
    // The set is closed in code: the crate's sources name no file a target
    // could declare one in, and never the compiler's derived directory.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(src).unwrap().flatten() {
        let text = std::fs::read_to_string(entry.path()).unwrap();
        assert!(!text.contains(".statecraft/derived"), "{:?}", entry.path());
        assert!(!text.contains("subscriptions."), "{:?}", entry.path());
    }
}
