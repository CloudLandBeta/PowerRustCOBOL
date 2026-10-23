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
        "{GOOD}\n[tls]\ncert_chain = \"c.pem\"\nprivate_key = \"k.pem\"\n[identity]\nprovider = \"cognito\"\n[identity.settings]\nclient_secret = \"super-secret-client-value\"\nregion = \"eu-west-1\"\n[identity.recaptcha]\nsecret = \"recaptcha-secret-value\"\n"
    );
    let cfg = ServerConfig::from_toml(&text).unwrap();
    let r = cfg.resolve(RunMode::Release, &dir).unwrap();
    let dump = format!("{cfg:?} {r:?}");
    for leaked in ["privateprivate", "super-secret-client-value", "recaptcha-secret-value"] {
        assert!(!dump.contains(leaked), "{leaked} is in the dump: {dump}");
    }
    assert!(dump.contains("client_secret"), "the names of the settings may show: {dump}");
    let _ = std::fs::remove_dir_all(&dir);
}
