// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The identity operations the generated `AUTH-…` programs call (WASM-009 R7-R10, D8).
//!
//! One entry point, [`IdentityService::run`]: an operation name, the `AUTH-REQUEST` group as
//! JSON, the caller's source address; the `AUTH-RESULT` group as JSON. What it adds to the
//! provider's adapter is the policy that must not live in a provider:
//!
//! - **generic answers** (R10): a failed sign-in is `AUTH-FAILED` with one message, whatever the
//!   provider said; a reset request is always `AUTH-RESET-REQUESTED`; a sign-up for a name that
//!   exists answers like one that worked;
//! - **a timing pad**: no answer of a guarded operation comes back sooner than the median of the
//!   recent successful sign-ins, and never under 250 ms, so the clock does not tell an unknown user
//!   from a wrong password;
//! - **rate limits of its own**, per source address and per account — the account key is a
//!   SHA-256 of the lower-cased name, never the name — added to the provider's own lockout, which
//!   is relied on and not reimplemented;
//! - **reCAPTCHA** where it is configured, failing closed, refused with the sign-in message;
//! - **sealed continuations**: a challenge's provider session, and the name it belongs to, leave
//!   the backend sealed (D7), so the page can neither read nor change them;
//! - **the hosted flow**: `START-HOSTED` builds the provider's address with PKCE and seals what the
//!   return needs; `FINISH-HOSTED` checks `state` and exchanges the code.
//!
//! Nothing here logs a password, a code, a token or a name: the log records the operation, the
//! outcome and the time.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use cobolt_wire::error::{codes, CallError, ErrorClass};

use super::oidc::{pkce, random_token};
use super::recaptcha::{CaptchaVerifier, Verdict};
use super::seal::Sealer;
use super::{Challenge, ChallengeResponse, IdentityProvider, IdpError, MfaChoice, MfaKind, SignIn, SignUp, SignUpRequest, Tokens};
use crate::limits::KeyedLimiter;

pub const FAILED_MESSAGE: &str = "The user name or password is not correct.";
pub const RESET_MESSAGE: &str = "If the account exists, a code was sent to it.";
pub const CODE_MESSAGE: &str = "The code is not correct or has expired.";
pub const RATE_MESSAGE: &str = "Too many attempts. Wait a minute and try again.";
pub const HOSTED_MESSAGE: &str = "Continue on the provider's sign-in page.";
pub const UNAVAILABLE_MESSAGE: &str = "The sign-in service is not available; try again later.";

/// The outcomes an `AUTH-RESULT` carries in `OUTCOME`.
pub mod outcome {
    pub const OK: &str = "OK";
    pub const CHALLENGE: &str = "CHALLENGE";
    pub const CONFIRMATION_REQUIRED: &str = "CONFIRMATION-REQUIRED";
    pub const FAILED: &str = "AUTH-FAILED";
    pub const RESET_REQUESTED: &str = "AUTH-RESET-REQUESTED";
    pub const INVALID: &str = "INVALID";
    pub const RATE_LIMITED: &str = "RATE-LIMITED";
    pub const HOSTED: &str = "HOSTED";
}

/// The members of `AUTH-RESULT`, all present in every answer (empty when unused).
pub const RESULT_MEMBERS: &[&str] = &[
    "OUTCOME", "OUTCOME-TEXT", "ACCESS-TOKEN", "ID-TOKEN", "REFRESH-TOKEN", "USERNAME", "CHALLENGE", "DESTINATION", "CHOICES", "SECRET", "OTPAUTH-URI", "SESSION", "URL", "STATE", "HOSTED",
    "RETURN-URI", "PAGE-STATE", "FLOW", "CAPABILITIES", "SITE-KEY",
];

/// The members of `AUTH-REQUEST`.
pub const REQUEST_MEMBERS: &[&str] = &[
    "USERNAME", "PASSWORD", "ONE-TIME-CODE", "SESSION", "CHOICE", "EMAIL", "PHONE", "GIVEN-NAME", "FAMILY-NAME", "CAPTCHA-TOKEN", "REFRESH-TOKEN", "ACCESS-TOKEN", "REDIRECT-URI", "RETURN-URI",
    "PAGE-STATE", "STATE", "HOSTED",
];

/// The operations, as `AUTH-OP` names them (D4: `AUTH-<op>` is the program).
pub const OPERATIONS: &[&str] = &["SIGN-IN", "SIGN-UP", "CONFIRM", "RESEND", "FORGOT", "RESET", "MFA", "MFA-SETUP", "REFRESH", "SIGN-OUT", "START-HOSTED", "FINISH-HOSTED", "CAPABILITIES"];

/// The operations that are padded, limited and (some) guarded by reCAPTCHA.
const GUARDED: &[&str] = &["SIGN-IN", "SIGN-UP", "CONFIRM", "RESEND", "FORGOT", "RESET", "MFA", "MFA-SETUP"];

/// The reCAPTCHA action each guarded operation's token must name (the player's `guarded` names).
fn captcha_action(op: &str) -> Option<&'static str> {
    match op {
        "SIGN-IN" => Some("sign_in"),
        "SIGN-UP" => Some("sign_up"),
        "FORGOT" => Some("forgot_password"),
        _ => None,
    }
}

/// How long a sealed continuation is good for.
const CHALLENGE_AGE: Duration = Duration::from_secs(15 * 60);
const FLOW_AGE: Duration = Duration::from_secs(60 * 60);
pub const HOSTED_AGE: Duration = Duration::from_secs(10 * 60);
/// The successes the pad's median is taken over.
const SAMPLES: usize = 64;

/// What the service needs besides the provider.
pub struct ServiceConfig {
    /// Attempts allowed per minute from one source address, and for one account.
    pub attempts_per_minute: u32,
    /// The shortest time a guarded answer may take.
    pub floor: Duration,
    /// The page origins a hosted sign-in may return to.
    pub origins: Vec<String>,
    /// The public reCAPTCHA site key, served by `CAPABILITIES`.
    pub site_key: Option<String>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        ServiceConfig { attempts_per_minute: 10, floor: Duration::from_millis(250), origins: Vec::new(), site_key: None }
    }
}

/// Who is calling, as far as the backend knows.
#[derive(Debug, Clone, Default)]
pub struct OpContext {
    pub ip: String,
}

pub struct IdentityService {
    provider: Box<dyn IdentityProvider>,
    captcha: Option<Box<dyn CaptchaVerifier>>,
    sealer: Sealer,
    cfg: ServiceConfig,
    by_source: KeyedLimiter,
    by_account: KeyedLimiter,
    successes: Mutex<VecDeque<Duration>>,
}

fn text(req: &Value, key: &str) -> String {
    req.get(key).and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
}

/// The rate-limit key of an account: never the name itself.
pub fn account_key(username: &str) -> String {
    let d = Sha256::digest(username.trim().to_lowercase().as_bytes());
    format!("a:{}", d.iter().take(16).map(|b| format!("{b:02x}")).collect::<String>())
}

/// The origin of an absolute address (`https://host[:port]`).
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.is_empty() && !host.contains('@') && matches!(scheme, "https" | "http")).then(|| format!("{scheme}://{host}"))
}

/// The result group: every member present, empty when unused.
struct Res(serde_json::Map<String, Value>);

impl Res {
    fn new(outcome: &str, message: &str) -> Res {
        let mut m = serde_json::Map::new();
        for k in RESULT_MEMBERS {
            m.insert(k.to_string(), json!(""));
        }
        m.insert("EXPIRES-IN".to_string(), json!(0));
        m.insert("OUTCOME".to_string(), json!(outcome));
        m.insert("OUTCOME-TEXT".to_string(), json!(message));
        Res(m)
    }

    fn set(mut self, key: &str, value: impl Into<Value>) -> Res {
        debug_assert!(self.0.contains_key(key), "{key} is not a member of AUTH-RESULT");
        self.0.insert(key.to_string(), value.into());
        self
    }

    fn json(self) -> String {
        Value::Object(self.0).to_string()
    }
}

fn failed() -> Result<String, CallError> {
    Ok(Res::new(outcome::FAILED, FAILED_MESSAGE).json())
}

impl IdentityService {
    pub fn new(provider: Box<dyn IdentityProvider>, captcha: Option<Box<dyn CaptchaVerifier>>, sealer: Sealer, cfg: ServiceConfig) -> Self {
        IdentityService {
            by_source: KeyedLimiter::per_minute(cfg.attempts_per_minute),
            by_account: KeyedLimiter::per_minute(cfg.attempts_per_minute),
            provider,
            captcha,
            sealer,
            cfg,
            successes: Mutex::new(VecDeque::new()),
        }
    }

    pub fn provider(&self) -> &dyn IdentityProvider {
        self.provider.as_ref()
    }

    pub fn sealer(&self) -> &Sealer {
        &self.sealer
    }

    /// The time no guarded answer comes back sooner than: the median of the recent successful
    /// sign-ins, at least the floor, at most five seconds.
    pub fn pad_target(&self) -> Duration {
        let median = self.successes.lock().ok().and_then(|s| {
            let mut v: Vec<Duration> = s.iter().copied().collect();
            v.sort();
            v.get(v.len() / 2).copied()
        });
        median.unwrap_or_default().clamp(self.cfg.floor, Duration::from_secs(5).max(self.cfg.floor))
    }

    /// One operation.
    pub fn run(&self, op: &str, request_json: &str, ctx: &OpContext) -> Result<String, CallError> {
        let started = Instant::now();
        let req: Value = serde_json::from_str(request_json).map_err(|_| CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, "the identity request is not JSON"))?;
        let op = op.trim().to_ascii_uppercase();
        let guarded = GUARDED.contains(&op.as_str());
        let result = self.dispatch(&op, &req, ctx);
        let what = outcome_of(&result);
        if guarded {
            // A sign-in the provider accepted (tokens, or a second step) teaches the pad how long
            // success takes; the time is taken before any pad.
            if op == "SIGN-IN" && (what == outcome::OK || what == outcome::CHALLENGE) {
                if let Ok(mut s) = self.successes.lock() {
                    if s.len() == SAMPLES {
                        s.pop_front();
                    }
                    s.push_back(started.elapsed());
                }
            }
            let target = self.pad_target();
            let spent = started.elapsed();
            if spent < target {
                std::thread::sleep(target - spent);
            }
        }
        tracing::info!(target: "identity", op = %op, outcome = %what, ms = started.elapsed().as_millis() as u64, "identity operation");
        result
    }

    fn limited(&self, ctx: &OpContext, username: &str) -> bool {
        // Both buckets are charged, so neither can be probed without spending the other.
        let source = self.by_source.take(&format!("s:{}", ctx.ip));
        let account = username.is_empty() || self.by_account.take(&account_key(username));
        !(source && account)
    }

    fn captcha_ok(&self, op: &str, req: &Value, ctx: &OpContext) -> bool {
        let (Some(c), Some(action)) = (&self.captcha, captcha_action(op)) else { return true };
        match c.verify(&text(req, "CAPTCHA-TOKEN"), action, Some(&ctx.ip)) {
            Verdict::Human => true,
            Verdict::Refused(why) => {
                tracing::info!(target: "identity", op = %op, "reCAPTCHA refused: {why}");
                false
            }
            Verdict::Unavailable(why) => {
                tracing::warn!(target: "identity", op = %op, "reCAPTCHA unavailable (failing closed): {why}");
                false
            }
        }
    }

    fn dispatch(&self, op: &str, req: &Value, ctx: &OpContext) -> Result<String, CallError> {
        // The name a continuation was sealed for wins over one the page sends.
        let flow = self.open_flow(req);
        let username = flow.as_ref().map(|f| f.0.clone()).filter(|u| !u.is_empty()).unwrap_or_else(|| text(req, "USERNAME"));
        if GUARDED.contains(&op) {
            if self.limited(ctx, &username) {
                return Ok(Res::new(outcome::RATE_LIMITED, RATE_MESSAGE).json());
            }
            if !self.captcha_ok(op, req, ctx) {
                // D9: the same answer as a wrong password.
                return match op {
                    "FORGOT" => Ok(Res::new(outcome::RESET_REQUESTED, RESET_MESSAGE).json()),
                    _ => failed(),
                };
            }
        }
        let provider_session = flow.map(|f| f.1).unwrap_or_default();
        match op {
            "SIGN-IN" => {
                let password = text(req, "PASSWORD");
                if username.is_empty() || password.is_empty() {
                    return failed();
                }
                let r = self.provider.sign_in(&username, &password);
                self.signin_result(r, &username)
            }
            "MFA" | "MFA-SETUP" => {
                let Some(c) = self.open_challenge(req) else { return failed() };
                let answer = ChallengeResponse {
                    username: c.0.clone(),
                    name: c.1,
                    session: c.2,
                    code: text(req, "ONE-TIME-CODE"),
                    new_password: text(req, "PASSWORD"),
                    choice: MfaChoice::from_wire(&text(req, "CHOICE")),
                    setup: op == "MFA-SETUP",
                };
                let r = self.provider.respond_to_challenge(&answer);
                self.signin_result(r, &c.0)
            }
            "SIGN-UP" => {
                let r = SignUpRequest {
                    username: username.clone(),
                    password: text(req, "PASSWORD"),
                    email: text(req, "EMAIL"),
                    phone: text(req, "PHONE"),
                    given_name: text(req, "GIVEN-NAME"),
                    family_name: text(req, "FAMILY-NAME"),
                };
                if r.username.is_empty() || r.password.is_empty() {
                    return Ok(Res::new(outcome::INVALID, "Enter a user name and a password.").json());
                }
                match self.provider.sign_up(&r) {
                    Ok(SignUp::Done) => Ok(Res::new(outcome::OK, "Your account is ready. Sign in.").set("USERNAME", username).json()),
                    Ok(SignUp::ConfirmationRequired { destination, session }) => Ok(self.confirmation(&username, &session).set("DESTINATION", destination).json()),
                    // A name that exists answers like a name that worked: the page cannot be used
                    // to find out who has an account.
                    Err(IdpError::Rejected) => Ok(self.confirmation(&username, "").json()),
                    Err(e) => self.failure(e),
                }
            }
            "CONFIRM" => match self.provider.confirm_sign_up(&username, &text(req, "ONE-TIME-CODE"), &provider_session) {
                Ok(()) => Ok(Res::new(outcome::OK, "Your account is confirmed. Sign in.").set("USERNAME", username).json()),
                Err(IdpError::Rejected) => Ok(Res::new(outcome::FAILED, CODE_MESSAGE).json()),
                Err(e) => self.failure(e),
            },
            "RESEND" => {
                // The same answer whatever the provider said about the name; the continuation is
                // the provider's new one when it rotates it, else the one the page holds.
                let session = match self.provider.resend_code(&username, &provider_session) {
                    Ok(next) => next.unwrap_or(provider_session),
                    Err(IdpError::Rejected) | Err(IdpError::Invalid(_)) => provider_session,
                    Err(e) => return self.failure(e),
                };
                Ok(Res::new(outcome::RESET_REQUESTED, RESET_MESSAGE).set("SESSION", self.seal_flow(&username, &session)).json())
            }
            "FORGOT" => {
                // Whatever the provider said about the name, the answer is the same — including
                // the sealed continuation, which exists whether or not the account does.
                let session = match self.provider.forgot_password(&username) {
                    Ok(s) => s.unwrap_or_default(),
                    Err(IdpError::Rejected) | Err(IdpError::Invalid(_)) => String::new(),
                    Err(e) => return self.failure(e),
                };
                Ok(Res::new(outcome::RESET_REQUESTED, RESET_MESSAGE).set("SESSION", self.seal_flow(&username, &session)).json())
            }
            "RESET" => match self.provider.confirm_forgot_password(&username, &text(req, "ONE-TIME-CODE"), &text(req, "PASSWORD"), &provider_session) {
                Ok(()) => Ok(Res::new(outcome::OK, "Your password was changed. Sign in.").set("USERNAME", username).json()),
                Err(IdpError::Rejected) => Ok(Res::new(outcome::FAILED, CODE_MESSAGE).json()),
                Err(e) => self.failure(e),
            },
            "REFRESH" => {
                let rt = text(req, "REFRESH-TOKEN");
                if rt.is_empty() {
                    return failed();
                }
                let r = self.provider.refresh(&rt, &username);
                match r {
                    Ok(t) => Ok(self.deliver(t, &username)),
                    Err(e) => self.failure(e),
                }
            }
            "SIGN-OUT" => {
                let rt = text(req, "REFRESH-TOKEN");
                if let Err(e) = self.provider.sign_out(&text(req, "ACCESS-TOKEN"), (!rt.is_empty()).then_some(rt.as_str())) {
                    // Signing out locally always works; the provider's half is best effort.
                    tracing::warn!(target: "identity", "the provider's sign-out did not complete: {e}");
                }
                Ok(Res::new(outcome::OK, "You are signed out.").json())
            }
            "START-HOSTED" => self.start_hosted(req),
            "FINISH-HOSTED" => self.finish_hosted(req),
            "CAPABILITIES" => {
                let caps = self.provider.capabilities();
                let on: Vec<&str> = caps.named().iter().filter(|(_, v)| *v).map(|(k, _)| *k).collect();
                Ok(Res::new(outcome::OK, "")
                    .set("FLOW", self.provider.flow().word())
                    .set("CAPABILITIES", on.join(","))
                    .set("SITE-KEY", self.cfg.site_key.clone().unwrap_or_default())
                    .json())
            }
            other => Err(CallError::new(ErrorClass::NotFound, codes::UNKNOWN_PROGRAM, format!("there is no identity operation \"{other}\""))),
        }
    }

    fn confirmation(&self, username: &str, session: &str) -> Res {
        Res::new(outcome::CONFIRMATION_REQUIRED, "Enter the code we sent you.").set("SESSION", self.seal_flow(username, session)).set("USERNAME", username.to_string())
    }

    fn seal_flow(&self, username: &str, session: &str) -> String {
        self.sealer.seal_json("flow", &json!({"u": username, "s": session}))
    }

    /// The name and provider session a `flow` continuation was sealed with.
    fn open_flow(&self, req: &Value) -> Option<(String, String)> {
        let s = text(req, "SESSION");
        let v = self.sealer.open_json("flow", &s, FLOW_AGE)?;
        Some((v.get("u")?.as_str()?.to_owned(), v.get("s")?.as_str()?.to_owned()))
    }

    /// The name, the provider's step name and its session a challenge was sealed with.
    fn open_challenge(&self, req: &Value) -> Option<(String, String, String)> {
        let v = self.sealer.open_json("challenge", &text(req, "SESSION"), CHALLENGE_AGE)?;
        Some((v.get("u")?.as_str()?.to_owned(), v.get("n")?.as_str()?.to_owned(), v.get("s")?.as_str()?.to_owned()))
    }

    fn failure(&self, e: IdpError) -> Result<String, CallError> {
        match e {
            IdpError::Invalid(m) => Ok(Res::new(outcome::INVALID, &m).json()),
            IdpError::Throttled => Ok(Res::new(outcome::RATE_LIMITED, RATE_MESSAGE).json()),
            IdpError::Unsupported => Ok(Res::new(outcome::HOSTED, HOSTED_MESSAGE).json()),
            IdpError::Rejected => failed(),
            IdpError::Unavailable(why) => {
                tracing::error!(target: "identity", "the identity provider is unavailable: {why}");
                Err(CallError::new(ErrorClass::Server, codes::INTERNAL, UNAVAILABLE_MESSAGE))
            }
        }
    }

    fn signin_result(&self, r: Result<SignIn, IdpError>, username: &str) -> Result<String, CallError> {
        match r {
            Ok(SignIn::Tokens(t)) => Ok(self.deliver(t, username)),
            Ok(SignIn::Challenge(Challenge { kind, name, session })) => {
                let sealed = self.sealer.seal_json("challenge", &json!({"u": username, "n": name, "s": session}));
                let mut res = Res::new(outcome::CHALLENGE, "Enter the code to continue.").set("CHALLENGE", kind.word()).set("SESSION", sealed);
                res = match kind {
                    MfaKind::Sms { destination } | MfaKind::Email { destination } => res.set("DESTINATION", destination),
                    MfaKind::Choice(c) => res.set("CHOICES", c.iter().map(|c| c.wire_name()).collect::<Vec<_>>().join(",")),
                    MfaKind::TotpSetup { secret, otpauth_uri } => res.set("SECRET", secret).set("OTPAUTH-URI", otpauth_uri),
                    MfaKind::Totp | MfaKind::NewPassword => res,
                };
                Ok(res.json())
            }
            Err(e) => self.failure(e),
        }
    }

    /// The tokens, for the route to deliver: it seals the refresh token into the cookie (web) or
    /// answers it in the body (`X-PR-Client: native`), and never forwards this group as it is.
    fn deliver(&self, t: Tokens, username: &str) -> String {
        let mut res = Res::new(outcome::OK, "").set("ACCESS-TOKEN", t.access).set("USERNAME", username.to_string()).set("EXPIRES-IN", t.expires_in);
        if let Some(id) = t.id {
            res = res.set("ID-TOKEN", id);
        }
        if let Some(rt) = t.refresh {
            res = res.set("REFRESH-TOKEN", rt);
        }
        res.json()
    }

    fn start_hosted(&self, req: &Value) -> Result<String, CallError> {
        let redirect = text(req, "REDIRECT-URI");
        let back = text(req, "RETURN-URI");
        let back_ok = origin_of(&back).is_some_and(|o| self.cfg.origins.iter().any(|a| a.eq_ignore_ascii_case(&o)));
        if redirect.is_empty() || !back_ok {
            return Ok(Res::new(outcome::INVALID, "That return address is not allowed.").json());
        }
        let state = random_token(16);
        let nonce = random_token(16);
        let (verifier, challenge) = pkce();
        match self.provider.start_hosted(&redirect, &state, &challenge, &nonce) {
            Ok(url) => {
                let hosted = self.sealer.seal_json("hosted", &json!({"v": verifier, "n": nonce, "st": state, "r": redirect, "b": back, "p": text(req, "PAGE-STATE")}));
                Ok(Res::new(outcome::OK, HOSTED_MESSAGE).set("URL", url).set("STATE", state).set("HOSTED", hosted).json())
            }
            Err(e) => self.failure(e),
        }
    }

    fn finish_hosted(&self, req: &Value) -> Result<String, CallError> {
        let Some(h) = self.sealer.open_json("hosted", &text(req, "HOSTED"), HOSTED_AGE) else { return failed() };
        let field = |k: &str| h.get(k).and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let (state, given) = (field("st"), text(req, "STATE"));
        // The state the provider echoed must be the one this browser's cookie was sealed with.
        if state.is_empty() || state.len() != given.len() || !state.bytes().zip(given.bytes()).fold(true, |ok, (a, b)| ok & (a == b)) {
            return failed();
        }
        match self.provider.finish_hosted(&text(req, "ONE-TIME-CODE"), &field("r"), &field("v")) {
            Ok(t) => {
                let delivered: Value = serde_json::from_str(&self.deliver(t, "")).unwrap_or_default();
                let mut res = Res(delivered.as_object().cloned().unwrap_or_default());
                res = res.set("RETURN-URI", field("b")).set("PAGE-STATE", field("p"));
                Ok(res.json())
            }
            Err(e) => self.failure(e),
        }
    }
}

fn outcome_of(r: &Result<String, CallError>) -> String {
    match r {
        Ok(s) => serde_json::from_str::<Value>(s).ok().and_then(|v| v.get("OUTCOME").and_then(|o| o.as_str()).map(str::to_owned)).unwrap_or_default(),
        Err(e) => format!("error:{}", e.code),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::super::client::{Http, HttpRequest, HttpResponse};
    use super::super::recaptcha::Recaptcha;
    use super::super::{Capabilities, Flow};
    use super::*;
    use crate::config::{ResolvedRecaptcha, Secret};

    /// A provider that knows `ana` / `right-password`.
    struct Fake;

    impl IdentityProvider for Fake {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn flow(&self) -> Flow {
            Flow::InApp
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities { sign_in: true, sign_up: true, refresh: true, ..Default::default() }
        }
        fn sign_in(&self, u: &str, p: &str) -> Result<SignIn, IdpError> {
            match (u, p) {
                ("ana", "right-password") => Ok(SignIn::Tokens(Tokens { access: "at-ana".into(), refresh: Some("rt-ana".into()), expires_in: 300, id: None })),
                ("mfa", "right-password") => Ok(SignIn::Challenge(Challenge { kind: MfaKind::Sms { destination: "+55*****12".into() }, name: "SMS_MFA".into(), session: "provider-session".into() })),
                _ => Err(IdpError::Rejected),
            }
        }
        fn respond_to_challenge(&self, a: &ChallengeResponse) -> Result<SignIn, IdpError> {
            if a.username == "mfa" && a.name == "SMS_MFA" && a.session == "provider-session" && a.code == "123456" {
                Ok(SignIn::Tokens(Tokens { access: "at-mfa".into(), ..Default::default() }))
            } else {
                Err(IdpError::Rejected)
            }
        }
        fn sign_up(&self, _: &SignUpRequest) -> Result<SignUp, IdpError> {
            Err(IdpError::Rejected)
        }
        fn confirm_sign_up(&self, _: &str, _: &str, _: &str) -> Result<(), IdpError> {
            Err(IdpError::Rejected)
        }
        fn resend_code(&self, _: &str, _: &str) -> Result<Option<String>, IdpError> {
            Ok(None)
        }
        fn forgot_password(&self, u: &str) -> Result<Option<String>, IdpError> {
            if u == "ana" {
                Ok(None)
            } else {
                Err(IdpError::Rejected)
            }
        }
        fn confirm_forgot_password(&self, _: &str, _: &str, _: &str, _: &str) -> Result<(), IdpError> {
            Err(IdpError::Rejected)
        }
        fn refresh(&self, _: &str, _: &str) -> Result<Tokens, IdpError> {
            Err(IdpError::Unavailable("down".into()))
        }
        fn sign_out(&self, _: &str, _: Option<&str>) -> Result<(), IdpError> {
            Ok(())
        }
        fn start_hosted(&self, r: &str, s: &str, c: &str, n: &str) -> Result<String, IdpError> {
            Ok(format!("https://idp.example.test/authorize?redirect_uri={r}&state={s}&code_challenge={c}&nonce={n}"))
        }
        fn finish_hosted(&self, code: &str, _: &str, verifier: &str) -> Result<Tokens, IdpError> {
            if code == "good-code" && verifier.len() == 43 {
                Ok(Tokens { access: "at-hosted".into(), refresh: Some("rt-hosted".into()), ..Default::default() })
            } else {
                Err(IdpError::Rejected)
            }
        }
    }

    fn service(captcha: Option<Box<dyn CaptchaVerifier>>) -> IdentityService {
        let cfg = ServiceConfig { attempts_per_minute: 1000, floor: Duration::from_millis(1), origins: vec!["https://app.example.test".into()], site_key: Some("site".into()) };
        IdentityService::new(Box::new(Fake), captcha, Sealer::new(&[9; 32]), cfg)
    }

    fn call(s: &IdentityService, op: &str, req: Value) -> Value {
        serde_json::from_str(&s.run(op, &req.to_string(), &OpContext { ip: "203.0.113.9".into() }).unwrap()).unwrap()
    }

    #[test]
    fn identity_every_answer_has_every_member_and_failures_are_generic() {
        let s = service(None);
        let ok = call(&s, "SIGN-IN", json!({"USERNAME": "ana", "PASSWORD": "right-password"}));
        assert_eq!((ok["OUTCOME"].as_str(), ok["ACCESS-TOKEN"].as_str(), ok["REFRESH-TOKEN"].as_str()), (Some("OK"), Some("at-ana"), Some("rt-ana")));
        for k in RESULT_MEMBERS.iter().chain(["EXPIRES-IN"].iter()) {
            assert!(ok.get(*k).is_some(), "{k}");
        }
        let unknown = call(&s, "SIGN-IN", json!({"USERNAME": "nobody", "PASSWORD": "x"}));
        let wrong = call(&s, "SIGN-IN", json!({"USERNAME": "ana", "PASSWORD": "wrong"}));
        assert_eq!(unknown, wrong);
        assert_eq!(unknown["OUTCOME"], "AUTH-FAILED");
        // A reset for an unknown and a known name: the same outcome and text (the sealed session differs only by its random nonce).
        let (a, b) = (call(&s, "FORGOT", json!({"USERNAME": "ana"})), call(&s, "FORGOT", json!({"USERNAME": "nobody"})));
        assert_eq!((&a["OUTCOME"], &a["OUTCOME-TEXT"]), (&b["OUTCOME"], &b["OUTCOME-TEXT"]));
        assert_eq!(a["OUTCOME"], "AUTH-RESET-REQUESTED");
        // A sign-up for a name that exists answers like one that worked.
        assert_eq!(call(&s, "SIGN-UP", json!({"USERNAME": "ana", "PASSWORD": "p"}))["OUTCOME"], "CONFIRMATION-REQUIRED");
        // An unreachable provider is a server error, with no detail for the page.
        let e = s.run("REFRESH", &json!({"REFRESH-TOKEN": "rt"}).to_string(), &OpContext::default()).unwrap_err();
        assert_eq!(e.message, UNAVAILABLE_MESSAGE);
        let caps = call(&s, "CAPABILITIES", json!({}));
        assert_eq!((caps["FLOW"].as_str(), caps["CAPABILITIES"].as_str(), caps["SITE-KEY"].as_str()), (Some("in-app"), Some("sign-in,sign-up,refresh"), Some("site")));
    }

    #[test]
    fn identity_a_challenge_session_is_sealed_and_cannot_be_moved_to_another_user() {
        let s = service(None);
        let c = call(&s, "SIGN-IN", json!({"USERNAME": "mfa", "PASSWORD": "right-password"}));
        assert_eq!((c["OUTCOME"].as_str(), c["CHALLENGE"].as_str(), c["DESTINATION"].as_str()), (Some("CHALLENGE"), Some("sms"), Some("+55*****12")));
        let session = c["SESSION"].as_str().unwrap();
        assert!(!session.contains("provider-session"), "the provider's session is sealed");
        // The page cannot swap the name: the sealed one is used.
        let done = call(&s, "MFA", json!({"SESSION": session, "ONE-TIME-CODE": "123456", "USERNAME": "ana"}));
        assert_eq!(done["ACCESS-TOKEN"], "at-mfa");
        let mut t = session.to_string();
        t.replace_range(10..11, if &t[10..11] == "A" { "B" } else { "A" });
        assert_eq!(call(&s, "MFA", json!({"SESSION": t, "ONE-TIME-CODE": "123456"}))["OUTCOME"], "AUTH-FAILED", "a tampered session");
    }

    #[test]
    fn identity_the_hosted_flow_checks_the_return_origin_and_the_state() {
        let s = service(None);
        let bad = call(&s, "START-HOSTED", json!({"REDIRECT-URI": "https://api.example.test/auth/hosted/callback", "RETURN-URI": "https://evil.example.test/"}));
        assert_eq!(bad["OUTCOME"], "INVALID");
        let st = call(&s, "START-HOSTED", json!({"REDIRECT-URI": "https://api.example.test/auth/hosted/callback", "RETURN-URI": "https://app.example.test/home", "PAGE-STATE": "ps1"}));
        assert_eq!(st["OUTCOME"], "OK");
        let url = st["URL"].as_str().unwrap();
        assert!(url.contains(&format!("state={}", st["STATE"].as_str().unwrap())), "{url}");
        let hosted = st["HOSTED"].as_str().unwrap();
        let wrong_state = call(&s, "FINISH-HOSTED", json!({"HOSTED": hosted, "STATE": "other", "ONE-TIME-CODE": "good-code"}));
        assert_eq!(wrong_state["OUTCOME"], "AUTH-FAILED");
        let done = call(&s, "FINISH-HOSTED", json!({"HOSTED": hosted, "STATE": st["STATE"], "ONE-TIME-CODE": "good-code"}));
        assert_eq!((done["ACCESS-TOKEN"].as_str(), done["RETURN-URI"].as_str(), done["PAGE-STATE"].as_str()), (Some("at-hosted"), Some("https://app.example.test/home"), Some("ps1")));
    }

    #[test]
    fn identity_attempts_are_limited_per_source_and_per_account_key() {
        let cfg = ServiceConfig { attempts_per_minute: 3, floor: Duration::from_millis(1), ..Default::default() };
        let s = IdentityService::new(Box::new(Fake), None, Sealer::new(&[9; 32]), cfg);
        let at = |ip: &str, user: &str| -> String {
            let r: Value = serde_json::from_str(&s.run("SIGN-IN", &json!({"USERNAME": user, "PASSWORD": "x"}).to_string(), &OpContext { ip: ip.into() }).unwrap()).unwrap();
            r["OUTCOME"].as_str().unwrap().to_owned()
        };
        let first: Vec<String> = (0..4).map(|_| at("198.51.100.1", "Ana")).collect();
        assert_eq!(first, ["AUTH-FAILED", "AUTH-FAILED", "AUTH-FAILED", "RATE-LIMITED"]);
        assert_eq!(at("198.51.100.2", "ANA"), "RATE-LIMITED", "the account is limited from another address, whatever the case of the name");
        assert_eq!(at("198.51.100.3", "bea"), "AUTH-FAILED");
        assert!(account_key("Ana").starts_with("a:") && !account_key("Ana").contains("ana"));
        assert_eq!(account_key(" Ana "), account_key("ana"));
    }

    #[test]
    fn identity_a_failure_is_padded_to_the_median_of_successes_and_never_under_the_floor() {
        let cfg = ServiceConfig { attempts_per_minute: 1000, floor: Duration::from_millis(60), ..Default::default() };
        let s = IdentityService::new(Box::new(Fake), None, Sealer::new(&[9; 32]), cfg);
        assert_eq!(s.pad_target(), Duration::from_millis(60), "no success yet: the floor");
        let t = Instant::now();
        s.run("SIGN-IN", &json!({"USERNAME": "x", "PASSWORD": "y"}).to_string(), &OpContext::default()).unwrap();
        assert!(t.elapsed() >= Duration::from_millis(60), "{:?}", t.elapsed());
        let t = Instant::now();
        s.run("CAPABILITIES", "{}", &OpContext::default()).unwrap();
        assert!(t.elapsed() < Duration::from_millis(60), "an unguarded operation is not padded");
    }

    // ── AC4: reCAPTCHA against a mocked siteverify ──

    /// `siteverify`, answering per token.
    struct SiteVerify(Mutex<u32>);

    impl Http for SiteVerify {
        fn send(&self, req: &HttpRequest) -> Result<HttpResponse, String> {
            *self.0.lock().unwrap() += 1;
            let f = req.form();
            let get = |k: &str| f.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
            assert_eq!(get("secret"), "recaptcha-secret");
            let body = match get("response").as_str() {
                "good" | "good-2" => json!({"success": true, "score": 0.9, "action": "sign_in", "hostname": "app.example.test"}),
                "other-action" => json!({"success": true, "score": 0.9, "action": "sign_up", "hostname": "app.example.test"}),
                "other-host" => json!({"success": true, "score": 0.9, "action": "sign_in", "hostname": "evil.example.test"}),
                "low" => json!({"success": true, "score": 0.2, "action": "sign_in", "hostname": "app.example.test"}),
                "quota" => json!({"success": true, "score": 0.9, "action": "sign_in", "hostname": "app.example.test", "error-codes": ["Over free quota."]}),
                _ => json!({"success": false, "error-codes": ["invalid-input-response"]}),
            };
            Ok(HttpResponse { status: 200, body: body.to_string() })
        }
    }

    #[test]
    fn identity_recaptcha_refuses_six_ways_with_the_sign_in_message_and_accepts_a_good_token() {
        let http = Arc::new(SiteVerify(Mutex::new(0)));
        let rc = Recaptcha::new(http.clone(), &ResolvedRecaptcha { site_key: None, secret: Secret::new("recaptcha-secret".to_string()), min_score: 0.5, hostnames: vec!["app.example.test".into()], endpoint: Some("http://127.0.0.1:1/siteverify".into()) });
        let s = service(Some(Box::new(rc)));
        let sign_in = |token: Option<&str>| {
            let mut r = json!({"USERNAME": "ana", "PASSWORD": "right-password"});
            if let Some(t) = token {
                r["CAPTCHA-TOKEN"] = json!(t);
            }
            call(&s, "SIGN-IN", r)
        };
        let good = sign_in(Some("good"));
        assert_eq!(good["OUTCOME"], "OK", "a good token is accepted");
        let mut rows = Vec::new();
        for (case, token) in [("no token", None), ("replayed", Some("good")), ("another action", Some("other-action")), ("another hostname", Some("other-host")), ("low score", Some("low")), ("over quota", Some("quota"))] {
            let r = sign_in(token);
            rows.push(format!("{case:16} -> {} / {}", r["OUTCOME"], r["OUTCOME-TEXT"]));
            assert_eq!((r["OUTCOME"].as_str(), r["OUTCOME-TEXT"].as_str()), (Some("AUTH-FAILED"), Some(FAILED_MESSAGE)), "{case}");
        }
        println!("AC4 reCAPTCHA cases:\n  {}", rows.join("\n  "));
        assert_eq!(sign_in(Some("good-2"))["OUTCOME"], "OK");
        // Not asked for an empty token or a replay: 4 refusals + 2 goods reached Google.
        assert_eq!(*http.0.lock().unwrap(), 6);
    }
}
