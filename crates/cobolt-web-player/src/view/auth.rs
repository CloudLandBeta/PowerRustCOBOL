// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Authentication control's screens** (WASM-009 R1–R3, WASM-004 class C).
//!
//! The control shows one screen at a time — sign in, create account, confirm, forgot password,
//! choose a new password, a verification code, and, once signed in, who is signed in and a sign-out
//! action — built from the machine's state in [`crate::auth`]. In the hosted flow (R8b) the sign-in
//! screen has the one "continue" action and the provider's pages do the rest.
//!
//! What a password manager and an assistive technology need is here, as data:
//!
//! * every field is a native `<input>` inside a `<label>` (its name is the label's text), with the
//!   `autocomplete` token that says what it is (`username`, `current-password`, `new-password`,
//!   `one-time-code`, `email`) — R2;
//! * the status line is a live region that exists before it has anything to say, so a failure is
//!   announced when it appears;
//! * a button that is not the submit action is `type="button"`, so Enter in a field submits the form
//!   and nothing else.
//!
//! A new `<form>` is made after every answer from the backend (its key carries the attempt count), so
//! a rejected password field is empty again; the user name is offered back as the field's default.
//! Nothing here holds a password: the browser layer reads the fields at the moment of the submit and
//! the host passes them to the machine, which forgets them when the call is made.

use super::controls::root;
use super::{VNode, ViewCtx};
use crate::auth::{AuthUi, Flow, Labels, Screen};

/// The control as an element.
pub fn view(cx: &ViewCtx, i: usize) -> VNode {
    let n = root(cx, i, "div").class("pr-auth").attr("role", "group");
    let Some(ui) = cx.tree.auth(i) else {
        return n.attr("aria-label", "Sign in");
    };
    let l = Labels::read(&|p| cx.tree.get(i, p));
    let m = &ui.machine;
    let title = match m.screen {
        Screen::SignIn | Screen::SignedIn => l.sign_in.clone(),
        Screen::SignUp => l.sign_up.clone(),
        Screen::Confirm => l.confirm.clone(),
        Screen::Forgot => l.forgot.clone(),
        Screen::Reset => l.reset.clone(),
        Screen::Mfa => l.mfa.clone(),
    };
    let mut n = n
        .attr("aria-label", title.clone())
        .attr("data-auth-screen", m.screen.name())
        .attr("data-auth-state", m.state_name())
        .attr("data-auth-flow", if m.cfg.flow == Flow::Hosted { "hosted" } else { "in-app" });
    if m.busy {
        n = n.attr("aria-busy", "true");
    }
    n.child(screen(ui, &l, &title))
}

fn screen(ui: &AuthUi, l: &Labels, title: &str) -> VNode {
    let m = &ui.machine;
    let status = VNode::new("p").key("status").class("pr-auth-message").attr("role", "alert").text(ui.message.clone());
    let form_key = format!("form-{}-{}", m.screen.name(), ui.attempt);
    let hint = ui.user_hint.as_str();
    let busy = m.busy;
    let submit = |label: &str| {
        let mut b = VNode::new("button").key("submit").class("pr-auth-primary").attr("type", "submit").text(label.to_owned());
        if busy {
            b = b.flag("disabled", true).attr("title", l.busy.clone());
        }
        b
    };
    let heading = || VNode::new("h3").key("title").class("pr-auth-title").text(title.to_owned());
    // No `novalidate`: the browser's own `required`/`type=email` checks run first, in the user's language.
    let form = |key: String| VNode::new("form").key("form").attr("data-form-key", key).on("submit");

    match m.screen {
        Screen::SignedIn => {
            let mut line = format!("{} {}", l.signed_in_as, m.user_name).trim().to_owned();
            if !m.email.is_empty() {
                line.push_str(&format!(" ({})", m.email));
            }
            VNode::new("div")
                .key("signed-in")
                .class("pr-auth-signed-in")
                .child(VNode::new("p").key("who").class("pr-auth-who").text(line))
                .child(status)
                .child(button("signout", "signout", &l.sign_out))
        }
        Screen::SignIn if m.cfg.flow == Flow::Hosted => {
            let mut go = VNode::new("button").key("hosted").class("pr-auth-primary").attr("type", "button").attr("data-id", "hosted").on("click").text(l.hosted.clone());
            if busy {
                go = go.flag("disabled", true);
            }
            form(form_key).child(heading()).child(status).child(go)
        }
        Screen::SignIn => {
            let mut nav = Vec::new();
            if m.cfg.sign_up && m.cfg.flow == Flow::InApp {
                nav.push(button("nav-signup", "show:SignUp", &l.create_account));
            }
            if m.cfg.forgot && m.cfg.flow == Flow::InApp {
                nav.push(button("nav-forgot", "show:Forgot", &l.forgot_link));
            }
            form(form_key)
                .child(heading())
                .child(field("user", &l.user, "text", "username", hint))
                .child(field("password", &l.password, "password", "current-password", ""))
                .child(status)
                .child(submit(&l.sign_in))
                .child(nav_row(nav))
        }
        Screen::SignUp => {
            let mut nav = vec![button("nav-signin", "show:SignIn", &l.back)];
            if m.cfg.confirm {
                nav.push(button("nav-confirm", "show:Confirm", &l.confirm));
            }
            form(form_key)
                .child(heading())
                .child(field("user", &l.user, "text", "username", hint))
                .child(field("email", &l.email, "email", "email", ""))
                .child(field("password", &l.password, "password", "new-password", ""))
                .child(status)
                .child(submit(&l.sign_up))
                .child(nav_row(nav))
        }
        Screen::Confirm => form(form_key)
            .child(heading())
            .child(field("user", &l.user, "text", "username", hint))
            .child(field("code", &l.code, "text", "one-time-code", ""))
            .child(status)
            .child(submit(&l.confirm))
            .child(nav_row(vec![button("nav-signin", "show:SignIn", &l.back)])),
        Screen::Forgot => form(form_key)
            .child(heading())
            .child(field("user", &l.user, "text", "username", hint))
            .child(status)
            .child(submit(&l.forgot))
            .child(nav_row(vec![button("nav-signin", "show:SignIn", &l.back)])),
        Screen::Reset => form(form_key)
            .child(heading())
            .child(field("user", &l.user, "text", "username", hint))
            .child(field("code", &l.code, "text", "one-time-code", ""))
            .child(field("new_password", &l.new_password, "password", "new-password", ""))
            .child(status)
            .child(submit(&l.reset))
            .child(nav_row(vec![button("nav-signin", "show:SignIn", &l.back)])),
        Screen::Mfa => form(form_key)
            .child(heading())
            .child(field("code", &l.code, "text", "one-time-code", ""))
            .child(status)
            .child(submit(&l.mfa))
            .child(nav_row(vec![button("nav-signin", "show:SignIn", &l.back)])),
    }
}

/// A labelled input. `hint` is the default value (the attribute, not the live value), so what the
/// user has typed is never overwritten by a redraw.
fn field(name: &'static str, label: &str, kind: &'static str, autocomplete: &'static str, hint: &str) -> VNode {
    let mut input = VNode::new("input")
        .key("input")
        .attr("name", name)
        .attr("type", kind)
        .attr("autocomplete", autocomplete)
        .attr("autocapitalize", "none")
        .attr("spellcheck", "false")
        .attr("required", "");
    if !hint.is_empty() {
        input = input.attr("value", hint);
    }
    if autocomplete == "one-time-code" {
        input = input.attr("inputmode", "numeric");
    }
    VNode::new("label").key(format!("f-{name}")).child(VNode::new("span").key("t").text(label.to_owned())).child(input)
}

fn button(key: &str, id: &str, label: &str) -> VNode {
    VNode::new("button").key(key.to_owned()).attr("type", "button").attr("data-id", id).on("click").text(label.to_owned())
}

fn nav_row(buttons: Vec<VNode>) -> VNode {
    VNode::new("nav").key("nav").children(buttons)
}

#[cfg(test)]
mod tests {
    use super::super::Empty;
    use super::*;
    use crate::auth::{Action, Flow};
    use crate::tree::testing::tree_with;

    fn view_of(props: &[(&str, &str)], flow: Flow) -> VNode {
        let (mut t, i) = tree_with("Authentication", "AUTH-1", props);
        t.init_auth(flow, "", &crate::auth::Session::default());
        let e = Empty::default();
        let cx = e.cx(&t);
        view(&cx, i)
    }

    fn with_screen(screen: Screen, props: &[(&str, &str)]) -> VNode {
        let (mut t, i) = tree_with("Authentication", "AUTH-1", props);
        t.init_auth(Flow::InApp, "", &crate::auth::Session::default());
        t.auth_mut(i).unwrap().machine.screen = screen;
        let e = Empty::default();
        let cx = e.cx(&t);
        view(&cx, i)
    }

    /// `(name, type, autocomplete)` of every input, in order.
    fn inputs(n: &VNode) -> Vec<(String, String, String)> {
        let mut found = Vec::new();
        n.find_all(&|x| x.tag == "input", &mut found);
        found.iter().map(|f| (f.attr_of("name").unwrap().to_owned(), f.attr_of("type").unwrap().to_owned(), f.attr_of("autocomplete").unwrap().to_owned())).collect()
    }

    #[test]
    fn the_sign_in_screen_has_the_fields_a_password_manager_recognises() {
        let n = view_of(&[], Flow::InApp);
        assert!(n.has_class("pr-auth") && n.has_class("pr-control"));
        assert_eq!(n.attr_of("role"), Some("group"));
        assert_eq!(n.attr_of("data-auth-screen"), Some("SignIn"));
        assert_eq!(n.attr_of("data-auth-state"), Some("SignedOut"));
        assert_eq!(
            inputs(&n),
            vec![("user".into(), "text".into(), "username".into()), ("password".into(), "password".into(), "current-password".into())],
            "R2: autocomplete says what each field is"
        );
        let form = n.find_tag("form").unwrap();
        assert!(form.on.contains(&"submit") && form.attr_of("novalidate").is_none(), "the browser validates the required fields in the user's language");
        assert!(n.find(&|x| x.tag == "input").unwrap().attr_of("required").is_some());
        // The only submit button is the primary action; the others are plain buttons (Enter submits, links navigate).
        let mut buttons = Vec::new();
        n.find_all(&|x| x.tag == "button", &mut buttons);
        assert_eq!(buttons.iter().filter(|b| b.attr_of("type") == Some("submit")).count(), 1);
        assert!(buttons.iter().filter(|b| b.attr_of("type") == Some("button")).all(|b| b.attr_of("data-id").is_some() && b.on.contains(&"click")));
        // The status line is a live region that is there before it has anything to say.
        let status = n.find(&|x| x.has_class("pr-auth-message")).unwrap();
        assert_eq!(status.attr_of("role"), Some("alert"));
        assert_eq!(status.all_text(), "");
    }

    #[test]
    fn every_field_is_inside_a_label_so_it_has_an_accessible_name() {
        for s in [Screen::SignIn, Screen::SignUp, Screen::Confirm, Screen::Forgot, Screen::Reset, Screen::Mfa] {
            let n = with_screen(s, &[]);
            let mut labels = Vec::new();
            n.find_all(&|x| x.tag == "label", &mut labels);
            let mut ins = Vec::new();
            n.find_all(&|x| x.tag == "input", &mut ins);
            assert!(!ins.is_empty(), "{s:?}");
            assert_eq!(labels.len(), ins.len(), "{s:?}: one label per field");
            for l in labels {
                assert!(l.find_tag("input").is_some() && !l.all_text().is_empty(), "{s:?}");
            }
        }
    }

    #[test]
    fn each_screen_has_the_fields_and_tokens_it_needs() {
        let sign_up = inputs(&with_screen(Screen::SignUp, &[]));
        assert_eq!(sign_up.iter().map(|x| x.2.as_str()).collect::<Vec<_>>(), ["username", "email", "new-password"]);
        let reset = inputs(&with_screen(Screen::Reset, &[]));
        assert_eq!(reset.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["user", "code", "new_password"]);
        assert_eq!(reset[1].2, "one-time-code");
        assert_eq!(reset[2].2, "new-password");
        let mfa = inputs(&with_screen(Screen::Mfa, &[]));
        assert_eq!(mfa, vec![("code".into(), "text".into(), "one-time-code".into())]);
        let forgot = inputs(&with_screen(Screen::Forgot, &[]));
        assert_eq!(forgot.len(), 1);
    }

    #[test]
    fn the_developer_chooses_which_screens_are_reachable_and_their_words() {
        let n = view_of(&[("ShowSignUp", "0"), ("ShowForgot", "0"), ("SignInCaption", "Entrar"), ("UserLabel", "Usuário")], Flow::InApp);
        assert!(n.find(&|x| x.attr_of("data-id") == Some("show:SignUp")).is_none(), "R1: no sign-up link when the screen is off");
        assert!(n.find(&|x| x.attr_of("data-id") == Some("show:Forgot")).is_none());
        assert!(n.all_text().contains("Entrar") && n.all_text().contains("Usuário"), "R1: the text is the developer's");
        let n = view_of(&[], Flow::InApp);
        assert!(n.find(&|x| x.attr_of("data-id") == Some("show:SignUp")).is_some());
        assert!(n.find(&|x| x.attr_of("data-id") == Some("show:Forgot")).is_some());
    }

    #[test]
    fn the_hosted_flow_offers_one_action_and_no_fields() {
        let n = view_of(&[], Flow::Hosted);
        assert_eq!(n.attr_of("data-auth-flow"), Some("hosted"));
        assert!(n.find_tag("input").is_none(), "R8b: the provider's pages take the credentials");
        let b = n.find(&|x| x.attr_of("data-id") == Some("hosted")).unwrap();
        assert!(b.on.contains(&"click"));
        let n = view_of(&[("Mode", "InApp")], Flow::Hosted);
        assert!(n.find_tag("input").is_some(), "the control's own Mode outranks the page's default");
    }

    #[test]
    fn the_signed_in_screen_names_the_user_and_offers_sign_out_and_never_a_token() {
        let (mut t, i) = tree_with("Authentication", "AUTH-1", &[]);
        t.init_auth(Flow::InApp, "", &crate::auth::Session::default());
        let token = crate::auth::MockBackend::token(&serde_json::json!({"preferred_username": "ana", "email": "ana@example.com", "exp": 4102444800u64}));
        {
            let ui = t.auth_mut(i).unwrap();
            ui.machine.session.sign_in(&token, Some(3600.0), 0.0);
            let s = ui.machine.session.clone();
            ui.machine.adopt(&s);
        }
        let e = Empty::default();
        let cx = e.cx(&t);
        let n = view(&cx, i);
        assert_eq!(n.attr_of("data-auth-state"), Some("SignedIn"));
        let text = n.all_text();
        assert!(text.contains("Signed in as ana") && text.contains("ana@example.com"), "{text}");
        assert!(n.find(&|x| x.attr_of("data-id") == Some("signout")).is_some());
        assert!(!n.describe().contains(&token[..20]), "R6: a token is never in the page");
        assert!(n.find_tag("input").is_none());
    }

    #[test]
    fn a_failure_is_shown_in_the_live_region_and_the_form_is_made_again_with_the_user_name_kept() {
        let (mut t, i) = tree_with("Authentication", "AUTH-1", &[]);
        t.init_auth(Flow::InApp, "", &crate::auth::Session::default());
        let before = {
            let e = Empty::default();
            let cx = e.cx(&t);
            view(&cx, i)
        };
        {
            let ui = t.auth_mut(i).unwrap();
            ui.message = "The user name or the password is not right.".into();
            ui.attempt += 1;
            ui.user_hint = "ana".into();
        }
        let e = Empty::default();
        let cx = e.cx(&t);
        let after = view(&cx, i);
        assert_eq!(after.find(&|x| x.has_class("pr-auth-message")).unwrap().all_text(), "The user name or the password is not right.");
        let f0 = before.find_tag("form").unwrap().attr_of("data-form-key").unwrap().to_owned();
        let f1 = after.find_tag("form").unwrap().attr_of("data-form-key").unwrap().to_owned();
        assert_ne!(f0, f1, "a new form after each answer");
        let user = after.find(&|x| x.attr_of("name") == Some("user")).unwrap();
        assert_eq!(user.attr_of("value"), Some("ana"));
        assert!(user.prop_of("value").is_none(), "the attribute, never the live value: typing is not overwritten");
        let pw = after.find(&|x| x.attr_of("name") == Some("password")).unwrap();
        assert!(pw.attr_of("value").is_none() && pw.prop_of("value").is_none(), "a password is never put back");
    }

    #[test]
    fn a_busy_machine_disables_the_submit_and_says_so_to_assistive_technology() {
        let (mut t, i) = tree_with("Authentication", "AUTH-1", &[]);
        t.init_auth(Flow::InApp, "", &crate::auth::Session::default());
        t.auth_mut(i).unwrap().machine.act(Action::SignIn { user: "a".into(), password: "b".into() });
        let e = Empty::default();
        let cx = e.cx(&t);
        let n = view(&cx, i);
        assert_eq!(n.attr_of("aria-busy"), Some("true"));
        assert!(n.find(&|x| x.attr_of("type") == Some("submit")).unwrap().attr_of("disabled").is_some());
    }

    #[test]
    fn the_program_reads_state_user_and_email_as_properties() {
        let (mut t, i) = tree_with("Authentication", "AUTH-1", &[]);
        t.init_auth(Flow::InApp, "", &crate::auth::Session::default());
        assert_eq!((t.get(i, "State").as_str(), t.get(i, "UserName").as_str(), t.get(i, "Mode").as_str()), ("SignedOut", "", "InApp"));
        let token = crate::auth::MockBackend::token(&serde_json::json!({"preferred_username": "ana", "email": "a@x.y", "exp": 4102444800u64}));
        let mut s = crate::auth::Session::default();
        s.sign_in(&token, Some(60.0), 0.0);
        t.auth_mut(i).unwrap().machine.adopt(&s);
        t.sync_auth_props(i);
        assert_eq!((t.get(i, "State").as_str(), t.get(i, "UserName").as_str(), t.get(i, "Email").as_str()), ("SignedIn", "ana", "a@x.y"));
    }
}
