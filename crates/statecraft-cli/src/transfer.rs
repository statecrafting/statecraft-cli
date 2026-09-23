//! The transfer verbs: `transfer plan`, `transfer apply`, `transfer revert`.
//!
//! Spec 006 section 3.11.7, for spec 002 section 3.35. Each verb is a binding in
//! the sense of section 3.2: it parses its arguments, calls one operation in
//! `statecraft_environment::transfer`, renders what came back, and maps it to
//! the codes section 3.11.7 fixes. Which moves are admitted, which paths are
//! protected, what makes a plan stale and when a reversal is refused are all
//! decided there, not here.

use crate::adapters;
use crate::exit::Exit;
use crate::render::Answer;
use statecraft_environment::claimant::ForeignClaims;
use statecraft_environment::time::SystemClock;
use statecraft_environment::transfer::{
    self, Act, Context, Outcome, Ownership, Plan, TransferError,
};
use std::path::{Path, PathBuf};

/// What a transfer verb was asked to do, once its arguments parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// `transfer plan <path> <file> <from> <to>`
    Plan {
        /// The repository.
        root: PathBuf,
        /// The repository-relative file.
        file: String,
        /// The class named as current.
        from: Ownership,
        /// The class it is to take.
        to: Ownership,
    },
    /// `transfer apply <path> <file> <from> <to> <plan-id> <operator> <reason...>`
    Apply {
        /// The repository.
        root: PathBuf,
        /// The repository-relative file.
        file: String,
        /// The class named as current.
        from: Ownership,
        /// The class it is to take.
        to: Ownership,
        /// The plan identity `transfer plan` printed.
        plan_id: String,
        /// Who, as supplied.
        operator: String,
        /// Why.
        reason: String,
    },
    /// `transfer revert <path> <transfer-id> <operator> <reason...>`
    Revert {
        /// The repository.
        root: PathBuf,
        /// The journal record to reverse.
        transfer_id: String,
        /// Who, as supplied.
        operator: String,
        /// Why.
        reason: String,
    },
}

impl Request {
    /// The repository the request names.
    pub fn root(&self) -> &Path {
        match self {
            Request::Plan { root, .. }
            | Request::Apply { root, .. }
            | Request::Revert { root, .. } => root,
        }
    }
}

/// What each verb takes after its name.
pub fn usage(verb: crate::Verb) -> &'static str {
    match verb {
        crate::Verb::TransferPlan => " <path> <file> <from> <to>",
        crate::Verb::TransferApply => " <path> <file> <from> <to> <plan-id> <operator> <reason...>",
        crate::Verb::TransferRevert => " <path> <transfer-id> <operator> <reason...>",
        _ => "",
    }
}

fn class(word: &str) -> Result<Ownership, String> {
    Ownership::from_word(word).ok_or_else(|| {
        format!("`{word}` is not a class: <from> and <to> are user, adopted or managed")
    })
}

/// The operator and the reason, as supplied. Their presence is usage; an
/// operator or reason that is present and blank is the library's refusal
/// (exit 2), as it is for the override verbs of section 3.11.5.
fn operator_and_reason(rest: &[String]) -> (String, String) {
    let operator = rest.first().cloned().unwrap_or_default();
    let reason = rest.get(1..).unwrap_or_default().join(" ");
    (operator, reason)
}

/// Parse a transfer verb's arguments. An `Err` is a usage error (exit 3): a
/// missing argument, or a class word this verb does not have.
pub fn parse(
    verb: crate::Verb,
    rest: &[String],
    absolute: impl Fn(&str) -> PathBuf,
) -> Result<Request, String> {
    let missing = || format!("usage: {}{}", verb.spelling(), usage(verb));
    match verb {
        crate::Verb::TransferPlan => {
            let [path, file, from, to] = rest else {
                return Err(missing());
            };
            Ok(Request::Plan {
                root: absolute(path),
                file: file.clone(),
                from: class(from)?,
                to: class(to)?,
            })
        }
        crate::Verb::TransferApply => {
            if rest.len() < 7 {
                return Err(missing());
            }
            let (operator, reason) = operator_and_reason(&rest[5..]);
            Ok(Request::Apply {
                root: absolute(&rest[0]),
                file: rest[1].clone(),
                from: class(&rest[2])?,
                to: class(&rest[3])?,
                plan_id: rest[4].clone(),
                operator,
                reason,
            })
        }
        crate::Verb::TransferRevert => {
            if rest.len() < 4 {
                return Err(missing());
            }
            let (operator, reason) = operator_and_reason(&rest[2..]);
            Ok(Request::Revert {
                root: absolute(&rest[0]),
                transfer_id: rest[1].clone(),
                operator,
                reason,
            })
        }
        _ => Err(missing()),
    }
}

/// The producer revision this build links, as the record names it.
pub fn producer() -> String {
    format!(
        "{}@{}",
        statecraft_home::producer::PRODUCER_NAME,
        statecraft_home::producer::PRODUCER_VERSION
    )
}

/// Perform a parsed request against the configured adapter set, with the
/// same probe `env apply` decides the adapter's readiness with, so a move to
/// `managed` is admitted only where the adapter claims its paths.
pub fn execute(request: &Request, home: &Path) -> Answer<serde_json::Value> {
    let declarations = adapters::declarations();
    let probe = adapters::probe(home);
    let foreign = ForeignClaims::none();
    let producer = producer();
    let ctx = Context {
        root: request.root(),
        producer: &producer,
        declarations: &declarations,
        probe: &probe,
        foreign: &foreign,
    };
    match request {
        Request::Plan { file, from, to, .. } => {
            plan_answer(request.root(), transfer::plan(&ctx, file, *from, *to))
        }
        Request::Apply {
            file,
            from,
            to,
            plan_id,
            operator,
            reason,
            ..
        } => outcome_answer(transfer::apply(
            &ctx,
            file,
            *from,
            *to,
            plan_id,
            &Act { operator, reason },
            &SystemClock,
        )),
        Request::Revert {
            transfer_id,
            operator,
            reason,
            ..
        } => outcome_answer(transfer::revert(
            &ctx,
            transfer_id,
            &Act { operator, reason },
            &SystemClock,
        )),
    }
}

fn error_answer(e: TransferError) -> Answer<serde_json::Value> {
    match e {
        TransferError::Refused(r) => {
            let mut summary = format!("refused ({}): {}", kind(&r), r.detail);
            if !r.changed.is_empty() {
                summary.push_str(&format!(
                    "\n  changed since the plan: {}",
                    r.changed.join(", ")
                ));
            }
            summary.push_str("\nnothing was written\n");
            Answer::new(serde_json::json!({ "refused": r }), Exit::Refused, summary)
        }
        // Section 3.11.7's 4, told truthfully: the transfer is in force, and
        // what is not known is that it survives a crash.
        TransferError::NotDurable { outcome, detail } => {
            let summary = format!(
                "failed durably: {} is in force (transfer {}), but {}; run `transfer plan` or \
                 read the manifest to confirm it after a crash\n",
                outcome.word(),
                outcome.record().id,
                detail
            );
            Answer::new(
                serde_json::json!({
                    "failed": "not-durable",
                    "in_force": *outcome,
                    "detail": detail,
                }),
                Exit::Failed,
                summary,
            )
        }
        // Section 3.11.7's 4: the manifest, or the file whose digest a plan
        // needs, could not be read or written durably.
        e => Answer::new(
            serde_json::json!({ "failed": e.to_string() }),
            Exit::Failed,
            format!("failed: {e}\n"),
        ),
    }
}

fn kind(r: &transfer::Refusal) -> String {
    serde_json::to_value(r.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// `transfer plan`, from what the library returned.
pub fn plan_answer(root: &Path, result: Result<Plan, TransferError>) -> Answer<serde_json::Value> {
    let p = match result {
        Ok(p) => p,
        Err(e) => return error_answer(e),
    };
    let mut s = format!(
        "transfer plan for {}: {} -> {}\n",
        p.path,
        p.from.word(),
        p.to.word()
    );
    s.push_str(&format!("  current class: {}", p.current.class.word()));
    match (&p.current.recorded_digest, p.current.matches_record) {
        (Some(d), Some(true)) => s.push_str(&format!(" (recorded {d}, the file matches)\n")),
        (Some(d), _) => s.push_str(&format!(" (recorded {d}, the file differs)\n")),
        _ => s.push_str(" (not in the manifest)\n"),
    }
    if let Some(adapter) = &p.current.declared_by {
        s.push_str(&format!("  declared by adapter: {adapter}\n"));
    }
    s.push_str(&format!("  file now: {} ({} bytes)\n", p.digest, p.bytes));
    match &p.resulting {
        Some(e) => s.push_str(&format!(
            "  resulting entry: {} from {} {}\n",
            match e.class {
                statecraft_environment::Class::Managed => "managed",
                statecraft_environment::Class::Adopted => "adopted",
            },
            match e.source.kind {
                statecraft_environment::manifest::SourceKind::Adapter => "adapter",
                statecraft_environment::manifest::SourceKind::Template => "template",
            },
            e.source.identity
        )),
        None => s.push_str("  resulting entry: none; the path becomes user class\n"),
    }
    s.push_str(&format!("  producer: {}\n", p.producer));
    s.push_str(&format!("  manifest now: {}\n", p.manifest_digest));
    s.push_str(&format!("  plan identity: {}\n", p.identity));
    s.push_str(&format!("  plan token: {}\n", p.plan_id));
    for path in &p.recorded_without_journal {
        s.push_str(&format!(
            "  note: {path} carries a transfer recorded without a journal; it is read as before \
             and never rewritten\n"
        ));
    }
    for d in &p.journal_disagreements {
        s.push_str(&format!("  journal disagrees: {d}\n"));
    }
    if p.journal_disagreements.is_empty() {
        s.push_str(&format!(
            "apply with: transfer apply {} {} {} {} {} <operator> <reason...>\n",
            root.display(),
            p.path,
            p.from.word(),
            p.to.word(),
            p.plan_id
        ));
    } else {
        s.push_str("transfer apply and transfer revert refuse until the journal agrees\n");
    }
    Answer::new(serde_json::to_value(&p).unwrap_or_default(), Exit::Ok, s)
}

/// `transfer apply` and `transfer revert`, from what the library returned.
pub fn outcome_answer(result: Result<Outcome, TransferError>) -> Answer<serde_json::Value> {
    let o = match result {
        Ok(o) => o,
        Err(e) => return error_answer(e),
    };
    let r = o.record();
    let mut s = format!(
        "{}: {} {} -> {} (transfer {})\n",
        o.word(),
        r.path,
        r.from.word(),
        r.to.word(),
        r.id
    );
    match &o {
        Outcome::AlreadySatisfied { .. } => {
            s.push_str("  the latest record already made this move; nothing was written\n");
        }
        _ => {
            s.push_str(&format!("  digest {} ({} bytes)\n", r.digest, r.bytes));
            s.push_str(&format!(
                "  by {} ({}): {}\n",
                r.operator, r.operator_provenance, r.reason
            ));
            if let Some(reverted) = &r.reverts {
                s.push_str(&format!("  reverses {reverted}\n"));
            }
            s.push_str("  ownership changed; no byte of the file did\n");
        }
    }
    for leftover in o.removed_leftovers() {
        s.push_str(&format!(
            "  removed {leftover}, a temporary file an interrupted earlier manifest write left \
             in .statecraft/\n"
        ));
    }
    Answer::new(serde_json::to_value(&o).unwrap_or_default(), Exit::Ok, s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_environment::transfer::TransferRecord;

    fn record() -> TransferRecord {
        TransferRecord {
            id: "t1".into(),
            path: "notes.md".into(),
            from: Ownership::User,
            to: Ownership::Adopted,
            digest: "d".into(),
            bytes: 1,
            producer: producer(),
            operator: "bart".into(),
            operator_provenance: "operator-supplied".into(),
            reason: "why".into(),
            at: "2026-09-23T00:00:00Z".into(),
            manifest_before: "m".into(),
            reverts: None,
        }
    }

    // Section 3.11.7's 4, when the rename is in force and the directory flush
    // failed: a failure, and an answer that says what is in force.
    #[test]
    fn a_transfer_in_force_but_not_durable_is_exit_4_and_says_it_is_in_force() {
        let answer = outcome_answer(Err(TransferError::NotDurable {
            outcome: Box::new(Outcome::Applied {
                record: record(),
                write: Default::default(),
            }),
            detail: "fsync failed".into(),
        }));
        assert_eq!(answer.exit, Exit::Failed);
        assert_eq!(answer.value["failed"], "not-durable");
        assert_eq!(answer.value["in_force"]["record"]["id"], "t1");
        assert!(answer.summary.contains("is in force"), "{}", answer.summary);
    }

    #[test]
    fn removed_leftovers_are_reported() {
        let answer = outcome_answer(Ok(Outcome::Applied {
            record: record(),
            write: statecraft_environment::manifest::Written {
                removed_leftovers: vec![".environment.json.9.0.tmp".into()],
            },
        }));
        assert_eq!(answer.exit, Exit::Ok);
        assert!(answer.summary.contains(".environment.json.9.0.tmp"));
        assert_eq!(
            answer.value["write"]["removed_leftovers"][0],
            ".environment.json.9.0.tmp"
        );
    }
}
