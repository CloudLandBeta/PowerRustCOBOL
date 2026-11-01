// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A mock of the three identity products, speaking the wire shapes each documents (WASM-009 plan
//! §3, researched 2026-10-20): Cognito's JSON target API and its managed login's OAuth endpoints,
//! Keycloak's and Entra's OAuth endpoints, and Entra External ID's native authentication API.
//!
//! It is the backend's [`Http`] client, in process: no socket, no real provider, no account. Every
//! request is recorded (method, address, body) so a test can assert what the backend sent — that
//! `SECRET_HASH` is right, that a refused attempt never reached the provider, that a password
//! appears nowhere it should not.
//!
//! Users: a password, whether confirmed, and the second step a sign-in meets (`Step`). Codes are
//! fixed: [`MFA_CODE`], [`CONFIRM_CODE`], [`RESET_CODE`], [`SETUP_CODE`].

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use cobolt_server::config::Discovery;
use cobolt_server::identity::client::{Http, HttpRequest, HttpResponse};

pub const CLIENT_ID: &str = "app-client-1";
pub const CLIENT_SECRET: &str = "mock-client-secret-0123456789";
pub const MFA_CODE: &str = "123456";
pub const CONFIRM_CODE: &str = "111111";
pub const RESET_CODE: &str = "222222";
pub const SETUP_CODE: &str = "333333";
pub const TOTP_SECRET: &str = "JBSWY3DPEHPK3PXP";

pub const COGNITO_API: &str = "https://cognito-idp.eu-west-1.amazonaws.com/";
pub const COGNITO_DOMAIN: &str = "https://shop.auth.eu-west-1.amazoncognito.com";
pub const KEYCLOAK_ISSUER: &str = "https://kc.example.test/realms/shop";
pub const ENTRA_TENANT: &str = "0f1e2d3c-0000-4000-8000-000000000001";
pub const ENTRA_EXTERNAL: &str = "https://shop.ciamlogin.com/shop.onmicrosoft.com";

/// What a sign-in meets after the password.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    None,
    Totp,
    Sms,
    Email,
    /// Cognito's spelling `EMAIL_MFA` (the documentation names both, plan §3.1).
    EmailMfa,
    /// `SELECT_MFA_TYPE` among SMS and TOTP.
    Choose,
    /// Enrol an authenticator app (`MFA_SETUP`).
    Setup,
    /// A temporary password (`NEW_PASSWORD_REQUIRED`).
    NewPassword,
}

#[derive(Clone, Debug)]
pub struct User {
    pub password: String,
    pub confirmed: bool,
    pub step: Step,
}

impl User {
    pub fn new(password: &str, step: Step) -> User {
        User { password: password.into(), confirmed: true, step }
    }
}

#[derive(Default)]
pub struct State {
    pub users: HashMap<String, User>,
    /// refresh token -> user
    pub refresh: HashMap<String, String>,
    pub revoked: HashSet<String>,
    /// auth code -> (user, redirect_uri, S256 challenge)
    pub codes: HashMap<String, (String, String, String)>,
    /// provider session / continuation token -> (user, what it continues)
    pub sessions: HashMap<String, (String, String)>,
    pub requests: Vec<(String, String, String)>,
    pub seq: u64,
    /// Cognito: the app client has a secret, so `SECRET_HASH` is required.
    pub secret: bool,
    /// Cognito: the app client rotates refresh tokens.
    pub rotation: bool,
}

pub struct IdpMock {
    pub state: Mutex<State>,
}

fn ok(v: Value) -> Result<HttpResponse, String> {
    Ok(HttpResponse { status: 200, body: v.to_string() })
}

fn err(status: u16, v: Value) -> Result<HttpResponse, String> {
    Ok(HttpResponse { status, body: v.to_string() })
}

fn amz(kind: &str, message: &str) -> Result<HttpResponse, String> {
    err(400, json!({"__type": kind, "message": message}))
}

impl IdpMock {
    pub fn new() -> IdpMock {
        let m = IdpMock { state: Mutex::new(State::default()) };
        for (name, step) in [
            ("ana", Step::None),
            ("tina", Step::Totp),
            ("sam", Step::Sms),
            ("eve", Step::Email),
            ("emma", Step::EmailMfa),
            ("cho", Step::Choose),
            ("nora", Step::Setup),
            ("temp", Step::NewPassword),
        ] {
            m.add_user(name, User::new("Passw0rd!", step));
        }
        m
    }

    pub fn add_user(&self, name: &str, user: User) {
        self.state.lock().unwrap().users.insert(name.to_string(), user);
    }

    pub fn requests(&self) -> Vec<(String, String, String)> {
        self.state.lock().unwrap().requests.clone()
    }

    /// How many requests have reached an address containing `needle`.
    pub fn hits(&self, needle: &str) -> usize {
        self.requests().iter().filter(|(_, u, _)| u.contains(needle)).count()
    }

    pub fn password_of(&self, user: &str) -> Option<String> {
        self.state.lock().unwrap().users.get(user).map(|u| u.password.clone())
    }

    /// The metadata of the provider whose discovery the backend would read.
    pub fn discovery(provider: &str) -> Discovery {
        match provider {
            "keycloak" => {
                let b = format!("{KEYCLOAK_ISSUER}/protocol/openid-connect");
                Discovery {
                    issuer: KEYCLOAK_ISSUER.into(),
                    jwks_uri: format!("{b}/certs"),
                    authorization_endpoint: format!("{b}/auth"),
                    token_endpoint: format!("{b}/token"),
                    revocation_endpoint: Some(format!("{b}/revoke")),
                    end_session_endpoint: Some(format!("{b}/logout")),
                }
            }
            "entra" => {
                let b = format!("https://login.microsoftonline.com/{ENTRA_TENANT}");
                Discovery {
                    issuer: format!("{b}/v2.0"),
                    jwks_uri: format!("{b}/discovery/v2.0/keys"),
                    authorization_endpoint: format!("{b}/oauth2/v2.0/authorize"),
                    token_endpoint: format!("{b}/oauth2/v2.0/token"),
                    revocation_endpoint: None,
                    end_session_endpoint: Some(format!("{b}/oauth2/v2.0/logout")),
                }
            }
            _ => Discovery {
                issuer: format!("https://{ENTRA_TENANT}.ciamlogin.com/{ENTRA_TENANT}/v2.0"),
                jwks_uri: format!("{ENTRA_EXTERNAL}/discovery/v2.0/keys"),
                authorization_endpoint: format!("{ENTRA_EXTERNAL}/oauth2/v2.0/authorize"),
                token_endpoint: format!("{ENTRA_EXTERNAL}/oauth2/v2.0/token"),
                revocation_endpoint: None,
                end_session_endpoint: Some(format!("{ENTRA_EXTERNAL}/oauth2/v2.0/logout")),
            },
        }
    }

    /// The provider's sign-in page: the user signs in at `authorize_url` and the page sends the
    /// browser to the `redirect_uri` with a code. Answers `(code, state)`.
    pub fn authorize(&self, authorize_url: &str, user: &str, password: &str) -> Option<(String, String)> {
        let query = authorize_url.split_once('?')?.1;
        let get = |k: &str| cobolt_server::http::query_param(query, k).unwrap_or_default();
        if get("response_type") != "code" || get("client_id") != CLIENT_ID || get("code_challenge_method") != "S256" || get("code_challenge").is_empty() {
            return None;
        }
        let mut st = self.state.lock().unwrap();
        if st.users.get(user).map(|u| u.password.as_str()) != Some(password) {
            return None;
        }
        st.seq += 1;
        let code = format!("code-{}", st.seq);
        st.codes.insert(code.clone(), (user.to_string(), get("redirect_uri"), get("code_challenge")));
        Some((code, get("state")))
    }

    fn tokens(st: &mut State, user: &str) -> (String, String) {
        st.seq += 1;
        let (at, rt) = (format!("at.{user}.{}", st.seq), format!("rt.{user}.{}", st.seq));
        st.refresh.insert(rt.clone(), user.to_string());
        (at, rt)
    }

    fn session(st: &mut State, user: &str, what: &str) -> String {
        st.seq += 1;
        let s = format!("sess-{}-{what}", st.seq);
        st.sessions.insert(s.clone(), (user.to_string(), what.to_string()));
        s
    }

    // ── Cognito's JSON target API ──

    fn secret_hash(user: &str) -> String {
        use hmac::{Hmac, Mac};
        let mut mac = Hmac::<Sha256>::new_from_slice(CLIENT_SECRET.as_bytes()).unwrap();
        mac.update(user.as_bytes());
        mac.update(CLIENT_ID.as_bytes());
        B64.encode(mac.finalize().into_bytes())
    }

    fn cognito_result(st: &mut State, user: &str) -> Value {
        let (at, rt) = Self::tokens(st, user);
        json!({"AuthenticationResult": {"AccessToken": at, "IdToken": format!("id.{user}"), "RefreshToken": rt, "ExpiresIn": 3600, "TokenType": "Bearer"}})
    }

    fn cognito_challenge(st: &mut State, user: &str, name: &str) -> Value {
        let session = Self::session(st, user, name);
        let params = match name {
            "SMS_MFA" => json!({"CODE_DELIVERY_DELIVERY_MEDIUM": "SMS", "CODE_DELIVERY_DESTINATION": "+*******1234"}),
            "EMAIL_OTP" | "EMAIL_MFA" => json!({"CODE_DELIVERY_DELIVERY_MEDIUM": "EMAIL", "CODE_DELIVERY_DESTINATION": "e***@e***"}),
            "SELECT_MFA_TYPE" => json!({"MFAS_CAN_CHOOSE": "[\"SMS_MFA\",\"SOFTWARE_TOKEN_MFA\"]"}),
            _ => json!({}),
        };
        json!({"ChallengeName": name, "Session": session, "ChallengeParameters": params})
    }

    fn cognito(&self, st: &mut State, target: &str, v: &Value) -> Result<HttpResponse, String> {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        if s("ClientId") != CLIENT_ID && !matches!(target, "AssociateSoftwareToken" | "VerifySoftwareToken" | "GlobalSignOut") {
            return amz("ResourceNotFoundException", "User pool client does not exist.");
        }
        // Every call naming a user carries SECRET_HASH when the client has a secret.
        let named = v.get("Username").or_else(|| v.pointer("/AuthParameters/USERNAME")).or_else(|| v.pointer("/ChallengeResponses/USERNAME")).and_then(|u| u.as_str()).map(str::to_owned);
        if st.secret {
            if let Some(u) = &named {
                let given = v.get("SecretHash").or_else(|| v.pointer("/AuthParameters/SECRET_HASH")).or_else(|| v.pointer("/ChallengeResponses/SECRET_HASH")).and_then(|h| h.as_str()).unwrap_or("");
                if given != Self::secret_hash(u) {
                    return amz("NotAuthorizedException", "Unable to verify secret hash for client.");
                }
            }
        }
        match target {
            "InitiateAuth" => match s("AuthFlow").as_str() {
                "USER_PASSWORD_AUTH" => {
                    let user = named.unwrap_or_default();
                    let password = v.pointer("/AuthParameters/PASSWORD").and_then(|p| p.as_str()).unwrap_or("");
                    // LEGACY error handling (the API default): the provider says who does not exist.
                    let Some(u) = st.users.get(&user).cloned() else { return amz("UserNotFoundException", "User does not exist.") };
                    if u.password != password {
                        return amz("NotAuthorizedException", "Incorrect username or password.");
                    }
                    if !u.confirmed {
                        return amz("UserNotConfirmedException", "User is not confirmed.");
                    }
                    let name = match u.step {
                        Step::None => return ok(Self::cognito_result(st, &user)),
                        Step::Totp => "SOFTWARE_TOKEN_MFA",
                        Step::Sms => "SMS_MFA",
                        Step::Email => "EMAIL_OTP",
                        Step::EmailMfa => "EMAIL_MFA",
                        Step::Choose => "SELECT_MFA_TYPE",
                        Step::Setup => "MFA_SETUP",
                        Step::NewPassword => "NEW_PASSWORD_REQUIRED",
                    };
                    ok(Self::cognito_challenge(st, &user, name))
                }
                "REFRESH_TOKEN_AUTH" => {
                    if st.rotation {
                        return amz("NotAuthorizedException", "Refresh token rotation is enabled; use GetTokensFromRefreshToken.");
                    }
                    let rt = v.pointer("/AuthParameters/REFRESH_TOKEN").and_then(|p| p.as_str()).unwrap_or("");
                    match st.refresh.get(rt).cloned().filter(|_| !st.revoked.contains(rt)) {
                        Some(user) => {
                            let (at, _) = Self::tokens(st, &user);
                            ok(json!({"AuthenticationResult": {"AccessToken": at, "ExpiresIn": 3600}}))
                        }
                        None => amz("NotAuthorizedException", "Invalid Refresh Token"),
                    }
                }
                _ => amz("InvalidParameterException", "Initiate Auth method not supported."),
            },
            "GetTokensFromRefreshToken" => {
                let rt = s("RefreshToken");
                match st.refresh.get(&rt).cloned().filter(|_| st.rotation && !st.revoked.contains(&rt)) {
                    Some(user) => {
                        st.revoked.insert(rt);
                        ok(Self::cognito_result(st, &user))
                    }
                    None => amz("NotAuthorizedException", "Invalid Refresh Token"),
                }
            }
            "RespondToAuthChallenge" => {
                let name = s("ChallengeName");
                let Some((user, what)) = st.sessions.remove(&s("Session")) else { return amz("NotAuthorizedException", "Invalid session for the user.") };
                let r = |k: &str| v.pointer(&format!("/ChallengeResponses/{k}")).and_then(|x| x.as_str()).unwrap_or("").to_string();
                if named.as_deref() != Some(user.as_str()) || what != name {
                    return amz("NotAuthorizedException", "Invalid session for the user.");
                }
                match name.as_str() {
                    "SMS_MFA" | "SOFTWARE_TOKEN_MFA" | "EMAIL_OTP" | "EMAIL_MFA" => {
                        let code = r(&format!("{name}_CODE"));
                        if code == MFA_CODE {
                            ok(Self::cognito_result(st, &user))
                        } else {
                            amz("CodeMismatchException", "Invalid code received for user")
                        }
                    }
                    "NEW_PASSWORD_REQUIRED" => {
                        let p = r("NEW_PASSWORD");
                        if p.len() < 8 {
                            return amz("InvalidPasswordException", "Password does not conform to policy: Password not long enough");
                        }
                        let u = st.users.get_mut(&user).unwrap();
                        u.password = p;
                        u.step = Step::None;
                        ok(Self::cognito_result(st, &user))
                    }
                    "SELECT_MFA_TYPE" => match r("ANSWER").as_str() {
                        a @ ("SMS_MFA" | "SOFTWARE_TOKEN_MFA") => ok(Self::cognito_challenge(st, &user, a)),
                        _ => amz("InvalidParameterException", "Invalid MFA type"),
                    },
                    "MFA_SETUP" => ok(Self::cognito_result(st, &user)),
                    _ => amz("InvalidParameterException", "Unknown challenge"),
                }
            }
            "AssociateSoftwareToken" => {
                let Some((user, what)) = st.sessions.remove(&s("Session")) else { return amz("NotAuthorizedException", "Invalid session") };
                if what != "MFA_SETUP" {
                    return amz("NotAuthorizedException", "Invalid session");
                }
                let session = Self::session(st, &user, "ASSOCIATED");
                ok(json!({"SecretCode": TOTP_SECRET, "Session": session}))
            }
            "VerifySoftwareToken" => {
                let Some((user, what)) = st.sessions.remove(&s("Session")) else { return amz("NotAuthorizedException", "Invalid session") };
                if what != "ASSOCIATED" || s("UserCode") != SETUP_CODE {
                    return amz("EnableSoftwareTokenMFAException", "Code mismatch");
                }
                let session = Self::session(st, &user, "MFA_SETUP");
                ok(json!({"Status": "SUCCESS", "Session": session}))
            }
            "SignUp" => {
                let user = s("Username");
                if st.users.contains_key(&user) {
                    return amz("UsernameExistsException", "User already exists");
                }
                if s("Password").len() < 8 {
                    return amz("InvalidPasswordException", "Password did not conform with policy: Password not long enough");
                }
                st.users.insert(user, User { password: s("Password"), confirmed: false, step: Step::None });
                ok(json!({"UserConfirmed": false, "UserSub": "sub", "CodeDeliveryDetails": {"Destination": "n***@e***", "DeliveryMedium": "EMAIL", "AttributeName": "email"}}))
            }
            "ConfirmSignUp" => match st.users.get_mut(&s("Username")) {
                Some(u) if s("ConfirmationCode") == CONFIRM_CODE => {
                    u.confirmed = true;
                    ok(json!({}))
                }
                Some(_) => amz("CodeMismatchException", "Invalid verification code provided"),
                None => amz("UserNotFoundException", "Username/client id combination not found."),
            },
            "ResendConfirmationCode" | "ForgotPassword" => match st.users.get(&s("Username")) {
                Some(_) => ok(json!({"CodeDeliveryDetails": {"Destination": "n***@e***", "DeliveryMedium": "EMAIL"}})),
                None => amz("UserNotFoundException", "Username/client id combination not found."),
            },
            "ConfirmForgotPassword" => match st.users.get_mut(&s("Username")) {
                Some(u) if s("ConfirmationCode") == RESET_CODE => {
                    u.password = s("Password");
                    ok(json!({}))
                }
                Some(_) => amz("CodeMismatchException", "Invalid verification code provided"),
                None => amz("UserNotFoundException", "Username/client id combination not found."),
            },
            "RevokeToken" => {
                if st.secret && s("ClientSecret") != CLIENT_SECRET {
                    return amz("UnauthorizedException", "Client secret mismatch");
                }
                st.revoked.insert(s("Token"));
                ok(json!({}))
            }
            "GlobalSignOut" => ok(json!({})),
            _ => amz("InvalidParameterException", "unknown target"),
        }
    }

    // ── the OAuth endpoints (Keycloak, Entra, Cognito's managed login) ──

    fn oauth_token(&self, st: &mut State, req: &HttpRequest, f: &HashMap<String, String>) -> Result<HttpResponse, String> {
        let get = |k: &str| f.get(k).cloned().unwrap_or_default();
        // A confidential client authenticates with client_secret_basic.
        if st.secret {
            let want = format!("Basic {}", B64.encode(format!("{CLIENT_ID}:{CLIENT_SECRET}")));
            if req.header("Authorization") != Some(want.as_str()) {
                return err(401, json!({"error": "invalid_client"}));
            }
        }
        match get("grant_type").as_str() {
            "authorization_code" => {
                let Some((user, redirect, challenge)) = st.codes.remove(&get("code")) else { return err(400, json!({"error": "invalid_grant"})) };
                let s256 = URL_SAFE_NO_PAD.encode(Sha256::digest(get("code_verifier").as_bytes()));
                if redirect != get("redirect_uri") || s256 != challenge {
                    return err(400, json!({"error": "invalid_grant", "error_description": "PKCE verification failed"}));
                }
                let (at, rt) = Self::tokens(st, &user);
                ok(json!({"access_token": at, "id_token": format!("id.{user}"), "refresh_token": rt, "expires_in": 300, "token_type": "Bearer"}))
            }
            "refresh_token" => {
                let rt = get("refresh_token");
                match st.refresh.get(&rt).cloned().filter(|_| !st.revoked.contains(&rt)) {
                    Some(user) => {
                        let (at, _) = Self::tokens(st, &user);
                        ok(json!({"access_token": at, "expires_in": 300, "token_type": "Bearer"}))
                    }
                    None => err(400, json!({"error": "invalid_grant"})),
                }
            }
            _ => err(400, json!({"error": "unsupported_grant_type"})),
        }
    }

    // ── Entra External ID's native authentication ──

    fn ct(st: &mut State, user: &str, what: &str) -> String {
        Self::session(st, user, what)
    }

    fn take(st: &mut State, f: &HashMap<String, String>, want: &str) -> Option<String> {
        let (user, what) = st.sessions.remove(f.get("continuation_token")?)?;
        (what == want).then_some(user)
    }

    fn native(&self, st: &mut State, path: &str, f: &HashMap<String, String>) -> Result<HttpResponse, String> {
        let get = |k: &str| f.get(k).cloned().unwrap_or_default();
        if get("client_id") != CLIENT_ID {
            return err(400, json!({"error": "unauthorized_client"}));
        }
        let expired = || err(400, json!({"error": "expired_token", "error_codes": [552003]}));
        match path {
            "/oauth2/v2.0/initiate" => {
                let user = get("username");
                if !st.users.contains_key(&user) {
                    return err(400, json!({"error": "user_not_found", "error_codes": [50034]}));
                }
                let ct = Self::ct(st, &user, "initiated");
                ok(json!({"continuation_token": ct}))
            }
            "/oauth2/v2.0/challenge" => {
                let Some((user, what)) = f.get("continuation_token").and_then(|c| st.sessions.remove(c)) else { return expired() };
                if what == "initiated" {
                    // A forced password change needs the provider's page.
                    if st.users.get(&user).map(|u| &u.step) == Some(&Step::NewPassword) {
                        return ok(json!({"challenge_type": "redirect"}));
                    }
                    let ct = Self::ct(st, &user, "password");
                    return ok(json!({"challenge_type": "password", "continuation_token": ct}));
                }
                if what == "introspected" {
                    return match get("id").as_str() {
                        "email-1" => {
                            let ct = Self::ct(st, &user, "mfa-sent");
                            ok(json!({"challenge_type": "oob", "binding_method": "prompt", "challenge_channel": "email", "challenge_target_label": "e***@e***", "code_length": 6, "continuation_token": ct}))
                        }
                        // SMS in the native API is UNVERIFIED (plan §3.3): modelled as refused.
                        _ => ok(json!({"challenge_type": "redirect"})),
                    };
                }
                expired()
            }
            "/oauth2/v2.0/token" => match get("grant_type").as_str() {
                "password" => {
                    let Some(user) = Self::take(st, f, "password") else { return expired() };
                    let u = st.users.get(&user).cloned().unwrap();
                    if u.password != get("password") {
                        return err(400, json!({"error": "invalid_grant", "error_codes": [50126]}));
                    }
                    match u.step {
                        Step::None => {
                            let (at, rt) = Self::tokens(st, &user);
                            ok(json!({"access_token": at, "id_token": format!("id.{user}"), "refresh_token": rt, "expires_in": 300, "token_type": "Bearer"}))
                        }
                        Step::Email | Step::Sms | Step::Choose | Step::Totp | Step::EmailMfa => {
                            let ct = Self::ct(st, &user, "mfa");
                            err(400, json!({"error": "invalid_grant", "suberror": "mfa_required", "continuation_token": ct}))
                        }
                        Step::Setup => err(400, json!({"error": "invalid_grant", "suberror": "registration_required"})),
                        Step::NewPassword => ok(json!({"challenge_type": "redirect"})),
                    }
                }
                "mfa_oob" => {
                    let Some(user) = Self::take(st, f, "mfa-sent") else { return expired() };
                    if get("oob") != MFA_CODE {
                        return err(400, json!({"error": "invalid_grant", "suberror": "invalid_oob_value"}));
                    }
                    let (at, rt) = Self::tokens(st, &user);
                    ok(json!({"access_token": at, "refresh_token": rt, "expires_in": 300, "token_type": "Bearer"}))
                }
                _ => err(400, json!({"error": "unsupported_grant_type"})),
            },
            "/oauth2/v2.0/introspect" => {
                let Some(user) = Self::take(st, f, "mfa") else { return expired() };
                let step = st.users.get(&user).map(|u| u.step.clone()).unwrap();
                let methods = match step {
                    Step::Email | Step::EmailMfa => json!([{"id": "email-1", "challenge_type": "oob", "challenge_channel": "email", "login_hint": "e***@e***"}]),
                    Step::Sms => json!([{"id": "sms-1", "challenge_type": "oob", "challenge_channel": "sms", "login_hint": "+*******1234"}]),
                    Step::Choose => json!([{"id": "email-1", "challenge_type": "oob", "challenge_channel": "email"}, {"id": "sms-1", "challenge_type": "oob", "challenge_channel": "sms"}]),
                    // An authenticator app is not a native method.
                    _ => json!([]),
                };
                let ct = Self::ct(st, &user, "introspected");
                ok(json!({"continuation_token": ct, "methods": methods}))
            }
            "/signup/v1.0/start" => {
                let user = get("username");
                if st.users.contains_key(&user) {
                    return err(400, json!({"error": "user_already_exists"}));
                }
                if get("password").len() < 8 {
                    return err(400, json!({"error": "invalid_grant", "suberror": "password_too_weak"}));
                }
                st.users.insert(user.clone(), User { password: get("password"), confirmed: false, step: Step::None });
                let ct = Self::ct(st, &user, "signup-started");
                ok(json!({"continuation_token": ct}))
            }
            "/signup/v1.0/challenge" => {
                let Some((user, what)) = f.get("continuation_token").and_then(|c| st.sessions.remove(c)) else { return expired() };
                if what != "signup-started" && what != "signup-sent" {
                    return expired();
                }
                let ct = Self::ct(st, &user, "signup-sent");
                ok(json!({"challenge_type": "oob", "binding_method": "prompt", "challenge_target_label": "n***@e***", "code_length": 6, "continuation_token": ct}))
            }
            "/signup/v1.0/continue" => {
                let Some(user) = Self::take(st, f, "signup-sent") else { return expired() };
                if get("oob") != CONFIRM_CODE {
                    return err(400, json!({"error": "invalid_grant", "suberror": "invalid_oob_value"}));
                }
                st.users.get_mut(&user).unwrap().confirmed = true;
                let ct = Self::ct(st, &user, "signed-up");
                ok(json!({"continuation_token": ct}))
            }
            "/resetpassword/v1.0/start" => {
                let user = get("username");
                if !st.users.contains_key(&user) {
                    return err(400, json!({"error": "user_not_found", "error_codes": [50034]}));
                }
                let ct = Self::ct(st, &user, "reset-started");
                ok(json!({"continuation_token": ct}))
            }
            "/resetpassword/v1.0/challenge" => {
                let Some(user) = Self::take(st, f, "reset-started") else { return expired() };
                let ct = Self::ct(st, &user, "reset-sent");
                ok(json!({"challenge_type": "oob", "challenge_target_label": "a***@e***", "code_length": 8, "continuation_token": ct}))
            }
            "/resetpassword/v1.0/continue" => {
                let Some(user) = Self::take(st, f, "reset-sent") else { return expired() };
                if get("oob") != RESET_CODE {
                    return err(400, json!({"error": "invalid_grant", "suberror": "invalid_oob_value"}));
                }
                let ct = Self::ct(st, &user, "reset-verified");
                ok(json!({"continuation_token": ct}))
            }
            "/resetpassword/v1.0/submit" => {
                let Some(user) = Self::take(st, f, "reset-verified") else { return expired() };
                st.users.get_mut(&user).unwrap().password = get("new_password");
                let ct = Self::ct(st, &user, "reset-submitted");
                ok(json!({"continuation_token": ct, "poll_interval": 1}))
            }
            "/resetpassword/v1.0/poll_completion" => {
                let Some(user) = Self::take(st, f, "reset-submitted") else { return expired() };
                let ct = Self::ct(st, &user, "reset-done");
                ok(json!({"status": "succeeded", "continuation_token": ct}))
            }
            _ => err(404, json!({"error": "not_found"})),
        }
    }
}

impl Http for IdpMock {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, String> {
        let mut st = self.state.lock().unwrap();
        let body = String::from_utf8_lossy(&req.body).into_owned();
        st.requests.push((req.method.to_string(), req.url.clone(), body.clone()));
        let form: HashMap<String, String> = req.form().into_iter().collect();
        let url = req.url.as_str();
        if url == COGNITO_API {
            let target = req.header("X-Amz-Target").unwrap_or("").rsplit('.').next().unwrap_or("").to_string();
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            return self.cognito(&mut st, &target, &v);
        }
        if let Some(path) = url.strip_prefix(ENTRA_EXTERNAL) {
            if path == "/oauth2/v2.0/token" && matches!(form.get("grant_type").map(String::as_str), Some("authorization_code" | "refresh_token")) {
                return self.oauth_token(&mut st, req, &form);
            }
            return self.native(&mut st, path, &form);
        }
        if url.ends_with("/token") {
            return self.oauth_token(&mut st, req, &form);
        }
        if url.ends_with("/revoke") {
            if let Some(t) = form.get("token") {
                st.revoked.insert(t.clone());
            }
            return ok(json!({}));
        }
        err(404, json!({"error": "no such endpoint in the mock", "url": url}))
    }
}
