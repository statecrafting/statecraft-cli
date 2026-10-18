//! This provider's half of a witnessed launch (spec 013 sections 3.3 and
//! 3.5): which authority the sidecar captures, the ports it may reach, and
//! how the child is pointed at the one endpoint and the one certificate.
//!
//! Spec 004 claims this crate; spec 013, which is approved, reaches it
//! through its additive `extends` edge. The provider's names live here
//! because this is the one crate allowed to name a provider.

use statecraft_adapter::witness::protocol::Authority;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;

/// The provider's API authority, the one the sidecar intercepts. Every
/// other authority is tunneled unread by the producer.
pub const CAPTURE_AUTHORITY: &str = "api.anthropic.com:443";

/// The producer's provider-family word for this provider.
pub const PROVIDER_FAMILY: &str = "anthropic";

/// The outbound TCP ports the sidecar's confinement allows.
pub const OUTBOUND_PORTS: &[u16] = &[443];

/// The capture allowlist of the `start` message.
#[must_use]
pub fn capture_authorities() -> Vec<Authority> {
    vec![Authority {
        authority: CAPTURE_AUTHORITY.to_string(),
        provider: PROVIDER_FAMILY.to_string(),
        protocols: vec!["http/1.1".to_string(), "sse".to_string()],
    }]
}

/// The child's environment additions: the endpoint as its HTTPS proxy and
/// the certificate as an extra trust anchor, both from the validated ready
/// message. The constructed environment inherits nothing, so no other proxy
/// variable reaches the child.
#[must_use]
pub fn child_variables(endpoint: SocketAddr, certificate: &Path) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("HTTPS_PROXY".to_string(), format!("http://{endpoint}")),
        (
            "NODE_EXTRA_CA_CERTS".to_string(),
            certificate.display().to_string(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_child_is_given_the_endpoint_and_the_certificate_and_nothing_else() {
        let variables = child_variables(
            "127.0.0.1:41999".parse().unwrap(),
            Path::new("/x/witness/capture/ca.pem"),
        );
        assert_eq!(
            variables.keys().collect::<Vec<_>>(),
            ["HTTPS_PROXY", "NODE_EXTRA_CA_CERTS"]
        );
        assert_eq!(variables["HTTPS_PROXY"], "http://127.0.0.1:41999");
        assert!(!OUTBOUND_PORTS.contains(&0));
    }
}
