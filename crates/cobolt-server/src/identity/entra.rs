// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Microsoft Entra — the hosted flow, and External ID's native authentication as an opt-in
//! (WASM-009 R8, R8b, plan §3.3, §6).
//!
//! **Workforce tenants** have one in-app flow, ROPC, which Microsoft says not to use and which
//! fails for MFA, passwordless and federated accounts; so the flow is **hosted**, at the addresses
//! the tenant's metadata names, with PKCE. Entra has no revocation endpoint: signing out ends the
//! application's session, and the provider's `logout` ends the browser's.
//!
//! **External ID tenants** have a *native authentication* API (`ciamlogin.com`, no CORS, so only a
//! backend can call it), used when `flow = "native"`: sign-in (`initiate` → `challenge` → `token`,
//! with the e-mail or SMS second factor through `introspect`), sign-up (`signup/v1.0/start` →
//! `challenge` → `continue`) and the password reset (`resetpassword/v1.0/start` → `challenge` →
//! `continue` → `submit` → `poll_completion`). Every step answers a `continuation_token` for the
//! next. An answer of `challenge_type = "redirect"` — the app cannot handle what the user needs —
//! and `registration_required` are [`IdpError::Unsupported`], which the control answers with the
//! hosted flow.
//!
//! The two failures that tell a user from a typo — `AADSTS50034` (no such user) and `AADSTS50126`
//! (bad password), `user_not_found` — are the same [`IdpError::Rejected`].

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::client::{Http, HttpRequest};
use super::oidc::Oidc;
use super::{Capabilities, Challenge, ChallengeResponse, Flow, IdentityProvider, IdpError, MfaChoice, MfaKind, SignIn, SignUp, SignUpRequest, Tokens};

/// What this app can handle, as the native API asks to be told.
const CHALLENGE_TYPES: &str = "oob password redirect";

pub struct Entra {
    oidc: Oidc,
    http: Arc<dyn Http>,
    /// `https://{subdomain}.ciamlogin.com/{subdomain}.onmicrosoft.com`, when the native API is on.
    native_base: Option<String>,
    external: bool,
}

impl Entra {
    /// `native_base` turns the External ID native API on (`flow = "native"`).
    pub fn new(http: Arc<dyn Http>, oidc: Oidc, external: bool, native_base: Option<String>) -> Self {
        Entra { oidc, http, native_base: native_base.map(|b| b.trim_end_matches('/').to_string()), external }
    }

    fn native(&self) -> Result<&str, IdpError> {
        self.native_base.as_deref().ok_or(IdpError::Unsupported)
    }

    /// One native call. A 200 is the answer; an error body is read for what it means.
    fn post(&self, path: &str, fields: &[(&str, &str)]) -> Result<Value, IdpError> {
        let base = self.native()?;
        let mut all: Vec<(&str, &str)> = vec![("client_id", self.oidc.client_id.as_str())];
        all.extend_from_slice(fields);
        let resp = self.http.send(&HttpRequest::post_form(&format!("{base}{path}"), &[], &all)).map_err(|e| IdpError::Unavailable(format!("Entra native {path}: {e}")))?;
        let v = resp.json().unwrap_or(Value::Null);
        match resp.status {
            200 => {
                if v.get("challenge_type").and_then(|c| c.as_str()) == Some("redirect") {
                    // The user needs something only the provider's page does.
                    return Err(IdpError::Unsupported);
                }
                Ok(v)
            }
            429 => Err(IdpError::Throttled),
            s if s >= 500 || s == 0 => Err(IdpError::Unavailable(format!("Entra native {path} answered {s}"))),
            _ => Err(NativeError::of(&v).into_idp(path)),
        }
    }

    fn ct(v: &Value) -> Result<String, IdpError> {
        v.get("continuation_token").and_then(|c| c.as_str()).map(str::to_owned).ok_or_else(|| IdpError::Unavailable("Entra native answered no continuation token".into()))
    }

    fn tokens(v: &Value) -> Result<Tokens, IdpError> {
        Ok(Tokens {
            access: v.get("access_token").and_then(|t| t.as_str()).ok_or_else(|| IdpError::Unavailable("Entra answered no access token".into()))?.to_string(),
            id: v.get("id_token").and_then(|t| t.as_str()).map(str::to_owned),
            refresh: v.get("refresh_token").and_then(|t| t.as_str()).map(str::to_owned),
            expires_in: v.get("expires_in").and_then(|t| t.as_u64()).unwrap_or(3600),
        })
    }

    /// The second factor (`mfa_required`): the methods the user has, then a code sent to one.
    fn second_factor(&self, ct: &str) -> Result<SignIn, IdpError> {
        let v = self.post("/oauth2/v2.0/introspect", &[("continuation_token", ct)])?;
        let ct = Self::ct(&v)?;
        let mut ids: Vec<(MfaChoice, String)> = Vec::new();
        for m in v.get("methods").and_then(|m| m.as_array()).cloned().unwrap_or_default() {
            let channel = match m.get("challenge_channel").and_then(|c| c.as_str()) {
                Some("email") => MfaChoice::Email,
                Some("sms") => MfaChoice::Sms,
                _ => continue,
            };
            if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                if !ids.iter().any(|(c, _)| *c == channel) {
                    ids.push((channel, id.to_string()));
                }
            }
        }
        match ids.len() {
            0 => Err(IdpError::Unsupported),
            1 => self.send_code(&ct, &ids[0].1, ids[0].0),
            _ => {
                let session = json!({"ct": ct, "ids": ids.iter().map(|(c, id)| (c.wire_name().to_string(), json!(id))).collect::<serde_json::Map<_, _>>()}).to_string();
                Ok(SignIn::Challenge(Challenge { kind: MfaKind::Choice(ids.iter().map(|(c, _)| *c).collect()), name: "mfa_choose".into(), session }))
            }
        }
    }

    fn send_code(&self, ct: &str, id: &str, channel: MfaChoice) -> Result<SignIn, IdpError> {
        let v = self.post("/oauth2/v2.0/challenge", &[("continuation_token", ct), ("challenge_type", CHALLENGE_TYPES), ("id", id)])?;
        let destination = v.get("challenge_target_label").and_then(|l| l.as_str()).unwrap_or("").to_string();
        let kind = match channel {
            MfaChoice::Sms => MfaKind::Sms { destination },
            _ => MfaKind::Email { destination },
        };
        Ok(SignIn::Challenge(Challenge { kind, name: "mfa_oob".into(), session: Self::ct(&v)? }))
    }
}

/// An error body of the native API.
struct NativeError {
    error: String,
    suberror: String,
    codes: Vec<u64>,
    continuation: Option<String>,
}

impl NativeError {
    fn of(v: &Value) -> NativeError {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        NativeError {
            error: s("error"),
            suberror: s("suberror"),
            codes: v.get("error_codes").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default(),
            continuation: v.get("continuation_token").and_then(|c| c.as_str()).map(str::to_owned),
        }
    }

    fn into_idp(self, path: &str) -> IdpError {
        match (self.error.as_str(), self.suberror.as_str()) {
            (_, "password_too_weak" | "password_too_short" | "password_too_long" | "password_recently_used" | "password_banned" | "password_is_invalid") => {
                IdpError::Invalid("The password does not meet the requirements.".into())
            }
            (_, "attribute_validation_failed") | ("invalid_attributes", _) => IdpError::Invalid("One of the values is not accepted.".into()),
            (_, "registration_required") | ("unsupported_challenge_type", _) | ("attributes_required", _) | ("credential_required", _) => IdpError::Unsupported,
            ("unauthorized_client" | "invalid_client" | "unsupported_grant_type" | "invalid_scope", _) => IdpError::Unavailable(format!("Entra native {path}: {} (check the app registration)", self.error)),
            // user_not_found, invalid_grant (AADSTS50034, AADSTS50126), user_already_exists,
            // invalid_oob_value, expired_token: all one answer.
            _ => {
                let _ = &self.codes;
                IdpError::Rejected
            }
        }
    }
}

impl IdentityProvider for Entra {
    fn name(&self) -> &'static str {
        if self.external {
            "entra-external"
        } else {
            "entra"
        }
    }

    fn flow(&self) -> Flow {
        if self.native_base.is_some() {
            Flow::Native
        } else {
            Flow::Hosted
        }
    }

    fn capabilities(&self) -> Capabilities {
        let native = self.native_base.is_some();
        Capabilities {
            sign_in: native,
            sign_up: native,
            confirm: native,
            reset: native,
            mfa_email: native,
            refresh: true,
            hosted: true,
            ..Capabilities::default()
        }
    }

    fn sign_in(&self, username: &str, password: &str) -> Result<SignIn, IdpError> {
        self.native()?;
        let v = self.post("/oauth2/v2.0/initiate", &[("username", username), ("challenge_type", CHALLENGE_TYPES)])?;
        let v = self.post("/oauth2/v2.0/challenge", &[("continuation_token", &Self::ct(&v)?), ("challenge_type", CHALLENGE_TYPES)])?;
        if v.get("challenge_type").and_then(|c| c.as_str()) != Some("password") {
            // An e-mail one-time passcode account: the provider's page signs it in.
            return Err(IdpError::Unsupported);
        }
        let ct = Self::ct(&v)?;
        let base = self.native()?;
        let fields = [("client_id", self.oidc.client_id.as_str()), ("grant_type", "password"), ("continuation_token", ct.as_str()), ("password", password), ("scope", self.oidc.scopes.as_str())];
        let resp = self.http.send(&HttpRequest::post_form(&format!("{base}/oauth2/v2.0/token"), &[], &fields)).map_err(|e| IdpError::Unavailable(format!("Entra native token: {e}")))?;
        let v = resp.json().unwrap_or(Value::Null);
        match resp.status {
            200 => Ok(SignIn::Tokens(Self::tokens(&v)?)),
            429 => Err(IdpError::Throttled),
            s if s >= 500 || s == 0 => Err(IdpError::Unavailable(format!("Entra native token answered {s}"))),
            _ => {
                let e = NativeError::of(&v);
                match (e.suberror.as_str(), e.continuation.clone()) {
                    ("mfa_required", Some(ct)) => self.second_factor(&ct),
                    _ => Err(e.into_idp("/oauth2/v2.0/token")),
                }
            }
        }
    }

    fn respond_to_challenge(&self, a: &ChallengeResponse) -> Result<SignIn, IdpError> {
        self.native()?;
        match a.name.as_str() {
            "mfa_choose" => {
                let pick = a.choice.ok_or(IdpError::Rejected)?;
                let s: Value = serde_json::from_str(&a.session).map_err(|_| IdpError::Rejected)?;
                let ct = s.get("ct").and_then(|c| c.as_str()).ok_or(IdpError::Rejected)?;
                let id = s.pointer(&format!("/ids/{}", pick.wire_name())).and_then(|i| i.as_str()).ok_or(IdpError::Rejected)?;
                self.send_code(ct, id, pick)
            }
            "mfa_oob" => {
                let v = self.post("/oauth2/v2.0/token", &[("grant_type", "mfa_oob"), ("oob", &a.code), ("continuation_token", &a.session), ("scope", &self.oidc.scopes)])?;
                Ok(SignIn::Tokens(Self::tokens(&v)?))
            }
            _ => Err(IdpError::Unsupported),
        }
    }

    fn sign_up(&self, r: &SignUpRequest) -> Result<SignUp, IdpError> {
        self.native()?;
        let mut attributes = serde_json::Map::new();
        for (name, value) in [("givenName", &r.given_name), ("surname", &r.family_name)] {
            if !value.is_empty() {
                attributes.insert(name.to_string(), json!(value));
            }
        }
        let attributes = Value::Object(attributes).to_string();
        let mut fields = vec![("username", r.username.as_str()), ("password", r.password.as_str()), ("challenge_type", CHALLENGE_TYPES)];
        if attributes != "{}" {
            fields.push(("attributes", attributes.as_str()));
        }
        let v = self.post("/signup/v1.0/start", &fields)?;
        let v = self.post("/signup/v1.0/challenge", &[("continuation_token", &Self::ct(&v)?), ("challenge_type", CHALLENGE_TYPES)])?;
        let destination = v.get("challenge_target_label").and_then(|l| l.as_str()).unwrap_or("").to_string();
        Ok(SignUp::ConfirmationRequired { destination, session: Self::ct(&v)? })
    }

    fn confirm_sign_up(&self, _username: &str, code: &str, session: &str) -> Result<(), IdpError> {
        if session.is_empty() {
            return Err(IdpError::Rejected);
        }
        self.post("/signup/v1.0/continue", &[("continuation_token", session), ("grant_type", "oob"), ("oob", code)]).map(|_| ())
    }

    fn resend_code(&self, _username: &str, session: &str) -> Result<Option<String>, IdpError> {
        if session.is_empty() {
            return Err(IdpError::Rejected);
        }
        let v = self.post("/signup/v1.0/challenge", &[("continuation_token", session), ("challenge_type", CHALLENGE_TYPES)])?;
        Ok(Some(Self::ct(&v)?))
    }

    fn forgot_password(&self, username: &str) -> Result<Option<String>, IdpError> {
        self.native()?;
        let v = self.post("/resetpassword/v1.0/start", &[("username", username), ("challenge_type", CHALLENGE_TYPES)])?;
        let v = self.post("/resetpassword/v1.0/challenge", &[("continuation_token", &Self::ct(&v)?), ("challenge_type", CHALLENGE_TYPES)])?;
        Ok(Some(Self::ct(&v)?))
    }

    fn confirm_forgot_password(&self, _username: &str, code: &str, new_password: &str, session: &str) -> Result<(), IdpError> {
        if session.is_empty() {
            return Err(IdpError::Rejected);
        }
        let v = self.post("/resetpassword/v1.0/continue", &[("continuation_token", session), ("grant_type", "oob"), ("oob", code)])?;
        let v = self.post("/resetpassword/v1.0/submit", &[("continuation_token", &Self::ct(&v)?), ("new_password", new_password)])?;
        let mut ct = Self::ct(&v)?;
        let wait = Duration::from_secs(v.get("poll_interval").and_then(|p| p.as_u64()).unwrap_or(1).clamp(1, 5));
        // The reset completes asynchronously; a few polls, then the provider is too slow to wait for.
        for attempt in 0..10 {
            let v = self.post("/resetpassword/v1.0/poll_completion", &[("continuation_token", &ct)])?;
            match v.get("status").and_then(|s| s.as_str()) {
                Some("succeeded") => return Ok(()),
                Some("failed") => return Err(IdpError::Rejected),
                _ => {
                    if let Ok(next) = Self::ct(&v) {
                        ct = next;
                    }
                    if attempt < 9 {
                        std::thread::sleep(wait);
                    }
                }
            }
        }
        Err(IdpError::Unavailable("Entra did not complete the password reset in time".into()))
    }

    fn refresh(&self, refresh_token: &str, _username: &str) -> Result<Tokens, IdpError> {
        self.oidc.refresh(refresh_token)
    }

    fn sign_out(&self, _access_token: &str, _refresh_token: Option<&str>) -> Result<(), IdpError> {
        // No revocation endpoint: the application ends its own session; the provider's `logout`
        // ends the browser's.
        Ok(())
    }

    fn start_hosted(&self, redirect_uri: &str, state: &str, code_challenge: &str, nonce: &str) -> Result<String, IdpError> {
        Ok(self.oidc.start(redirect_uri, state, code_challenge, nonce))
    }

    fn finish_hosted(&self, code: &str, redirect_uri: &str, code_verifier: &str) -> Result<Tokens, IdpError> {
        self.oidc.exchange(code, redirect_uri, code_verifier)
    }
}
