// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! reCAPTCHA v3 verification, on the backend (WASM-009 R4, D9).
//!
//! The browser obtains a token for an *action* (`sign_in`, `sign_up`, `forgot_password`); the
//! backend asks Google (`siteverify`) and **checks four things, not one**: `success`; the
//! **score** against the minimum (0.5 by default); the **action** this operation expects; and
//! the **hostname**, when the deployment lists any. A token is verifiable **once**: the backend
//! remembers the tokens it accepted for two minutes, which is as long as one is valid.
//!
//! **The fail-open trap.** Without billing, over the free quota Google answers `HTTP 200`,
//! `success: true`, score 0.9 and — only in a message — "Over free quota". That answer is a
//! **failure** here, or a quota ending would wave every bot through.
//!
//! The secret appears in no log and no error. `CreateAssessment` (Enterprise) is not implemented;
//! [`CaptchaVerifier`] is its seam.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::client::{Http, HttpRequest};
use crate::config::{ResolvedRecaptcha, Secret};

/// What a verification decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Human,
    /// Not accepted. The reason is for the operator's log, never the user's screen.
    Refused(String),
    /// Google could not be asked. The service fails closed.
    Unavailable(String),
}

pub trait CaptchaVerifier: Send + Sync {
    fn verify(&self, token: &str, expected_action: &str, remote_ip: Option<&str>) -> Verdict;
}

pub struct Recaptcha {
    http: Arc<dyn Http>,
    secret: Secret,
    min_score: f64,
    hostnames: Vec<String>,
    endpoint: String,
    /// Tokens accepted in the last two minutes, so none is accepted twice.
    seen: Mutex<HashMap<String, Instant>>,
}

pub const DEFAULT_ENDPOINT: &str = "https://www.google.com/recaptcha/api/siteverify";
const VALID_FOR: Duration = Duration::from_secs(120);

impl Recaptcha {
    pub fn new(http: Arc<dyn Http>, cfg: &ResolvedRecaptcha) -> Self {
        Recaptcha {
            http,
            secret: cfg.secret.clone(),
            min_score: cfg.min_score,
            hostnames: cfg.hostnames.clone(),
            endpoint: cfg.endpoint.clone().unwrap_or_else(|| DEFAULT_ENDPOINT.to_string()),
            seen: Mutex::new(HashMap::new()),
        }
    }

    fn remember(&self, token: &str) -> bool {
        let Ok(mut seen) = self.seen.lock() else { return false };
        let now = Instant::now();
        seen.retain(|_, at| now.duration_since(*at) < VALID_FOR);
        if seen.contains_key(token) {
            return false;
        }
        seen.insert(token.to_string(), now);
        true
    }

    fn seen_before(&self, token: &str) -> bool {
        self.seen.lock().map(|s| s.get(token).is_some_and(|at| at.elapsed() < VALID_FOR)).unwrap_or(true)
    }
}

impl CaptchaVerifier for Recaptcha {
    fn verify(&self, token: &str, expected_action: &str, remote_ip: Option<&str>) -> Verdict {
        if token.is_empty() || token.len() > 4096 {
            return Verdict::Refused("no usable token".into());
        }
        // A replay is refused before Google is asked again (Google would refuse it too, at a cost).
        if self.seen_before(token) {
            return Verdict::Refused("the token was used already".into());
        }
        let mut fields = vec![("secret", self.secret.expose()), ("response", token)];
        if let Some(ip) = remote_ip.filter(|ip| !ip.is_empty()) {
            fields.push(("remoteip", ip));
        }
        let resp = match self.http.send(&HttpRequest::post_form(&self.endpoint, &[], &fields)) {
            Ok(r) => r,
            Err(e) => return Verdict::Unavailable(e),
        };
        if resp.status != 200 {
            return Verdict::Unavailable(format!("siteverify answered {}", resp.status));
        }
        let Some(v) = resp.json() else { return Verdict::Unavailable("siteverify answered something that is not JSON".into()) };
        if v.to_string().to_ascii_lowercase().contains("over free quota") {
            return Verdict::Refused("recaptcha-quota: the free quota is spent; a pass would be meaningless".into());
        }
        if v.get("success").and_then(|s| s.as_bool()) != Some(true) {
            let codes = v.get("error-codes").map(|c| c.to_string()).unwrap_or_default();
            return Verdict::Refused(format!("Google did not accept the token {codes}"));
        }
        let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
        if score < self.min_score {
            return Verdict::Refused(format!("score {score:.2} is under the minimum {:.2}", self.min_score));
        }
        let action = v.get("action").and_then(|s| s.as_str()).unwrap_or("");
        if action != expected_action {
            return Verdict::Refused(format!("the token was issued for the action \"{action}\", this operation expects \"{expected_action}\""));
        }
        if !self.hostnames.is_empty() {
            let host = v.get("hostname").and_then(|s| s.as_str()).unwrap_or("");
            if !self.hostnames.iter().any(|h| h.eq_ignore_ascii_case(host)) {
                return Verdict::Refused(format!("the token was issued for the host \"{host}\""));
            }
        }
        if !self.remember(token) {
            return Verdict::Refused("the token was used already".into());
        }
        Verdict::Human
    }
}
