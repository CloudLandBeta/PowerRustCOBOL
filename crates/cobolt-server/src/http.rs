// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The small HTTPS surface beside the WebSocket (WASM-003 Q3, WASM-013 R4).
//!
//! Requests are parsed by hyper and routed by the closed route table in
//! `serve.rs` (`front_router`); what stays here is what every answer shares.

/// The largest request head accepted; past it hyper answers 431.
pub const MAX_HEAD: usize = 16 * 1024;

/// The security headers every response of the HTTPS surface carries
/// (WASM-013 R4).
pub const SECURITY_HEADERS: &[(&str, &str)] = &[
    ("strict-transport-security", "max-age=63072000; includeSubDomains"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("permissions-policy", "geolocation=(), camera=(), microphone=()"),
    ("cache-control", "no-store"),
];

/// The largest request body the identity routes read (WASM-009 D5): a sign-in is a few hundred bytes.
pub const MAX_BODY: usize = 16 * 1024;

/// A body longer than the cap, or one that broke off: the route answers 413 and reads no further.
#[derive(Debug, PartialEq, Eq)]
pub enum BodyError {
    TooLarge,
    Broken,
}

/// Read a request body, refusing it as soon as it passes `limit` bytes — a `Content-Length` that
/// says so is refused before a byte is read, and a body that lies is cut off when it overruns.
pub async fn read_body(headers: &axum::http::HeaderMap, body: axum::body::Body, limit: usize) -> Result<Vec<u8>, BodyError> {
    let declared = headers.get(axum::http::header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok());
    if declared.is_some_and(|n| n > limit as u64) {
        return Err(BodyError::TooLarge);
    }
    use futures_util::StreamExt;
    let mut out = Vec::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| BodyError::Broken)?;
        if out.len() + chunk.len() > limit {
            return Err(BodyError::TooLarge);
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// The cookies of a `Cookie` header, in order. A pair without `=` and an empty name are skipped;
/// values are taken as sent (the backend's own are base64url, so nothing is unquoted).
pub fn parse_cookies(header: &str) -> Vec<(String, String)> {
    header
        .split(';')
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            let k = k.trim();
            (!k.is_empty()).then(|| (k.to_owned(), v.trim().trim_matches('"').to_owned()))
        })
        .collect()
}

/// One cookie of the request's `Cookie` headers (the first of that name: a `__Host-` cookie cannot
/// be shadowed by a sub-domain's, which is why the backend's cookies carry the prefix).
pub fn cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(parse_cookies)
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    Strict,
    Lax,
}

/// A `Set-Cookie` with its attributes as data. The backend's cookies are all `__Host-` cookies:
/// `Secure`, `Path=/`, no `Domain` (so the browser binds them to this host only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCookie {
    pub name: String,
    pub value: String,
    pub http_only: bool,
    pub same_site: SameSite,
    /// Seconds; 0 deletes the cookie. `None` is a session cookie.
    pub max_age: Option<u64>,
}

impl SetCookie {
    /// A `__Host-` cookie. The name and value are checked: a cookie the browser would mangle or
    /// split is a bug, refused here rather than sent.
    pub fn host(name: &str, value: &str, same_site: SameSite, max_age: Option<u64>) -> Result<SetCookie, String> {
        let token = |c: char| c.is_ascii_alphanumeric() || "-_.".contains(c);
        if !name.starts_with("__Host-") || !name.chars().all(token) {
            return Err(format!("{name} is not a __Host- cookie name"));
        }
        if !value.chars().all(|c| token(c) || c == '~') {
            return Err(format!("the value of {name} is not base64url"));
        }
        Ok(SetCookie { name: name.to_owned(), value: value.to_owned(), http_only: true, same_site, max_age })
    }

    /// The cookie that deletes `name`.
    pub fn delete(name: &str, same_site: SameSite) -> SetCookie {
        SetCookie { name: name.to_owned(), value: String::new(), http_only: true, same_site, max_age: Some(0) }
    }

    /// The header value.
    pub fn header(&self) -> String {
        let mut h = format!("{}={}; Path=/; Secure", self.name, self.value);
        if self.http_only {
            h.push_str("; HttpOnly");
        }
        h.push_str(match self.same_site {
            SameSite::Strict => "; SameSite=Strict",
            SameSite::Lax => "; SameSite=Lax",
        });
        if let Some(a) = self.max_age {
            h.push_str(&format!("; Max-Age={a}"));
        }
        h
    }
}

/// Does the answer carry the browser's cookies and credentials (WASM-013 R6)?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credentials {
    No,
    Yes,
}

/// **The one CORS function** (WASM-013 R6): no other place in the server writes `Access-Control-Allow-Origin` or
/// `Access-Control-Allow-Credentials` (`tests/cors_sources.rs` searches for it).
///
/// * `Credentials::Yes` — a request that carries cookies (D5): the one origin is echoed **only when it is listed**
///   in `allowed`, with `Allow-Credentials: true`. Never `*`, never under `any_origin`, never `null`.
/// * `Credentials::No` — a plain answer (a download): a listed origin is echoed, and so is any origin when
///   `any_origin` is on (a development run); never `*`, never `Allow-Credentials`.
///
/// `Vary: Origin` is always added, so a cache cannot hand one origin's answer to another. An origin that is not
/// answered gets no CORS header at all, and the browser keeps the answer from the page.
pub fn cors_headers(origin: Option<&str>, allowed: &[String], any_origin: bool, c: Credentials) -> Vec<(&'static str, String)> {
    let vary = ("vary", "Origin".to_owned());
    let Some(o) = origin.filter(|o| *o != "null" && *o != "*") else { return vec![vary] };
    let listed = allowed.iter().any(|a| a == o);
    match c {
        Credentials::Yes if listed => vec![("access-control-allow-origin", o.to_owned()), ("access-control-allow-credentials", "true".to_owned()), vary],
        Credentials::No if listed || any_origin => vec![("access-control-allow-origin", o.to_owned()), vary],
        _ => vec![vary],
    }
}

/// The extra headers of a CORS preflight answer for an allowed origin: the identity routes' methods
/// and the request headers they need. `X-PR-Client` is not among them: it asks for the refresh token in
/// the body, which only the mobile shell's own Rust side may do (D6), never a page's script.
pub const PREFLIGHT_HEADERS: &[(&str, &str)] = &[
    ("access-control-allow-methods", "GET, POST"),
    ("access-control-allow-headers", "content-type, x-pr-identity"),
    ("access-control-max-age", "600"),
];

/// Percent-decode a query component (enough for the signed-link and callback parameters).
pub fn query_param(query: &str, name: &str) -> Option<String> {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == name {
            return Some(percent_decode(v));
        }
    }
    None
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|x| u8::from_str_radix(x, 16).ok());
                match h {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header, HeaderMap, HeaderValue};

    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(f)
    }

    #[test]
    fn an_over_long_body_is_refused_whether_or_not_it_says_so() {
        let ok = run(read_body(&HeaderMap::new(), axum::body::Body::from(vec![b'a'; MAX_BODY]), MAX_BODY));
        assert_eq!(ok.map(|b| b.len()), Ok(MAX_BODY), "exactly the cap is read");
        let over = run(read_body(&HeaderMap::new(), axum::body::Body::from(vec![b'a'; MAX_BODY + 1]), MAX_BODY));
        assert_eq!(over, Err(BodyError::TooLarge), "a body that does not declare its length is cut off");
        let mut h = HeaderMap::new();
        h.insert(header::CONTENT_LENGTH, HeaderValue::from_static("1000000"));
        assert_eq!(run(read_body(&h, axum::body::Body::from("{}"), MAX_BODY)), Err(BodyError::TooLarge), "a declared length past the cap is refused unread");
    }

    #[test]
    fn cookies_parse_and_the_host_cookie_carries_its_attributes() {
        let c = SetCookie::host("__Host-pr_refresh", "AbC-_123", SameSite::Strict, None).unwrap();
        assert_eq!(c.header(), "__Host-pr_refresh=AbC-_123; Path=/; Secure; HttpOnly; SameSite=Strict");
        let h = SetCookie::host("__Host-pr_hosted", "x", SameSite::Lax, Some(600)).unwrap();
        assert_eq!(h.header(), "__Host-pr_hosted=x; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=600");
        assert_eq!(SetCookie::delete("__Host-pr_refresh", SameSite::Strict).header(), "__Host-pr_refresh=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0");
        assert!(!c.header().to_ascii_lowercase().contains("domain="), "a __Host- cookie names no domain");
        // Round trip: what was set is what comes back.
        let mut req = HeaderMap::new();
        req.append(header::COOKIE, HeaderValue::from_static("theme=dark; __Host-pr_refresh=AbC-_123"));
        req.append(header::COOKIE, HeaderValue::from_static("__Host-pr_hosted=x"));
        assert_eq!(cookie(&req, "__Host-pr_refresh").as_deref(), Some("AbC-_123"));
        assert_eq!(cookie(&req, "__Host-pr_hosted").as_deref(), Some("x"));
        assert_eq!(cookie(&req, "pr_refresh"), None);
        assert_eq!(parse_cookies(" a=1;; =2; b ; c=\"3\""), vec![("a".to_string(), "1".to_string()), ("c".to_string(), "3".to_string())]);
        // A name without the prefix, or a value that would split the header, is refused.
        assert!(SetCookie::host("pr_refresh", "x", SameSite::Strict, None).is_err());
        assert!(SetCookie::host("__Host-pr_refresh", "a;b", SameSite::Strict, None).is_err());
        assert!(SetCookie::host("__Host-pr_refresh", "a\r\nSet-Cookie: x=1", SameSite::Strict, None).is_err());
    }

    #[test]
    fn cors_echoes_one_listed_origin_with_credentials_and_never_a_wildcard() {
        let allowed = vec!["https://app.example.test".to_string()];
        let h = cors_headers(Some("https://app.example.test"), &allowed, false, Credentials::Yes);
        assert!(h.contains(&("access-control-allow-origin", "https://app.example.test".to_string())));
        assert!(h.contains(&("access-control-allow-credentials", "true".to_string())));
        for other in [Some("https://evil.example.test"), Some("null"), None, Some("https://app.example.test.evil.test")] {
            let h = cors_headers(other, &allowed, false, Credentials::Yes);
            assert!(h.iter().all(|(k, _)| !k.starts_with("access-control-")), "{other:?}: {h:?}");
            assert!(h.contains(&("vary", "Origin".to_string())));
        }
        assert!(!format!("{h:?}").contains('*'));
    }

    /// WASM-013 T10: under `any_origin` a credentialed answer echoes nothing and carries no `Allow-Credentials`; a plain
    /// answer echoes the origin (a development download) but never credentials, and a `*` or `null` origin is never echoed.
    #[test]
    fn any_origin_never_carries_credentials_and_a_wildcard_is_never_echoed() {
        let none: Vec<String> = Vec::new();
        let listed = vec!["https://app.example.test".to_string()];
        let yes = cors_headers(Some("https://evil.example.test"), &none, true, Credentials::Yes);
        assert_eq!(yes, vec![("vary", "Origin".to_string())], "credentials + any origin: no echo, no Allow-Credentials");
        let no = cors_headers(Some("https://evil.example.test"), &none, true, Credentials::No);
        assert!(no.contains(&("access-control-allow-origin", "https://evil.example.test".to_string())));
        assert!(no.iter().all(|(k, _)| *k != "access-control-allow-credentials"), "{no:?}");
        assert!(cors_headers(Some("https://evil.example.test"), &listed, false, Credentials::No).iter().all(|(k, _)| !k.starts_with("access-control-")));
        assert!(cors_headers(Some("https://app.example.test"), &listed, false, Credentials::No).iter().any(|(k, _)| *k == "access-control-allow-origin"));
        for odd in ["*", "null"] {
            assert_eq!(cors_headers(Some(odd), &listed, true, Credentials::No), vec![("vary", "Origin".to_string())], "{odd}");
        }
        println!("cors_headers: credentials only for a listed origin, any_origin never with credentials, * and null never echoed");
    }

    #[test]
    fn every_answer_is_not_stored() {
        assert!(SECURITY_HEADERS.contains(&("cache-control", "no-store")));
    }
}

