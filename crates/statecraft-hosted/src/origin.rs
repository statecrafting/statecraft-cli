//! The one cell an operator names, by its origin (spec 038 section 3.1 rule
//! 1): `https://<host>[:port]`, no path, no userinfo, no query.
//!
//! The origin is never inferred: it comes from the project's committed
//! declaration or from `--cell`, never from a Git remote, an environment
//! variable or a response. Plain `http` is admitted for a literal loopback
//! address only (`127.0.0.1` or `[::1]`), as RFC 8252 treats a loopback
//! redirect: such a request never leaves the machine, and it is how the
//! acceptance tests stand up a cell without a certificate authority.

use crate::Error;

/// A validated cell origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    text: String,
}

impl Origin {
    /// Parse and validate `s`. A single trailing `/` is accepted and dropped.
    pub fn parse(s: &str) -> Result<Origin, Error> {
        let refuse = |why: &str| Error::Refused(format!("the cell origin `{s}` {why}"));
        let s = s.strip_suffix('/').unwrap_or(s);
        let (scheme, authority) = if let Some(rest) = s.strip_prefix("https://") {
            ("https", rest)
        } else if let Some(rest) = s.strip_prefix("http://") {
            ("http", rest)
        } else {
            return Err(refuse("is not an https origin"));
        };
        if authority.is_empty() {
            return Err(refuse("names no host"));
        }
        if authority.contains(['/', '?', '#']) {
            return Err(refuse("carries a path, a query or a fragment"));
        }
        if authority.contains('@') {
            return Err(refuse("carries userinfo"));
        }
        let (host, port) = split_port(authority).ok_or_else(|| refuse("has an invalid port"))?;
        let host = host.to_ascii_lowercase();
        let valid_host = if let Some(inner) = host.strip_prefix('[') {
            inner
                .strip_suffix(']')
                .is_some_and(|ip| ip.parse::<std::net::Ipv6Addr>().is_ok())
        } else {
            !host.is_empty()
                && !host.starts_with(['.', '-'])
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        };
        if !valid_host {
            return Err(refuse("names an invalid host"));
        }
        if scheme == "http" && host != "127.0.0.1" && host != "[::1]" {
            return Err(refuse(
                "is plain http; only https, or http to a literal loopback address, is admitted",
            ));
        }
        let text = match port {
            Some(p) => format!("{scheme}://{host}:{p}"),
            None => format!("{scheme}://{host}"),
        };
        Ok(Origin { text })
    }

    /// The origin as text, `scheme://host[:port]`.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The issuer this cell must name (section 3.1 rule 2), trailing slash
    /// included.
    pub fn issuer(&self) -> String {
        format!("{}/auth/v1/", self.text)
    }

    /// An absolute URL on this origin; `path` begins with `/`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.text)
    }

    /// Whether `url` is on this origin.
    pub fn owns(&self, url: &str) -> bool {
        url.strip_prefix(&self.text)
            .is_some_and(|rest| rest.starts_with('/'))
    }

    /// The key the credential is held under: SHA-256 of the origin text.
    pub fn digest(&self) -> String {
        crate::sha256_hex(self.text.as_bytes())
    }
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// `host[:port]`, with a bracketed IPv6 host. `None` for a bad port.
fn split_port(authority: &str) -> Option<(&str, Option<u16>)> {
    let (host, port) = if authority.starts_with('[') {
        match authority.find(']') {
            Some(end) => {
                let rest = &authority[end + 1..];
                if rest.is_empty() {
                    (authority, None)
                } else {
                    (&authority[..=end], Some(rest.strip_prefix(':')?))
                }
            }
            None => (authority, None),
        }
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (authority, None),
        }
    };
    match port {
        None => Some((host, None)),
        Some(p) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => {
            let n: u16 = p.parse().ok()?;
            (n != 0).then_some((host, Some(n)))
        }
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_origin_is_https_with_no_path_userinfo_or_query() {
        for ok in [
            "https://cell.example",
            "https://cell.example/",
            "https://Cell.Example:8443",
            "http://127.0.0.1:9000",
            "http://[::1]:9000",
            "https://[2001:db8::1]",
        ] {
            assert!(Origin::parse(ok).is_ok(), "{ok}");
        }
        for bad in [
            "cell.example",
            "ftp://cell.example",
            "https://",
            "https://cell.example/api",
            "https://cell.example?x=1",
            "https://user@cell.example",
            "https://cell.example:0",
            "https://cell.example:99999",
            "https://cell.example:",
            "http://cell.example",
            "http://localhost:9000",
            "https://-cell.example",
            "https://cell_example",
        ] {
            assert!(Origin::parse(bad).is_err(), "{bad}");
        }
        let o = Origin::parse("https://Cell.Example:8443/").unwrap();
        assert_eq!(o.as_str(), "https://cell.example:8443");
        assert_eq!(o.issuer(), "https://cell.example:8443/auth/v1/");
        assert!(o.owns("https://cell.example:8443/api/x"));
        assert!(!o.owns("https://cell.example:8443.evil/api"));
        assert!(!o.owns("https://cell.example:8443"));
    }
}
