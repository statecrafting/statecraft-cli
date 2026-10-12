//! Discovery is the cell's (spec 038 section 3.1 rule 2).
//!
//! The client reads `<origin>/.well-known/oauth-protected-resource` (RFC
//! 9728), requires the resource to be the origin and the issuer
//! `<origin>/auth/v1/` to be among its authorization servers, then reads that
//! issuer's discovery document and requires it to name the same issuer.
//! Every endpoint it names must be on the origin: a token is never posted to a
//! host the cell's own documents point elsewhere. Any mismatch refuses before
//! a grant starts.

use crate::http::{Body, Http, Route};
use crate::{Error, Origin};
use serde::Deserialize;

/// The endpoints a login and a refresh use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// The issuer, `<origin>/auth/v1/`.
    pub issuer: String,
    /// The device authorization endpoint.
    pub device_authorization: String,
    /// The token endpoint.
    pub token: String,
}

const RESOURCE: Route = Route {
    method: "GET",
    template: "/.well-known/oauth-protected-resource",
};
const CONFIGURATION: Route = Route {
    method: "GET",
    template: "<issuer>/.well-known/openid-configuration",
};

#[derive(Deserialize)]
struct ResourceMetadata {
    resource: String,
    #[serde(default)]
    authorization_servers: Vec<String>,
}

#[derive(Deserialize)]
struct Configuration {
    issuer: String,
    device_authorization_endpoint: Option<String>,
    token_endpoint: Option<String>,
}

/// Discover `origin`'s endpoints.
pub fn discover(http: &Http, origin: &Origin) -> Result<Endpoints, Error> {
    let refuse = |why: String| Error::Refused(format!("discovery at {origin}: {why}"));
    let reply = http.send(
        RESOURCE,
        &origin.url("/.well-known/oauth-protected-resource"),
        None,
        &[("Accept", "application/json")],
        Body::None,
    )?;
    if reply.status >= 500 {
        return Err(Error::Failed {
            method: RESOURCE.method,
            route: RESOURCE.template,
            why: format!("status {}", reply.status),
        });
    }
    if reply.status != 200 {
        return Err(refuse(format!(
            "the protected-resource metadata answered {}",
            reply.status
        )));
    }
    let meta: ResourceMetadata = serde_json::from_slice(&reply.body)
        .map_err(|e| refuse(format!("the protected-resource metadata is malformed: {e}")))?;
    let resource = meta.resource.strip_suffix('/').unwrap_or(&meta.resource);
    if resource != origin.as_str() {
        return Err(refuse(format!(
            "the resource identifier is `{}`, not the origin",
            meta.resource
        )));
    }
    let issuer = origin.issuer();
    if !meta.authorization_servers.contains(&issuer) {
        return Err(refuse(format!(
            "the authorization servers {:?} do not include the issuer {issuer}",
            meta.authorization_servers
        )));
    }
    let reply = http.send(
        CONFIGURATION,
        &format!("{issuer}.well-known/openid-configuration"),
        None,
        &[("Accept", "application/json")],
        Body::None,
    )?;
    if reply.status >= 500 {
        return Err(Error::Failed {
            method: CONFIGURATION.method,
            route: CONFIGURATION.template,
            why: format!("status {}", reply.status),
        });
    }
    if reply.status != 200 {
        return Err(refuse(format!(
            "the issuer's discovery document answered {}",
            reply.status
        )));
    }
    let config: Configuration = serde_json::from_slice(&reply.body)
        .map_err(|e| refuse(format!("the issuer's discovery document is malformed: {e}")))?;
    if config.issuer != issuer {
        return Err(refuse(format!(
            "the discovery document names the issuer `{}`, not {issuer}",
            config.issuer
        )));
    }
    let on_origin = |name: &str, url: Option<String>| -> Result<String, Error> {
        let url = url.ok_or_else(|| refuse(format!("the discovery document names no {name}")))?;
        if !origin.owns(&url) {
            return Err(refuse(format!("the {name} `{url}` is not on the origin")));
        }
        Ok(url)
    };
    Ok(Endpoints {
        device_authorization: on_origin(
            "device authorization endpoint",
            config.device_authorization_endpoint,
        )?,
        token: on_origin("token endpoint", config.token_endpoint)?,
        issuer,
    })
}
