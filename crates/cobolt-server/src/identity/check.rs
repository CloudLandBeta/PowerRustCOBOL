// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Test connection (WASM-009 R9, D13): what `rcrun backend auth test` and the IDE's Identity page
//! print. Each check is one line, and no line holds a configured value: no secret, no client id, no
//! address. A line says what was checked and how it went, so the operator can tell a mistyped secret
//! from a provider that is not reachable.
//!
//! The checks:
//! 1. the provider's metadata is read and names the configured issuer;
//! 2. the key set it points to has signing keys;
//! 3. in-app Cognito: a sign-in for a random user is refused as an unknown user — which the provider
//!    says only after it has accepted the app client (and its secret);
//! 4. a hosted flow: the provider's authorise page answers;
//! 5. reCAPTCHA: Google says `invalid-input-response` for a made-up token (the secret is good), not
//!    `invalid-input-secret`.

use std::sync::Arc;

use serde_json::Value;

use crate::config::{discover, IdentityFlow, Provider, ResolvedIdentity};
use crate::identity::client::{Http, HttpRequest};
use crate::identity::oidc::{pkce, random_token};
use crate::identity::recaptcha::DEFAULT_ENDPOINT;
use crate::identity::{adapter, IdpError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Fail,
    Skip,
}

impl Status {
    pub fn word(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Fail => "FAIL",
            Status::Skip => "skip",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub status: Status,
    pub text: String,
}

impl Line {
    fn new(status: Status, text: impl Into<String>) -> Line {
        Line { status, text: text.into() }
    }
}

impl std::fmt::Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<4}  {}", self.status.word(), self.text)
    }
}

/// `http` reached as the key fetcher of [`discover`], so every request of the test goes one way.
struct ViaHttp(Arc<dyn Http>);

impl crate::jwks::JwksFetcher for ViaHttp {
    fn fetch(&self, uri: &str) -> Result<String, String> {
        if !crate::jwks::address_allowed(uri) {
            return Err("the address is not https".into());
        }
        let r = self.0.send(&HttpRequest::get(uri, &[("accept", "application/json")]))?;
        if r.status == 200 {
            Ok(r.body)
        } else {
            Err(format!("HTTP {}", r.status))
        }
    }
}

/// An error, without the addresses and values some messages carry: only its first clause, cut at
/// the first `(` or `:` after which a configured value may follow.
fn reason(e: &str) -> String {
    let cut = e.find([':', '(']).map(|i| &e[..i]).unwrap_or(e).trim();
    if cut.is_empty() {
        "the request failed".into()
    } else {
        cut.to_string()
    }
}

/// Run every check that `identity` calls for.
pub fn connection_test(identity: &ResolvedIdentity, http: Arc<dyn Http>) -> Vec<Line> {
    let mut out = Vec::new();
    let fetch = ViaHttp(http.clone());
    let provider = identity.provider.word();

    // 1 and 2: the metadata and the keys.
    let discovery = match discover(identity, &fetch) {
        Ok(d) => {
            out.push(Line::new(Status::Ok, format!("discovery: the {provider} metadata names the configured issuer")));
            match crate::jwks::JwksFetcher::fetch(&fetch, &d.jwks_uri) {
                Ok(text) => {
                    let n = serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("keys").and_then(|k| k.as_array()).map(Vec::len)).unwrap_or(0);
                    out.push(if n > 0 {
                        Line::new(Status::Ok, format!("keys: {n} signing key(s) published"))
                    } else {
                        Line::new(Status::Fail, "keys: the key set has no keys")
                    });
                }
                Err(e) => out.push(Line::new(Status::Fail, format!("keys: not read ({})", reason(&e)))),
            }
            Some(d)
        }
        Err(e) => {
            let why = if e.contains("names the issuer") {
                "it names another issuer: check the provider's settings".to_string()
            } else if e.contains("not https") {
                "an address in it is not https".to_string()
            } else {
                // "the provider's metadata (<address>): <why>": the why, without the address.
                format!("not read ({})", reason(e.split_once("): ").map(|(_, why)| why).unwrap_or("the request failed")))
            };
            out.push(Line::new(Status::Fail, format!("discovery: the {provider} metadata: {why}")));
            out.push(Line::new(Status::Skip, "keys: the metadata was not read"));
            None
        }
    };

    let adapter = match adapter(identity, discovery.as_ref(), http.clone()) {
        Ok(a) => Some(a),
        Err(e) => {
            out.push(Line::new(Status::Fail, format!("adapter: {}", reason(&e))));
            None
        }
    };

    // 3: in-app Cognito accepts the app client.
    if identity.provider == Provider::Cognito && identity.flow == IdentityFlow::InApp {
        match &adapter {
            Some(a) => match a.sign_in(&format!("prc-check-{}", random_token(9)), &random_token(18)) {
                Err(IdpError::Rejected) => out.push(Line::new(Status::Ok, "sign-in: an unknown user is refused (the app client and its secret are accepted)")),
                Err(IdpError::Invalid(_)) => out.push(Line::new(Status::Fail, "sign-in: the provider refused the request itself (the app client, its secret or the user pool)")),
                Err(IdpError::Unavailable(e)) => out.push(Line::new(Status::Fail, format!("sign-in: the provider is not reachable ({})", reason(&e)))),
                Err(other) => out.push(Line::new(Status::Fail, format!("sign-in: an unexpected answer ({})", reason(&other.to_string())))),
                Ok(_) => out.push(Line::new(Status::Fail, "sign-in: a random user was signed in")),
            },
            None => out.push(Line::new(Status::Skip, "sign-in: no adapter")),
        }
    } else {
        out.push(Line::new(Status::Skip, format!("sign-in: the {} flow signs in on the provider's page", identity.flow.word())));
    }

    // 4: the hosted flow's authorise page.
    let hosted = identity.flow == IdentityFlow::Hosted || (identity.provider == Provider::Cognito && identity.settings.contains_key("domain"));
    match (&adapter, hosted) {
        (Some(a), true) => {
            let (_, challenge) = pkce();
            match a.start_hosted("https://localhost/auth/hosted/callback", &random_token(16), &challenge, &random_token(16)) {
                Ok(url) => match http.send(&HttpRequest::get(&url, &[])) {
                    Ok(r) if r.status < 500 => out.push(Line::new(Status::Ok, format!("hosted: the authorise page answers (HTTP {})", r.status))),
                    Ok(r) => out.push(Line::new(Status::Fail, format!("hosted: the authorise page answers HTTP {}", r.status))),
                    Err(e) => out.push(Line::new(Status::Fail, format!("hosted: the authorise page is not reachable ({})", reason(&e)))),
                },
                Err(e) => out.push(Line::new(Status::Fail, format!("hosted: no authorise address ({})", reason(&e.to_string())))),
            }
        }
        (None, true) => out.push(Line::new(Status::Skip, "hosted: no adapter")),
        (_, false) => out.push(Line::new(Status::Skip, "hosted: not configured")),
    }

    // 5: reCAPTCHA's secret.
    match &identity.recaptcha {
        Some(r) => {
            let endpoint = r.endpoint.clone().unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
            let req = HttpRequest::post_form(&endpoint, &[], &[("secret", r.secret.expose()), ("response", "prc-connection-test")]);
            match http.send(&req) {
                Ok(resp) => {
                    let codes: Vec<String> = serde_json::from_str::<Value>(&resp.body)
                        .ok()
                        .and_then(|v| v.get("error-codes").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()))
                        .unwrap_or_default();
                    if codes.iter().any(|c| c == "invalid-input-secret" || c == "missing-input-secret") {
                        out.push(Line::new(Status::Fail, "reCAPTCHA: Google does not accept the secret (invalid-input-secret)"));
                    } else if codes.iter().any(|c| c == "invalid-input-response") {
                        out.push(Line::new(Status::Ok, "reCAPTCHA: the secret is accepted (a made-up token is invalid-input-response)"));
                    } else {
                        out.push(Line::new(Status::Fail, format!("reCAPTCHA: an unexpected answer (HTTP {}, {} error code(s))", resp.status, codes.len())));
                    }
                }
                Err(e) => out.push(Line::new(Status::Fail, format!("reCAPTCHA: siteverify is not reachable ({})", reason(&e)))),
            }
        }
        None => out.push(Line::new(Status::Skip, "reCAPTCHA: not configured")),
    }
    out
}

/// `rcrun backend auth test`: `backend_dir`'s `backend.toml`, resolved as for development (a literal
/// secret is a warning there, not an error), then [`connection_test`]. `Err` is why there is nothing
/// to test.
pub fn test_backend(backend_dir: &std::path::Path, http: Arc<dyn Http>) -> Result<Vec<Line>, Vec<String>> {
    let path = backend_dir.join("backend.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| vec![format!("{}: {e}", path.display())])?;
    // Asked first: without the section the rest of the file may not resolve on its own (no [auth]).
    if !text.parse::<toml::Table>().is_ok_and(|t| t.contains_key("identity")) {
        return Err(vec!["backend.toml has no [identity] section: there is no identity provider to test".to_string()]);
    }
    let resolved = crate::config::ServerConfig::from_toml(&text).map_err(|e| vec![e])?.resolve(crate::config::RunMode::Development, backend_dir)?;
    let identity = resolved.identity.ok_or_else(|| vec!["backend.toml has no [identity] section: there is no identity provider to test".to_string()])?;
    Ok(connection_test(&identity, http))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reason_stops_before_a_value_could_follow() {
        assert_eq!(reason("connection refused: tcp connect error 127.0.0.1:1"), "connection refused");
        assert_eq!(reason("HTTP 404"), "HTTP 404");
        assert_eq!(reason(": x"), "the request failed");
    }
}
