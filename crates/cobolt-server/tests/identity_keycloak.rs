// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **AC2 against a real Keycloak** (WASM-009 T26; R12; O2, O7).
//!
//! Runs only when `PR_KEYCLOAK_URL` names a realm (`http://localhost:8080/realms/demo`, or an https one);
//! otherwise it says it was skipped, and why. With it, it also reads:
//!
//! | variable | what |
//! |---|---|
//! | `PR_KEYCLOAK_CLIENT` | a public client of the realm with the standard flow on (default `powerrustcobol`) |
//! | `PR_KEYCLOAK_REDIRECT` | a redirect URI the client allows (default `http://localhost:8099/callback`); nothing need listen there |
//! | `PR_KEYCLOAK_USER`, `PR_KEYCLOAK_PASSWORD` | a user of the realm with no required action pending |
//!
//! Keycloak is a hosted provider (plan §3.2): the account, its confirmation, a forgotten password and the second
//! factor are the realm's own pages, and the backend's part is the code exchange, the refresh and the revocation.
//! So the steps driven here are the backend's, through the real `Keycloak` adapter: metadata, the authorization
//! address with PKCE, the realm's login page answered as a browser answers it, the code exchanged, the token
//! refreshed, signed out (revoked), and the revoked refresh refused. Creating an account, confirming it, resetting a
//! password and MFA happen on Keycloak's pages and are the operator's to try by hand; the call of an exported
//! program with the token waits on O7 (the envelope alignment of WASM-014/016) and is printed as skipped.

#![cfg(feature = "server")]

use std::sync::Arc;

use cobolt_server::config::Discovery;
use cobolt_server::identity::client::UreqHttp;
use cobolt_server::identity::keycloak::Keycloak;
use cobolt_server::identity::oidc::{pkce, random_token, Oidc};
use cobolt_server::identity::{Flow, IdentityProvider};

fn var(name: &str, default: &str) -> String {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_owned())
}

/// `&amp;` and the other entities an attribute value may hold.
fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">")
}

/// The `action` of the realm's login form (`id="kc-form-login"`).
fn login_action(page: &str) -> Option<String> {
    let at = page.find("id=\"kc-form-login\"")?;
    let form_start = page[..at].rfind("<form")?;
    let tag = &page[form_start..form_start + page[form_start..].find('>')?];
    let a = tag.find("action=\"")? + "action=\"".len();
    Some(unescape(&tag[a..a + tag[a..].find('"')?]))
}

/// One query parameter of an address, decoded.
fn param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1.split('#').next()?;
    query.split('&').find_map(|p| p.strip_prefix(&format!("{name}="))).map(|v| {
        let b = v.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' && i + 2 < b.len() {
                out.push(u8::from_str_radix(&v[i + 1..i + 3], 16).unwrap_or(b'?'));
                i += 3;
            } else {
                out.push(if b[i] == b'+' { b' ' } else { b[i] });
                i += 1;
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    })
}

/// The cookies a response set, as a `Cookie` header adds them.
fn cookies(resp: &ureq::Response, jar: &mut Vec<String>) {
    for c in resp.all("set-cookie") {
        let pair = c.split(';').next().unwrap_or("").trim().to_owned();
        let name = pair.split('=').next().unwrap_or("").to_owned();
        jar.retain(|p| p.split('=').next() != Some(name.as_str()));
        if !pair.ends_with('=') {
            jar.push(pair);
        }
    }
}

#[test]
fn the_hosted_flow_signs_in_refreshes_and_signs_out_against_keycloak() {
    let Ok(realm) = std::env::var("PR_KEYCLOAK_URL") else {
        println!("SKIPPED: PR_KEYCLOAK_URL is not set (a realm address, e.g. http://localhost:8080/realms/demo); no Keycloak step ran");
        return;
    };
    let realm = realm.trim_end_matches('/').to_owned();
    let (client, redirect) = (var("PR_KEYCLOAK_CLIENT", "powerrustcobol"), var("PR_KEYCLOAK_REDIRECT", "http://localhost:8099/callback"));
    let (user, password) = (var("PR_KEYCLOAK_USER", ""), var("PR_KEYCLOAK_PASSWORD", ""));
    assert!(!user.is_empty() && !password.is_empty(), "PR_KEYCLOAK_USER and PR_KEYCLOAK_PASSWORD name the realm's test user");
    let mut ran: Vec<&str> = Vec::new();

    // 1. The realm's metadata.
    let meta: serde_json::Value = ureq::get(&format!("{realm}/.well-known/openid-configuration")).call().expect("the realm's metadata").into_json().expect("JSON metadata");
    let text = |k: &str| meta[k].as_str().map(str::to_owned);
    let discovery = Discovery {
        issuer: text("issuer").expect("issuer"),
        jwks_uri: text("jwks_uri").expect("jwks_uri"),
        authorization_endpoint: text("authorization_endpoint").expect("authorization_endpoint"),
        token_endpoint: text("token_endpoint").expect("token_endpoint"),
        revocation_endpoint: text("revocation_endpoint"),
        end_session_endpoint: text("end_session_endpoint"),
    };
    assert_eq!(discovery.issuer.trim_end_matches('/'), realm, "the metadata is the realm's");
    let kc = Keycloak::new(Oidc::from_discovery(Arc::new(UreqHttp), &discovery, &client, None, "openid profile email"));
    assert_eq!(kc.flow(), Flow::Hosted);
    assert!(kc.capabilities().refresh, "Keycloak refreshes");
    ran.push("metadata");

    // 2. The provider's page, answered as a browser answers it.
    let (verifier, challenge) = pkce();
    let (state, nonce) = (random_token(16), random_token(16));
    let start = kc.start_hosted(&redirect, &state, &challenge, &nonce).expect("the authorization address");
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let mut jar = Vec::new();
    let page = agent.get(&start).call().expect("the realm's login page");
    cookies(&page, &mut jar);
    let action = login_action(&page.into_string().unwrap()).expect("the realm's login form (id=\"kc-form-login\")");
    let answer = match agent.post(&action).set("Cookie", &jar.join("; ")).send_form(&[("username", &user), ("password", &password), ("credentialId", "")]) {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => panic!("posting the login form: {}", e.kind()),
    };
    let back = answer.header("location").map(str::to_owned).unwrap_or_else(|| panic!("the realm did not send the browser back (HTTP {}): a wrong password, or a required action pending", answer.status()));
    assert!(back.starts_with(&redirect), "the realm sends the browser back to the redirect URI");
    assert_eq!(param(&back, "state").as_deref(), Some(state.as_str()), "the state comes back unchanged");
    let code = param(&back, "code").expect("an authorization code");
    ran.push("hosted sign-in");

    // 3. The backend's half: the code exchanged, the token refreshed.
    let tokens = kc.finish_hosted(&code, &redirect, &verifier).unwrap_or_else(|e| panic!("the code exchange: {e:?}"));
    assert!(!tokens.access.is_empty() && tokens.refresh.is_some(), "{tokens:?}");
    ran.push("code exchange");
    let refreshed = kc.refresh(tokens.refresh.as_deref().unwrap(), &user).unwrap_or_else(|e| panic!("the refresh: {e:?}"));
    assert!(!refreshed.access.is_empty(), "{refreshed:?}");
    ran.push("refresh");

    // 4. The call of an exported program with the token: O7.
    println!("  skipped: the call of an exported program with the token (O7, the envelope alignment of WASM-014/016)");

    // 5. Signed out: the refresh token is revoked and refused afterwards.
    let last = refreshed.refresh.clone().or(tokens.refresh.clone()).unwrap();
    kc.sign_out(&refreshed.access, Some(&last)).unwrap_or_else(|e| panic!("the sign-out: {e:?}"));
    if kc.capabilities().revoke {
        assert!(kc.refresh(&last, &user).is_err(), "a revoked refresh token is refused");
        ran.push("sign-out (revoked)");
    } else {
        ran.push("sign-out (the realm names no revocation endpoint)");
    }
    println!("Keycloak at {realm}: ran {}", ran.join(", "));
    println!("  not driven here (the realm's own pages; the operator's): create an account, confirm it, forgot and reset, MFA");
}
