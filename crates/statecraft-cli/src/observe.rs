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
        Refusal::Materialize(detail) | Refusal::Overrides(detail) | Refusal::RunRecord(detail) => {
            failed(&detail)
        }
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
    let at = statecraft_environment::time::rfc3339_utc(clock.now_unix());
    if let Err(answer) = recovered(&held, &mut chain, home, &root, &at) {
        return Ok(answer);
    }
    let judge = match crate::judge::report_source(&root) {
        Ok(judge) => judge,
        Err(e) => return Ok(crate::slice::report_error_answer(&e).erase()),
    };
    let commits = git(home);
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

/// `proposal list`'s usage.
pub const LIST_USAGE: &str = "usage: proposal list <path>";
/// `proposal show`'s usage.
pub const PROPOSAL_SHOW_USAGE: &str = "usage: proposal show <path> <id>";
/// `proposal dismiss`'s usage.
pub const DISMISS_USAGE: &str = "usage: proposal dismiss <path> <id> <operator> <reason...>";
/// `proposal acknowledge`'s usage.
pub const ACKNOWLEDGE_USAGE: &str = "usage: proposal acknowledge <path> <id> <operator>";

fn positional(rest: &[String]) -> Vec<&str> {
    rest.iter()
        .filter(|a| a.as_str() != "--json")
        .map(String::as_str)
        .collect()
}

fn not_admissible(why: &statecraft_observe::proposals::NotAdmissible) -> Answer<Value> {
    use statecraft_observe::proposals::NotAdmissible;
    let reason = match why {
        NotAdmissible::Unknown(_) => "no-such-proposal",
        NotAdmissible::WrongKind { .. } => "wrong-kind",
        NotAdmissible::NotOpen(_) => "not-open",
    };
    let answer = Answer::new(
        json!({ "outcome": "refused", "reason": reason, "detail": why.to_string() }),
        Exit::Refused,
        format!("refused: {why}\n"),
    );
    if matches!(why, NotAdmissible::Unknown(_)) {
        answer.with_kind(crate::render::ErrorKind::NotFound)
    } else {
        answer
    }
}

/// Recover every orphaned admission under the caller's hold (section 3.8):
/// the first thing every chain writer does after taking the lock.
fn recovered(
    held: &store::Held,
    chain: &mut Store,
    home: &Path,
    root: &Path,
    at: &str,
) -> Result<Vec<statecraft_observe::proposal::Entry>, Answer<Value>> {
    statecraft_observe::proposals::recover(held, chain, home, root, at).map_err(|e| match e {
        statecraft_observe::proposals::RecoverError::Store(e) => store_answer(&e),
        other => failed(&other.to_string()),
    })
}

/// `proposal list <path>`: open proposals in the order first proposed, and
/// any admission not yet recovered. Takes no lock and writes nothing.
pub fn proposal_list(home: &Path, registry: &Registry, rest: &[String]) -> Outcome {
    let args = positional(rest);
    let [path] = args.as_slice() else {
        return Err(LIST_USAGE.to_string());
    };
    let root = match registered(registry, &absolute(path)) {
        Ok(root) => root,
        Err(answer) => return Ok(answer),
    };
    let chain = match Store::open(home, &root) {
        Ok(chain) => chain,
        Err(e) => return Ok(store_answer(&e)),
    };
    let ledger = statecraft_observe::proposal::Ledger::fold(chain.entries().to_vec());
    let rows = statecraft_observe::proposals::list(&ledger);
    let mut summary = format!("{} open proposal(s)\n", rows.len());
    for r in &rows {
        summary.push_str(&format!(
            "  {} {} for {} ({}, {}): {}\n",
            r.kind.word(),
            short(&r.proposal),
            r.spec.as_deref().unwrap_or("the repository"),
            r.subscription,
            r.git_ref,
            r.state
        ));
    }
    Ok(Answer::new(
        json!({ "proposals": rows, "tornTail": chain.torn_tail().map(|t| t.bytes) }),
        Exit::Ok,
        summary,
    ))
}

/// spec-spine's `delta` between two commits, verbatim, or `unavailable` with
/// its reason (section 3.9): never derived from the projections.
fn delta(root: &Path, base: &str, head: &str) -> Value {
    let program = match crate::judge::program(root) {
        Ok(p) => p,
        Err(why) => return json!({ "state": "unavailable", "reason": why }),
    };
    match std::process::Command::new(&program)
        .args(["delta", "--base", base, "--head", head, "--json"])
        .current_dir(root)
        .output()
    {
        Ok(out) if out.status.success() => match serde_json::from_slice::<Value>(&out.stdout) {
            Ok(report) => json!({ "state": "reported", "report": report }),
            Err(e) => json!({ "state": "unavailable", "reason": format!("not JSON: {e}") }),
        },
        Ok(out) => json!({
            "state": "unavailable",
            "reason": String::from_utf8_lossy(&out.stderr).trim().to_string(),
        }),
        Err(e) => json!({ "state": "unavailable", "reason": e.to_string() }),
    }
}

/// `proposal show <path> <id>`: the causal trace. Takes no lock and writes
/// nothing.
pub fn proposal_show(home: &Path, registry: &Registry, rest: &[String]) -> Outcome {
    use statecraft_observe::proposal::{Disposition, Ledger};
    let args = positional(rest);
    let [path, id] = args.as_slice() else {
        return Err(PROPOSAL_SHOW_USAGE.to_string());
    };
    let root = match registered(registry, &absolute(path)) {
        Ok(root) => root,
        Err(answer) => return Ok(answer),
    };
    let chain = match Store::open(home, &root) {
        Ok(chain) => chain,
        Err(e) => return Ok(store_answer(&e)),
    };
    let ledger = Ledger::fold(chain.entries().to_vec());
    let Some(p) = ledger.proposal(id) else {
        return Ok(not_admissible(
            &statecraft_observe::proposals::NotAdmissible::Unknown(id.to_string()),
        ));
    };
    let (causes, dispositions) = statecraft_observe::proposals::trace(&ledger, chain.entries(), p);
    let causes: Vec<Value> = causes
        .into_iter()
        .map(|c| {
            let explanation = match &c.compared_with {
                None => json!({ "state": "initial", "commit": c.commit }),
                Some(base) => delta(&root, base, &c.commit),
            };
            let mut v = serde_json::to_value(&c).expect("a cause serializes");
            v["delta"] = explanation;
            v
        })
        .collect();
    let mut run = Value::Null;
    if let statecraft_observe::proposal::State::Disposed(Disposition::Admitted { run_id, attempt }) =
        &p.state
        && let Ok((record, _)) = statecraft_run::record::Chain::open(home, &root)
    {
        let outcome = statecraft_run::session::runs(&record)
            .into_iter()
            .find(|r| &r.id == run_id)
            .and_then(|r| r.attempts.into_iter().find(|a| a.number == *attempt))
            .and_then(|a| a.outcome.map(|o| o.word().to_string()));
        // Spec 005 judges acceptance when `accept` is asked and records no
        // verdict, so the record has none to report.
        run = json!({
            "runId": run_id,
            "attempt": attempt,
            "outcome": outcome,
            "acceptance": "not-recorded",
        });
    }
    let dispositions: Vec<Value> = dispositions
        .into_iter()
        .map(|(position, e)| {
            let mut v = serde_json::to_value(&e).expect("an entry serializes");
            v["position"] = json!(position);
            v
        })
        .collect();
    let summary = format!(
        "{} {} for {} ({}): {}, {} cause(s)\n",
        p.kind.word(),
        short(&p.id),
        p.scope.spec().unwrap_or("the repository"),
        p.subscription.id(),
        p.state.word(),
        causes.len()
    );
    Ok(Answer::new(
        json!({
            "proposal": p.id,
            "kind": p.kind,
            "scope": p.scope,
            "subscription": p.subscription.id(),
            "row": p.row,
            "state": p.state.word(),
            "causes": causes,
            "dispositions": dispositions,
            "run": run,
        }),
        Exit::Ok,
        summary,
    ))
}

/// `proposal dismiss` and `proposal acknowledge`: under the observation lock,
/// recover any orphaned admission, then write the one disposition. Exit 0
/// when written; 2 when the lock is held or the proposal is of the other kind
/// or not open.
pub fn proposal_close(
    home: &Path,
    registry: &Registry,
    clock: &dyn statecraft_environment::time::Clock,
    rest: &[String],
    acknowledge: bool,
) -> Outcome {
    use statecraft_observe::proposals::Closing;
    let args = positional(rest);
    let (path, id, closing) = if acknowledge {
        let [path, id, operator] = args.as_slice() else {
            return Err(ACKNOWLEDGE_USAGE.to_string());
        };
        (
            *path,
            *id,
            Closing::Acknowledged {
                operator: operator.to_string(),
            },
        )
    } else {
        let [path, id, operator, reason @ ..] = args.as_slice() else {
            return Err(DISMISS_USAGE.to_string());
        };
        if reason.is_empty() {
            return Err(DISMISS_USAGE.to_string());
        }
        (
            *path,
            *id,
            Closing::Dismissed {
                operator: operator.to_string(),
                reason: reason.join(" "),
            },
        )
    };
    let root = match registered(registry, &absolute(path)) {
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
    let at = statecraft_environment::time::rfc3339_utc(clock.now_unix());
    if let Err(answer) = recovered(&held, &mut chain, home, &root, &at) {
        return Ok(answer);
    }
    match statecraft_observe::proposals::close(&held, &mut chain, id, closing, &at) {
        Err(e) => Ok(store_answer(&e)),
        Ok(Err(why)) => Ok(not_admissible(&why)),
        Ok(Ok(entry)) => {
            let word = if acknowledge {
                "acknowledged"
            } else {
                "dismissed"
            };
            Ok(Answer::new(
                serde_json::to_value(&entry).expect("an entry serializes"),
                Exit::Ok,
                format!("{word} {}\n", short(id)),
            ))
        }
    }
}

/// `proposal admit`'s usage.
pub const ADMIT_USAGE: &str = "usage: proposal admit <path> <id> <operator> <reason...>";

/// `proposal admit <path> <id> <operator> <reason...>` (section 3.6): under
/// the observation lock, recover, observe the proposal's ref afresh, and
/// either dispose it `stale` (exit 1) or enter spec 003's run path at the
/// fresh commit through spec 040's `run` operation, which takes the
/// repository lock itself (observation lock, then repository lock, in that
/// order only). Admitted, it answers as `run` does; refused by spec 003, it
/// answers that refusal and the proposal is open again.
pub fn proposal_admit(
    home: &Path,
    registry: &Registry,
    clock: &dyn statecraft_environment::time::Clock,
    rest: &[String],
) -> Outcome {
    use statecraft_observe::proposals::{Admission, AdmitError, admit};
    let args = positional(rest);
    let [path, id, operator, reason @ ..] = args.as_slice() else {
        return Err(ADMIT_USAGE.to_string());
    };
    if reason.is_empty() {
        return Err(ADMIT_USAGE.to_string());
    }
    let reason = reason.join(" ");
    let path = absolute(path);
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
    let at = statecraft_environment::time::rfc3339_utc(clock.now_unix());
    if let Err(answer) = recovered(&held, &mut chain, home, &root, &at) {
        return Ok(answer);
    }
    let judge = match crate::judge::report_source(&root) {
        Ok(judge) => judge,
        Err(e) => return Ok(crate::slice::report_error_answer(&e).erase()),
    };
    let commits = git(home);
    let ctx = crate::application::Context {
        home,
        registry,
        clock,
    };
    let admitted = admit(
        &held,
        &mut chain,
        &Request {
            home,
            root: &root,
            git_ref: None,
            at: &at,
        },
        &Sources {
            commits: &commits,
            reports: &judge,
            contracts: &judge,
        },
        id,
        operator,
        &reason,
        |spec, commit| {
            crate::application::run_admitted_proposal(
                &ctx,
                &root,
                spec,
                &crate::application::Origin {
                    commit,
                    proposal: id,
                },
            )
        },
    );
    match admitted {
        Ok(Admission::Admitted { answer, .. } | Admission::Released { answer }) => Ok(answer),
        Ok(Admission::Stale { differing, .. }) => Ok(Answer::new(
            json!({ "proposal": id, "disposition": "stale", "differing": differing }),
            Exit::Finding,
            format!("stale: {differing}; nothing was run\n"),
        )),
        Err(AdmitError::NotAdmissible(why)) => Ok(not_admissible(&why)),
        Err(AdmitError::Observe(r)) => Ok(refusal_answer(r)),
        Err(AdmitError::Store(e)) => Ok(store_answer(&e)),
        Err(AdmitError::RunRecord(detail) | AdmitError::Unscoped(detail)) => Ok(failed(&detail)),
    }
}
