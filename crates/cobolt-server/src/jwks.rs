// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The issuer's signing keys (JWKS), fetched and cached (WASM-003 R10).
//!
//! A key is looked up by the token's `kid`. Keys are trusted for a configured
//! time; an unknown `kid` may mean the issuer rotated, so one fetch is allowed —
//! but never more often than `jwks_min_refresh`, whatever the cause, so a flood
//! of tokens naming random key ids cannot turn the backend into a request
//! amplifier against the issuer. A failed fetch keeps serving the last keys
//! (up to a day) and says so in the log, because an issuer outage must not take
//! the application down with it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::Jwk;
use jsonwebtoken::{Algorithm, DecodingKey};
use tokio::sync::Mutex;

use cobolt_wire::error::{codes, CallError, ErrorClass};

/// How the key document is obtained. The default reads it over HTTPS; a test
/// supplies its own.
pub trait JwksFetcher: Send + Sync {
    /// Blocking; called on a worker thread.
    fn fetch(&self, uri: &str) -> Result<String, String>;
}

/// May the backend fetch from this address? `https://` anywhere; `http://` only to
/// a loopback host (a local mock or a local container). The host is parsed, not
/// matched by prefix: `http://127.0.0.1.evil.test` is not loopback.
pub fn address_allowed(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // Anything before an '@' is credentials, and the host is what follows it.
    let authority = authority.rsplit('@').next().unwrap_or("");
    let host = if let Some(v6) = authority.strip_prefix('[') { v6.split(']').next().unwrap_or("") } else { authority.split(':').next().unwrap_or("") };
    host == "localhost" || host.parse::<std::net::IpAddr>().map(|a| a.is_loopback()).unwrap_or(false)
}

/// `ureq` with the operating system's TLS.
pub struct HttpFetcher;

impl JwksFetcher for HttpFetcher {
    fn fetch(&self, uri: &str) -> Result<String, String> {
        if !address_allowed(uri) {
            return Err(format!("the key address {uri} is not https"));
        }
        let tls = native_tls::TlsConnector::new().map_err(|e| format!("TLS: {e}"))?;
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .tls_connector(Arc::new(tls))
            .redirects(0)
            .build();
        let resp = agent.get(uri).call().map_err(|e| format!("fetching the keys: {e}"))?;
        let mut body = String::new();
        use std::io::Read;
        resp.into_reader().take(1024 * 1024).read_to_string(&mut body).map_err(|e| format!("reading the keys: {e}"))?;
        Ok(body)
    }
}

struct Entry {
    key: DecodingKey,
    alg: Option<Algorithm>,
}

#[derive(Default)]
struct State {
    keys: HashMap<String, Entry>,
    /// Keys without a `kid`, in document order.
    anonymous: Vec<Entry>,
    fetched_at: Option<Instant>,
    last_attempt: Option<Instant>,
}

pub struct Jwks {
    uri: String,
    ttl: Duration,
    min_refresh: Duration,
    fetcher: Arc<dyn JwksFetcher>,
    state: Mutex<State>,
    /// Counts fetches, for tests and the health report.
    fetches: std::sync::atomic::AtomicU64,
}

/// The longest a failed fetch may be papered over with old keys.
const STALE_LIMIT: Duration = Duration::from_secs(24 * 3600);

impl Jwks {
    pub fn new(uri: String, ttl_secs: u64, min_refresh_secs: u64, fetcher: Arc<dyn JwksFetcher>) -> Self {
        Jwks {
            uri,
            ttl: Duration::from_secs(ttl_secs),
            min_refresh: Duration::from_secs(min_refresh_secs),
            fetcher,
            state: Mutex::new(State::default()),
            fetches: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn fetch_count(&self) -> u64 {
        self.fetches.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The verification key for a token with this `kid` and `alg`.
    pub async fn key_for(&self, kid: Option<&str>, alg: Algorithm) -> Result<DecodingKey, CallError> {
        let invalid = |m: &str| CallError::new(ErrorClass::Authentication, codes::TOKEN_INVALID, m.to_string());
        let mut st = self.state.lock().await;
        let fresh = st.fetched_at.map(|t| t.elapsed() < self.ttl).unwrap_or(false);
        if fresh {
            if let Some(k) = lookup(&st, kid, alg) {
                return Ok(k);
            }
        }
        // Not found, or stale: refresh if allowed.
        let may_fetch = st.last_attempt.map(|t| t.elapsed() >= self.min_refresh).unwrap_or(true);
        if may_fetch {
            st.last_attempt = Some(Instant::now());
            let uri = self.uri.clone();
            let fetcher = self.fetcher.clone();
            self.fetches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // The lock is held across the fetch on purpose: concurrent callers
            // wait for this one fetch instead of each starting their own.
            let fetched = tokio::task::spawn_blocking(move || fetcher.fetch(&uri)).await.map_err(|e| format!("join: {e}")).and_then(|r| r);
            match fetched.and_then(|doc| parse(&doc)) {
                Ok((keys, anon)) => {
                    st.keys = keys;
                    st.anonymous = anon;
                    st.fetched_at = Some(Instant::now());
                }
                Err(e) => {
                    tracing::warn!(target: "auth", "the issuer's keys could not be refreshed: {e}");
                    let stale_ok = st.fetched_at.map(|t| t.elapsed() < STALE_LIMIT).unwrap_or(false);
                    if !stale_ok {
                        return Err(CallError::new(ErrorClass::Authentication, codes::TOKEN_INVALID, "the issuer's signing keys are unavailable"));
                    }
                }
            }
        } else if st.fetched_at.is_none() {
            return Err(invalid("the issuer's signing keys are unavailable"));
        }
        lookup(&st, kid, alg).ok_or_else(|| invalid("the token is signed with a key the issuer does not publish"))
    }
}

fn lookup(st: &State, kid: Option<&str>, alg: Algorithm) -> Option<DecodingKey> {
    let ok = |e: &Entry| e.alg.map(|a| a == alg).unwrap_or(true);
    match kid {
        Some(k) => st.keys.get(k).filter(|e| ok(e)).map(|e| e.key.clone()),
        // No kid: only when the issuer publishes exactly one usable key.
        None => {
            let all: Vec<&Entry> = st.keys.values().chain(st.anonymous.iter()).filter(|e| ok(e)).collect();
            if all.len() == 1 {
                Some(all[0].key.clone())
            } else {
                None
            }
        }
    }
}

/// Parse a JWKS document, key by key: one the library cannot use (an encryption
/// key, an unknown curve) is skipped and does not spoil the rest.
fn parse(doc: &str) -> Result<(HashMap<String, Entry>, Vec<Entry>), String> {
    let v: serde_json::Value = serde_json::from_str(doc).map_err(|e| format!("the key document is not JSON: {e}"))?;
    let arr = v.get("keys").and_then(|k| k.as_array()).ok_or("the key document has no \"keys\"")?;
    let mut keys = HashMap::new();
    let mut anon = Vec::new();
    for k in arr {
        // Only signature keys.
        if let Some(u) = k.get("use").and_then(|u| u.as_str()) {
            if u != "sig" {
                continue;
            }
        }
        let Ok(jwk) = serde_json::from_value::<Jwk>(k.clone()) else { continue };
        let Ok(key) = DecodingKey::from_jwk(&jwk) else { continue };
        let alg = jwk.common.key_algorithm.and_then(|a| Algorithm::try_from(a).ok());
        let entry = Entry { key, alg };
        match jwk.common.key_id.clone() {
            Some(kid) => {
                keys.insert(kid, entry);
            }
            None => anon.push(entry),
        }
    }
    if keys.is_empty() && anon.is_empty() {
        return Err("the key document holds no usable signing key".to_string());
    }
    Ok((keys, anon))
}

#[cfg(test)]
mod tests {
    use super::address_allowed;

    #[test]
    fn only_https_and_loopback_http_may_be_fetched_from() {
        for ok in ["https://idp.example.test/jwks", "http://127.0.0.1:9000/jwks", "http://localhost/jwks", "http://[::1]:8080/x", "http://127.0.0.1", "http://user@127.0.0.1/x"] {
            assert!(address_allowed(ok), "{ok}");
        }
        for bad in ["http://idp.example.test/jwks", "http://127.0.0.1.evil.test/jwks", "http://localhost.evil.test/", "http://127.0.0.1@evil.test/", "ftp://x", "gopher://x", "", "x"] {
            assert!(!address_allowed(bad), "{bad}");
        }
    }
}
