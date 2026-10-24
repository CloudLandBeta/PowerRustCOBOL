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
            Screen::SignedIn => "SignedIn",
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
            texts: Texts::default(),
        }
    }
}

/// The control's texts, translatable by the developer (WASM-009 R1). These are the defaults.
#[derive(Debug, Clone)]
pub struct Texts {
    pub sign_in_failed: String,
    pub reset_requested: String,
    pub generic_error: String,
}

impl Default for Texts {
    fn default() -> Self {
        Texts {
            sign_in_failed: "The user name or the password is not right.".into(),
            reset_requested: "If the address is registered, a code has been sent to it.".into(),
            generic_error: "That did not work. Try again in a moment.".into(),
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
    Mfa { code: String },
    Hosted,
    /// The page came back from the provider with an authorisation code (the hosted flow).
    HostedFinish { code: String, state: String },
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
            Action::Hosted => f.write_str("Hosted"),
            Action::HostedFinish { .. } => f.debug_struct("HostedFinish").field("code", &Hidden).field("state", &Hidden).finish(),
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
    NeedsMfa { session: String },
    NeedsNewPassword,
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
            AuthResult::NeedsMfa { .. } => f.debug_struct("NeedsMfa").field("session", &Hidden).finish(),
            AuthResult::NeedsNewPassword => f.write_str("NeedsNewPassword"),
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
    /// A sign-in/up/forgot waiting for its reCAPTCHA token.
    pending: Option<Pending>,
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
            pending: None,
            busy: false,
            flow_user: String::new(),
        }
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
            Screen::Mfa => self.cfg.mfa && self.cfg.flow == Flow::InApp,
        }
    }

    /// The user acted.
    pub fn act(&mut self, a: Action) -> Vec<Op> {
        match a {
            Action::Show(s) => {
                if self.allowed(s) && !self.session.is_signed_in() {
                    self.screen = s;
                }
                vec![]
            }
            Action::SignIn { user, password } => self.guarded(Pending::SignIn { user, password }, "sign_in"),
            Action::SignUp { user, email, password } => self.guarded(Pending::SignUp { user, email, password }, "sign_up"),
            Action::Forgot { user } => self.guarded(Pending::Forgot { user }, "forgot_password"),
            Action::Confirm { user, code } => self.call(BackendCall::Confirm { user, code }),
            Action::Reset { user, code, new_password } => self.call(BackendCall::Reset { user, code, new_password }),
            Action::Mfa { code } => match self.mfa_session.clone() {
                Some(Secret(session)) => self.call(BackendCall::Mfa { session, code }),
                None => vec![],
            },
            Action::Hosted => self.call(BackendCall::HostedStart),
            Action::HostedFinish { code, state } => self.call(BackendCall::HostedFinish { code, state }),
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

    /// A sign-in, sign-up or forgot request: with reCAPTCHA on it waits for the token first (R4).
    fn guarded(&mut self, p: Pending, action: &'static str) -> Vec<Op> {
        if self.busy {
            return vec![];
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
                // A silent refresh does not announce a new sign-in.
                if matches!(call, BackendCall::Refresh) && was {
                    vec![]
                } else {
                    vec![Op::Notify(AuthEvent::SignedIn)]
                }
            }
            (BackendCall::SignUp { .. }, AuthResult::NeedsConfirm) | (BackendCall::SignIn { .. }, AuthResult::NeedsConfirm) => {
                self.screen = if self.cfg.confirm { Screen::Confirm } else { Screen::SignIn };
                vec![Op::Notify(AuthEvent::SignUpCompleted)]
            }
            (BackendCall::SignUp { .. }, AuthResult::Done) | (BackendCall::Confirm { .. }, AuthResult::Done) => {
                self.screen = Screen::SignIn;
                vec![Op::Notify(AuthEvent::SignUpCompleted)]
            }
            (_, AuthResult::NeedsMfa { session }) => {
                self.mfa_session = Some(Secret(session));
                self.screen = Screen::Mfa;
                vec![Op::Notify(AuthEvent::MfaRequired)]
            }
            // The provider asks for a new password before it signs the user in (a temporary one).
            (BackendCall::SignIn { .. }, AuthResult::NeedsNewPassword) if self.cfg.forgot => {
                self.screen = Screen::Reset;
                vec![]
            }
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
            (BackendCall::SignIn { .. } | BackendCall::Mfa { .. } | BackendCall::HostedFinish { .. }, AuthResult::Failed { detail }) => {
                // One message for an unknown user and a wrong password alike (R10).
                vec![Op::Message(t.sign_in_failed), Op::Notify(AuthEvent::Error(detail))]
            }
            (_, AuthResult::Failed { detail }) => vec![Op::Message(t.generic_error), Op::Notify(AuthEvent::Error(detail))],
            (_, _) => vec![Op::Message(t.generic_error)],
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
}

impl AuthUi {
    pub fn new(cfg: Config) -> AuthUi {
        AuthUi { machine: Machine::new(cfg), message: String::new(), attempt: 0, user_hint: String::new(), interacted: false }
    }
}

/// The words on the control. Every one is a property the developer can set (WASM-009 R1); these
/// are the defaults.
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
}

impl Labels {
    /// Read the labels through `get` (a property by name; empty = not set).
    pub fn read(get: &dyn Fn(&str) -> String) -> Labels {
        let pick = |prop: &str, default: &str| {
            let v = get(prop);
            if v.trim().is_empty() {
                default.to_owned()
            } else {
                v
            }
        };
        Labels {
            sign_in: pick("SignInCaption", "Sign in"),
            sign_up: pick("SignUpCaption", "Create account"),
            confirm: pick("ConfirmCaption", "Confirm account"),
            forgot: pick("ForgotCaption", "Forgot password"),
            reset: pick("ResetCaption", "Choose a new password"),
            mfa: pick("MfaCaption", "Verification code"),
            sign_out: pick("SignOutCaption", "Sign out"),
            hosted: pick("HostedCaption", "Continue to sign in"),
            user: pick("UserLabel", "User name"),
            email: pick("EmailLabel", "E-mail"),
            password: pick("PasswordLabel", "Password"),
            new_password: pick("NewPasswordLabel", "New password"),
            code: pick("CodeLabel", "Code"),
            create_account: pick("CreateAccountLink", "Create an account"),
            forgot_link: pick("ForgotLink", "Forgot your password?"),
            back: pick("BackCaption", "Back to sign in"),
            signed_in_as: pick("SignedInAsText", "Signed in as"),
            busy: pick("BusyText", "Please wait"),
        }
    }
}

/// The control's configuration from its properties; the page's settings fill what the control leaves open.
pub fn config_from(get: &dyn Fn(&str) -> String, default_flow: Flow, default_site_key: &str) -> Config {
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
    let mut texts = Texts::default();
    for (prop, slot) in [("SignInFailedText", &mut texts.sign_in_failed), ("ResetRequestedText", &mut texts.reset_requested), ("ErrorText", &mut texts.generic_error)] {
        let v = get(prop);
        if !v.trim().is_empty() {
            *slot = v;
        }
    }
    Config { flow, sign_up: on("ShowSignUp"), confirm: on("ShowConfirm"), forgot: on("ShowForgot"), mfa: on("ShowMfa"), recaptcha_site_key: site, texts }
}

// ── the wire ────────────────────────────────────────────────────────────────

/// How a [`BackendCall`] travels over the player's connection to the backend and how its answer
/// comes back. **The player's proposal** pending the identity programs' wire (WASM-009 R7,
/// WASM-014); it lives here, in one place, so that aligning it is a change to this module.
///
/// The request is the connection's ordinary call frame (a JSON object), the answer its ordinary
/// answer: `{"id":N,"result":{"status":…}}` or `{"id":N,"error":{…}}`.
pub mod wire {
    use super::{AuthResult, BackendCall};
    use serde_json::{json, Value};

    /// Ids from this one up to the remote grids' range (`crate::grid::ID_BASE`) are the
    /// Authentication control's; below it the program module's (ABI A8).
    pub const ID_BASE: u32 = 0x8000_0000;

    pub fn is_auth_id(id: u32) -> bool {
        id >= ID_BASE && id < crate::grid::ID_BASE
    }

    /// The request frame for `call`.
    pub fn encode(call: &BackendCall) -> Vec<u8> {
        let v = match call {
            BackendCall::SignIn { user, password, captcha } => json!({"identity": "sign-in", "user": user, "password": password, "captcha": captcha}),
            BackendCall::SignUp { user, email, password, captcha } => {
                json!({"identity": "sign-up", "user": user, "email": email, "password": password, "captcha": captcha})
            }
            BackendCall::Confirm { user, code } => json!({"identity": "confirm", "user": user, "code": code}),
            BackendCall::Forgot { user, captcha } => json!({"identity": "forgot", "user": user, "captcha": captcha}),
            BackendCall::Reset { user, code, new_password } => json!({"identity": "reset", "user": user, "code": code, "new_password": new_password}),
            BackendCall::Mfa { session, code } => json!({"identity": "mfa", "session": session, "code": code}),
            BackendCall::Refresh => json!({"identity": "refresh"}),
            BackendCall::SignOut => json!({"identity": "sign-out"}),
            BackendCall::HostedStart => json!({"identity": "hosted-start"}),
            BackendCall::HostedFinish { code, state } => json!({"identity": "hosted-finish", "code": code, "state": state}),
        };
        v.to_string().into_bytes()
    }

    /// What an answer frame means. An error frame, or one that cannot be read, is a failure whose
    /// detail is the error's class and code — never its text (R6, R10).
    pub fn decode(frame: &[u8]) -> AuthResult {
        let Ok(v) = serde_json::from_slice::<Value>(frame) else {
            return AuthResult::Failed { detail: "malformed answer".into() };
        };
        if let Some(e) = v.get("error") {
            let class = e.get("class").and_then(Value::as_str).unwrap_or("error");
            let code = e.get("code").and_then(Value::as_i64).unwrap_or(0);
            return AuthResult::Failed { detail: format!("{class}/{code}") };
        }
        let Some(r) = v.get("result") else {
            return AuthResult::Failed { detail: "malformed answer".into() };
        };
        let text = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_owned);
        match r.get("status").and_then(Value::as_str) {
            Some("signed-in") => match text("access_token") {
                Some(access_token) => AuthResult::SignedIn { access_token, expires_in_s: r.get("expires_in").and_then(Value::as_f64) },
                None => AuthResult::Failed { detail: "signed-in answer without a token".into() },
            },
            Some("needs-confirm") => AuthResult::NeedsConfirm,
            Some("needs-mfa") => AuthResult::NeedsMfa { session: text("session").unwrap_or_default() },
            Some("needs-new-password") => AuthResult::NeedsNewPassword,
            Some("reset-requested") => AuthResult::ResetRequested,
            Some("done") => AuthResult::Done,
            Some("hosted") => match text("redirect_url") {
                Some(redirect_url) => AuthResult::Hosted { redirect_url },
                None => AuthResult::Failed { detail: "hosted answer without an address".into() },
            },
            _ => AuthResult::Failed { detail: "unknown status".into() },
        }
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
        MockBackend { users: Default::default(), log: Default::default(), require_mfa: false, captchas_seen: Default::default() }
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
                    Some(u) if u.password == password && self.require_mfa => AuthResult::NeedsMfa { session: "mfa-session".into() },
                    Some(u) if u.password == password => AuthResult::SignedIn {
                        access_token: MockBackend::token(&serde_json::json!({"sub": "u-1", "email": u.email, "preferred_username": user, "exp": 4102444800u64, "groups": ["a", "b"]})),
                        expires_in_s: Some(3600.0),
                    },
                    _ => AuthResult::Failed { detail: "bad credentials".into() },
                }
            }
            BackendCall::SignUp { user, email, password, captcha } => {
                self.captchas_seen.borrow_mut().push(captcha);
                self.users.borrow_mut().insert(user, MockUser { password, confirmed: false, email });
                AuthResult::NeedsConfirm
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
            BackendCall::Mfa { .. } => AuthResult::Failed { detail: "bad mfa".into() },
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
        assert!(shown.contains(&Op::Notify(AuthEvent::SignUpCompleted)));
        assert_eq!(m.screen, Screen::Confirm);
        let bad = run(&mut m, &b, Action::Confirm { user: "bia".into(), code: "000".into() });
        assert!(bad.iter().any(|o| matches!(o, Op::Message(_))));
        run(&mut m, &b, Action::Confirm { user: "bia".into(), code: "123456".into() });
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
        let ops = m.on_result(&call, AuthResult::NeedsNewPassword, 0.0);
        assert!(ops.is_empty());
        assert_eq!(m.screen, Screen::Reset);
    }

    #[test]
    fn every_call_encodes_to_one_json_object_and_every_status_decodes() {
        let calls = [
            BackendCall::SignIn { user: "u".into(), password: "p".into(), captcha: Some("c".into()) },
            BackendCall::SignUp { user: "u".into(), email: "e".into(), password: "p".into(), captcha: None },
            BackendCall::Confirm { user: "u".into(), code: "1".into() },
            BackendCall::Forgot { user: "u".into(), captcha: None },
            BackendCall::Reset { user: "u".into(), code: "1".into(), new_password: "n".into() },
            BackendCall::Mfa { session: "s".into(), code: "1".into() },
            BackendCall::Refresh,
            BackendCall::SignOut,
            BackendCall::HostedStart,
            BackendCall::HostedFinish { code: "c".into(), state: "s".into() },
        ];
        let mut kinds = std::collections::BTreeSet::new();
        for c in &calls {
            let v: serde_json::Value = serde_json::from_slice(&wire::encode(c)).unwrap();
            assert!(v.is_object());
            kinds.insert(v["identity"].as_str().unwrap().to_owned());
        }
        assert_eq!(kinds.len(), calls.len(), "one request kind per call: {kinds:?}");

        let ok = |s: &str| format!(r#"{{"id":1,"result":{s}}}"#).into_bytes();
        assert_eq!(wire::decode(&ok(r#"{"status":"signed-in","access_token":"t","expires_in":60}"#)), AuthResult::SignedIn { access_token: "t".into(), expires_in_s: Some(60.0) });
        assert_eq!(wire::decode(&ok(r#"{"status":"needs-confirm"}"#)), AuthResult::NeedsConfirm);
        assert_eq!(wire::decode(&ok(r#"{"status":"needs-mfa","session":"s"}"#)), AuthResult::NeedsMfa { session: "s".into() });
        assert_eq!(wire::decode(&ok(r#"{"status":"needs-new-password"}"#)), AuthResult::NeedsNewPassword);
        assert_eq!(wire::decode(&ok(r#"{"status":"reset-requested"}"#)), AuthResult::ResetRequested);
        assert_eq!(wire::decode(&ok(r#"{"status":"done"}"#)), AuthResult::Done);
        assert_eq!(wire::decode(&ok(r#"{"status":"hosted","redirect_url":"https://idp/x"}"#)), AuthResult::Hosted { redirect_url: "https://idp/x".into() });
        // Malformed or refused answers are failures with no text from the network in them.
        for bad in [&b"not json"[..], br#"{"id":1}"#, br#"{"id":1,"result":{"status":"signed-in"}}"#, br#"{"id":1,"result":{"status":"?"}}"#] {
            assert!(matches!(wire::decode(bad), AuthResult::Failed { .. }), "{}", String::from_utf8_lossy(bad));
        }
        let f = wire::decode(br#"{"id":1,"error":{"class":"authentication","code":3,"message":"Nope, ana@example.com is unknown"}}"#);
        assert_eq!(f, AuthResult::Failed { detail: "authentication/3".into() }, "the error's text is never carried (R6, R10)");
    }

    #[test]
    fn auth_ids_are_apart_from_the_modules() {
        assert!(!wire::is_auth_id(1) && !wire::is_auth_id(0x7fff_ffff));
        assert!(wire::is_auth_id(wire::ID_BASE) && wire::is_auth_id(wire::ID_BASE + 5));
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
        let shown = format!("{:?}", AuthResult::NeedsMfa { session: "mfa-session-xyz".into() });
        assert!(!shown.contains("mfa-session-xyz"), "{shown}");
        // The machine, while a password waits for its reCAPTCHA token and while an MFA session is held.
        let mut m = Machine::new(Config { recaptcha_site_key: "k".into(), ..Config::default() });
        m.act(Action::SignIn { user: "ana".into(), password: "hunter2".into() });
        m.busy = false;
        m.on_result(&BackendCall::SignIn { user: "ana".into(), password: String::new(), captcha: None }, AuthResult::NeedsMfa { session: "mfa-session-xyz".into() }, 0.0);
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
        let l = Labels::read(&none);
        assert_eq!((l.sign_in.as_str(), l.user.as_str(), l.password.as_str()), ("Sign in", "User name", "Password"));
        let pt = |p: &str| if p == "SignInCaption" { "Entrar".to_owned() } else { String::new() };
        assert_eq!(Labels::read(&pt).sign_in, "Entrar");
        assert_eq!(Labels::read(&pt).sign_up, "Create account", "the others keep their defaults");

        let c = config_from(&none, Flow::InApp, "");
        assert!(c.sign_up && c.confirm && c.forgot && c.mfa && c.flow == Flow::InApp && c.recaptcha_site_key.is_empty());
        let props = |p: &str| match p {
            "Mode" => "Hosted".to_owned(),
            "ShowSignUp" => "0".to_owned(),
            "RecaptchaSiteKey" => " site ".to_owned(),
            "ErrorText" => "Falhou.".to_owned(),
            _ => String::new(),
        };
        let c = config_from(&props, Flow::InApp, "page-key");
        assert_eq!((c.flow, c.sign_up, c.forgot, c.recaptcha_site_key.as_str(), c.texts.generic_error.as_str()), (Flow::Hosted, false, true, "site", "Falhou."));
        let c = config_from(&none, Flow::Hosted, "page-key");
        assert_eq!((c.flow, c.recaptcha_site_key.as_str()), (Flow::Hosted, "page-key"), "the page's settings fill what the control leaves open");
    }
}
