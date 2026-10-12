//! Spec 038's acceptance for the hosted client crate, against a scripted
//! cell on a loopback port: discovery (3.1), the device grant (3.2), holding
//! a credential (3.3), the request rules (3.4), and the submission and
//! retrieval routes (3.5, 3.6). Section 3.7's negative cases are each a test
//! here or in the CLI's `hosted` suite.

mod support;

use serde_json::json;
use statecraft_hosted::api::{self, Retrieved};
use statecraft_hosted::credential::{Credential, Store};
use statecraft_hosted::declaration::Declaration;
use statecraft_hosted::discovery::discover;
use statecraft_hosted::grant::{self, Revocation};
use statecraft_hosted::http::Http;
use statecraft_hosted::{Clock, Error, Origin};
use std::cell::Cell as StdCell;
use support::cell::{Answer, Cell, discovery, json as answer, jwt};

/// A clock that only moves when it sleeps, and records each sleep.
struct Fake {
    now: StdCell<u64>,
    sleeps: std::cell::RefCell<Vec<u64>>,
}

impl Fake {
    fn at(now: u64) -> Fake {
        Fake {
            now: StdCell::new(now),
            sleeps: Default::default(),
        }
    }
}

impl Clock for Fake {
    fn now(&self) -> u64 {
        self.now.get()
    }
    fn sleep(&self, s: u64) {
        self.sleeps.borrow_mut().push(s);
        self.now.set(self.now.get() + s);
    }
}

fn token_body(origin: &str, alg: &str, token_type: &str) -> serde_json::Value {
    json!({
        "access_token": jwt(alg, json!([origin, "other"])),
        "token_type": token_type,
        "expires_in": 3600,
        "refresh_token": "refresh-secret-value",
        "scope": "statecraft:submit",
    })
}

/// A cell that discovers well and answers the device grant from `token`.
fn grant_cell(token: impl Fn(&str, usize) -> Answer + Send + Sync + 'static) -> Cell {
    Cell::start(Box::new(move |origin, req, n| {
        if let Some(a) = discovery(origin, &req.path) {
            return a;
        }
        match req.path.as_str() {
            "/auth/v1/oidc/device" => answer(
                200,
                json!({
                    "device_code": "device-secret-value",
                    "user_code": "WXYZ-1234",
                    "verification_uri": format!("{origin}/auth/v1/device"),
                    "expires_in": 300,
                    "interval": 5,
                }),
            ),
            "/auth/v1/oidc/token" => token(origin, n),
            _ => answer(404, json!({})),
        }
    }))
}

#[test]
fn discovery_requires_the_origins_own_issuer() {
    let cell = grant_cell(|_, _| answer(400, json!({})));
    let origin = Origin::parse(&cell.origin).unwrap();
    let e = discover(&Http::new(), &origin).unwrap();
    assert_eq!(e.issuer, format!("{}/auth/v1/", cell.origin));
    assert_eq!(e.token, format!("{}/auth/v1/oidc/token", cell.origin));

    for (label, broken) in [
        ("another issuer", "issuer"),
        ("another resource", "resource"),
        ("an endpoint off the origin", "endpoint"),
        ("no issuer listed", "servers"),
    ] {
        let cell = Cell::start(Box::new(move |origin, req, _| {
            let mut a = discovery(origin, &req.path).unwrap_or_else(|| answer(404, json!({})));
            let mut v: serde_json::Value = serde_json::from_slice(&a.body).unwrap_or(json!({}));
            match (broken, req.path.as_str()) {
                ("issuer", "/auth/v1/.well-known/openid-configuration") => {
                    v["issuer"] = json!("https://other.example/auth/v1/")
                }
                ("resource", "/.well-known/oauth-protected-resource") => {
                    v["resource"] = json!("https://other.example")
                }
                ("endpoint", "/auth/v1/.well-known/openid-configuration") => {
                    v["token_endpoint"] = json!("https://other.example/token")
                }
                ("servers", "/.well-known/oauth-protected-resource") => {
                    v["authorization_servers"] = json!(["https://other.example/auth/v1/"])
                }
                _ => {}
            }
            a.body = serde_json::to_vec(&v).unwrap();
            a
        }));
        let origin = Origin::parse(&cell.origin).unwrap();
        let err = discover(&Http::new(), &origin).unwrap_err();
        assert!(matches!(err, Error::Refused(_)), "{label}: {err}");
        // Refused before any grant starts.
        assert_eq!(cell.count("POST", "/auth/v1/oidc/device"), 0, "{label}");
    }
}

#[test]
fn the_device_grant_polls_at_the_interval_and_slows_down() {
    let cell = grant_cell(|origin, n| match n {
        0 => answer(400, json!({"error": "authorization_pending"})),
        1 => answer(400, json!({"error": "slow_down"})),
        2 => answer(400, json!({"error": "authorization_pending"})),
        _ => answer(200, token_body(origin, "RS256", "Bearer")),
    });
    let origin = Origin::parse(&cell.origin).unwrap();
    let http = Http::new();
    let endpoints = discover(&http, &origin).unwrap();
    let scopes = vec![
        "statecraft:submit".to_string(),
        "statecraft:read".to_string(),
    ];
    let start = grant::start(&http, &endpoints, "cli-client", &scopes).unwrap();
    assert_eq!(start.user_code, "WXYZ-1234");
    // The device code is never rendered.
    assert!(!format!("{start:?}").contains("device-secret-value"));
    let device = &cell.requests()[2];
    assert_eq!(device.form("client_id").as_deref(), Some("cli-client"));
    assert_eq!(
        device.form("scope").as_deref(),
        Some("statecraft:submit statecraft:read")
    );
    let clock = Fake::at(1_000);
    let c = grant::poll(&http, &endpoints, &origin, "cli-client", &start, &clock).unwrap();
    assert_eq!(*clock.sleeps.borrow(), [5, 5, 10, 10]);
    assert_eq!(c.issued_at, 1_030);
    assert_eq!(c.expires_at(), 4_630);
    assert_eq!(c.refresh_not_before(), 4_570);
    // No cookie and no CSRF pair, ever.
    for r in cell.requests() {
        assert!(r.header("cookie").is_none(), "{}", r.path);
        assert!(r.header("x-csrf-token").is_none(), "{}", r.path);
    }
}

#[test]
fn a_denied_or_expired_grant_ends_without_retry() {
    for (error, want) in [
        ("access_denied", "access_denied"),
        ("expired_token", "expired_token"),
    ] {
        let cell = grant_cell(move |_, _| answer(400, json!({"error": error})));
        let origin = Origin::parse(&cell.origin).unwrap();
        let http = Http::new();
        let endpoints = discover(&http, &origin).unwrap();
        let start = grant::start(&http, &endpoints, "c", &["s".into()]).unwrap();
        let err = grant::poll(&http, &endpoints, &origin, "c", &start, &Fake::at(0)).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains(want), "{err}");
        assert_eq!(
            cell.count("POST", "/auth/v1/oidc/token"),
            1,
            "{error}: retried"
        );
    }
    // The code's own lifetime ends a grant nobody answers.
    let cell = grant_cell(|_, _| answer(400, json!({"error": "authorization_pending"})));
    let origin = Origin::parse(&cell.origin).unwrap();
    let http = Http::new();
    let endpoints = discover(&http, &origin).unwrap();
    let start = grant::start(&http, &endpoints, "c", &["s".into()]).unwrap();
    let err = grant::poll(&http, &endpoints, &origin, "c", &start, &Fake::at(0)).unwrap_err();
    assert!(err.to_string().contains("expired_token"), "{err}");
    assert!(cell.count("POST", "/auth/v1/oidc/token") <= 60);
}

#[test]
fn a_token_that_is_not_rs256_or_lacks_the_origin_is_not_stored() {
    let origin = Origin::parse("https://cell.example").unwrap();
    let ok = token_body("https://cell.example", "RS256", "Bearer");
    assert!(grant::accept(&origin, &serde_json::to_vec(&ok).unwrap(), 0).is_ok());
    let mut lower = ok.clone();
    lower["token_type"] = json!("bearer");
    assert!(grant::accept(&origin, &serde_json::to_vec(&lower).unwrap(), 0).is_ok());
    for (label, body) in [
        (
            "HS256",
            token_body("https://cell.example", "HS256", "Bearer"),
        ),
        ("none", token_body("https://cell.example", "none", "Bearer")),
        (
            "another audience",
            token_body("https://other.example", "RS256", "Bearer"),
        ),
        (
            "not bearer",
            token_body("https://cell.example", "RS256", "MAC"),
        ),
        (
            "not a jwt",
            json!({"access_token": "opaque", "token_type": "Bearer", "expires_in": 60}),
        ),
    ] {
        let err = grant::accept(&origin, &serde_json::to_vec(&body).unwrap(), 0).unwrap_err();
        assert_eq!(err.exit_code(), 2, "{label}");
        assert!(err.to_string().contains("not stored"), "{label}: {err}");
        assert!(!err.to_string().contains("refresh-secret-value"), "{label}");
    }
}

fn credential(origin: &Origin, issued_at: u64) -> Credential {
    Credential {
        origin: origin.as_str().to_string(),
        access_token: "access-secret-value".into(),
        refresh_token: Some("refresh-secret-value".into()),
        expires_in: 600,
        issued_at,
        scope: None,
    }
}

#[test]
fn an_early_refresh_is_refused_locally_and_a_late_one_is_sent() {
    let cell = grant_cell(|origin, _| answer(200, token_body(origin, "RS256", "Bearer")));
    let origin = Origin::parse(&cell.origin).unwrap();
    let http = Http::new();
    let endpoints = discover(&http, &origin).unwrap();
    let held = credential(&origin, 10_000);
    // 60 seconds before expiry is the first permitted instant.
    let err =
        grant::refresh(&http, &endpoints, &origin, "c", &held, &Fake::at(10_539)).unwrap_err();
    assert!(err.to_string().contains("wait until 10540"), "{err}");
    assert_eq!(
        cell.count("POST", "/auth/v1/oidc/token"),
        0,
        "presented early"
    );
    let renewed =
        grant::refresh(&http, &endpoints, &origin, "c", &held, &Fake::at(10_540)).unwrap();
    assert_eq!(renewed.issued_at, 10_540);
    let sent = &cell.requests()[2];
    assert_eq!(sent.form("grant_type").as_deref(), Some("refresh_token"));
    assert_eq!(
        sent.form("refresh_token").as_deref(),
        Some("refresh-secret-value")
    );
}

#[test]
fn the_credential_is_owner_only_under_the_home_and_never_rendered() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let origin = Origin::parse("https://cell.example").unwrap();
    let store = Store::new(home.path());
    assert!(store.load(&origin).unwrap().is_none());
    let c = credential(&origin, 5);
    store.save(&origin, &c).unwrap();
    let path = store.path(&origin);
    assert!(path.starts_with(home.path()));
    assert!(path.ends_with(format!("{}.json", origin.digest())));
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(store.load(&origin).unwrap(), Some(c.clone()));
    let shown = format!("{c:?}");
    assert!(!shown.contains("access-secret-value") && !shown.contains("refresh-secret-value"));
    // A file another user could read is refused, not used.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    assert!(store.load(&origin).is_err());
    assert!(store.delete(&origin).unwrap());
    assert!(!store.delete(&origin).unwrap());
}

#[test]
fn logout_revokes_with_the_bearer_token_and_reports_what_the_cell_said() {
    let cell = Cell::start(Box::new(|_, req, _| match req.path.as_str() {
        "/session/token/revoke" => answer(200, json!({})),
        _ => answer(404, json!({})),
    }));
    let origin = Origin::parse(&cell.origin).unwrap();
    let c = credential(&origin, 0);
    assert_eq!(
        grant::revoke(&Http::new(), &origin, &c),
        Revocation::Confirmed
    );
    let r = &cell.requests()[0];
    assert_eq!(
        r.header("authorization"),
        Some("Bearer access-secret-value")
    );
    assert!(r.header("cookie").is_none());
    let unreachable = Origin::parse("http://127.0.0.1:9").unwrap();
    assert_eq!(
        grant::revoke(&Http::new(), &unreachable, &c),
        Revocation::Unreachable
    );
}

/// A cell whose API routes answer from `api`; discovery is not needed.
fn api_cell(
    api: impl Fn(&str, &support::cell::Request, usize) -> Answer + Send + Sync + 'static,
) -> (Cell, Origin) {
    let cell = Cell::start(Box::new(api));
    let origin = Origin::parse(&cell.origin).unwrap();
    (cell, origin)
}

const REPO: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const REV: &str = "2222222222222222222222222222222222222222222222222222222222222222";

#[test]
fn the_request_rules_classify_and_never_retry_a_4xx() {
    let cases: Vec<(&str, Answer, i32, &str)> = vec![
        (
            "401 bootstrap",
            Answer {
                status: 401,
                headers: vec![(
                    "WWW-Authenticate".into(),
                    "Bearer resource_metadata=\"x\"".into(),
                )],
                body: b"{}".to_vec(),
            },
            2,
            "log in to",
        ),
        (
            "insufficient scope",
            Answer {
                status: 403,
                headers: vec![(
                    "WWW-Authenticate".into(),
                    "Bearer error=\"insufficient_scope\", scope=\"statecraft:submit\"".into(),
                )],
                body: b"{}".to_vec(),
            },
            2,
            "statecraft:submit",
        ),
        ("409", answer(409, json!({})), 2, "refused (409)"),
        ("503", answer(503, json!({})), 4, "status 503"),
    ];
    for (label, a, exit, says) in cases {
        let a = std::sync::Mutex::new(Some(a));
        let (cell, origin) = api_cell(move |_, _, _| {
            a.lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| answer(500, json!({})))
        });
        let c = credential(&origin, 0);
        let err =
            api::register_revision(&Http::new(), &origin, &c, REPO, "abc", None, None).unwrap_err();
        assert_eq!(err.exit_code(), exit, "{label}: {err}");
        assert!(err.to_string().contains(says), "{label}: {err}");
        // Named by route template, never by URL; never the token. (The
        // login refusal names the origin, which is not a URL to a route.)
        assert!(
            !err.to_string().contains(&format!("{}/api", cell.origin)),
            "{label}: {err}"
        );
        assert!(!err.to_string().contains("access-secret-value"), "{label}");
        assert_eq!(cell.requests().len(), 1, "{label}: retried");
    }
    // A transport failure names the route, not the URL.
    let gone = Origin::parse("http://127.0.0.1:9").unwrap();
    let err = api::register_revision(
        &Http::new(),
        &gone,
        &credential(&gone, 0),
        REPO,
        "abc",
        None,
        None,
    )
    .unwrap_err();
    assert_eq!(err.exit_code(), 4);
    assert!(
        err.to_string().contains("/api/repositories/{id}/revisions"),
        "{err}"
    );
    assert!(!err.to_string().contains("127.0.0.1"), "{err}");
}

#[test]
fn registering_twice_is_success_and_says_which() {
    let (cell, origin) = api_cell(|_, req, n| {
        assert_eq!(req.path, format!("/api/repositories/{REPO}/revisions"));
        answer(
            if n == 0 { 201 } else { 200 },
            json!({"revision": REV, "repository": REPO}),
        )
    });
    let c = credential(&origin, 0);
    let http = Http::new();
    let first =
        api::register_revision(&http, &origin, &c, REPO, "c0ffee", Some("7ee"), None).unwrap();
    let again =
        api::register_revision(&http, &origin, &c, REPO, "c0ffee", Some("7ee"), None).unwrap();
    assert_eq!((first.revision.as_str(), first.existed), (REV, false));
    assert_eq!((again.revision.as_str(), again.existed), (REV, true));
    let body: serde_json::Value = serde_json::from_slice(&cell.requests()[0].body).unwrap();
    assert_eq!(body, json!({"commit": "c0ffee", "tree": "7ee"}));
    assert_eq!(
        cell.requests()[0].header("authorization"),
        Some("Bearer access-secret-value")
    );
}

#[test]
fn a_submission_is_idempotent_by_construction() {
    let (cell, origin) =
        api_cell(|_, _, _| answer(200, json!({"attestations": ["a"], "admission": "pending"})));
    let c = credential(&origin, 0);
    let http = Http::new();
    let submission =
        serde_json::to_vec(&json!({"repository": REPO, "revision": REV, "manifest": {}})).unwrap();
    let artifacts = vec![b"receipt bytes".to_vec(), b"artifact".to_vec()];
    let a = api::submit(&http, &origin, &c, REPO, REV, &submission, &artifacts).unwrap();
    let b = api::submit(&http, &origin, &c, REPO, REV, &submission, &artifacts).unwrap();
    assert_eq!(a.idempotency_key, b.idempotency_key);
    assert_eq!(a.response_digest, b.response_digest);
    let reqs = cell.requests();
    assert_eq!(
        reqs[0].body, reqs[1].body,
        "the body is a function of its parts"
    );
    assert_eq!(
        reqs[0].header("idempotency-key"),
        Some(a.idempotency_key.as_str())
    );
    let body = String::from_utf8_lossy(&reqs[0].body);
    assert_eq!(body.matches("name=\"artifact\"").count(), 2);
    assert!(body.find("name=\"submission\"").unwrap() < body.find("receipt bytes").unwrap());
    // A different manifest is a different key.
    let other = api::idempotency_key(REPO, REV, b"{}");
    assert_ne!(other, a.idempotency_key);
}

#[test]
fn a_decision_for_an_unregistered_revision_is_not_registered() {
    let (_cell, origin) = api_cell(|_, req, _| {
        if req.path.ends_with(&format!("{REV}/decision")) {
            answer(404, json!({}))
        } else {
            answer(200, json!({"revision": REV, "admissionWord": "none"}))
        }
    });
    let c = credential(&origin, 0);
    let http = Http::new();
    assert_eq!(
        api::decision(&http, &origin, &c, REPO, REV).unwrap(),
        Retrieved::NotRegistered
    );
    let other = "3333333333333333333333333333333333333333333333333333333333333333";
    match api::decision(&http, &origin, &c, REPO, other).unwrap() {
        Retrieved::Bundle(b) => assert!(String::from_utf8_lossy(&b).contains("none")),
        r => panic!("{r:?}"),
    }
}

#[test]
fn the_declaration_names_the_cell_client_and_scopes_or_refuses() {
    let ok = json!({
        "schemaVersion": 1,
        "cell": "https://cell.example",
        "repository": REPO,
        "clientId": "statecraft-cli",
        "scopes": ["statecraft:submit"],
    });
    let d = Declaration::parse(&serde_json::to_vec(&ok).unwrap()).unwrap();
    assert_eq!(d.cell.as_str(), "https://cell.example");
    for (label, edit) in [
        ("no client", ("clientId", json!(""))),
        ("no scopes", ("scopes", json!([]))),
        ("bad cell", ("cell", json!("http://cell.example"))),
        ("bad repository", ("repository", json!("xyz"))),
        ("unknown version", ("schemaVersion", json!(2))),
    ] {
        let mut v = ok.clone();
        v[edit.0] = edit.1;
        assert!(
            Declaration::parse(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{label}"
        );
    }
    let mut extra = ok.clone();
    extra["token"] = json!("x");
    assert!(Declaration::parse(&serde_json::to_vec(&extra).unwrap()).is_err());
    let root = tempfile::tempdir().unwrap();
    let err = Declaration::read(root.path()).unwrap_err();
    assert!(
        err.to_string()
            .contains(".statecraft/hosted.json is absent"),
        "{err}"
    );
}
