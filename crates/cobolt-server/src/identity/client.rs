// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend's HTTP client for an identity provider and for reCAPTCHA: `ureq` over the
//! operating system's TLS, behind a trait so a test stands a mock provider in front of it.
//!
//! Only `https://`, or `http://` to a loopback host (a local mock or a local Keycloak) — the same
//! rule, and the same parser, as the key fetcher's ([`crate::jwks::address_allowed`]). No redirect
//! is followed (a redirect is how a secret leaves for somewhere else), a call takes at most ten
//! seconds, and an answer is read up to a megabyte.

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// The method and the address only: headers and bodies carry secrets.
impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.method, self.url.split('?').next().unwrap_or(""))
    }
}

impl HttpRequest {
    pub fn get(url: &str, headers: &[(&str, &str)]) -> Self {
        HttpRequest { method: "GET", url: url.to_string(), headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), body: Vec::new() }
    }

    pub fn post_json(url: &str, headers: &[(&str, &str)], body: &serde_json::Value) -> Self {
        Self::post(url, headers, "application/json", body.to_string().into_bytes())
    }

    /// Amazon's JSON 1.1 protocol (Cognito): `X-Amz-Target` names the operation.
    pub fn post_amz(url: &str, target: &str, body: &serde_json::Value) -> Self {
        Self::post(url, &[("X-Amz-Target", target)], "application/x-amz-json-1.1", body.to_string().into_bytes())
    }

    pub fn post_form(url: &str, headers: &[(&str, &str)], fields: &[(&str, &str)]) -> Self {
        Self::post(url, headers, "application/x-www-form-urlencoded", form_encode(fields).into_bytes())
    }

    fn post(url: &str, headers: &[(&str, &str)], content_type: &str, body: Vec<u8>) -> Self {
        let mut h: Vec<(String, String)> = headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        h.push(("Content-Type".into(), content_type.into()));
        HttpRequest { method: "POST", url: url.to_string(), headers: h, body }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The body's `application/x-www-form-urlencoded` fields (a mock reads them).
    pub fn form(&self) -> Vec<(String, String)> {
        let body = String::from_utf8_lossy(&self.body);
        body.split('&')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                (crate::http::percent_decode(k), crate::http::percent_decode(v))
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_str(&self.body).ok()
    }
}

pub trait Http: Send + Sync {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, String>;
}

/// `application/x-www-form-urlencoded`.
pub fn form_encode(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(k, v)| format!("{}={}", percent(k), percent(v))).collect::<Vec<_>>().join("&")
}

/// Percent-encode everything but the unreserved characters (RFC 3986).
pub fn percent(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `ureq` with the operating system's TLS.
pub struct UreqHttp;

impl Http for UreqHttp {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, String> {
        if !crate::jwks::address_allowed(&req.url) {
            return Err("the address is not https".to_string());
        }
        let tls = native_tls::TlsConnector::new().map_err(|e| format!("TLS: {e}"))?;
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).tls_connector(Arc::new(tls)).redirects(0).build();
        let mut r = agent.request(req.method, &req.url);
        for (k, v) in &req.headers {
            r = r.set(k, v);
        }
        let resp = match r.send_bytes(&req.body) {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            // The transport error names the host, never the body.
            Err(e) => return Err(e.kind().to_string()),
        };
        let status = resp.status();
        let mut body = String::new();
        resp.into_reader().take(1024 * 1024).read_to_string(&mut body).map_err(|e| format!("reading the answer: {e}"))?;
        Ok(HttpResponse { status, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_values_are_percent_encoded_and_read_back() {
        assert_eq!(form_encode(&[("a", "b c"), ("redirect_uri", "https://x.test/cb?a=1&b=2")]), "a=b%20c&redirect_uri=https%3A%2F%2Fx.test%2Fcb%3Fa%3D1%26b%3D2");
        assert_eq!(percent("é~-_."), "%C3%A9~-_.");
        let r = HttpRequest::post_form("https://x.test/token", &[], &[("code", "a+b/c="), ("x", "")]);
        assert_eq!(r.form(), vec![("code".to_string(), "a+b/c=".to_string()), ("x".to_string(), String::new())]);
        assert_eq!(format!("{r:?}"), "POST https://x.test/token", "a request prints without its body");
    }

    #[test]
    fn plain_http_to_another_host_is_never_spoken() {
        let e = UreqHttp.send(&HttpRequest::get("http://idp.example.test/x", &[])).unwrap_err();
        assert!(e.contains("not https"), "{e}");
    }
}
