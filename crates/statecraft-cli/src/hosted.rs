//! The hosted verbs (spec 038): `hosted login`, `hosted logout`, `hosted
//! submit` and `hosted decision`, against the one cell a project's committed
//! `.statecraft/hosted.json` names.
//!
//! The HTTP, the grant and the credential custody are `statecraft-hosted`'s;
//! the offline bundle reader is the envelope crate's. What lives here is the
//! binding: options, the project's declaration, the receipt an `accept`
//! answered, the runtime record of a submission, and the answer. No token
//! reaches an answer, a message or a file this module writes, and no verb here
//! approves, merges or dispatches anything.

use serde_json::{Value, json};
use statecraft_envelope::bundle::{AdmissionCheck, DecisionBundle, verify};
use statecraft_envelope::dimensions::AdmissionPolicy;
use statecraft_envelope::issuer::{RootSetPin, RootSetV2, admit_pinned, strict_hash};
use statecraft_envelope::reference::Reference;
use statecraft_envelope::value::PORTABLE_MAX;
use statecraft_hosted::api::{self, Retrieved};
use statecraft_hosted::credential::{Credential, Store};
use statecraft_hosted::declaration::Declaration;
use statecraft_hosted::discovery::discover;
use statecraft_hosted::grant::{self, Revocation};
use statecraft_hosted::http::Http;
use statecraft_hosted::{Clock, Error as HostedError, Origin, SystemClock};
use std::path::{Path, PathBuf};

use crate::exit::Exit;
use crate::render::{Answer, ErrorKind};
use crate::trust::{Opts, envelope_refusal, number, parse_opts, read_input, refused, require};

/// What a hosted verb returns: an answer, or a usage message.
pub type Outcome = Result<Answer<Value>, String>;

/// `hosted login`'s usage line.
pub const LOGIN_USAGE: &str = "usage: hosted login <path> [--cell <origin>]";
/// `hosted logout`'s usage line.
pub const LOGOUT_USAGE: &str = "usage: hosted logout <path> [--cell <origin>]";
/// `hosted submit`'s usage line.
pub const SUBMIT_USAGE: &str =
    "usage: hosted submit <path> --receipt <accept-answer.json> [--plan]";
/// `hosted decision`'s usage line.
pub const DECISION_USAGE: &str = "usage: hosted decision <path> --revision <revision-id> \
     --root-set <pinned-v2.json> --pin-version <n> --pin-digest <hex> \
     [--policy <admission-policy.json>] [--scope <scope>]";

/// The evidence type a receipt is submitted under.
pub const RECEIPT_TYPE: &str = "statecraft-cli/receipt";
/// Where a project's submissions are recorded: runtime state, not committed.
pub const SUBMISSIONS: &str = ".statecraft/state/hosted/submissions.jsonl";

fn options(
    rest: &[String],
    valued: &[&str],
    flags: &[&str],
    usage: &str,
) -> Result<(PathBuf, Opts), String> {
    let args: Vec<String> = rest
        .iter()
        .filter(|a| a.as_str() != "--json")
        .cloned()
        .collect();
    let Some(first) = args.first().filter(|a| !a.starts_with("--")) else {
        return Err(format!("a project path is required\n{usage}"));
    };
    let o = parse_opts(&args[1..], valued, flags, usage)?;
    Ok((PathBuf::from(first), o))
}

/// A hosted refusal or failure, as an answer.
fn hosted(e: &HostedError) -> Answer<Value> {
    match e.exit_code() {
        4 => Answer::new(json!({}), Exit::Failed, e.to_string()).with_kind(ErrorKind::Io),
        _ => Answer::new(json!({}), Exit::Refused, e.to_string()).with_kind(ErrorKind::Refused),
    }
}

/// The project's declaration, with `--cell` replacing the origin only.
fn declaration(root: &Path, o: &Opts) -> Result<Declaration, Answer<Value>> {
    let mut d = Declaration::read(root).map_err(|e| hosted(&e))?;
    if let Some(cell) = o.get("cell") {
        d.cell = Origin::parse(cell).map_err(|e| hosted(&e))?;
    }
    Ok(d)
}

/// The held credential, renewed late if it has expired (section 3.3).
fn credential(
    home: &Path,
    http: &Http,
    d: &Declaration,
    clock: &dyn Clock,
) -> Result<Credential, HostedError> {
    let store = Store::new(home);
    let held = store
        .load(&d.cell)?
        .ok_or_else(|| HostedError::LoginRequired(d.cell.to_string()))?;
    if clock.now() < held.expires_at() {
        return Ok(held);
    }
    let endpoints = discover(http, &d.cell)?;
    let renewed = grant::refresh(http, &endpoints, &d.cell, &d.client_id, &held, clock)?;
    store.save(&d.cell, &renewed)?;
    Ok(renewed)
}

/// `hosted login <path> [--cell <origin>]`.
pub fn login(home: &Path, rest: &[String]) -> Outcome {
    let (root, o) = options(rest, &["cell"], &[], LOGIN_USAGE)?;
    let d = match declaration(&root, &o) {
        Ok(d) => d,
        Err(a) => return Ok(a),
    };
    let http = Http::new();
    let clock = SystemClock;
    let run = || -> Result<Credential, HostedError> {
        let endpoints = discover(&http, &d.cell)?;
        let start = grant::start(&http, &endpoints, &d.client_id, &d.scopes)?;
        // The person acts now; the answer comes when the grant ends.
        eprintln!(
            "To log in to {}, visit {} and enter the code {}",
            d.cell,
            start
                .verification_uri_complete
                .as_deref()
                .unwrap_or(&start.verification_uri),
            start.user_code
        );
        let c = grant::poll(&http, &endpoints, &d.cell, &d.client_id, &start, &clock)?;
        Store::new(home).save(&d.cell, &c)?;
        Ok(c)
    };
    Ok(match run() {
        Ok(c) => Answer::new(
            json!({
                "cell": d.cell.as_str(),
                "scope": c.scope,
                "expiresAt": c.expires_at(),
                "refreshNotBefore": c.refresh_not_before(),
            }),
            Exit::Ok,
            format!("logged in to {}\n", d.cell),
        ),
        Err(e) => hosted(&e),
    })
}

/// `hosted logout <path> [--cell <origin>]`: revoke, then delete.
pub fn logout(home: &Path, rest: &[String]) -> Outcome {
    let (root, o) = options(rest, &["cell"], &[], LOGOUT_USAGE)?;
    let d = match declaration(&root, &o) {
        Ok(d) => d,
        Err(a) => return Ok(a),
    };
    let store = Store::new(home);
    let held = match store.load(&d.cell) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Ok(Answer::new(
                json!({"cell": d.cell.as_str(), "held": false}),
                Exit::Ok,
                format!("no credential is held for {}\n", d.cell),
            ));
        }
        Err(e) => return Ok(hosted(&e)),
    };
    let revocation = grant::revoke(&Http::new(), &d.cell, &held);
    if let Err(e) = store.delete(&d.cell) {
        return Ok(hosted(&e));
    }
    let valid_until = held.expires_at().saturating_add(60);
    let (word, summary) = match revocation {
        Revocation::Confirmed => (
            "confirmed".to_string(),
            "the cell confirmed the revocation".to_string(),
        ),
        Revocation::NotConfirmed(status) => (
            "not-confirmed".to_string(),
            format!(
                "the cell answered {status} and did not confirm the revocation; the token stays valid until {valid_until}"
            ),
        ),
        Revocation::Unreachable => (
            "unreachable".to_string(),
            format!(
                "the cell could not be reached; the token stays valid until {valid_until} (its exp plus 60 seconds)"
            ),
        ),
    };
    Ok(Answer::new(
        json!({"cell": d.cell.as_str(), "held": true, "revocation": word, "deleted": true, "validUntil": valid_until}),
        Exit::Ok,
        format!("{summary}; the local credential is deleted\n"),
    ))
}

/// The receipt in a saved `accept --json` answer.
fn receipt_of(bytes: &[u8]) -> Result<statecraft_acceptance::receipt::Receipt, String> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("not JSON: {e}"))?;
    if v["verb"] != "accept" {
        return Err("it is not an `accept --json` answer".into());
    }
    let report = &v["report"];
    if report["acceptance"] != "accepted" {
        return Err(format!(
            "its acceptance is `{}`, not accepted: there is no receipt",
            report["acceptance"].as_str().unwrap_or("absent")
        ));
    }
    serde_json::from_value(report["receipt"].clone())
        .map_err(|e| format!("its receipt is malformed: {e}"))
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = statecraft_run::trusted_git::command(root)
        .ok()?
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `hosted submit <path> --receipt <accept-answer.json> [--plan]`.
pub fn submit(home: &Path, rest: &[String]) -> Outcome {
    let (root, o) = options(rest, &["receipt", "cell"], &["plan"], SUBMIT_USAGE)?;
    require(&o, &["receipt"], SUBMIT_USAGE)?;
    let d = match declaration(&root, &o) {
        Ok(d) => d,
        Err(a) => return Ok(a),
    };
    let bytes = match read_input(&o.path("receipt").expect("given"), "--receipt") {
        Ok(b) => b,
        Err(a) => return Ok(a),
    };
    let receipt = match receipt_of(&bytes) {
        Ok(r) => r,
        Err(why) => {
            return Ok(refused(
                ErrorKind::Refused,
                format!("no local receipt to submit: --receipt {why}"),
                Value::Null,
            ));
        }
    };
    // Section 3.5 rule 1: a revision this product accepted, in this project.
    let commit = receipt.candidate.clone();
    if git(&root, &["cat-file", "-e", &format!("{commit}^{{commit}}")]).is_none() {
        return Ok(refused(
            ErrorKind::Refused,
            format!(
                "the receipt's candidate {commit} is not a commit in {}",
                root.display()
            ),
            Value::Null,
        ));
    }
    let format =
        git(&root, &["rev-parse", "--show-object-format"]).unwrap_or_else(|| "sha1".into());
    let tree = git(&root, &["rev-parse", &format!("{commit}^{{tree}}")]);
    let receipt_bytes = serde_json::to_vec(&receipt).expect("a receipt serializes");
    let reference = Reference::over_file_bytes(RECEIPT_TYPE, "1", &receipt_bytes);
    let manifest = json!({
        "submission_version": 1,
        "producer": {"name": "statecraft-cli", "version": env!("CARGO_PKG_VERSION")},
        "artifacts": [reference],
    });
    let plan = json!({
        "cell": d.cell.as_str(),
        "repository": d.repository,
        "register": {"format": format, "commit": commit, "tree": tree, "base": receipt.base},
        "artifacts": [{"reference": RECEIPT_TYPE, "digest": reference.digest, "length": reference.bytes}],
    });
    if o.has("plan") {
        return Ok(Answer::new(
            json!({"plan": plan}),
            Exit::Ok,
            format!(
                "would register {commit} with {} and submit 1 artifact ({} bytes); nothing was sent\n",
                d.cell, reference.bytes
            ),
        ));
    }
    let http = Http::new();
    let clock = SystemClock;
    let run = || -> Result<(api::Registered, api::Submitted), HostedError> {
        let c = credential(home, &http, &d, &clock)?;
        let registered = api::register_revision(
            &http,
            &d.cell,
            &c,
            &d.repository,
            &api::Subject {
                format: &format,
                commit: &commit,
                tree: tree.as_deref(),
                base: Some(receipt.base.as_str()).filter(|b| !b.is_empty()),
            },
        )?;
        let submission = serde_json::to_vec(&json!({
            "repository": d.repository,
            "revision": registered.revision,
            "manifest": manifest,
        }))
        .expect("serializable");
        let submitted = api::submit(
            &http,
            &d.cell,
            &c,
            &d.repository,
            &registered.revision,
            &submission,
            std::slice::from_ref(&receipt_bytes),
        )?;
        Ok((registered, submitted))
    };
    let (registered, submitted) = match run() {
        Ok(v) => v,
        Err(e) => return Ok(hosted(&e)),
    };
    let record = json!({
        "cell": d.cell.as_str(),
        "repository": d.repository,
        "commit": commit,
        "revision": registered.revision,
        "registered": if registered.existed { "existing" } else { "new" },
        "idempotencyKey": submitted.idempotency_key,
        "responseDigest": submitted.response_digest,
    });
    if let Err(e) = append_record(&root, &record) {
        return Ok(Answer::new(
            record,
            Exit::Failed,
            format!("the submission was accepted but could not be recorded: {e}"),
        )
        .with_kind(ErrorKind::Io));
    }
    Ok(Answer::new(
        json!({"submission": record, "answer": submitted.answer}),
        Exit::Ok,
        format!(
            "submitted {commit} to {} as revision {}; admission is not reported here (`hosted decision`)\n",
            d.cell, registered.revision
        ),
    ))
}

fn append_record(root: &Path, record: &Value) -> std::io::Result<()> {
    use std::io::Write;
    let path = root.join(SUBMISSIONS);
    std::fs::create_dir_all(path.parent().expect("a parent"))?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{record}")
}

/// `hosted decision <path> --revision <id> --root-set <pinned-v2.json>
/// --pin-version <n> --pin-digest <hex> [--policy <file>] [--scope <scope>]`.
pub fn decision(home: &Path, rest: &[String]) -> Outcome {
    let (root, o) = options(
        rest,
        &[
            "revision",
            "root-set",
            "pin-version",
            "pin-digest",
            "policy",
            "scope",
            "cell",
        ],
        &[],
        DECISION_USAGE,
    )?;
    require(
        &o,
        &["revision", "root-set", "pin-version", "pin-digest"],
        DECISION_USAGE,
    )?;
    let d = match declaration(&root, &o) {
        Ok(d) => d,
        Err(a) => return Ok(a),
    };
    // The operator's pinned root set, admitted under its exact pin (spec 035).
    let pin = match (|| {
        Ok::<_, Answer<Value>>(RootSetPin {
            root_set_version: number(&o, "pin-version", PORTABLE_MAX as u64)?,
            digest: strict_hash(o.get("pin-digest").expect("given"), "--pin-digest")
                .map_err(|e| envelope_refusal("--pin-digest", &e))?,
        })
    })() {
        Ok(p) => p,
        Err(a) => return Ok(a),
    };
    let set = match read_input(&o.path("root-set").expect("given"), "--root-set").and_then(|b| {
        let set = RootSetV2::parse(&b)
            .and_then(|s| s.verify())
            .map_err(|e| envelope_refusal("--root-set", &e))?;
        admit_pinned(None, &pin, &set).map_err(|e| envelope_refusal("--root-set", &e))?;
        Ok(set)
    }) {
        Ok(s) => s,
        Err(a) => return Ok(a),
    };
    let policy: Option<AdmissionPolicy> = match o.path("policy") {
        None => None,
        Some(p) => match read_input(&p, "--policy").and_then(|b| {
            serde_json::from_slice(&b).map_err(|e| {
                refused(
                    ErrorKind::Schema,
                    format!("--policy is not an admission policy: {e}"),
                    Value::Null,
                )
            })
        }) {
            Ok(p) => Some(p),
            Err(a) => return Ok(a),
        },
    };
    let revision = o.get("revision").expect("given").to_string();
    let http = Http::new();
    let fetched = credential(home, &http, &d, &SystemClock)
        .and_then(|c| api::decision(&http, &d.cell, &c, &d.repository, &revision));
    let bytes = match fetched {
        Ok(Retrieved::NotRegistered) => {
            return Ok(refused(
                ErrorKind::NotFound,
                format!("revision {revision} is not registered at {}", d.cell),
                Value::Null,
            ));
        }
        Ok(Retrieved::Bundle(b)) => b,
        Err(e) => return Ok(hosted(&e)),
    };
    let bundle = match DecisionBundle::parse(&bytes) {
        Ok(b) => b,
        Err(e) => {
            return Ok(Answer::new(
                json!({"divergence": e.to_string()}),
                Exit::Finding,
                format!("the bundle does not verify: {e}\n"),
            ));
        }
    };
    let scope = o.get("scope").unwrap_or("platform");
    let v = verify(&bundle, &set, scope, policy.as_ref());
    let admission = match &v.admission {
        AdmissionCheck::None => json!({"state": "none"}),
        AdmissionCheck::Reproduced { decision } => {
            json!({"state": "reproduced", "decision": decision})
        }
        AdmissionCheck::Diverged {
            recorded,
            reproduced,
        } => {
            json!({"state": "diverged", "recorded": recorded, "reproduced": reproduced})
        }
        AdmissionCheck::NotReproduced(why) => json!({"state": "not-reproduced", "reason": why}),
    };
    let report = json!({
        "cell": d.cell.as_str(),
        "revision": revision,
        "rootSet": {
            "pinned": v.pinned_root_set.to_hex(),
            "bundle": v.bundle_root_set.to_hex(),
            "matches": v.root_set_matches(),
        },
        "entries": v.entries,
        "divergence": v.divergence,
        "tombstoned": v.tombstoned,
        "decisions": v.decisions.iter().map(|x| json!({
            "id": x.id.to_hex(),
            "approves": x.approves,
            "reviewer": x.reviewer.id,
        })).collect::<Vec<_>>(),
        "admission": admission,
    });
    let mut summary = String::new();
    if !v.root_set_matches() {
        summary.push_str(&format!(
            "the bundle was judged under root set {}, not the pinned {}\n",
            v.bundle_root_set.to_hex(),
            v.pinned_root_set.to_hex()
        ));
    }
    if let Some(why) = &v.divergence {
        summary.push_str(&format!("first divergence: {why}\n"));
    }
    match &v.admission {
        AdmissionCheck::None => summary.push_str("admission: none (no decision; not a pass)\n"),
        AdmissionCheck::Reproduced { decision } => summary.push_str(&format!("admission: {decision}, reproduced under the pinned policy\n")),
        AdmissionCheck::Diverged { recorded, reproduced } => summary.push_str(&format!(
            "admission diverges: the cell recorded {recorded}, the pinned policy gives {reproduced}\n"
        )),
        AdmissionCheck::NotReproduced(why) => summary.push_str(&format!("admission not reproduced: {why}\n")),
    }
    for x in &v.decisions {
        summary.push_str(&format!(
            "decision {} by {}: {}\n",
            x.id.to_hex(),
            x.reviewer.id,
            if x.approves {
                "approve"
            } else {
                "request changes"
            }
        ));
    }
    let exit = if v.passes() { Exit::Ok } else { Exit::Finding };
    Ok(Answer::new(report, exit, summary))
}
