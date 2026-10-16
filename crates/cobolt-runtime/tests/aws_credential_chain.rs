// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 103 R6–R8 (AC6, AC7): which credentials sign a request, in what order,
//! and what is said when there are none — against the loopback fake of the
//! hosted endpoint, with the signatures verified by independent code.
//!
//! The AWS providers read the process environment (`AWS_CONFIG_FILE`, `HOME`,
//! `AWS_ACCESS_KEY_ID`, …), so every test here takes one lock and sets it up
//! itself; this file is its own test binary, so nothing else reads it meanwhile.
//! The keys are made-up and labelled as such; none is anyone's.

#![cfg(feature = "aws")]

mod support {
    #[path = "http_fake.rs"]
    pub mod http_fake;
    #[path = "sigv4.rs"]
    pub mod sigv4;
}

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use cobolt_mcp::Mode;
use cobolt_runtime::aws::ops;
use cobolt_runtime::aws::pool::{self, Budget, CallError, ServerKey};
use cobolt_runtime::aws::process::{Credentials, HttpsEndpoint, Launch};
use serde_json::json;
use support::http_fake::HttpFake;
use support::sigv4;

static ENV: Mutex<()> = Mutex::new(());

/// Made-up keys: one per source, so a signature says which source signed.
const FILE_KEY: (&str, &str) = ("AKIDFILEEXAMPLE", "file-secret-EXAMPLE-not-real");
const ENV_KEY: (&str, &str) = ("AKIDENVEXAMPLE", "env-secret-EXAMPLE-not-real");
const SET_KEY: (&str, &str) = ("AKIDSETEXAMPLE", "set-secret-EXAMPLE-not-real");

/// A sandbox for one test: a home with an AWS folder, the environment pointed
/// at it and stripped of everything that could sign, restored when dropped.
struct Sandbox {
    dir: PathBuf,
    _lock: MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

const VARS: [&str; 8] = [
    "HOME",
    "AWS_CONFIG_FILE",
    "AWS_SHARED_CREDENTIALS_FILE",
    "AWS_PROFILE",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_EC2_METADATA_DISABLED",
];

impl Sandbox {
    fn new(tag: &str, config: &str, credentials: &str) -> Sandbox {
        let lock = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("prc-103-chain-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".aws")).unwrap();
        std::fs::write(dir.join(".aws/config"), config).unwrap();
        std::fs::write(dir.join(".aws/credentials"), credentials).unwrap();
        let saved = VARS.iter().map(|v| (*v, std::env::var_os(v))).collect();
        for v in VARS {
            std::env::remove_var(v);
        }
        std::env::set_var("HOME", &dir);
        std::env::set_var("AWS_CONFIG_FILE", dir.join(".aws/config"));
        std::env::set_var("AWS_SHARED_CREDENTIALS_FILE", dir.join(".aws/credentials"));
        // No instance-role lookup on a developer's machine.
        std::env::set_var("AWS_EC2_METADATA_DISABLED", "true");
        Sandbox { dir, _lock: lock, saved }
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        for (name, value) in &self.saved {
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fake() -> HttpFake {
    HttpFake::start(json!({"tools": [{"name": "other_tool", "inputSchema": {"type": "object"}}]}))
}

fn launch(f: &HttpFake, profile: Option<&str>) -> Launch {
    Launch {
        profile: profile.map(str::to_owned),
        region: Some("eu-west-1".into()),
        https: Some(HttpsEndpoint {
            url: f.url.clone(),
            service: "aws-mcp".into(),
            signing_region: "us-east-1".into(),
            metadata: vec![("AWS_REGION".into(), "eu-west-1".into())],
        }),
        ..Launch::default()
    }
}

fn key(tag: &str) -> ServerKey {
    ServerKey { connection: format!("chain-{tag}-{}", std::process::id()), server: "hosted".into() }
}

fn call(k: &ServerKey, l: &Launch) -> Result<String, CallError> {
    let b = Budget { call: Duration::from_secs(10), start: Duration::from_secs(20) };
    pool::call_tool(k, l, Mode::Handshake, "other_tool", json!({"n": 1}), b)
        .map(|r| r.content[0].as_text().unwrap_or_default().to_owned())
}

/// The access key id each logged request was signed with.
fn signed_with(f: &HttpFake, secret: &str) -> Vec<String> {
    f.requests()
        .iter()
        .filter(|r| r.method == "POST")
        .map(|r| {
            sigv4::verify(r, secret, "aws-mcp", "us-east-1")
                .unwrap_or_else(|e| panic!("{:?} does not verify with the expected secret: {e}", r.rpc_method()))
                .key_id
        })
        .collect()
}

fn credentials_file(profile: &str, key: (&str, &str)) -> String {
    format!("[{profile}]\naws_access_key_id = {}\naws_secret_access_key = {}\n", key.0, key.1)
}

// ── order of precedence ───────────────────────────────────────────────────────

#[test]
fn a_named_profile_signs_with_the_keys_in_the_shared_credentials_file() {
    let _sb = Sandbox::new("file", "", &credentials_file("dev", FILE_KEY));
    let f = fake();
    let k = key("file");
    assert!(call(&k, &launch(&f, Some("dev"))).is_ok());
    let ids = signed_with(&f, FILE_KEY.1);
    pool::stop(&k);
    assert!(ids.len() >= 3 && ids.iter().all(|i| i == FILE_KEY.0), "{ids:?}");
    println!("\n  103 R6.2 — profile \"dev\" (static keys in the credentials file): {} requests, all signed by {}", ids.len(), FILE_KEY.0);
}

#[test]
fn with_no_profile_the_default_chain_signs_with_the_environment() {
    let _sb = Sandbox::new("env", "", "");
    std::env::set_var("AWS_ACCESS_KEY_ID", ENV_KEY.0);
    std::env::set_var("AWS_SECRET_ACCESS_KEY", ENV_KEY.1);
    let f = fake();
    let k = key("env");
    assert!(call(&k, &launch(&f, None)).is_ok());
    let ids = signed_with(&f, ENV_KEY.1);
    pool::stop(&k);
    assert!(ids.iter().all(|i| i == ENV_KEY.0), "{ids:?}");
    println!("\n  103 R6.2 — no profile, keys in the environment: {} requests, all signed by {}", ids.len(), ENV_KEY.0);
}

#[test]
fn a_named_profile_wins_over_keys_in_the_environment() {
    let _sb = Sandbox::new("wins", "", &credentials_file("dev", FILE_KEY));
    std::env::set_var("AWS_ACCESS_KEY_ID", ENV_KEY.0);
    std::env::set_var("AWS_SECRET_ACCESS_KEY", ENV_KEY.1);
    let f = fake();
    let k = key("wins");
    assert!(call(&k, &launch(&f, Some("dev"))).is_ok());
    let ids = signed_with(&f, FILE_KEY.1);
    pool::stop(&k);
    assert!(ids.iter().all(|i| i == FILE_KEY.0), "the profile, not the environment: {ids:?}");
    println!("\n  103 R6 — environment keys present, profile \"dev\" named: all {} requests signed by the profile's {}", ids.len(), FILE_KEY.0);
}

#[test]
fn keys_the_program_set_win_over_a_profile() {
    let _sb = Sandbox::new("set", "", &credentials_file("dev", FILE_KEY));
    let f = fake();
    let k = key("set");
    let mut l = launch(&f, None); // with_credentials() clears the profile, as ops::prepare does
    l.credentials = Some(Arc::new(Credentials::new(SET_KEY.0.into(), SET_KEY.1.into(), String::new())));
    assert!(call(&k, &l).is_ok());
    let ids = signed_with(&f, SET_KEY.1);
    pool::stop(&k);
    assert!(ids.iter().all(|i| i == SET_KEY.0), "{ids:?}");
    println!("\n  103 R6.1 — keys set by the program: all {} requests signed by {}, the profile's keys unused", ids.len(), SET_KEY.0);
}

// ── a program that provides credentials, and their refresh ──────────────────────────

#[cfg(unix)]
#[test]
fn a_credential_process_is_asked_again_before_its_credential_expires() {
    use std::os::unix::fs::PermissionsExt;
    let sb = Sandbox::new("process", "", "");
    // The program prints a credential good for 30 seconds — inside the one-minute
    // margin — and a different key id each time it is run; a counter file says how often.
    let script = sb.path("provide.sh");
    let counter = sb.path("runs.txt");
    let body = format!(
        "#!/bin/sh\necho x >> '{c}'\nn=$(wc -l < '{c}' | tr -d ' ')\nexp=$(date -u -v+30S +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d '+30 seconds' +%Y-%m-%dT%H:%M:%SZ)\n\
         printf '{{\"Version\":1,\"AccessKeyId\":\"AKIDPROC%sEXAMPLE\",\"SecretAccessKey\":\"{s}\",\"Expiration\":\"%s\"}}' \"$n\" \"$exp\"\n",
        c = counter.display(),
        s = FILE_KEY.1
    );
    std::fs::write(&script, body).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(sb.path(".aws/config"), format!("[profile proc]\ncredential_process = {}\n", script.display())).unwrap();

    let f = fake();
    let k = key("process");
    let l = launch(&f, Some("proc"));
    assert!(call(&k, &l).is_ok());
    assert!(call(&k, &l).is_ok());
    let ids = signed_with(&f, FILE_KEY.1);
    pool::stop(&k);
    let runs = std::fs::read_to_string(&counter).unwrap().lines().count();
    let distinct: std::collections::BTreeSet<&String> = ids.iter().collect();
    assert_eq!(runs, ids.len(), "a credential about to expire is asked for again before every request");
    assert_eq!(distinct.len(), ids.len(), "each request was signed with the credential just provided: {ids:?}");
    println!("\n  103 R3 — credential_process with a 30 s credential: run {runs} times for {} signed requests, {} distinct key ids", ids.len(), distinct.len());
}

// ── nothing signed in ─────────────────────────────────────────────────────────

#[test]
fn with_nothing_signed_in_the_message_names_the_profile_and_nothing_is_sent() {
    let _sb = Sandbox::new("none", "", "");
    let f = fake();
    let mut table = Vec::new();
    for (profile, tag) in [(Some("dev"), "none-dev"), (None, "none-default")] {
        let e = call(&key(tag), &launch(&f, profile)).unwrap_err();
        let CallError::Start(m) = &e else { panic!("{e:?}") };
        let plain = ops::plain_error(m, profile.unwrap_or(""));
        let expect = profile.unwrap_or("default");
        assert!(plain.contains(&format!("The AWS profile \"{expect}\"")) && plain.contains("aws login"), "{plain}");
        assert!(!plain.contains("AKID") && !m.contains("secret"), "no key in a message: {m}");
        table.push(plain);
    }
    assert_eq!(f.requests().len(), 0, "nothing is sent without a credential");
    println!("\n  103 R7 — no credentials: 2 messages, 0 requests sent:\n    {}", table.join("\n    "));
}

#[test]
fn an_identity_center_profile_that_is_not_signed_in_is_told_to_use_sso_login() {
    let sb = Sandbox::new("sso", "", "");
    std::fs::write(
        sb.path(".aws/config"),
        "[profile corp]\nsso_session = work\nsso_account_id = 111122223333\nsso_role_name = Dev\n\
         [sso-session work]\nsso_start_url = https://work.example/start\nsso_region = us-east-1\nsso_registration_scopes = sso:account:access\n",
    )
    .unwrap();
    let f = fake();
    let e = call(&key("sso"), &launch(&f, Some("corp"))).unwrap_err();
    let CallError::Start(m) = &e else { panic!("{e:?}") };
    let plain = ops::plain_error(m, "corp");
    assert!(plain.contains("aws sso login --profile corp"), "{plain} (raw: {m})");
    assert_eq!(f.requests().len(), 0);
    println!("\n  103 R7 — IAM Identity Center profile with no cached token: \"{plain}\"");
}

#[test]
fn a_refused_credential_is_not_kept() {
    // The credentials file changes after a 403: the next request must read the new one.
    let sb = Sandbox::new("renew", "", &credentials_file("dev", FILE_KEY));
    let f = HttpFake::start(json!({
        "tools": [{"name": "other_tool", "inputSchema": {"type": "object"}}],
        "http_error": {"status": 403, "body": "{\"__type\":\"ExpiredTokenException\"}", "on": "initialize"}
    }));
    let k = key("renew");
    let l = launch(&f, Some("dev"));
    assert!(matches!(call(&k, &l), Err(CallError::Start(_))));
    std::fs::write(sb.path(".aws/credentials"), credentials_file("dev", ENV_KEY)).unwrap();
    assert!(matches!(call(&k, &l), Err(CallError::Start(_))), "still refused by the fake");
    pool::stop(&k);
    let ids: Vec<String> = f
        .requests()
        .iter()
        .map(|r| sigv4::parse_authorization(r.header("authorization").unwrap()).unwrap().key_id)
        .collect();
    assert_eq!(ids, [FILE_KEY.0, ENV_KEY.0], "the second request was signed with the renewed credentials: {ids:?}");
    println!("\n  103 R3 — after a 403 the providers are asked again: {ids:?}");
    let _ = Path::new("");
}
