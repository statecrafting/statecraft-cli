//! The gateway route this provider honours (spec 051 section 3.2).
//!
//! Spec 051 moves the operator's provider token out of the child: the
//! supervisor holds it behind a loopback gateway, and the child is given the
//! gateway's address and a token minted for its attempt. The supervisor needs
//! three facts from the adapter to do that, and they are this provider's, so
//! they are stated here rather than in the provider-neutral seam:
//!
//! 1. the variable the provider reads its base URL from;
//! 2. the variable it reads its OAuth token from;
//! 3. the one authority the gateway forwards to.
//!
//! Measured on 2026-10-21 against Claude Code 2.1.307, `claude -p` on macOS,
//! unconfined (spec 051 section 5): with [`ROUTE`]'s two variables set, the
//! provider sent `HEAD /api/hello` and then `POST /v1/messages` to the
//! loopback address, each `POST` with `Authorization: Bearer` carrying
//! exactly the token it was given and no `x-api-key`.

/// How a provider is routed through the supervisor's gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    /// The variable the provider reads its API base URL from.
    pub base_url_variable: &'static str,
    /// The variable the provider reads its bearer token from.
    pub token_variable: &'static str,
    /// The provider authority's host, sent as `Host` and verified by TLS.
    pub host: &'static str,
    /// The provider authority's TLS port.
    pub port: u16,
    /// The preflight the provider sends before its first request, which the
    /// gateway answers itself (spec 051 section 5).
    pub preflight: (&'static str, &'static str),
}

/// This adapter's gateway route.
pub const ROUTE: Route = Route {
    base_url_variable: "ANTHROPIC_BASE_URL",
    token_variable: "CLAUDE_CODE_OAUTH_TOKEN",
    host: "api.anthropic.com",
    port: 443,
    preflight: ("HEAD", "/api/hello"),
};

/// The route this adapter declares. `None` would refuse every unwitnessed
/// authenticated launch (spec 051 section 3.4), so it is a function and not
/// only the constant: a provider version that stops honouring the override
/// is withdrawn here, in one place.
#[must_use]
pub fn route() -> Option<Route> {
    Some(ROUTE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_route_names_the_measured_variables_and_one_tls_authority() {
        let route = route().expect("the adapter declares a route");
        assert_eq!(route.base_url_variable, "ANTHROPIC_BASE_URL");
        assert_eq!(route.token_variable, "CLAUDE_CODE_OAUTH_TOKEN");
        assert_eq!((route.host, route.port), ("api.anthropic.com", 443));
        assert_eq!(route.preflight, ("HEAD", "/api/hello"));
    }
}
