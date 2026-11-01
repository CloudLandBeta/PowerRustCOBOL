// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-009 T18: Test connection (`rcrun backend auth test`, the IDE's Identity page) against the
//! provider mocks. Every check passes on a good configuration; a wrong reCAPTCHA secret, metadata
//! naming another issuer and an unreachable provider each fail on their own line; and no line of any
//! run holds a configured value.

#[path = "support/idp_mock.rs"]
mod idp_mock;

use std::path::PathBuf;
use std::sync::Arc;

use cobolt_server::identity::check::{test_backend, Status};
use cobolt_server::identity::client::{Http, HttpRequest, HttpResponse};
use idp_mock::*;
use serde_json::json;

const POOL: &str = "https://cognito-idp.eu-west-1.amazonaws.com/eu-west-1_Pool";
const RECAPTCHA_SECRET: &str = "6Lc-recaptcha-secret-for-the-test";
const COOKIE_KEY: &str = "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=";

/// The provider as the connection test meets it: the metadata, the keys, the authorise page and
/// Google's `siteverify` here, the rest (the sign-in API) the provider mock's.
struct World {
    mock: IdpMock,
    issuer: String,
    reachable: bool,
}

impl Http for World {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, String> {
        if !self.reachable {
            return Err(format!("connection refused: {}", req.url));
        }
        let ok = |v: serde_json::Value| Ok(HttpResponse { status: 200, body: v.to_string() });
        if req.url == format!("{POOL}/.well-known/openid-configuration") {
            return ok(json!({
                "issuer": self.issuer,
                "jwks_uri": format!("{POOL}/.well-known/jwks.json"),
                "authorization_endpoint": format!("{COGNITO_DOMAIN}/oauth2/authorize"),
                "token_endpoint": format!("{COGNITO_DOMAIN}/oauth2/token"),
            }));
        }
        if req.url == format!("{POOL}/.well-known/jwks.json") {
            return ok(json!({"keys": [{"kty": "RSA", "kid": "k1"}, {"kty": "RSA", "kid": "k2"}]}));
        }
        if req.url.starts_with(&format!("{COGNITO_DOMAIN}/oauth2/authorize")) {
            return Ok(HttpResponse { status: 200, body: "<html>sign in</html>".into() });
        }
        if req.url.starts_with("https://www.google.com/recaptcha/api/siteverify") {
            let body = String::from_utf8_lossy(&req.body);
            let code = if body.contains(&format!("secret={}", cobolt_server::identity::client::percent(RECAPTCHA_SECRET))) { "invalid-input-response" } else { "invalid-input-secret" };
            return ok(json!({"success": false, "error-codes": [code]}));
        }
        self.mock.send(req)
    }
}

fn backend(tag: &str, recaptcha_secret: &str) -> PathBuf {
    std::env::set_var("PRC_CHECK_SECRET", CLIENT_SECRET);
    std::env::set_var("PRC_CHECK_COOKIE", COOKIE_KEY);
    std::env::set_var(format!("PRC_CHECK_RECAPTCHA_{}", tag.to_ascii_uppercase()), recaptcha_secret);
    let dir = std::env::temp_dir().join(format!("pr-identity-check-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("backend.toml"),
        format!(
            "[server]\nlisten = \"127.0.0.1:0\"\nallowed_origins = [\"https://app.example.test\"]\n[identity]\nprovider = \"cognito\"\ncookie_key = \"env:PRC_CHECK_COOKIE\"\n[identity.settings]\nregion = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\nclient_id = \"{CLIENT_ID}\"\nclient_secret = \"env:PRC_CHECK_SECRET\"\ndomain = \"{COGNITO_DOMAIN}\"\n[identity.recaptcha]\nsite_key = \"site-key-public\"\nsecret = \"env:PRC_CHECK_RECAPTCHA_{}\"\n",
            tag.to_ascii_uppercase()
        ),
    )
    .unwrap();
    dir
}

fn run(tag: &str, recaptcha_secret: &str, issuer: &str, reachable: bool) -> Vec<(Status, String)> {
    let dir = backend(tag, recaptcha_secret);
    let mock = IdpMock::new();
    mock.state.lock().unwrap().secret = true;
    let world = Arc::new(World { mock, issuer: issuer.to_string(), reachable });
    let lines = test_backend(&dir, world).unwrap_or_else(|e| panic!("{e:?}"));
    println!("  {tag}:");
    for l in &lines {
        println!("    {l}");
    }
    let printed: String = lines.iter().map(|l| format!("{l}\n")).collect();
    for value in [CLIENT_SECRET, CLIENT_ID, RECAPTCHA_SECRET, "6Lc-wrong", COOKIE_KEY, POOL, COGNITO_DOMAIN, "eu-west-1_Pool", "site-key-public"] {
        assert!(!printed.contains(value), "{tag}: a configured value is printed: {value}\n{printed}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    lines.into_iter().map(|l| (l.status, l.text)).collect()
}

#[test]
fn every_check_passes_on_a_good_configuration() {
    let lines = run("good", RECAPTCHA_SECRET, POOL, true);
    let statuses: Vec<Status> = lines.iter().map(|l| l.0).collect();
    assert_eq!(statuses, vec![Status::Ok; 5], "{lines:?}");
    assert!(lines[1].1.contains("2 signing key(s)"));
    assert!(lines[2].1.contains("unknown user is refused"));
}

#[test]
fn a_wrong_recaptcha_secret_another_issuer_and_an_unreachable_provider_each_fail_on_their_line() {
    let lines = run("badsecret", "6Lc-wrong", POOL, true);
    assert_eq!(lines[4].0, Status::Fail);
    assert!(lines[4].1.contains("invalid-input-secret"), "{lines:?}");
    assert!(lines[..4].iter().all(|l| l.0 == Status::Ok));

    let lines = run("issuer", RECAPTCHA_SECRET, "https://cognito-idp.eu-west-1.amazonaws.com/eu-west-1_Other", true);
    assert_eq!((lines[0].0, lines[1].0), (Status::Fail, Status::Skip), "{lines:?}");
    assert!(lines[0].1.contains("another issuer"));

    let lines = run("down", RECAPTCHA_SECRET, POOL, false);
    assert!(lines.iter().filter(|l| l.0 == Status::Fail).count() >= 3, "{lines:?}");
    assert!(lines.iter().all(|l| !l.1.contains("https://")), "no address: {lines:?}");
}

#[test]
fn a_backend_without_identity_has_nothing_to_test() {
    let dir = std::env::temp_dir().join(format!("pr-identity-check-none-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("backend.toml"), "[server]\nlisten = \"127.0.0.1:0\"\n").unwrap();
    let e = test_backend(&dir, Arc::new(World { mock: IdpMock::new(), issuer: String::new(), reachable: true })).err().expect("nothing to test");
    assert!(e[0].contains("no [identity] section"), "{e:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
