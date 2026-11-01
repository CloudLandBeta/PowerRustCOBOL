// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend half of authentication (WASM-009 R4, R7-R10, R12).
//!
//! The Authentication control talks to the backend's `/auth/*` routes, which run the generated
//! public programs `AUTH-SIGN-IN`, `AUTH-SIGN-UP`, … Each program's body is one built-in,
//! `COBOL::"AUTH-OP"`, which lands in [`ops::IdentityService::run`]. Everything a provider
//! conversation needs — the client secret, the code exchange, the refresh token — stays here and
//! is spoken to the provider by an [`IdentityProvider`] adapter.
//!
//! **The adapter contract is what makes a fourth provider cheap** (R8, D2): an adapter implements
//! the operations and says, through [`Capabilities`], which of them its flow supports; the
//! service decides what the application sees.
//!
//! **What an adapter can never return** is "no such user": every result type carries only the
//! *generic* failure (R10), so the difference between an unknown user and a wrong password is not
//! representable above this line.

pub mod check;
pub mod client;
pub mod cognito;
pub mod entra;
pub mod keycloak;
pub mod oidc;
pub mod ops;
pub mod recaptcha;
pub mod routes;
pub mod seal;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::config::{Discovery, Provider, ResolvedIdentity};

pub use crate::config::IdentityFlow as Flow;

/// What one provider, in its flow, does from the application's own screens (plan §6). An
/// operation it does not do answers [`IdpError::Unsupported`], and the control uses the provider's
/// page for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub sign_in: bool,
    pub sign_up: bool,
    pub confirm: bool,
    pub reset: bool,
    pub new_password: bool,
    pub mfa_totp: bool,
    pub mfa_sms: bool,
    pub mfa_email: bool,
    pub mfa_setup: bool,
    pub refresh: bool,
    pub revoke: bool,
    /// The provider's own page is available (the hosted flow, or the fallback of an in-app one).
    pub hosted: bool,
}

impl Capabilities {
    /// The flags by name, in a fixed order (`AUTH-CAPABILITIES`, the conformance matrix).
    pub fn named(&self) -> [(&'static str, bool); 12] {
        [
            ("sign-in", self.sign_in),
            ("sign-up", self.sign_up),
            ("confirm", self.confirm),
            ("reset", self.reset),
            ("new-password", self.new_password),
            ("mfa-totp", self.mfa_totp),
            ("mfa-sms", self.mfa_sms),
            ("mfa-email", self.mfa_email),
            ("mfa-setup", self.mfa_setup),
            ("refresh", self.refresh),
            ("revoke", self.revoke),
            ("hosted", self.hosted),
        ]
    }
}

/// The tokens a successful sign-in or refresh yields.
#[derive(Clone, Default)]
pub struct Tokens {
    pub access: String,
    pub id: Option<String>,
    pub refresh: Option<String>,
    pub expires_in: u64,
}

impl fmt::Debug for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tokens {{ access: <{} bytes>, id: {}, refresh: {}, expires_in: {} }}", self.access.len(), self.id.is_some(), self.refresh.is_some(), self.expires_in)
    }
}

/// A second factor the user may pick (`SELECT_MFA_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaChoice {
    Totp,
    Sms,
    Email,
}

impl MfaChoice {
    pub fn wire_name(self) -> &'static str {
        match self {
            MfaChoice::Totp => "totp",
            MfaChoice::Sms => "sms",
            MfaChoice::Email => "email",
        }
    }

    pub fn from_wire(word: &str) -> Option<MfaChoice> {
        match word.trim().to_ascii_lowercase().as_str() {
            "totp" => Some(MfaChoice::Totp),
            "sms" => Some(MfaChoice::Sms),
            "email" => Some(MfaChoice::Email),
            _ => None,
        }
    }
}

/// What the user must still do before there are tokens (D2, C3) — the same kinds the player's
/// screens show.
#[derive(Clone, PartialEq, Eq)]
pub enum MfaKind {
    Totp,
    /// The destination as the provider masked it ("+55*******12").
    Sms { destination: String },
    Email { destination: String },
    Choice(Vec<MfaChoice>),
    /// Enrolment of an authenticator app: the shared secret (base32) and its `otpauth://` address.
    TotpSetup { secret: String, otpauth_uri: String },
    NewPassword,
}

impl fmt::Debug for MfaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MfaKind::TotpSetup { .. } => f.write_str("TotpSetup { <secret> }"),
            MfaKind::Sms { destination } => write!(f, "Sms({destination})"),
            MfaKind::Email { destination } => write!(f, "Email({destination})"),
            MfaKind::Choice(c) => write!(f, "Choice({c:?})"),
            MfaKind::Totp => f.write_str("Totp"),
            MfaKind::NewPassword => f.write_str("NewPassword"),
        }
    }
}

impl MfaKind {
    pub fn word(&self) -> &'static str {
        match self {
            MfaKind::Totp => "totp",
            MfaKind::Sms { .. } => "sms",
            MfaKind::Email { .. } => "email",
            MfaKind::Choice(_) => "choice",
            MfaKind::TotpSetup { .. } => "totp-setup",
            MfaKind::NewPassword => "new-password",
        }
    }
}

/// A step the user must still take before there are tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    pub kind: MfaKind,
    /// The provider's own name for the step (`SOFTWARE_TOKEN_MFA`, `mfa_required`, …), handed back
    /// with the answer.
    pub name: String,
    /// The provider's continuation (Cognito `Session`, Entra `continuation_token`). The service
    /// seals it before it leaves the backend.
    pub session: String,
}

/// The result of a sign-in step.
#[derive(Debug, Clone)]
pub enum SignIn {
    Tokens(Tokens),
    Challenge(Challenge),
}

/// The result of a sign-up step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignUp {
    /// The account exists and a code was sent to confirm it; `session` is the provider's
    /// continuation, when its steps span requests.
    ConfirmationRequired { destination: String, session: String },
    /// The account exists and is confirmed.
    Done,
}

/// Why an operation did not do what was asked. **Never "no such user".**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdpError {
    /// The credentials, the code or the session were not accepted, for any reason the user must
    /// not learn: unknown user, wrong password, locked, disabled, expired code.
    Rejected,
    /// The request is wrong in a way the user can fix and learn nothing private from (a password
    /// that breaks the policy, an address that is not an e-mail address). Safe to show.
    Invalid(String),
    /// The provider says to slow down.
    Throttled,
    /// This provider, in this flow, does not do that from the application's own screens: use the
    /// provider's page.
    Unsupported,
    /// The provider could not be reached or answered something unreadable. Not the user's doing;
    /// the message is for the operator's log only.
    Unavailable(String),
}

impl fmt::Display for IdpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdpError::Rejected => write!(f, "rejected"),
            IdpError::Invalid(m) => write!(f, "invalid: {m}"),
            IdpError::Throttled => write!(f, "throttled"),
            IdpError::Unsupported => write!(f, "not supported in this flow"),
            IdpError::Unavailable(m) => write!(f, "provider unavailable: {m}"),
        }
    }
}

/// What a sign-up carries.
#[derive(Clone, Default)]
pub struct SignUpRequest {
    pub username: String,
    pub password: String,
    pub email: String,
    pub phone: String,
    pub given_name: String,
    pub family_name: String,
}

impl fmt::Debug for SignUpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignUpRequest { <hidden> }")
    }
}

/// An answer to a challenge.
#[derive(Clone, Default)]
pub struct ChallengeResponse {
    pub username: String,
    /// [`Challenge::name`], as the provider gave it.
    pub name: String,
    /// [`Challenge::session`], as the provider gave it.
    pub session: String,
    /// The code (MFA, TOTP set-up).
    pub code: String,
    /// The new password (`NEW_PASSWORD_REQUIRED`).
    pub new_password: String,
    /// The factor picked (`SELECT_MFA_TYPE`).
    pub choice: Option<MfaChoice>,
    /// The answer verifies an authenticator app being enrolled.
    pub setup: bool,
}

impl fmt::Debug for ChallengeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChallengeResponse {{ name: {:?}, choice: {:?}, setup: {} }}", self.name, self.choice, self.setup)
    }
}

/// A provider's adapter (D2).
pub trait IdentityProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn flow(&self) -> Flow;
    fn capabilities(&self) -> Capabilities;

    fn sign_in(&self, username: &str, password: &str) -> Result<SignIn, IdpError>;
    fn respond_to_challenge(&self, answer: &ChallengeResponse) -> Result<SignIn, IdpError>;
    fn sign_up(&self, request: &SignUpRequest) -> Result<SignUp, IdpError>;
    /// `session` is the provider's continuation for the conversation, when its steps span requests (empty otherwise).
    fn confirm_sign_up(&self, username: &str, code: &str, session: &str) -> Result<(), IdpError>;
    /// Sends the code again; the answer is the provider's new continuation, when it rotates it.
    fn resend_code(&self, username: &str, session: &str) -> Result<Option<String>, IdpError>;
    /// Starts a reset; the answer is the provider's continuation, when it has one.
    fn forgot_password(&self, username: &str) -> Result<Option<String>, IdpError>;
    fn confirm_forgot_password(&self, username: &str, code: &str, new_password: &str, session: &str) -> Result<(), IdpError>;
    /// `username` is for providers whose refresh needs it (Cognito with a client secret).
    fn refresh(&self, refresh_token: &str, username: &str) -> Result<Tokens, IdpError>;
    fn sign_out(&self, access_token: &str, refresh_token: Option<&str>) -> Result<(), IdpError>;

    /// The address of the provider's own page for sign-in.
    fn start_hosted(&self, redirect_uri: &str, state: &str, code_challenge: &str, nonce: &str) -> Result<String, IdpError>;
    /// Exchange the code the provider's page returned for tokens.
    fn finish_hosted(&self, code: &str, redirect_uri: &str, code_verifier: &str) -> Result<Tokens, IdpError>;
}

/// The adapter `[identity]` names. Keycloak and both Entra providers need the provider's metadata
/// (their endpoints are wherever it says); Cognito's are fixed by its region and its domain.
pub fn adapter(identity: &ResolvedIdentity, discovery: Option<&Discovery>, http: Arc<dyn client::Http>) -> Result<Box<dyn IdentityProvider>, String> {
    let setting = |k: &str| identity.settings.get(k).cloned().filter(|v| !v.is_empty());
    let secret = identity.client_secret.as_ref().map(|s| s.expose().to_owned());
    let oidc = |default_scopes: &str| -> Result<oidc::Oidc, String> {
        let d = discovery.ok_or_else(|| format!("the {} adapter needs the provider's metadata, which was not read", identity.provider.word()))?;
        Ok(oidc::Oidc::from_discovery(http.clone(), d, &identity.client_id, secret.as_deref(), &setting("scopes").unwrap_or_else(|| default_scopes.to_string())))
    };
    Ok(match identity.provider {
        Provider::Cognito => Box::new(cognito::Cognito::new(
            http.clone(),
            cognito::CognitoSettings {
                region: setting("region").unwrap_or_default(),
                user_pool_id: setting("user_pool_id").unwrap_or_default(),
                client_id: identity.client_id.clone(),
                client_secret: secret.clone(),
                endpoint: setting("endpoint"),
                domain: setting("domain"),
                scopes: setting("scopes"),
                flow: identity.flow,
                refresh_rotation: setting("refresh_rotation").is_some_and(|v| v.eq_ignore_ascii_case("true")),
                totp_issuer: setting("totp_issuer"),
            },
        )),
        Provider::Keycloak => Box::new(keycloak::Keycloak::new(oidc("openid profile email")?)),
        Provider::Entra | Provider::EntraExternal => {
            let native = (identity.flow == Flow::Native).then(|| identity.authority());
            Box::new(entra::Entra::new(http.clone(), oidc("openid profile email offline_access")?, identity.provider == Provider::EntraExternal, native))
        }
    })
}

/// The identity service `[identity]` describes, around an adapter.
pub fn service(identity: &ResolvedIdentity, provider: Box<dyn IdentityProvider>, http: Arc<dyn client::Http>) -> Result<ops::IdentityService, String> {
    let sealer = seal::Sealer::from_setting(identity.cookie_key.expose()).map_err(|e| format!("identity.cookie_key: {e}"))?;
    let captcha: Option<Box<dyn recaptcha::CaptchaVerifier>> = identity.recaptcha.as_ref().map(|r| Box::new(recaptcha::Recaptcha::new(http.clone(), r)) as Box<dyn recaptcha::CaptchaVerifier>);
    let cfg = ops::ServiceConfig {
        attempts_per_minute: identity.attempts_per_minute,
        floor: Duration::from_millis(250),
        origins: identity.origins.clone(),
        site_key: identity.recaptcha.as_ref().and_then(|r| r.site_key.clone()),
    };
    Ok(ops::IdentityService::new(provider, captcha, sealer, cfg))
}

