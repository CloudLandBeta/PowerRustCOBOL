// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! REST Client Runtime — Phase 10.
//!
//! `HttpClient` manages per-session HTTP state (persistent headers, last
//! status code) and is owned by the interpreter.
//!
//! # Supported built-in CALLs
//!
//! | CALL name                | Arguments (BY REFERENCE)                          |
//! |--------------------------|---------------------------------------------------|
//! | `COBOL-HTTP-GET`         | url-var, response-var, status-var                 |
//! | `COBOL-HTTP-POST`        | url-var, body-var, response-var, status-var        |
//! | `COBOL-HTTP-PUT`         | url-var, body-var, response-var, status-var        |
//! | `COBOL-HTTP-DELETE`      | url-var, response-var, status-var                 |
//! | `COBOL-HTTP-SET-HEADER`  | name-var, value-var                               |
//! | `COBOL-HTTP-CLEAR-HEADERS` | (no arguments)                                  |
//!
//! # Argument conventions
//!
//! - **url-var** — `PIC X(2048)` COBOL variable holding the full URL
//!   (trimmed of trailing spaces before use).
//! - **body-var** — `PIC X(32767)` request body string (for POST / PUT).
//! - **response-var** — `PIC X(32767)` receives the response body (truncated
//!   if longer than 32 767 bytes).
//! - **status-var** — `PIC 9(4)` receives the HTTP status code (e.g. 200,
//!   404).  On network errors it is set to 0.
//!
//! # Connection strings / URL format
//!
//! Any valid HTTP / HTTPS URL.  TLS is the operating system's own stack
//! (schannel on Windows, Security.framework on macOS, OpenSSL on Linux),
//! reached through `ureq`'s native-tls adapter.

//! # The `http` feature
//!
//! Everything below that touches the network is behind it. TLS here is the
//! platform's own stack through `native-tls`, and on **Linux** that is OpenSSL
//! — a system library the build machine must have development headers for. A
//! program that never calls one of the verbs above should not have to; the
//! compiler leaves this feature off for those, and `openssl-sys` drops out of
//! the dependency graph entirely.
//!
//! The `HttpClient` type, its header state and its whole method surface exist
//! either way. Only the sending is gone — an off-build reports that through the
//! same `(body, status)` pair a network failure uses, so no caller changes.

use std::collections::HashMap;
#[cfg(feature = "http")]
use std::sync::{Arc, OnceLock};

/// What a verb reports when the HTTP bridge was left out of the build.
///
/// Travels back as the response body with status 0 — the same channel a DNS
/// failure or a refused connection uses, so a COBOL program that checks its
/// status var already handles it. Reachable only when the compiler proved the
/// program never reaches an HTTP verb and it did anyway.
#[cfg(not(feature = "http"))]
const HTTP_NOT_LINKED: &str = "the HTTP bridge is not linked into this program: \
     the build found no CALL to COBOL-HTTP-GET (or any of its companions) and \
     left the client out, which is what lets an application build without the \
     platform's TLS development files. Reaching an HTTP verb through a name \
     assembled at run time defeats that reading — CALL the verb by its literal \
     name somewhere the build can see it";

// ── TLS ────────────────────────────────────────────────────────────────────

/// One shared OS-TLS connector for every request this runtime makes.
///
/// `ureq`'s `native-tls` feature is an **adapter only** — the crate-level helpers
/// (`ureq::get`, `ureq::post`, …) and a bare `AgentBuilder` never pick it up, so
/// every HTTPS call must go through an agent carrying this connector or it fails
/// with "no TLS backend". rustls is deliberately not used: it pulls in `ring`,
/// which compiles C and would put a C toolchain back on the list of things you
/// need in order to build PowerRustCOBOL.
///
/// `None` means the platform refused to hand over its TLS stack; plain HTTP still
/// works and HTTPS reports the failure through the usual status-0 path.
#[cfg(feature = "http")]
fn tls_connector() -> Option<Arc<native_tls::TlsConnector>> {
    static CONNECTOR: OnceLock<Option<Arc<native_tls::TlsConnector>>> = OnceLock::new();
    CONNECTOR
        .get_or_init(|| match native_tls::TlsConnector::new() {
            Ok(c) => Some(Arc::new(c)),
            Err(e) => {
                tracing::warn!(%e, "no platform TLS available; HTTPS calls will fail");
                None
            }
        })
        .clone()
}

/// The agent every request runs through. `timeout_ms` of 0 keeps ureq's defaults.
///
/// `pub(crate)` so `ors_bridge` reuses this one TLS setup rather than building a
/// second: the connector is the whole reason HTTPS works here at all, and a
/// module that quietly forgot it would fail with "no TLS backend" only in
/// production, only over HTTPS.
#[cfg(feature = "http")]
pub(crate) fn agent(timeout_ms: u64) -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new();
    if timeout_ms > 0 {
        builder = builder.timeout(std::time::Duration::from_millis(timeout_ms));
    }
    if let Some(connector) = tls_connector() {
        builder = builder.tls_connector(connector);
    }
    builder.build()
}

/// A TLS connector that accepts any certificate.
///
/// Reached only when a control sets `VerifyTLS = false`, which is how a
/// developer works against a staging server with a self-signed certificate.
/// Cached separately from [`tls_connector`] so the verifying connector — the
/// default every other call uses — is never replaced by this one.
#[cfg(feature = "http")]
fn permissive_tls_connector() -> Option<Arc<native_tls::TlsConnector>> {
    static CONNECTOR: OnceLock<Option<Arc<native_tls::TlsConnector>>> = OnceLock::new();
    CONNECTOR
        .get_or_init(|| {
            match native_tls::TlsConnector::builder()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .build()
            {
                Ok(c) => Some(Arc::new(c)),
                Err(e) => {
                    tracing::warn!(%e, "no platform TLS available; HTTPS calls will fail");
                    None
                }
            }
        })
        .clone()
}

/// The agent for a request that carries its own redirect / TLS policy.
///
/// [`agent`] stays the entry point for everything that does not (the
/// `COBOL-HTTP-*` CALLs, the Maps and search bridges), so their behaviour is
/// untouched.
#[cfg(feature = "http")]
fn agent_configured(timeout_ms: u64, follow_redirects: bool, verify_tls: bool) -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new();
    if timeout_ms > 0 {
        builder = builder.timeout(std::time::Duration::from_millis(timeout_ms));
    }
    if !follow_redirects {
        builder = builder.redirects(0);
    }
    let connector = if verify_tls {
        tls_connector()
    } else {
        permissive_tls_connector()
    };
    if let Some(connector) = connector {
        builder = builder.tls_connector(connector);
    }
    builder.build()
}

// ── HttpClient ────────────────────────────────────────────────────────────────

/// Per-interpreter HTTP client state.
///
/// Headers set via `COBOL-HTTP-SET-HEADER` persist across calls until
/// `COBOL-HTTP-CLEAR-HEADERS` resets them.
#[derive(Default, Clone)]
pub struct HttpClient {
    /// Persistent extra headers sent with every request.
    headers: HashMap<String, String>,
}

/// Base64, standard alphabet with padding — enough for one HTTP Basic header.
///
/// Hand-rolled rather than pulled from a crate: this is the only base64 in the
/// workspace, and `cobolt-runtime` builds without the `http` feature at all, so
/// a dependency would have to be feature-gated to stay out of a console
/// program's graph. Twenty lines cost less than that.
pub(crate) fn encode_base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// What one request may override, on top of the session-wide header state.
///
/// A `RestClient` control carries its own address, credentials, redirect and
/// TLS policy. Those are *per control*, so they cannot be folded into
/// [`HttpClient`], which is one shared object per interpreter: writing them
/// there would leak one control's Authorization header onto every other
/// control's calls. They travel with the request instead.
///
/// [`Default`] is exactly the behaviour every caller had before this existed:
/// no timeout, redirects followed, TLS verified, no extra headers.
#[derive(Clone, Debug)]
pub struct RequestConfig {
    /// Overall transport timeout; 0 keeps `ureq`'s defaults.
    pub timeout_ms: u64,
    /// Follow 3xx responses. When false the redirect itself is returned.
    pub follow_redirects: bool,
    /// Verify the server's TLS certificate and hostname.
    pub verify_tls: bool,
    /// Extra headers for this request. Session headers set through
    /// `COBOL-HTTP-SET-HEADER` override these by name: an explicit runtime
    /// call is more specific than design-time configuration.
    pub headers: Vec<(String, String)>,
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 0,
            follow_redirects: true,
            verify_tls: true,
            headers: Vec::new(),
        }
    }
}

impl HttpClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add / overwrite a persistent request header.
    pub fn set_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.headers.insert(name.into(), value.into());
    }

    /// Remove all persistent headers.
    pub fn clear_headers(&mut self) {
        self.headers.clear();
    }
}

// ── Sending, when the `http` feature is off ──────────────────────────────────
//
// Deliberately adjacent to the real implementation rather than in a file of its
// own: a method added below must be added here too, and a shim kept somewhere
// else is a shim that silently rots until someone builds the configuration
// nobody builds by default.
#[cfg(not(feature = "http"))]
impl HttpClient {
    pub fn get(&self, _url: &str) -> (String, u16) {
        (HTTP_NOT_LINKED.to_owned(), 0)
    }
    pub fn post(&self, _url: &str, _body: &str) -> (String, u16) {
        (HTTP_NOT_LINKED.to_owned(), 0)
    }
    pub fn put(&self, _url: &str, _body: &str) -> (String, u16) {
        (HTTP_NOT_LINKED.to_owned(), 0)
    }
    pub fn delete(&self, _url: &str) -> (String, u16) {
        (HTTP_NOT_LINKED.to_owned(), 0)
    }
    pub fn get_with_timeout(&self, _url: &str, _timeout_ms: u64) -> (String, u16) {
        (HTTP_NOT_LINKED.to_owned(), 0)
    }
    pub fn delete_with_timeout(&self, _url: &str, _timeout_ms: u64) -> (String, u16) {
        (HTTP_NOT_LINKED.to_owned(), 0)
    }
    pub fn send_with_body_timeout(
        &self,
        _method: &str,
        _url: &str,
        _body: &str,
        _timeout_ms: u64,
    ) -> (String, u16) {
        (HTTP_NOT_LINKED.to_owned(), 0)
    }
    pub fn send_configured(
        &self,
        _method: &str,
        _url: &str,
        _body: Option<&str>,
        _cfg: &RequestConfig,
    ) -> (String, u16) {
        (HTTP_NOT_LINKED.to_owned(), 0)
    }
}

#[cfg(feature = "http")]
impl HttpClient {
    /// Execute an HTTP GET.
    ///
    /// Returns `(body, status_code)`.  On network failure status is 0 and
    /// body contains the error description.
    pub fn get(&self, url: &str) -> (String, u16) {
        let url = url.trim();
        let mut req = agent(0).get(url);
        for (k, v) in &self.headers {
            req = req.set(k.as_str(), v.as_str());
        }
        match req.call() {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.into_string().unwrap_or_default();
                (body, status)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                (body, code)
            }
            Err(e) => (format!("HTTP GET error: {e}"), 0),
        }
    }

    /// Execute an HTTP POST with a string body.
    ///
    /// The `Content-Type` header defaults to `application/json` unless
    /// overridden via `COBOL-HTTP-SET-HEADER`.
    pub fn post(&self, url: &str, body: &str) -> (String, u16) {
        self.send_with_body("POST", url, body)
    }

    /// Execute an HTTP PUT with a string body.
    pub fn put(&self, url: &str, body: &str) -> (String, u16) {
        self.send_with_body("PUT", url, body)
    }

    /// Execute an HTTP DELETE.
    pub fn delete(&self, url: &str) -> (String, u16) {
        let url = url.trim();
        let mut req = agent(0).delete(url);
        for (k, v) in &self.headers {
            req = req.set(k.as_str(), v.as_str());
        }
        match req.call() {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.into_string().unwrap_or_default();
                (body, status)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                (body, code)
            }
            Err(e) => (format!("HTTP DELETE error: {e}"), 0),
        }
    }

    // ── Timeout-aware variants (spec 032 — async operations) ──────────────────
    //
    // These build a per-call `ureq::Agent` carrying an overall timeout so a
    // background worker thread cannot live forever when a server stalls. A
    // `timeout_ms` of 0 means "no transport timeout" (the agent uses `ureq`'s
    // defaults). Interpreter-side timeout semantics (`onTimeout`) are enforced
    // separately by the wait-loop sweep; this bound is a thread-lifetime
    // backstop. Behaviour is otherwise identical to the plain methods above.

    /// Build a `ureq::Agent` with an optional overall timeout.
    fn agent_with_timeout(timeout_ms: u64) -> ureq::Agent {
        agent(timeout_ms)
    }

    /// GET with an overall timeout (see [`get`](Self::get)).
    pub fn get_with_timeout(&self, url: &str, timeout_ms: u64) -> (String, u16) {
        let url = url.trim();
        let agent = Self::agent_with_timeout(timeout_ms);
        let mut req = agent.get(url);
        for (k, v) in &self.headers {
            req = req.set(k.as_str(), v.as_str());
        }
        Self::finish_call(req, "GET")
    }

    /// DELETE with an overall timeout (see [`delete`](Self::delete)).
    pub fn delete_with_timeout(&self, url: &str, timeout_ms: u64) -> (String, u16) {
        let url = url.trim();
        let agent = Self::agent_with_timeout(timeout_ms);
        let mut req = agent.delete(url);
        for (k, v) in &self.headers {
            req = req.set(k.as_str(), v.as_str());
        }
        Self::finish_call(req, "DELETE")
    }

    /// POST/PUT with a string body and an overall timeout
    /// (see [`send_with_body`](Self::send_with_body)).
    pub fn send_with_body_timeout(
        &self,
        method: &str,
        url: &str,
        body: &str,
        timeout_ms: u64,
    ) -> (String, u16) {
        let url = url.trim();
        let agent = Self::agent_with_timeout(timeout_ms);

        let content_type = self
            .headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == "content-type")
            .map(|(_, v)| v.as_str())
            .unwrap_or("application/json");

        let mut req = match method {
            "PUT" => agent.put(url),
            _ => agent.post(url),
        };
        for (k, v) in &self.headers {
            if k.to_ascii_lowercase() != "content-type" {
                req = req.set(k.as_str(), v.as_str());
            }
        }
        match req.set("Content-Type", content_type).send_string(body) {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.into_string().unwrap_or_default();
                (body, status)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                (body, code)
            }
            Err(e) => (format!("HTTP {method} error: {e}"), 0),
        }
    }

    /// Run a prepared no-body request and map the result to `(body, status)`.
    fn finish_call(req: ureq::Request, label: &str) -> (String, u16) {
        match req.call() {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.into_string().unwrap_or_default();
                (body, status)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                (body, code)
            }
            Err(e) => (format!("HTTP {label} error: {e}"), 0),
        }
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    /// Send one request under an explicit [`RequestConfig`].
    ///
    /// `body` of `None` is a request that carries none (GET / DELETE / HEAD);
    /// `Some` sends it as a string and defaults `Content-Type` to
    /// `application/json`, matching [`send_with_body`](Self::send_with_body).
    /// Any method name is accepted, so `PATCH` works without a fifth near-copy
    /// of this function.
    pub fn send_configured(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        cfg: &RequestConfig,
    ) -> (String, u16) {
        let url = url.trim();
        let method = method.trim().to_ascii_uppercase();
        let agent = agent_configured(cfg.timeout_ms, cfg.follow_redirects, cfg.verify_tls);
        let headers = self.merged_headers(cfg);

        let mut req = agent.request(&method, url);
        for (k, v) in &headers {
            if body.is_some() && k.eq_ignore_ascii_case("content-type") {
                continue; // applied last, so an explicit one still wins
            }
            req = req.set(k.as_str(), v.as_str());
        }

        let result = match body {
            Some(b) => {
                let content_type = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("application/json");
                req.set("Content-Type", content_type).send_string(b)
            }
            None => req.call(),
        };

        match result {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.into_string().unwrap_or_default();
                (body, status)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                (body, code)
            }
            Err(e) => (format!("HTTP {method} error: {e}"), 0),
        }
    }

    /// The headers one request actually sends: the config's, then the session's
    /// on top. A `COBOL-HTTP-SET-HEADER` call is an explicit runtime act and
    /// outranks a control's design-time `DefaultHeaders`.
    ///
    /// Names are matched case-insensitively (HTTP treats them so), and the
    /// session's are visited in name order so the result does not depend on
    /// `HashMap` iteration order.
    fn merged_headers(&self, cfg: &RequestConfig) -> Vec<(String, String)> {
        let mut session: Vec<(&String, &String)> = self.headers.iter().collect();
        session.sort_by(|a, b| a.0.cmp(b.0));

        let mut out: Vec<(String, String)> = Vec::new();
        for (k, v) in cfg
            .headers
            .iter()
            .map(|(k, v)| (k, v))
            .chain(session.into_iter())
        {
            match out
                .iter_mut()
                .find(|(existing, _)| existing.eq_ignore_ascii_case(k))
            {
                Some(slot) => *slot = (k.clone(), v.clone()),
                None => out.push((k.clone(), v.clone())),
            }
        }
        out
    }

    fn send_with_body(&self, method: &str, url: &str, body: &str) -> (String, u16) {
        let url = url.trim();

        // Default content-type unless overridden.
        let content_type = self
            .headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == "content-type")
            .map(|(_, v)| v.as_str())
            .unwrap_or("application/json");

        let mut req = match method {
            "POST" => agent(0).post(url),
            "PUT" => agent(0).put(url),
            _ => agent(0).post(url),
        };

        for (k, v) in &self.headers {
            if k.to_ascii_lowercase() != "content-type" {
                req = req.set(k.as_str(), v.as_str());
            }
        }

        match req.set("Content-Type", content_type).send_string(body) {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.into_string().unwrap_or_default();
                (body, status)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                (body, code)
            }
            Err(e) => (format!("HTTP {method} error: {e}"), 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc4648_vectors_including_padding() {
        // Every remainder case: 0, 1 and 2 bytes over a group of three.
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode_base64(input.as_bytes()), want, "encoding {input:?}");
        }
    }

    #[test]
    fn base64_encodes_bytes_that_are_not_text() {
        // The `+` and `/` end of the alphabet, which `user:password` rarely reaches.
        assert_eq!(encode_base64(&[0xfb, 0xff, 0xfe]), "+//+");
    }

    #[test]
    fn the_default_request_config_is_the_behaviour_callers_already_had() {
        let cfg = RequestConfig::default();
        assert_eq!(cfg.timeout_ms, 0);
        assert!(cfg.follow_redirects);
        assert!(cfg.verify_tls);
        assert!(cfg.headers.is_empty());
    }

    #[cfg(feature = "http")]
    #[test]
    fn an_explicit_session_header_overrides_a_controls_default_header() {
        // `COBOL-HTTP-SET-HEADER` is something the program did on purpose at run
        // time; `DefaultHeaders` is design-time configuration. The explicit act
        // wins, and matching is case-insensitive because HTTP says so.
        let mut client = HttpClient::new();
        client.set_header("accept", "text/plain");

        let cfg = RequestConfig {
            headers: vec![
                ("Accept".to_owned(), "application/json".to_owned()),
                ("X-Trace".to_owned(), "42".to_owned()),
            ],
            ..RequestConfig::default()
        };

        let merged = client.merged_headers(&cfg);
        let find = |n: &str| {
            merged
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(n))
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(find("Accept"), Some("text/plain"), "session header wins");
        assert_eq!(find("X-Trace"), Some("42"), "unopposed control header survives");
        assert_eq!(
            merged.iter().filter(|(k, _)| k.eq_ignore_ascii_case("accept")).count(),
            1,
            "Accept must not be sent twice: {merged:?}"
        );
    }
}
