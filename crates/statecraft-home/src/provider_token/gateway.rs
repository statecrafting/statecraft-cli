//! The loopback authentication gateway (spec 051).
//!
//! For an unwitnessed authenticated attempt the supervisor binds a gateway on
//! a loopback port before the child starts (section 3.1). The child is given
//! the gateway's address and a token minted for its attempt (section 3.2);
//! the gateway checks that token, replaces it with the operator's token read
//! from spec 042's store, and forwards to the one provider authority the
//! adapter names. It answers anything else with 401 and forwards nothing, and
//! once the attempt has an outcome it drops the attempt's token, so a copy
//! the child made is refused.
//!
//! The operator's token is held in this process's memory only. The gateway
//! writes it to no file, no record and no answer; the one file it writes is
//! the attempt's token, owner-only and no-replace, in the attempt's per-run
//! configuration directory.
//!
//! The gateway is not a proxy. It accepts origin-form requests only (no
//! `CONNECT`, no absolute URI), forwards one request per connection with
//! `Connection: close` on both sides, and its destination is the
//! [`Upstream`] it was started with, never a header the child sent.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use statecraft_adapter_claude_code::gateway::Route;
use statecraft_secret::SecretString;

/// The attempt token's file name inside the per-run configuration directory.
pub const TOKEN_FILE: &str = "gateway-token";

/// A prefix that names a leaked attempt token for what it is.
const TOKEN_PREFIX: &str = "statecraft-attempt-";

/// The largest request or response head accepted, in bytes.
const MAX_HEAD: usize = 64 * 1024;

/// The largest single head line or chunk-size line, in bytes.
const MAX_LINE: usize = 8 * 1024;

/// How long the gateway waits on the child between bytes of a request.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long an [`Upstream`] connection waits on the provider between bytes
/// of a response. A streamed response can be quiet for a long time while the
/// model works.
pub const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(600);

/// Request headers that are the gateway's to set, or are hop-by-hop, and are
/// never forwarded as the child sent them.
const DROPPED: &[&str] = &[
    "host",
    "authorization",
    "x-api-key",
    "proxy-authorization",
    "proxy-connection",
    "connection",
    "keep-alive",
    "te",
    "upgrade",
    "content-length",
    "transfer-encoding",
];

/// A connection to the provider authority.
pub trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

/// Where the gateway forwards: the one authority its adapter names.
pub trait Upstream: Send + Sync {
    /// The authority's host, sent as `Host`.
    fn host(&self) -> &str;
    /// A fresh connection to the authority, over TLS in production.
    fn connect(&self) -> io::Result<Box<dyn Stream>>;
}

/// The production upstream for `route`: TLS to `route.host:route.port`,
/// verified against the bundled web PKI roots with the `ring` provider (spec
/// 051 section 5, 2026-11-02). No system trust store is read, so what the
/// gateway trusts does not depend on the host it runs on.
#[must_use]
pub fn provider_upstream(route: &Route) -> Arc<dyn Upstream> {
    Arc::new(Tls {
        host: route.host,
        port: route.port,
        config: tls_config(),
    })
}

fn tls_config() -> Option<Arc<rustls::ClientConfig>> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .ok()
        .map(|builder| Arc::new(builder.with_root_certificates(roots).with_no_client_auth()))
}

struct Tls {
    host: &'static str,
    port: u16,
    /// `None` only if the provider offers no safe protocol version, which
    /// the bundled `ring` provider always does; every forward then fails.
    config: Option<Arc<rustls::ClientConfig>>,
}

impl Upstream for Tls {
    fn host(&self) -> &str {
        self.host
    }
    fn connect(&self) -> io::Result<Box<dyn Stream>> {
        let config = self
            .config
            .clone()
            .ok_or_else(|| io::Error::other("no TLS protocol version is available"))?;
        let name = rustls::pki_types::ServerName::try_from(self.host)
            .map_err(io::Error::other)?
            .to_owned();
        let connection = rustls::ClientConnection::new(config, name).map_err(io::Error::other)?;
        let tcp = TcpStream::connect((self.host, self.port))?;
        tcp.set_read_timeout(Some(UPSTREAM_TIMEOUT))?;
        tcp.set_write_timeout(Some(UPSTREAM_TIMEOUT))?;
        Ok(Box::new(rustls::StreamOwned::new(connection, tcp)))
    }
}

/// A token held in memory only, as spec 046's one secret type: redacted when
/// shown, zeroed when dropped, and read only through `expose`.
#[derive(Debug)]
struct Held(SecretString);

impl Held {
    fn new(value: SecretString) -> Self {
        Self(value)
    }
    fn matches(&self, presented: &[u8]) -> bool {
        let held = self.0.expose().as_bytes();
        // Constant time in the presented token's content.
        held.len() == presented.len()
            && held
                .iter()
                .zip(presented)
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    }
}

/// Why a gateway could not be opened. Never carries a token.
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    /// The listener could not be bound, or is not on loopback.
    #[error("the provider gateway could not bind a loopback endpoint: {0}")]
    Bind(String),
    /// The attempt token could not be minted or written.
    #[error("the provider gateway's attempt token could not be written: {0}")]
    Token(String),
}

struct State {
    preflight: (&'static str, &'static str),
    attempt: Mutex<Option<Held>>,
    operator: Held,
    upstream: Arc<dyn Upstream>,
    stopping: AtomicBool,
}

impl State {
    fn admits(&self, presented: Option<&[u8]>) -> bool {
        let Some(presented) = presented else {
            return false;
        };
        self.attempt
            .lock()
            .map(|held| held.as_ref().is_some_and(|t| t.matches(presented)))
            .unwrap_or(false)
    }
}

/// A running gateway for one attempt. Dropping it revokes the attempt's
/// token, stops accepting and waits for the acceptor.
pub struct Gateway {
    address: SocketAddr,
    attempt_token: SecretString,
    token_file: PathBuf,
    state: Arc<State>,
    acceptor: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl Gateway {
    /// Open a gateway for one attempt on a fresh `127.0.0.1` port: bind
    /// first, then mint the attempt's token and write it, owner-only and
    /// no-replace, as [`TOKEN_FILE`] in `configuration`, then accept. A bind
    /// that fails writes nothing.
    pub fn open(
        route: &Route,
        operator: SecretString,
        configuration: &Path,
        upstream: Arc<dyn Upstream>,
    ) -> Result<Self, GatewayError> {
        Self::open_at(loopback(), route, operator, configuration, upstream)
    }

    /// [`Gateway::open`] at an exact address, which must be loopback.
    pub fn open_at(
        address: SocketAddr,
        route: &Route,
        operator: SecretString,
        configuration: &Path,
        upstream: Arc<dyn Upstream>,
    ) -> Result<Self, GatewayError> {
        let operator = Held::new(operator);
        if !address.ip().is_loopback() {
            return Err(GatewayError::Bind(format!("{address} is not loopback")));
        }
        let listener =
            TcpListener::bind(address).map_err(|e| GatewayError::Bind(e.kind().to_string()))?;
        let address = listener
            .local_addr()
            .map_err(|e| GatewayError::Bind(e.kind().to_string()))?;
        let attempt_token =
            SecretString::new(mint().map_err(|e| GatewayError::Token(e.kind().to_string()))?);
        let token_file = configuration.join(TOKEN_FILE);
        write_token_file(&token_file, attempt_token.expose())
            .map_err(|e| GatewayError::Token(e.kind().to_string()))?;
        let state = Arc::new(State {
            preflight: route.preflight,
            attempt: Mutex::new(Some(Held::new(attempt_token.clone()))),
            operator,
            upstream,
            stopping: AtomicBool::new(false),
        });
        let accepting = Arc::clone(&state);
        let acceptor = std::thread::Builder::new()
            .name("statecraft-provider-gateway".into())
            .spawn(move || accept(&listener, &accepting))
            .map_err(|e| GatewayError::Bind(e.kind().to_string()))?;
        Ok(Self {
            address,
            attempt_token,
            token_file,
            state,
            acceptor: Some(acceptor),
        })
    }

    /// The loopback endpoint the child's confinement grants.
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The base URL the provider is pointed at.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    /// The attempt's token: the child's, and worth nothing once revoked.
    #[must_use]
    pub fn attempt_token(&self) -> &str {
        self.attempt_token.expose()
    }

    /// The owner-only file the attempt's token was written to.
    #[must_use]
    pub fn token_file(&self) -> &Path {
        &self.token_file
    }

    /// The attempt has an outcome: refuse its token from now on. Idempotent.
    pub fn revoke(&self) {
        if let Ok(mut held) = self.state.attempt.lock() {
            *held = None;
        }
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.revoke();
        self.state.stopping.store(true, Ordering::SeqCst);
        // Wake the acceptor, which then sees `stopping` and returns.
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        if let Some(acceptor) = self.acceptor.take() {
            let _ = acceptor.join();
        }
    }
}

/// A random attempt token: 32 bytes from the system's random source, hex,
/// drawn as two of the launch's nonces. In a debug build given
/// `STATECRAFT_TEST_NONCE_SEED` they come from that sequence, so spec 040's
/// differential tests see the same token on both sides; a release build
/// never reads the variable.
fn mint() -> io::Result<String> {
    Ok(format!(
        "{TOKEN_PREFIX}{}{}",
        crate::launch::fresh_nonce()?,
        crate::launch::fresh_nonce()?
    ))
}

/// Where [`Gateway::open`] binds: a fresh `127.0.0.1` port, or in a debug
/// build the port `STATECRAFT_TEST_GATEWAY_PORT` names, so spec 040's
/// differential tests see the same endpoint, and so the same confinement
/// digest, on both sides. A release build never reads the variable.
fn loopback() -> SocketAddr {
    #[cfg(debug_assertions)]
    if let Some(port) = std::env::var("STATECRAFT_TEST_GATEWAY_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
    {
        return SocketAddr::from(([127, 0, 0, 1], port));
    }
    SocketAddr::from(([127, 0, 0, 1], 0))
}

fn write_token_file(path: &Path, token: &str) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.write_all(token.as_bytes())?;
    file.sync_all()
}

fn accept(listener: &TcpListener, state: &Arc<State>) {
    for connection in listener.incoming() {
        if state.stopping.load(Ordering::SeqCst) {
            return;
        }
        let Ok(connection) = connection else {
            continue;
        };
        let serving = Arc::clone(state);
        let _ = std::thread::Builder::new()
            .name("statecraft-provider-gateway-connection".into())
            .spawn(move || {
                let _ = serve(connection, &serving);
            });
    }
}

/// How a request's body is delimited.
#[derive(Debug, PartialEq, Eq)]
enum Framing {
    None,
    Length(u64),
    Chunked,
}

/// A parsed request head. Header names are kept as sent. Deliberately not
/// `Debug`: its headers carry the attempt's token.
struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    framing: Framing,
}

impl Request {
    fn parse(head: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(head).ok()?;
        let mut lines = text.split("\r\n");
        let mut start = lines.next()?.split(' ');
        let (method, target, version) = (start.next()?, start.next()?, start.next()?);
        if start.next().is_some()
            || !matches!(version, "HTTP/1.1" | "HTTP/1.0")
            || method.is_empty()
            || !method.bytes().all(|b| b.is_ascii_uppercase())
            || method == "CONNECT"
            || !target.starts_with('/')
            || target.starts_with("//")
            || !target.bytes().all(|b| b.is_ascii_graphic())
        {
            return None;
        }
        let mut headers = Vec::new();
        for line in lines.filter(|line| !line.is_empty()) {
            if line.starts_with([' ', '\t']) {
                return None;
            }
            let (name, value) = line.split_once(':')?;
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_graphic()) {
                return None;
            }
            // A field value is never forwarded with a control byte: a bare CR
            // or LF in it would end the line upstream and add a header the
            // gateway never read (RFC 9110 section 5.5).
            if value.bytes().any(|b| (b < 0x20 && b != b'\t') || b == 0x7f) {
                return None;
            }
            headers.push((name.to_string(), value.trim().to_string()));
        }
        let lengths: Vec<&str> = values(&headers, "content-length").collect();
        let encodings: Vec<&str> = values(&headers, "transfer-encoding").collect();
        let framing = match (lengths.as_slice(), encodings.as_slice()) {
            ([], []) => Framing::None,
            ([], [encoding]) if encoding.eq_ignore_ascii_case("chunked") => Framing::Chunked,
            (lengths, []) if lengths.windows(2).all(|w| w[0] == w[1]) => {
                Framing::Length(lengths[0].parse().ok()?)
            }
            _ => return None,
        };
        Some(Self {
            method: method.into(),
            target: target.into(),
            headers,
            framing,
        })
    }

    /// The bearer token presented, if exactly one `Authorization` header
    /// carries one.
    fn bearer(&self) -> Option<&[u8]> {
        let mut found = values(&self.headers, "authorization");
        let value = found.next()?;
        if found.next().is_some() {
            return None;
        }
        let (scheme, token) = value.split_once(' ')?;
        scheme
            .eq_ignore_ascii_case("bearer")
            .then(|| token.trim().as_bytes())
    }

    /// The head sent upstream: the child's headers less [`DROPPED`], the
    /// authority's `Host`, the operator's token and `Connection: close`.
    fn forwarded(&self, host: &str, operator: &Held) -> Vec<u8> {
        let mut head = format!(
            "{} {} HTTP/1.1\r\nHost: {host}\r\n",
            self.method, self.target
        )
        .into_bytes();
        for (name, value) in &self.headers {
            if !DROPPED.iter().any(|d| name.eq_ignore_ascii_case(d)) {
                head.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
            }
        }
        head.extend_from_slice(b"Authorization: Bearer ");
        head.extend_from_slice(operator.0.expose().as_bytes());
        head.extend_from_slice(b"\r\n");
        match self.framing {
            Framing::None => {}
            Framing::Length(n) => {
                head.extend_from_slice(format!("Content-Length: {n}\r\n").as_bytes())
            }
            Framing::Chunked => head.extend_from_slice(b"Transfer-Encoding: chunked\r\n"),
        }
        head.extend_from_slice(b"Connection: close\r\n\r\n");
        head
    }
}

/// The values of every header named `wanted`, case-insensitively.
fn values<'a>(
    headers: &'a [(String, String)],
    wanted: &'a str,
) -> impl Iterator<Item = &'a str> + 'a {
    headers
        .iter()
        .filter(move |(name, _)| name.eq_ignore_ascii_case(wanted))
        .map(|(_, value)| value.as_str())
}

/// Read one line, bounded, including its `\n`. Empty at end of stream.
fn read_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    reader.take(limit as u64 + 1).read_until(b'\n', &mut line)?;
    if line.len() > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
    }
    Ok(line)
}

/// Read a head through its blank line, without the blank line. `None` when
/// the peer closed before sending a byte, as the self-test's connect does.
fn read_head(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut head = Vec::new();
    loop {
        let line = read_line(reader, MAX_LINE)?;
        if line.is_empty() {
            return if head.is_empty() {
                Ok(None)
            } else {
                Err(io::ErrorKind::UnexpectedEof.into())
            };
        }
        if line == b"\r\n" || line == b"\n" {
            if head.is_empty() {
                continue;
            }
            return Ok(Some(head));
        }
        if head.len() + line.len() > MAX_HEAD {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "head too large"));
        }
        let trimmed = line.strip_suffix(b"\n").unwrap_or(&line);
        let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
        if !head.is_empty() {
            head.extend_from_slice(b"\r\n");
        }
        head.extend_from_slice(trimmed);
    }
}

fn respond(client: &mut TcpStream, status: &str) -> io::Result<()> {
    client.write_all(
        format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
    )?;
    client.flush()?;
    let _ = client.shutdown(Shutdown::Write);
    Ok(())
}

fn copy_exact(from: &mut impl Read, to: &mut impl Write, n: u64) -> io::Result<()> {
    if io::copy(&mut from.take(n), to)? != n {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok(())
}

/// Relay a request body as its framing delimits it, and no further.
fn relay_body(from: &mut impl BufRead, to: &mut impl Write, framing: &Framing) -> io::Result<()> {
    match framing {
        Framing::None => Ok(()),
        Framing::Length(n) => copy_exact(from, to, *n),
        Framing::Chunked => loop {
            let line = read_line(from, MAX_LINE)?;
            if line.is_empty() {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            to.write_all(&line)?;
            let size = std::str::from_utf8(&line)
                .ok()
                .and_then(|l| l.trim().split(';').next())
                .and_then(|s| u64::from_str_radix(s.trim(), 16).ok())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "chunk size"))?;
            if size == 0 {
                // Trailers, through the blank line.
                loop {
                    let trailer = read_line(from, MAX_LINE)?;
                    if trailer.is_empty() {
                        return Err(io::ErrorKind::UnexpectedEof.into());
                    }
                    to.write_all(&trailer)?;
                    if trailer == b"\r\n" || trailer == b"\n" {
                        return Ok(());
                    }
                }
            }
            copy_exact(from, to, size)?;
            let end = read_line(from, 2)?;
            if end != b"\r\n" && end != b"\n" {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "chunk end"));
            }
            to.write_all(&end)?;
        },
    }
}

/// Relay the provider's response: its head with `Connection: close` in
/// place of any connection header, then every byte until it closes.
fn relay_response(upstream: Box<dyn Stream>, client: &mut TcpStream) -> io::Result<()> {
    let mut upstream = BufReader::new(upstream);
    let head = read_head(&mut upstream)?.ok_or(io::ErrorKind::UnexpectedEof)?;
    let text = String::from_utf8_lossy(&head);
    let mut lines = text.split("\r\n");
    let mut rewritten = format!("{}\r\n", lines.next().unwrap_or_default());
    for line in lines {
        let name = line.split(':').next().unwrap_or_default();
        if !name.eq_ignore_ascii_case("connection") && !name.eq_ignore_ascii_case("keep-alive") {
            rewritten.push_str(line);
            rewritten.push_str("\r\n");
        }
    }
    rewritten.push_str("Connection: close\r\n\r\n");
    client.write_all(rewritten.as_bytes())?;
    io::copy(&mut upstream, client)?;
    client.flush()?;
    let _ = client.shutdown(Shutdown::Write);
    Ok(())
}

fn serve(mut client: TcpStream, state: &State) -> io::Result<()> {
    client.set_read_timeout(Some(CLIENT_TIMEOUT))?;
    let mut reader = BufReader::new(client.try_clone()?);
    let Some(head) = read_head(&mut reader)? else {
        return Ok(());
    };
    let Some(request) = Request::parse(&head) else {
        return respond(&mut client, "400 Bad Request");
    };
    if !state.admits(request.bearer()) {
        return respond(&mut client, "401 Unauthorized");
    }
    if (request.method.as_str(), request.target.as_str()) == state.preflight {
        // Section 5: the preflight is answered here, and nothing is forwarded.
        return respond(&mut client, "200 OK");
    }
    let mut upstream = match state.upstream.connect() {
        Ok(upstream) => upstream,
        Err(_) => return respond(&mut client, "502 Bad Gateway"),
    };
    upstream.write_all(&request.forwarded(state.upstream.host(), &state.operator))?;
    relay_body(&mut reader, &mut upstream, &request.framing)?;
    upstream.flush()?;
    relay_response(upstream, &mut client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_adapter_claude_code::gateway::ROUTE;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::mpsc;

    const OPERATOR: &str = "operator-token-0051";

    #[test]
    fn the_production_upstream_is_tls_to_the_routes_one_authority_with_bundled_roots() {
        let upstream = provider_upstream(&ROUTE);
        assert_eq!(upstream.host(), "api.anthropic.com");
        // The ring provider offers a safe protocol version, so a config is
        // built, from the bundled roots alone.
        assert!(tls_config().is_some());
        assert!(!webpki_roots::TLS_SERVER_ROOTS.is_empty());
    }

    /// A plain-TCP stand-in for the provider: records each request it
    /// receives and answers with `answer`. Test-only: production forwards
    /// over TLS to the adapter's authority.
    struct Recording {
        address: SocketAddr,
    }

    impl Upstream for Recording {
        fn host(&self) -> &str {
            "provider.invalid"
        }
        fn connect(&self) -> io::Result<Box<dyn Stream>> {
            let stream = TcpStream::connect(self.address)?;
            stream.set_read_timeout(Some(UPSTREAM_TIMEOUT))?;
            Ok(Box::new(stream))
        }
    }

    fn recording(answer: &'static [u8]) -> (Arc<dyn Upstream>, mpsc::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, received) = mpsc::channel();
        std::thread::spawn(move || {
            for connection in listener.incoming() {
                let mut connection = connection.unwrap();
                connection
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                let mut bytes = Vec::new();
                let _ = connection.read_to_end(&mut bytes);
                connection.write_all(answer).unwrap();
                sender.send(bytes).unwrap();
            }
        });
        (Arc::new(Recording { address }), received)
    }

    fn exchange(gateway: &Gateway, request: &str) -> String {
        let mut client = TcpStream::connect(gateway.address()).unwrap();
        client.write_all(request.as_bytes()).unwrap();
        let mut answer = String::new();
        client.read_to_string(&mut answer).unwrap();
        answer
    }

    fn open(upstream: Arc<dyn Upstream>) -> (tempfile::TempDir, Gateway) {
        let configuration = tempfile::tempdir().unwrap();
        let gateway =
            Gateway::open(&ROUTE, OPERATOR.into(), configuration.path(), upstream).unwrap();
        (configuration, gateway)
    }

    #[test]
    fn the_attempt_token_is_written_owner_only_once_and_is_not_the_operators() {
        let (upstream, _) = recording(b"");
        let (configuration, gateway) = open(upstream);
        assert!(gateway.address().ip().is_loopback());
        assert_eq!(gateway.base_url(), format!("http://{}", gateway.address()));
        let file = configuration.path().join(TOKEN_FILE);
        assert_eq!(gateway.token_file(), file.as_path());
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            gateway.attempt_token()
        );
        assert!(gateway.attempt_token().starts_with(TOKEN_PREFIX));
        assert!(!gateway.attempt_token().contains(OPERATOR));
        // No-replace: a second gateway in the same directory refuses.
        let (upstream, _) = recording(b"");
        let again = Gateway::open(&ROUTE, OPERATOR.into(), configuration.path(), upstream);
        assert!(matches!(again, Err(GatewayError::Token(_))));
        assert!(!format!("{gateway:?}").contains(gateway.attempt_token()));
    }

    #[test]
    fn a_request_is_forwarded_with_the_operators_token_in_place_of_the_attempts() {
        let (upstream, received) =
            recording(b"HTTP/1.1 200 OK\r\nConnection: keep-alive\r\nContent-Length: 2\r\n\r\nok");
        let (_dir, gateway) = open(upstream);
        let request = format!(
            "POST /v1/messages?beta=true HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nx-api-key: stray\r\nanthropic-beta: oauth-2025-04-20\r\nContent-Length: 4\r\nConnection: keep-alive\r\n\r\nbody",
            gateway.address(),
            gateway.attempt_token()
        );
        let answer = exchange(&gateway, &request);
        assert!(answer.starts_with("HTTP/1.1 200 OK\r\n"), "{answer}");
        assert!(answer.contains("Connection: close\r\n"), "{answer}");
        assert!(!answer.contains("keep-alive"), "{answer}");
        assert!(answer.ends_with("\r\n\r\nok"), "{answer}");
        let forwarded = String::from_utf8(received.recv().unwrap()).unwrap();
        assert!(
            forwarded
                .starts_with("POST /v1/messages?beta=true HTTP/1.1\r\nHost: provider.invalid\r\n"),
            "{forwarded}"
        );
        assert!(forwarded.contains(&format!("Authorization: Bearer {OPERATOR}\r\n")));
        assert!(forwarded.contains("anthropic-beta: oauth-2025-04-20\r\n"));
        assert!(!forwarded.contains(gateway.attempt_token()));
        assert!(!forwarded.contains("x-api-key"));
        assert!(forwarded.contains("Connection: close\r\n"));
        assert!(forwarded.ends_with("\r\n\r\nbody"), "{forwarded}");
    }

    #[test]
    fn a_chunked_body_is_relayed_through_its_last_chunk_and_no_further() {
        let (upstream, received) = recording(b"HTTP/1.1 204 No Content\r\n\r\n");
        let (_dir, gateway) = open(upstream);
        let request = format!(
            "POST /v1/messages HTTP/1.1\r\nAuthorization: Bearer {}\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n",
            gateway.attempt_token()
        );
        let answer = exchange(&gateway, &request);
        assert!(answer.starts_with("HTTP/1.1 204"), "{answer}");
        let forwarded = String::from_utf8(received.recv().unwrap()).unwrap();
        assert!(forwarded.contains("Transfer-Encoding: chunked\r\n"));
        assert!(
            forwarded.ends_with("\r\n\r\n3\r\nabc\r\n0\r\n\r\n"),
            "{forwarded}"
        );
    }

    #[test]
    fn a_missing_wrong_revoked_or_other_attempts_token_is_refused_and_nothing_is_forwarded() {
        let (upstream, received) = recording(b"HTTP/1.1 200 OK\r\n\r\n");
        let (_dir, gateway) = open(upstream);
        let (other_upstream, _) = recording(b"");
        let (_other_dir, other) = open(other_upstream);
        let ask = |token: Option<&str>| {
            let authorization = token
                .map(|t| format!("Authorization: Bearer {t}\r\n"))
                .unwrap_or_default();
            exchange(
                &gateway,
                &format!("POST /v1/messages HTTP/1.1\r\n{authorization}\r\n"),
            )
        };
        for token in [
            None,
            Some("wrong"),
            Some(other.attempt_token()),
            Some(OPERATOR),
        ] {
            assert!(ask(token).starts_with("HTTP/1.1 401 "), "{token:?}");
        }
        // Two authorization headers are not a token.
        let doubled = format!(
            "GET / HTTP/1.1\r\nAuthorization: Bearer {t}\r\nAuthorization: Bearer {t}\r\n\r\n",
            t = gateway.attempt_token()
        );
        assert!(exchange(&gateway, &doubled).starts_with("HTTP/1.1 401 "));
        let token = gateway.attempt_token().to_string();
        gateway.revoke();
        assert!(ask(Some(&token)).starts_with("HTTP/1.1 401 "));
        assert!(
            received.recv_timeout(Duration::from_millis(200)).is_err(),
            "a refused request reached the provider"
        );
        // Dropped, the gateway accepts nothing at all.
        let address = gateway.address();
        drop(gateway);
        assert!(TcpStream::connect(address).is_err());
    }

    #[test]
    fn the_preflight_is_answered_locally_and_a_proxy_request_is_refused() {
        let (upstream, received) = recording(b"HTTP/1.1 200 OK\r\n\r\n");
        let (_dir, gateway) = open(upstream);
        let t = gateway.attempt_token().to_string();
        let preflight = exchange(
            &gateway,
            &format!("HEAD /api/hello HTTP/1.1\r\nAuthorization: Bearer {t}\r\n\r\n"),
        );
        assert!(preflight.starts_with("HTTP/1.1 200 OK"), "{preflight}");
        for refused in [
            format!("CONNECT evil.invalid:443 HTTP/1.1\r\nAuthorization: Bearer {t}\r\n\r\n"),
            format!("GET http://evil.invalid/ HTTP/1.1\r\nAuthorization: Bearer {t}\r\n\r\n"),
            format!("GET //evil.invalid/ HTTP/1.1\r\nAuthorization: Bearer {t}\r\n\r\n"),
            format!(
                "POST / HTTP/1.1\r\nAuthorization: Bearer {t}\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n"
            ),
            // A bare CR in a value or in the target would split a forwarded line.
            format!("GET / HTTP/1.1\r\nAuthorization: Bearer {t}\r\nX-A: a\rX-Injected: b\r\n\r\n"),
            format!("GET /a\rX-Injected:b HTTP/1.1\r\nAuthorization: Bearer {t}\r\n\r\n"),
        ] {
            assert!(
                exchange(&gateway, &refused).starts_with("HTTP/1.1 400 "),
                "{refused}"
            );
        }
        assert!(received.recv_timeout(Duration::from_millis(200)).is_err());
        // A connect that sends nothing, as the self-test's does, is harmless.
        drop(TcpStream::connect(gateway.address()).unwrap());
        assert!(
            exchange(
                &gateway,
                &format!("HEAD /api/hello HTTP/1.1\r\nAuthorization: Bearer {t}\r\n\r\n")
            )
            .starts_with("HTTP/1.1 200")
        );
    }

    #[test]
    fn a_gateway_that_cannot_bind_writes_nothing_and_an_unreachable_upstream_answers_502() {
        // An upstream that cannot be reached, with no network: a 502, and the
        // operator's token in no answer.
        struct Unreachable;
        impl Upstream for Unreachable {
            fn host(&self) -> &str {
                "provider.invalid"
            }
            fn connect(&self) -> io::Result<Box<dyn Stream>> {
                Err(io::Error::other("unreachable"))
            }
        }
        let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
        let configuration = tempfile::tempdir().unwrap();
        for address in [
            occupied.local_addr().unwrap(),
            SocketAddr::from(([0, 0, 0, 0], 0)),
        ] {
            let refused = Gateway::open_at(
                address,
                &ROUTE,
                OPERATOR.into(),
                configuration.path(),
                provider_upstream(&ROUTE),
            );
            assert!(matches!(refused, Err(GatewayError::Bind(_))), "{address}");
        }
        assert_eq!(std::fs::read_dir(configuration.path()).unwrap().count(), 0);
        let gateway = Gateway::open(
            &ROUTE,
            OPERATOR.into(),
            configuration.path(),
            Arc::new(Unreachable),
        )
        .unwrap();
        let answer = exchange(
            &gateway,
            &format!(
                "POST /v1/messages HTTP/1.1\r\nAuthorization: Bearer {}\r\n\r\n",
                gateway.attempt_token()
            ),
        );
        assert!(answer.starts_with("HTTP/1.1 502 "), "{answer}");
        assert!(!answer.contains(OPERATOR), "{answer}");
        for e in [
            GatewayError::Bind("x".into()).to_string(),
            GatewayError::Token("x".into()).to_string(),
        ] {
            assert!(!e.contains(OPERATOR));
        }
    }
}
