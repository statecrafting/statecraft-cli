//! The supervisor's side of `wire-witness.sidecar/1`, spec 013 sections 3.2
//! to 3.5.
//!
//! The supervisor writes exactly one [`Start`] line and then reads
//! newline-delimited JSON through a [`Reader`]. The reader is bounded before a
//! byte is parsed (section 3.4): a line longer than [`MAX_LINE_BYTES`], more
//! than [`MAX_MESSAGES`] messages, or a string longer than [`MAX_TEXT`] is a
//! [`Finding`], never a value. Every message must repeat the supplied
//! [`AttemptBinding`] byte for byte; one that does not binds nothing.
//!
//! Nothing here establishes or repairs a binding from time, a process id, a
//! child header, connection order, or content. The dedicated stdio channel of
//! the launched effect and the tuple it carries are the binding.

use crate::witness::ledger::hex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// The one interface this module speaks.
pub const PROTOCOL: &str = "wire-witness.sidecar/1";
/// The bound on one sidecar line, newline included.
pub const MAX_LINE_BYTES: usize = 65_536;
/// The bound on the number of messages one sidecar may send.
pub const MAX_MESSAGES: u64 = 100_000;
/// The bound on any one string the sidecar sends.
pub const MAX_TEXT: usize = 4096;
/// The bound on the child-visible CA certificate.
pub const MAX_CERTIFICATE_BYTES: u64 = 64 * 1024;
/// The bound on the capture manifest read for its integrity check.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;

/// Section 3.2's binding: minted by this product before spawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptBinding {
    /// The run.
    pub run_id: String,
    /// The attempt within it.
    pub attempt: u32,
    /// The sidecar effect's own identity, fresh per attempt.
    pub effect_id: String,
}

impl AttemptBinding {
    /// A binding, or `None` when a string is empty, too long, or carries a
    /// line break, which the producer refuses before it creates a CA.
    #[must_use]
    pub fn new(run_id: &str, attempt: u32, effect_id: &str) -> Option<Self> {
        let valid = |s: &str| !s.is_empty() && s.len() <= MAX_TEXT && !s.contains(['\r', '\n']);
        (valid(run_id) && valid(effect_id)).then(|| Self {
            run_id: run_id.to_string(),
            attempt,
            effect_id: effect_id.to_string(),
        })
    }

    fn json(&self) -> Value {
        json!({ "run_id": self.run_id, "attempt": self.attempt, "effect_id": self.effect_id })
    }
}

/// One allowlisted authority in the `start` message.
///
/// The provider family is the caller's word: this crate names no provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authority {
    /// `host:port`.
    pub authority: String,
    /// The producer's provider-family word.
    pub provider: String,
    /// Application protocols, for example `http/1.1`.
    pub protocols: Vec<String>,
}

/// The bounded resource limits of the `start` message. Every value is
/// positive; the producer refuses a zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct Limits {
    pub header_bytes: u64,
    pub header_count: u64,
    pub frame_bytes: u64,
    pub message_bytes: u64,
    pub decompressed_bytes: u64,
    pub event_bytes: u64,
    pub concurrent_streams: u64,
    pub connection_count: u64,
    pub queue_bytes: u64,
    pub idle_seconds: u64,
    pub total_seconds: u64,
}

impl Limits {
    /// The producer's wire spelling, which is not this product's JSON
    /// convention and so is written by hand.
    fn json(&self) -> Value {
        json!({
            "header_bytes": self.header_bytes,
            "header_count": self.header_count,
            "frame_bytes": self.frame_bytes,
            "message_bytes": self.message_bytes,
            "decompressed_bytes": self.decompressed_bytes,
            "event_bytes": self.event_bytes,
            "concurrent_streams": self.concurrent_streams,
            "connection_count": self.connection_count,
            "queue_bytes": self.queue_bytes,
            "idle_seconds": self.idle_seconds,
            "total_seconds": self.total_seconds,
        })
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            header_bytes: 64 * 1024,
            header_count: 256,
            frame_bytes: 1024 * 1024,
            message_bytes: 16 * 1024 * 1024,
            decompressed_bytes: 64 * 1024 * 1024,
            event_bytes: 1024 * 1024,
            concurrent_streams: 64,
            connection_count: 64,
            queue_bytes: 16 * 1024 * 1024,
            idle_seconds: 600,
            total_seconds: 4 * 60 * 60,
        }
    }
}

/// The one `start` message (section 3.2 of the producer's spec 005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    /// The binding every reply must repeat.
    pub binding: AttemptBinding,
    /// Authorities whose traffic is captured.
    pub capture_authorities: Vec<Authority>,
    /// Authorities tunnelled without capture.
    pub authentication_tunnels: Vec<Authority>,
    /// `metadata-only`, `content` or `disabled`.
    pub requested_retention: String,
    /// The bounded limits.
    pub limits: Limits,
    /// The attempt's sidecar output directory; must not yet exist.
    pub output_directory: PathBuf,
    /// The child-visible certificate path, directly in `output_directory`.
    pub certificate_path: PathBuf,
    /// Recorded, never interpreted as permission.
    pub host_policy_id: String,
    /// The single reserved per-attempt loopback endpoint (section 3.3).
    pub listen_endpoint: SocketAddr,
}

impl Start {
    /// The line written to the sidecar's stdin, newline included.
    #[must_use]
    pub fn line(&self) -> Vec<u8> {
        let authorities = |list: &[Authority]| -> Value {
            list.iter()
                .map(|a| {
                    json!({ "authority": a.authority, "provider": a.provider, "protocols": a.protocols })
                })
                .collect()
        };
        let message = json!({
            "protocol": PROTOCOL,
            "type": "start",
            "binding": self.binding.json(),
            "capture_authorities": authorities(&self.capture_authorities),
            "authentication_tunnels": authorities(&self.authentication_tunnels),
            "requested_retention": self.requested_retention,
            "limits": self.limits.json(),
            "output_directory": self.output_directory.display().to_string(),
            "certificate_path": self.certificate_path.display().to_string(),
            "host_policy_id": self.host_policy_id,
            "listen_endpoint": self.listen_endpoint.to_string(),
        });
        let mut line = serde_json::to_vec(&message).unwrap_or_default();
        line.push(b'\n');
        line
    }
}

/// The `ready` message, as reported. Untrusted until [`Ready::validate`].
///
/// Only [`Reader`] constructs one, so the multi-endpoint fact it records
/// cannot be lost to a deserializer; it is serialized for the record only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ready {
    /// The reported endpoint text, exactly as sent.
    pub endpoint: String,
    /// The reported certificate path, exactly as sent.
    pub certificate_path: String,
    /// The retention the sidecar applied.
    pub effective_retention: Option<String>,
    /// The CA memory-lock posture it reported.
    pub memory_lock: Option<String>,
    /// Limits the producer named as not enforced.
    pub unenforced_limits: Vec<String>,
    /// The producer identity subset it repeated.
    pub producer: Value,
    /// Set when the message carried more than one endpoint.
    #[serde(skip)]
    second_endpoint: bool,
}

/// The `finished` message, as reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Terminal {
    /// What the sidecar says its capture was.
    pub completion: Completion,
    /// `sha256:` digest of the manifest bytes, when one exists.
    pub capture_digest: Option<String>,
    /// Where the manifest is, when one exists.
    pub manifest_path: Option<String>,
    /// Why capture is not complete, when it is not.
    pub reason: Option<String>,
    /// Every gap it named.
    pub gaps: Vec<String>,
    /// How its custody closed.
    pub end_disposition: Option<String>,
    /// Exchanges it reported.
    pub exchange_count: u64,
    /// Findings it reported.
    pub finding_count: u64,
    /// The producer identity subset it repeated.
    pub producer: Value,
    /// The producer's evidence reference, kept as it was written and never
    /// interpreted here: spec 014 judges it, through its additive edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_reference: Option<Value>,
}

/// Capture completion (section 3.2). `Unknown` is this product's word for a
/// closing outcome with no terminal message; the sidecar never sends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Completion {
    /// Capture is complete.
    Complete,
    /// Capture is incomplete; gaps are named.
    Incomplete,
    /// Nothing was captured.
    Absent,
    /// No terminal message says.
    Unknown,
}

/// What one accepted line was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// The one readiness message.
    Ready(Ready),
    /// A finding notice: a code and maybe an authority.
    Finding {
        /// The producer's finding code.
        code: String,
    },
    /// An exchange notice: identities and digests only.
    Exchange {
        /// Its sequence number.
        sequence: u64,
        /// Its record digest.
        record_digest: String,
    },
    /// The one terminal message.
    Finished(Terminal),
}

/// A protocol finding (section 3.2). None of them is a successful finish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case", tag = "finding")]
pub enum Finding {
    /// A line over [`MAX_LINE_BYTES`].
    #[error("oversized-line")]
    OversizedLine,
    /// Not valid UTF-8.
    #[error("invalid-utf8")]
    InvalidUtf8,
    /// Not one JSON object on one line.
    #[error("malformed-line")]
    MalformedLine,
    /// The `protocol` member is not [`PROTOCOL`].
    #[error("invalid-protocol")]
    InvalidProtocol,
    /// An unknown `type`.
    #[error("unknown-message-type")]
    UnknownMessageType,
    /// The binding differs from the supplied one, in any byte.
    #[error("binding-mismatch")]
    BindingMismatch,
    /// A second `ready`.
    #[error("duplicate-ready")]
    DuplicateReady,
    /// A second `finished`; it does not replace or amend the first.
    #[error("duplicate-terminal")]
    DuplicateTerminal,
    /// Anything after `finished`.
    #[error("message-after-terminal")]
    MessageAfterTerminal,
    /// A notice or terminal before `ready`, or a `start` from the sidecar.
    #[error("unexpected-message")]
    UnexpectedMessage,
    /// A required member is missing, mistyped or oversized.
    #[error("invalid-field: {0}")]
    InvalidField(String),
    /// More than [`MAX_MESSAGES`].
    #[error("too-many-messages")]
    TooManyMessages,
    /// Private-key material in a protocol field: a custody violation
    /// (section 3.5). The attempt is refused and the child and sidecar stop.
    #[error("custody-violation")]
    CustodyViolation,
}

impl Finding {
    /// Whether this finding refuses the attempt outright (section 3.5).
    #[must_use]
    pub fn is_custody_violation(&self) -> bool {
        matches!(self, Finding::CustodyViolation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    AwaitReady,
    Running,
    Finished,
}

/// The bounded reader of one sidecar's stdout.
#[derive(Debug, Clone)]
pub struct Reader {
    binding: AttemptBinding,
    phase: Phase,
    messages: u64,
    findings: Vec<Finding>,
    terminal: Option<Terminal>,
}

impl Reader {
    /// A reader bound to the supplied tuple.
    #[must_use]
    pub fn new(binding: AttemptBinding) -> Self {
        Self {
            binding,
            phase: Phase::AwaitReady,
            messages: 0,
            findings: Vec::new(),
            terminal: None,
        }
    }

    /// Read one line, newline included. A finding is recorded and returned;
    /// the reader stays usable so later findings are also counted.
    ///
    /// # Errors
    ///
    /// The [`Finding`] the line produced.
    pub fn read(&mut self, line: &[u8]) -> Result<Notice, Finding> {
        let result = self.classify(line);
        if let Err(finding) = &result {
            self.findings.push(finding.clone());
        }
        result
    }

    fn classify(&mut self, line: &[u8]) -> Result<Notice, Finding> {
        self.messages += 1;
        if self.messages > MAX_MESSAGES {
            return Err(Finding::TooManyMessages);
        }
        if line.len() > MAX_LINE_BYTES {
            return Err(Finding::OversizedLine);
        }
        let Some(body) = line.strip_suffix(b"\n") else {
            return Err(Finding::MalformedLine);
        };
        if body.contains(&b'\n') {
            return Err(Finding::MalformedLine);
        }
        let text = std::str::from_utf8(body).map_err(|_| Finding::InvalidUtf8)?;
        let value: Value = serde_json::from_str(text).map_err(|_| Finding::MalformedLine)?;
        if carries_key_material(&value) {
            return Err(Finding::CustodyViolation);
        }
        let Value::Object(message) = value else {
            return Err(Finding::MalformedLine);
        };
        if message.get("protocol").and_then(Value::as_str) != Some(PROTOCOL) {
            return Err(Finding::InvalidProtocol);
        }
        if message.get("binding") != Some(&self.binding.json()) {
            return Err(Finding::BindingMismatch);
        }
        let kind = message.get("type").and_then(Value::as_str);
        if self.phase == Phase::Finished {
            return Err(if kind == Some("finished") {
                Finding::DuplicateTerminal
            } else {
                Finding::MessageAfterTerminal
            });
        }
        match (self.phase, kind) {
            (Phase::AwaitReady, Some("ready")) => {
                let ready = parse_ready(&message)?;
                self.phase = Phase::Running;
                Ok(Notice::Ready(ready))
            }
            (Phase::Running, Some("ready")) => Err(Finding::DuplicateReady),
            (Phase::Running, Some("finding")) => Ok(Notice::Finding {
                code: text_field(&message, "code")?,
            }),
            (Phase::Running, Some("exchange")) => Ok(Notice::Exchange {
                sequence: number_field(&message, "sequence")?,
                record_digest: text_field(&message, "record_digest")?,
            }),
            (Phase::Running, Some("finished")) => {
                let terminal = parse_terminal(&message)?;
                self.phase = Phase::Finished;
                self.terminal = Some(terminal.clone());
                Ok(Notice::Finished(terminal))
            }
            (_, Some("ready" | "finding" | "exchange" | "finished" | "start")) => {
                Err(Finding::UnexpectedMessage)
            }
            _ => Err(Finding::UnknownMessageType),
        }
    }

    /// Whether a `ready` was accepted.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.phase != Phase::AwaitReady
    }

    /// The first accepted terminal message, if any. EOF before one is
    /// `sidecar-stopped`; nothing here synthesizes it.
    #[must_use]
    pub fn terminal(&self) -> Option<&Terminal> {
        self.terminal.as_ref()
    }

    /// Every finding so far, in order.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// The binding this reader accepts.
    #[must_use]
    pub fn binding(&self) -> &AttemptBinding {
        &self.binding
    }
}

/// Field names whose presence anywhere is key material or credential
/// content. The producer's own parser refuses the same names.
const FORBIDDEN_FIELDS: &[&str] = &[
    "ca_key",
    "ca_key_path",
    "private_key",
    "private_key_path",
    "request_body",
    "response_body",
    "captured_content",
    "content",
    "authorization",
    "proxy_authorization",
    "cookie",
    "set_cookie",
    "api_key",
    "access_token",
    "refresh_token",
    "client_secret",
];

fn carries_key_material(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains("PRIVATE KEY"),
        Value::Array(values) => values.iter().any(carries_key_material),
        Value::Object(fields) => fields.iter().any(|(name, child)| {
            let normalized = name.to_ascii_lowercase().replace('-', "_");
            FORBIDDEN_FIELDS.contains(&normalized.as_str()) || carries_key_material(child)
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn text_field(message: &Map<String, Value>, name: &str) -> Result<String, Finding> {
    match message.get(name) {
        Some(Value::String(s)) if !s.is_empty() && s.len() <= MAX_TEXT => Ok(s.clone()),
        _ => Err(Finding::InvalidField(name.to_string())),
    }
}

fn optional_text(message: &Map<String, Value>, name: &str) -> Result<Option<String>, Finding> {
    match message.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => text_field(message, name).map(Some),
    }
}

fn number_field(message: &Map<String, Value>, name: &str) -> Result<u64, Finding> {
    message
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| Finding::InvalidField(name.to_string()))
}

fn text_list(message: &Map<String, Value>, name: &str) -> Result<Vec<String>, Finding> {
    match message.get(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|v| match v {
                Value::String(s) if !s.is_empty() && s.len() <= MAX_TEXT => Ok(s.clone()),
                _ => Err(Finding::InvalidField(name.to_string())),
            })
            .collect(),
        Some(_) => Err(Finding::InvalidField(name.to_string())),
    }
}

fn parse_ready(message: &Map<String, Value>) -> Result<Ready, Finding> {
    // The protocol's endpoint is one string. More than one endpoint, in an
    // array or beside it, refuses child spawn as a second endpoint; any other
    // shape, a one-element array included, is not the protocol's form.
    let second_endpoint = match message.get("endpoint") {
        Some(Value::Array(values)) if values.len() > 1 => true,
        Some(Value::Array(_)) => return Err(Finding::InvalidField("endpoint".into())),
        _ => message.contains_key("endpoints"),
    };
    let endpoint = match message.get("endpoint") {
        Some(Value::Array(values)) => values
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => text_field(message, "endpoint")?,
    };
    Ok(Ready {
        endpoint,
        certificate_path: text_field(message, "certificate_path")?,
        effective_retention: optional_text(message, "effective_retention")?,
        memory_lock: optional_text(message, "memory_lock")?,
        unenforced_limits: text_list(message, "unenforced_limits")?,
        producer: message.get("producer").cloned().unwrap_or(Value::Null),
        second_endpoint,
    })
}

fn parse_terminal(message: &Map<String, Value>) -> Result<Terminal, Finding> {
    let completion = match message.get("completeness").and_then(Value::as_str) {
        Some("complete") => Completion::Complete,
        Some("incomplete") => Completion::Incomplete,
        Some("absent") => Completion::Absent,
        _ => return Err(Finding::InvalidField("completeness".into())),
    };
    let terminal = Terminal {
        completion,
        capture_digest: optional_text(message, "capture_digest")?,
        manifest_path: optional_text(message, "manifest_path")?,
        reason: optional_text(message, "reason")?,
        gaps: text_list(message, "gaps")?,
        end_disposition: optional_text(message, "end_disposition")?,
        exchange_count: number_field(message, "exchange_count")?,
        finding_count: number_field(message, "finding_count")?,
        producer: message.get("producer").cloned().unwrap_or(Value::Null),
        evidence_reference: match message.get("evidence_reference") {
            None | Some(Value::Null) => None,
            Some(Value::Object(reference)) => Some(Value::Object(reference.clone())),
            Some(_) => return Err(Finding::InvalidField("evidence_reference".into())),
        },
    };
    // A complete capture names its manifest and digest and no reason; any
    // other names a reason. The producer's parser holds the same rule.
    let complete_ok = terminal.manifest_path.is_some()
        && terminal.capture_digest.is_some()
        && terminal.reason.is_none();
    match completion {
        Completion::Complete if !complete_ok => Err(Finding::InvalidField("completeness".into())),
        Completion::Incomplete | Completion::Absent if terminal.reason.is_none() => {
            Err(Finding::InvalidField("reason".into()))
        }
        _ => Ok(terminal),
    }
}

/// Why a `ready` refuses child spawn (sections 3.3 and 3.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case", tag = "refusal")]
pub enum ReadyRefusal {
    /// More than one endpoint was reported.
    #[error("second-endpoint")]
    SecondEndpoint,
    /// The endpoint is not a socket address.
    #[error("endpoint-unparsable")]
    EndpointUnparsable,
    /// A wildcard listener.
    #[error("endpoint-wildcard")]
    Wildcard,
    /// A non-loopback reported address.
    #[error("endpoint-not-loopback")]
    NotLoopback,
    /// Not the endpoint this product reserved.
    #[error("endpoint-not-reserved")]
    NotReserved,
    /// The certificate is not directly in the attempt's sidecar area.
    #[error("certificate-outside-area")]
    CertificateOutsideArea,
    /// The certificate is not a bounded regular file.
    #[error("certificate-not-regular")]
    CertificateNotRegular,
    /// The certificate file carries private-key material.
    #[error("custody-violation")]
    CustodyViolation,
    /// A different certificate path from the one supplied in `start`, so a
    /// second CA.
    #[error("second-certificate")]
    SecondCertificate,
}

/// A `ready` that may be supplied to the child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedReady {
    /// The one endpoint the child receives.
    pub endpoint: SocketAddr,
    /// The one certificate path the child receives.
    pub certificate_path: PathBuf,
}

impl Ready {
    /// Validate against the reserved endpoint and the attempt's sidecar area.
    ///
    /// The endpoint check is of the **reported** address. On Linux this is
    /// not OS-enforced address restriction (section 3.6.1), and nothing here
    /// claims it is.
    ///
    /// # Errors
    ///
    /// The [`ReadyRefusal`] that refuses child spawn.
    pub fn validate(
        &self,
        reserved: SocketAddr,
        area: &Path,
        supplied_certificate: &Path,
    ) -> Result<ValidatedReady, ReadyRefusal> {
        if self.second_endpoint {
            return Err(ReadyRefusal::SecondEndpoint);
        }
        let endpoint: SocketAddr = self
            .endpoint
            .parse()
            .map_err(|_| ReadyRefusal::EndpointUnparsable)?;
        if endpoint.ip().is_unspecified() {
            return Err(ReadyRefusal::Wildcard);
        }
        if !endpoint.ip().is_loopback() {
            return Err(ReadyRefusal::NotLoopback);
        }
        if endpoint != reserved {
            return Err(ReadyRefusal::NotReserved);
        }
        let certificate = PathBuf::from(&self.certificate_path);
        if certificate.parent() != Some(area) {
            return Err(ReadyRefusal::CertificateOutsideArea);
        }
        if certificate != supplied_certificate {
            return Err(ReadyRefusal::SecondCertificate);
        }
        let metadata = std::fs::symlink_metadata(&certificate)
            .map_err(|_| ReadyRefusal::CertificateNotRegular)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_CERTIFICATE_BYTES {
            return Err(ReadyRefusal::CertificateNotRegular);
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&certificate)
            .and_then(|f| f.take(MAX_CERTIFICATE_BYTES).read_to_end(&mut bytes))
            .map_err(|_| ReadyRefusal::CertificateNotRegular)?;
        if bytes
            .windows(b"PRIVATE KEY".len())
            .any(|w| w == b"PRIVATE KEY")
        {
            return Err(ReadyRefusal::CustodyViolation);
        }
        Ok(ValidatedReady {
            endpoint,
            certificate_path: certificate,
        })
    }
}

/// Whether the manifest a terminal message names is the bytes it digests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Integrity {
    /// The manifest is in the area and hashes to the reported digest.
    Intact,
    /// It hashes to something else. The bytes are not repaired.
    Mismatch,
    /// No manifest or no digest was reported, or the file is unreadable.
    Missing,
    /// The manifest path is outside the attempt's sidecar area.
    OutsideArea,
}

impl Terminal {
    /// Check the manifest's bytes against the reported digest.
    #[must_use]
    pub fn integrity(&self, area: &Path) -> Integrity {
        let (Some(path), Some(digest)) = (&self.manifest_path, &self.capture_digest) else {
            return Integrity::Missing;
        };
        let path = Path::new(path);
        if !path.starts_with(area)
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Integrity::OutsideArea;
        }
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            return Integrity::Missing;
        };
        if !metadata.file_type().is_file() || metadata.len() > MAX_MANIFEST_BYTES {
            return Integrity::Missing;
        }
        let mut bytes = Vec::new();
        if std::fs::File::open(path)
            .and_then(|f| f.take(MAX_MANIFEST_BYTES).read_to_end(&mut bytes))
            .is_err()
        {
            return Integrity::Missing;
        }
        if format!("sha256:{}", hex(&Sha256::digest(&bytes))) == *digest {
            Integrity::Intact
        } else {
            Integrity::Mismatch
        }
    }
}

/// How the sidecar process ended (section 3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Termination {
    /// Exited after writing `finished`.
    Finished,
    /// Exited non-zero or broke the protocol.
    Failed,
    /// Stopped by the supervisor closing its stdin after the child.
    Stopped,
    /// Killed after its grace period.
    Killed,
    /// Not observed.
    Unknown,
}

/// The sidecar effect's one closing outcome (section 3.2).
///
/// Written to the run record repeating the effect id. Every field is
/// testimony or this product's own observation; nothing here is acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Closing {
    /// The effect this closes.
    pub effect_id: String,
    /// The capture digest, when one exists.
    pub capture_digest: Option<String>,
    /// Why no digest exists, when none does. Never both absent.
    pub no_digest_reason: Option<String>,
    /// The attestation identity, or `unknown`.
    pub attestation: String,
    /// What capture completion the terminal message reported, or `unknown`.
    pub completion: Completion,
    /// Every gap supplied.
    pub gaps: Vec<String>,
    /// How the sidecar ended.
    pub termination: Termination,
    /// The manifest's integrity against its digest.
    pub integrity: Integrity,
    /// Every protocol finding, in order.
    pub findings: Vec<Finding>,
    /// The exact producer identity.
    pub producer: String,
    /// The exact interface identity.
    pub interface: String,
    /// The final witness posture (section 3.7).
    pub posture: crate::witness::WitnessPosture,
}

impl Closing {
    /// Assemble the closing outcome from what was observed. A missing
    /// terminal message is `unknown` completion and an explicit no-digest
    /// reason; nothing is synthesized.
    #[must_use]
    pub fn assemble(
        reader: &Reader,
        termination: Termination,
        integrity: Integrity,
        producer: &str,
        interface: &str,
        posture: crate::witness::WitnessPosture,
    ) -> Self {
        let terminal = reader.terminal();
        let capture_digest = terminal.and_then(|t| t.capture_digest.clone());
        let no_digest_reason = match (terminal, &capture_digest) {
            (_, Some(_)) => None,
            (Some(t), None) => Some(
                t.reason
                    .clone()
                    .unwrap_or_else(|| "no-digest-reported".into()),
            ),
            (None, None) if reader.is_ready() => Some("sidecar-stopped".into()),
            (None, None) => Some("sidecar-exited-before-ready".into()),
        };
        Self {
            effect_id: reader.binding().effect_id.clone(),
            capture_digest,
            no_digest_reason,
            attestation: "unknown".into(),
            completion: terminal.map_or(Completion::Unknown, |t| t.completion),
            gaps: terminal.map(|t| t.gaps.clone()).unwrap_or_default(),
            termination,
            integrity,
            findings: reader.findings().to_vec(),
            producer: producer.to_string(),
            interface: interface.to_string(),
            posture,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> AttemptBinding {
        AttemptBinding::new("013-run", 2, "effect-7").unwrap()
    }

    fn line(value: Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn message(kind: &str, extra: Value) -> Vec<u8> {
        let mut value = json!({ "protocol": PROTOCOL, "type": kind, "binding": binding().json() });
        for (k, v) in extra.as_object().unwrap() {
            value[k] = v.clone();
        }
        line(value)
    }

    fn ready(endpoint: &str, certificate: &str) -> Vec<u8> {
        message(
            "ready",
            json!({ "endpoint": endpoint, "certificate_path": certificate, "memory_lock": "not-attempted" }),
        )
    }

    fn finished_complete() -> Vec<u8> {
        message(
            "finished",
            json!({ "completeness": "complete", "capture_digest": "sha256:ab", "manifest_path": "/a/m.json",
                     "exchange_count": 1, "finding_count": 0, "gaps": [], "reason": null }),
        )
    }

    #[test]
    fn a_binding_needs_two_bounded_single_line_strings() {
        assert!(AttemptBinding::new("", 1, "e").is_none());
        assert!(AttemptBinding::new("r", 1, "").is_none());
        assert!(AttemptBinding::new("r\n", 1, "e").is_none());
        assert!(AttemptBinding::new(&"r".repeat(MAX_TEXT + 1), 1, "e").is_none());
        assert!(AttemptBinding::new("r", 0, "e").is_some());
    }

    #[test]
    fn the_start_line_carries_the_binding_and_the_reserved_endpoint() {
        let start = Start {
            binding: binding(),
            capture_authorities: vec![Authority {
                authority: "api.example.test:443".into(),
                provider: "unknown".into(),
                protocols: vec!["http/1.1".into()],
            }],
            authentication_tunnels: Vec::new(),
            requested_retention: "metadata-only".into(),
            limits: Limits::default(),
            output_directory: "/area/out".into(),
            certificate_path: "/area/out/ca.pem".into(),
            host_policy_id: "statecraft/013".into(),
            listen_endpoint: "127.0.0.1:41000".parse().unwrap(),
        };
        let bytes = start.line();
        assert!(bytes.ends_with(b"\n"));
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["type"], "start");
        assert_eq!(value["binding"], binding().json());
        assert_eq!(value["listen_endpoint"], "127.0.0.1:41000");
        assert_eq!(value["limits"]["total_seconds"], 14_400);
        assert_eq!(value["limits"]["header_bytes"], 65_536);
    }

    #[test]
    fn a_complete_session_reads_ready_notices_and_one_terminal() {
        let mut reader = Reader::new(binding());
        assert!(matches!(
            reader.read(&ready("127.0.0.1:41000", "/a/ca.pem")),
            Ok(Notice::Ready(_))
        ));
        assert!(matches!(
            reader.read(&message("finding", json!({ "code": "tls-alpn-mismatch" }))),
            Ok(Notice::Finding { .. })
        ));
        assert!(matches!(
            reader.read(&message(
                "exchange",
                json!({ "sequence": 0, "record_digest": "sha256:cd" })
            )),
            Ok(Notice::Exchange { sequence: 0, .. })
        ));
        assert!(matches!(
            reader.read(&finished_complete()),
            Ok(Notice::Finished(_))
        ));
        assert_eq!(reader.terminal().unwrap().completion, Completion::Complete);
        assert!(reader.findings().is_empty());
    }

    #[test]
    fn the_evidence_reference_is_kept_as_written_and_a_non_object_is_a_finding() {
        let reference = json!({ "evidence_type": "statecraft/wire-exchange/v1", "sha256": "ab" });
        let finished = |r: Value| {
            message(
                "finished",
                json!({ "completeness": "complete", "capture_digest": "sha256:ab",
                         "manifest_path": "/a/m.json", "exchange_count": 1, "finding_count": 0,
                         "gaps": [], "reason": null, "evidence_reference": r }),
            )
        };
        let mut reader = Reader::new(binding());
        reader.read(&ready("127.0.0.1:41000", "/a/ca.pem")).unwrap();
        reader.read(&finished(reference.clone())).unwrap();
        assert_eq!(
            reader.terminal().unwrap().evidence_reference,
            Some(reference)
        );
        let mut reader = Reader::new(binding());
        reader.read(&ready("127.0.0.1:41000", "/a/ca.pem")).unwrap();
        assert_eq!(
            reader.read(&finished(json!("not-an-object"))),
            Err(Finding::InvalidField("evidence_reference".into()))
        );
    }

    #[test]
    fn a_changed_binding_byte_binds_nothing() {
        let mut reader = Reader::new(binding());
        for changed in [
            json!({ "run_id": "013-RUN", "attempt": 2, "effect_id": "effect-7" }),
            json!({ "run_id": "013-run", "attempt": 3, "effect_id": "effect-7" }),
            json!({ "run_id": "013-run", "attempt": 2, "effect_id": "effect-8" }),
            json!({ "run_id": "013-run", "attempt": "2", "effect_id": "effect-7" }),
        ] {
            let bytes = line(
                json!({ "protocol": PROTOCOL, "type": "ready", "binding": changed,
                                      "endpoint": "127.0.0.1:1", "certificate_path": "/a/c" }),
            );
            assert_eq!(reader.read(&bytes), Err(Finding::BindingMismatch));
        }
        assert!(!reader.is_ready());
    }

    #[test]
    fn duplicates_and_late_messages_are_findings_and_never_replace_the_first() {
        let mut reader = Reader::new(binding());
        assert_eq!(
            reader.read(&finished_complete()),
            Err(Finding::UnexpectedMessage)
        );
        reader.read(&ready("127.0.0.1:41000", "/a/ca.pem")).unwrap();
        assert_eq!(
            reader.read(&ready("127.0.0.1:41001", "/a/ca.pem")),
            Err(Finding::DuplicateReady)
        );
        reader.read(&finished_complete()).unwrap();
        let second = message(
            "finished",
            json!({ "completeness": "absent", "reason": "late", "exchange_count": 0, "finding_count": 0 }),
        );
        assert_eq!(reader.read(&second), Err(Finding::DuplicateTerminal));
        assert_eq!(
            reader.read(&message("finding", json!({ "code": "x" }))),
            Err(Finding::MessageAfterTerminal)
        );
        assert_eq!(reader.terminal().unwrap().completion, Completion::Complete);
        assert_eq!(reader.findings().len(), 4);
    }

    #[test]
    fn malformed_oversized_and_foreign_lines_are_findings() {
        let mut reader = Reader::new(binding());
        assert_eq!(reader.read(b"{}"), Err(Finding::MalformedLine));
        assert_eq!(reader.read(b"not json\n"), Err(Finding::MalformedLine));
        assert_eq!(reader.read(b"\xff\xfe\n"), Err(Finding::InvalidUtf8));
        let mut huge = vec![b' '; MAX_LINE_BYTES];
        huge.push(b'\n');
        assert_eq!(reader.read(&huge), Err(Finding::OversizedLine));
        let foreign = line(
            json!({ "protocol": "wire-witness.sidecar/2", "type": "ready", "binding": binding().json() }),
        );
        assert_eq!(reader.read(&foreign), Err(Finding::InvalidProtocol));
        assert_eq!(
            reader.read(&message("telemetry", json!({}))),
            Err(Finding::UnknownMessageType)
        );
    }

    #[test]
    fn key_material_in_any_field_is_a_custody_violation() {
        let mut reader = Reader::new(binding());
        let named = message(
            "ready",
            json!({ "endpoint": "127.0.0.1:1", "certificate_path": "/a/c", "ca_key": "x" }),
        );
        assert!(reader.read(&named).unwrap_err().is_custody_violation());
        let pem = message("finding", json!({ "code": "-----BEGIN PRIVATE KEY-----" }));
        assert!(reader.read(&pem).unwrap_err().is_custody_violation());
    }

    #[test]
    fn an_incomplete_terminal_must_name_a_reason_and_a_complete_one_its_digest() {
        let mut reader = Reader::new(binding());
        reader.read(&ready("127.0.0.1:41000", "/a/ca.pem")).unwrap();
        let no_digest = message(
            "finished",
            json!({ "completeness": "complete", "manifest_path": "/a/m", "exchange_count": 0, "finding_count": 0 }),
        );
        assert!(matches!(
            reader.read(&no_digest),
            Err(Finding::InvalidField(_))
        ));
        let no_reason = message(
            "finished",
            json!({ "completeness": "incomplete", "exchange_count": 0, "finding_count": 0 }),
        );
        assert!(matches!(
            reader.read(&no_reason),
            Err(Finding::InvalidField(_))
        ));
        let empty_gap = message(
            "finished",
            json!({ "completeness": "incomplete", "reason": "r", "gaps": [""], "exchange_count": 0, "finding_count": 0 }),
        );
        assert_eq!(
            reader.read(&empty_gap),
            Err(Finding::InvalidField("gaps".into()))
        );
        assert!(reader.terminal().is_none());
    }

    fn area_with_certificate(pem: &[u8]) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let area = dir.path().join("out");
        std::fs::create_dir(&area).unwrap();
        let cert = area.join("ca.pem");
        std::fs::write(&cert, pem).unwrap();
        (dir, area, cert)
    }

    fn reported(endpoint: &str, certificate: &Path) -> Ready {
        let mut reader = Reader::new(binding());
        match reader.read(&ready(endpoint, &certificate.display().to_string())) {
            Ok(Notice::Ready(ready)) => ready,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn only_the_reserved_loopback_endpoint_and_supplied_certificate_validate() {
        let (_dir, area, cert) = area_with_certificate(b"-----BEGIN CERTIFICATE-----\n");
        let reserved: SocketAddr = "127.0.0.1:41000".parse().unwrap();
        let ok = reported("127.0.0.1:41000", &cert)
            .validate(reserved, &area, &cert)
            .unwrap();
        assert_eq!(ok.endpoint, reserved);
        for (endpoint, refusal) in [
            ("0.0.0.0:41000", ReadyRefusal::Wildcard),
            ("[::]:41000", ReadyRefusal::Wildcard),
            ("192.0.2.1:41000", ReadyRefusal::NotLoopback),
            ("127.0.0.1:41001", ReadyRefusal::NotReserved),
            ("localhost:41000", ReadyRefusal::EndpointUnparsable),
        ] {
            assert_eq!(
                reported(endpoint, &cert).validate(reserved, &area, &cert),
                Err(refusal),
                "{endpoint}"
            );
        }
        let two = {
            let mut reader = Reader::new(binding());
            let bytes = message(
                "ready",
                json!({ "endpoint": ["127.0.0.1:41000", "127.0.0.1:41001"],
                         "certificate_path": cert.display().to_string() }),
            );
            match reader.read(&bytes) {
                Ok(Notice::Ready(r)) => r,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(
            two.validate(reserved, &area, &cert),
            Err(ReadyRefusal::SecondEndpoint)
        );
        let mut reader = Reader::new(binding());
        let one_in_an_array = message(
            "ready",
            json!({ "endpoint": ["127.0.0.1:41000"], "certificate_path": cert.display().to_string() }),
        );
        assert_eq!(
            reader.read(&one_in_an_array),
            Err(Finding::InvalidField("endpoint".into()))
        );
    }

    #[test]
    fn a_certificate_elsewhere_a_second_one_or_one_carrying_a_key_refuses_spawn() {
        let (dir, area, cert) = area_with_certificate(b"-----BEGIN CERTIFICATE-----\n");
        let reserved: SocketAddr = "127.0.0.1:41000".parse().unwrap();
        let outside = dir.path().join("ca.pem");
        std::fs::write(&outside, b"cert").unwrap();
        assert_eq!(
            reported("127.0.0.1:41000", &outside).validate(reserved, &area, &cert),
            Err(ReadyRefusal::CertificateOutsideArea)
        );
        let other = area.join("other.pem");
        std::fs::write(&other, b"cert").unwrap();
        assert_eq!(
            reported("127.0.0.1:41000", &other).validate(reserved, &area, &cert),
            Err(ReadyRefusal::SecondCertificate)
        );
        std::fs::write(&cert, b"-----BEGIN EC PRIVATE KEY-----\n").unwrap();
        assert_eq!(
            reported("127.0.0.1:41000", &cert).validate(reserved, &area, &cert),
            Err(ReadyRefusal::CustodyViolation)
        );
        std::fs::remove_file(&cert).unwrap();
        std::os::unix::fs::symlink(&outside, &cert).unwrap();
        assert_eq!(
            reported("127.0.0.1:41000", &cert).validate(reserved, &area, &cert),
            Err(ReadyRefusal::CertificateNotRegular)
        );
    }

    fn terminal_for(manifest: &Path, digest: &str) -> Terminal {
        Terminal {
            completion: Completion::Complete,
            capture_digest: Some(digest.into()),
            manifest_path: Some(manifest.display().to_string()),
            reason: None,
            gaps: Vec::new(),
            end_disposition: None,
            exchange_count: 0,
            finding_count: 0,
            producer: Value::Null,
            evidence_reference: None,
        }
    }

    #[test]
    fn integrity_compares_the_manifest_bytes_and_never_repairs_them() {
        let (dir, area, _) = area_with_certificate(b"c");
        let manifest = area.join("wire-exchange.json");
        std::fs::write(&manifest, b"manifest").unwrap();
        let digest = format!("sha256:{}", hex(&Sha256::digest(b"manifest")));
        assert_eq!(
            terminal_for(&manifest, &digest).integrity(&area),
            Integrity::Intact
        );
        assert_eq!(
            terminal_for(&manifest, "sha256:00").integrity(&area),
            Integrity::Mismatch
        );
        let outside = dir.path().join("m.json");
        std::fs::write(&outside, b"manifest").unwrap();
        assert_eq!(
            terminal_for(&outside, &digest).integrity(&area),
            Integrity::OutsideArea
        );
        assert_eq!(
            terminal_for(&area.join("absent"), &digest).integrity(&area),
            Integrity::Missing
        );
        assert_eq!(std::fs::read(&manifest).unwrap(), b"manifest");
    }

    #[test]
    fn a_closing_without_a_terminal_names_why_no_digest_exists() {
        let posture = crate::witness::WitnessPosture::WitnessFailed;
        let early = Reader::new(binding());
        let closing = Closing::assemble(
            &early,
            Termination::Failed,
            Integrity::Missing,
            "p",
            "i",
            posture,
        );
        assert_eq!(closing.capture_digest, None);
        assert_eq!(
            closing.no_digest_reason.as_deref(),
            Some("sidecar-exited-before-ready")
        );
        assert_eq!(closing.completion, Completion::Unknown);
        assert_eq!(closing.attestation, "unknown");
        assert_eq!(closing.effect_id, "effect-7");

        let mut late = Reader::new(binding());
        late.read(&ready("127.0.0.1:41000", "/a/ca.pem")).unwrap();
        let closing = Closing::assemble(
            &late,
            Termination::Stopped,
            Integrity::Missing,
            "p",
            "i",
            posture,
        );
        assert_eq!(closing.no_digest_reason.as_deref(), Some("sidecar-stopped"));
    }
}
