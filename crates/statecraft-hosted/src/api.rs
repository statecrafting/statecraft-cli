//! The request rules every hosted verb follows (spec 038 section 3.4), and
//! the three cell routes the verbs use: revision registration, evidence
//! submission and decision retrieval (sections 3.5 and 3.6).

use crate::credential::Credential;
use crate::http::{Body, Http, Reply, Route};
use crate::{Error, Origin};
use serde::Deserialize;
use serde_json::Value;

const REGISTER: Route = Route {
    method: "POST",
    template: "/api/repositories/{id}/revisions",
};
const SUBMIT: Route = Route {
    method: "POST",
    template: "/api/evidence/submissions",
};
const DECISION: Route = Route {
    method: "GET",
    template: "/api/repositories/{id}/revisions/{revisionId}/decision",
};

/// Apply section 3.4 to a reply: `2xx` passes; `401` asks for a login; `403`
/// with `insufficient_scope` is final and names the scope; any other `4xx`
/// refuses and is not retried; a `5xx` failed.
pub fn classify(origin: &Origin, route: Route, reply: Reply) -> Result<Reply, Error> {
    match reply.status {
        200..=299 => Ok(reply),
        401 => Err(Error::LoginRequired(origin.to_string())),
        403 => {
            let header = reply.www_authenticate.as_deref().unwrap_or("");
            if header.contains("insufficient_scope") {
                let scope = header
                    .split("scope=\"")
                    .nth(1)
                    .and_then(|s| s.split('"').next())
                    .unwrap_or("(unnamed)");
                Err(Error::InsufficientScope(scope.to_string()))
            } else {
                Err(Error::Refused(format!(
                    "{} {} was refused (403)",
                    route.method, route.template
                )))
            }
        }
        400..=499 => Err(Error::Refused(format!(
            "{} {} was refused ({})",
            route.method, route.template, reply.status
        ))),
        status => Err(Error::Failed {
            method: route.method,
            route: route.template,
            why: format!("status {status}"),
        }),
    }
}

/// A registered revision, and whether this request registered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    /// The `RevisionId`, lowercase hexadecimal.
    pub revision: String,
    /// `true` when the cell already held it (`200`), `false` when this request
    /// registered it (`201`).
    pub existed: bool,
}

#[derive(Deserialize)]
struct RegisteredBody {
    revision: String,
}

/// Register `commit` (with `tree` and `base` where known) under `repository`
/// (section 3.5 rule 3). A repeat returns the existing `RevisionId`, which is
/// success.
pub fn register_revision(
    http: &Http,
    origin: &Origin,
    credential: &Credential,
    repository: &str,
    commit: &str,
    tree: Option<&str>,
    base: Option<&str>,
) -> Result<Registered, Error> {
    require_id("repository", repository)?;
    let mut body = serde_json::json!({ "commit": commit });
    if let Some(t) = tree {
        body["tree"] = t.into();
    }
    if let Some(b) = base {
        body["base"] = b.into();
    }
    let bytes = serde_json::to_vec(&body).expect("serializable");
    let reply = http.send(
        REGISTER,
        &origin.url(&format!("/api/repositories/{repository}/revisions")),
        Some(&credential.access_token),
        &[("Accept", "application/json")],
        Body::Bytes {
            content_type: "application/json",
            bytes: &bytes,
        },
    )?;
    let reply = classify(origin, REGISTER, reply)?;
    let parsed: RegisteredBody = serde_json::from_slice(&reply.body)
        .map_err(|_| Error::Refused("the registration answer names no revision".to_string()))?;
    if !is_hex64(&parsed.revision) {
        return Err(Error::Refused(
            "the registration answer's revision is not a 64-character hexadecimal id".to_string(),
        ));
    }
    Ok(Registered {
        revision: parsed.revision,
        existed: reply.status == 200,
    })
}

/// Refuse an id that is not a lowercase 64-character hexadecimal hash before
/// it is placed in a request path, so no caller can steer the URL.
fn require_id(what: &str, id: &str) -> Result<(), Error> {
    if is_hex64(id) {
        Ok(())
    } else {
        Err(Error::Refused(format!(
            "the {what} id is not a lowercase 64-character hexadecimal hash"
        )))
    }
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The idempotency key of a submission (section 3.5 rule 4): a digest over
/// the repository id, the `RevisionId` and the submission field's bytes,
/// which carry the manifest. The same submission always has the same key.
pub fn idempotency_key(repository: &str, revision: &str, submission: &[u8]) -> String {
    let mut input = b"statecraft-hosted-submission/1\0".to_vec();
    input.extend_from_slice(repository.as_bytes());
    input.push(0);
    input.extend_from_slice(revision.as_bytes());
    input.push(0);
    input.extend_from_slice(submission);
    crate::sha256_hex(&input)
}

/// What a submission answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Submitted {
    /// The HTTP status.
    pub status: u16,
    /// The answer, as the platform returned it.
    pub answer: Value,
    /// The key the request carried.
    pub idempotency_key: String,
    /// SHA-256 of the answer's bytes.
    pub response_digest: String,
}

/// Submit the `submission` field (repository, revision and manifest, as
/// JSON) and each artifact's bytes in manifest order, as one multipart body
/// (platform 003 B-21).
pub fn submit(
    http: &Http,
    origin: &Origin,
    credential: &Credential,
    repository: &str,
    revision: &str,
    submission: &[u8],
    artifacts: &[Vec<u8>],
) -> Result<Submitted, Error> {
    require_id("repository", repository)?;
    require_id("revision", revision)?;
    let key = idempotency_key(repository, revision, submission);
    let (content_type, body) = multipart(submission, artifacts);
    let reply = http.send(
        SUBMIT,
        &origin.url("/api/evidence/submissions"),
        Some(&credential.access_token),
        &[("Accept", "application/json"), ("Idempotency-Key", &key)],
        Body::Bytes {
            content_type: &content_type,
            bytes: &body,
        },
    )?;
    let reply = classify(origin, SUBMIT, reply)?;
    let answer: Value = serde_json::from_slice(&reply.body)
        .map_err(|_| Error::Refused("the submission answer is not JSON".to_string()))?;
    Ok(Submitted {
        status: reply.status,
        answer,
        idempotency_key: key,
        response_digest: crate::sha256_hex(&reply.body),
    })
}

/// A `multipart/form-data` body: one `submission` field, then one
/// `artifact` field per artifact. The boundary is derived from the content
/// and checked against it, so the body is a function of its parts.
pub fn multipart(submission: &[u8], artifacts: &[Vec<u8>]) -> (String, Vec<u8>) {
    let mut seed = submission.to_vec();
    for a in artifacts {
        seed.extend_from_slice(crate::sha256_hex(a).as_bytes());
    }
    let digest = crate::sha256_hex(&seed);
    let mut n = 0u32;
    let boundary = loop {
        let candidate = format!("statecraft-{}-{n}", &digest[..32]);
        let needle = candidate.as_bytes();
        let occurs = |hay: &[u8]| hay.windows(needle.len()).any(|w| w == needle);
        if !occurs(submission) && !artifacts.iter().any(|a| occurs(a)) {
            break candidate;
        }
        n += 1;
    };
    let mut body = Vec::new();
    let mut part = |name: &str, content_type: &str, bytes: &[u8]| {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n").as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    };
    part("submission", "application/json", submission);
    for a in artifacts {
        part("artifact", "application/octet-stream", a);
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// What a decision retrieval returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retrieved {
    /// The revision is not registered (`404`).
    NotRegistered,
    /// The bundle's bytes, exactly as the cell sent them.
    Bundle(Vec<u8>),
}

/// Retrieve the decision bundle for `revision` (section 3.6 rule 1).
pub fn decision(
    http: &Http,
    origin: &Origin,
    credential: &Credential,
    repository: &str,
    revision: &str,
) -> Result<Retrieved, Error> {
    require_id("repository", repository)?;
    require_id("revision", revision)?;
    let reply = http.send(
        DECISION,
        &origin.url(&format!(
            "/api/repositories/{repository}/revisions/{revision}/decision"
        )),
        Some(&credential.access_token),
        &[("Accept", "application/json")],
        Body::None,
    )?;
    if reply.status == 404 {
        return Ok(Retrieved::NotRegistered);
    }
    let reply = classify(origin, DECISION, reply)?;
    Ok(Retrieved::Bundle(reply.body))
}
