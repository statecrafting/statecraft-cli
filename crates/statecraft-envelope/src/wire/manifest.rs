//! The exact v1 manifest: its member set, and the facts section 3.3 names.
//!
//! [`SCHEMA`] is the member set of `statecraft/wire-exchange/v1` as
//! wire-witness 0.1.0 writes it (`crates/wire-witness-cli/src/wire_exchange.rs`
//! at tag `v0.1.0`), plus the two section 3.3 representations that release
//! does not yet emit: a known attestation (`identity`, `construction`) and a
//! labeled estimate (`label`, `value`, `currency`, `method`, `inputs`). Every
//! member in it is metadata under wire-witness spec 003 sections 3.1 to 3.3;
//! none is a body, a header, a cookie, an authorization value or a token.
//! The screen ([`super::screen`]) refuses every member outside it, which is
//! how classification comes from the versioned schema and not from a guess at
//! what a member name means.
//!
//! [`Facts::read`] is step 9: required facts and cross-field consistency. It
//! runs only after the screen passed, so the values it copies are ones the
//! screen already judged safe to retain.

use serde::{Deserialize, Serialize};

use super::bounded::Json;
use super::{Producer, Stated, Subject};

/// The type a schema position holds. The screen reads only member names from
/// it; [`Facts::read`] reads the types.
#[derive(Debug)]
pub enum Shape {
    /// A leaf: string, number or `null`.
    Leaf,
    /// An array whose every element has the shape.
    Array(&'static Shape),
    /// An object with exactly these optional members.
    Object(&'static [(&'static str, Shape)]),
    /// Either a leaf or an object of these members (a value or its state).
    LeafOr(&'static [(&'static str, Shape)]),
}

const STATE: &[(&str, Shape)] = &[("state", Shape::Leaf)];

const MODEL: &[(&str, Shape)] = &[("state", Shape::Leaf), ("value", Shape::Leaf)];

const ENDPOINT: &[(&str, Shape)] = &[
    ("destination", Shape::Leaf),
    ("model", Shape::Object(MODEL)),
    ("protocol", Shape::Leaf),
];

const USAGE_VALUE: &[(&str, Shape)] = &[
    ("location", Shape::Leaf),
    ("providerField", Shape::Leaf),
    ("unit", Shape::LeafOr(STATE)),
    ("value", Shape::Leaf),
];

const ESTIMATE: &[(&str, Shape)] = &[
    ("currency", Shape::Leaf),
    ("inputs", Shape::Array(&Shape::Leaf)),
    ("label", Shape::Leaf),
    ("method", Shape::Leaf),
    ("state", Shape::Leaf),
    ("value", Shape::Leaf),
];

const EXCHANGE: &[(&str, Shape)] = &[
    ("completeness", Shape::Leaf),
    ("estimatedCost", Shape::Object(ESTIMATE)),
    ("recordByteLength", Shape::Leaf),
    ("recordDigest", Shape::Leaf),
    ("requested", Shape::Object(ENDPOINT)),
    ("sequence", Shape::Leaf),
    ("served", Shape::Object(ENDPOINT)),
    (
        "usage",
        Shape::Object(&[
            ("label", Shape::Leaf),
            ("values", Shape::Array(&Shape::Object(USAGE_VALUE))),
        ]),
    ),
];

/// The v1 manifest's member set.
pub const SCHEMA: Shape = Shape::Object(&[
    ("artifactType", Shape::Leaf),
    (
        "attestation",
        Shape::Object(&[
            ("construction", Shape::Leaf),
            ("identity", Shape::Leaf),
            ("state", Shape::Leaf),
        ]),
    ),
    (
        "capture",
        Shape::Object(&[
            ("endDisposition", Shape::Leaf),
            ("startDisposition", Shape::Leaf),
        ]),
    ),
    (
        "completeness",
        Shape::Object(&[
            ("gaps", Shape::Array(&Shape::Leaf)),
            ("reason", Shape::Leaf),
            ("state", Shape::Leaf),
        ]),
    ),
    (
        "counts",
        Shape::Object(&[
            ("droppedExchanges", Shape::Leaf),
            ("findings", Shape::Leaf),
            ("observedExchanges", Shape::Leaf),
            ("recordedExchanges", Shape::Leaf),
        ]),
    ),
    ("exchanges", Shape::Array(&Shape::Object(EXCHANGE))),
    ("hostPolicyId", Shape::Leaf),
    (
        "producer",
        Shape::Object(&[
            ("contentDigest", Shape::LeafOr(STATE)),
            ("name", Shape::Leaf),
            ("revision", Shape::Leaf),
        ]),
    ),
    ("recordDigestConstruction", Shape::Leaf),
    (
        "redaction",
        Shape::Object(&[
            ("fieldClasses", Shape::Array(&Shape::Leaf)),
            ("reason", Shape::Leaf),
            ("result", Shape::Leaf),
        ]),
    ),
    (
        "retention",
        Shape::Object(&[
            ("reason", Shape::Leaf),
            ("requested", Shape::Leaf),
            ("result", Shape::Leaf),
        ]),
    ),
    ("schemaVersion", Shape::Leaf),
    (
        "subject",
        Shape::Object(&[
            ("attempt", Shape::Leaf),
            ("runId", Shape::Leaf),
            ("sidecarEffectId", Shape::Leaf),
        ]),
    ),
]);

/// Section 3.3's completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Completeness {
    /// Could not be decided. The lowest.
    Unknown,
    /// At least one named gap.
    Incomplete,
    /// No gap. The highest.
    Complete,
}

/// Section 3.4's retention result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Retention {
    /// The default: metadata only.
    MetadataOnly,
    /// Unsupported until a separately ratified policy names it.
    EncryptedContent,
    /// Nothing was retained.
    Discarded,
    /// Retention failed.
    Failed,
    /// Not decided.
    Unknown,
}

/// Section 3.4's redaction result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Redaction {
    /// Nothing needed removing.
    NotNeeded,
    /// Something was removed.
    Applied,
    /// Redaction failed.
    Failed,
    /// Not decided.
    Unknown,
}

/// The capture's two dispositions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    /// How capture started.
    pub start_disposition: String,
    /// How capture ended.
    pub end_disposition: String,
}

/// Completeness with its gaps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletenessFact {
    /// The state.
    pub state: Completeness,
    /// The named gaps, sorted and distinct.
    pub gaps: Vec<String>,
    /// Why completeness could not be decided, for `unknown`.
    pub reason: Stated<String>,
}

/// Retention, with its reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionFact {
    /// What the attempt asked for, as the producer wrote it.
    pub requested: String,
    /// The result.
    pub result: Retention,
    /// Why, when the producer said.
    pub reason: Stated<String>,
}

/// Redaction, with its reason and the field classes it touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionFact {
    /// The result.
    pub result: Redaction,
    /// Why, when the producer said.
    pub reason: Stated<String>,
    /// The affected field classes.
    pub field_classes: Vec<String>,
}

/// One side of an exchange: what the child asked for, or what was served.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    /// The destination authority.
    pub destination: String,
    /// The protocol identity.
    pub protocol: String,
    /// The model identity, or its state.
    pub model: Stated<String>,
}

/// One provider-reported usage value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageValue {
    /// The provider's own field name.
    pub provider_field: String,
    /// Where in the message it was found.
    pub location: String,
    /// The unit, or its state.
    pub unit: Stated<String>,
    /// The value, as the provider's token text.
    pub value: String,
}

/// Usage: provider testimony, never this product's measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// `provider-testimony`, or `absent` when the provider reported none.
    pub label: String,
    /// The values, in the producer's order.
    pub values: Vec<UsageValue>,
}

/// An estimated cost: an estimate, never billing or observed spend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    /// Always `estimate`.
    pub label: String,
    /// The value, as written.
    pub value: String,
    /// The currency.
    pub currency: String,
    /// The method's identity.
    pub method: String,
    /// The identities of the inputs it used.
    pub inputs: Vec<String>,
}

/// One recorded exchange's facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Exchange {
    /// Its sequence number.
    pub sequence: u64,
    /// Its own completeness word.
    pub completeness: String,
    /// The record's digest.
    pub record_digest: String,
    /// The record's byte length.
    pub record_byte_length: u64,
    /// What the child requested. Never collapsed into `served`.
    pub requested: Endpoint,
    /// What was reported as served.
    pub served: Endpoint,
    /// Provider-reported usage.
    pub usage: Usage,
    /// The estimate, or `not-estimated` as `absent`.
    pub estimated_cost: Stated<Estimate>,
}

/// The attestation: its identity and construction, or `unknown`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attestation {
    /// The attester's identity, or `unknown`; never anonymous success.
    pub identity: Stated<String>,
    /// Its construction, when known.
    pub construction: Stated<String>,
}

/// Every fact section 3.3 names, as one decoded manifest states them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    /// The artifact type the manifest names.
    pub artifact_type: String,
    /// The schema version the manifest names.
    pub schema_version: u64,
    /// The construction the manifest names for its records.
    pub record_digest_construction: String,
    /// The producer the manifest names.
    pub producer: Producer,
    /// The subject the manifest repeats.
    pub subject: Subject,
    /// Capture dispositions.
    pub capture: Capture,
    /// Completeness and gaps.
    pub completeness: CompletenessFact,
    /// Retention.
    pub retention: RetentionFact,
    /// Redaction.
    pub redaction: RedactionFact,
    /// Attestation.
    pub attestation: Attestation,
    /// The exchanges, by sequence.
    pub exchanges: Vec<Exchange>,
}

/// Why the facts are not consistent. Reported as `malformed`; names the
/// position, never its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inconsistent(pub String);

type Read<T> = Result<T, Inconsistent>;

fn bad<T>(at: &str) -> Read<T> {
    Err(Inconsistent(at.to_string()))
}

fn member<'a>(json: &'a Json, at: &str, name: &str) -> Read<&'a Json> {
    match json.get(name) {
        Some(v) => Ok(v),
        None => bad(&format!("{at}.{name}")),
    }
}

fn text(json: &Json, at: &str, name: &str) -> Read<String> {
    match member(json, at, name)? {
        Json::String(s) => Ok(s.clone()),
        _ => bad(&format!("{at}.{name}")),
    }
}

fn number(json: &Json, at: &str, name: &str) -> Read<u64> {
    member(json, at, name)?
        .as_u64()
        .map_or_else(|| bad(&format!("{at}.{name}")), Ok)
}

fn strings(json: &Json, at: &str, name: &str) -> Read<Vec<String>> {
    let Json::Array(items) = member(json, at, name)? else {
        return bad(&format!("{at}.{name}"));
    };
    items
        .iter()
        .map(|i| i.as_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()
        .map_or_else(|| bad(&format!("{at}.{name}")), Ok)
}

/// A reason member: a string, or `null`/missing for none given.
fn reason(json: &Json, at: &str) -> Read<Stated<String>> {
    match json.get("reason") {
        Some(Json::String(s)) => Ok(Stated::known(s.clone())),
        Some(Json::Null) | None => Ok(Stated::Absent),
        Some(_) => bad(&format!("{at}.reason")),
    }
}

/// A `{ state, value }` or bare-value position.
fn stated(json: &Json, at: &str) -> Read<Stated<String>> {
    match json {
        Json::String(s) => Ok(Stated::known(s.clone())),
        Json::Object(_) => match json.get("state").and_then(Json::as_str) {
            Some("known") => match json.get("value") {
                Some(Json::String(s)) => Ok(Stated::known(s.clone())),
                _ => bad(&format!("{at}.value")),
            },
            Some("absent") => Ok(Stated::Absent),
            Some("unknown") => Ok(Stated::Unknown),
            _ => bad(&format!("{at}.state")),
        },
        _ => bad(at),
    }
}

/// A number's exact token text. [`Json::Number`] is this crate's bounded
/// reader's variant, which keeps the token as a `String` (never a
/// `serde_json::Number`), so a provider's value is retained as written.
fn number_text(text: &str) -> String {
    text.to_string()
}

fn sorted_distinct(items: &[String]) -> bool {
    items.windows(2).all(|w| w[0] < w[1])
}

impl Facts {
    /// Step 9: read the facts, refusing every contradiction rather than
    /// normalizing it.
    ///
    /// # Errors
    ///
    /// The position of the first missing, mistyped or contradictory fact.
    pub fn read(json: &Json) -> Read<Facts> {
        let at = "$";
        let producer = member(json, at, "producer")?;
        let subject = member(json, at, "subject")?;
        let attempt = number(subject, "$.subject", "attempt")?;
        let attempt = match u32::try_from(attempt) {
            Ok(n) if n > 0 => n,
            _ => return bad("$.subject.attempt"),
        };
        let capture = member(json, at, "capture")?;
        let completeness = completeness(member(json, at, "completeness")?)?;
        let retention = member(json, at, "retention")?;
        let redaction = member(json, at, "redaction")?;
        let field_classes = strings(redaction, "$.redaction", "fieldClasses")?;
        if !sorted_distinct(&field_classes) {
            return bad("$.redaction.fieldClasses");
        }
        let Json::Array(exchanges) = member(json, at, "exchanges")? else {
            return bad("$.exchanges");
        };
        let exchanges = exchanges
            .iter()
            .enumerate()
            .map(|(i, e)| exchange(e, &format!("$.exchanges[{i}]")))
            .collect::<Read<Vec<_>>>()?;
        if !exchanges.windows(2).all(|w| w[0].sequence < w[1].sequence) {
            return bad("$.exchanges");
        }
        Ok(Facts {
            artifact_type: text(json, at, "artifactType")?,
            schema_version: number(json, at, "schemaVersion")?,
            record_digest_construction: text(json, at, "recordDigestConstruction")?,
            producer: Producer {
                name: Stated::known(text(producer, "$.producer", "name")?),
                revision: Stated::known(text(producer, "$.producer", "revision")?),
                content_digest: stated(
                    member(producer, "$.producer", "contentDigest")?,
                    "$.producer.contentDigest",
                )?,
            },
            subject: Subject {
                run_id: text(subject, "$.subject", "runId")?,
                attempt,
                sidecar_effect_id: text(subject, "$.subject", "sidecarEffectId")?,
            },
            capture: Capture {
                start_disposition: text(capture, "$.capture", "startDisposition")?,
                end_disposition: text(capture, "$.capture", "endDisposition")?,
            },
            completeness,
            retention: RetentionFact {
                requested: text(retention, "$.retention", "requested")?,
                result: word(retention, "$.retention", "result")?,
                reason: reason(retention, "$.retention")?,
            },
            redaction: RedactionFact {
                result: word(redaction, "$.redaction", "result")?,
                reason: reason(redaction, "$.redaction")?,
                field_classes,
            },
            attestation: attestation(member(json, at, "attestation")?)?,
            exchanges,
        })
    }
}

/// A closed word, read through its serde spelling.
fn word<T: for<'de> Deserialize<'de>>(json: &Json, at: &str, name: &str) -> Read<T> {
    let w = text(json, at, name)?;
    serde_json::from_value(serde_json::Value::String(w))
        .map_or_else(|_| bad(&format!("{at}.{name}")), Ok)
}

fn completeness(json: &Json) -> Read<CompletenessFact> {
    let at = "$.completeness";
    let state: Completeness = word(json, at, "state")?;
    let gaps = strings(json, at, "gaps")?;
    let reason = reason(json, at)?;
    if !sorted_distinct(&gaps) {
        return bad(&format!("{at}.gaps"));
    }
    let consistent = match state {
        Completeness::Complete => gaps.is_empty(),
        Completeness::Incomplete => !gaps.is_empty(),
        Completeness::Unknown => reason.value().is_some(),
    };
    if !consistent {
        return bad(at);
    }
    Ok(CompletenessFact {
        state,
        gaps,
        reason,
    })
}

fn attestation(json: &Json) -> Read<Attestation> {
    let at = "$.attestation";
    match (
        json.get("state").and_then(Json::as_str),
        json.get("identity"),
    ) {
        (Some("unknown"), None) if json.get("construction").is_none() => Ok(Attestation {
            identity: Stated::Unknown,
            construction: Stated::Unknown,
        }),
        (Some("known"), Some(Json::String(identity))) => Ok(Attestation {
            identity: Stated::known(identity.clone()),
            construction: Stated::known(text(json, at, "construction")?),
        }),
        _ => bad(at),
    }
}

fn endpoint(json: &Json, at: &str) -> Read<Endpoint> {
    Ok(Endpoint {
        destination: text(json, at, "destination")?,
        protocol: text(json, at, "protocol")?,
        model: stated(member(json, at, "model")?, &format!("{at}.model"))?,
    })
}

fn exchange(json: &Json, at: &str) -> Read<Exchange> {
    let usage = member(json, at, "usage")?;
    let label = text(usage, &format!("{at}.usage"), "label")?;
    let values = match (label.as_str(), usage.get("values")) {
        ("absent", None) => Vec::new(),
        ("provider-testimony", Some(Json::Array(values))) if !values.is_empty() => values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let at = format!("{at}.usage.values[{i}]");
                Ok(UsageValue {
                    provider_field: text(v, &at, "providerField")?,
                    location: text(v, &at, "location")?,
                    unit: stated(member(v, &at, "unit")?, &format!("{at}.unit"))?,
                    value: match member(v, &at, "value")? {
                        Json::Number(text) => number_text(text),
                        _ => return bad(&format!("{at}.value")),
                    },
                })
            })
            .collect::<Read<Vec<_>>>()?,
        _ => return bad(&format!("{at}.usage")),
    };
    let cost = member(json, at, "estimatedCost")?;
    let cost_at = format!("{at}.estimatedCost");
    let estimated_cost = match (
        cost.get("state").and_then(Json::as_str),
        cost.get("label").and_then(Json::as_str),
    ) {
        (Some("not-estimated"), None) => Stated::Absent,
        (None, Some("estimate")) => Stated::known(Estimate {
            label: "estimate".to_string(),
            value: match member(cost, &cost_at, "value")? {
                Json::Number(text) => number_text(text),
                _ => return bad(&format!("{cost_at}.value")),
            },
            currency: text(cost, &cost_at, "currency")?,
            method: text(cost, &cost_at, "method")?,
            inputs: strings(cost, &cost_at, "inputs")?,
        }),
        _ => return bad(&cost_at),
    };
    Ok(Exchange {
        sequence: number(json, at, "sequence")?,
        completeness: text(json, at, "completeness")?,
        record_digest: text(json, at, "recordDigest")?,
        record_byte_length: number(json, at, "recordByteLength")?,
        requested: endpoint(member(json, at, "requested")?, &format!("{at}.requested"))?,
        served: endpoint(member(json, at, "served")?, &format!("{at}.served"))?,
        usage: Usage { label, values },
        estimated_cost,
    })
}
