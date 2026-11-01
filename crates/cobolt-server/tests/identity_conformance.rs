// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-009 AC8: one conformance suite over the four adapters — Cognito (in-app), Keycloak
//! (hosted), Entra workforce (hosted) and Entra External ID (`native`) — each against a mock that
//! speaks its product's documented wire shapes (`support/idp_mock.rs`).
//!
//! Every operation plan §6 marks `Y` for an adapter succeeds through the identity service; every
//! operation it marks `-` or "hosted" answers `HOSTED` ("continue on the provider's page"). A
//! second test checks that §6's table and each adapter's `capabilities()` agree. The suite prints
//! the operations × adapters matrix it ran.

#[path = "support/idp_mock.rs"]
mod idp_mock;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use cobolt_server::config::{RunMode, ServerConfig};
use cobolt_server::identity::client::Http;
use cobolt_server::identity::ops::{IdentityService, OpContext, ServiceConfig};
use cobolt_server::identity::seal::Sealer;
use cobolt_server::identity::adapter;
use idp_mock::*;
use serde_json::{json, Value};

const OPS: [&str; 12] = ["sign-in", "sign-up", "confirm", "reset", "new-password", "mfa-totp", "mfa-sms", "mfa-email", "mfa-setup", "refresh", "revoke", "hosted"];

/// Plan §6, by row, for the four adapters in their default flows (`Y` supported in-app, `-`
/// otherwise; `hosted` is the provider's page). Entra External ID is in its `native` opt-in.
fn table(adapter: &str) -> [bool; 12] {
    match adapter {
        //                    in  up  conf reset newpw totp sms  email setup refr revoke hosted
        "cognito" => [true, true, true, true, true, true, true, true, true, true, true, true],
        "keycloak" => [false, false, false, false, false, false, false, false, false, true, true, true],
        "entra" => [false, false, false, false, false, false, false, false, false, true, false, true],
        "entra-external" => [true, true, true, true, false, false, false, true, false, true, false, true],
        _ => unreachable!(),
    }
}

fn identity_toml(adapter: &str) -> String {
    let settings = match adapter {
        "cognito" => format!(
            "provider = \"cognito\"\n[identity.settings]\nregion = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\nclient_id = \"{CLIENT_ID}\"\nclient_secret = \"env:PRC_CONF_SECRET\"\ndomain = \"{COGNITO_DOMAIN}\"\n"
        ),
        "keycloak" => format!("provider = \"keycloak\"\n[identity.settings]\nissuer = \"{KEYCLOAK_ISSUER}\"\nclient_id = \"{CLIENT_ID}\"\nclient_secret = \"env:PRC_CONF_SECRET\"\n"),
        "entra" => format!("provider = \"entra\"\n[identity.settings]\ntenant = \"{ENTRA_TENANT}\"\nclient_id = \"{CLIENT_ID}\"\nclient_secret = \"env:PRC_CONF_SECRET\"\n"),
        _ => format!("provider = \"entra-external\"\nflow = \"native\"\n[identity.settings]\ntenant_subdomain = \"shop\"\nclient_id = \"{CLIENT_ID}\"\nclient_secret = \"env:PRC_CONF_SECRET\"\n"),
    };
    // The [identity] keys must come before its sub-tables.
    let (head, tables) = settings.split_once("[identity.settings]").unwrap();
    format!(
        "[server]\nlisten = \"127.0.0.1:8443\"\nallowed_origins = [\"https://app.example.test\"]\n[identity]\ncookie_key = \"env:PRC_CONF_COOKIE\"\n{head}[identity.settings]{tables}"
    )
}

struct Under {
    service: IdentityService,
    mock: Arc<IdpMock>,
}

fn under(name: &str) -> Under {
    std::env::set_var("PRC_CONF_SECRET", CLIENT_SECRET);
    std::env::set_var("PRC_CONF_COOKIE", "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=");
    let cfg = ServerConfig::from_toml(&identity_toml(name)).unwrap().resolve(RunMode::Release, Path::new(".")).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    let identity = cfg.identity.expect("an [identity]");
    let mock = Arc::new(IdpMock::new());
    mock.state.lock().unwrap().secret = true;
    let discovery = IdpMock::discovery(name);
    let provider = adapter(&identity, Some(&discovery), mock.clone() as Arc<dyn Http>).unwrap();
    // The real service, with a short pad and a high limit, so the suite runs quickly.
    let service = IdentityService::new(
        provider,
        None,
        Sealer::from_setting(identity.cookie_key.expose()).unwrap(),
        ServiceConfig { attempts_per_minute: 10_000, floor: Duration::from_millis(1), origins: identity.origins.clone(), site_key: None },
    );
    Under { service, mock }
}

impl Under {
    fn run(&self, op: &str, req: Value) -> Value {
        match self.service.run(op, &req.to_string(), &OpContext { ip: "192.0.2.1".into() }) {
            Ok(b) => serde_json::from_str(&b).unwrap(),
            Err(e) => json!({"OUTCOME": format!("error {}: {}", e.code, e.message)}),
        }
    }

    /// A sign-in through the provider's page.
    fn hosted(&self, user: &str, password: &str) -> Value {
        let st = self.run("START-HOSTED", json!({"REDIRECT-URI": "https://api.example.test/auth/hosted/callback", "RETURN-URI": "https://app.example.test/", "PAGE-STATE": "p"}));
        let Some(url) = st["URL"].as_str().filter(|u| !u.is_empty()) else { return st };
        let Some((code, state)) = self.mock.authorize(url, user, password) else { return json!({"OUTCOME": "the provider's page refused"}) };
        assert_eq!(state, st["STATE"].as_str().unwrap(), "the state travels through the provider's page");
        self.run("FINISH-HOSTED", json!({"HOSTED": st["HOSTED"], "STATE": state, "ONE-TIME-CODE": code}))
    }

    /// Tokens by whatever sign-in this adapter has: in-app when it can, else hosted.
    fn tokens(&self, user: &str) -> Value {
        let r = self.run("SIGN-IN", json!({"USERNAME": user, "PASSWORD": "Passw0rd!"}));
        if r["OUTCOME"] == "OK" {
            r
        } else {
            self.hosted(user, "Passw0rd!")
        }
    }

    /// A sign-in that meets a second step, answered with `answer`.
    fn second_step(&self, user: &str, kind: &str, op: &str, answer: Value) -> String {
        let c = self.run("SIGN-IN", json!({"USERNAME": user, "PASSWORD": "Passw0rd!"}));
        if c["OUTCOME"] != "CHALLENGE" {
            return c["OUTCOME"].as_str().unwrap_or("?").to_string();
        }
        assert_eq!(c["CHALLENGE"], kind, "{user}: {c}");
        let mut a = answer;
        a["SESSION"] = c["SESSION"].clone();
        let done = self.run(op, a);
        if done["OUTCOME"] == "OK" && !done["ACCESS-TOKEN"].as_str().unwrap_or("").is_empty() {
            "OK".into()
        } else {
            format!("{} {}", done["OUTCOME"], done["OUTCOME-TEXT"])
        }
    }

    /// One row of the matrix: "OK" when the operation succeeded, "HOSTED" when it was handed to
    /// the provider's page, anything else is a failure.
    fn operation(&self, op: &str) -> String {
        let outcome = |v: &Value| v["OUTCOME"].as_str().unwrap_or("?").to_string();
        match op {
            "sign-in" => {
                let r = self.run("SIGN-IN", json!({"USERNAME": "ana", "PASSWORD": "Passw0rd!"}));
                if r["OUTCOME"] == "OK" && r["ACCESS-TOKEN"].as_str().unwrap().starts_with("at.ana.") {
                    "OK".into()
                } else {
                    outcome(&r)
                }
            }
            "sign-up" => {
                let r = self.run("SIGN-UP", json!({"USERNAME": "new-1", "PASSWORD": "Passw0rd!", "EMAIL": "new@example.test"}));
                match outcome(&r).as_str() {
                    "CONFIRMATION-REQUIRED" | "OK" => "OK".into(),
                    o => o.into(),
                }
            }
            "confirm" => {
                let r = self.run("SIGN-UP", json!({"USERNAME": "new-2", "PASSWORD": "Passw0rd!", "EMAIL": "new2@example.test"}));
                if r["OUTCOME"] != "CONFIRMATION-REQUIRED" {
                    return outcome(&r);
                }
                let resent = self.run("RESEND", json!({"SESSION": r["SESSION"]}));
                let c = self.run("CONFIRM", json!({"SESSION": resent["SESSION"], "ONE-TIME-CODE": CONFIRM_CODE}));
                if c["OUTCOME"] != "OK" {
                    return format!("confirm: {}", outcome(&c));
                }
                let s = self.run("SIGN-IN", json!({"USERNAME": "new-2", "PASSWORD": "Passw0rd!"}));
                if s["OUTCOME"] == "OK" { "OK".into() } else { format!("sign-in after confirm: {}", outcome(&s)) }
            }
            "reset" => {
                let f = self.run("FORGOT", json!({"USERNAME": "ana"}));
                if f["OUTCOME"] != "AUTH-RESET-REQUESTED" {
                    return outcome(&f);
                }
                let r = self.run("RESET", json!({"SESSION": f["SESSION"], "ONE-TIME-CODE": RESET_CODE, "PASSWORD": "N3w-Passw0rd!"}));
                if r["OUTCOME"] != "OK" {
                    return format!("reset: {}", outcome(&r));
                }
                let s = self.run("SIGN-IN", json!({"USERNAME": "ana", "PASSWORD": "N3w-Passw0rd!"}));
                // Put the old password back for the rows that follow.
                self.mock.state.lock().unwrap().users.get_mut("ana").unwrap().password = "Passw0rd!".into();
                if s["OUTCOME"] == "OK" { "OK".into() } else { format!("sign-in after reset: {}", outcome(&s)) }
            }
            "new-password" => self.second_step("temp", "new-password", "MFA", json!({"PASSWORD": "Br4nd-new-pass"})),
            "mfa-totp" => self.second_step("tina", "totp", "MFA", json!({"ONE-TIME-CODE": MFA_CODE})),
            "mfa-sms" => self.second_step("sam", "sms", "MFA", json!({"ONE-TIME-CODE": MFA_CODE})),
            "mfa-email" => self.second_step("eve", "email", "MFA", json!({"ONE-TIME-CODE": MFA_CODE})),
            "mfa-setup" => self.second_step("nora", "totp-setup", "MFA-SETUP", json!({"ONE-TIME-CODE": SETUP_CODE})),
            "refresh" => {
                let t = self.tokens("ana");
                let r = self.run("REFRESH", json!({"REFRESH-TOKEN": t["REFRESH-TOKEN"], "USERNAME": "ana"}));
                if r["OUTCOME"] == "OK" && r["ACCESS-TOKEN"] != t["ACCESS-TOKEN"] { "OK".into() } else { outcome(&r) }
            }
            "revoke" => {
                let t = self.tokens("ana");
                self.run("SIGN-OUT", json!({"REFRESH-TOKEN": t["REFRESH-TOKEN"], "ACCESS-TOKEN": t["ACCESS-TOKEN"]}));
                let r = self.run("REFRESH", json!({"REFRESH-TOKEN": t["REFRESH-TOKEN"], "USERNAME": "ana"}));
                // Revoked: the refresh token no longer works. Not revoked ("logout only"): it does.
                if r["OUTCOME"] == "AUTH-FAILED" { "OK".into() } else { "HOSTED".into() }
            }
            "hosted" => {
                let r = self.hosted("ana", "Passw0rd!");
                if r["OUTCOME"] == "OK" && !r["ACCESS-TOKEN"].as_str().unwrap_or("").is_empty() { "OK".into() } else { outcome(&r) }
            }
            _ => unreachable!(),
        }
    }
}

const ADAPTERS: [&str; 4] = ["cognito", "keycloak", "entra", "entra-external"];

#[test]
fn every_y_succeeds_and_every_other_cell_hands_over_to_the_providers_page() {
    let mut matrix = format!("{:14}", "operation");
    for a in ADAPTERS {
        matrix.push_str(&format!("{a:>16}"));
    }
    let mut failures = Vec::new();
    let suites: Vec<Under> = ADAPTERS.iter().map(|a| under(a)).collect();
    for (row, op) in OPS.iter().enumerate() {
        matrix.push_str(&format!("\n{op:14}"));
        for (col, a) in ADAPTERS.iter().enumerate() {
            let got = suites[col].operation(op);
            let want_y = table(a)[row];
            let cell = match (want_y, got.as_str()) {
                (true, "OK") => "Y",
                (false, "HOSTED") => "- (hosted)",
                _ => {
                    failures.push(format!("{a} {op}: the table says {}, the run answered {got}", if want_y { "Y" } else { "-" }));
                    "FAIL"
                }
            };
            matrix.push_str(&format!("{cell:>16}"));
        }
    }
    println!("AC8 operations x adapters:\n{matrix}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_table_and_each_adapters_capabilities_agree() {
    for a in ADAPTERS {
        let u = under(a);
        let named = u.service.provider().capabilities().named();
        for (row, (name, value)) in named.iter().enumerate() {
            assert_eq!(*name, OPS[row]);
            assert_eq!(*value, table(a)[row], "{a} {name}: capabilities() says {value}");
        }
        let flow = u.service.provider().flow().word();
        let want = match a {
            "cognito" => "in-app",
            "entra-external" => "native",
            _ => "hosted",
        };
        assert_eq!(flow, want, "{a}: the default flow (O1, the working assumption)");
    }
}

#[test]
fn an_unknown_user_and_a_wrong_password_answer_the_same_on_every_adapter() {
    for a in ["cognito", "entra-external"] {
        let u = under(a);
        // The mocks answer the products' telling errors: Cognito's LEGACY `UserNotFoundException`,
        // Entra's AADSTS50034 against AADSTS50126.
        let unknown = u.run("SIGN-IN", json!({"USERNAME": "nobody", "PASSWORD": "Passw0rd!"}));
        let wrong = u.run("SIGN-IN", json!({"USERNAME": "ana", "PASSWORD": "wrong-password"}));
        assert_eq!(unknown, wrong, "{a}");
        assert_eq!(unknown["OUTCOME"], "AUTH-FAILED", "{a}");
        let (f1, f2) = (u.run("FORGOT", json!({"USERNAME": "ana"})), u.run("FORGOT", json!({"USERNAME": "bob"})));
        assert_eq!((&f1["OUTCOME"], &f1["OUTCOME-TEXT"]), (&f2["OUTCOME"], &f2["OUTCOME-TEXT"]), "{a}");
        let up = u.run("SIGN-UP", json!({"USERNAME": "ana", "PASSWORD": "Passw0rd!"}));
        assert_eq!(up["OUTCOME"], "CONFIRMATION-REQUIRED", "{a}: a name that exists answers like a new one");
    }
}

#[test]
fn cognito_details_secret_hash_both_email_spellings_choice_and_rotation() {
    let u = under("cognito");
    // Every call that named a user carried the right SECRET_HASH (the mock refuses a wrong one).
    assert_eq!(u.operation("sign-in"), "OK");
    // Both e-mail spellings the documentation names.
    assert_eq!(u.second_step("emma", "email", "MFA", json!({"ONE-TIME-CODE": MFA_CODE})), "OK");
    // SELECT_MFA_TYPE: the choice, then the code of the factor chosen.
    let c = u.run("SIGN-IN", json!({"USERNAME": "cho", "PASSWORD": "Passw0rd!"}));
    assert_eq!((c["CHALLENGE"].as_str(), c["CHOICES"].as_str()), (Some("choice"), Some("sms,totp")));
    let next = u.run("MFA", json!({"SESSION": c["SESSION"], "CHOICE": "sms"}));
    assert_eq!((next["CHALLENGE"].as_str(), next["DESTINATION"].as_str()), (Some("sms"), Some("+*******1234")));
    let done = u.run("MFA", json!({"SESSION": next["SESSION"], "ONE-TIME-CODE": MFA_CODE}));
    assert_eq!(done["OUTCOME"], "OK");
    // The TOTP set-up shows the secret and an otpauth:// address naming the pool and the user.
    let s = u.run("SIGN-IN", json!({"USERNAME": "nora", "PASSWORD": "Passw0rd!"}));
    assert_eq!(s["SECRET"], TOTP_SECRET);
    assert_eq!(s["OTPAUTH-URI"], format!("otpauth://totp/eu-west-1_Pool%3Anora?secret={TOTP_SECRET}&issuer=eu-west-1_Pool"));
    // Rotation on: REFRESH_TOKEN_AUTH is refused by Cognito, so GetTokensFromRefreshToken is used.
    u.mock.state.lock().unwrap().rotation = true;
    let t = u.run("SIGN-IN", json!({"USERNAME": "ana", "PASSWORD": "Passw0rd!"}));
    let r = u.run("REFRESH", json!({"REFRESH-TOKEN": t["REFRESH-TOKEN"], "USERNAME": "ana"}));
    assert_eq!(r["OUTCOME"].as_str(), Some("AUTH-FAILED"), "without the setting, the rotated client refuses");
    std::env::set_var("PRC_CONF_SECRET", CLIENT_SECRET);
    let text = identity_toml("cognito").replace("domain = ", "refresh_rotation = \"true\"\ndomain = ");
    let cfg = ServerConfig::from_toml(&text).unwrap().resolve(RunMode::Release, Path::new(".")).unwrap();
    let id = cfg.identity.unwrap();
    let rotating = IdentityService::new(adapter(&id, None, u.mock.clone() as Arc<dyn Http>).unwrap(), None, Sealer::new(&[1; 32]), ServiceConfig { floor: Duration::from_millis(1), ..ServiceConfig::default() });
    let r: Value = serde_json::from_str(&rotating.run("REFRESH", &json!({"REFRESH-TOKEN": t["REFRESH-TOKEN"], "USERNAME": "ana"}).to_string(), &OpContext::default()).unwrap()).unwrap();
    assert_eq!(r["OUTCOME"], "OK");
    assert_ne!(r["REFRESH-TOKEN"], t["REFRESH-TOKEN"], "a rotated refresh token");
    assert_eq!(u.mock.hits("GetTokensFromRefreshToken"), 0, "the target travels in a header, not the address");
    assert!(u.mock.requests().iter().any(|(_, _, b)| b.contains("\"RefreshToken\"")));
}

#[test]
fn entra_external_falls_back_to_the_hosted_flow_when_the_native_api_says_redirect() {
    let u = under("entra-external");
    let r = u.run("SIGN-IN", json!({"USERNAME": "temp", "PASSWORD": "Passw0rd!"}));
    assert_eq!(r["OUTCOME"], "HOSTED", "challenge_type=redirect is the provider's page");
    // Two e-mail/SMS methods: the user chooses; SMS is refused by the native API (UNVERIFIED there), so hosted.
    let c = u.run("SIGN-IN", json!({"USERNAME": "cho", "PASSWORD": "Passw0rd!"}));
    assert_eq!(c["CHALLENGE"], "choice");
    let email = u.run("MFA", json!({"SESSION": c["SESSION"], "CHOICE": "email"}));
    assert_eq!((email["CHALLENGE"].as_str(), email["DESTINATION"].as_str()), (Some("email"), Some("e***@e***")));
    assert_eq!(u.run("MFA", json!({"SESSION": email["SESSION"], "ONE-TIME-CODE": MFA_CODE}))["OUTCOME"], "OK");
    // A sign-in through the native API never sent the password anywhere but the token step.
    let with_password: Vec<String> = u.mock.requests().into_iter().filter(|(_, _, b)| b.contains("Passw0rd")).map(|(_, url, _)| url).collect();
    assert!(with_password.iter().all(|url| url.ends_with("/oauth2/v2.0/token") || url.ends_with("/signup/v1.0/start")), "{with_password:?}");
}
