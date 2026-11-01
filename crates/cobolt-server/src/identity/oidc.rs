// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The hosted OpenID Connect flow, shared by the providers whose sign-in is their own page
//! (Keycloak, Microsoft Entra, Cognito's managed login) — WASM-009 R8b, D5.
//!
//! The backend builds the address of that page (authorization code with **PKCE, S256**, plus
//! `state` and `nonce`); when the page sends the browser back with a `code`, it exchanges it at
//! the token endpoint — with the client secret when the client has one, which only the backend
//! holds. The same endpoint refreshes; the revocation endpoint, where the provider has one, ends a
//! refresh token. The addresses come from the provider's metadata ([`crate::config::discover`]).

use std::sync::Arc;

use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD};
use base64::Engine;
use sha2::{Digest, Sha256};

use super::client::{percent, Http, HttpRequest};
use super::{IdpError, Tokens};
use crate::config::Discovery;

/// A PKCE verifier (43 characters from 32 random bytes) and its S256 challenge (RFC 7636).
pub fn pkce() -> (String, String) {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("the operating system's random source");
    let verifier = URL_SAFE_NO_PAD.encode(b);
    let challenge = s256(&verifier);
    (verifier, challenge)
}

/// `BASE64URL(SHA256(verifier))`.
pub fn s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// `n` random bytes, base64url: a `state`, a `nonce`, a handle.
pub fn random_token(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("the operating system's random source");
    URL_SAFE_NO_PAD.encode(b)
}

pub struct Oidc {
    pub http: Arc<dyn Http>,
    pub authorize_url: String,
    pub token_url: String,
    pub revoke_url: Option<String>,
    pub end_session_url: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: String,
}

impl Oidc {
    pub fn from_discovery(http: Arc<dyn Http>, d: &Discovery, client_id: &str, client_secret: Option<&str>, scopes: &str) -> Oidc {
        Oidc {
            http,
            authorize_url: d.authorization_endpoint.clone(),
            token_url: d.token_endpoint.clone(),
            revoke_url: d.revocation_endpoint.clone(),
            end_session_url: d.end_session_endpoint.clone(),
            client_id: client_id.to_owned(),
            client_secret: client_secret.map(str::to_owned),
            scopes: scopes.to_owned(),
        }
    }

    pub fn start(&self, redirect_uri: &str, state: &str, code_challenge: &str, nonce: &str) -> String {
        let sep = if self.authorize_url.contains('?') { '&' } else { '?' };
        format!(
            "{}{sep}response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
            self.authorize_url,
            percent(&self.client_id),
            percent(redirect_uri),
            percent(&self.scopes),
            percent(state),
            percent(nonce),
            percent(code_challenge)
        )
    }

    /// `client_secret_basic` (RFC 6749 §2.3.1: the id and the secret form-encoded, then base64).
    fn basic(&self) -> Vec<(String, String)> {
        match &self.client_secret {
            Some(s) => vec![("Authorization".to_string(), format!("Basic {}", B64.encode(format!("{}:{}", percent(&self.client_id), percent(s)))))],
            None => Vec::new(),
        }
    }

    fn token_call(&self, fields: &[(&str, &str)]) -> Result<Tokens, IdpError> {
        let headers = self.basic();
        let hs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = self.http.send(&HttpRequest::post_form(&self.token_url, &hs, fields)).map_err(|e| IdpError::Unavailable(format!("token endpoint: {e}")))?;
        match resp.status {
            200 => {}
            429 => return Err(IdpError::Throttled),
            s if s >= 500 || s == 0 => return Err(IdpError::Unavailable(format!("the token endpoint answered {s}"))),
            // invalid_grant, invalid_client, …: the code, the verifier or the token was not accepted.
            _ => return Err(IdpError::Rejected),
        }
        let v = resp.json().ok_or_else(|| IdpError::Unavailable("the token endpoint answered no JSON".into()))?;
        Ok(Tokens {
            access: v.get("access_token").and_then(|t| t.as_str()).ok_or(IdpError::Rejected)?.to_string(),
            id: v.get("id_token").and_then(|t| t.as_str()).map(str::to_owned),
            refresh: v.get("refresh_token").and_then(|t| t.as_str()).map(str::to_owned),
            expires_in: v.get("expires_in").and_then(|t| t.as_u64()).unwrap_or(3600),
        })
    }

    pub fn exchange(&self, code: &str, redirect_uri: &str, verifier: &str) -> Result<Tokens, IdpError> {
        self.token_call(&[("grant_type", "authorization_code"), ("client_id", self.client_id.as_str()), ("code", code), ("redirect_uri", redirect_uri), ("code_verifier", verifier)])
    }

    pub fn refresh(&self, refresh_token: &str) -> Result<Tokens, IdpError> {
        let mut t = self.token_call(&[("grant_type", "refresh_token"), ("client_id", self.client_id.as_str()), ("refresh_token", refresh_token), ("scope", self.scopes.as_str())])?;
        // A provider without rotation does not repeat the refresh token.
        if t.refresh.is_none() {
            t.refresh = Some(refresh_token.to_string());
        }
        Ok(t)
    }

    /// Revoke a refresh token (RFC 7009); nothing to do where the provider has no endpoint.
    pub fn revoke(&self, refresh_token: &str) -> Result<(), IdpError> {
        let Some(url) = &self.revoke_url else { return Ok(()) };
        let headers = self.basic();
        let hs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = self
            .http
            .send(&HttpRequest::post_form(url, &hs, &[("token", refresh_token), ("token_type_hint", "refresh_token"), ("client_id", self.client_id.as_str())]))
            .map_err(|e| IdpError::Unavailable(format!("revocation endpoint: {e}")))?;
        // RFC 7009: 200 even for a token that was unknown.
        if resp.status >= 500 {
            return Err(IdpError::Unavailable(format!("the revocation endpoint answered {}", resp.status)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_is_s256_as_rfc_7636_appendix_b_computes_it() {
        assert_eq!(s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let (v, c) = pkce();
        assert_eq!(v.len(), 43, "32 bytes, base64url");
        assert_eq!(c, s256(&v));
        assert_ne!(pkce().0, v);
    }

    struct Nobody;
    impl Http for Nobody {
        fn send(&self, _: &HttpRequest) -> Result<super::super::client::HttpResponse, String> {
            Err("offline".into())
        }
    }

    #[test]
    fn the_authorize_address_carries_s256_state_and_nonce_and_no_secret() {
        let o = Oidc {
            http: Arc::new(Nobody),
            authorize_url: "https://kc.example.test/realms/shop/protocol/openid-connect/auth".into(),
            token_url: String::new(),
            revoke_url: None,
            end_session_url: None,
            client_id: "web app".into(),
            client_secret: Some("client-secret-value".into()),
            scopes: "openid profile".into(),
        };
        let url = o.start("https://app.example.test/", "st", "ch", "no");
        assert!(url.starts_with("https://kc.example.test/realms/shop/protocol/openid-connect/auth?response_type=code&client_id=web%20app"), "{url}");
        for part in ["redirect_uri=https%3A%2F%2Fapp.example.test%2F", "scope=openid%20profile", "state=st", "nonce=no", "code_challenge=ch", "code_challenge_method=S256"] {
            assert!(url.contains(part), "{part}: {url}");
        }
        assert!(!url.contains("client-secret-value"));
        assert!(matches!(o.exchange("c", "r", "v"), Err(IdpError::Unavailable(_))), "an unreachable provider is not the user's fault");
    }
}
