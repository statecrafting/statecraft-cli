//! A scripted cell on a loopback port, for spec 038's acceptance.
//!
//! One thread accepts connections, parses one HTTP/1.1 request per
//! connection, records it, and answers from a handler. Every response closes
//! its connection, so the client never reuses one across scripted answers.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// One recorded request.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn form(&self, key: &str) -> Option<String> {
        String::from_utf8_lossy(&self.body)
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| decode(v))
    }
}

fn decode(s: &str) -> String {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'?'));
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// An answer: status, extra headers, body.
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub fn json(status: u16, v: serde_json::Value) -> Answer {
    Answer {
        status,
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: serde_json::to_vec(&v).unwrap(),
    }
}

pub type Handler = Box<dyn Fn(&str, &Request, usize) -> Answer + Send + Sync>;

/// A running cell.
pub struct Cell {
    pub origin: String,
    pub requests: Arc<Mutex<Vec<Request>>>,
}

impl Cell {
    /// Start a cell whose answers come from `handler`, given the cell's
    /// origin, each request, and how many requests with the same method and
    /// path came before it.
    pub fn start(handler: Handler) -> Cell {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests: Arc<Mutex<Vec<Request>>> = Arc::default();
        let seen = requests.clone();
        let own = origin.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let Some(req) = read_request(&mut stream) else {
                    continue;
                };
                let count = {
                    let mut all = seen.lock().unwrap();
                    let n = all
                        .iter()
                        .filter(|r| r.method == req.method && r.path == req.path)
                        .count();
                    all.push(req.clone());
                    n
                };
                let answer = handler(&own, &req, count);
                let mut head = format!(
                    "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                    answer.status,
                    answer.body.len()
                );
                for (k, v) in &answer.headers {
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                head.push_str("\r\n");
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&answer.body);
                let _ = stream.flush();
            }
        });
        Cell { origin, requests }
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    pub fn count(&self, method: &str, path: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| r.method == method && r.path == path)
            .count()
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    let mut length = 0usize;
    loop {
        let mut h = String::new();
        reader.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            let (k, v) = (k.trim().to_string(), v.trim().to_string());
            if k.eq_ignore_ascii_case("content-length") {
                length = v.parse().unwrap_or(0);
            }
            headers.push((k, v));
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path,
        headers,
        body,
    })
}

/// A JWT with `alg` and `aud`; the signature is not checked by the client.
pub fn jwt(alg: &str, aud: serde_json::Value) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header =
        b64.encode(serde_json::to_vec(&serde_json::json!({"alg": alg, "typ": "JWT"})).unwrap());
    let claims =
        b64.encode(serde_json::to_vec(&serde_json::json!({"aud": aud, "sub": "person"})).unwrap());
    format!("{header}.{claims}.c2lnbmF0dXJl")
}

/// The discovery documents a well-behaved cell serves for `origin`.
pub fn discovery(origin: &str, path: &str) -> Option<Answer> {
    match path {
        "/.well-known/oauth-protected-resource" => Some(json(
            200,
            serde_json::json!({
                "resource": origin,
                "authorization_servers": [format!("{origin}/auth/v1/")],
                "scopes_supported": ["statecraft:submit", "statecraft:read"],
            }),
        )),
        "/auth/v1/.well-known/openid-configuration" => Some(json(
            200,
            serde_json::json!({
                "issuer": format!("{origin}/auth/v1/"),
                "device_authorization_endpoint": format!("{origin}/auth/v1/oidc/device"),
                "token_endpoint": format!("{origin}/auth/v1/oidc/token"),
            }),
        )),
        _ => None,
    }
}
