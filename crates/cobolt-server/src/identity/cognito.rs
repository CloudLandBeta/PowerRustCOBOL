// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Amazon Cognito user pools — in-app by default, hosted through the managed login (WASM-009
//! R8, plan §3.1, §6).
//!
//! Every self-service operation is a public JSON call (`ClientId` required, no AWS credentials):
//! `POST https://cognito-idp.{region}.amazonaws.com/` with `X-Amz-Target:
//! AWSCognitoIdentityProviderService.<Operation>`. When the app client has a secret, every call
//! that names a user also carries `SECRET_HASH`, the base64 HMAC-SHA256 of `username + client_id`
//! keyed with the client secret.
//!
//! **Password sign-in is `USER_PASSWORD_AUTH`**, which the app client must allow. SRP is not in
//! this cut (plan §12): the password travels to the backend and from it to Cognito over TLS, and
//! never into a log.
//!
//! **What a failure tells the user** is decided by [`classify`], not by Cognito:
//! `UserNotFoundException`, `NotAuthorizedException`, `UserNotConfirmedException`,
//! `PasswordResetRequiredException` and a locked account are all [`IdpError::Rejected`] (R10).

use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;

use super::client::{percent, Http, HttpRequest};
use super::oidc::Oidc;
use super::{Capabilities, Challenge, ChallengeResponse, Flow, IdentityProvider, IdpError, MfaChoice, MfaKind, SignIn, SignUp, SignUpRequest, Tokens};

pub struct Cognito {
    http: Arc<dyn Http>,
    endpoint: String,
    client_id: String,
    client_secret: Option<String>,
    flow: Flow,
    /// The managed login (`https://<prefix>.auth.<region>.amazoncognito.com`), for the hosted flow.
    hosted: Option<Oidc>,
    /// `GetTokensFromRefreshToken` instead of `REFRESH_TOKEN_AUTH`: the app client rotates refresh tokens.
    rotation: bool,
    /// The name an authenticator app shows for the account (`otpauth://totp/<issuer>:<user>`).
    totp_issuer: String,
}

pub struct CognitoSettings {
    pub region: String,
    pub user_pool_id: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub endpoint: Option<String>,
    pub domain: Option<String>,
    pub scopes: Option<String>,
    pub flow: Flow,
    pub refresh_rotation: bool,
    pub totp_issuer: Option<String>,
}

impl Cognito {
    pub fn new(http: Arc<dyn Http>, s: CognitoSettings) -> Self {
        let client_secret = s.client_secret.filter(|x| !x.is_empty());
        let scopes = s.scopes.unwrap_or_else(|| "openid email profile".to_string());
        let hosted = s.domain.as_ref().map(|d| {
            let d = d.trim_end_matches('/');
            Oidc {
                http: http.clone(),
                authorize_url: format!("{d}/oauth2/authorize"),
                token_url: format!("{d}/oauth2/token"),
                revoke_url: Some(format!("{d}/oauth2/revoke")),
                end_session_url: Some(format!("{d}/logout")),
                client_id: s.client_id.clone(),
                client_secret: client_secret.clone(),
                scopes: scopes.clone(),
            }
        });
        Cognito {
            endpoint: s.endpoint.unwrap_or_else(|| format!("https://cognito-idp.{}.amazonaws.com/", s.region)),
            http,
            client_id: s.client_id,
            client_secret,
            flow: s.flow,
            hosted,
            rotation: s.refresh_rotation,
            totp_issuer: s.totp_issuer.unwrap_or(s.user_pool_id),
        }
    }

    /// `SECRET_HASH`: base64(HMAC-SHA256(client secret, username ‖ client id)).
    pub fn secret_hash(&self, username: &str) -> Option<String> {
        let secret = self.client_secret.as_ref()?;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
        mac.update(username.as_bytes());
        mac.update(self.client_id.as_bytes());
        Some(B64.encode(mac.finalize().into_bytes()))
    }

    fn in_app(&self) -> Result<(), IdpError> {
        if self.flow == Flow::InApp {
            Ok(())
        } else {
            Err(IdpError::Unsupported)
        }
    }

    fn call(&self, target: &str, body: Value) -> Result<Value, IdpError> {
        let req = HttpRequest::post_amz(&self.endpoint, &format!("AWSCognitoIdentityProviderService.{target}"), &body);
        let resp = self.http.send(&req).map_err(|e| IdpError::Unavailable(format!("Cognito {target}: {e}")))?;
        let v = resp.json().unwrap_or(Value::Null);
        if (200..300).contains(&resp.status) {
            return Ok(v);
        }
        let kind = v.get("__type").and_then(|t| t.as_str()).map(|t| t.rsplit('#').next().unwrap_or(t).to_string()).unwrap_or_default();
        let message = v.get("message").or_else(|| v.get("Message")).and_then(|m| m.as_str()).unwrap_or("").to_string();
        Err(classify(target, resp.status, &kind, &message))
    }

    /// A body naming `username`, with `SecretHash` when the client has a secret.
    fn named(&self, username: &str, mut body: Value) -> Value {
        body["ClientId"] = json!(self.client_id);
        body["Username"] = json!(username);
        if let Some(h) = self.secret_hash(username) {
            body["SecretHash"] = json!(h);
        }
        body
    }

    fn tokens(v: &Value) -> Option<Tokens> {
        let r = v.get("AuthenticationResult")?;
        Some(Tokens {
            access: r.get("AccessToken")?.as_str()?.to_string(),
            id: r.get("IdToken").and_then(|t| t.as_str()).map(str::to_owned),
            refresh: r.get("RefreshToken").and_then(|t| t.as_str()).map(str::to_owned),
            expires_in: r.get("ExpiresIn").and_then(|t| t.as_u64()).unwrap_or(3600),
        })
    }

    /// Tokens, or the next step as the player's screens know it.
    fn sign_in_result(&self, v: &Value, username: &str) -> Result<SignIn, IdpError> {
        if let Some(t) = Self::tokens(v) {
            return Ok(SignIn::Tokens(t));
        }
        let name = v.get("ChallengeName").and_then(|c| c.as_str()).ok_or_else(|| IdpError::Unavailable("Cognito answered neither tokens nor a challenge".into()))?;
        let session = v.get("Session").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let param = |k: &str| v.get("ChallengeParameters").and_then(|p| p.get(k)).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let kind = match name {
            "SMS_MFA" => MfaKind::Sms { destination: param("CODE_DELIVERY_DESTINATION") },
            "EMAIL_OTP" | "EMAIL_MFA" => MfaKind::Email { destination: param("CODE_DELIVERY_DESTINATION") },
            "SOFTWARE_TOKEN_MFA" => MfaKind::Totp,
            "NEW_PASSWORD_REQUIRED" => MfaKind::NewPassword,
            "SELECT_MFA_TYPE" => {
                // `MFAS_CAN_CHOOSE` is a JSON array in a string: `["SMS_MFA","SOFTWARE_TOKEN_MFA"]`.
                let offered: Vec<String> = serde_json::from_str(&param("MFAS_CAN_CHOOSE")).unwrap_or_default();
                let choices: Vec<MfaChoice> = offered.iter().filter_map(|n| choice_of(n)).collect();
                if choices.is_empty() {
                    return Err(IdpError::Unsupported);
                }
                // The names as offered travel with the step, so the answer uses Cognito's own spelling.
                return Ok(SignIn::Challenge(Challenge { kind: MfaKind::Choice(choices), name: format!("SELECT_MFA_TYPE:{}", offered.join(",")), session }));
            }
            "MFA_SETUP" => {
                // Enrol an authenticator app: Cognito makes the secret, the user scans it.
                let a = self.call("AssociateSoftwareToken", json!({"Session": session}))?;
                let secret = a.get("SecretCode").and_then(|s| s.as_str()).ok_or_else(|| IdpError::Unavailable("AssociateSoftwareToken answered no secret".into()))?.to_string();
                let session = a.get("Session").and_then(|s| s.as_str()).unwrap_or(&session).to_string();
                let label = format!("{}:{}", self.totp_issuer, username);
                let otpauth_uri = format!("otpauth://totp/{}?secret={secret}&issuer={}", percent(&label), percent(&self.totp_issuer));
                return Ok(SignIn::Challenge(Challenge { kind: MfaKind::TotpSetup { secret, otpauth_uri }, name: "MFA_SETUP".into(), session }));
            }
            // CUSTOM_CHALLENGE, DEVICE_SRP_AUTH, PASSWORD_VERIFIER, …: the managed login handles them.
            _ => return Err(IdpError::Unsupported),
        };
        Ok(SignIn::Challenge(Challenge { kind, name: name.to_string(), session }))
    }
}

fn choice_of(name: &str) -> Option<MfaChoice> {
    match name {
        "SMS_MFA" => Some(MfaChoice::Sms),
        "SOFTWARE_TOKEN_MFA" => Some(MfaChoice::Totp),
        "EMAIL_OTP" | "EMAIL_MFA" => Some(MfaChoice::Email),
        _ => None,
    }
}

/// What a Cognito error means for the application. **Nothing here distinguishes an unknown user
/// from a wrong password.**
pub fn classify(target: &str, status: u16, kind: &str, message: &str) -> IdpError {
    match kind {
        // Credentials, user, account state: all one answer.
        "NotAuthorizedException" | "UserNotFoundException" | "UserNotConfirmedException" | "PasswordResetRequiredException" | "UserLambdaValidationException" | "InvalidLambdaResponseException" => IdpError::Rejected,
        // A code: wrong, expired, or for nobody.
        "CodeMismatchException" | "ExpiredCodeException" | "AliasExistsException" | "EnableSoftwareTokenMFAException" => IdpError::Rejected,
        "TooManyRequestsException" | "LimitExceededException" | "TooManyFailedAttemptsException" => IdpError::Throttled,
        // The user can fix this and learns nothing private from it.
        "InvalidPasswordException" => IdpError::Invalid(if message.is_empty() { "The password does not meet the requirements.".into() } else { message.to_string() }),
        "InvalidParameterException" => {
            // On these, Cognito is saying the account cannot recover by this route (no verified
            // e-mail or phone) — as private as "no such user".
            if matches!(target, "ForgotPassword" | "ResendConfirmationCode" | "InitiateAuth" | "RespondToAuthChallenge") {
                IdpError::Rejected
            } else {
                IdpError::Invalid("One of the values is not accepted.".into())
            }
        }
        // A sign-up with a name that exists: the service answers it like a success.
        "UsernameExistsException" => IdpError::Rejected,
        "ResourceNotFoundException" | "InvalidUserPoolConfigurationException" | "UnsupportedOperationException" | "InvalidSmsRoleAccessPolicyException" | "InvalidEmailRoleAccessPolicyException" => {
            IdpError::Unavailable(format!("Cognito {target}: {kind}: {message}"))
        }
        _ if status >= 500 || status == 0 => IdpError::Unavailable(format!("Cognito {target}: {status} {kind}")),
        _ => IdpError::Rejected,
    }
}

impl IdentityProvider for Cognito {
    fn name(&self) -> &'static str {
        "cognito"
    }

    fn flow(&self) -> Flow {
        self.flow
    }

    fn capabilities(&self) -> Capabilities {
        let in_app = self.flow == Flow::InApp;
        Capabilities {
            sign_in: in_app,
            sign_up: in_app,
            confirm: in_app,
            reset: in_app,
            new_password: in_app,
            mfa_totp: in_app,
            mfa_sms: in_app,
            mfa_email: in_app,
            mfa_setup: in_app,
            refresh: true,
            revoke: true,
            hosted: self.hosted.is_some(),
        }
    }

    fn sign_in(&self, username: &str, password: &str) -> Result<SignIn, IdpError> {
        self.in_app()?;
        let mut params = json!({"USERNAME": username, "PASSWORD": password});
        if let Some(h) = self.secret_hash(username) {
            params["SECRET_HASH"] = json!(h);
        }
        let v = self.call("InitiateAuth", json!({"AuthFlow": "USER_PASSWORD_AUTH", "ClientId": self.client_id, "AuthParameters": params}))?;
        self.sign_in_result(&v, username)
    }

    fn respond_to_challenge(&self, a: &ChallengeResponse) -> Result<SignIn, IdpError> {
        self.in_app()?;
        let mut session = a.session.clone();
        let mut responses = json!({"USERNAME": a.username});
        let (name, offered) = a.name.split_once(':').unwrap_or((a.name.as_str(), ""));
        match name {
            "SMS_MFA" => responses["SMS_MFA_CODE"] = json!(a.code),
            "SOFTWARE_TOKEN_MFA" => responses["SOFTWARE_TOKEN_MFA_CODE"] = json!(a.code),
            // The documentation names both spellings (plan §3.1): answer in the one the step used.
            "EMAIL_OTP" => responses["EMAIL_OTP_CODE"] = json!(a.code),
            "EMAIL_MFA" => responses["EMAIL_MFA_CODE"] = json!(a.code),
            "NEW_PASSWORD_REQUIRED" => {
                if a.new_password.is_empty() {
                    return Err(IdpError::Invalid("Enter a new password.".into()));
                }
                responses["NEW_PASSWORD"] = json!(a.new_password)
            }
            "SELECT_MFA_TYPE" => {
                let pick = a.choice.ok_or(IdpError::Rejected)?;
                let answer = offered.split(',').find(|n| choice_of(n) == Some(pick)).ok_or(IdpError::Rejected)?;
                responses["ANSWER"] = json!(answer);
            }
            "MFA_SETUP" => {
                if !a.setup {
                    return Err(IdpError::Rejected);
                }
                let v = self.call("VerifySoftwareToken", json!({"Session": session, "UserCode": a.code, "FriendlyDeviceName": "Authenticator"}))?;
                if v.get("Status").and_then(|s| s.as_str()) != Some("SUCCESS") {
                    return Err(IdpError::Rejected);
                }
                session = v.get("Session").and_then(|s| s.as_str()).unwrap_or(&session).to_string();
            }
            _ => return Err(IdpError::Unsupported),
        }
        if let Some(h) = self.secret_hash(&a.username) {
            responses["SECRET_HASH"] = json!(h);
        }
        let v = self.call("RespondToAuthChallenge", json!({"ClientId": self.client_id, "ChallengeName": name, "Session": session, "ChallengeResponses": responses}))?;
        self.sign_in_result(&v, &a.username)
    }

    fn sign_up(&self, r: &SignUpRequest) -> Result<SignUp, IdpError> {
        self.in_app()?;
        let mut attrs = Vec::new();
        for (name, value) in [("email", &r.email), ("phone_number", &r.phone), ("given_name", &r.given_name), ("family_name", &r.family_name)] {
            if !value.is_empty() {
                attrs.push(json!({"Name": name, "Value": value}));
            }
        }
        let v = self.call("SignUp", self.named(&r.username, json!({"Password": r.password, "UserAttributes": attrs})))?;
        if v.get("UserConfirmed").and_then(|c| c.as_bool()) == Some(true) {
            Ok(SignUp::Done)
        } else {
            let destination = v.pointer("/CodeDeliveryDetails/Destination").and_then(|d| d.as_str()).unwrap_or("").to_string();
            Ok(SignUp::ConfirmationRequired { destination, session: String::new() })
        }
    }

    fn confirm_sign_up(&self, username: &str, code: &str, _session: &str) -> Result<(), IdpError> {
        self.in_app()?;
        self.call("ConfirmSignUp", self.named(username, json!({"ConfirmationCode": code}))).map(|_| ())
    }

    fn resend_code(&self, username: &str, _session: &str) -> Result<Option<String>, IdpError> {
        self.in_app()?;
        self.call("ResendConfirmationCode", self.named(username, json!({}))).map(|_| None)
    }

    fn forgot_password(&self, username: &str) -> Result<Option<String>, IdpError> {
        self.in_app()?;
        self.call("ForgotPassword", self.named(username, json!({}))).map(|_| None)
    }

    fn confirm_forgot_password(&self, username: &str, code: &str, new_password: &str, _session: &str) -> Result<(), IdpError> {
        self.in_app()?;
        self.call("ConfirmForgotPassword", self.named(username, json!({"ConfirmationCode": code, "Password": new_password}))).map(|_| ())
    }

    fn refresh(&self, refresh_token: &str, username: &str) -> Result<Tokens, IdpError> {
        let v = if self.rotation {
            // With rotation on, `REFRESH_TOKEN_AUTH` is refused and this answers a new refresh token.
            let mut body = json!({"ClientId": self.client_id, "RefreshToken": refresh_token});
            if let Some(s) = &self.client_secret {
                body["ClientSecret"] = json!(s);
            }
            self.call("GetTokensFromRefreshToken", body)?
        } else {
            let mut params = json!({"REFRESH_TOKEN": refresh_token});
            if let Some(h) = self.secret_hash(username) {
                params["SECRET_HASH"] = json!(h);
            }
            self.call("InitiateAuth", json!({"AuthFlow": "REFRESH_TOKEN_AUTH", "ClientId": self.client_id, "AuthParameters": params}))?
        };
        let mut t = Self::tokens(&v).ok_or_else(|| IdpError::Unavailable("Cognito answered no tokens to a refresh".into()))?;
        // Without rotation the refresh token stays the same and Cognito does not repeat it.
        if t.refresh.is_none() {
            t.refresh = Some(refresh_token.to_string());
        }
        Ok(t)
    }

    fn sign_out(&self, access_token: &str, refresh_token: Option<&str>) -> Result<(), IdpError> {
        let mut first: Result<(), IdpError> = Ok(());
        if let Some(rt) = refresh_token.filter(|t| !t.is_empty()) {
            let mut body = json!({"ClientId": self.client_id, "Token": rt});
            if let Some(s) = &self.client_secret {
                body["ClientSecret"] = json!(s);
            }
            first = self.call("RevokeToken", body).map(|_| ());
        }
        let second = if access_token.is_empty() { Ok(()) } else { self.call("GlobalSignOut", json!({"AccessToken": access_token})).map(|_| ()) };
        first.and(second)
    }

    fn start_hosted(&self, redirect_uri: &str, state: &str, code_challenge: &str, nonce: &str) -> Result<String, IdpError> {
        Ok(self.hosted.as_ref().ok_or(IdpError::Unsupported)?.start(redirect_uri, state, code_challenge, nonce))
    }

    fn finish_hosted(&self, code: &str, redirect_uri: &str, code_verifier: &str) -> Result<Tokens, IdpError> {
        self.hosted.as_ref().ok_or(IdpError::Unsupported)?.exchange(code, redirect_uri, code_verifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_secret_hash_is_hmac_sha256_of_the_user_and_the_client_keyed_by_the_secret() {
        struct Nobody;
        impl Http for Nobody {
            fn send(&self, _: &HttpRequest) -> Result<super::super::client::HttpResponse, String> {
                Err("offline".into())
            }
        }
        let c = Cognito::new(
            Arc::new(Nobody),
            CognitoSettings {
                region: "eu-west-1".into(),
                user_pool_id: "eu-west-1_Pool".into(),
                client_id: "client".into(),
                client_secret: Some("secret".into()),
                endpoint: None,
                domain: None,
                scopes: None,
                flow: Flow::InApp,
                refresh_rotation: false,
                totp_issuer: None,
            },
        );
        // HMAC-SHA256(key "secret", "userclient"), base64.
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
        mac.update(b"userclient");
        assert_eq!(c.secret_hash("user"), Some(B64.encode(mac.finalize().into_bytes())));
        assert_eq!(c.secret_hash("user").unwrap().len(), 44);
    }

    #[test]
    fn no_cognito_error_tells_an_unknown_user_from_a_wrong_password() {
        for kind in ["UserNotFoundException", "NotAuthorizedException", "UserNotConfirmedException", "PasswordResetRequiredException"] {
            assert_eq!(classify("InitiateAuth", 400, kind, "User does not exist."), IdpError::Rejected, "{kind}");
        }
        assert_eq!(classify("ForgotPassword", 400, "InvalidParameterException", "no verified e-mail"), IdpError::Rejected);
        assert!(matches!(classify("SignUp", 400, "InvalidPasswordException", "too short"), IdpError::Invalid(_)));
        assert_eq!(classify("InitiateAuth", 400, "TooManyRequestsException", ""), IdpError::Throttled);
        assert!(matches!(classify("InitiateAuth", 503, "", ""), IdpError::Unavailable(_)));
    }
}
