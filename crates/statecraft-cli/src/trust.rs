//! The offline issuer verbs: `trust root-generate`, `trust issuer-sign` and
//! `trust issuer-verify` (spec 036, amending 006).
//!
//! The formats and the verifier are the envelope crate's (spec 035), which is
//! pure. What lives here is what that crate refuses to own: explicit files,
//! exclusive creation, owner-only permissions, the operating system's random
//! source and the owner's confirmation. Nothing here reads the product home,
//! a registered project, the network, a cell or a store, and nothing searches
//! for a key: every path is an argument.
//!
//! The seed is held only in zeroizing buffers and the signer. It is written to
//! the one `--out` path `root-generate` was given, read from the one
//! `--seed-file` path `issuer-sign` was given, and appears in no answer,
//! message, log or other file.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use statecraft_envelope::Error as EnvelopeError;
use statecraft_envelope::hash::{Hash, KeyId};
use statecraft_envelope::hlc::Hlc;
use statecraft_envelope::issuer::{
    DetachedSignature, IssuerEligibility, IssuerIdentity, IssuerPayload, IssuerRecordKind,
    OwnerRootPublic, RootSetPin, RootSetV2, admit_pinned, lower_hex, strict_hash, verify_detached,
};
use statecraft_envelope::sign::Signer;
use statecraft_envelope::value::PORTABLE_MAX;
use zeroize::Zeroizing;

use crate::exit::Exit;
use crate::render::{Answer, ErrorKind};

/// The largest input file read, in bytes. Every input here is a small record.
const MAX_INPUT: u64 = 1 << 20;

/// What a trust verb returns: an answer to render, or a usage message.
pub type Outcome = Result<Answer<Value>, String>;

/// `trust root-generate`'s usage line.
pub const ROOT_GENERATE_USAGE: &str = "usage: trust root-generate --out <new-offline-seed-file> --public-out <new-public-json> --confirm-new-root";
/// `trust issuer-sign`'s usage line.
pub const ISSUER_SIGN_USAGE: &str = "usage: trust issuer-sign --kind <enrolment|rotation|revocation> --seed-file <offline-seed-file> \
     --root-key-id <pinned-id> --request <canonical.cbor> --identity <identity.created.cbor> \
     --out <new-signature.json> [--history-root-set <pinned-v2.json>] [--confirm <payload-digest>]";
/// `trust issuer-verify`'s usage line.
pub const ISSUER_VERIFY_USAGE: &str = "usage: trust issuer-verify --kind <kind> --root-public <pinned-public.json> \
     --request <canonical.cbor> --identity <identity.created.cbor> --signature <signature.json> \
     [--history-root-set <pinned-v2.json> --pin-version <n> --pin-digest <hex> \
     --at-physical <n> --at-logical <n> --scope <scope>]";

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

struct Opts {
    values: BTreeMap<String, String>,
    flags: BTreeSet<String>,
}

impl Opts {
    fn get(&self, k: &str) -> Option<&str> {
        self.values.get(k).map(String::as_str)
    }
    fn path(&self, k: &str) -> Option<PathBuf> {
        self.get(k).map(PathBuf::from)
    }
}

/// `--name value` or `--name=value` for valued options, bare `--name` for
/// flags. A repeated, unknown or positional argument, or a valued option with
/// no value, is a usage error.
fn parse_opts(
    rest: &[String],
    valued: &[&str],
    flags: &[&str],
    usage: &str,
) -> Result<Opts, String> {
    let mut o = Opts {
        values: BTreeMap::new(),
        flags: BTreeSet::new(),
    };
    let mut i = 0;
    while i < rest.len() {
        let arg = &rest[i];
        let Some(body) = arg.strip_prefix("--") else {
            return Err(format!("unexpected argument `{arg}`\n{usage}"));
        };
        let (name, inline) = match body.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (body, None),
        };
        if flags.contains(&name) {
            if inline.is_some() {
                return Err(format!("--{name} takes no value\n{usage}"));
            }
            if !o.flags.insert(name.to_string()) {
                return Err(format!("--{name} is repeated\n{usage}"));
            }
        } else if valued.contains(&name) {
            let value = match inline {
                Some(v) => v,
                None => {
                    i += 1;
                    match rest.get(i) {
                        Some(v) if !v.starts_with("--") => v.clone(),
                        _ => return Err(format!("--{name} needs a value\n{usage}")),
                    }
                }
            };
            if value.is_empty() {
                return Err(format!("--{name} needs a value\n{usage}"));
            }
            if o.values.insert(name.to_string(), value).is_some() {
                return Err(format!("--{name} is repeated\n{usage}"));
            }
        } else {
            return Err(format!("unknown option `--{name}`\n{usage}"));
        }
        i += 1;
    }
    Ok(o)
}

fn require(o: &Opts, names: &[&str], usage: &str) -> Result<(), String> {
    for n in names {
        if o.get(n).is_none() {
            return Err(format!("--{n} is required\n{usage}"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Answers
// ---------------------------------------------------------------------------

fn refused(kind: ErrorKind, summary: impl Into<String>, details: Value) -> Answer<Value> {
    Answer::new(details, Exit::Refused, summary).with_kind(kind)
}

fn failed(summary: impl Into<String>, details: Value) -> Answer<Value> {
    Answer::new(details, Exit::Failed, summary).with_kind(ErrorKind::Io)
}

fn envelope_refusal(what: &str, e: &EnvelopeError) -> Answer<Value> {
    let kind = match e {
        EnvelopeError::Decode(_) => ErrorKind::Schema,
        _ => ErrorKind::Validation,
    };
    refused(kind, format!("{what}: {e}"), Value::Null)
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

/// An output path must not exist in any form, a dangling link included, and
/// its parent must be an existing directory. Nothing is created to make it so.
fn check_new_output(path: &Path, what: &str) -> Result<(), Answer<Value>> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(refused(
                ErrorKind::Refused,
                format!(
                    "{what} {} already exists; custody files are never overwritten",
                    path.display()
                ),
                Value::Null,
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(refused(
                ErrorKind::Io,
                format!("{what} {}: {e}", path.display()),
                Value::Null,
            ));
        }
    }
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    match std::fs::metadata(parent) {
        Ok(m) if m.is_dir() => Ok(()),
        _ => Err(refused(
            ErrorKind::Refused,
            format!("{what}: the directory {} does not exist", parent.display()),
            Value::Null,
        )),
    }
}

/// Create a new file exclusively (never following or replacing anything)
/// with the given mode, write it and flush it to disk.
#[cfg(unix)]
fn create_new(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Read a regular, non-symlinked input file of bounded size.
fn read_input(path: &Path, what: &str) -> Result<Vec<u8>, Answer<Value>> {
    let refuse = |msg: String| refused(ErrorKind::Refused, msg, Value::Null);
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| refuse(format!("{what} {}: {e}", path.display())))?;
    if meta.file_type().is_symlink() {
        return Err(refuse(format!(
            "{what} {} is a symbolic link",
            path.display()
        )));
    }
    if !meta.is_file() {
        return Err(refuse(format!(
            "{what} {} is not a regular file",
            path.display()
        )));
    }
    if meta.len() > MAX_INPUT {
        return Err(refuse(format!(
            "{what} {} is larger than {MAX_INPUT} bytes",
            path.display()
        )));
    }
    std::fs::read(path).map_err(|e| refuse(format!("{what} {}: {e}", path.display())))
}

/// Load an offline seed file: a regular file, not a link, readable by its
/// owner only, holding exactly 64 lowercase hex characters and one LF. No
/// message names any byte of its content.
#[cfg(unix)]
fn load_seed(path: &Path) -> Result<Signer, Answer<Value>> {
    use std::os::unix::fs::MetadataExt;
    let refuse = |msg: &str| {
        refused(
            ErrorKind::Refused,
            format!("seed file {}: {msg}", path.display()),
            Value::Null,
        )
    };
    let meta = std::fs::symlink_metadata(path).map_err(|e| refuse(&e.kind().to_string()))?;
    if meta.file_type().is_symlink() {
        return Err(refuse("is a symbolic link"));
    }
    if !meta.is_file() {
        return Err(refuse("is not a regular file"));
    }
    if meta.mode() & 0o077 != 0 {
        return Err(refuse(
            "is readable or writable by others; owner-only permissions are required",
        ));
    }
    if meta.len() != 65 {
        return Err(refuse(
            "is not a canonical seed file (64 lowercase hex characters and LF)",
        ));
    }
    let mut f = std::fs::File::open(path).map_err(|e| refuse(&e.kind().to_string()))?;
    let opened = f.metadata().map_err(|e| refuse(&e.kind().to_string()))?;
    if opened.dev() != meta.dev() || opened.ino() != meta.ino() {
        return Err(refuse("changed while it was being opened"));
    }
    let mut buf = Zeroizing::new(Vec::with_capacity(66));
    Read::by_ref(&mut f)
        .take(66)
        .read_to_end(&mut buf)
        .map_err(|e| refuse(&e.kind().to_string()))?;
    if buf.len() != 65 || buf[64] != b'\n' {
        return Err(refuse(
            "is not a canonical seed file (64 lowercase hex characters and LF)",
        ));
    }
    let text = std::str::from_utf8(&buf[..64])
        .map_err(|_| refuse("is not a canonical seed file (64 lowercase hex characters and LF)"))?;
    let bytes = Zeroizing::new(lower_hex(text, 32, "seed").map_err(|_| {
        refuse("is not a canonical seed file (64 lowercase hex characters and LF)")
    })?);
    let mut seed = Zeroizing::new([0u8; 32]);
    seed.copy_from_slice(&bytes);
    Ok(Signer::from_seed(&seed))
}

#[cfg(not(unix))]
fn unsupported() -> Answer<Value> {
    refused(
        ErrorKind::Refused,
        "the offline trust verbs need owner-only file permissions, which this platform does not provide",
        Value::Null,
    )
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

fn hlc_json(h: Hlc) -> Value {
    json!({ "physical": h.physical, "logical": h.logical })
}

/// The decoded public fields, in this product's key convention.
fn fields(p: &IssuerPayload) -> Value {
    match p {
        IssuerPayload::Enrolment(e) => {
            let mut m = json!({
                "issuerId": e.issuer_id.to_hex(),
                "publicKey": hex::encode(e.public_key.0),
                "keyId": e.public_key.id().0.to_hex(),
                "scope": e.scope,
            });
            if let Some(nb) = e.not_before {
                m["notBefore"] = hlc_json(nb);
            }
            if let Some(na) = e.not_after {
                m["notAfter"] = hlc_json(na);
            }
            m
        }
        IssuerPayload::Rotation(r) => json!({
            "identity": r.identity.to_hex(),
            "from": r.from.0.to_hex(),
            "to": hex::encode(r.to.0),
            "toKeyId": r.to.id().0.to_hex(),
            "effective": hlc_json(r.effective),
        }),
        IssuerPayload::Revocation(r) => json!({
            "identity": r.identity.to_hex(),
            "key": r.key.0.to_hex(),
            "since": hlc_json(r.since),
            "reason": r.reason,
        }),
    }
}

fn describe(kind: IssuerRecordKind, root: &KeyId, digest: &Hash, p: &IssuerPayload) -> String {
    let mut s = format!(
        "kind: {}\nroot key id: {}\npayload digest: {}\n",
        kind.word(),
        root.0.to_hex(),
        digest.to_hex()
    );
    let f = fields(p);
    if let Some(m) = f.as_object() {
        for (k, v) in m {
            s.push_str(&format!("{k}: {v}\n"));
        }
    }
    s
}

// ---------------------------------------------------------------------------
// trust root-generate
// ---------------------------------------------------------------------------

/// Generate a new offline owner root: 32 bytes from the operating system's
/// initialized random source, written once to `--out` with owner-only
/// permissions, and the public record to `--public-out`. Refused without
/// `--confirm-new-root`; nothing is written then.
pub fn root_generate(rest: &[String]) -> Outcome {
    let o = parse_opts(
        rest,
        &["out", "public-out"],
        &["confirm-new-root"],
        ROOT_GENERATE_USAGE,
    )?;
    require(&o, &["out", "public-out"], ROOT_GENERATE_USAGE)?;
    let out = o.path("out").expect("required");
    let public_out = o.path("public-out").expect("required");
    if out == public_out {
        return Err(format!(
            "--out and --public-out must differ\n{ROOT_GENERATE_USAGE}"
        ));
    }
    if !o.flags.contains("confirm-new-root") {
        return Ok(refused(
            ErrorKind::Refused,
            "root-generate creates a new offline owner root and writes its seed; \
             it runs only with the explicit --confirm-new-root option, and nothing was written",
            Value::Null,
        ));
    }
    #[cfg(not(unix))]
    {
        let _ = (out, public_out);
        Ok(unsupported())
    }
    #[cfg(unix)]
    {
        Ok(generate(&out, &public_out))
    }
}

#[cfg(unix)]
fn generate(out: &Path, public_out: &Path) -> Answer<Value> {
    if let Err(a) = check_new_output(out, "--out") {
        return a;
    }
    if let Err(a) = check_new_output(public_out, "--public-out") {
        return a;
    }
    let mut seed = Zeroizing::new([0u8; 32]);
    if let Err(e) = getrandom::getrandom(&mut seed[..]) {
        return failed(
            format!("the operating system's random source is unavailable: {e}"),
            Value::Null,
        );
    }
    let signer = Signer::from_seed(&seed);
    let mut line = Zeroizing::new(hex::encode(&seed[..]).into_bytes());
    line.push(b'\n');
    if let Err(e) = create_new(out, &line, 0o600) {
        return failed(
            format!("--out {}: {e}; nothing was written", out.display()),
            Value::Null,
        );
    }
    drop(line);
    drop(seed);
    let public = OwnerRootPublic::of(&signer);
    drop(signer);
    let report = json!({
        "keyId": public.key_id().0.to_hex(),
        "publicKey": hex::encode(public.public_key.0),
        "scope": "platform",
        "seedFile": out.display().to_string(),
        "publicFile": public_out.display().to_string(),
    });
    if let Err(e) = create_new(public_out, &public.to_bytes(), 0o644) {
        return failed(
            format!(
                "--public-out {}: {e}; the seed file {} WAS written and is the only copy",
                public_out.display(),
                out.display()
            ),
            report,
        );
    }
    let summary = format!(
        "new offline owner root {}\nseed written to {} (owner-only); public record written to {}\n",
        public.key_id().0.to_hex(),
        out.display(),
        public_out.display()
    );
    Answer::new(report, Exit::Ok, summary)
}

// ---------------------------------------------------------------------------
// trust issuer-sign
// ---------------------------------------------------------------------------

/// Sign one enrollment, rotation or revocation with an offline owner seed.
///
/// The request and identity are decoded strictly and bound to each other; a
/// rotation or revocation is checked against the pinned history first. The
/// decoded public fields, the payload digest and the root id are reported,
/// and the signature is made only when `--confirm` names that exact digest.
pub fn issuer_sign(rest: &[String]) -> Outcome {
    let o = parse_opts(
        rest,
        &[
            "kind",
            "seed-file",
            "root-key-id",
            "request",
            "identity",
            "out",
            "history-root-set",
            "confirm",
        ],
        &[],
        ISSUER_SIGN_USAGE,
    )?;
    require(
        &o,
        &[
            "kind",
            "seed-file",
            "root-key-id",
            "request",
            "identity",
            "out",
        ],
        ISSUER_SIGN_USAGE,
    )?;
    let kind = IssuerRecordKind::parse(o.get("kind").expect("required")).map_err(|_| {
        format!("--kind must be enrolment, rotation or revocation\n{ISSUER_SIGN_USAGE}")
    })?;
    match (kind, o.get("history-root-set")) {
        (IssuerRecordKind::Enrolment, Some(_)) => {
            return Err(format!(
                "--history-root-set is for rotation and revocation only\n{ISSUER_SIGN_USAGE}"
            ));
        }
        (IssuerRecordKind::Rotation | IssuerRecordKind::Revocation, None) => {
            return Err(format!(
                "--history-root-set is required for {}\n{ISSUER_SIGN_USAGE}",
                kind.word()
            ));
        }
        _ => {}
    }
    #[cfg(not(unix))]
    {
        let _ = o;
        Ok(unsupported())
    }
    #[cfg(unix)]
    {
        Ok(sign(kind, &o).unwrap_or_else(|a| a))
    }
}

#[cfg(unix)]
fn sign(kind: IssuerRecordKind, o: &Opts) -> Result<Answer<Value>, Answer<Value>> {
    let root_key_id = KeyId(
        strict_hash(o.get("root-key-id").expect("required"), "--root-key-id")
            .map_err(|e| envelope_refusal("--root-key-id", &e))?,
    );
    let out = o.path("out").expect("required");
    check_new_output(&out, "--out")?;
    let request = read_input(&o.path("request").expect("required"), "--request")?;
    let identity_bytes = read_input(&o.path("identity").expect("required"), "--identity")?;
    let payload =
        IssuerPayload::decode(kind, &request).map_err(|e| envelope_refusal("--request", &e))?;
    let identity =
        IssuerIdentity::decode(&identity_bytes).map_err(|e| envelope_refusal("--identity", &e))?;
    payload
        .check_identity(&identity)
        .map_err(|e| envelope_refusal("--request and --identity", &e))?;
    if let IssuerPayload::Enrolment(e) = &payload {
        e.authorization_time(Some(&identity))
            .map_err(|e| envelope_refusal("--request", &e))?;
    }
    let mut history = Value::Null;
    if let Some(path) = o.path("history-root-set") {
        let bytes = read_input(&path, "--history-root-set")?;
        let set = RootSetV2::parse(&bytes)
            .and_then(|s| s.verify())
            .map_err(|e| envelope_refusal("--history-root-set", &e))?;
        let step = match &payload {
            IssuerPayload::Rotation(r) => set.check_rotation(&root_key_id, r),
            IssuerPayload::Revocation(r) => set.check_revocation(&root_key_id, r),
            IssuerPayload::Enrolment(_) => Ok(()),
        };
        step.map_err(|e| envelope_refusal("--request against the pinned history", &e))?;
        history = json!({
            "rootSetVersion": set.root_set_version,
            "rootSetDigest": set.digest.to_hex(),
        });
    }
    let digest = Hash::of(&request);
    let mut report = json!({
        "kind": kind.word(),
        "rootKeyId": root_key_id.0.to_hex(),
        "payloadDigest": digest.to_hex(),
        "identity": identity.id.to_hex(),
        "fields": fields(&payload),
    });
    if !history.is_null() {
        report["history"] = history;
    }
    let inspected = describe(kind, &root_key_id, &digest, &payload);
    match o.get("confirm") {
        None => {
            return Err(refused(
                ErrorKind::Refused,
                format!(
                    "{inspected}not signed: inspect the fields above, then rerun with --confirm {}",
                    digest.to_hex()
                ),
                report,
            ));
        }
        Some(c) if c != digest.to_hex() => {
            return Err(refused(
                ErrorKind::Refused,
                format!("{inspected}not signed: --confirm does not name this payload digest"),
                report,
            ));
        }
        Some(_) => {}
    }
    let signer = load_seed(&o.path("seed-file").expect("required"))?;
    if signer.public().id() != root_key_id {
        return Err(refused(
            ErrorKind::Refused,
            "not signed: the seed file is not the root --root-key-id names",
            report,
        ));
    }
    let signature = DetachedSignature::sign(&signer, kind, &request);
    let public = signer.public();
    drop(signer);
    if let Err(e) = signature.verify(&public, kind, &request) {
        return Err(failed(
            format!("the signature made does not verify: {e}"),
            report,
        ));
    }
    create_new(&out, &signature.to_bytes(), 0o644).map_err(|e| {
        failed(
            format!("--out {}: {e}; nothing was written", out.display()),
            report.clone(),
        )
    })?;
    report["signatureFile"] = json!(out.display().to_string());
    Ok(Answer::new(
        report,
        Exit::Ok,
        format!(
            "{inspected}signed: detached signature written to {}\n",
            out.display()
        ),
    ))
}

// ---------------------------------------------------------------------------
// trust issuer-verify
// ---------------------------------------------------------------------------

const FULL: [&str; 6] = [
    "history-root-set",
    "pin-version",
    "pin-digest",
    "at-physical",
    "at-logical",
    "scope",
];

/// Verify a detached owner authorization against an independently pinned
/// owner public record. With the full option group, also verify the pinned
/// history under the operator's exact version and digest, and answer the
/// eligibility of the key the request concerns at an explicit time and scope.
pub fn issuer_verify(rest: &[String]) -> Outcome {
    let mut valued = vec!["kind", "root-public", "request", "identity", "signature"];
    valued.extend(FULL);
    let o = parse_opts(rest, &valued, &[], ISSUER_VERIFY_USAGE)?;
    require(
        &o,
        &["kind", "root-public", "request", "identity", "signature"],
        ISSUER_VERIFY_USAGE,
    )?;
    let given = FULL.iter().filter(|k| o.get(k).is_some()).count();
    if given != 0 && given != FULL.len() {
        return Err(format!(
            "the eligibility options are given together or not at all: --{}\n{ISSUER_VERIFY_USAGE}",
            FULL.join(", --")
        ));
    }
    let kind = IssuerRecordKind::parse(o.get("kind").expect("required")).map_err(|_| {
        format!("--kind must be enrolment, rotation or revocation\n{ISSUER_VERIFY_USAGE}")
    })?;
    Ok(verify(kind, &o, given != 0).unwrap_or_else(|a| a))
}

fn number(o: &Opts, k: &str, max: u64) -> Result<u64, Answer<Value>> {
    let s = o.get(k).expect("given");
    let ok =
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'));
    match s.parse::<u64>() {
        Ok(n) if ok && n <= max => Ok(n),
        _ => Err(refused(
            ErrorKind::Validation,
            format!("--{k} must be an integer in 0..={max}"),
            Value::Null,
        )),
    }
}

fn verify(kind: IssuerRecordKind, o: &Opts, full: bool) -> Result<Answer<Value>, Answer<Value>> {
    let root = OwnerRootPublic::parse(&read_input(
        &o.path("root-public").expect("required"),
        "--root-public",
    )?)
    .map_err(|e| envelope_refusal("--root-public", &e))?;
    let request = read_input(&o.path("request").expect("required"), "--request")?;
    let identity = read_input(&o.path("identity").expect("required"), "--identity")?;
    let signature = DetachedSignature::parse(&read_input(
        &o.path("signature").expect("required"),
        "--signature",
    )?)
    .map_err(|e| envelope_refusal("--signature", &e))?;
    let digest = Hash::of(&request);
    let mut report = json!({
        "kind": kind.word(),
        "rootKeyId": root.key_id().0.to_hex(),
        "payloadDigest": digest.to_hex(),
    });
    let payload = match verify_detached(&root, kind, &request, &identity, &signature) {
        Ok(p) => p,
        Err(EnvelopeError::Crypto(why)) => {
            report["verdict"] = json!("not-verified");
            report["reason"] = json!(why);
            return Ok(Answer::new(
                report,
                Exit::Finding,
                format!("not verified: {why}\n"),
            ));
        }
        Err(e) => return Err(envelope_refusal("the authorization", &e)),
    };
    report["verdict"] = json!("verified");
    report["identity"] = json!(payload.identity().to_hex());
    report["fields"] = fields(&payload);
    let mut summary = format!(
        "verified: {} authorized by root {}\n",
        kind.word(),
        root.key_id().0.to_hex()
    );
    let mut exit = Exit::Ok;
    if full {
        let pin = RootSetPin {
            root_set_version: number(o, "pin-version", PORTABLE_MAX as u64)?,
            digest: strict_hash(o.get("pin-digest").expect("given"), "--pin-digest")
                .map_err(|e| envelope_refusal("--pin-digest", &e))?,
        };
        let at = Hlc::new(
            number(o, "at-physical", PORTABLE_MAX as u64)?,
            number(o, "at-logical", u32::MAX as u64)? as u32,
        );
        let scope = o.get("scope").expect("given");
        let bytes = read_input(
            &o.path("history-root-set").expect("given"),
            "--history-root-set",
        )?;
        let set = RootSetV2::parse(&bytes)
            .and_then(|s| s.verify())
            .map_err(|e| envelope_refusal("--history-root-set", &e))?;
        admit_pinned(None, &pin, &set).map_err(|e| envelope_refusal("--history-root-set", &e))?;
        if !set.holds_root(&root) {
            return Err(refused(
                ErrorKind::Refused,
                "--root-public is not an owner root of the pinned history",
                report,
            ));
        }
        let el = set.eligibility(&payload.subject_key(), at, scope);
        let reason = match &el {
            IssuerEligibility::Refused(why) => Value::from(*why),
            IssuerEligibility::Unknown => Value::from("not-enrolled"),
            IssuerEligibility::Eligible { .. } => Value::Null,
        };
        let mut e = json!({
            "issuerTrust": el.word(),
            "key": payload.subject_key().0.to_hex(),
            "at": hlc_json(at),
            "scope": scope,
            "rootSetVersion": set.root_set_version,
            "rootSetDigest": set.digest.to_hex(),
        });
        if !reason.is_null() {
            e["reason"] = reason.clone();
        }
        report["eligibility"] = e;
        summary.push_str(&format!(
            "issuer trust at {}.{} in scope {scope}: {}{}\n",
            at.physical,
            at.logical,
            el.word(),
            reason
                .as_str()
                .map(|r| format!(" ({r})"))
                .unwrap_or_default()
        ));
        if !matches!(el, IssuerEligibility::Eligible { .. }) {
            exit = Exit::Finding;
        }
    }
    Ok(Answer::new(report, exit, summary))
}
