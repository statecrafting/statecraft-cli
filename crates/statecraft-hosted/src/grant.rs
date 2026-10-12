//! Login by the OAuth device authorization grant (RFC 8628), renewal by the
//! refresh grant, and logout by revocation (spec 038 sections 3.2 and 3.3).

use crate::credential::Credential;
use crate::discovery::Endpoints;
use crate::http::{Body, Http, Reply, Route};
use crate::{Clock, Error, Origin};
use base64::Engine;
use serde::Deserialize;

const DEVICE: Route = Route {
    method: "POST",
    template: "<device authorization endpoint>",
};
const TOKEN: Route = Route {
    method: "POST",
    template: "<token endpoint>",
};
const REVOKE: Route = Route {
    method: "POST",
    template: "/session/token/revoke",
};

/// The device grant's start (RFC 8628 section 3.2).
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct DeviceStart {
    /// The device code; never displayed.
    pub device_code: String,
    /// The code the person enters.
    pub user_code: String,
    /// Where the person goes.
    pub verification_uri: String,
    /// The same, with the code filled in.
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    /// Seconds until the codes expire.
    pub expires_in: u64,
    /// Seconds between polls; 5 when the issuer does not say (RFC 8628).
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

impl std::fmt::Debug for DeviceStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceStart")
            .field("device_code", &"<redacted>")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("expires_in", &self.expires_in)
            .field("interval", &self.interval)
            .finish()
    }
}

fn server_error(route: Route, reply: &Reply) -> Option<Error> {
    (reply.status >= 500).then(|| Error::Failed {
        method: route.method,
        route: route.template,
        why: format!("status {}", reply.status),
    })
}

/// Begin a device grant for `client_id` with exactly `scopes`.
pub fn start(
    http: &Http,
    endpoints: &Endpoints,
    client_id: &str,
    scopes: &[String],
) -> Result<DeviceStart, Error> {
    let scope = scopes.join(" ");
    let reply = http.send(
        DEVICE,
        &endpoints.device_authorization,
        None,
        &[("Accept", "application/json")],
        Body::Form(&[("client_id", client_id), ("scope", &scope)]),
    )?;
    if let Some(e) = server_error(DEVICE, &reply) {
        return Err(e);
    }
    if reply.status != 200 {
        return Err(Error::Refused(format!(
            "the device authorization request was refused ({}{})",
            reply.status,
            oauth_error(&reply.body)
                .map(|e| format!(": {e}"))
                .unwrap_or_default()
        )));
    }
    let start: DeviceStart = serde_json::from_slice(&reply.body).map_err(|_| {
        Error::Refused("the device authorization response is malformed".to_string())
    })?;
    if start.interval == 0 || start.expires_in == 0 {
        return Err(Error::Refused(
            "the device authorization response names a zero interval or lifetime".to_string(),
        ));
    }
    Ok(start)
}

#[derive(Deserialize)]
struct OAuthError {
    error: String,
}

fn oauth_error(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<OAuthError>(body)
        .ok()
        .map(|e| e.error)
        .filter(|e| e.chars().all(|c| c.is_ascii_graphic()))
}

/// Poll the token endpoint at the interval the start carried, adding 5
/// seconds on `slow_down`, until a token arrives or the codes expire.
/// `access_denied` and `expired_token` end the login; neither is retried.
pub fn poll(
    http: &Http,
    endpoints: &Endpoints,
    origin: &Origin,
    client_id: &str,
    start: &DeviceStart,
    clock: &dyn Clock,
) -> Result<Credential, Error> {
    let deadline = clock.now().saturating_add(start.expires_in);
    let mut interval = start.interval;
    loop {
        clock.sleep(interval);
        if clock.now() >= deadline {
            return Err(Error::Refused(
                "the login ended: expired_token (the device code expired)".to_string(),
            ));
        }
        let reply = http.send(
            TOKEN,
            &endpoints.token,
            None,
            &[("Accept", "application/json")],
            Body::Form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &start.device_code),
                ("client_id", client_id),
            ]),
        )?;
        if let Some(e) = server_error(TOKEN, &reply) {
            return Err(e);
        }
        if reply.status == 200 {
            return accept(origin, &reply.body, clock.now());
        }
        match oauth_error(&reply.body).as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval = interval.saturating_add(5),
            Some("access_denied") => {
                return Err(Error::Refused(
                    "the login ended: access_denied (the person declined)".to_string(),
                ));
            }
            Some("expired_token") => {
                return Err(Error::Refused(
                    "the login ended: expired_token (the device code expired)".to_string(),
                ));
            }
            Some(other) => {
                return Err(Error::Refused(format!("the login ended: {other}")));
            }
            None => {
                return Err(Error::Refused(format!(
                    "the token endpoint answered {} without an OAuth error",
                    reply.status
                )));
            }
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Deserialize)]
struct JwtHeader {
    alg: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct JwtClaims {
    aud: Option<Audience>,
}

/// Accept a token response (section 3.2 rule 3): `token_type` is `Bearer`,
/// and the access token is a JWT whose header names `RS256` and whose `aud`
/// contains the origin. The claims are read, the signature is not verified:
/// the cell verifies every token it receives, and these checks only keep
/// this product from storing one the cell will refuse.
pub fn accept(origin: &Origin, body: &[u8], now: u64) -> Result<Credential, Error> {
    let refuse = |why: &str| Error::Refused(format!("the token was not stored: {why}"));
    let t: TokenResponse =
        serde_json::from_slice(body).map_err(|_| refuse("the token response is malformed"))?;
    if !t.token_type.eq_ignore_ascii_case("Bearer") {
        return Err(refuse("its token_type is not Bearer"));
    }
    if t.expires_in == 0 {
        return Err(refuse("its expires_in is zero"));
    }
    let mut parts = t.access_token.split('.');
    let (Some(header), Some(claims), Some(_sig), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(refuse("the access token is not a JWT"));
    };
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let decode = |part: &str| b64.decode(part.trim_end_matches('='));
    let header: JwtHeader = decode(header)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(|| refuse("the access token's header is malformed"))?;
    if header.alg != "RS256" {
        return Err(refuse("the access token is not RS256"));
    }
    let claims: JwtClaims = decode(claims)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(|| refuse("the access token's claims are malformed"))?;
    let audience_ok = match claims.aud {
        Some(Audience::One(a)) => a == origin.as_str(),
        Some(Audience::Many(list)) => list.iter().any(|a| a == origin.as_str()),
        None => false,
    };
    if !audience_ok {
        return Err(refuse("the access token's aud does not contain the origin"));
    }
    Ok(Credential {
        origin: origin.as_str().to_string(),
        access_token: t.access_token,
        refresh_token: t.refresh_token,
        expires_in: t.expires_in,
        issued_at: now,
        scope: t.scope,
    })
}

/// Renew `credential` with its refresh token (section 3.3 rule 3). Refused
/// locally, without presenting the refresh token, before
/// [`Credential::refresh_not_before`].
pub fn refresh(
    http: &Http,
    endpoints: &Endpoints,
    origin: &Origin,
    client_id: &str,
    credential: &Credential,
    clock: &dyn Clock,
) -> Result<Credential, Error> {
    let Some(refresh_token) = credential.refresh_token.as_deref() else {
        return Err(Error::LoginRequired(origin.to_string()));
    };
    let not_before = credential.refresh_not_before();
    if clock.now() < not_before {
        return Err(Error::Refused(format!(
            "a refresh now would be early and would end every linked session; log in again, or wait until {not_before} (seconds since the Unix epoch)"
        )));
    }
    let reply = http.send(
        TOKEN,
        &endpoints.token,
        None,
        &[("Accept", "application/json")],
        Body::Form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ]),
    )?;
    if let Some(e) = server_error(TOKEN, &reply) {
        return Err(e);
    }
    if reply.status != 200 {
        return Err(Error::LoginRequired(origin.to_string()));
    }
    accept(origin, &reply.body, clock.now())
}

/// What a logout's revocation request established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Revocation {
    /// The cell confirmed it.
    Confirmed,
    /// The cell answered and did not confirm it, with this status.
    NotConfirmed(u16),
    /// The cell could not be reached.
    Unreachable,
}

/// Post the access token to the cell's revocation route (section 3.3 rule 4).
pub fn revoke(http: &Http, origin: &Origin, credential: &Credential) -> Revocation {
    match http.send(
        REVOKE,
        &origin.url("/session/token/revoke"),
        Some(&credential.access_token),
        &[],
        Body::Form(&[("token", &credential.access_token)]),
    ) {
        Ok(r) if (200..300).contains(&r.status) => Revocation::Confirmed,
        Ok(r) => Revocation::NotConfirmed(r.status),
        Err(_) => Revocation::Unreachable,
    }
}
