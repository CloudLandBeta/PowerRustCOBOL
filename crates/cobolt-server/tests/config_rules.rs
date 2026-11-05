// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The configuration's rules (WASM-003 R5, R9-R17; WASM-013 R11): what stops a
//! start, what only warns in development, and that a secret never prints.

use std::path::Path;

use cobolt_server::config::{resolve_secret, RunMode, ServerConfig};

const GOOD: &str = r#"
[server]
listen = "127.0.0.1:8443"
allowed_origins = ["https://app.example.test"]
[auth]
issuer = ["https://idp.example.test/pool"]
audiences = ["prc-api"]
"#;

fn resolve(text: &str, mode: RunMode) -> Result<cobolt_server::config::Resolved, Vec<String>> {
    ServerConfig::from_toml(text).map_err(|e| vec![e])?.resolve(mode, Path::new("."))
}

fn errors(text: &str, mode: RunMode) -> String {
    resolve(text, mode).expect_err("the configuration should be refused").join("\n")
}

#[test]
fn a_good_configuration_resolves_with_the_documented_defaults() {
    let r = resolve(GOOD, RunMode::Release).expect("good");
    assert_eq!(r.listen.to_string(), "127.0.0.1:8443");
    assert_eq!(r.limits.max_frame_bytes, 1024 * 1024);
    assert_eq!(r.limits.call_timeout_ms, 30_000);
    assert_eq!(r.limits.max_calls_per_connection, 8);
    assert_eq!(r.limits.connection_rate, (20, 40));
    assert_eq!(r.limits.file_lock_wait_ms, 5000);
    assert_eq!(r.auth.leeway, 60);
    assert_eq!(r.auth.jwks_uri.as_deref(), Some("https://idp.example.test/pool/.well-known/jwks.json"));
    assert_eq!(r.auth.algorithms.len(), 2, "RS256 and ES256 by default");
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn a_release_start_refuses_every_unsafe_setting_and_a_development_start_only_warns() {
    for setting in ["unsafe_allow_any_origin = true", "unsafe_dev_token_secret = \"0123456789abcdef0123456789abcdef\""] {
        let text = format!("{setting}\n{GOOD}");
        // (A top-level key, written ahead of the first section.)
        let e = errors(&text, RunMode::Release);
        assert!(e.contains(setting.split(' ').next().unwrap()) && e.contains("release backend refuses"), "{setting}: {e}");
        let ok = resolve(&text, RunMode::Development).unwrap_or_else(|e| panic!("{setting}: {e:?}"));
        assert!(ok.warnings.iter().any(|w| w.contains(setting.split(' ').next().unwrap())), "{setting}: {:?}", ok.warnings);
    }
}

#[test]
fn a_misspelt_setting_stops_the_start_instead_of_being_ignored() {
    let e = errors(&GOOD.replace("audiences", "audiencess"), RunMode::Release);
    assert!(e.contains("audiencess") || e.contains("unknown field"), "{e}");
    let e = errors(&format!("unsafe_allow_any_origins = true\n{GOOD}"), RunMode::Release);
    assert!(e.contains("unknown field"), "{e}");
}

/// WASM-003 T4 (R5, Q7; D4): plain `ws://` is served on a loopback address only,
/// in a release start and in a development one alike — there is no relaxation,
/// and a `backend.toml` that still names the removed one is told why.
#[test]
fn plain_ws_is_never_served_on_a_non_loopback_address() {
    for addr in ["0.0.0.0:8443", "192.168.1.20:8443", "[::]:8443", "10.0.0.5:80", "127.0.0.1:8443", "[::1]:8443", "127.0.0.2:8443"] {
        let text = GOOD.replace("127.0.0.1:8443", addr);
        let mut outcomes = Vec::new();
        for mode in [RunMode::Release, RunMode::Development] {
            let r = resolve(&text, mode);
            let loopback = addr.parse::<std::net::SocketAddr>().unwrap().ip().is_loopback();
            match &r {
                Ok(_) => assert!(loopback, "{addr} {mode:?}: plain ws:// served on a network address"),
                Err(e) => {
                    assert!(!loopback, "{addr} {mode:?}: {e:?}");
                    assert!(e.iter().any(|m| m.contains("not a loopback address") && m.contains("TLS")), "{addr}: {e:?}");
                }
            }
            outcomes.push(format!("{mode:?} {}", if r.is_ok() { "served" } else { "refused" }));
        }
        println!("  ws:// on {addr:<18} {}", outcomes.join(", "));
    }
    for mode in [RunMode::Release, RunMode::Development] {
        let e = errors(&format!("unsafe_plain_ws = true\n{}", GOOD.replace("127.0.0.1:8443", "0.0.0.0:8443")), mode);
        assert!(e.contains("`unsafe_plain_ws` was removed") && e.contains("loopback") && !e.contains("unknown field"), "{mode:?}: {e}");
        println!("  {mode:?}, a backend.toml naming unsafe_plain_ws: {e}");
    }
}

#[test]
fn tls_is_all_or_nothing() {
    let half = format!("{GOOD}\n[tls]\ncert_chain = \"cert.pem\"\n");
    let e = errors(&half, RunMode::Release);
    assert!(e.contains("not half of either"), "{e}");
    let both = format!("{GOOD}\n[tls]\ncert_chain = \"cert.pem\"\nprivate_key = \"key.pem\"\npkcs12 = \"id.p12\"\n");
    assert!(errors(&both, RunMode::Release).contains("not half of either"));
    // A certificate that is not on disk is reported by name, not as a crash later.
    let missing = format!("{GOOD}\n[tls]\ncert_chain = \"no-such-cert.pem\"\nprivate_key = \"no-such-key.pem\"\n");
    assert!(errors(&missing, RunMode::Release).contains("tls.cert_chain"), "{}", errors(&missing, RunMode::Release));
}

#[test]
fn no_and_shared_secret_algorithms_are_never_accepted_from_a_token() {
    for alg in ["none", "NONE", "HS256", "HS384", "HS512"] {
        let e = errors(&GOOD.replace("audiences = [\"prc-api\"]", &format!("audiences = [\"prc-api\"]\nalgorithms = [\"{alg}\"]")), RunMode::Release);
        assert!(e.contains("auth.algorithms"), "{alg}: {e}");
    }
    let e = errors(&GOOD.replace("audiences = [\"prc-api\"]", "audiences = [\"prc-api\"]\nalgorithms = [\"RS999\"]"), RunMode::Release);
    assert!(e.contains("unknown algorithm"), "{e}");
    let ok = resolve(&GOOD.replace("audiences = [\"prc-api\"]", "audiences = [\"prc-api\"]\nalgorithms = [\"rs256\", \"PS256\", \"EdDSA\"]"), RunMode::Release).unwrap();
    assert_eq!(ok.auth.algorithms.len(), 3);
}

#[test]
fn the_issuer_the_audience_and_the_skew_are_all_required_and_bounded() {
    assert!(errors(&GOOD.replace("issuer = [\"https://idp.example.test/pool\"]", "issuer = []"), RunMode::Release).contains("auth.issuer is empty"));
    assert!(errors(&GOOD.replace("https://idp", "http://idp"), RunMode::Release).contains("must be an https:// URL"));
    assert!(resolve(&GOOD.replace("https://idp.example.test", "http://127.0.0.1:9000"), RunMode::Release).is_ok(), "http is allowed for a loopback issuer");
    assert!(errors(&GOOD.replace("audiences = [\"prc-api\"]", ""), RunMode::Release).contains("a token for any other application"));
    assert!(errors(&GOOD.replace("audiences = [\"prc-api\"]", "audiences = [\"prc-api\"]\nleeway_seconds = 301"), RunMode::Release).contains("over 300"));
    assert!(resolve(&GOOD.replace("audiences = [\"prc-api\"]", "audiences = [\"prc-api\"]\nleeway_seconds = 300"), RunMode::Release).is_ok());
}

#[test]
fn origins_must_be_listed_and_a_wildcard_is_not_an_origin() {
    assert!(errors(&GOOD.replace("allowed_origins = [\"https://app.example.test\"]", "allowed_origins = [\"*\"]"), RunMode::Release).contains("not an origin"));
    assert!(errors(&GOOD.replace("allowed_origins = [\"https://app.example.test\"]", "allowed_origins = []"), RunMode::Release).contains("no browser could connect"));
    // A backend for non-browser clients says so on purpose.
    assert!(resolve(&GOOD.replace("allowed_origins = [\"https://app.example.test\"]", "allow_missing_origin = true"), RunMode::Release).is_ok());
}

#[test]
fn every_fault_is_reported_together_so_one_start_shows_them_all() {
    let bad = r#"
unsafe_allow_any_origin = true
[server]
listen = "not an address"
allowed_origins = ["*"]
[auth]
issuer = ["http://idp.example.test"]
algorithms = ["none"]
leeway_seconds = 999
[limits]
max_frame_bytes = 10
"#;
    let errs = resolve(bad, RunMode::Release).expect_err("bad");
    assert!(errs.len() >= 7, "{} faults: {errs:#?}", errs.len());
}

#[test]
fn secrets_come_from_the_environment_or_a_file_and_never_print() {
    std::env::set_var("PRC_TEST_SECRET_ABC", "from-the-environment-0123456789");
    assert_eq!(resolve_secret("env:PRC_TEST_SECRET_ABC").unwrap(), "from-the-environment-0123456789");
    assert!(resolve_secret("env:PRC_TEST_SECRET_NOT_SET").unwrap_err().contains("PRC_TEST_SECRET_NOT_SET"));
    let file = std::env::temp_dir().join(format!("prc-secret-{}.txt", std::process::id()));
    std::fs::write(&file, "from-a-file-0123456789abcdef\n").unwrap();
    assert_eq!(resolve_secret(&format!("file:{}", file.display())).unwrap(), "from-a-file-0123456789abcdef");
    let _ = std::fs::remove_file(&file);

    // A development secret read from the environment, resolved and then dumped
    // the way a log line or a panic would dump it: the secret is not in it.
    let text = "unsafe_dev_token_secret = \"env:PRC_TEST_SECRET_ABC\"\n[auth]\nissuer = []\n";
    let cfg = ServerConfig::from_toml(text).unwrap();
    let resolved = cfg.resolve(RunMode::Development, Path::new(".")).unwrap();
    let dump = format!("{cfg:?} {resolved:?}");
    assert!(!dump.contains("from-the-environment"), "{dump}");
    assert!(!dump.contains("PRC_TEST_SECRET_ABC") && dump.contains("Secret(***)"), "{dump}");
    assert_eq!(resolved.dev_token_secret.as_ref().unwrap().expose(), "from-the-environment-0123456789");

    // Too short to be a secret.
    assert!(errors("unsafe_dev_token_secret = \"short\"\n[auth]\nissuer = []\n", RunMode::Development).contains("shorter than 16"));
}

#[test]
fn the_tls_identity_and_the_provider_settings_print_without_their_secrets() {
    let dir = std::env::temp_dir().join(format!("prc-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.pem"), "-----BEGIN CERTIFICATE-----\nMIIBcertcertcert\n-----END CERTIFICATE-----\n").unwrap();
    // A stand-in key (the block markers are assembled, so no key-shaped literal sits in the source).
    let kind = ["PRI", "VATE"].concat();
    std::fs::write(dir.join("k.pem"), format!("-----BEGIN {kind} KEY-----\nMIIEvQprivateprivateprivate\n-----END {kind} KEY-----\n")).unwrap();
    let text = format!(
        "{GOOD}\n[tls]\ncert_chain = \"c.pem\"\nprivate_key = \"k.pem\"\n[identity]\nprovider = \"cognito\"\ncookie_key = \"{COOKIE_KEY}\"\n[identity.settings]\nclient_secret = \"super-secret-client-value\"\nregion = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\nclient_id = \"app-client\"\n[identity.recaptcha]\nsecret = \"recaptcha-secret-value\"\n"
    );
    let cfg = ServerConfig::from_toml(&text).unwrap();
    // Development: the literal secrets are accepted, with a warning (a release start refuses them).
    let r = cfg.resolve(RunMode::Development, &dir).unwrap();
    let dump = format!("{cfg:?} {r:?}");
    for leaked in ["privateprivate", "super-secret-client-value", "recaptcha-secret-value", COOKIE_KEY] {
        assert!(!dump.contains(leaked), "{leaked} is in the dump: {dump}");
    }
    assert!(dump.contains("client_secret"), "the names of the settings may show: {dump}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── [identity] (WASM-009 T10: D3, D7, D13) ────────────────────────────────────

/// 32 bytes, base64: a stand-in sealing key.
const COOKIE_KEY: &str = "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=";

/// A Cognito `[identity]` with its secrets given as `env:` names the test sets.
fn cognito(extra: &str) -> String {
    std::env::set_var("PRC_T10_COOKIE_KEY", COOKIE_KEY);
    std::env::set_var("PRC_T10_CLIENT_SECRET", "client-secret-value");
    std::env::set_var("PRC_T10_RECAPTCHA", "recaptcha-secret-value");
    format!(
        "[server]\nlisten = \"127.0.0.1:8443\"\nallowed_origins = [\"https://app.example.test\"]\n[identity]\nprovider = \"cognito\"\ncookie_key = \"env:PRC_T10_COOKIE_KEY\"\n{extra}\n[identity.settings]\nregion = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\nclient_id = \"app-client\"\nclient_secret = \"env:PRC_T10_CLIENT_SECRET\"\n"
    )
}

#[test]
fn an_identity_section_alone_resolves_and_derives_auth_from_the_provider() {
    use cobolt_server::config::{IdentityFlow, Profile, Provider};
    let r = resolve(&cognito(""), RunMode::Release).expect("an [identity] needs no [auth]");
    let i = r.identity.as_ref().unwrap();
    assert_eq!(i.provider, Provider::Cognito);
    assert_eq!(i.flow, IdentityFlow::InApp, "auto is Cognito's in-app flow (O1, the working assumption)");
    assert_eq!(i.client_id, "app-client");
    assert_eq!(i.client_secret.as_ref().map(|s| s.expose()), Some("client-secret-value"));
    assert!(!i.settings.contains_key("client_secret"), "a secret is not kept with the plain settings");
    assert_eq!(i.origins, vec!["https://app.example.test".to_string()], "the origins default to server.allowed_origins");
    assert_eq!(i.attempts_per_minute, 10);
    assert!(r.auth_from_identity);
    assert_eq!(r.auth.profile, Profile::Cognito);
    assert_eq!(r.auth.client_ids, vec!["app-client".to_string()]);
    assert_eq!(i.discovery_uri(), "https://cognito-idp.eu-west-1.amazonaws.com/eu-west-1_Pool/.well-known/openid-configuration");
    // An explicit [auth] wins.
    let text = format!("{}\n[auth]\nissuer = [\"https://idp.example.test/pool\"]\naudiences = [\"prc-api\"]\n", cognito(""));
    let r = resolve(&text, RunMode::Release).unwrap();
    assert!(!r.auth_from_identity);
    assert_eq!(r.auth.issuers, vec!["https://idp.example.test/pool".to_string()]);
    assert_eq!(r.auth.audiences, vec!["prc-api".to_string()]);
}

#[test]
fn the_provider_and_the_flow_must_be_known_and_each_provider_names_what_it_needs() {
    let e = errors(&cognito("").replace("\"cognito\"", "\"okta\""), RunMode::Release);
    assert!(e.contains("okta") && e.contains("cognito, keycloak, entra, entra-external"), "{e}");
    let e = errors(&cognito("flow = \"popup\""), RunMode::Release);
    assert!(e.contains("popup") && e.contains("auto, in-app, hosted, native"), "{e}");
    let e = errors(&cognito("").replace("user_pool_id = \"eu-west-1_Pool\"\n", ""), RunMode::Release);
    assert!(e.contains("identity.settings.user_pool_id is required for provider cognito"), "{e}");
    let e = errors(&cognito("").replace("region = ", "realm = "), RunMode::Release);
    assert!(e.contains("identity.settings.realm is not a cognito setting"), "{e}");
    // Keycloak's sign-in is its own page: in-app is refused; hosted (auto) resolves.
    let kc = cognito("").replace("\"cognito\"", "\"keycloak\"").replace("region = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\n", "issuer = \"https://kc.example.test/realms/shop\"\n");
    let r = resolve(&kc, RunMode::Release).expect("keycloak");
    assert_eq!(r.identity.as_ref().unwrap().flow, cobolt_server::config::IdentityFlow::Hosted);
    let e = errors(&kc.replace("[identity]\n", "[identity]\nflow = \"in-app\"\n"), RunMode::Release);
    assert!(e.contains("not available for keycloak"), "{e}");
    let e = errors(&kc.replace("https://kc.example.test", "http://kc.example.test"), RunMode::Release);
    assert!(e.contains("must be an https:// address"), "{e}");
    // Cognito's hosted flow needs the managed login's domain.
    let e = errors(&cognito("flow = \"hosted\""), RunMode::Release);
    assert!(e.contains("needs identity.settings.domain"), "{e}");
    // Entra and Entra External ID.
    let entra = cognito("").replace("\"cognito\"", "\"entra\"").replace("region = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\n", "tenant = \"contoso.onmicrosoft.com\"\n");
    let r = resolve(&entra, RunMode::Release).expect("entra");
    assert_eq!(r.identity.as_ref().unwrap().discovery_uri(), "https://login.microsoftonline.com/contoso.onmicrosoft.com/v2.0/.well-known/openid-configuration");
    assert_eq!(r.auth.audiences, vec!["app-client".to_string()], "Entra's aud is the client id unless an audience is set");
    let ext = cognito("flow = \"native\"").replace("\"cognito\"", "\"entra-external\"").replace("region = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\n", "tenant_subdomain = \"shop\"\n");
    let r = resolve(&ext, RunMode::Release).expect("entra-external native");
    assert_eq!(r.identity.as_ref().unwrap().flow, cobolt_server::config::IdentityFlow::Native);
    assert_eq!(r.identity.as_ref().unwrap().authority(), "https://shop.ciamlogin.com/shop.onmicrosoft.com");
}

#[test]
fn a_secret_written_in_the_file_stops_a_release_start_and_only_warns_in_development() {
    for (find, put, name) in [
        ("client_secret = \"env:PRC_T10_CLIENT_SECRET\"", "client_secret = \"written-in-the-file\"", "identity.settings.client_secret"),
        ("cookie_key = \"env:PRC_T10_COOKIE_KEY\"", "cookie_key = \"AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=\"", "identity.cookie_key"),
    ] {
        let text = cognito("").replace(find, put);
        let e = errors(&text, RunMode::Release);
        assert!(e.contains(name) && e.contains("env:NAME or file:PATH"), "{name}: {e}");
        let r = resolve(&text, RunMode::Development).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(r.warnings.iter().any(|w| w.contains(name)), "{name}: {:?}", r.warnings);
    }
    let text = format!("{}[identity.recaptcha]\nsite_key = \"site-key-public\"\nsecret = \"literal-recaptcha\"\n", cognito(""));
    assert!(errors(&text, RunMode::Release).contains("identity.recaptcha.secret"));
    let text = format!("{}[identity.recaptcha]\nsite_key = \"site-key-public\"\nsecret = \"env:PRC_T10_RECAPTCHA\"\nmin_score = 0.7\n", cognito(""));
    let r = resolve(&text, RunMode::Release).unwrap();
    let rc = r.identity.as_ref().unwrap().recaptcha.as_ref().unwrap();
    assert_eq!((rc.site_key.as_deref(), rc.secret.expose(), rc.min_score), (Some("site-key-public"), "recaptcha-secret-value", 0.7));
    assert!(!format!("{r:?}").contains("recaptcha-secret-value"));
}

#[test]
fn without_a_32_byte_cookie_key_the_backend_does_not_start() {
    let e = errors(&cognito("").replace("cookie_key = \"env:PRC_T10_COOKIE_KEY\"\n", ""), RunMode::Development);
    assert!(e.contains("identity.cookie_key is missing") && e.contains("no unsealed fallback"), "{e}");
    std::env::set_var("PRC_T10_SHORT_KEY", "AQIDBA==");
    let e = errors(&cognito("").replace("PRC_T10_COOKIE_KEY", "PRC_T10_SHORT_KEY"), RunMode::Release);
    assert!(e.contains("not a 32-byte key") && e.contains("4 bytes"), "{e}");
    let e = errors(&cognito("").replace("PRC_T10_COOKIE_KEY", "PRC_T10_NOT_SET_ANYWHERE"), RunMode::Release);
    assert!(e.contains("identity.cookie_key"), "{e}");
    assert_eq!(cobolt_server::config::cookie_key_bytes(&"ab".repeat(32)).unwrap(), [0xab; 32], "64 hex digits are a key too");
}

#[test]
fn the_identity_origins_are_origins_and_one_is_needed_in_release() {
    let e = errors(&cognito("origins = [\"https://app.example.test/path\", \"https://*.example.test\", \"http://app.example.test\"]"), RunMode::Release);
    assert_eq!(e.matches("is not an origin").count(), 3, "{e}");
    let r = resolve(&cognito("origins = [\"https://shop.example.test\", \"http://localhost:5173\"]"), RunMode::Release).unwrap();
    assert_eq!(r.identity.unwrap().origins.len(), 2);
    let e = errors(&cognito("").replace("allowed_origins = [\"https://app.example.test\"]\n", ""), RunMode::Release);
    assert!(e.contains("identity.origins is empty"), "{e}");
}

/// A provider's metadata, served by a stand-in fetcher: the issuer and the key address are taken
/// from it, never guessed — Keycloak's keys are under `/protocol/openid-connect/certs`, Entra's
/// under `/discovery/v2.0/keys`.
struct Metadata(std::collections::HashMap<String, String>);

impl cobolt_server::jwks::JwksFetcher for Metadata {
    fn fetch(&self, uri: &str) -> Result<String, String> {
        self.0.get(uri).cloned().ok_or_else(|| format!("404 {uri}"))
    }
}

fn metadata(uri: &str, issuer: &str, jwks: &str) -> Metadata {
    let doc = serde_json::json!({
        "issuer": issuer,
        "jwks_uri": jwks,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "end_session_endpoint": format!("{issuer}/logout"),
    });
    Metadata([(uri.to_string(), doc.to_string())].into_iter().collect())
}

#[test]
fn discovery_takes_the_issuer_and_the_keys_from_the_provider_metadata() {
    let kc = cognito("").replace("\"cognito\"", "\"keycloak\"").replace("region = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\n", "issuer = \"https://kc.example.test/realms/shop\"\n");
    let mut r = resolve(&kc, RunMode::Release).unwrap();
    let iss = "https://kc.example.test/realms/shop";
    let certs = "https://kc.example.test/realms/shop/protocol/openid-connect/certs";
    r.discover(&metadata(&format!("{iss}/.well-known/openid-configuration"), iss, certs)).unwrap();
    assert_eq!(r.auth.issuers, vec![iss.to_string()]);
    assert_eq!(r.auth.jwks_uri.as_deref(), Some(certs), "the key address is the metadata's, not <issuer>/.well-known/jwks.json");

    let entra = cognito("").replace("\"cognito\"", "\"entra\"").replace("region = \"eu-west-1\"\nuser_pool_id = \"eu-west-1_Pool\"\n", "tenant = \"contoso.onmicrosoft.com\"\n");
    let mut r = resolve(&entra, RunMode::Release).unwrap();
    let tenant_iss = "https://login.microsoftonline.com/0f1e2d3c-0000-4000-8000-000000000001/v2.0";
    let keys = "https://login.microsoftonline.com/0f1e2d3c-0000-4000-8000-000000000001/discovery/v2.0/keys";
    let uri = "https://login.microsoftonline.com/contoso.onmicrosoft.com/v2.0/.well-known/openid-configuration";
    r.discover(&metadata(uri, tenant_iss, keys)).unwrap();
    assert_eq!(r.auth.issuers, vec![tenant_iss.to_string()], "Entra names the tenant by its id");
    assert_eq!(r.auth.jwks_uri.as_deref(), Some(keys));

    // Metadata that names another issuer, or an http key address, stops the start.
    let mut r = resolve(&kc, RunMode::Release).unwrap();
    let e = r.discover(&metadata(&format!("{iss}/.well-known/openid-configuration"), "https://evil.example.test/realms/shop", certs)).unwrap_err();
    assert!(e.contains("not the configured keycloak"), "{e}");
    let e = r.discover(&metadata(&format!("{iss}/.well-known/openid-configuration"), iss, "http://kc.example.test/certs")).unwrap_err();
    assert!(e.contains("not https"), "{e}");
    let e = r.discover(&Metadata(Default::default())).unwrap_err();
    assert!(e.contains("the provider's metadata"), "{e}");
    // With an explicit [auth], Keycloak still reads its metadata — the adapter's endpoints are there
    // (T17) — but [auth] stays as written.
    let explicit = format!("{kc}\n[auth]\nissuer = [\"{iss}\"]\naudiences = [\"prc-api\"]\njwks_uri = \"https://keys.example.test/k\"\n");
    let mut r = resolve(&explicit, RunMode::Release).unwrap();
    assert!(r.discover(&Metadata(Default::default())).is_err(), "Keycloak's endpoints come from its metadata");
    r.discover(&metadata(&format!("{iss}/.well-known/openid-configuration"), iss, certs)).unwrap();
    assert_eq!(r.auth.jwks_uri.as_deref(), Some("https://keys.example.test/k"), "[auth] as written");
    assert_eq!(r.discovery.as_ref().map(|d| d.token_endpoint.as_str()), Some(format!("{iss}/token").as_str()));
    // Cognito's endpoints follow from its region and domain: with [auth] given there is nothing to read.
    let mut r = resolve(&format!("{}\n[auth]\nissuer = [\"{iss}\"]\naudiences = [\"prc-api\"]\n", cognito("")), RunMode::Release).unwrap();
    r.discover(&Metadata(Default::default())).expect("no fetch for Cognito when [auth] is given");
    assert!(r.discovery.is_none());
}


#[test]
fn an_existing_backend_toml_without_contract_still_starts() {
    // WASM-014 T3: a project written before sessions has no `[contract]`; it starts, with the answers kept in memory.
    let r = resolve(GOOD, RunMode::Release).expect("good");
    assert_eq!(r.contract.store, cobolt_server::config::IdempotencyStore::Memory);
    assert_eq!(r.contract.session_idle, 900);
    assert!(r.contract.path.ends_with("data/contract/idempotency.redb"), "{:?}", r.contract.path);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn contract_section_reads_both_store_modes_and_refuses_an_unknown_key() {
    let server = format!("{GOOD}\n[contract]\nidempotency_store = \"server\"\nsession_idle_seconds = 60\nidempotency_path = \"/var/lib/app/idem.redb\"\n");
    let r = resolve(&server, RunMode::Release).expect("server mode");
    assert_eq!(r.contract.store, cobolt_server::config::IdempotencyStore::Server);
    assert_eq!((r.contract.session_idle, r.contract.path.to_str()), (60, Some("/var/lib/app/idem.redb")));
    let memory = format!("{GOOD}\n[contract]\nidempotency_store = \"memory\"\n");
    assert_eq!(resolve(&memory, RunMode::Release).expect("memory mode").contract.store, cobolt_server::config::IdempotencyStore::Memory);
    // A misspelt key or value stops the start, as every other section does.
    for bad in ["idempotency_stor = \"server\"", "idempotency_store = \"browser\"", "session_idle = 5"] {
        let text = format!("{GOOD}\n[contract]\n{bad}\n");
        assert!(ServerConfig::from_toml(&text).is_err(), "`{bad}` must be refused");
    }
    println!("[contract]: memory and server read; 3 misspellings refused");
}
