// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Authentication** (WASM-009): the control's vocabulary, in one place.
//!
//! The control signs an application's users in, up and out, recovers a password and answers a
//! second factor. It shows its own screens (the in-app flow) or one button that goes to the
//! provider's page (the hosted flow, R8b). It works through the backend (class C): the identity
//! requests go to the backend's `/auth/*` routes, never to the provider from the page.
//!
//! The browser player draws and drives it (`cobolt-web-player` `auth.rs`, `view/auth.rs`); the
//! Designer paints its Sign in screen ([`crate::auth_texts`] holds the default texts of both). This
//! module is pure data — no egui, no I/O — so the player builds it for `wasm32`.

/// The control's events (plan §4). None carries data on the desktop; in the browser `onError`'s
/// argument is `class/code`, never a message (ABI §12.2).
pub const EVENTS: &[&str] = &[
    "onSignedIn",
    "onSignedOut",
    "onSignUpCompleted",
    "onPasswordResetRequested",
    "onMfaRequired",
    "onError",
    "onConsentRequired",
];

/// The values of `Mode` at design time. `Auto` is the provider's own flow (the default); at run
/// time the property reads the effective flow, `InApp` or `Hosted`.
pub const MODES: &[&str] = &["Auto", "InApp", "Hosted"];

/// The values of `RecaptchaConsent` (D10). `Implied` loads reCAPTCHA at the first protected action;
/// `Ask` raises `onConsentRequired` first and waits for `GrantConsent` or `DenyConsent`.
pub const CONSENTS: &[&str] = &["Implied", "Ask"];

/// The screens the developer chooses (R1); each is on unless set to false.
pub const SCREEN_FLAGS: &[&str] = &["ShowSignUp", "ShowConfirm", "ShowForgot", "ShowMfa"];

/// The properties whose words the end user reads: the 18 captions and labels and the 3 messages,
/// then the texts of the New password, MFA-kind, Choose and MFA set-up screens. Each is seeded
/// empty: empty means the default of [`crate::auth_texts`] in the application's language, and a
/// developer's value always wins (R1).
pub const TEXT_PROPS: &[&str] = &[
    "SignInCaption",
    "SignUpCaption",
    "ConfirmCaption",
    "ForgotCaption",
    "ResetCaption",
    "MfaCaption",
    "SignOutCaption",
    "HostedCaption",
    "UserLabel",
    "EmailLabel",
    "PasswordLabel",
    "NewPasswordLabel",
    "CodeLabel",
    "CreateAccountLink",
    "ForgotLink",
    "BackCaption",
    "SignedInAsText",
    "BusyText",
    "SignInFailedText",
    "ResetRequestedText",
    "ErrorText",
    "NewPasswordCaption",
    "RepeatPasswordLabel",
    "MfaTotpText",
    "MfaSmsText",
    "MfaEmailText",
    "MfaChooseCaption",
    "MfaTotpChoice",
    "MfaSmsChoice",
    "MfaEmailChoice",
    "MfaSetupCaption",
    "MfaSetupText",
    "SecretLabel",
    "PasswordsDifferText",
];

/// What `Control::new` seeds besides the texts: the flow, the screens, reCAPTCHA.
/// `RecaptchaSiteKey` is an override; its default is the backend's `[identity.recaptcha] site_key`.
pub const SEEDS: &[(&str, &str)] = &[("Mode", "Auto"), ("RecaptchaSiteKey", ""), ("RecaptchaConsent", "Implied")];

/// Read-only properties the control carries at run time only: who is signed in. `Mode` is
/// seeded and also rewritten at run time with the effective flow.
pub const RUNTIME_PROPS: &[&str] = &["State", "UserName", "Email"];

/// The control's methods (R3), as the closed vocabulary spells them (`cobolt-ast` `is_known_method`).
pub const METHODS: &[&str] = &["SIGNOUT", "REFRESH", "GETCLAIM", "GRANTCONSENT", "DENYCONSENT"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Control, ControlType};

    #[test]
    fn every_seed_and_text_is_a_property_of_a_new_control_and_none_repeats() {
        let c = Control::new("AUTH-1", ControlType::Authentication, 0, 0);
        let mut all: Vec<&str> = SEEDS.iter().map(|(k, _)| *k).chain(SCREEN_FLAGS.iter().copied()).chain(TEXT_PROPS.iter().copied()).collect();
        for k in &all {
            assert!(c.get_prop(k).is_some(), "{k} is not seeded");
        }
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "a property is listed twice");
        println!("Authentication: {n} seeded properties ({} texts), {} events, {} methods", TEXT_PROPS.len(), EVENTS.len(), METHODS.len());
        assert_eq!(ControlType::Authentication.supported_events(), EVENTS);
        for m in METHODS {
            assert!(cobolt_ast::methods::is_known_method(m), "{m} is not in the closed vocabulary");
        }
    }

    /// AC9 — every property survives save and load, and a second save is identical.
    #[test]
    fn the_control_round_trips_with_every_property() {
        use crate::model::{Form, PropValue};
        use crate::xml::{form_to_string, load_form_from_str};
        let mut c = Control::new("AUTH-1", ControlType::Authentication, 10, 20);
        c.set_prop("Mode", PropValue::String("Hosted".into()));
        c.set_prop("ShowMfa", PropValue::Bool(false));
        c.set_prop("RecaptchaConsent", PropValue::String("Ask".into()));
        c.set_prop("PasswordLabel", PropValue::String("Senha".into()));
        c.set_prop("SignInFailedText", PropValue::String("Usuário ou senha incorretos.".into()));
        let mut f = Form::new("LOGIN", "Login", 640, 480);
        f.controls.push(c.clone());
        let text = form_to_string(&f).unwrap();
        assert!(text.contains(r#"type="Authentication""#), "{text}");
        let back = load_form_from_str(&text).unwrap();
        let l = back.find_control("AUTH-1").unwrap();
        assert_eq!(l.control_type, ControlType::Authentication);
        let mut n = 0;
        for (k, v) in &c.properties {
            assert_eq!(l.get_prop(k).map(|p| p.as_str().to_string()), Some(v.as_str().to_string()), "{k}");
            n += 1;
        }
        assert_eq!(form_to_string(&back).unwrap(), text, "stable on a second save");
        println!("round-trip: {n} properties compared");
    }

    /// D1 — a form saved before T2 (`type="unknown:Authentication"`) loads as the real type and is
    /// saved back as `type="Authentication"`, its properties kept.
    #[test]
    fn a_legacy_unknown_authentication_reloads_as_the_variant() {
        use crate::xml::{form_to_string, load_form_from_str};
        let legacy = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="LOGIN" title="Login" width="640" height="480" index-base="1">
  <Control id="AUTH-1" type="unknown:Authentication" x="10" y="10" w="300" h="300"><Property name="Mode">Hosted</Property></Control>
</Form>"#;
        let f = load_form_from_str(legacy).unwrap();
        let c = f.find_control("AUTH-1").unwrap();
        assert_eq!(c.control_type, ControlType::Authentication);
        assert_eq!(c.get_prop("Mode").map(|v| v.as_str().to_string()), Some("Hosted".to_string()));
        assert_eq!(c.control_type.supported_events(), EVENTS, "the Designer can bind the seven events");
        let text = form_to_string(&f).unwrap();
        assert!(text.contains(r#"type="Authentication""#) && !text.contains("unknown:"), "{text}");
    }

    /// D11 — `requires-sign-in` is written only when true and round-trips; a form without it is
    /// saved byte for byte as before.
    #[test]
    fn requires_sign_in_round_trips_and_is_absent_by_default() {
        use crate::model::Form;
        use crate::xml::{form_to_string, load_form_from_str};
        let plain = Form::new("ORDERS", "Orders", 800, 600);
        let text = form_to_string(&plain).unwrap();
        assert!(!text.contains("requires-sign-in"));
        let back = load_form_from_str(&text).unwrap();
        assert!(!back.requires_sign_in);
        assert_eq!(form_to_string(&back).unwrap(), text, "an older form is unchanged byte for byte");

        let mut guarded = plain.clone();
        guarded.requires_sign_in = true;
        let text = form_to_string(&guarded).unwrap();
        assert!(text.contains(r#"requires-sign-in="true""#), "{text}");
        let back = load_form_from_str(&text).unwrap();
        assert!(back.requires_sign_in);
        assert_eq!(form_to_string(&back).unwrap(), text);
    }

    /// T5 — what the Designer PAINTS, measured from the shapes: the Sign in screen's texts in the
    /// published language, the developer's own label winning, the two links following the Show
    /// flags, and `Mode = Hosted` painting one button with the hosted caption.
    #[cfg(feature = "render")]
    #[test]
    fn the_designer_paints_the_sign_in_screen_or_one_hosted_button() {
        use crate::model::PropValue;
        fn painted(c: &Control, locale: &str) -> (Vec<String>, usize) {
            let ctx = egui::Context::default();
            crate::paint::set_authentication_locale(&ctx, locale);
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(600.0, 600.0)));
            let mut full = ctx.run_ui(input, |ui| {
                crate::paint::draw_control(ui.painter(), egui::Pos2::ZERO, c, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, texts: &mut Vec<String>, filled: &mut usize) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts, filled)),
                    egui::Shape::Text(t) => texts.push(t.galley.text().to_owned()),
                    // The button is the accent; with no theme published that is the default blue.
                    egui::Shape::Rect(r) if r.fill == egui::Color32::from_rgb(37, 99, 235) => *filled += 1,
                    _ => {}
                }
            }
            let (mut texts, mut filled) = (Vec::new(), 0);
            for cs in &full.shapes {
                walk(&cs.shape, &mut texts, &mut filled);
            }
            (texts, filled)
        }
        let mut c = Control::new("AUTH-1", ControlType::Authentication, 0, 0);
        let (en, buttons) = painted(&c, "en");
        for t in ["Sign in", "User name", "Password", "Forgot your password?", "Create an account"] {
            assert!(en.iter().any(|x| x == t), "{t} not painted: {en:?}");
        }
        assert_eq!(en.iter().filter(|x| *x == "Sign in").count(), 2, "the title and the button");
        assert_eq!(buttons, 1, "one filled button");
        let (pt, _) = painted(&c, "pt-BR");
        assert!(pt.iter().any(|x| x == "Senha") && pt.iter().any(|x| x == "Entrar"), "{pt:?}");
        c.set_prop("PasswordLabel", PropValue::String("Palavra-chave".into()));
        c.set_prop("ShowSignUp", PropValue::Bool(false));
        let (own, _) = painted(&c, "pt-BR");
        assert!(own.iter().any(|x| x == "Palavra-chave") && !own.iter().any(|x| x == "Senha"), "the developer's label wins: {own:?}");
        assert!(!own.iter().any(|x| x == "Criar uma conta"), "ShowSignUp off removes its link: {own:?}");
        c.set_prop("Mode", PropValue::String("Hosted".into()));
        let (hosted, buttons) = painted(&c, "en");
        assert_eq!(hosted, vec!["Continue to sign in".to_owned()], "Hosted paints one button and nothing else");
        assert_eq!(buttons, 1);
        println!("in-app: {} texts painted (en), hosted: {:?}", en.len(), hosted);
    }
}
