//! The one HTTP client (spec 038 section 3.4): blocking, no redirects, no
//! cookies, and errors that name a request by method and route template.
//!
//! Redirects are not followed: a `3xx` is an answer like any other, so a
//! bearer token is never carried to a location the cell's response chose.

use crate::Error;
use std::time::Duration;

/// The largest response body read, in bytes.
pub const MAX_BODY: u64 = 64 << 20;

/// A response: its status, its `WWW-Authenticate` header and its body.
#[derive(Clone, PartialEq, Eq)]
pub struct Reply {
    /// The HTTP status.
    pub status: u16,
    /// `WWW-Authenticate`, when present.
    pub www_authenticate: Option<String>,
    /// The body bytes.
    pub body: Vec<u8>,
}

impl std::fmt::Debug for Reply {
    // A body can carry a token (a token response does), so it is never
    // rendered.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reply")
            .field("status", &self.status)
            .field("www_authenticate", &self.www_authenticate)
            .field("body", &format_args!("<{} bytes>", self.body.len()))
            .finish()
    }
}

/// One request's identity, for errors: never the URL.
#[derive(Debug, Clone, Copy)]
pub struct Route {
    /// The method.
    pub method: &'static str,
    /// The route template.
    pub template: &'static str,
}

/// The body a request sends.
pub enum Body<'a> {
    /// No body.
    None,
    /// `application/x-www-form-urlencoded`.
    Form(&'a [(&'a str, &'a str)]),
    /// A body with its content type.
    Bytes {
        /// The content type.
        content_type: &'a str,
        /// The bytes.
        bytes: &'a [u8],
    },
}

/// The client.
pub struct Http {
    agent: ureq::Agent,
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

impl Http {
    /// A client with a 60-second overall timeout per request.
    pub fn new() -> Http {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .save_redirect_history(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .user_agent(concat!("statecraft-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Http { agent }
    }

    /// Send one request. `bearer` adds `Authorization: Bearer`; `headers`
    /// adds the rest. Never a cookie.
    pub fn send(
        &self,
        route: Route,
        url: &str,
        bearer: Option<&str>,
        headers: &[(&str, &str)],
        body: Body<'_>,
    ) -> Result<Reply, Error> {
        let failed = |why: String| Error::Failed {
            method: route.method,
            route: route.template,
            why,
        };
        // A ureq error renders the URL; only its kind is kept.
        let describe = |e: ureq::Error| -> String {
            match e {
                ureq::Error::Io(io) => format!("transport: {}", io.kind()),
                ureq::Error::Timeout(_) => "timed out".to_string(),
                ureq::Error::HostNotFound => "host not found".to_string(),
                ureq::Error::ConnectionFailed => "connection failed".to_string(),
                ureq::Error::Tls(_) | ureq::Error::Rustls(_) => "TLS failure".to_string(),
                ureq::Error::BodyExceedsLimit(_) => "response too large".to_string(),
                _ => "request failed".to_string(),
            }
        };
        let auth = bearer.map(|t| format!("Bearer {t}"));
        let result = match route.method {
            "GET" => {
                let mut req = self.agent.get(url);
                if let Some(a) = &auth {
                    req = req.header("Authorization", a.as_str());
                }
                for (k, v) in headers {
                    req = req.header(*k, *v);
                }
                req.call()
            }
            "POST" => {
                let mut req = self.agent.post(url);
                if let Some(a) = &auth {
                    req = req.header("Authorization", a.as_str());
                }
                for (k, v) in headers {
                    req = req.header(*k, *v);
                }
                match body {
                    Body::None => req.send_empty(),
                    Body::Form(fields) => req.send_form(fields.iter().copied()),
                    Body::Bytes {
                        content_type,
                        bytes,
                    } => req.header("Content-Type", content_type).send(bytes),
                }
            }
            other => return Err(failed(format!("unsupported method {other}"))),
        };
        let response = result.map_err(|e| failed(describe(e)))?;
        let status = response.status().as_u16();
        let www_authenticate = response
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = response
            .into_body()
            .with_config()
            .limit(MAX_BODY)
            .read_to_vec()
            .map_err(|e| failed(describe(e)))?;
        Ok(Reply {
            status,
            www_authenticate,
            body,
        })
    }
}
