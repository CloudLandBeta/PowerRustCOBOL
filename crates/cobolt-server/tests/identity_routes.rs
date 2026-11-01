// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-009 T17: the `/auth/*` route table (D5), driven in process. Every request runs the real
//! generated `AUTH-*` program (T15) on the interpreter, whose `COBOL::"AUTH-OP"` reaches the real
//! identity service over the provider mocks (`support/idp_mock.rs`). The cases, each printed:
//! the cookie attributes; the preflight; no `X-PR-Identity` or no JSON → 403; a foreign origin → 403;
//! a page cannot ask for the refresh token; the native client gets it in the body; refresh and sign-out
//! through the cookie, a tampered cookie refused; the hosted flow end to end, a foreign `return`
//! refused, a `state` mismatch refused, the handle single-use and good for its age only; and no
//! `Location` carrying a token.

#[path = "support/idp_mock.rs"]
mod idp_mock;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cobolt_server::backend::Backend;
use cobolt_server::config::{RunMode, ServerConfig};
use cobolt_server::identity::adapter;
use cobolt_server::identity::client::Http;
use cobolt_server::identity::ops::{IdentityService, ServiceConfig};
use cobolt_server::identity::routes::{cobol_runner, AuthRequest, AuthResponse, AuthRoutes, HOSTED_COOKIE, REFRESH_COOKIE};
use cobolt_server::identity::seal::Sealer;
use idp_mock::*;
use serde_json::{json, Value};

const APP: &str = "https://app.example.test";
const COOKIE_KEY: &str = "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=";

fn backend_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pr-identity-routes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("exports.toml"), "[contract]\nversion = \"1.0.0\"\n").unwrap();
    cobolt_codegen::identity::sync(&dir, true).expect("generates");
    dir
}

struct Rig {
    routes: AuthRoutes,
    mock: Arc<IdpMock>,
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn rig(provider: &str, tag: &str) -> Rig {
    rig_with(provider, tag, None)
}

/// [`rig`], with the hosted handles living `handle_age` instead of a minute.
fn rig_with(provider: &str, tag: &str, handle_age: Option<Duration>) -> Rig {
    std::env::set_var("PRC_ROUTES_SECRET", CLIENT_SECRET);
    std::env::set_var("PRC_ROUTES_COOKIE", COOKIE_KEY);
    let settings = match provider {
        "cognito" => format!(
            "provider = \"cognito\"\n[identity.settings]\nregion = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\nclient_id = \"{CLIENT_ID}\"\nclient_secret = \"env:PRC_ROUTES_SECRET\"\ndomain = \"{COGNITO_DOMAIN}\"\n"
        ),
        _ => format!("provider = \"keycloak\"\n[identity.settings]\nissuer = \"{KEYCLOAK_ISSUER}\"\nclient_id = \"{CLIENT_ID}\"\nclient_secret = \"env:PRC_ROUTES_SECRET\"\n"),
    };
    let (head, tables) = settings.split_once("[identity.settings]").unwrap();
    let toml = format!(
        "[server]\nlisten = \"127.0.0.1:8443\"\nallowed_origins = [\"{APP}\"]\n[identity]\ncookie_key = \"env:PRC_ROUTES_COOKIE\"\n{head}[identity.settings]{tables}"
    );
    let cfg = ServerConfig::from_toml(&toml).unwrap().resolve(RunMode::Release, Path::new(".")).unwrap_or_else(|e| panic!("{e:?}"));
    let identity = cfg.identity.expect("[identity]");
    let mock = Arc::new(IdpMock::new());
    mock.state.lock().unwrap().secret = true;
    let provider_box = adapter(&identity, Some(&IdpMock::discovery(provider)), mock.clone() as Arc<dyn Http>).unwrap();
    let service = Arc::new(IdentityService::new(
        provider_box,
        None,
        Sealer::from_setting(identity.cookie_key.expose()).unwrap(),
        ServiceConfig { attempts_per_minute: 10_000, floor: Duration::from_millis(1), origins: identity.origins.clone(), site_key: None },
    ));
    let dir = backend_dir(tag);
    let backend = Arc::new(Backend::load(&dir).unwrap_or_else(|e| panic!("{e:#?}")));
    let run = cobol_runner(backend, service.clone(), Duration::from_secs(1));
    let mut routes = AuthRoutes::new(service, run, identity.origins.clone());
    if let Some(age) = handle_age {
        routes = routes.with_handle_age(age);
    }
    Rig { routes, mock, dir }
}

fn post(route: &str, body: Value) -> AuthRequest {
    AuthRequest {
        method: "POST".into(),
        route: route.into(),
        headers: vec![("origin".into(), APP.into()), ("content-type".into(), "application/json".into()), ("x-pr-identity".into(), "1".into())],
        body: body.to_string().into_bytes(),
        peer_ip: "192.0.2.7".into(),
        host: "api.example.test".into(),
        secure: true,
        ..Default::default()
    }
}

fn get(route: &str, query: &str) -> AuthRequest {
    AuthRequest { method: "GET".into(), route: route.into(), query: query.into(), peer_ip: "192.0.2.7".into(), host: "api.example.test".into(), secure: true, ..Default::default() }
}

fn with_cookie(mut r: AuthRequest, name: &str, value: &str) -> AuthRequest {
    r.headers.push(("cookie".into(), format!("other=1; {name}={value}")));
    r
}

fn without(mut r: AuthRequest, header: &str) -> AuthRequest {
    r.headers.retain(|(k, _)| k != header);
    r
}

fn body(r: &AuthResponse) -> Value {
    serde_json::from_str(&r.body).unwrap_or_else(|_| panic!("not JSON: {} {}", r.status, r.body))
}

/// The value a `Set-Cookie` of `name` sets, and the whole header.
fn set_cookie<'a>(r: &'a AuthResponse, name: &str) -> Option<(&'a str, &'a str)> {
    r.header_values("set-cookie").into_iter().find(|h| h.starts_with(&format!("{name}="))).map(|h| (h[name.len() + 1..].split(';').next().unwrap(), h))
}

#[test]
fn sign_in_sets_the_sealed_cookie_and_the_page_never_sees_the_refresh_token() {
    let rig = rig("cognito", "cookie");
    let r = rig.routes.handle(&post("sign-in", json!({"USERNAME": "ana", "PASSWORD": "Passw0rd!"})));
    assert_eq!(r.status, 200, "{}", r.body);
    let b = body(&r);
    assert_eq!(b["OUTCOME"], "OK", "{b}");
    assert!(b["ACCESS-TOKEN"].as_str().unwrap().starts_with("at.ana."), "{b}");
    assert_eq!(b["REFRESH-TOKEN"], "", "the refresh token is not in the page's answer");
    let (value, header) = set_cookie(&r, REFRESH_COOKIE).expect("the refresh cookie");
    for attr in ["Path=/", "Secure", "HttpOnly", "SameSite=Strict", "Max-Age=2592000"] {
        assert!(header.contains(attr), "{attr} missing: {header}");
    }
    assert!(!header.contains("Domain"), "a __Host- cookie names no domain: {header}");
    assert!(!value.contains("rt.ana"), "the cookie is sealed, not the token: {value}");
    assert_eq!(r.header_values("access-control-allow-origin"), vec![APP]);
    assert_eq!(r.header_values("access-control-allow-credentials"), vec!["true"]);
    assert!(r.header_values("cache-control").contains(&"no-store"));
    println!("cookie: {}", header.replacen(value, "<sealed>", 1));
}

#[test]
fn the_post_checks_refuse_what_a_cross_site_page_could_send() {
    let rig = rig("cognito", "guard");
    let ok = post("sign-in", json!({"USERNAME": "ana", "PASSWORD": "Passw0rd!"}));
    let cases: Vec<(&str, AuthRequest, u16)> = vec![
        ("no X-PR-Identity", without(ok.clone(), "x-pr-identity"), 403),
        ("X-PR-Identity: 0", {
            let mut r = without(ok.clone(), "x-pr-identity");
            r.headers.push(("x-pr-identity".into(), "0".into()));
            r
        }, 403),
        ("form content type", {
            let mut r = without(ok.clone(), "content-type");
            r.headers.push(("content-type".into(), "application/x-www-form-urlencoded".into()));
            r
        }, 403),
        ("foreign origin", {
            let mut r = without(ok.clone(), "origin");
            r.headers.push(("origin".into(), "https://evil.example".into()));
            r
        }, 403),
        ("origin null", {
            let mut r = without(ok.clone(), "origin");
            r.headers.push(("origin".into(), "null".into()));
            r
        }, 403),
        ("no origin, not native", without(ok.clone(), "origin"), 403),
        ("a page sends REFRESH-TOKEN", post("refresh", json!({"REFRESH-TOKEN": "rt.ana.1"})), 400),
        ("a page sends HOSTED", post("sign-in", json!({"HOSTED": "x"})), 400),
        ("an unknown route", post("sign-in-now", json!({})), 404),
    ];
    for (name, req, want) in cases {
        let r = rig.routes.handle(&req);
        println!("{name:<28} → {}", r.status);
        assert_eq!(r.status, want, "{name}: {}", r.body);
        assert!(r.header_values("access-control-allow-origin").iter().all(|o| *o == APP), "{name}: CORS only for the allowed origin");
    }
    // The preflight: the allowed origin gets the methods and headers, without X-PR-Client.
    let mut pre = get("sign-in", "");
    pre.method = "OPTIONS".into();
    pre.headers.push(("origin".into(), APP.into()));
    let r = rig.routes.handle(&pre);
    assert_eq!(r.status, 204);
    assert_eq!(r.header_values("access-control-allow-headers"), vec!["content-type, x-pr-identity"]);
    pre.headers = vec![("origin".into(), "https://evil.example".into())];
    assert_eq!(rig.routes.handle(&pre).status, 403);
    println!("preflight: allowed origin 204, foreign 403");
    // Nothing reached the provider: every refusal came before the program ran.
    assert_eq!(rig.mock.requests().len(), 0, "{:?}", rig.mock.requests());
}

#[test]
fn the_native_client_gets_the_refresh_token_in_the_body_and_no_cookie() {
    let rig = rig("cognito", "native");
    let mut req = without(post("sign-in", json!({"USERNAME": "ana", "PASSWORD": "Passw0rd!"})), "origin");
    req.headers.push(("x-pr-client".into(), "native".into()));
    let r = rig.routes.handle(&req);
    let b = body(&r);
    assert_eq!(b["OUTCOME"], "OK", "{b}");
    let rt = b["REFRESH-TOKEN"].as_str().unwrap().to_string();
    assert!(rt.starts_with("rt.ana."), "{b}");
    assert!(r.header_values("set-cookie").is_empty());
    // Its refresh sends the token in the body.
    let mut refresh = without(post("refresh", json!({"REFRESH-TOKEN": rt, "USERNAME": "ana"})), "origin");
    refresh.headers.push(("x-pr-client".into(), "native".into()));
    assert_eq!(body(&rig.routes.handle(&refresh))["OUTCOME"], "OK");
    // A page cannot claim to be native: with an Origin the header changes nothing.
    let mut page = post("sign-in", json!({"USERNAME": "ana", "PASSWORD": "Passw0rd!"}));
    page.headers.push(("x-pr-client".into(), "native".into()));
    let r = rig.routes.handle(&page);
    assert_eq!(body(&r)["REFRESH-TOKEN"], "");
    assert!(set_cookie(&r, REFRESH_COOKIE).is_some());
    println!("native: refresh token in the body, no cookie; a page with X-PR-Client: native still gets the cookie");
}

#[test]
fn refresh_and_sign_out_go_through_the_cookie() {
    let rig = rig("cognito", "refresh");
    let r = rig.routes.handle(&post("sign-in", json!({"USERNAME": "ana", "PASSWORD": "Passw0rd!"})));
    let (cookie, _) = set_cookie(&r, REFRESH_COOKIE).unwrap();
    let cookie = cookie.to_string();

    let r = rig.routes.handle(&with_cookie(post("refresh", json!({})), REFRESH_COOKIE, &cookie));
    let b = body(&r);
    assert_eq!(b["OUTCOME"], "OK", "{b}");
    assert!(b["ACCESS-TOKEN"].as_str().unwrap().starts_with("at.ana."));
    assert_eq!(b["REFRESH-TOKEN"], "");

    // No cookie, a tampered one, one sealed with another key: refused, and the cookie removed.
    let other = Sealer::new(&[9u8; 32]).seal_json("refresh", &json!({"u": "ana", "r": "rt.ana.1"}));
    let mut flipped = cookie.clone().into_bytes();
    let i = flipped.len() / 2;
    flipped[i] = if flipped[i] == b'A' { b'B' } else { b'A' };
    for (name, req) in [
        ("no cookie", post("refresh", json!({}))),
        ("tampered cookie", with_cookie(post("refresh", json!({})), REFRESH_COOKIE, std::str::from_utf8(&flipped).unwrap())),
        ("another key's cookie", with_cookie(post("refresh", json!({})), REFRESH_COOKIE, &other)),
    ] {
        let r = rig.routes.handle(&req);
        assert_eq!(body(&r)["OUTCOME"], "AUTH-FAILED", "{name}");
        assert!(set_cookie(&r, REFRESH_COOKIE).is_some_and(|(v, h)| v.is_empty() && h.contains("Max-Age=0")), "{name}: the cookie is removed");
        println!("refresh, {name:<20} → AUTH-FAILED, cookie removed");
    }

    let r = rig.routes.handle(&with_cookie(post("sign-out", json!({})), REFRESH_COOKIE, &cookie));
    assert_eq!(body(&r)["OUTCOME"], "OK");
    assert!(set_cookie(&r, REFRESH_COOKIE).is_some_and(|(v, _)| v.is_empty()));
    assert!(rig.mock.state.lock().unwrap().revoked.iter().any(|t| t.starts_with("rt.ana.")), "the provider revoked the refresh token");
    println!("sign-out: cookie removed, the provider's token revoked");
}

/// One hosted sign-in up to the callback: the start's redirect and cookie, the provider's page, the
/// callback's answer.
fn hosted_to_callback(rig: &Rig, state_override: Option<&str>) -> (AuthResponse, AuthResponse) {
    let start = rig.routes.handle(&get("hosted/start", &format!("return={}&state=page-123", percent(&format!("{APP}/orders?x=1")))));
    assert_eq!(start.status, 302, "{}", start.body);
    let (hosted, header) = set_cookie(&start, HOSTED_COOKIE).expect("the hosted cookie");
    assert!(header.contains("SameSite=Lax") && header.contains("Max-Age=600") && header.contains("HttpOnly") && header.contains("Secure"), "{header}");
    let location = start.header_values("location")[0];
    assert!(location.contains("redirect_uri=https%3A%2F%2Fapi.example.test%2Fauth%2Fhosted%2Fcallback"), "{location}");
    let (code, state) = rig.mock.authorize(location, "ana", "Passw0rd!").expect("the provider's page signs in");
    let state = state_override.map(str::to_owned).unwrap_or(state);
    let cb = rig.routes.handle(&with_cookie(get("hosted/callback", &format!("code={code}&state={state}")), HOSTED_COOKIE, hosted));
    (start, cb)
}

fn percent(s: &str) -> String {
    cobolt_server::identity::client::percent(s)
}

#[test]
fn the_hosted_flow_returns_a_single_use_handle_and_never_a_token_in_a_url() {
    let rig = rig("keycloak", "hosted");
    let (start, cb) = hosted_to_callback(&rig, None);
    assert_eq!(cb.status, 302, "{}", cb.body);
    let location = cb.header_values("location")[0].to_string();
    assert!(location.starts_with(&format!("{APP}/orders?x=1&pr-code=")), "{location}");
    assert!(location.ends_with("&pr-state=page-123"), "{location}");
    assert!(set_cookie(&cb, HOSTED_COOKIE).is_some_and(|(v, _)| v.is_empty()), "the hosted cookie is spent");
    let (refresh_cookie, _) = set_cookie(&cb, REFRESH_COOKIE).expect("the refresh cookie");
    let handle = location.split("pr-code=").nth(1).unwrap().split('&').next().unwrap().to_string();
    assert_eq!(handle.len(), 43, "32 random bytes, base64url");

    // Every token the provider issued, searched in every Location.
    let issued: Vec<String> = {
        let st = rig.mock.state.lock().unwrap();
        st.refresh.keys().cloned().chain(st.refresh.keys().map(|r| r.replacen("rt.", "at.", 1))).collect()
    };
    for loc in start.header_values("location").into_iter().chain(cb.header_values("location")) {
        for t in &issued {
            assert!(!loc.contains(t.as_str()), "a token in a Location: {loc}");
        }
    }

    let finish = rig.routes.handle(&post("hosted/finish", json!({"HANDLE": handle})));
    let b = body(&finish);
    assert_eq!(b["OUTCOME"], "OK", "{b}");
    assert!(b["ACCESS-TOKEN"].as_str().unwrap().starts_with("at.ana."), "{b}");
    assert_eq!(b["REFRESH-TOKEN"], "");
    // Single use: the second time there is no such handle, and without the cookie that is a failure…
    assert_eq!(body(&rig.routes.handle(&post("hosted/finish", json!({"HANDLE": handle}))))["OUTCOME"], "AUTH-FAILED");
    // …while with the cookie (another instance, a lost answer) the refresh stands in.
    let again = rig.routes.handle(&with_cookie(post("hosted/finish", json!({"HANDLE": handle})), REFRESH_COOKIE, refresh_cookie));
    assert_eq!(body(&again)["OUTCOME"], "OK");
    println!("hosted: 302 to the provider, back with pr-code (43 chars) and pr-state, {} tokens searched in {} Locations: 0 hits; the handle answers once", issued.len(), 2);
}

#[test]
fn the_hosted_flow_refuses_a_foreign_return_a_wrong_state_and_an_old_handle() {
    let rig = rig("keycloak", "hosted-refusals");
    let r = rig.routes.handle(&get("hosted/start", &format!("return={}&state=s", percent("https://evil.example/"))));
    assert_eq!(r.status, 400, "{}", r.body);
    assert!(set_cookie(&r, HOSTED_COOKIE).is_none());
    println!("foreign return → {}", r.status);

    let (_, cb) = hosted_to_callback(&rig, Some("not-the-state"));
    assert_eq!(cb.status, 400, "{}", cb.body);
    assert!(cb.header_values("location").is_empty());
    assert!(set_cookie(&cb, REFRESH_COOKIE).is_none());
    println!("state mismatch → {}", cb.status);

    let no_cookie = rig.routes.handle(&get("hosted/callback", "code=code-1&state=x"));
    assert_eq!(no_cookie.status, 400);
    let denied = rig.routes.handle(&get("hosted/callback", "error=access_denied&state=x"));
    assert_eq!(denied.status, 400);
    println!("no hosted cookie → 400; the provider's error → 400");

    // A handle is good for its age only (a 50 ms age here, 60 s in the backend).
    let rig = rig_with("keycloak", "hosted-age", Some(Duration::from_millis(50)));
    let (_, cb) = hosted_to_callback(&rig, None);
    let location = cb.header_values("location")[0].to_string();
    let handle = location.split("pr-code=").nth(1).unwrap().split('&').next().unwrap().to_string();
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(body(&rig.routes.handle(&post("hosted/finish", json!({"HANDLE": handle}))))["OUTCOME"], "AUTH-FAILED");
    println!("a handle past its age → AUTH-FAILED (no cookie)");
}
