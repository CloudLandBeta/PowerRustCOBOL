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
//! Any valid HTTP / HTTPS URL.  TLS is handled transparently by `ureq`.

use std::collections::HashMap;

// ── HttpClient ────────────────────────────────────────────────────────────────

/// Per-interpreter HTTP client state.
///
/// Headers set via `COBOL-HTTP-SET-HEADER` persist across calls until
/// `COBOL-HTTP-CLEAR-HEADERS` resets them.
#[derive(Default)]
pub struct HttpClient {
    /// Persistent extra headers sent with every request.
    headers: HashMap<String, String>,
}

impl HttpClient {
    pub fn new() -> Self { Self::default() }

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
        let mut req = ureq::get(url);
        for (k, v) in &self.headers {
            req = req.set(k.as_str(), v.as_str());
        }
        match req.call() {
            Ok(resp)  => {
                let status = resp.status();
                let body   = resp.into_string().unwrap_or_default();
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
        let mut req = ureq::delete(url);
        for (k, v) in &self.headers {
            req = req.set(k.as_str(), v.as_str());
        }
        match req.call() {
            Ok(resp) => {
                let status = resp.status();
                let body   = resp.into_string().unwrap_or_default();
                (body, status)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                (body, code)
            }
            Err(e) => (format!("HTTP DELETE error: {e}"), 0),
        }
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    fn send_with_body(&self, method: &str, url: &str, body: &str) -> (String, u16) {
        let url = url.trim();

        // Default content-type unless overridden.
        let content_type = self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == "content-type")
            .map(|(_, v)| v.as_str())
            .unwrap_or("application/json");

        let mut req = match method {
            "POST" => ureq::post(url),
            "PUT"  => ureq::put(url),
            _      => ureq::post(url),
        };

        for (k, v) in &self.headers {
            if k.to_ascii_lowercase() != "content-type" {
                req = req.set(k.as_str(), v.as_str());
            }
        }

        match req.set("Content-Type", content_type).send_string(body) {
            Ok(resp) => {
                let status = resp.status();
                let body   = resp.into_string().unwrap_or_default();
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
