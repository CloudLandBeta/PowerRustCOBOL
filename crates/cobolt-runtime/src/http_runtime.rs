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

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

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
