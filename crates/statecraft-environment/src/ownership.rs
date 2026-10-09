//! Ownership disagreement: three facts per path, compared (spec 026).
//!
//! `doctor` reads, for every path any of them names, the class the manifest
//! records, the class the transfer journal last left it in, and the class the
//! current selection's rendering would give it. Where those facts imply
//! incompatible ownership the path is reported, naming the pair and both
//! values with their sources. Nothing here writes, transfers or repairs, and a
//! rendering that could not be computed is never read as agreement.

use crate::adapter::Declaration;
use crate::claimant::resolve;
use crate::doctor::{Finding, Report, State};
use crate::manifest::{Class, Manifest};
use crate::transfer::{self, Ownership};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The class a rendering gives a path (spec 026 section 3.1, fact 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rendered {
    /// A profile or adapter only reads it: never a disagreement.
    ReadOnly,
    /// This product would write it.
    Managed,
}

impl Rendered {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            Rendered::ReadOnly => "read-only",
            Rendered::Managed => "managed",
        }
    }
}

/// What the current selection would render, path by path, with the source
/// that renders each one: an adapter, the governance producer's revision, or
/// the setup profile's revision. A path not named is rendered absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rendering {
    paths: BTreeMap<String, (Rendered, String)>,
}

impl Rendering {
    /// An empty rendering.
    pub fn new() -> Self {
        Self::default()
    }

    /// Name a path. Where two sources name one path, `managed` is kept over
    /// `read-only`, because a write by any of them is a write.
    pub fn name(&mut self, path: &str, class: Rendered, source: &str) {
        match self.paths.get(path) {
            Some((held, _)) if *held >= class => {}
            _ => {
                self.paths
                    .insert(path.to_string(), (class, source.to_string()));
            }
        }
    }

    /// The class and source for a path, or `None` when the rendering does not
    /// name it.
    pub fn of(&self, path: &str) -> Option<&(Rendered, String)> {
        self.paths.get(path)
    }

    /// Name every path the selected adapters declare, as `env plan` reads
    /// them: each is a path this product would write. An adapter that does
    /// not claim its paths on this host still renders them; its absence is
    /// `doctor`'s `adapter-unavailable`, not a change of owner.
    pub fn name_declarations(&mut self, declarations: &[Declaration]) {
        for d in declarations {
            let source = format!("adapter {} {}", d.name, d.version);
            for path in d.paths() {
                self.name(path, Rendered::Managed, &source);
            }
        }
    }

    /// Every path the rendering names.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.paths.keys().map(String::as_str)
    }
}

/// The rendering, or why it could not be computed (section 3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderingFact {
    /// Computed, writing nothing.
    Computed(Rendering),
    /// The producer, the selection or the pin did not let it be computed.
    Unavailable(String),
}

/// Which two facts disagree, in the order of section 3.2's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pair {
    /// Recorded and journaled.
    RecordedJournaled,
    /// Journaled and rendered.
    JournaledRendered,
    /// Recorded and rendered.
    RecordedRendered,
}

impl Pair {
    /// The pair's name.
    pub fn word(self) -> &'static str {
        match self {
            Pair::RecordedJournaled => "recorded/journaled",
            Pair::JournaledRendered => "journaled/rendered",
            Pair::RecordedRendered => "recorded/rendered",
        }
    }
}

/// One side of a disagreement: the value and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    /// Which fact: `recorded`, `journaled` or `rendered`.
    pub name: &'static str,
    /// Its value: a class word, `none` or `absent`.
    pub value: String,
    /// Its source: the manifest entry, the journal record's identity, or the
    /// rendering's source.
    pub source: String,
}

/// One `ownership-disagreement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disagreement {
    /// The path.
    pub path: String,
    /// The pair.
    pub pair: Pair,
    /// The table row's index within the pair, for a stable order.
    row: u8,
    /// The first fact.
    pub left: Fact,
    /// The second fact.
    pub right: Fact,
    /// What the row predicts.
    pub consequence: &'static str,
    /// The verb an operator would use.
    pub verb: &'static str,
}

impl Disagreement {
    /// A one-line rendering.
    pub fn describe(&self) -> String {
        format!(
            "ownership-disagreement {} {}: {} {} ({}), {} {} ({}); {}; see `{}`",
            self.path,
            self.pair.word(),
            self.left.name,
            self.left.value,
            self.left.source,
            self.right.name,
            self.right.value,
            self.right.source,
            self.consequence,
            self.verb,
        )
    }
}

/// The three facts for every path any source names, compared.
///
/// `foreign` holds the paths `doctor` already reports as `foreign`: the row
/// that overlaps it is omitted for them (section 3.2). Ordered by path, then
/// by pair in the table's order (section 3.4).
pub fn disagreements(
    root: &Path,
    manifest: &Manifest,
    rendering: Option<&Rendering>,
    foreign: &BTreeSet<String>,
) -> Vec<Disagreement> {
    let mut paths: BTreeSet<&str> = manifest.entries.iter().map(|e| e.path.as_str()).collect();
    paths.extend(manifest.transfers.iter().map(|r| r.path.as_str()));
    if let Some(r) = rendering {
        paths.extend(r.paths());
    }

    let mut out = Vec::new();
    for path in paths {
        let recorded = Ownership::of(manifest, path);
        let recorded_fact = || Fact {
            name: "recorded",
            value: recorded.word().to_string(),
            source: match manifest.entry(path) {
                Some(e) => format!(
                    "manifest entry, source {} {}",
                    match e.source.kind {
                        crate::manifest::SourceKind::Adapter => "adapter",
                        crate::manifest::SourceKind::Template => "template",
                    },
                    e.source.identity
                ),
                None => "no manifest entry".to_string(),
            },
        };
        // Section 3.1, fact 2: the latest record, unless a recorded operation
        // of this product followed it (spec 002 section 3.35 rule 5).
        let journaled =
            transfer::latest(manifest, path).filter(|r| !transfer::superseded(manifest, r));
        let journaled_fact = || match journaled {
            Some(r) => Fact {
                name: "journaled",
                value: r.to.word().to_string(),
                source: format!("journal record {}", r.id),
            },
            None => Fact {
                name: "journaled",
                value: "none".to_string(),
                source: "no journal record".to_string(),
            },
        };
        let rendered = rendering.map(|r| r.of(path));
        let rendered_fact = || match rendered.flatten() {
            Some((class, source)) => Fact {
                name: "rendered",
                value: class.word().to_string(),
                source: source.clone(),
            },
            None => Fact {
                name: "rendered",
                value: "absent".to_string(),
                source: "the current selection's rendering".to_string(),
            },
        };
        let mut push = |pair, row, left: Fact, right: Fact, consequence, verb| {
            out.push(Disagreement {
                path: path.to_string(),
                pair,
                row,
                left,
                right,
                consequence,
                verb,
            });
        };

        if let Some(r) = journaled
            && r.to != recorded
        {
            push(
                Pair::RecordedJournaled,
                0,
                recorded_fact(),
                journaled_fact(),
                "a transfer of this path is refused until they agree",
                "transfer plan",
            );
        }
        // Without a rendering only recorded and journaled are compared.
        let Some(rendered) = rendered else {
            continue;
        };
        let rendered_class = rendered.map(|(c, _)| *c);
        if journaled.is_some_and(|r| r.to == Ownership::User)
            && rendered_class == Some(Rendered::Managed)
        {
            push(
                Pair::JournaledRendered,
                0,
                journaled_fact(),
                rendered_fact(),
                "the next apply would write a file the operator released",
                "env plan",
            );
        }
        if recorded == Ownership::Adopted && rendered_class == Some(Rendered::Managed) {
            push(
                Pair::RecordedRendered,
                0,
                recorded_fact(),
                rendered_fact(),
                "the selection asks to write a path the manifest says is never rewritten",
                "env plan",
            );
        }
        if manifest
            .entry(path)
            .is_some_and(|e| e.class == Class::Managed)
            && rendered_class.is_none()
        {
            push(
                Pair::RecordedRendered,
                1,
                recorded_fact(),
                rendered_fact(),
                "no source rewrites it, yet `env remove` would delete it",
                "env plan",
            );
        }
        if recorded == Ownership::User
            && journaled.is_none()
            && rendered_class == Some(Rendered::Managed)
            && !foreign.contains(path)
            && std::fs::symlink_metadata(resolve(root, path)).is_ok()
        {
            push(
                Pair::RecordedRendered,
                2,
                recorded_fact(),
                rendered_fact(),
                "the path would be written over bytes this product never recorded",
                "env plan",
            );
        }
    }
    out.sort_by(|a, b| (&a.path, a.pair, a.row).cmp(&(&b.path, b.pair, b.row)));
    out
}

/// Add spec 026's findings to a `doctor` report: every disagreement, or, when
/// the rendering could not be computed, one `ownership-rendering-unavailable`
/// note and the recorded and journaled comparison alone.
pub fn report(root: &Path, manifest: &Manifest, rendering: &RenderingFact, report: &mut Report) {
    let mut foreign: BTreeSet<String> = report
        .findings
        .iter()
        .filter_map(|f| match f {
            Finding::Foreign { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    foreign.extend(
        report
            .entries
            .iter()
            .filter(|e| matches!(e.state, State::Foreign { .. }))
            .map(|e| e.path.clone()),
    );
    let computed = match rendering {
        RenderingFact::Computed(r) => Some(r),
        RenderingFact::Unavailable(reason) => {
            report.notes.push(format!(
                "ownership-rendering-unavailable: {reason}; only recorded and journaled ownership were compared"
            ));
            None
        }
    };
    report.findings.extend(
        disagreements(root, manifest, computed, &foreign)
            .into_iter()
            .map(Finding::OwnershipDisagreement),
    );
}
