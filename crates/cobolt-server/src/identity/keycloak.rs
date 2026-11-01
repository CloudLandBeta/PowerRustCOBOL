// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Keycloak — the hosted flow (WASM-009 R8, plan §3.2, §6).
//!
//! Keycloak's direct grant (ROPC) "MUST NOT be used" by its own guide (RFC 9700) and supports
//! neither self-registration nor required actions; registration and "forgot password" are realm
//! settings that turn on **browser pages**, with no public JSON API. So every in-app operation is
//! [`IdpError::Unsupported`] and the control uses the realm's page. What the backend does for
//! Keycloak is the code exchange, the refresh and the revocation — at the addresses the realm's
//! metadata names.

use super::oidc::Oidc;
use super::{Capabilities, ChallengeResponse, Flow, IdentityProvider, IdpError, SignIn, SignUp, SignUpRequest, Tokens};

pub struct Keycloak {
    oidc: Oidc,
}

impl Keycloak {
    pub fn new(oidc: Oidc) -> Self {
        Keycloak { oidc }
    }
}

impl IdentityProvider for Keycloak {
    fn name(&self) -> &'static str {
        "keycloak"
    }

    fn flow(&self) -> Flow {
        Flow::Hosted
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities { refresh: true, revoke: self.oidc.revoke_url.is_some(), hosted: true, ..Capabilities::default() }
    }

    fn sign_in(&self, _: &str, _: &str) -> Result<SignIn, IdpError> {
        Err(IdpError::Unsupported)
    }

    fn respond_to_challenge(&self, _: &ChallengeResponse) -> Result<SignIn, IdpError> {
        Err(IdpError::Unsupported)
    }

    fn sign_up(&self, _: &SignUpRequest) -> Result<SignUp, IdpError> {
        Err(IdpError::Unsupported)
    }

    fn confirm_sign_up(&self, _: &str, _: &str, _: &str) -> Result<(), IdpError> {
        Err(IdpError::Unsupported)
    }

    fn resend_code(&self, _: &str, _: &str) -> Result<Option<String>, IdpError> {
        Err(IdpError::Unsupported)
    }

    fn forgot_password(&self, _: &str) -> Result<Option<String>, IdpError> {
        Err(IdpError::Unsupported)
    }

    fn confirm_forgot_password(&self, _: &str, _: &str, _: &str, _: &str) -> Result<(), IdpError> {
        Err(IdpError::Unsupported)
    }

    fn refresh(&self, refresh_token: &str, _username: &str) -> Result<Tokens, IdpError> {
        self.oidc.refresh(refresh_token)
    }

    fn sign_out(&self, _access_token: &str, refresh_token: Option<&str>) -> Result<(), IdpError> {
        match refresh_token.filter(|t| !t.is_empty()) {
            Some(t) => self.oidc.revoke(t),
            None => Ok(()),
        }
    }

    fn start_hosted(&self, redirect_uri: &str, state: &str, code_challenge: &str, nonce: &str) -> Result<String, IdpError> {
        Ok(self.oidc.start(redirect_uri, state, code_challenge, nonce))
    }

    fn finish_hosted(&self, code: &str, redirect_uri: &str, code_verifier: &str) -> Result<Tokens, IdpError> {
        self.oidc.exchange(code, redirect_uri, code_verifier)
    }
}
