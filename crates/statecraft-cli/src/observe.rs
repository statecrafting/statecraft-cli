//! Spec 027's verbs (section 3.9): each takes the target path first, calls one
//! library operation in `statecraft-observe`, and answers in the family
//! envelope with spec 006's exit contract.

use serde_json::{Value, json};
use statecraft_environment::registry::Registry;
use statecraft_observe::observe::{Git, Refusal, Request, Sources};
use statecraft_observe::store::{self, Store, StoreError};
use std::path::{Path, PathBuf};

use crate::application::{failed, registered};
use crate::exit::Exit;
use crate::render::Answer;

/// What a verb answers: an envelope, or a usage message.
pub type Outcome = Result<Answer<Value>, String>;

/// `observe`'s usage.
pub const OBSERVE_USAGE: &str = "usage: observe <path> [--ref <ref>]";
/// `observe show`'s usage.
pub const SHOW_USAGE: &str = "usage: observe show <path> [--ref <ref>]";

/// The target and the optional `--ref`, in any order after the path.
fn operands(rest: &[String], usage: &str) -> Result<(PathBuf, Option<String>), String> {
    let mut args = rest.iter().filter(|a| a.as_str() != "--json");
    let path = args.next().ok_or_else(|| usage.to_string())?;
    let mut git_ref = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ref" if git_ref.is_none() => {
                git_ref = Some(args.next().ok_or_else(|| usage.to_string())?.clone());
            }
            _ => return Err(usage.to_string()),
        }
    }
    Ok((absolute(path), git_ref))
}

fn store_answer(e: &StoreError) -> Answer<Value> {
    match e {
        StoreError::Busy { .. } => Answer::new(
            json!({ "outcome": "refused", "reason": "observation-lock-held", "detail": e.to_string() }),
            Exit::Refused,
            format!("refused: {e}\n"),
        ),
        _ => failed(&e.to_string()),
    }
}

fn refusal_answer(r: Refusal) -> Answer<Value> {
    match r {
        Refusal::Ref(detail) => Answer::new(
            json!({ "outcome": "refused", "reason": "ref-unresolved", "detail": detail }),
            Exit::Refused,
            format!("refused: {detail}\n"),
        ),
        Refusal::Report(e) => crate::slice::report_error_answer(&e).erase(),
        Refusal::Store(e) => store_answer(&e),
        Refusal::Materialize(detail) | Refusal::Overrides(detail) => failed(&detail),
    }
}

fn git(home: &Path) -> Git {
    Git {
        trees: store::directory(home).join("trees"),
    }
}

/// `observe <path> [--ref <ref>]`: one observation, under the observation
/// lock (sections 3.1 to 3.5). Exit 0 for `answered` or `unchanged`, 1 for
/// `refused`, 2 when the lock is held or the ref does not resolve, 4 when the
/// chain cannot be written.
pub fn observe(
    home: &Path,
    registry: &Registry,
    clock: &dyn statecraft_environment::time::Clock,
    rest: &[String],
) -> Outcome {
    let (path, git_ref) = operands(rest, OBSERVE_USAGE)?;
    let root = match registered(registry, &path) {
        Ok(root) => root,
        Err(answer) => return Ok(answer),
    };
    let held = match store::lock(home, &root) {
        Ok(held) => held,
        Err(e) => return Ok(store_answer(&e)),
    };
    let mut chain = match Store::open(home, &root) {
        Ok(chain) => chain,
        Err(e) => return Ok(store_answer(&e)),
    };
    let judge = match crate::judge::report_source(&root) {
        Ok(judge) => judge,
        Err(e) => return Ok(crate::slice::report_error_answer(&e).erase()),
    };
    let commits = git(home);
    let at = statecraft_environment::time::rfc3339_utc(clock.now_unix());
    let found = statecraft_observe::observe::observe(
        &held,
        &mut chain,
        &Request {
            home,
            root: &root,
            git_ref: git_ref.as_deref(),
            at: &at,
        },
        &Sources {
            commits: &commits,
            reports: &judge,
            contracts: &judge,
        },
    );
    let found = match found {
        Ok(found) => found,
        Err(r) => return Ok(refusal_answer(r)),
    };
    let exit = if found.result == "refused" {
        Exit::Finding
    } else {
        Exit::Ok
    };
    let mut summary = format!(
        "observation {} of {} at {}: {}\n",
        short(&found.observation),
        found.git_ref,
        short(&found.commit),
        found.result
    );
    if let Some(refusal) = &found.refusal {
        summary.push_str(&format!(
            "  spec-spine refused ({}): {}\n",
            refusal["class"].as_str().unwrap_or_default(),
            refusal["message"].as_str().unwrap_or_default()
        ));
    }
    for t in &found.transitions {
        summary.push_str(&format!(
            "  {} {} {}\n",
            t.projection(),
            t.spec().unwrap_or("(repository)"),
            t.kind()
        ));
    }
    for p in &found.proposals {
        summary.push_str(&format!(
            "  {} {} {} for {}\n",
            p.entry,
            p.kind.word(),
            short(&p.proposal),
            p.scope.spec().unwrap_or("the repository")
        ));
    }
    for (old, by) in &found.superseded {
        summary.push_str(&format!("  superseded {} by {}\n", short(old), short(by)));
    }
    let value = serde_json::to_value(&found).expect("an observation serializes");
    Ok(Answer::new(value, exit, summary))
}

/// `observe show <path> [--ref <ref>]`: the last answered observation, and
/// whether the ref now resolves to a commit with no `observed` entry. Takes
/// no lock and writes nothing.
pub fn show(home: &Path, registry: &Registry, rest: &[String]) -> Outcome {
    let (path, git_ref) = operands(rest, SHOW_USAGE)?;
    let root = match registered(registry, &path) {
        Ok(root) => root,
        Err(answer) => return Ok(answer),
    };
    let chain = match Store::open(home, &root) {
        Ok(chain) => chain,
        Err(e) => return Ok(store_answer(&e)),
    };
    let shown =
        match statecraft_observe::observe::show(&chain, &root, git_ref.as_deref(), &git(home)) {
            Ok(shown) => shown,
            Err(r) => return Ok(refusal_answer(r)),
        };
    let mut summary = match &shown.last_answered {
        None => "none: no answered observation is recorded for this repository\n".to_string(),
        Some(o) => format!(
            "last answered observation {} of {} at {}: {} eligible\n",
            short(&o.observation),
            o.git_ref,
            short(&o.commit),
            o.projections.eligible().count()
        ),
    };
    summary.push_str(&format!(
        "  {} resolves to {}{}\n",
        shown.git_ref,
        short(&shown.commit),
        if shown.unobserved {
            ", which is not observed"
        } else {
            ", which is observed"
        }
    ));
    if let Some(bytes) = shown.torn_tail {
        summary.push_str(&format!(
            "  the chain ends in an incomplete entry of {bytes} byte(s), not interpreted\n"
        ));
    }
    let value = serde_json::to_value(&shown).expect("a shown observation serializes");
    Ok(Answer::new(value, Exit::Ok, summary))
}

/// What the operator typed, made absolute without resolving links.
fn absolute(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        return p;
    }
    std::env::current_dir().map_or(p.clone(), |cwd| cwd.join(p))
}

fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}
