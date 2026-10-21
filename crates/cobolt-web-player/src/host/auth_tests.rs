// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The host's Authentication control (WASM-009): the machine of `crate::auth` driven through the
//! host the way the page drives it — a user's act, a backend answer, a reCAPTCHA token, the clock —
//! with every effect the host asks for read back. A child module of `host` so it may reach the
//! host's private state (the call tables, `create_tree`, `destroy`).

use super::*;
use crate::auth::{Action, AuthResult, BackendCall, MockBackend, Screen, Texts};
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

const EVENTS: [&str; 6] = ["onSignedIn", "onSignedOut", "onError", "onSignUpCompleted", "onMfaRequired", "onPasswordResetRequested"];

/// A program with one form holding one Authentication control; handlers bound to `bound`.
fn host(bound: &[&str], props: &str) -> Host {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Demo" width="640" height="480" index-base="1">
  <Control id="AUTH-1" type="Authentication" x="10" y="10" w="300" h="300">{props}</Control>
</Form>"#
    );
    let form = load_form_from_str(&xml).expect("the form loads");
    let events: Vec<String> = EVENTS.iter().map(|s| s.to_string()).collect();
    let handlers = bound.iter().enumerate().map(|(n, e)| HandlerEntry { ctrl: 3, event: events.iter().position(|x| x == e).unwrap() as u32, entry: n as u32 }).collect();
    let manifest = Manifest {
        abi: 1,
        generator: "test".into(),
        project: "demo".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec![],
        events,
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 2,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "AUTH-1".into(), kind: "Authentication".into() }],
            handlers,
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.add_form(form).unwrap();
    h
}

fn open(h: &mut Host, inst: u32) {
    h.create_tree(inst, 0, None, Vec::new()).expect("the tree builds");
}

/// The one backend call the host asked for since the last look.
fn call_of(h: &mut Host) -> (u32, BackendCall) {
    let found: Vec<_> = h.effects.drain(..).filter_map(|e| if let Effect::AuthCall { id, call } = e { Some((id, call)) } else { None }).collect();
    assert_eq!(found.len(), 1, "exactly one backend call: {found:?}");
    found.into_iter().next().unwrap()
}

fn signed_in_token() -> String {
    MockBackend::token(&serde_json::json!({"sub": "u-1", "preferred_username": "ana", "email": "ana@example.com", "exp": 4102444800u64}))
}

fn signed_in(token: String) -> AuthResult {
    AuthResult::SignedIn { access_token: token, expires_in_s: Some(3600.0) }
}

#[test]
fn a_sign_in_makes_one_backend_call_and_the_answer_signs_the_application_in() {
    let mut h = host(&["onSignedIn"], "");
    open(&mut h, 0);
    h.auth_act(0, 1, Action::SignIn { user: "ana".into(), password: "s3cret!".into() });
    let (id, call) = call_of(&mut h);
    assert!(crate::auth::wire::is_auth_id(id), "an id of the control's range, never the module's");
    assert_eq!(call, BackendCall::SignIn { user: "ana".into(), password: "s3cret!".into(), captcha: None });
    assert!(h.trees[&0].auth(1).unwrap().machine.busy);

    let token = signed_in_token();
    let epoch = h.auth_epoch;
    h.auth_answer(id, signed_in(token.clone()), 1_000.0);
    assert!(h.auth.is_signed_in(), "the application's session");
    assert_eq!(h.auth_epoch, epoch + 1, "the connection is to be given the new token");
    assert_eq!(h.stats.events_queued, 1, "onSignedIn is bound, so it was queued");
    let t = &h.trees[&0];
    assert_eq!((t.get(1, "State").as_str(), t.get(1, "UserName").as_str(), t.get(1, "Email").as_str()), ("SignedIn", "ana", "ana@example.com"));
    assert_eq!(h.invoke(0, "AUTH-1", "GetClaim", &["sub"]).unwrap().as_deref(), Some("u-1"));
    // R6: no token and no password in anything the host lets out.
    assert!(!format!("{:?}", h.effects).contains(&token[..20]));
    assert!(!h.snapshot().to_string().contains(&token[..20]));
    assert!(!h.snapshot().to_string().contains("s3cret!"), "a password is nowhere in the state");
}

#[test]
fn a_refused_sign_in_shows_the_one_generic_message_and_raises_on_error_only_when_bound() {
    for (bound, queued) in [(vec!["onError"], 1u64), (vec![], 0u64)] {
        let mut h = host(&bound, "");
        open(&mut h, 0);
        h.auth_act(0, 1, Action::SignIn { user: "ghost".into(), password: "x".into() });
        let (id, _) = call_of(&mut h);
        h.auth_answer(id, AuthResult::Failed { detail: "authentication/3".into() }, 0.0);
        let ui = h.trees[&0].auth(1).unwrap();
        assert!(ui.interacted, "the user acted, so the next screen takes the focus");
        assert_eq!(ui.message, Texts::default().sign_in_failed);
        assert_eq!((ui.attempt, ui.user_hint.as_str()), (1, "ghost"), "a new form, the user name offered back");
        assert!(!h.auth.is_signed_in());
        assert_eq!(h.stats.events_queued, queued);
    }
}

#[test]
fn recaptcha_is_obtained_before_the_request_and_a_missing_token_means_no_request() {
    let mut h = host(&[], r#"<Property name="RecaptchaSiteKey">site-key</Property>"#);
    open(&mut h, 0);
    h.auth_act(0, 1, Action::SignIn { user: "ana".into(), password: "p".into() });
    let Some(Effect::AuthCaptcha { id, site_key, action }) = h.effects.drain(..).next() else { panic!("a token is asked for first") };
    assert_eq!((site_key.as_str(), action), ("site-key", "sign_in"));
    h.auth_captcha(id, Some("tok".into()));
    let (_, call) = call_of(&mut h);
    assert_eq!(call, BackendCall::SignIn { user: "ana".into(), password: "p".into(), captcha: Some("tok".into()) });

    let mut h = host(&[], r#"<Property name="RecaptchaSiteKey">site-key</Property>"#);
    open(&mut h, 0);
    h.auth_act(0, 1, Action::Forgot { user: "ana".into() });
    let Some(Effect::AuthCaptcha { id, .. }) = h.effects.drain(..).next() else { panic!() };
    h.auth_captcha(id, None);
    assert!(h.effects.iter().all(|e| !matches!(e, Effect::AuthCall { .. })), "no token, no request");
    assert!(!h.trees[&0].auth(1).unwrap().message.is_empty());
}

#[test]
fn the_page_settings_fill_what_the_control_leaves_open() {
    let mut h = host(&[], "");
    h.set_auth_defaults(crate::auth::Flow::Hosted, "page-key");
    open(&mut h, 0);
    let cfg = &h.trees[&0].auth(1).unwrap().machine.cfg;
    assert_eq!((cfg.flow, cfg.recaptcha_site_key.as_str()), (crate::auth::Flow::Hosted, "page-key"));
    assert_eq!(h.trees[&0].get(1, "Mode"), "Hosted");
}

#[test]
fn the_hosted_flow_navigates_to_an_https_address_and_refuses_anything_else() {
    let mut h = host(&[], r#"<Property name="Mode">Hosted</Property>"#);
    open(&mut h, 0);
    h.auth_act(0, 1, Action::Hosted);
    let (id, call) = call_of(&mut h);
    assert_eq!(call, BackendCall::HostedStart);
    h.auth_answer(id, AuthResult::Hosted { redirect_url: "https://idp.example/authorize?state=s".into() }, 0.0);
    assert!(h.effects.iter().any(|e| matches!(e, Effect::Navigate { url, target } if url.starts_with("https://idp.example/") && target == "_self")));

    h.effects.clear();
    h.auth_act(0, 1, Action::Hosted);
    let (id, _) = call_of(&mut h);
    h.auth_answer(id, AuthResult::Hosted { redirect_url: "javascript:alert(1)".into() }, 0.0);
    assert!(h.effects.iter().all(|e| !matches!(e, Effect::Navigate { .. })), "an address that came over the network is checked");
    assert_eq!(h.trees[&0].auth(1).unwrap().message, Texts::default().generic_error);
}

#[test]
fn coming_back_from_the_provider_with_a_code_signs_in() {
    let mut h = host(&["onSignedIn"], r#"<Property name="Mode">Hosted</Property>"#);
    assert!(!h.auth_hosted_return("c0de", "s"), "no control yet: the page keeps the code until a form with one opens");
    open(&mut h, 0);
    assert!(h.auth_hosted_return("c0de", "s"));
    let (id, call) = call_of(&mut h);
    assert_eq!(call, BackendCall::HostedFinish { code: "c0de".into(), state: "s".into() });
    h.auth_answer(id, signed_in(signed_in_token()), 0.0);
    assert!(h.auth.is_signed_in());
    assert_eq!(h.stats.events_queued, 1);
}

#[test]
fn a_form_opened_later_and_a_second_control_follow_the_one_session() {
    let mut h = host(&[], "");
    open(&mut h, 0);
    h.auth_act(0, 1, Action::SignIn { user: "ana".into(), password: "p".into() });
    let (id, _) = call_of(&mut h);
    h.auth_answer(id, signed_in(signed_in_token()), 0.0);
    // A second instance opened while signed in starts signed in.
    open(&mut h, 1);
    assert_eq!(h.trees[&1].get(1, "State"), "SignedIn");
    assert_eq!(h.trees[&1].auth(1).unwrap().machine.screen, Screen::SignedIn);
    // Signing out in one is signing out in both.
    h.auth_act(1, 1, Action::SignOut);
    assert!(!h.auth.is_signed_in());
    assert_eq!(h.trees[&0].get(1, "State"), "SignedOut", "the first control followed");
    assert_eq!(h.trees[&0].auth(1).unwrap().machine.screen, Screen::SignIn);
}

#[test]
fn the_program_signs_out_and_refreshes_through_methods() {
    let mut h = host(&["onSignedOut"], "");
    open(&mut h, 0);
    h.auth_act(0, 1, Action::SignIn { user: "ana".into(), password: "p".into() });
    let (id, _) = call_of(&mut h);
    h.auth_answer(id, signed_in(signed_in_token()), 0.0);
    h.stats.events_queued = 0;

    h.invoke(0, "AUTH-1", "Refresh", &[]).unwrap();
    assert!(h.effects.iter().all(|e| !matches!(e, Effect::AuthCall { .. })), "nothing happens until the driver gives the machine its turn");
    h.trees.get_mut(&0).unwrap().auth_mut(1).unwrap().interacted = false;
    h.auth_flush();
    assert!(!h.trees[&0].auth(1).unwrap().interacted, "what the program asks for never moves the keyboard focus");
    let (id, call) = call_of(&mut h);
    assert_eq!(call, BackendCall::Refresh);
    h.auth_answer(id, signed_in(signed_in_token()), 10.0);
    assert_eq!(h.stats.events_queued, 0, "a silent refresh raises nothing");

    h.invoke(0, "AUTH-1", "SignOut", &[]).unwrap();
    h.auth_flush();
    assert!(!h.auth.is_signed_in(), "the token is forgotten at once, before the backend answers");
    assert_eq!(h.stats.events_queued, 1, "onSignedOut");
    let (_, call) = call_of(&mut h);
    assert_eq!(call, BackendCall::SignOut);
}

#[test]
fn the_session_refreshes_itself_before_the_token_expires() {
    let mut h = host(&[], "");
    open(&mut h, 0);
    h.auth_act(0, 1, Action::SignIn { user: "ana".into(), password: "p".into() });
    let (id, _) = call_of(&mut h);
    h.auth_answer(id, signed_in(signed_in_token()), 0.0);
    let due = h.auth_deadline().expect("a session has a deadline");
    assert_eq!(due, 3_600_000.0 - 60_000.0, "one minute before it expires");
    h.auth_tick(due - 1_000.0);
    assert!(h.effects.is_empty(), "too early");
    h.auth_tick(due + 1.0);
    let (id, call) = call_of(&mut h);
    assert_eq!(call, BackendCall::Refresh);
    // A refused refresh signs the user out.
    h.auth_answer(id, AuthResult::Failed { detail: "expired".into() }, due + 2.0);
    assert!(!h.auth.is_signed_in());
    assert_eq!(h.auth_deadline(), None);
}

#[test]
fn the_server_saying_the_token_expired_refreshes_it_now() {
    let mut h = host(&[], "");
    open(&mut h, 0);
    h.auth_refresh();
    assert!(h.effects.is_empty(), "nobody is signed in: nothing to refresh");
    h.auth_act(0, 1, Action::SignIn { user: "ana".into(), password: "p".into() });
    let (id, _) = call_of(&mut h);
    h.auth_answer(id, signed_in(signed_in_token()), 0.0);
    h.auth_refresh();
    let (_, call) = call_of(&mut h);
    assert_eq!(call, BackendCall::Refresh);
}

#[test]
fn an_answer_for_a_form_that_closed_is_dropped() {
    let mut h = host(&[], "");
    open(&mut h, 0);
    h.auth_act(0, 1, Action::SignIn { user: "ana".into(), password: "p".into() });
    let (id, _) = call_of(&mut h);
    h.destroy(0);
    h.auth_answer(id, signed_in(signed_in_token()), 0.0);
    assert!(!h.auth.is_signed_in(), "nowhere to show it, so nothing is signed in");
    assert!(h.auth_calls.is_empty());
}

#[test]
fn auth_call_ids_are_apart_from_the_modules_and_grow() {
    let mut h = host(&[], "");
    open(&mut h, 0);
    for _ in 0..3 {
        h.auth_act(0, 1, Action::Forgot { user: "u".into() });
        h.trees.get_mut(&0).unwrap().auth_mut(1).unwrap().machine.busy = false;
    }
    let ids: Vec<u32> = h.effects.iter().filter_map(|e| if let Effect::AuthCall { id, .. } = e { Some(*id) } else { None }).collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.iter().all(|i| crate::auth::wire::is_auth_id(*i)) && ids.windows(2).all(|w| w[0] < w[1]));
}
