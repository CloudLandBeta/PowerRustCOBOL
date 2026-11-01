// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Authentication control, front half** (WASM-009).
//!
//! The screens and flows of signing in, creating an account, confirming it, recovering a password
//! and answering a second factor — as a *sans-IO* state machine ([`Machine`]) that never talks to
//! a network: it turns a user's action into [`Op`]s (a backend call to make, an event to raise,
//! a reCAPTCHA token to obtain) and takes the answer back through [`Machine::on_result`]. The
//! backend sits behind the [`AuthBackend`] trait so the flows are tested against a mock
//! (`MockBackend`) with no provider and no socket; the shell implements the trait over the
//! player's connection to the project's backend (WASM-003/005), whose identity programs talk to
//! Cognito, Keycloak or Entra (WASM-009 R7–R8, not this crate).
//!
//! Rules this module enforces, each with a test:
//!
//! * **R5** the access token is held in memory only — the [`Session`] has no way to serialise it
//!   and its `Debug` redacts it — and the refresh credential is not here at all (an `HttpOnly`
//!   cookie the backend sets);
//! * **R6** a token is never in a log or a visible text, and `GetClaim` returns a claim, not a token;
//! * **R10** a failed sign-in shows one generic message whether the account exists or not, and a
//!   password-reset request shows one message whether the address is known or not;
//! * **R4** where reCAPTCHA v3 is on, sign-in, sign-up and forgot-password obtain a token for the
//!   action first and send it with the request; the secret key is the backend's.
//!
//! The flow is chosen **per provider** (R8b): `Flow::InApp` shows the control's own screens;
//! `Flow::Hosted` offers one "continue with the provider" action and the provider's pages do the
//! rest (authorisation code + PKCE, brokered by the backend). The events a form's handlers get are
//! the same in both.

use std::collections::BTreeMap;

// ── the session ─────────────────────────────────────────────────────────────

/// The signed-in session. The access token is in memory only (R5).
#[derive(Default, Clone)]
pub struct Session {
    token: Option<String>,
    claims: BTreeMap<String, String>,
    /// When the token expires, ms since the epoch.
    pub expires_at_ms: Option<f64>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("claims", &self.claims.keys().collect::<Vec<_>>())
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

impl Session {
    pub fn is_signed_in(&self) -> bool {
        self.token.is_some()
    }

    /// Start a session from an access token. Its claims are read for `GetClaim` and display —
    /// **not verified**: the backend validates the token against the issuer's keys (WASM-003 R10).
    pub fn sign_in(&mut self, token: &str, expires_in_s: Option<f64>, now_ms: f64) {
        self.claims = decode_claims(token);
        self.token = Some(token.to_owned());
        self.expires_at_ms = expires_in_s
            .map(|s| now_ms + s * 1000.0)
            .or_else(|| self.claims.get("exp").and_then(|e| e.parse::<f64>().ok()).map(|e| e * 1000.0));
    }

    pub fn sign_out(&mut self) {
        *self = Session::default();
    }

    /// The raw token, for the transport's first message. Crate-private on purpose.
    pub(crate) fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// `GetClaim`: a claim's value as text, empty when absent (never the token).
    pub fn claim(&self, name: &str) -> String {
        self.claims.get(name.trim()).cloned().unwrap_or_default()
    }

    /// Is the token within `margin_s` seconds of expiring?
    pub fn expiring(&self, now_ms: f64, margin_s: f64) -> bool {
        matches!(self.expires_at_ms, Some(e) if e - now_ms <= margin_s * 1000.0)
    }
}

fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The claims of a JWT's payload as text (numbers and booleans printed, arrays joined with `,`).
pub fn decode_claims(token: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(payload) = token.split('.').nth(1) else { return out };
    let Some(bytes) = b64url_decode(payload) else { return out };
    let Ok(serde_json::Value::Object(map)) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return out };
    for (k, v) in map {
        let text = match v {
            serde_json::Value::String(s) => s,
            serde_json::Value::Array(a) => {
                a.iter().map(|x| x.as_str().map(str::to_owned).unwrap_or_else(|| x.to_string())).collect::<Vec<_>>().join(",")
            }
            other => other.to_string(),
        };
        out.insert(k, text);
    }
    out
}

// ── the machine ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    SignIn,
    SignUp,
    Confirm,
    Forgot,
    Reset,
    Mfa,
    /// The provider wants a new password before it signs the user in (a temporary one; D16).
    NewPassword,
    /// The provider offers several second factors: one button per kind.
    MfaChoose,
    /// The user enrols an authenticator app: its link, its key, then a code.
    MfaSetup,
    SignedIn,
}

impl Screen {
    pub fn name(self) -> &'static str {
        match self {
            Screen::SignIn => "SignIn",
            Screen::SignUp => "SignUp",
            Screen::Confirm => "Confirm",
            Screen::Forgot => "Forgot",
            Screen::Reset => "Reset",
            Screen::Mfa => "Mfa",
            Screen::NewPassword => "NewPassword",
            Screen::MfaChoose => "MfaChoose",
            Screen::MfaSetup => "MfaSetup",
            Screen::SignedIn => "SignedIn",
        }
    }

    /// A screen that answers a provider's challenge: reached from an answer, never by `Show`.
    fn is_challenge(self) -> bool {
        matches!(self, Screen::Mfa | Screen::NewPassword | Screen::MfaChoose | Screen::MfaSetup)
    }
}

/// A second factor the provider can send a code through (`Choice` lists them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaChoice {
    Totp,
    Sms,
    Email,
}

impl MfaChoice {
    /// Its name on the wire.
    pub fn wire_name(self) -> &'static str {
        match self {
            MfaChoice::Totp => "totp",
            MfaChoice::Sms => "sms",
            MfaChoice::Email => "email",
        }
    }

    pub fn from_wire(s: &str) -> Option<MfaChoice> {
        match s {
            "totp" => Some(MfaChoice::Totp),
            "sms" => Some(MfaChoice::Sms),
            "email" => Some(MfaChoice::Email),
            _ => None,
        }
    }
}

/// What a challenge asks for (WASM-009 D2): a code from an app, by text message or by e-mail (with
/// the masked destination the provider reported), a choice between them, the enrolment of an
/// authenticator app, or a new password.
#[derive(Clone, PartialEq, Eq)]
pub enum MfaKind {
    Totp,
    Sms { masked: String },
    Email { masked: String },
    Choice(Vec<MfaChoice>),
    /// The key is shown to the user, who types it into the app; it is never logged.
    TotpSetup { secret: String, otpauth_uri: String },
    NewPassword,
}

impl std::fmt::Debug for MfaKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MfaKind::Totp => f.write_str("Totp"),
            MfaKind::Sms { masked } => f.debug_struct("Sms").field("masked", masked).finish(),
            MfaKind::Email { masked } => f.debug_struct("Email").field("masked", masked).finish(),
            MfaKind::Choice(c) => f.debug_tuple("Choice").field(c).finish(),
            MfaKind::TotpSetup { .. } => f.debug_struct("TotpSetup").field("secret", &Hidden).field("otpauth_uri", &Hidden).finish(),
            MfaKind::NewPassword => f.write_str("NewPassword"),
        }
    }
}

impl MfaKind {
    /// The screen that answers it.
    pub fn screen(&self) -> Screen {
        match self {
            MfaKind::Totp | MfaKind::Sms { .. } | MfaKind::Email { .. } => Screen::Mfa,
            MfaKind::Choice(_) => Screen::MfaChoose,
            MfaKind::TotpSetup { .. } => Screen::MfaSetup,
            MfaKind::NewPassword => Screen::NewPassword,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    InApp,
    Hosted,
}

/// Which screens the developer included, and how the control behaves.
#[derive(Debug, Clone)]
pub struct Config {
    pub flow: Flow,
    pub sign_up: bool,
    pub confirm: bool,
    pub forgot: bool,
    pub mfa: bool,
    /// The reCAPTCHA v3 *site* key (public); empty = off.
    pub recaptcha_site_key: String,
    /// `RecaptchaConsent = Ask` (D10): the first protected action waits for `GrantConsent`.
    pub consent_ask: bool,
    pub texts: Texts,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            flow: Flow::InApp,
            sign_up: true,
            confirm: true,
            forgot: true,
            mfa: true,
            recaptcha_site_key: String::new(),
            consent_ask: false,
            texts: Texts::default(),
        }
    }
}

/// The messages the machine shows, translatable by the developer (WASM-009 R1). An empty property
/// is the default in the application's language (`cobolt_forms::auth_texts`, D12).
#[derive(Debug, Clone)]
pub struct Texts {
    pub sign_in_failed: String,
    pub reset_requested: String,
    pub generic_error: String,
    pub passwords_differ: String,
}

impl Default for Texts {
    /// The English defaults.
    fn default() -> Self {
        Texts::read(&|_| String::new(), "en")
    }
}

impl Texts {
    /// The messages through `get` (a property by name; empty = not set), defaults in `locale`.
    pub fn read(get: &dyn Fn(&str) -> String, locale: &str) -> Texts {
        let pick = |prop: &str| cobolt_forms::auth_texts::text_or_default(&get(prop), prop, locale);
        Texts {
            sign_in_failed: pick("SignInFailedText"),
            reset_requested: pick("ResetRequestedText"),
            generic_error: pick("ErrorText"),
            passwords_differ: pick("PasswordsDifferText"),
        }
    }
}

/// Prints as `<redacted>`: what stands in for a password, a code or a token in a `Debug` output
/// (R6: they never reach a log; R10: passwords are never logged).
struct Hidden;

impl std::fmt::Debug for Hidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// A string whose `Debug` is [`Hidden`].
#[derive(Clone, PartialEq, Eq)]
struct Secret(String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Hidden.fmt(f)
    }
}

/// What the user did.
#[derive(Clone, PartialEq)]
pub enum Action {
    Show(Screen),
    SignIn { user: String, password: String },
    SignUp { user: String, email: String, password: String },
    Confirm { user: String, code: String },
    Forgot { user: String },
    Reset { user: String, code: String, new_password: String },
    /// A code for the challenge on screen: `Mfa`, or `MfaSetup`'s first code from the new app.
    Mfa { code: String },
    /// The New password screen: the password, typed twice.
    NewPassword { password: String, repeat: String },
    /// The Choose screen: the second factor the user picked.
    MfaChoose(MfaChoice),
    Hosted,
    /// The page came back from the provider with an authorisation code (the hosted flow).
    HostedFinish { code: String, state: String },
    /// The program's `GrantConsent` / `DenyConsent` (D10).
    GrantConsent,
    DenyConsent,
    SignOut,
    /// The program asked (`Refresh`), or the server refused an expired token: refresh now.
    Refresh,
    /// The clock ticked: refresh if the token is about to expire.
    Tick { now_ms: f64 },
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Show(s) => f.debug_tuple("Show").field(s).finish(),
            Action::SignIn { user, .. } => f.debug_struct("SignIn").field("user", user).field("password", &Hidden).finish(),
            Action::SignUp { user, email, .. } => f.debug_struct("SignUp").field("user", user).field("email", email).field("password", &Hidden).finish(),
            Action::Confirm { user, .. } => f.debug_struct("Confirm").field("user", user).field("code", &Hidden).finish(),
            Action::Forgot { user } => f.debug_struct("Forgot").field("user", user).finish(),
            Action::Reset { user, .. } => f.debug_struct("Reset").field("user", user).field("code", &Hidden).field("new_password", &Hidden).finish(),
            Action::Mfa { .. } => f.debug_struct("Mfa").field("code", &Hidden).finish(),
            Action::NewPassword { .. } => f.debug_struct("NewPassword").field("password", &Hidden).field("repeat", &Hidden).finish(),
            Action::MfaChoose(c) => f.debug_tuple("MfaChoose").field(c).finish(),
            Action::Hosted => f.write_str("Hosted"),
            Action::HostedFinish { .. } => f.debug_struct("HostedFinish").field("code", &Hidden).field("state", &Hidden).finish(),
            Action::GrantConsent => f.write_str("GrantConsent"),
            Action::DenyConsent => f.write_str("DenyConsent"),
            Action::SignOut => f.write_str("SignOut"),
            Action::Refresh => f.write_str("Refresh"),
            Action::Tick { now_ms } => f.debug_struct("Tick").field("now_ms", now_ms).finish(),
        }
    }
}

/// The backend calls the identity programs of the project's Backend answer (WASM-009 R7).
#[derive(Clone, PartialEq, Eq)]
pub enum BackendCall {
    SignIn { user: String, password: String, captcha: Option<String> },
    SignUp { user: String, email: String, password: String, captcha: Option<String> },
    Confirm { user: String, code: String },
    Forgot { user: String, captcha: Option<String> },
    Reset { user: String, code: String, new_password: String },
    Mfa { session: String, code: String },
    /// The answer to a new-password challenge.
    NewPassword { session: String, new_password: String },
    /// The second factor picked on the Choose screen.
    MfaChoose { session: String, choice: MfaChoice },
    /// The first code of a newly enrolled authenticator app.
    MfaSetup { session: String, code: String },
    /// Silent refresh with the `HttpOnly` cookie the browser holds.
    Refresh,
    SignOut,
    /// The hosted flow: ask the backend for the provider's sign-in address.
    HostedStart,
    /// The hosted flow came back with an authorisation code.
    HostedFinish { code: String, state: String },
}

impl std::fmt::Debug for BackendCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hide = |o: &Option<String>| o.as_ref().map(|_| Hidden);
        match self {
            BackendCall::SignIn { user, captcha, .. } => {
                f.debug_struct("SignIn").field("user", user).field("password", &Hidden).field("captcha", &hide(captcha)).finish()
            }
            BackendCall::SignUp { user, email, captcha, .. } => {
                f.debug_struct("SignUp").field("user", user).field("email", email).field("password", &Hidden).field("captcha", &hide(captcha)).finish()
            }
            BackendCall::Confirm { user, .. } => f.debug_struct("Confirm").field("user", user).field("code", &Hidden).finish(),
            BackendCall::Forgot { user, captcha } => f.debug_struct("Forgot").field("user", user).field("captcha", &hide(captcha)).finish(),
            BackendCall::Reset { user, .. } => f.debug_struct("Reset").field("user", user).field("code", &Hidden).field("new_password", &Hidden).finish(),
            BackendCall::Mfa { .. } => f.debug_struct("Mfa").field("session", &Hidden).field("code", &Hidden).finish(),
            BackendCall::NewPassword { .. } => f.debug_struct("NewPassword").field("session", &Hidden).field("new_password", &Hidden).finish(),
            BackendCall::MfaChoose { choice, .. } => f.debug_struct("MfaChoose").field("session", &Hidden).field("choice", choice).finish(),
            BackendCall::MfaSetup { .. } => f.debug_struct("MfaSetup").field("session", &Hidden).field("code", &Hidden).finish(),
            BackendCall::Refresh => f.write_str("Refresh"),
            BackendCall::SignOut => f.write_str("SignOut"),
            BackendCall::HostedStart => f.write_str("HostedStart"),
            BackendCall::HostedFinish { .. } => f.debug_struct("HostedFinish").field("code", &Hidden).field("state", &Hidden).finish(),
        }
    }
}

/// What a backend call came back with.
#[derive(Clone, PartialEq)]
pub enum AuthResult {
    SignedIn { access_token: String, expires_in_s: Option<f64> },
    NeedsConfirm,
    /// A challenge: `session` is the provider's, sent back with the answer; `kind` says what is asked.
    NeedsMfa { session: String, kind: MfaKind },
    /// A temporary password must be replaced first (Cognito `NEW_PASSWORD_REQUIRED`): the same as
    /// `NeedsMfa` with `MfaKind::NewPassword`.
    NeedsNewPassword { session: String },
    ResetRequested,
    Done,
    Hosted { redirect_url: String },
    /// Any failure. The detail is for the log (without secrets); the user is never shown it (R10).
    Failed { detail: String },
}

impl std::fmt::Debug for AuthResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthResult::SignedIn { expires_in_s, .. } => f.debug_struct("SignedIn").field("access_token", &Hidden).field("expires_in_s", expires_in_s).finish(),
            AuthResult::NeedsConfirm => f.write_str("NeedsConfirm"),
            AuthResult::NeedsMfa { kind, .. } => f.debug_struct("NeedsMfa").field("session", &Hidden).field("kind", kind).finish(),
            AuthResult::NeedsNewPassword { .. } => f.debug_struct("NeedsNewPassword").field("session", &Hidden).finish(),
            AuthResult::ResetRequested => f.write_str("ResetRequested"),
            AuthResult::Done => f.write_str("Done"),
            AuthResult::Hosted { redirect_url } => f.debug_struct("Hosted").field("redirect_url", redirect_url).finish(),
            AuthResult::Failed { detail } => f.debug_struct("Failed").field("detail", detail).finish(),
        }
    }
}

/// Events for the form's handlers (WASM-009 R3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthEvent {
    SignedIn,
    SignedOut,
    SignUpCompleted,
    PasswordResetRequested,
    MfaRequired,
    /// A protected action waits for `GrantConsent` / `DenyConsent` (D10).
    ConsentRequired,
    Error(String),
}

impl AuthEvent {
    pub fn handler_name(&self) -> &'static str {
        match self {
            AuthEvent::SignedIn => "onSignedIn",
            AuthEvent::SignedOut => "onSignedOut",
            AuthEvent::SignUpCompleted => "onSignUpCompleted",
            AuthEvent::PasswordResetRequested => "onPasswordResetRequested",
            AuthEvent::MfaRequired => "onMfaRequired",
            AuthEvent::ConsentRequired => "onConsentRequired",
            AuthEvent::Error(_) => "onError",
        }
    }
}

/// What the machine asks the shell to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Backend(BackendCall),
    /// Obtain a reCAPTCHA v3 token for this action name, then call [`Machine::captcha_token`].
    Captcha { action: &'static str },
    Notify(AuthEvent),
    /// Show this message on the current screen (a status line, not a token).
    Message(String),
    /// The hosted flow: go to the provider's page.
    Redirect(String),
}

#[derive(Clone, PartialEq)]
enum Pending {
    SignIn { user: String, password: String },
    SignUp { user: String, email: String, password: String },
    Forgot { user: String },
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pending::SignIn { user, .. } => f.debug_struct("SignIn").field("user", user).field("password", &Hidden).finish(),
            Pending::SignUp { user, email, .. } => f.debug_struct("SignUp").field("user", user).field("email", email).field("password", &Hidden).finish(),
            Pending::Forgot { user } => f.debug_struct("Forgot").field("user", user).finish(),
        }
    }
}

/// The state machine of one Authentication control.
#[derive(Debug, Clone)]
pub struct Machine {
    pub cfg: Config,
    pub screen: Screen,
    pub session: Session,
    pub user_name: String,
    pub email: String,
    mfa_session: Option<Secret>,
    /// What the challenge on screen asks for (the MFA, Choose, Set-up and New password screens).
    pub mfa_kind: Option<MfaKind>,
    /// A sign-in/up/forgot waiting for its reCAPTCHA token.
    pending: Option<Pending>,
    /// A sign-in/up/forgot held until the program grants or denies consent (D10).
    held: Option<(Pending, &'static str)>,
    /// Consent to load reCAPTCHA was given; it holds for the page's lifetime (the host shares it).
    pub consent_given: bool,
    /// A backend call is in flight.
    pub busy: bool,
    /// Who the current flow is for (confirm / reset screens).
    flow_user: String,
}

impl Machine {
    pub fn new(cfg: Config) -> Machine {
        Machine {
            cfg,
            screen: Screen::SignIn,
            session: Session::default(),
            user_name: String::new(),
            email: String::new(),
            mfa_session: None,
            mfa_kind: None,
            pending: None,
            held: None,
            consent_given: false,
            busy: false,
            flow_user: String::new(),
        }
    }

    /// Is a protected action held, waiting for `GrantConsent` / `DenyConsent`?
    pub fn consent_pending(&self) -> bool {
        self.held.is_some()
    }

    pub fn state_name(&self) -> &'static str {
        if self.session.is_signed_in() {
            "SignedIn"
        } else {
            "SignedOut"
        }
    }

    fn captcha_on(&self) -> bool {
        !self.cfg.recaptcha_site_key.is_empty()
    }

    fn allowed(&self, s: Screen) -> bool {
        match s {
            Screen::SignIn | Screen::SignedIn => true,
            Screen::SignUp => self.cfg.sign_up && self.cfg.flow == Flow::InApp,
            Screen::Confirm => self.cfg.confirm && self.cfg.flow == Flow::InApp,
            Screen::Forgot | Screen::Reset => self.cfg.forgot && self.cfg.flow == Flow::InApp,
            // A challenge's screens answer a provider's question: only an answer opens them.
            Screen::Mfa | Screen::NewPassword | Screen::MfaChoose | Screen::MfaSetup => false,
        }
    }

    /// The challenge session, if one is open and the screen on show answers it.
    fn challenge(&self, on: Screen) -> Option<String> {
        if self.screen != on {
            return None;
        }
        self.mfa_session.as_ref().map(|Secret(s)| s.clone())
    }

    /// The user acted.
    pub fn act(&mut self, a: Action) -> Vec<Op> {
        match a {
            Action::Show(s) => {
                if self.allowed(s) && !self.session.is_signed_in() {
                    if self.screen.is_challenge() {
                        // Leaving a challenge abandons it: its session is no use any more.
                        self.mfa_session = None;
                        self.mfa_kind = None;
                    }
                    self.screen = s;
                }
                vec![]
            }
            Action::SignIn { user, password } => self.guarded(Pending::SignIn { user, password }, "sign_in"),
            Action::SignUp { user, email, password } => self.guarded(Pending::SignUp { user, email, password }, "sign_up"),
            Action::Forgot { user } => self.guarded(Pending::Forgot { user }, "forgot_password"),
            Action::Confirm { user, code } => self.call(BackendCall::Confirm { user, code }),
            Action::Reset { user, code, new_password } => self.call(BackendCall::Reset { user, code, new_password }),
            Action::Mfa { code } => {
                if let Some(session) = self.challenge(Screen::Mfa) {
                    self.call(BackendCall::Mfa { session, code })
                } else if let Some(session) = self.challenge(Screen::MfaSetup) {
                    self.call(BackendCall::MfaSetup { session, code })
                } else {
                    vec![]
                }
            }
            Action::NewPassword { password, repeat } => {
                let Some(session) = self.challenge(Screen::NewPassword) else { return vec![] };
                if password != repeat {
                    // Checked here, not by the provider: a typing slip costs no attempt.
                    return vec![Op::Message(self.cfg.texts.passwords_differ.clone())];
                }
                self.call(BackendCall::NewPassword { session, new_password: password })
            }
            Action::MfaChoose(choice) => {
                let offered = matches!(&self.mfa_kind, Some(MfaKind::Choice(c)) if c.contains(&choice));
                match self.challenge(Screen::MfaChoose) {
                    Some(session) if offered => self.call(BackendCall::MfaChoose { session, choice }),
                    _ => vec![],
                }
            }
            Action::Hosted => self.call(BackendCall::HostedStart),
            Action::HostedFinish { code, state } => self.call(BackendCall::HostedFinish { code, state }),
            Action::GrantConsent => {
                self.consent_given = true;
                match self.held.take() {
                    Some((p, action)) => {
                        self.busy = false;
                        self.guarded(p, action)
                    }
                    None => vec![],
                }
            }
            Action::DenyConsent => match self.held.take() {
                Some(_) => {
                    self.busy = false;
                    vec![Op::Message(self.cfg.texts.generic_error.clone()), Op::Notify(AuthEvent::Error("consent/declined".into()))]
                }
                None => vec![],
            },
            Action::Refresh => {
                if self.session.is_signed_in() && !self.busy {
                    self.call(BackendCall::Refresh)
                } else {
                    vec![]
                }
            }
            Action::SignOut => {
                // Signing out forgets the token at once; the backend revokes the refresh cookie.
                let was = self.session.is_signed_in();
                self.session.sign_out();
                self.user_name.clear();
                self.email.clear();
                self.mfa_session = None;
                self.mfa_kind = None;
                self.screen = Screen::SignIn;
                let mut ops = vec![Op::Backend(BackendCall::SignOut)];
                if was {
                    ops.push(Op::Notify(AuthEvent::SignedOut));
                }
                ops
            }
            Action::Tick { now_ms } => {
                if self.session.is_signed_in() && !self.busy && self.session.expiring(now_ms, 60.0) {
                    self.call(BackendCall::Refresh)
                } else {
                    vec![]
                }
            }
        }
    }

    /// Another control of the same application changed the session (one session per application):
    /// follow it, without raising anything.
    pub fn adopt(&mut self, session: &Session) {
        self.session = session.clone();
        if session.is_signed_in() {
            self.user_name = first_nonempty(&[session.claim("cognito:username"), session.claim("preferred_username"), self.user_name.clone()]);
            let email = session.claim("email");
            if !email.is_empty() {
                self.email = email;
            }
            self.screen = Screen::SignedIn;
        } else {
            self.user_name.clear();
            self.email.clear();
            if self.screen == Screen::SignedIn {
                self.screen = Screen::SignIn;
            }
        }
    }

    /// A sign-in, sign-up or forgot request: with reCAPTCHA on it waits for the token first (R4);
    /// with `RecaptchaConsent = Ask` and no consent yet, it waits for the program first (D10), and
    /// nothing of reCAPTCHA is loaded meanwhile.
    fn guarded(&mut self, p: Pending, action: &'static str) -> Vec<Op> {
        if self.busy {
            return vec![];
        }
        if self.captcha_on() && self.cfg.consent_ask && !self.consent_given {
            self.held = Some((p, action));
            self.busy = true;
            return vec![Op::Notify(AuthEvent::ConsentRequired)];
        }
        if self.captcha_on() {
            self.pending = Some(p);
            self.busy = true;
            return vec![Op::Captcha { action }];
        }
        self.send(p, None)
    }

    /// The token reCAPTCHA gave for the pending action (`None` when the script failed to load or
    /// to answer). With no token there is no request, and the user gets the sign-in message (R10).
    pub fn captcha_token(&mut self, token: Option<String>) -> Vec<Op> {
        let Some(p) = self.pending.take() else { return vec![] };
        self.busy = false;
        if token.is_none() {
            return vec![Op::Message(self.cfg.texts.sign_in_failed.clone()), Op::Notify(AuthEvent::Error("captcha".into()))];
        }
        self.send(p, token)
    }

    fn send(&mut self, p: Pending, token: Option<String>) -> Vec<Op> {
        match p {
            Pending::SignIn { user, password } => {
                self.flow_user = user.clone();
                self.call(BackendCall::SignIn { user, password, captcha: token })
            }
            Pending::SignUp { user, email, password } => {
                self.flow_user = user.clone();
                self.email = email.clone();
                self.call(BackendCall::SignUp { user, email, password, captcha: token })
            }
            Pending::Forgot { user } => {
                self.flow_user = user.clone();
                self.call(BackendCall::Forgot { user, captcha: token })
            }
        }
    }

    fn call(&mut self, c: BackendCall) -> Vec<Op> {
        if self.busy && !matches!(c, BackendCall::SignOut) {
            return vec![];
        }
        self.busy = true;
        vec![Op::Backend(c)]
    }

    /// The identity programs answered.
    pub fn on_result(&mut self, call: &BackendCall, r: AuthResult, now_ms: f64) -> Vec<Op> {
        self.busy = false;
        let t = self.cfg.texts.clone();
        match (call, r) {
            (_, AuthResult::SignedIn { access_token, expires_in_s }) => {
                let was = self.session.is_signed_in();
                self.session.sign_in(&access_token, expires_in_s, now_ms);
                if self.user_name.is_empty() {
                    self.user_name = self.session.claim("cognito:username");
                    if self.user_name.is_empty() {
                        self.user_name = self.session.claim("preferred_username");
                    }
                    if self.user_name.is_empty() {
                        self.user_name = self.flow_user.clone();
                    }
                }
                let email = self.session.claim("email");
                if !email.is_empty() {
                    self.email = email;
                }
                self.screen = Screen::SignedIn;
                self.mfa_session = None;
                self.mfa_kind = None;
                // A silent refresh does not announce a new sign-in.
                if matches!(call, BackendCall::Refresh) && was {
                    vec![]
                } else if matches!(call, BackendCall::SignUp { .. }) {
                    // A provider that signs a new account straight in: the account is usable (C8).
                    vec![Op::Notify(AuthEvent::SignUpCompleted), Op::Notify(AuthEvent::SignedIn)]
                } else {
                    vec![Op::Notify(AuthEvent::SignedIn)]
                }
            }
            // The account exists but is not usable yet: to the Confirm screen, and no event — it is
            // raised once, when the confirmation succeeds (C8). Without a Confirm screen the user
            // confirms elsewhere (the provider's e-mail link) and signs in afterwards.
            (BackendCall::SignUp { .. } | BackendCall::SignIn { .. }, AuthResult::NeedsConfirm) => {
                self.screen = if self.cfg.confirm { Screen::Confirm } else { Screen::SignIn };
                vec![]
            }
            // The account is usable: confirmed, or created by a provider that needs no confirmation.
            (BackendCall::SignUp { .. } | BackendCall::Confirm { .. }, AuthResult::Done) => {
                self.screen = Screen::SignIn;
                vec![Op::Notify(AuthEvent::SignUpCompleted)]
            }
            (_, AuthResult::NeedsNewPassword { session }) => self.challenged(call, session, MfaKind::NewPassword),
            (_, AuthResult::NeedsMfa { session, kind }) => self.challenged(call, session, kind),
            (BackendCall::Forgot { .. }, _) => {
                // The same message whether or not the address is known (R10).
                self.screen = Screen::Reset;
                vec![Op::Message(t.reset_requested), Op::Notify(AuthEvent::PasswordResetRequested)]
            }
            (BackendCall::Reset { .. }, AuthResult::Done) => {
                self.screen = Screen::SignIn;
                vec![]
            }
            (BackendCall::HostedStart, AuthResult::Hosted { redirect_url }) => vec![Op::Redirect(redirect_url)],
            (BackendCall::SignOut, _) => vec![],
            (BackendCall::Refresh, AuthResult::Failed { detail }) => {
                // The refresh credential is gone or refused: the user is signed out (R5).
                self.session.sign_out();
                self.screen = Screen::SignIn;
                vec![Op::Notify(AuthEvent::SignedOut), Op::Notify(AuthEvent::Error(detail))]
            }
            (
                BackendCall::SignIn { .. }
                | BackendCall::Mfa { .. }
                | BackendCall::MfaSetup { .. }
                | BackendCall::MfaChoose { .. }
                | BackendCall::NewPassword { .. }
                | BackendCall::HostedFinish { .. },
                AuthResult::Failed { detail },
            ) => {
                // One message for an unknown user and a wrong password alike (R10).
                vec![Op::Message(t.sign_in_failed), Op::Notify(AuthEvent::Error(detail))]
            }
            (_, AuthResult::Failed { detail }) => vec![Op::Message(t.generic_error), Op::Notify(AuthEvent::Error(detail))],
            (_, _) => vec![Op::Message(t.generic_error)],
        }
    }

    /// The provider answered with a challenge: open the screen that answers it. `onMfaRequired` is
    /// raised when a second factor starts, not again when one challenge leads to the next (a Choose
    /// then its code), and never for a new password, which is not a second factor.
    fn challenged(&mut self, call: &BackendCall, session: String, kind: MfaKind) -> Vec<Op> {
        let is_mfa = kind != MfaKind::NewPassword;
        if is_mfa && !self.cfg.mfa {
            // The developer offers no second-factor screen (`ShowMfa` off): the control cannot answer.
            self.mfa_session = None;
            self.mfa_kind = None;
            self.screen = Screen::SignIn;
            return vec![Op::Message(self.cfg.texts.generic_error.clone()), Op::Notify(AuthEvent::Error("mfa/not-offered".into()))];
        }
        let follows = matches!(call, BackendCall::Mfa { .. } | BackendCall::MfaChoose { .. } | BackendCall::MfaSetup { .. });
        self.mfa_session = Some(Secret(session));
        self.screen = kind.screen();
        self.mfa_kind = Some(kind);
        if is_mfa && !follows {
            vec![Op::Notify(AuthEvent::MfaRequired)]
        } else {
            vec![]
        }
    }
}

fn first_nonempty(v: &[String]) -> String {
    v.iter().find(|s| !s.is_empty()).cloned().unwrap_or_default()
}

// ── what the control shows ──────────────────────────────────────────────────

/// One Authentication control's live state: its machine and what the screen says.
#[derive(Debug, Clone)]
pub struct AuthUi {
    pub machine: Machine,
    /// The status line under the form (a generic failure, "a code was sent"); never a token.
    pub message: String,
    /// Counts the answers received: a new form is made after each, so a failed password field is empty.
    pub attempt: u32,
    /// The user name last submitted, offered again after a failure.
    pub user_hint: String,
    /// The user has acted on this control: a screen made from now on takes the keyboard focus (the
    /// one they were in has just been replaced). Never set on the first screen, which must not
    /// steal the focus from the page when it loads.
    pub interacted: bool,
    /// The language of the defaults the screens show: the application's `Locale` (ABI §12.1),
    /// else the page's (D12). The host keeps it current.
    pub locale: String,
}

impl AuthUi {
    pub fn new(cfg: Config) -> AuthUi {
        AuthUi { machine: Machine::new(cfg), message: String::new(), attempt: 0, user_hint: String::new(), interacted: false, locale: "en".into() }
    }
}

/// The words on the control. Every one is a property the developer can set (WASM-009 R1); an empty
/// one is the default in the application's language (`cobolt_forms::auth_texts`, D12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    pub sign_in: String,
    pub sign_up: String,
    pub confirm: String,
    pub forgot: String,
    pub reset: String,
    pub mfa: String,
    pub sign_out: String,
    pub hosted: String,
    pub user: String,
    pub email: String,
    pub password: String,
    pub new_password: String,
    pub code: String,
    pub create_account: String,
    pub forgot_link: String,
    pub back: String,
    pub signed_in_as: String,
    pub busy: String,
    pub new_password_caption: String,
    pub repeat_password: String,
    /// `{0}` stands for the masked destination.
    pub mfa_totp: String,
    pub mfa_sms: String,
    pub mfa_email: String,
    pub mfa_choose: String,
    pub choice_totp: String,
    pub choice_sms: String,
    pub choice_email: String,
    pub mfa_setup_caption: String,
    pub mfa_setup: String,
    pub secret: String,
}

impl Labels {
    /// Read the labels through `get` (a property by name; empty = not set), defaults in `locale`.
    pub fn read(get: &dyn Fn(&str) -> String, locale: &str) -> Labels {
        let pick = |prop: &str| cobolt_forms::auth_texts::text_or_default(&get(prop), prop, locale);
        Labels {
            sign_in: pick("SignInCaption"),
            sign_up: pick("SignUpCaption"),
            confirm: pick("ConfirmCaption"),
            forgot: pick("ForgotCaption"),
            reset: pick("ResetCaption"),
            mfa: pick("MfaCaption"),
            sign_out: pick("SignOutCaption"),
            hosted: pick("HostedCaption"),
            user: pick("UserLabel"),
            email: pick("EmailLabel"),
            password: pick("PasswordLabel"),
            new_password: pick("NewPasswordLabel"),
            code: pick("CodeLabel"),
            create_account: pick("CreateAccountLink"),
            forgot_link: pick("ForgotLink"),
            back: pick("BackCaption"),
            signed_in_as: pick("SignedInAsText"),
            busy: pick("BusyText"),
            new_password_caption: pick("NewPasswordCaption"),
            repeat_password: pick("RepeatPasswordLabel"),
            mfa_totp: pick("MfaTotpText"),
            mfa_sms: pick("MfaSmsText"),
            mfa_email: pick("MfaEmailText"),
            mfa_choose: pick("MfaChooseCaption"),
            choice_totp: pick("MfaTotpChoice"),
            choice_sms: pick("MfaSmsChoice"),
            choice_email: pick("MfaEmailChoice"),
            mfa_setup_caption: pick("MfaSetupCaption"),
            mfa_setup: pick("MfaSetupText"),
            secret: pick("SecretLabel"),
        }
    }

    /// What the MFA screen says about a code's kind, naming the masked destination.
    pub fn mfa_text(&self, kind: &MfaKind) -> String {
        match kind {
            MfaKind::Sms { masked } => self.mfa_sms.replace("{0}", masked),
            MfaKind::Email { masked } => self.mfa_email.replace("{0}", masked),
            _ => self.mfa_totp.clone(),
        }
    }

    /// The Choose screen's button for `c`.
    pub fn choice(&self, c: MfaChoice) -> &str {
        match c {
            MfaChoice::Totp => &self.choice_totp,
            MfaChoice::Sms => &self.choice_sms,
            MfaChoice::Email => &self.choice_email,
        }
    }
}

/// The control's configuration from its properties; the page's settings fill what the control
/// leaves open. `locale` is the language of the default messages.
pub fn config_from(get: &dyn Fn(&str) -> String, default_flow: Flow, default_site_key: &str, locale: &str) -> Config {
    let on = |prop: &str| {
        let v = get(prop);
        let v = v.trim().to_ascii_lowercase();
        !matches!(v.as_str(), "0" | "false" | "no" | "off")
    };
    let flow = match get("Mode").trim().to_ascii_lowercase().as_str() {
        "hosted" => Flow::Hosted,
        "inapp" | "in-app" | "in_app" => Flow::InApp,
        _ => default_flow,
    };
    let site = get("RecaptchaSiteKey");
    let site = if site.trim().is_empty() { default_site_key.to_owned() } else { site.trim().to_owned() };
    Config {
        flow,
        sign_up: on("ShowSignUp"),
        confirm: on("ShowConfirm"),
        forgot: on("ShowForgot"),
        mfa: on("ShowMfa"),
        recaptcha_site_key: site,
        consent_ask: get("RecaptchaConsent").trim().eq_ignore_ascii_case("ask"),
        texts: Texts::read(get, locale),
    }
}

// ── the identity requests ───────────────────────────────────────────────────

/// How a [`BackendCall`] reaches the backend and how its answer comes back (WASM-009 D5, C2): an
/// HTTPS request to the backend's `/auth/*` routes, never a WebSocket frame. A WebSocket message
/// cannot set a cookie, and the refresh token lives in the backend's `HttpOnly` cookie, which the
/// page never reads. The request body is the identity program's `AUTH-REQUEST` (only the members a
/// page may send); the answer is its `AUTH-RESULT`.
///
/// This module is pure: the browser half (`api.rs`) sends what [`post`] describes with
/// `credentials: "include"`, `Content-Type: application/json` and `X-PR-Identity: 1`, and hands the
/// answer to [`decode`].
pub mod https {
    use super::{AuthResult, BackendCall, Hidden, MfaChoice, MfaKind};
    use serde_json::{Map, Value};

    /// The ids the host gives its identity calls, to match each answer to the call that asked. They
    /// stay in the page: nothing sends them (ABI A8's range is no longer used for identity, T27).
    pub const ID_BASE: u32 = 0x8000_0000;

    pub fn is_auth_id(id: u32) -> bool {
        id >= ID_BASE && id < crate::grid::ID_BASE
    }

    /// The header every identity POST carries: no HTML form can send it, so a cross-site form cannot
    /// post to the routes (the backend refuses a POST without it).
    pub const IDENTITY_HEADER: (&str, &str) = ("X-PR-Identity", "1");

    /// One POST to `/auth/<route>` with a JSON body.
    #[derive(Clone, PartialEq, Eq)]
    pub struct Post {
        pub route: &'static str,
        pub body: String,
    }

    impl Post {
        /// The address on the backend at `base` (`https://host[:port]`).
        pub fn url(&self, base: &str) -> String {
            format!("{}/auth/{}", base.trim_end_matches('/'), self.route)
        }
    }

    /// The body holds passwords and codes: a log shows the route only.
    impl std::fmt::Debug for Post {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Post").field("route", &self.route).field("body", &Hidden).finish()
        }
    }

    fn body(members: &[(&str, &str)]) -> String {
        let mut m = Map::new();
        for (k, v) in members {
            m.insert((*k).to_owned(), Value::String((*v).to_owned()));
        }
        Value::Object(m).to_string()
    }

    /// The POST for `call`. `None` for [`BackendCall::HostedStart`], which is a navigation
    /// ([`hosted_start_url`]), not a request; [`BackendCall::HostedFinish`] posts its handle
    /// ([`hosted_finish`]) once the page has checked the state it kept.
    pub fn post(call: &BackendCall) -> Option<Post> {
        let captcha = |c: &Option<String>| c.clone().unwrap_or_default();
        let (route, members): (&'static str, Vec<(&str, String)>) = match call {
            BackendCall::SignIn { user, password, captcha: c } => ("sign-in", vec![("USERNAME", user.clone()), ("PASSWORD", password.clone()), ("CAPTCHA-TOKEN", captcha(c))]),
            BackendCall::SignUp { user, email, password, captcha: c } => {
                ("sign-up", vec![("USERNAME", user.clone()), ("EMAIL", email.clone()), ("PASSWORD", password.clone()), ("CAPTCHA-TOKEN", captcha(c))])
            }
            BackendCall::Confirm { user, code } => ("confirm", vec![("USERNAME", user.clone()), ("ONE-TIME-CODE", code.clone())]),
            BackendCall::Forgot { user, captcha: c } => ("forgot", vec![("USERNAME", user.clone()), ("CAPTCHA-TOKEN", captcha(c))]),
            BackendCall::Reset { user, code, new_password } => ("reset", vec![("USERNAME", user.clone()), ("ONE-TIME-CODE", code.clone()), ("PASSWORD", new_password.clone())]),
            BackendCall::Mfa { session, code } => ("mfa", vec![("SESSION", session.clone()), ("ONE-TIME-CODE", code.clone())]),
            BackendCall::NewPassword { session, new_password } => ("mfa", vec![("SESSION", session.clone()), ("PASSWORD", new_password.clone())]),
            BackendCall::MfaChoose { session, choice } => ("mfa", vec![("SESSION", session.clone()), ("CHOICE", choice.wire_name().to_owned())]),
            BackendCall::MfaSetup { session, code } => ("mfa-setup", vec![("SESSION", session.clone()), ("ONE-TIME-CODE", code.clone())]),
            BackendCall::Refresh => ("refresh", Vec::new()),
            BackendCall::SignOut => ("sign-out", Vec::new()),
            BackendCall::HostedStart => return None,
            BackendCall::HostedFinish { code, .. } => return Some(hosted_finish(code)),
        };
        let members: Vec<(&str, &str)> = members.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| (*k, v.as_str())).collect();
        Some(Post { route, body: body(&members) })
    }

    /// `POST /auth/hosted/finish {"HANDLE": …}`: the single-use handle the hosted return carried
    /// (`?pr-code=`). It is not a token: the backend answers the sign-in it stands for, once.
    pub fn hosted_finish(handle: &str) -> Post {
        Post { route: "hosted/finish", body: body(&[("HANDLE", handle)]) }
    }

    /// The origin the backend at `address` (`wss://host[:port]/path`) answers HTTPS on: `https://`
    /// for `wss://`, `http://` for a `ws://` (the local machine only, as the connection itself).
    pub fn base_of(address: &str) -> Option<String> {
        let (scheme, rest) = address.trim().split_once("://")?;
        let scheme = match scheme.to_ascii_lowercase().as_str() {
            "wss" | "https" => "https",
            "ws" | "http" => "http",
            _ => return None,
        };
        let host = rest.split(['/', '?', '#']).next().filter(|h| !h.is_empty())?;
        Some(format!("{scheme}://{host}"))
    }

    /// Where the page goes to sign in on the provider's page: `GET /auth/hosted/start`, which sets
    /// the flow's cookie and sends the browser on. `back` is the page to come back to; `state` the
    /// random value the page keeps until it does.
    pub fn hosted_start_url(base: &str, back: &str, state: &str) -> String {
        format!("{}/auth/hosted/start?return={}&state={}", base.trim_end_matches('/'), percent(back), percent(state))
    }

    /// Does the hosted return's `pr-state` match the value the page kept before it left? Without a
    /// kept value (a link someone else made, a second tab) nothing matches: a handle from another
    /// browser's sign-in is never redeemed here.
    pub fn state_matches(kept: Option<&str>, given: &str) -> bool {
        match kept {
            Some(k) if !k.is_empty() && k.len() == given.len() => k.bytes().zip(given.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0,
            _ => false,
        }
    }

    fn percent(s: &str) -> String {
        let mut out = String::new();
        for b in s.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }

    /// What an answer means. A refusal (any status but 200), or a body that cannot be read, is a
    /// failure whose detail is the status or the outcome word — never the backend's text (R6, R10).
    pub fn decode(status: u16, body: &[u8]) -> AuthResult {
        if status != 200 {
            return AuthResult::Failed { detail: format!("http/{status}") };
        }
        let Ok(Value::Object(r)) = serde_json::from_slice::<Value>(body) else {
            return AuthResult::Failed { detail: "malformed answer".into() };
        };
        let text = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").trim().to_owned();
        match text("OUTCOME").as_str() {
            "OK" => {
                let access_token = text("ACCESS-TOKEN");
                if access_token.is_empty() {
                    return AuthResult::Done;
                }
                let expires_in_s = r.get("EXPIRES-IN").and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).filter(|s| *s > 0.0);
                AuthResult::SignedIn { access_token, expires_in_s }
            }
            "CHALLENGE" => {
                let session = text("SESSION");
                match mfa_kind(&r) {
                    Some(MfaKind::NewPassword) => AuthResult::NeedsNewPassword { session },
                    Some(kind) => AuthResult::NeedsMfa { session, kind },
                    None => AuthResult::Failed { detail: "unknown challenge".into() },
                }
            }
            "CONFIRMATION-REQUIRED" => AuthResult::NeedsConfirm,
            "AUTH-RESET-REQUESTED" => AuthResult::ResetRequested,
            "" => AuthResult::Failed { detail: "malformed answer".into() },
            // AUTH-FAILED, INVALID, RATE-LIMITED, HOSTED (this provider signs in on its own page).
            other if other.len() <= 32 && other.bytes().all(|b| b.is_ascii_uppercase() || b == b'-') => AuthResult::Failed { detail: other.to_ascii_lowercase() },
            _ => AuthResult::Failed { detail: "unknown outcome".into() },
        }
    }

    /// The kind of a `CHALLENGE` answer (D2). An answer without one is an app code. The masked
    /// destination is shown as text, so it is kept short and printable; an enrolment link is only
    /// ever an `otpauth://` address. Anything else is no challenge.
    fn mfa_kind(r: &Map<String, Value>) -> Option<MfaKind> {
        let text = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").trim().to_owned();
        let masked = || {
            let d = text("DESTINATION");
            (d.chars().count() <= 64 && !d.chars().any(char::is_control)).then_some(d)
        };
        let word = text("CHALLENGE");
        Some(match if word.is_empty() { "totp" } else { word.as_str() } {
            "totp" => MfaKind::Totp,
            "sms" => MfaKind::Sms { masked: masked()? },
            "email" => MfaKind::Email { masked: masked()? },
            "new-password" => MfaKind::NewPassword,
            "choice" => {
                let mut kinds: Vec<MfaChoice> = Vec::new();
                for c in text("CHOICES").split(',').map(str::trim).filter(|c| !c.is_empty()) {
                    let c = MfaChoice::from_wire(c)?;
                    if !kinds.contains(&c) {
                        kinds.push(c);
                    }
                }
                if kinds.is_empty() {
                    return None;
                }
                MfaKind::Choice(kinds)
            }
            "totp-setup" => {
                let (secret, otpauth_uri) = (text("SECRET"), text("OTPAUTH-URI"));
                let uri_ok = otpauth_uri.to_ascii_lowercase().starts_with("otpauth://totp/") && !otpauth_uri.chars().any(|c| c.is_control() || c == ' ');
                let secret_ok = !secret.is_empty() && secret.chars().all(|c| c.is_ascii_alphanumeric() || c == '=');
                if !(uri_ok && secret_ok) {
                    return None;
                }
                MfaKind::TotpSetup { secret, otpauth_uri }
            }
            _ => return None,
        })
    }
}

/// Is `url` somewhere the hosted flow may send the browser? The provider's sign-in page is
/// `https://`; plain `http://` is for the local machine only. Anything else (`javascript:`, `data:`,
/// a relative address) is refused: the answer came from the network.
pub fn redirect_allowed(url: &str) -> bool {
    let u = url.trim();
    let lower = u.to_ascii_lowercase();
    if u.chars().any(|c| c.is_control() || c == ' ') {
        return false;
    }
    if lower.starts_with("https://") {
        return lower.len() > "https://".len();
    }
    if let Some(rest) = lower.strip_prefix("http://") {
        let host = rest.split(['/', ':', '?', '#']).next().unwrap_or("");
        return matches!(host, "localhost" | "127.0.0.1" | "[::1]") || host.ends_with(".localhost");
    }
    false
}

// ── the backend interface ───────────────────────────────────────────────────

/// The interface the identity programs of the project's Backend are reached through.
///
/// The shell implements it over the player's connection; the tests implement it with
/// [`MockBackend`]. The callback style fits an answer that arrives later.
pub trait AuthBackend {
    fn call(&self, call: BackendCall, done: Box<dyn FnOnce(AuthResult)>);
}

/// Run `ops` against `backend`, feeding answers back to `machine`; returns the events and messages
/// the shell shows. A synchronous driver for the mock; the shell has an asynchronous twin.
pub fn drive_sync(machine: &mut Machine, backend: &dyn AuthBackend, mut ops: Vec<Op>, now_ms: f64) -> Vec<Op> {
    let mut shown = Vec::new();
    while let Some(op) = (!ops.is_empty()).then(|| ops.remove(0)) {
        match op {
            Op::Backend(call) => {
                let slot: std::rc::Rc<std::cell::RefCell<Option<AuthResult>>> = Default::default();
                let s2 = slot.clone();
                backend.call(call.clone(), Box::new(move |r| *s2.borrow_mut() = Some(r)));
                let r = slot.borrow_mut().take().expect("the mock answers at once");
                ops.extend(machine.on_result(&call, r, now_ms));
            }
            Op::Captcha { action } => {
                shown.push(Op::Captcha { action });
                ops.extend(machine.captcha_token(Some(format!("captcha-token-for-{action}"))));
            }
            other => shown.push(other),
        }
    }
    shown
}

/// A mock identity provider: one user, a password, optional MFA, a confirmation code.
pub struct MockBackend {
    pub users: std::cell::RefCell<BTreeMap<String, MockUser>>,
    pub log: std::cell::RefCell<Vec<String>>,
    pub require_mfa: bool,
    /// The challenge a sign-in gets when `require_mfa` is on.
    pub mfa_kind: MfaKind,
    /// `false`: the provider makes a new account usable at once (no confirmation step).
    pub sign_up_needs_confirm: bool,
    pub captchas_seen: std::cell::RefCell<Vec<Option<String>>>,
}

#[derive(Clone)]
pub struct MockUser {
    pub password: String,
    pub confirmed: bool,
    pub email: String,
}

impl MockBackend {
    pub fn new() -> MockBackend {
        MockBackend {
            users: Default::default(),
            log: Default::default(),
            require_mfa: false,
            mfa_kind: MfaKind::Totp,
            sign_up_needs_confirm: true,
            captchas_seen: Default::default(),
        }
    }

    /// A syntactically valid, *unsigned* JWT with these claims (the machine does not verify; the backend does).
    pub fn token(claims: &serde_json::Value) -> String {
        fn b64(bytes: &[u8]) -> String {
            const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let mut out = String::new();
            for ch in bytes.chunks(3) {
                let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
                for i in 0..=ch.len() {
                    out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
                }
            }
            out
        }
        format!("{}.{}.{}", b64(br#"{"alg":"RS256","typ":"JWT"}"#), b64(claims.to_string().as_bytes()), "c2ln")
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthBackend for MockBackend {
    fn call(&self, call: BackendCall, done: Box<dyn FnOnce(AuthResult)>) {
        self.log.borrow_mut().push(format!("{:?}", redact(&call)));
        let result = match call {
            BackendCall::SignIn { user, password, captcha } => {
                self.captchas_seen.borrow_mut().push(captcha);
                match self.users.borrow().get(&user) {
                    Some(u) if u.password == password && !u.confirmed => AuthResult::NeedsConfirm,
                    Some(u) if u.password == password && self.require_mfa => AuthResult::NeedsMfa { session: "mfa-session".into(), kind: self.mfa_kind.clone() },
                    Some(u) if u.password == password => AuthResult::SignedIn {
                        access_token: MockBackend::token(&serde_json::json!({"sub": "u-1", "email": u.email, "preferred_username": user, "exp": 4102444800u64, "groups": ["a", "b"]})),
                        expires_in_s: Some(3600.0),
                    },
                    _ => AuthResult::Failed { detail: "bad credentials".into() },
                }
            }
            BackendCall::SignUp { user, email, password, captcha } => {
                self.captchas_seen.borrow_mut().push(captcha);
                let confirmed = !self.sign_up_needs_confirm;
                self.users.borrow_mut().insert(user, MockUser { password, confirmed, email });
                if confirmed {
                    AuthResult::Done
                } else {
                    AuthResult::NeedsConfirm
                }
            }
            BackendCall::Confirm { user, code } => match self.users.borrow_mut().get_mut(&user) {
                Some(u) if code == "123456" => {
                    u.confirmed = true;
                    AuthResult::Done
                }
                _ => AuthResult::Failed { detail: "bad code".into() },
            },
            BackendCall::Forgot { captcha, .. } => {
                self.captchas_seen.borrow_mut().push(captcha);
                AuthResult::ResetRequested
            }
            BackendCall::Reset { user, code, new_password } => match self.users.borrow_mut().get_mut(&user) {
                Some(u) if code == "654321" => {
                    u.password = new_password;
                    AuthResult::Done
                }
                _ => AuthResult::Failed { detail: "bad code".into() },
            },
            BackendCall::Mfa { code, .. } if code == "000000" => AuthResult::SignedIn {
                access_token: MockBackend::token(&serde_json::json!({"sub": "u-1", "preferred_username": "mfa-user", "exp": 4102444800u64})),
                expires_in_s: Some(3600.0),
            },
            BackendCall::MfaSetup { code, .. } if code == "000000" => AuthResult::SignedIn {
                access_token: MockBackend::token(&serde_json::json!({"sub": "u-1", "preferred_username": "enrolled", "exp": 4102444800u64})),
                expires_in_s: Some(3600.0),
            },
            BackendCall::Mfa { .. } | BackendCall::MfaSetup { .. } => AuthResult::Failed { detail: "bad mfa".into() },
            BackendCall::MfaChoose { choice, .. } => AuthResult::NeedsMfa {
                session: "mfa-session-2".into(),
                kind: match choice {
                    MfaChoice::Totp => MfaKind::Totp,
                    MfaChoice::Sms => MfaKind::Sms { masked: "+1•••23".into() },
                    MfaChoice::Email => MfaKind::Email { masked: "a•••@example.com".into() },
                },
            },
            BackendCall::NewPassword { new_password, .. } if new_password.len() >= 8 => AuthResult::SignedIn {
                access_token: MockBackend::token(&serde_json::json!({"sub": "u-1", "preferred_username": "renewed", "exp": 4102444800u64})),
                expires_in_s: Some(3600.0),
            },
            BackendCall::NewPassword { .. } => AuthResult::Failed { detail: "weak password".into() },
            BackendCall::Refresh => AuthResult::SignedIn {
                access_token: MockBackend::token(&serde_json::json!({"sub": "u-1", "preferred_username": "refreshed", "exp": 4102444800u64})),
                expires_in_s: Some(3600.0),
            },
            BackendCall::SignOut => AuthResult::Done,
            BackendCall::HostedStart => AuthResult::Hosted { redirect_url: "https://idp.example/authorize?state=abc".into() },
            BackendCall::HostedFinish { .. } => AuthResult::SignedIn {
                access_token: MockBackend::token(&serde_json::json!({"sub": "u-2", "preferred_username": "hosted", "exp": 4102444800u64})),
                expires_in_s: Some(3600.0),
            },
        };
        done(result);
    }
}

/// A call with its secrets blanked, for logs (R6/R10: passwords are never logged).
pub fn redact(c: &BackendCall) -> BackendCall {
    let hide = |_: &str| "<redacted>".to_owned();
    match c {
        BackendCall::SignIn { user, password, captcha } => BackendCall::SignIn { user: user.clone(), password: hide(password), captcha: captcha.as_ref().map(|c| hide(c)) },
        BackendCall::SignUp { user, email, password, captcha } => {
            BackendCall::SignUp { user: user.clone(), email: email.clone(), password: hide(password), captcha: captcha.as_ref().map(|c| hide(c)) }
        }
        BackendCall::Reset { user, code, new_password } => BackendCall::Reset { user: user.clone(), code: hide(code), new_password: hide(new_password) },
        BackendCall::Mfa { session, code } => BackendCall::Mfa { session: hide(session), code: hide(code) },
        BackendCall::MfaSetup { session, code } => BackendCall::MfaSetup { session: hide(session), code: hide(code) },
        BackendCall::NewPassword { session, new_password } => BackendCall::NewPassword { session: hide(session), new_password: hide(new_password) },
        BackendCall::MfaChoose { session, choice } => BackendCall::MfaChoose { session: hide(session), choice: *choice },
        BackendCall::Confirm { user, code } => BackendCall::Confirm { user: user.clone(), code: hide(code) },
        BackendCall::HostedFinish { code, state } => BackendCall::HostedFinish { code: hide(code), state: hide(state) },
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend_with_user() -> MockBackend {
        let b = MockBackend::new();
        b.users.borrow_mut().insert("ana".into(), MockUser { password: "s3cret!".into(), confirmed: true, email: "ana@example.com".into() });
        b
    }

    fn run(m: &mut Machine, b: &MockBackend, a: Action) -> Vec<Op> {
        let ops = m.act(a);
        drive_sync(m, b, ops, 1_000.0)
    }

    #[test]
    fn a_good_sign_in_signs_in_raises_the_event_and_exposes_claims_not_the_token() {
        let b = backend_with_user();
        let mut m = Machine::new(Config::default());
        let shown = run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        assert!(shown.contains(&Op::Notify(AuthEvent::SignedIn)));
        assert_eq!(m.state_name(), "SignedIn");
        assert_eq!(m.screen, Screen::SignedIn);
        assert_eq!(m.user_name, "ana");
        assert_eq!(m.email, "ana@example.com");
        assert_eq!(m.session.claim("sub"), "u-1");
        assert_eq!(m.session.claim("groups"), "a,b", "arrays are joined");
        assert_eq!(m.session.claim("nope"), "");
        let dbg = format!("{:?}", m.session);
        assert!(dbg.contains("<redacted>") && !dbg.contains("eyJ"), "R6: a token never reaches a log: {dbg}");
    }

    #[test]
    fn an_unknown_user_and_a_wrong_password_get_the_same_message() {
        let b = backend_with_user();
        let mut m = Machine::new(Config::default());
        let wrong = run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "nope".into() });
        let unknown = run(&mut m, &b, Action::SignIn { user: "ghost".into(), password: "nope".into() });
        let msg = |ops: &[Op]| ops.iter().find_map(|o| if let Op::Message(t) = o { Some(t.clone()) } else { None });
        assert_eq!(msg(&wrong), msg(&unknown), "R10: the user cannot tell the two cases apart");
        assert_eq!(msg(&wrong).unwrap(), Texts::default().sign_in_failed);
        assert!(!m.session.is_signed_in());
    }

    #[test]
    fn a_reset_request_shows_one_message_whether_or_not_the_address_is_known() {
        let b = backend_with_user();
        let mut m = Machine::new(Config::default());
        let known = run(&mut m, &b, Action::Forgot { user: "ana".into() });
        let unknown = run(&mut m, &b, Action::Forgot { user: "ghost".into() });
        assert_eq!(known, unknown);
        assert!(known.contains(&Op::Notify(AuthEvent::PasswordResetRequested)));
        assert_eq!(m.screen, Screen::Reset);
        // ... then the code and the new password.
        run(&mut m, &b, Action::Reset { user: "ana".into(), code: "654321".into(), new_password: "n3w-pass".into() });
        assert_eq!(m.screen, Screen::SignIn);
        let shown = run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "n3w-pass".into() });
        assert!(shown.contains(&Op::Notify(AuthEvent::SignedIn)));
    }

    #[test]
    fn sign_up_confirm_sign_in() {
        let b = MockBackend::new();
        let mut m = Machine::new(Config::default());
        let shown = run(&mut m, &b, Action::SignUp { user: "bia".into(), email: "bia@example.com".into(), password: "p4ss!word".into() });
        // C8 (WASM-009 T7): this test pinned the event here AND after Confirm; the account is not
        // usable yet, so it is raised once, after the confirmation below.
        assert!(!shown.contains(&Op::Notify(AuthEvent::SignUpCompleted)), "{shown:?}");
        assert_eq!(m.screen, Screen::Confirm);
        let bad = run(&mut m, &b, Action::Confirm { user: "bia".into(), code: "000".into() });
        assert!(bad.iter().any(|o| matches!(o, Op::Message(_))));
        let done = run(&mut m, &b, Action::Confirm { user: "bia".into(), code: "123456".into() });
        assert_eq!(done.iter().filter(|o| **o == Op::Notify(AuthEvent::SignUpCompleted)).count(), 1);
        assert_eq!(m.screen, Screen::SignIn);
        let shown = run(&mut m, &b, Action::SignIn { user: "bia".into(), password: "p4ss!word".into() });
        assert!(shown.contains(&Op::Notify(AuthEvent::SignedIn)));
    }

    #[test]
    fn an_mfa_challenge_interposes_and_raises_its_event() {
        let mut b = backend_with_user();
        b.require_mfa = true;
        let mut m = Machine::new(Config::default());
        let shown = run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        assert!(shown.contains(&Op::Notify(AuthEvent::MfaRequired)));
        assert_eq!(m.screen, Screen::Mfa);
        assert!(!m.session.is_signed_in());
        let bad = run(&mut m, &b, Action::Mfa { code: "111111".into() });
        assert!(!m.session.is_signed_in() && bad.iter().any(|o| matches!(o, Op::Message(_))));
        let ok = run(&mut m, &b, Action::Mfa { code: "000000".into() });
        assert!(ok.contains(&Op::Notify(AuthEvent::SignedIn)) && m.session.is_signed_in());
    }

    #[test]
    fn recaptcha_obtains_a_token_per_protected_action_and_sends_it() {
        let b = backend_with_user();
        let mut m = Machine::new(Config { recaptcha_site_key: "site-key".into(), ..Config::default() });
        let shown = run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        assert!(shown.contains(&Op::Captcha { action: "sign_in" }));
        run(&mut m, &b, Action::Forgot { user: "ana".into() });
        let seen = b.captchas_seen.borrow().clone();
        assert_eq!(seen, vec![Some("captcha-token-for-sign_in".to_owned()), Some("captcha-token-for-forgot_password".to_owned())]);
        // With no token (the script failed), no request is made.
        let mut m2 = Machine::new(Config { recaptcha_site_key: "site-key".into(), ..Config::default() });
        let ops = m2.act(Action::SignIn { user: "ana".into(), password: "x".into() });
        assert_eq!(ops, vec![Op::Captcha { action: "sign_in" }]);
        let after = m2.captcha_token(None);
        assert!(after.iter().all(|o| !matches!(o, Op::Backend(_))), "no token, no request");
    }

    #[test]
    fn the_hosted_flow_offers_one_action_and_hides_the_in_app_screens() {
        let b = backend_with_user();
        let mut m = Machine::new(Config { flow: Flow::Hosted, ..Config::default() });
        m.act(Action::Show(Screen::SignUp));
        assert_eq!(m.screen, Screen::SignIn, "sign-up is the provider's page in the hosted flow");
        let shown = run(&mut m, &b, Action::Hosted);
        assert!(matches!(shown.as_slice(), [Op::Redirect(u)] if u.starts_with("https://idp.example/")));
        // Back from the provider with an authorisation code (not a token).
        let ops = vec![Op::Backend(BackendCall::HostedFinish { code: "c".into(), state: "abc".into() })];
        m.busy = true;
        let shown = drive_sync(&mut m, &b, ops, 0.0);
        assert!(shown.contains(&Op::Notify(AuthEvent::SignedIn)));
        assert_eq!(m.user_name, "hosted");
    }

    #[test]
    fn a_failed_silent_refresh_signs_the_user_out_and_a_good_one_is_quiet() {
        let b = backend_with_user();
        let mut m = Machine::new(Config::default());
        run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        // Not near expiry: nothing happens.
        assert!(m.act(Action::Tick { now_ms: 2_000.0 }).is_empty());
        // Near expiry: one refresh, no second SignedIn event.
        m.session.expires_at_ms = Some(10_000.0);
        let ops = m.act(Action::Tick { now_ms: 5_000.0 });
        assert_eq!(ops, vec![Op::Backend(BackendCall::Refresh)]);
        let shown = drive_sync(&mut m, &b, ops, 5_000.0);
        assert!(shown.is_empty(), "a silent refresh raises nothing");
        assert!(m.session.is_signed_in());
        // A refused refresh ends the session.
        let r = m.on_result(&BackendCall::Refresh, AuthResult::Failed { detail: "expired".into() }, 6_000.0);
        assert!(r.contains(&Op::Notify(AuthEvent::SignedOut)));
        assert!(!m.session.is_signed_in());
    }

    #[test]
    fn sign_out_forgets_the_token_at_once_and_raises_the_event() {
        let b = backend_with_user();
        let mut m = Machine::new(Config::default());
        run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        let shown = run(&mut m, &b, Action::SignOut);
        assert!(shown.contains(&Op::Notify(AuthEvent::SignedOut)));
        assert!(!m.session.is_signed_in());
        assert_eq!(m.session.claim("sub"), "");
        assert_eq!(m.screen, Screen::SignIn);
    }

    #[test]
    fn passwords_and_codes_never_reach_the_backend_log() {
        let b = backend_with_user();
        let mut m = Machine::new(Config::default());
        run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        run(&mut m, &b, Action::SignOut);
        let log = b.log.borrow().join("\n");
        assert!(!log.contains("s3cret!"), "R10: passwords are never logged: {log}");
        assert!(log.contains("<redacted>"));
    }

    #[test]
    fn claims_decode_from_a_real_shaped_jwt() {
        let t = MockBackend::token(&serde_json::json!({"sub": "abc", "n": 5, "b": true, "email": "é@x.y"}));
        let c = decode_claims(&t);
        assert_eq!((c["sub"].as_str(), c["n"].as_str(), c["b"].as_str(), c["email"].as_str()), ("abc", "5", "true", "é@x.y"));
        assert!(decode_claims("not-a-jwt").is_empty());
        assert!(decode_claims("a.%%%.c").is_empty());
    }

    #[test]
    fn the_program_can_ask_for_a_refresh_and_only_when_signed_in() {
        let b = backend_with_user();
        let mut m = Machine::new(Config::default());
        assert!(m.act(Action::Refresh).is_empty(), "nothing to refresh before a sign-in");
        run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        let ops = m.act(Action::Refresh);
        assert_eq!(ops, vec![Op::Backend(BackendCall::Refresh)]);
    }

    #[test]
    fn a_second_control_follows_the_session_of_the_first() {
        let b = backend_with_user();
        let mut a = Machine::new(Config::default());
        let mut other = Machine::new(Config::default());
        run(&mut a, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        other.adopt(&a.session);
        assert_eq!((other.screen, other.user_name.as_str(), other.email.as_str()), (Screen::SignedIn, "ana", "ana@example.com"));
        run(&mut a, &b, Action::SignOut);
        other.adopt(&a.session);
        assert_eq!((other.screen, other.user_name.as_str()), (Screen::SignIn, ""));
    }

    #[test]
    fn a_temporary_password_sends_the_user_to_the_new_password_screen() {
        let mut m = Machine::new(Config::default());
        let call = BackendCall::SignIn { user: "ana".into(), password: "tmp".into(), captcha: None };
        m.busy = true;
        let ops = m.on_result(&call, AuthResult::NeedsNewPassword { session: "np".into() }, 0.0);
        assert!(ops.is_empty(), "a new password is not a second factor: no onMfaRequired");
        // D16 (WASM-009 T7): this test pinned `Screen::Reset`, which asks for a code this case does
        // not have; the temporary password now has its own screen.
        assert_eq!(m.screen, Screen::NewPassword);
        assert_eq!(m.mfa_kind, Some(MfaKind::NewPassword));
        // The two fields must agree before anything is sent.
        let ops = m.act(Action::NewPassword { password: "Longer-pass1".into(), repeat: "Longer-pass2".into() });
        assert_eq!(ops, vec![Op::Message(Texts::default().passwords_differ)]);
        let ops = m.act(Action::NewPassword { password: "Longer-pass1".into(), repeat: "Longer-pass1".into() });
        assert_eq!(ops, vec![Op::Backend(BackendCall::NewPassword { session: "np".into(), new_password: "Longer-pass1".into() })]);
        let b = MockBackend::new();
        let shown = drive_sync(&mut m, &b, ops, 0.0);
        assert!(shown.contains(&Op::Notify(AuthEvent::SignedIn)) && m.session.is_signed_in());
        assert_eq!(m.mfa_kind, None, "the challenge is over");
    }

    /// T21: each call is one POST to its route with only the members a page sends; every outcome
    /// of `AUTH-RESULT` decodes; a refusal carries the status, never the backend's text.
    #[test]
    fn every_call_is_one_post_to_its_route_and_every_outcome_decodes() {
        let calls = [
            (BackendCall::SignIn { user: "u".into(), password: "p".into(), captcha: Some("c".into()) }, "sign-in", r#"{"CAPTCHA-TOKEN":"c","PASSWORD":"p","USERNAME":"u"}"#),
            (BackendCall::SignUp { user: "u".into(), email: "e".into(), password: "p".into(), captcha: None }, "sign-up", r#"{"EMAIL":"e","PASSWORD":"p","USERNAME":"u"}"#),
            (BackendCall::Confirm { user: "u".into(), code: "1".into() }, "confirm", r#"{"ONE-TIME-CODE":"1","USERNAME":"u"}"#),
            (BackendCall::Forgot { user: "u".into(), captcha: None }, "forgot", r#"{"USERNAME":"u"}"#),
            (BackendCall::Reset { user: "u".into(), code: "1".into(), new_password: "n".into() }, "reset", r#"{"ONE-TIME-CODE":"1","PASSWORD":"n","USERNAME":"u"}"#),
            (BackendCall::Mfa { session: "s".into(), code: "1".into() }, "mfa", r#"{"ONE-TIME-CODE":"1","SESSION":"s"}"#),
            (BackendCall::NewPassword { session: "s".into(), new_password: "n".into() }, "mfa", r#"{"PASSWORD":"n","SESSION":"s"}"#),
            (BackendCall::MfaChoose { session: "s".into(), choice: MfaChoice::Email }, "mfa", r#"{"CHOICE":"email","SESSION":"s"}"#),
            (BackendCall::MfaSetup { session: "s".into(), code: "1".into() }, "mfa-setup", r#"{"ONE-TIME-CODE":"1","SESSION":"s"}"#),
            (BackendCall::Refresh, "refresh", "{}"),
            (BackendCall::SignOut, "sign-out", "{}"),
            (BackendCall::HostedFinish { code: "h".into(), state: "s".into() }, "hosted/finish", r#"{"HANDLE":"h"}"#),
        ];
        for (call, route, body) in &calls {
            let p = https::post(call).expect("a request");
            assert_eq!((p.route, p.body.as_str()), (*route, *body), "{call:?}");
            assert_eq!(p.url("https://api.example.test:8443/"), format!("https://api.example.test:8443/auth/{route}"));
        }
        assert_eq!(https::post(&BackendCall::HostedStart), None, "the hosted start is a navigation");

        let ok = |s: &str| https::decode(200, s.as_bytes());
        assert_eq!(ok(r#"{"OUTCOME":"OK","ACCESS-TOKEN":"t","EXPIRES-IN":60,"REFRESH-TOKEN":""}"#), AuthResult::SignedIn { access_token: "t".into(), expires_in_s: Some(60.0) });
        assert_eq!(ok(r#"{"OUTCOME":"OK","ACCESS-TOKEN":"t","EXPIRES-IN":0}"#), AuthResult::SignedIn { access_token: "t".into(), expires_in_s: None });
        assert_eq!(ok(r#"{"OUTCOME":"CONFIRMATION-REQUIRED"}"#), AuthResult::NeedsConfirm);
        assert_eq!(ok(r#"{"OUTCOME":"CHALLENGE","SESSION":"s","CHALLENGE":""}"#), AuthResult::NeedsMfa { session: "s".into(), kind: MfaKind::Totp });
        assert_eq!(ok(r#"{"OUTCOME":"CHALLENGE","SESSION":"s","CHALLENGE":"new-password"}"#), AuthResult::NeedsNewPassword { session: "s".into() });
        assert_eq!(ok(r#"{"OUTCOME":"AUTH-RESET-REQUESTED"}"#), AuthResult::ResetRequested);
        assert_eq!(ok(r#"{"OUTCOME":"OK","ACCESS-TOKEN":""}"#), AuthResult::Done);
        assert_eq!(ok(r#"{"OUTCOME":"AUTH-FAILED","OUTCOME-TEXT":"Nope, ana@example.com is unknown"}"#), AuthResult::Failed { detail: "auth-failed".into() }, "the outcome word, never its text (R6, R10)");
        assert_eq!(ok(r#"{"OUTCOME":"RATE-LIMITED"}"#), AuthResult::Failed { detail: "rate-limited".into() });
        for bad in ["not json", "{}", r#"{"OUTCOME":"?<script>"}"#, "[1]"] {
            assert!(matches!(ok(bad), AuthResult::Failed { .. }), "{bad}");
        }
        assert_eq!(https::decode(403, b"origin not allowed"), AuthResult::Failed { detail: "http/403".into() });
        assert_eq!(https::decode(500, br#"{"error":"the provider is down at 10.0.0.7","code":1}"#), AuthResult::Failed { detail: "http/500".into() });
    }

    /// T21: the hosted flow leaves by a navigation to the backend and comes back with a handle the
    /// page redeems only with the state it kept.
    #[test]
    fn the_hosted_flow_starts_on_the_backend_and_its_return_needs_the_kept_state() {
        assert_eq!(https::base_of("wss://api.example.test:8443/ws").as_deref(), Some("https://api.example.test:8443"));
        assert_eq!(https::base_of("ws://127.0.0.1:8080/ws").as_deref(), Some("http://127.0.0.1:8080"));
        assert_eq!(https::base_of("ftp://x/"), None);
        assert_eq!(https::base_of("wss:///ws"), None);
        let url = https::hosted_start_url("https://api.example.test", "https://app.example.test/shop?x=1#top", "st-1");
        assert_eq!(url, "https://api.example.test/auth/hosted/start?return=https%3A%2F%2Fapp.example.test%2Fshop%3Fx%3D1%23top&state=st-1");
        assert!(redirect_allowed(&url));
        assert!(https::state_matches(Some("Zq3-abc"), "Zq3-abc"));
        for (kept, given) in [(None, "Zq3-abc"), (Some(""), ""), (Some("Zq3-abc"), "Zq3-abd"), (Some("Zq3-abc"), "Zq3-ab")] {
            assert!(!https::state_matches(kept, given), "{kept:?} / {given}");
        }
    }

    #[test]
    fn auth_ids_are_apart_from_the_modules() {
        assert!(!https::is_auth_id(1) && !https::is_auth_id(0x7fff_ffff));
        assert!(https::is_auth_id(https::ID_BASE) && https::is_auth_id(https::ID_BASE + 5));
    }

    #[test]
    fn no_debug_output_of_a_call_an_answer_or_the_machine_holds_a_secret() {
        let secrets = ["hunter2", "c0de-1234", "tok-abcdef", "mfa-session-xyz", "captcha-tok"];
        let calls = [
            BackendCall::SignIn { user: "ana".into(), password: "hunter2".into(), captcha: Some("captcha-tok".into()) },
            BackendCall::SignUp { user: "ana".into(), email: "a@x.y".into(), password: "hunter2".into(), captcha: None },
            BackendCall::Confirm { user: "ana".into(), code: "c0de-1234".into() },
            BackendCall::Reset { user: "ana".into(), code: "c0de-1234".into(), new_password: "hunter2".into() },
            BackendCall::Mfa { session: "mfa-session-xyz".into(), code: "c0de-1234".into() },
            BackendCall::HostedFinish { code: "c0de-1234".into(), state: "tok-abcdef".into() },
        ];
        for c in &calls {
            let shown = format!("{c:?}");
            assert!(secrets.iter().all(|s| !shown.contains(s)), "{shown}");
        }
        let actions = [
            Action::SignIn { user: "ana".into(), password: "hunter2".into() },
            Action::Reset { user: "ana".into(), code: "c0de-1234".into(), new_password: "hunter2".into() },
            Action::Mfa { code: "c0de-1234".into() },
            Action::HostedFinish { code: "c0de-1234".into(), state: "tok-abcdef".into() },
        ];
        for a in &actions {
            let shown = format!("{a:?}");
            assert!(secrets.iter().all(|s| !shown.contains(s)), "{shown}");
        }
        let shown = format!("{:?}", AuthResult::SignedIn { access_token: "tok-abcdef".into(), expires_in_s: Some(1.0) });
        assert!(!shown.contains("tok-abcdef"), "{shown}");
        let shown = format!("{:?}", AuthResult::NeedsMfa { session: "mfa-session-xyz".into(), kind: MfaKind::Totp });
        assert!(!shown.contains("mfa-session-xyz"), "{shown}");
        let setup = MfaKind::TotpSetup { secret: "JBSWY3DPEHPK3PXP".into(), otpauth_uri: "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP".into() };
        let shown = format!("{:?}", AuthResult::NeedsMfa { session: "s".into(), kind: setup });
        assert!(!shown.contains("JBSWY3DPEHPK3PXP"), "the enrolment key never reaches a log: {shown}");
        let shown = format!("{:?}", BackendCall::NewPassword { session: "mfa-session-xyz".into(), new_password: "hunter2".into() });
        assert!(secrets.iter().all(|s| !shown.contains(s)), "{shown}");
        let shown = format!("{:?}", Action::NewPassword { password: "hunter2".into(), repeat: "hunter2".into() });
        assert!(!shown.contains("hunter2"), "{shown}");
        // The machine, while a password waits for its reCAPTCHA token and while an MFA session is held.
        let mut m = Machine::new(Config { recaptcha_site_key: "k".into(), ..Config::default() });
        m.act(Action::SignIn { user: "ana".into(), password: "hunter2".into() });
        m.busy = false;
        m.on_result(&BackendCall::SignIn { user: "ana".into(), password: String::new(), captcha: None }, AuthResult::NeedsMfa { session: "mfa-session-xyz".into(), kind: MfaKind::Totp }, 0.0);
        let shown = format!("{m:?}");
        assert!(secrets.iter().all(|s| !shown.contains(s)), "{shown}");
    }

    #[test]
    fn the_hosted_redirect_is_https_or_the_local_machine() {
        for good in ["https://idp.example/authorize?x=1", "http://localhost:8080/auth", "http://127.0.0.1/auth", "HTTPS://IDP.EXAMPLE/"] {
            assert!(redirect_allowed(good), "{good}");
        }
        for bad in ["http://idp.example/", "javascript:alert(1)", "data:text/html,x", "//idp.example/", "/relative", "https://", "https://a b", "https://a\n.b", "ftp://x", ""] {
            assert!(!redirect_allowed(bad), "{bad:?}");
        }
    }

    #[test]
    fn labels_and_configuration_come_from_properties_with_defaults() {
        let none = |_: &str| String::new();
        let l = Labels::read(&none, "en");
        assert_eq!((l.sign_in.as_str(), l.user.as_str(), l.password.as_str()), ("Sign in", "User name", "Password"));
        let pt = |p: &str| if p == "SignInCaption" { "Entrar".to_owned() } else { String::new() };
        assert_eq!(Labels::read(&pt, "en").sign_in, "Entrar");
        assert_eq!(Labels::read(&pt, "en").sign_up, "Create account", "the others keep their defaults");

        let c = config_from(&none, Flow::InApp, "", "en");
        assert!(c.sign_up && c.confirm && c.forgot && c.mfa && c.flow == Flow::InApp && c.recaptcha_site_key.is_empty() && !c.consent_ask);
        let props = |p: &str| match p {
            "Mode" => "Hosted".to_owned(),
            "ShowSignUp" => "0".to_owned(),
            "RecaptchaSiteKey" => " site ".to_owned(),
            "ErrorText" => "Falhou.".to_owned(),
            _ => String::new(),
        };
        let c = config_from(&props, Flow::InApp, "page-key", "en");
        assert_eq!((c.flow, c.sign_up, c.forgot, c.recaptcha_site_key.as_str(), c.texts.generic_error.as_str()), (Flow::Hosted, false, true, "site", "Falhou."));
        let c = config_from(&none, Flow::Hosted, "page-key", "en");
        assert_eq!((c.flow, c.recaptcha_site_key.as_str()), (Flow::Hosted, "page-key"), "the page's settings fill what the control leaves open");
        let ask = |p: &str| if p == "RecaptchaConsent" { "Ask".to_owned() } else { String::new() };
        assert!(config_from(&ask, Flow::InApp, "", "en").consent_ask);
    }

    // ── T7: C8, D2, D16, D12 ────────────────────────────────────────────────

    fn count(ops: &[Op], ev: AuthEvent) -> usize {
        ops.iter().filter(|o| **o == Op::Notify(ev.clone())).count()
    }

    /// C8 — one `onSignUpCompleted` per sign-up, with a confirmation step and without one.
    #[test]
    fn sign_up_completed_is_raised_once_with_and_without_confirmation() {
        let b = MockBackend::new();
        let mut m = Machine::new(Config::default());
        let mut all = run(&mut m, &b, Action::SignUp { user: "bia".into(), email: "b@x.y".into(), password: "p4ss!word".into() });
        all.extend(run(&mut m, &b, Action::Confirm { user: "bia".into(), code: "123456".into() }));
        all.extend(run(&mut m, &b, Action::SignIn { user: "bia".into(), password: "p4ss!word".into() }));
        assert_eq!(count(&all, AuthEvent::SignUpCompleted), 1, "with confirmation: {all:?}");

        let mut b = MockBackend::new();
        b.sign_up_needs_confirm = false;
        let mut m = Machine::new(Config::default());
        let mut all = run(&mut m, &b, Action::SignUp { user: "caio".into(), email: "c@x.y".into(), password: "p4ss!word".into() });
        assert_eq!(m.screen, Screen::SignIn, "no confirmation: straight back to Sign in");
        all.extend(run(&mut m, &b, Action::SignIn { user: "caio".into(), password: "p4ss!word".into() }));
        assert_eq!(count(&all, AuthEvent::SignUpCompleted), 1, "without confirmation: {all:?}");

        // A provider that signs the new account straight in: completed, then signed in.
        let mut m = Machine::new(Config::default());
        m.busy = true;
        let call = BackendCall::SignUp { user: "d".into(), email: "d@x.y".into(), password: "p".into(), captcha: None };
        let ops = m.on_result(&call, AuthResult::SignedIn { access_token: MockBackend::token(&serde_json::json!({"sub": "d"})), expires_in_s: None }, 0.0);
        assert_eq!(ops, vec![Op::Notify(AuthEvent::SignUpCompleted), Op::Notify(AuthEvent::SignedIn)]);
    }

    /// C8 — a sign-in to an account not yet confirmed goes to Confirm and raises nothing.
    #[test]
    fn a_sign_in_answered_needs_confirm_raises_no_event() {
        let b = MockBackend::new();
        b.users.borrow_mut().insert("eva".into(), MockUser { password: "p4ss!word".into(), confirmed: false, email: String::new() });
        let mut m = Machine::new(Config::default());
        let shown = run(&mut m, &b, Action::SignIn { user: "eva".into(), password: "p4ss!word".into() });
        assert!(shown.iter().all(|o| !matches!(o, Op::Notify(_))), "{shown:?}");
        assert_eq!(m.screen, Screen::Confirm);
    }

    /// D2/D16 — each kind leads to its screen; the MFA text names the masked destination.
    #[test]
    fn each_mfa_kind_leads_to_its_screen_and_names_the_destination() {
        let l = Labels::read(&|_| String::new(), "en");
        let setup = MfaKind::TotpSetup { secret: "JBSWY3DPEHPK3PXP".into(), otpauth_uri: "otpauth://totp/App:ana?secret=JBSWY3DPEHPK3PXP".into() };
        let cases = [
            (MfaKind::Totp, Screen::Mfa),
            (MfaKind::Sms { masked: "+1•••23".into() }, Screen::Mfa),
            (MfaKind::Email { masked: "a•••@example.com".into() }, Screen::Mfa),
            (MfaKind::Choice(vec![MfaChoice::Sms, MfaChoice::Email]), Screen::MfaChoose),
            (setup, Screen::MfaSetup),
            (MfaKind::NewPassword, Screen::NewPassword),
        ];
        for (kind, screen) in cases {
            let mut b = backend_with_user();
            b.require_mfa = true;
            b.mfa_kind = kind.clone();
            let mut m = Machine::new(Config::default());
            let shown = run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
            assert_eq!(m.screen, screen, "{kind:?}");
            assert_eq!(m.mfa_kind.as_ref(), Some(&kind));
            let mfa_events = count(&shown, AuthEvent::MfaRequired);
            assert_eq!(mfa_events, usize::from(kind != MfaKind::NewPassword), "{kind:?}: {shown:?}");
            println!("{kind:?} -> {} ({})", screen.name(), l.mfa_text(&kind));
        }
        assert_eq!(l.mfa_text(&MfaKind::Sms { masked: "+1•••23".into() }), "Enter the code sent by text message to +1•••23.");
        assert_eq!(l.mfa_text(&MfaKind::Email { masked: "a•••@example.com".into() }), "Enter the code sent by e-mail to a•••@example.com.");

        // Choose → its code, one onMfaRequired in all; an unoffered kind is ignored.
        let mut b = backend_with_user();
        b.require_mfa = true;
        b.mfa_kind = MfaKind::Choice(vec![MfaChoice::Sms, MfaChoice::Email]);
        let mut m = Machine::new(Config::default());
        let mut all = run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        assert!(m.act(Action::MfaChoose(MfaChoice::Totp)).is_empty(), "not offered");
        all.extend(run(&mut m, &b, Action::MfaChoose(MfaChoice::Email)));
        assert_eq!((m.screen, m.mfa_kind.clone()), (Screen::Mfa, Some(MfaKind::Email { masked: "a•••@example.com".into() })));
        all.extend(run(&mut m, &b, Action::Mfa { code: "000000".into() }));
        assert!(m.session.is_signed_in());
        assert_eq!(count(&all, AuthEvent::MfaRequired), 1, "{all:?}");

        // Set-up: the first code goes to the set-up call.
        let mut b = backend_with_user();
        b.require_mfa = true;
        b.mfa_kind = MfaKind::TotpSetup { secret: "JBSWY3DPEHPK3PXP".into(), otpauth_uri: "otpauth://totp/A:a?secret=JBSWY3DPEHPK3PXP".into() };
        let mut m = Machine::new(Config::default());
        run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        let ops = m.act(Action::Mfa { code: "000000".into() });
        assert!(matches!(ops.as_slice(), [Op::Backend(BackendCall::MfaSetup { .. })]), "{ops:?}");
        drive_sync(&mut m, &b, ops, 0.0);
        assert_eq!((m.user_name.as_str(), m.screen), ("enrolled", Screen::SignedIn), "signed in by the enrolment's first code");

        // A challenge screen is never opened by `Show`, and leaving it abandons the challenge.
        let mut m = Machine::new(Config::default());
        m.act(Action::Show(Screen::Mfa));
        assert_eq!(m.screen, Screen::SignIn);
        m.busy = true;
        m.on_result(&BackendCall::SignIn { user: "a".into(), password: "p".into(), captcha: None }, AuthResult::NeedsMfa { session: "s".into(), kind: MfaKind::Totp }, 0.0);
        m.act(Action::Show(Screen::SignIn));
        assert!(m.act(Action::Mfa { code: "1".into() }).is_empty(), "no session once abandoned");
    }

    /// `ShowMfa` off: the control cannot answer a second factor — a generic failure, never a hang.
    #[test]
    fn with_show_mfa_off_a_second_factor_fails_generically() {
        let mut b = backend_with_user();
        b.require_mfa = true;
        let mut m = Machine::new(Config { mfa: false, ..Config::default() });
        let shown = run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        assert_eq!(m.screen, Screen::SignIn);
        assert!(shown.contains(&Op::Message(Texts::default().generic_error)));
        assert!(shown.contains(&Op::Notify(AuthEvent::Error("mfa/not-offered".into()))));
        // A new password is not a second factor: still answered.
        b.mfa_kind = MfaKind::NewPassword;
        run(&mut m, &b, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        assert_eq!(m.screen, Screen::NewPassword);
    }

    /// D12 — the defaults follow the locale; a developer's value never does.
    #[test]
    fn a_locale_switch_changes_the_defaults_but_never_a_developer_label() {
        let props = |p: &str| match p {
            "PasswordLabel" => "Palavra-passe".to_owned(),
            "ErrorText" => "Erro nosso.".to_owned(),
            _ => String::new(),
        };
        let en = Labels::read(&props, "en");
        let pt = Labels::read(&props, "pt-BR");
        let ja = Labels::read(&props, "ja-JP");
        assert_eq!((en.sign_in.as_str(), pt.sign_in.as_str(), ja.sign_in.as_str()), ("Sign in", "Entrar", "サインイン"));
        assert_eq!((en.password.as_str(), pt.password.as_str(), ja.password.as_str()), ("Palavra-passe", "Palavra-passe", "Palavra-passe"));
        assert_eq!(pt.mfa_text(&MfaKind::Sms { masked: "+55•••12".into() }), "Digite o código enviado por SMS para +55•••12.");
        let (te, tp) = (Texts::read(&props, "en"), Texts::read(&props, "pt-BR"));
        assert_eq!((te.sign_in_failed.as_str(), tp.sign_in_failed.as_str()), ("The user name or the password is not right.", "O nome de usuário ou a senha não estão corretos."));
        assert_eq!((te.generic_error.as_str(), tp.generic_error.as_str()), ("Erro nosso.", "Erro nosso."));
    }

    /// D2 — the challenge kinds on the wire; what the network sends is checked before it is shown.
    #[test]
    fn challenge_kinds_decode_and_bad_ones_are_refused() {
        let kind = |s: &str| match https::decode(200, s.as_bytes()) {
            AuthResult::NeedsMfa { kind, .. } => Some(kind),
            _ => None,
        };
        assert_eq!(kind(r#"{"OUTCOME":"CHALLENGE","SESSION":"s","CHALLENGE":"sms","DESTINATION":"+1•••23"}"#), Some(MfaKind::Sms { masked: "+1•••23".into() }));
        assert_eq!(kind(r#"{"OUTCOME":"CHALLENGE","SESSION":"s","CHALLENGE":"email","DESTINATION":"a•••@x.y"}"#), Some(MfaKind::Email { masked: "a•••@x.y".into() }));
        assert_eq!(kind(r#"{"OUTCOME":"CHALLENGE","SESSION":"s","CHALLENGE":"choice","CHOICES":"sms,totp,sms"}"#), Some(MfaKind::Choice(vec![MfaChoice::Sms, MfaChoice::Totp])));
        assert_eq!(https::decode(200, br#"{"OUTCOME":"CHALLENGE","SESSION":"s","CHALLENGE":"new-password"}"#), AuthResult::NeedsNewPassword { session: "s".into() });
        assert_eq!(
            kind(r#"{"OUTCOME":"CHALLENGE","SESSION":"s","CHALLENGE":"totp-setup","SECRET":"JBSWY3DPEHPK3PXP","OTPAUTH-URI":"otpauth://totp/A:ana?secret=JBSWY3DPEHPK3PXP&issuer=A"}"#),
            Some(MfaKind::TotpSetup { secret: "JBSWY3DPEHPK3PXP".into(), otpauth_uri: "otpauth://totp/A:ana?secret=JBSWY3DPEHPK3PXP&issuer=A".into() })
        );
        for bad in [
            r#"{"OUTCOME":"CHALLENGE","CHALLENGE":"carrier-pigeon"}"#,
            r#"{"OUTCOME":"CHALLENGE","CHALLENGE":"choice","CHOICES":""}"#,
            r#"{"OUTCOME":"CHALLENGE","CHALLENGE":"choice","CHOICES":"fax"}"#,
            r#"{"OUTCOME":"CHALLENGE","CHALLENGE":"totp-setup","SECRET":"K","OTPAUTH-URI":"javascript:alert(1)"}"#,
            r#"{"OUTCOME":"CHALLENGE","CHALLENGE":"totp-setup","SECRET":"<b>","OTPAUTH-URI":"otpauth://totp/a"}"#,
            r#"{"OUTCOME":"CHALLENGE","CHALLENGE":"sms","DESTINATION":"a\nb"}"#,
        ] {
            assert!(matches!(https::decode(200, bad.as_bytes()), AuthResult::Failed { .. }), "{bad}");
        }
    }

    // ── T8: the consent hook (D10) ──────────────────────────────────────────

    fn ask() -> Config {
        Config { recaptcha_site_key: "site".into(), consent_ask: true, ..Config::default() }
    }

    #[test]
    fn with_ask_nothing_of_recaptcha_happens_before_consent_and_grant_runs_the_action_once() {
        let b = backend_with_user();
        let mut m = Machine::new(ask());
        let ops = m.act(Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
        assert_eq!(ops, vec![Op::Notify(AuthEvent::ConsentRequired)], "no Op::Captcha before consent");
        assert!(m.consent_pending());
        assert!(m.act(Action::SignIn { user: "ana".into(), password: "s3cret!".into() }).is_empty(), "a second submit waits too");
        let ops = m.act(Action::GrantConsent);
        assert_eq!(ops, vec![Op::Captcha { action: "sign_in" }]);
        let shown = drive_sync(&mut m, &b, ops, 0.0);
        assert!(shown.contains(&Op::Notify(AuthEvent::SignedIn)));
        assert_eq!(b.captchas_seen.borrow().len(), 1, "the held action ran once");
        assert!(m.act(Action::GrantConsent).is_empty(), "nothing is held any more");
        // Consent holds: the next protected action goes straight to reCAPTCHA.
        run(&mut m, &b, Action::SignOut);
        assert_eq!(m.act(Action::Forgot { user: "ana".into() }), vec![Op::Captcha { action: "forgot_password" }]);
    }

    #[test]
    fn deny_fails_the_held_action_with_the_generic_message_and_consent_declined() {
        let mut m = Machine::new(ask());
        m.act(Action::SignUp { user: "u".into(), email: "e@x.y".into(), password: "p".into() });
        let ops = m.act(Action::DenyConsent);
        assert_eq!(ops, vec![Op::Message(Texts::default().generic_error), Op::Notify(AuthEvent::Error("consent/declined".into()))]);
        assert!(!m.busy && !m.consent_pending());
        // Asked again at the next protected action: a refusal is not remembered as consent.
        assert_eq!(m.act(Action::SignIn { user: "u".into(), password: "p".into() }), vec![Op::Notify(AuthEvent::ConsentRequired)]);
    }

    #[test]
    fn implied_and_no_recaptcha_behave_as_before() {
        let mut m = Machine::new(Config { recaptcha_site_key: "site".into(), ..Config::default() });
        assert_eq!(m.act(Action::SignIn { user: "a".into(), password: "p".into() }), vec![Op::Captcha { action: "sign_in" }]);
        // `Ask` without a site key: nothing to consent to.
        let mut m = Machine::new(Config { consent_ask: true, ..Config::default() });
        assert!(matches!(m.act(Action::SignIn { user: "a".into(), password: "p".into() }).as_slice(), [Op::Backend(BackendCall::SignIn { .. })]));
    }
}
