// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a bearer token must be to be accepted, one rule at a time (WASM-003
//! R9-R13, WASM-009 R8): the validator is driven directly, with no socket, so
//! each rule has its own named test and its own failure.

mod support;

use std::sync::Arc;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Value};
use support::*;

use cobolt_server::config::{RunMode, ServerConfig};
use cobolt_server::jwks::{Jwks, JwksFetcher};
use cobolt_server::jwt::{now, TokenValidator};
use cobolt_wire::error::codes;

const ISS: &str = "https://idp.example.test/pool";
const AUD: &str = "prc-api";
const CLIENT: &str = "app-client-1";

fn validator_with(auth_extra: &str, doc: String, dev_secret: Option<&str>) -> (TokenValidator, Arc<StaticJwks>, Arc<Jwks>) {
    let text = format!("[auth]\nissuer = [\"{ISS}\"]\naudiences = [\"{AUD}\"]\nclient_ids = [\"{CLIENT}\"]\nalgorithms = [\"RS256\", \"ES256\"]\n{auth_extra}\n");
    let cfg = ServerConfig::from_toml(&text).expect("config");
    let resolved = cfg.resolve(RunMode::Development, std::path::Path::new(".")).expect("resolve");
    let fetcher = StaticJwks::new(doc);
    let jwks = Arc::new(Jwks::new(format!("{ISS}/.well-known/jwks.json"), resolved.auth.jwks_ttl, resolved.auth.jwks_min_refresh, fetcher.clone()));
    (TokenValidator::new(resolved.auth, Some(jwks.clone()), dev_secret.map(str::to_owned)), fetcher, jwks)
}

fn ec_validator(auth_extra: &str) -> (TokenValidator, Issuer) {
    let issuer = Issuer::new(ISS, "k1");
    let (v, _, _) = validator_with(auth_extra, issuer.jwks(), None);
    (v, issuer)
}

fn claims(extra: Value) -> Value {
    let t = now();
    let mut c = json!({"iss": ISS, "sub": "user-1", "aud": AUD, "iat": t, "exp": t + 600});
    for (k, v) in extra.as_object().unwrap() {
        if v.is_null() {
            c.as_object_mut().unwrap().remove(k);
        } else {
            c[k] = v.clone();
        }
    }
    c
}

fn b64(v: &Value) -> String {
    URL_SAFE_NO_PAD.encode(v.to_string())
}

async fn refused(v: &TokenValidator, token: &str) -> (u32, String) {
    let e = v.validate(token).await.expect_err("the token should be refused");
    (e.code, e.message)
}

#[tokio::test]
async fn an_rs256_token_from_the_issuer_is_accepted() {
    let rsa = RsaIssuer::new(ISS, "rsa1");
    let (v, _, _) = validator_with("", rsa.jwks(), None);
    let token = rsa.mint(&claims(json!({"roles": ["admin"]})));
    let c = v.validate(&token).await.expect("a valid RS256 token");
    assert_eq!(c.sub, "user-1");
    assert!(c.roles.contains("admin"));
    assert_eq!(c.flat.get("iss").map(String::as_str), Some(ISS));
    assert!(!c.dev);
}

#[tokio::test]
async fn a_key_that_names_its_algorithm_is_not_used_for_another() {
    // The issuer publishes an ES256 key; a token claiming RS256 with that kid is
    // the classic algorithm-confusion shape. It gets no key.
    let issuer = Issuer::new(ISS, "k1");
    let rsa = RsaIssuer::new(ISS, "k1");
    let (v, _, _) = validator_with("", issuer.jwks(), None);
    let (code, message) = refused(&v, &rsa.mint(&claims(json!({})))).await;
    assert_eq!(code, codes::TOKEN_INVALID);
    assert!(message.contains("key the issuer does not publish"), "{message}");
}

#[tokio::test]
async fn an_unsigned_token_and_a_shared_secret_token_are_never_accepted() {
    let (v, issuer) = ec_validator("");
    // alg: none, with a signature part or without.
    let header = b64(&json!({"alg": "none", "typ": "JWT"}));
    let body = b64(&claims(json!({})));
    for token in [format!("{header}.{body}."), format!("{header}.{body}.AAAA")] {
        assert_eq!(refused(&v, &token).await.0, codes::TOKEN_INVALID, "{token}");
    }
    // HS256 signed with the issuer's PUBLIC key document as the secret (the
    // attack on a verifier that lets the token choose): refused, because there
    // is no development secret.
    let forged = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims(json!({})),
        &jsonwebtoken::EncodingKey::from_secret(issuer.jwks().as_bytes()),
    )
    .unwrap();
    let (code, message) = refused(&v, &forged).await;
    assert_eq!(code, codes::TOKEN_INVALID);
    assert!(message.contains("algorithm is not accepted"), "{message}");
}

#[tokio::test]
async fn the_development_token_works_only_where_a_secret_was_configured() {
    let secret = "0123456789abcdef".repeat(2);
    let secret = secret.as_str();
    let issuer = Issuer::new(ISS, "k1");
    let (with, _, _) = validator_with("", issuer.jwks(), Some(secret));
    let token = cobolt_server::dev::mint_dev_token(secret, "dev-user", &["admin"], 600);
    let c = with.validate(&token).await.expect("the development token");
    assert!(c.dev);
    assert_eq!(c.sub, "dev-user");
    assert!(c.roles.contains("admin"));

    let (without, _, _) = validator_with("", issuer.jwks(), None);
    assert_eq!(refused(&without, &token).await.0, codes::TOKEN_INVALID);

    // …and a development token signed with some other secret is no token at all.
    let other = cobolt_server::dev::mint_dev_token("another secret, also long enough", "dev-user", &[], 600);
    assert_eq!(refused(&with, &other).await.0, codes::TOKEN_INVALID);
}

#[tokio::test]
async fn expiry_and_not_before_are_judged_with_a_bounded_skew() {
    let (v, issuer) = ec_validator("leeway_seconds = 60");
    let t = now();
    // Expired 30 s ago, inside the 60 s skew: accepted. 120 s ago: expired.
    assert!(v.validate(&issuer.mint(&claims(json!({"exp": t - 30})))).await.is_ok());
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"exp": t - 120})))).await.0, codes::TOKEN_EXPIRED);
    // Not valid yet: 30 s ahead is inside the skew, 300 s ahead is not.
    assert!(v.validate(&issuer.mint(&claims(json!({"nbf": t + 30})))).await.is_ok());
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"nbf": t + 300})))).await.0, codes::TOKEN_INVALID);
    // No exp at all: a token that never expires is not accepted.
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"exp": null})))).await.0, codes::TOKEN_INVALID);
}

#[tokio::test]
async fn a_token_that_was_verified_is_judged_again_for_expiry_when_it_is_used_again() {
    let (v, issuer) = ec_validator("leeway_seconds = 0");
    let token = issuer.mint(&claims(json!({"exp": now() + 2})));
    let c = v.validate(&token).await.expect("fresh");
    assert!(v.recheck(&c).is_ok());
    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
    // The signature check is cached; the clock is not.
    assert_eq!(refused(&v, &token).await.0, codes::TOKEN_EXPIRED);
    assert_eq!(v.recheck(&c).unwrap_err().code, codes::TOKEN_EXPIRED);
}

#[tokio::test]
async fn the_issuer_and_the_audience_must_be_this_applications() {
    let (v, issuer) = ec_validator("");
    assert!(v.validate(&issuer.mint(&claims(json!({})))).await.is_ok());
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"iss": "https://elsewhere.example.test/pool"})))).await.0, codes::TOKEN_INVALID);
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"aud": "another-api"})))).await.0, codes::TOKEN_INVALID);
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"aud": null})))).await.0, codes::TOKEN_INVALID);
    // An audience list is fine when this application is in it.
    assert!(v.validate(&issuer.mint(&claims(json!({"aud": ["x", AUD]})))).await.is_ok());
    // The client id stands in for the audience (Keycloak's azp).
    assert!(v.validate(&issuer.mint(&claims(json!({"aud": null, "azp": CLIENT})))).await.is_ok());
    // No subject, no caller.
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"sub": null})))).await.0, codes::TOKEN_INVALID);
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"sub": ""})))).await.0, codes::TOKEN_INVALID);
}

#[tokio::test]
async fn an_identity_token_is_never_an_access_token() {
    let (v, issuer) = ec_validator("");
    for marker in [json!({"at_hash": "abc"}), json!({"nonce": "n-0S6_WzA2Mj"}), json!({"token_use": "id"}), json!({"typ": "ID"}), json!({"typ": "Refresh"})] {
        let (code, message) = refused(&v, &issuer.mint(&claims(marker.clone()))).await;
        assert_eq!(code, codes::TOKEN_INVALID, "{marker}");
        assert!(message.contains("not an access token") || message.contains("identity token"), "{marker}: {message}");
    }
}

#[tokio::test]
async fn cognito_wants_token_use_access_and_the_app_client() {
    let (v, issuer) = ec_validator("profile = \"cognito\"");
    // A Cognito access token: no `aud`, the app client in `client_id`, groups in `cognito:groups`.
    let ok = issuer.mint(&claims(json!({"aud": null, "token_use": "access", "client_id": CLIENT, "cognito:groups": ["staff", "admin"]})));
    let c = v.validate(&ok).await.expect("a Cognito access token");
    assert!(c.roles.contains("admin") && c.roles.contains("staff"), "{:?}", c.roles);
    // The ID token of the same user, same pool, same client.
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"aud": CLIENT, "token_use": "id", "client_id": CLIENT})))).await.0, codes::TOKEN_INVALID);
    // An access token with no token_use is not recognised as one.
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"aud": null, "client_id": CLIENT})))).await.0, codes::TOKEN_INVALID);
    // Another application's client in the same pool.
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"aud": null, "token_use": "access", "client_id": "someone-else"})))).await.0, codes::TOKEN_INVALID);
}

#[tokio::test]
async fn keycloak_wants_a_bearer_and_reads_the_realm_roles() {
    let (v, issuer) = ec_validator("profile = \"keycloak\"");
    let ok = issuer.mint(&claims(json!({"typ": "Bearer", "azp": CLIENT, "realm_access": {"roles": ["offline_access", "clerk"]}})));
    let c = v.validate(&ok).await.expect("a Keycloak access token");
    assert!(c.roles.contains("clerk"), "{:?}", c.roles);
    assert_eq!(c.flat.get("realm_access.roles").map(String::as_str), Some("offline_access,clerk"));
    // typ ID is Keycloak's identity token; no typ at all is not a Bearer.
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"typ": "ID"})))).await.0, codes::TOKEN_INVALID);
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({})))).await.0, codes::TOKEN_INVALID);
}

#[tokio::test]
async fn entra_wants_the_api_audience_and_scp_or_roles() {
    let (v, issuer) = ec_validator("profile = \"entra\"");
    let scoped = issuer.mint(&claims(json!({"scp": "Orders.Read Orders.Write"})));
    assert!(v.validate(&scoped).await.is_ok());
    let app_role = issuer.mint(&claims(json!({"roles": ["Orders.Admin"]})));
    let c = v.validate(&app_role).await.expect("an app-role token");
    assert!(c.roles.contains("Orders.Admin"));
    // An ID token for the same audience has neither scp nor roles.
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({})))).await.0, codes::TOKEN_INVALID);
    // The client id does not stand in for the audience on Entra.
    assert_eq!(refused(&v, &issuer.mint(&claims(json!({"aud": null, "azp": CLIENT, "scp": "x"})))).await.0, codes::TOKEN_INVALID);
}

#[tokio::test]
async fn the_roles_come_from_the_claims_the_configuration_names() {
    let (v, issuer) = ec_validator("roles_claims = [\"resource_access.prc-api.roles\", \"department\"]");
    let t = issuer.mint(&claims(json!({"resource_access": {"prc-api": {"roles": ["a", "b"]}}, "department": "sales support"})));
    let c = v.validate(&t).await.unwrap();
    assert_eq!(c.roles.iter().cloned().collect::<Vec<_>>(), vec!["a", "b", "sales", "support"]);
    assert!(c.has_any_role(&["b".to_string()]));
    assert!(!c.has_any_role(&["admin".to_string()]));
    assert!(c.has_any_role(&[]), "a program with no role requirement lets everybody in");
}

#[tokio::test]
async fn what_is_not_a_token_is_refused_before_anything_is_fetched() {
    let issuer = Issuer::new(ISS, "k1");
    let (v, fetcher, _) = validator_with("", issuer.jwks(), None);
    assert_eq!(refused(&v, "").await.0, codes::TOKEN_MISSING);
    for t in ["x", "a.b", "a.b.c.d", &"a.".repeat(20_000)] {
        assert_eq!(refused(&v, t).await.0, codes::TOKEN_INVALID, "{}", &t[..t.len().min(20)]);
    }
    // Three parts that are not base64 JSON.
    assert_eq!(refused(&v, "####.####.####").await.0, codes::TOKEN_INVALID);
    assert_eq!(*fetcher.hits.lock().unwrap(), 0, "none of them reached the key fetch");
}

#[tokio::test]
async fn keys_are_fetched_once_and_a_flood_of_unknown_key_ids_cannot_amplify() {
    let issuer = Issuer::new(ISS, "k1");
    let (v, fetcher, jwks) = validator_with("jwks_min_refresh_seconds = 30", issuer.jwks(), None);
    for _ in 0..5 {
        assert!(v.validate(&issuer.access("u", AUD, 600, json!({}))).await.is_ok());
    }
    assert_eq!(jwks.fetch_count(), 1, "five tokens, one fetch");
    // 200 tokens naming keys that do not exist: refused, and the issuer is not asked again.
    let stranger = Issuer::new(ISS, "no-such-key");
    for i in 0..200 {
        let t = stranger.access(&format!("u{i}"), AUD, 600, json!({}));
        assert_eq!(refused(&v, &t).await.0, codes::TOKEN_INVALID);
    }
    assert_eq!(jwks.fetch_count(), 1, "200 unknown kids, still one fetch (min refresh is 30 s)");
    assert_eq!(*fetcher.hits.lock().unwrap(), 1);
}

struct Failing(std::sync::Mutex<bool>, String);

impl JwksFetcher for Failing {
    fn fetch(&self, _uri: &str) -> Result<String, String> {
        if *self.0.lock().unwrap() {
            Err("the issuer is down".to_string())
        } else {
            Ok(self.1.clone())
        }
    }
}

#[tokio::test]
async fn an_issuer_outage_does_not_stop_callers_whose_keys_are_known() {
    let issuer = Issuer::new(ISS, "k1");
    let down = Arc::new(Failing(std::sync::Mutex::new(false), issuer.jwks()));
    // A zero lifetime: every lookup wants a refresh, and may have one.
    let jwks = Arc::new(Jwks::new(format!("{ISS}/.well-known/jwks.json"), 0, 0, down.clone()));
    let text = format!("[auth]\nissuer = [\"{ISS}\"]\naudiences = [\"{AUD}\"]\n");
    let resolved = ServerConfig::from_toml(&text).unwrap().resolve(RunMode::Development, std::path::Path::new(".")).unwrap();
    let v = TokenValidator::new(resolved.auth, Some(jwks), None);
    assert!(v.validate(&issuer.access("first", AUD, 600, json!({}))).await.is_ok());
    *down.0.lock().unwrap() = true;
    assert!(v.validate(&issuer.access("second", AUD, 600, json!({}))).await.is_ok(), "the last keys keep serving while the issuer is down");

    // …but a backend that never got keys has nothing to serve.
    let never = Arc::new(Failing(std::sync::Mutex::new(true), String::new()));
    let jwks = Arc::new(Jwks::new(format!("{ISS}/.well-known/jwks.json"), 0, 0, never));
    let resolved = ServerConfig::from_toml(&text).unwrap().resolve(RunMode::Development, std::path::Path::new(".")).unwrap();
    let v = TokenValidator::new(resolved.auth, Some(jwks), None);
    let (code, message) = refused(&v, &issuer.access("third", AUD, 600, json!({}))).await;
    assert_eq!(code, codes::TOKEN_INVALID);
    assert!(message.contains("unavailable"), "{message}");
}
