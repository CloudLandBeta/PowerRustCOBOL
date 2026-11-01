// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-009 AC3: the identity backend does not tell an unknown user from a wrong password —
//! not by the answer, and not by the clock — and a reset answers the same for an address that
//! has an account and one that has none.
//!
//! The provider here is slow where a real one is: an unknown name is refused at once, a wrong
//! password after the password hash (40 ms), a success after 80 ms. The service pads every guarded
//! answer to the median of the successes, never under 250 ms.

use std::time::{Duration, Instant};

use cobolt_server::identity::ops::{IdentityService, OpContext, ServiceConfig};
use cobolt_server::identity::seal::Sealer;
use cobolt_server::identity::{Capabilities, ChallengeResponse, Flow, IdentityProvider, IdpError, SignIn, SignUp, SignUpRequest, Tokens};
use serde_json::{json, Value};

struct SlowProvider;

impl IdentityProvider for SlowProvider {
    fn name(&self) -> &'static str {
        "slow"
    }
    fn flow(&self) -> Flow {
        Flow::InApp
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities { sign_in: true, reset: true, ..Default::default() }
    }
    fn sign_in(&self, u: &str, p: &str) -> Result<SignIn, IdpError> {
        if u != "ana" {
            return Err(IdpError::Rejected);
        }
        std::thread::sleep(Duration::from_millis(40));
        if p != "right-password" {
            return Err(IdpError::Rejected);
        }
        std::thread::sleep(Duration::from_millis(40));
        Ok(SignIn::Tokens(Tokens { access: "at".into(), ..Default::default() }))
    }
    fn respond_to_challenge(&self, _: &ChallengeResponse) -> Result<SignIn, IdpError> {
        Err(IdpError::Rejected)
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
    fn forgot_password(&self, u: &str) -> Result<Option<String>, IdpError> {
        if u == "ana" {
            std::thread::sleep(Duration::from_millis(30));
            Ok(None)
        } else {
            Err(IdpError::Rejected)
        }
    }
    fn confirm_forgot_password(&self, _: &str, _: &str, _: &str, _: &str) -> Result<(), IdpError> {
        Err(IdpError::Rejected)
    }
    fn refresh(&self, _: &str, _: &str) -> Result<Tokens, IdpError> {
        Err(IdpError::Rejected)
    }
    fn sign_out(&self, _: &str, _: Option<&str>) -> Result<(), IdpError> {
        Ok(())
    }
    fn start_hosted(&self, _: &str, _: &str, _: &str, _: &str) -> Result<String, IdpError> {
        Err(IdpError::Unsupported)
    }
    fn finish_hosted(&self, _: &str, _: &str, _: &str) -> Result<Tokens, IdpError> {
        Err(IdpError::Unsupported)
    }
}

fn service() -> IdentityService {
    // The default floor (250 ms); a limit high enough for the 100 attempts of this test.
    let cfg = ServiceConfig { attempts_per_minute: 10_000, ..ServiceConfig::default() };
    IdentityService::new(Box::new(SlowProvider), None, Sealer::new(&[3; 32]), cfg)
}

fn timed(s: &IdentityService, op: &str, req: Value) -> (String, Duration) {
    let t = Instant::now();
    let body = s.run(op, &req.to_string(), &OpContext { ip: "192.0.2.10".into() }).expect("an answer");
    (body, t.elapsed())
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

#[test]
fn an_unknown_user_and_a_wrong_password_look_the_same_in_body_and_time() {
    let s = service();
    for _ in 0..5 {
        let (ok, _) = timed(&s, "SIGN-IN", json!({"USERNAME": "ana", "PASSWORD": "right-password"}));
        assert!(ok.contains("\"OUTCOME\":\"OK\""), "{ok}");
    }
    let (mut unknown, mut wrong) = (Vec::new(), Vec::new());
    let (mut unknown_body, mut wrong_body) = (String::new(), String::new());
    // Interleaved, so a slow moment of the machine lands on both series alike.
    for i in 0..50 {
        let (b, t) = timed(&s, "SIGN-IN", json!({"USERNAME": format!("nobody-{i}"), "PASSWORD": "x"}));
        unknown.push(t);
        unknown_body = b;
        let (b, t) = timed(&s, "SIGN-IN", json!({"USERNAME": "ana", "PASSWORD": format!("wrong-{i}")}));
        wrong.push(t);
        wrong_body = b;
    }
    assert_eq!(unknown_body, wrong_body, "the bodies are identical");
    let (mu, mw) = (median(unknown), median(wrong));
    let diff = mu.abs_diff(mw);
    println!("AC3 medians over 50 attempts each: unknown user {mu:?}, wrong password {mw:?}, difference {diff:?} (pad target {:?})", s.pad_target());
    assert!(diff < Duration::from_millis(50), "the difference is {diff:?}");
    assert!(mu >= Duration::from_millis(250) && mw >= Duration::from_millis(250), "never under the floor");
}

#[test]
fn a_reset_for_an_unknown_address_is_the_reset_for_a_known_one() {
    let s = service();
    let strip = |b: &str| {
        let mut v: Value = serde_json::from_str(b).unwrap();
        // The sealed continuation is random by construction (its nonce); it exists in both.
        assert!(!v["SESSION"].as_str().unwrap().is_empty());
        v["SESSION"] = json!("<sealed>");
        v
    };
    let (known, tk) = timed(&s, "FORGOT", json!({"USERNAME": "ana"}));
    // A name of the same length: the sealed continuation's size follows the name the caller typed,
    // never whether an account has it.
    let (unknown, tu) = timed(&s, "FORGOT", json!({"USERNAME": "bob"}));
    println!("AC3 reset: known {tk:?}, unknown {tu:?}");
    assert_eq!(strip(&known), strip(&unknown));
    assert_eq!(known.len(), unknown.len(), "the same length, sealed session included");
    assert!(tk.abs_diff(tu) < Duration::from_millis(50));
}
