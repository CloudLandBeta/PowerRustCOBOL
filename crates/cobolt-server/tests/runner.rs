// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The backend runner** (WASM-015 T7, AC3): the test kit starts a backend, signs a test user in, calls it,
//! calls it again as a client one minor behind and one a minor ahead, and shuts it down leaving no task and no
//! port. Every step prints its time. In-process always; against the built binary only with `PRC_WEB_E2E=1`
//! (that mode needs `cargo` and the SDK to build the backend, and is reported as skipped without them).
//!
//! A backend names its issuer's keys with `jwks_uri`, which may be a loopback address
//! (`jwks::address_allowed`): the binary mode needs no `unsafe_*` setting to trust the kit's key set.

use std::time::{Duration, Instant};

use cobolt_server::client::{ClientConfig, StaticToken, WsClient};
use cobolt_server::testkit::*;
use cobolt_wire::call::{CallRequest, Transport};
use cobolt_wire::error::{codes, ErrorClass};
use serde_json::json;
use std::sync::Arc;

const EXPORTS: &str = "[contract]\nversion = \"1.2.0\"\n\n[[export]]\nprogram = \"PING\"\n";
const PING: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. PING.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-PONG.\n    05 REPLY PIC X(4).\nPROCEDURE DIVISION RETURNING LK-PONG.\nMAIN.\n    MOVE \"pong\" TO REPLY\n    GOBACK.\n";

fn client(rig: &Rig, contract: &str, token: Option<String>) -> WsClient {
    let mut cfg = ClientConfig::new(format!("wss://localhost:{}/ws", rig.server.addr.port()), contract);
    cfg.extra_root_pem = Some(rig.cert.cert_pem.clone().into_bytes());
    cfg.origin = Some(ORIGIN.to_string());
    cfg.interface = Some(rig.interface());
    cfg.call_timeout = Duration::from_secs(20);
    if let Some(t) = token {
        cfg.token = Some(Arc::new(StaticToken(t)));
    }
    WsClient::new(cfg)
}

fn ping() -> CallRequest {
    CallRequest { program: "PING".into(), args: vec![], wants_returning: true, idempotency_key: None }
}

/// AC3, in process: start, sign in, one call, one call a minor behind, one a minor ahead, shut down, re-bind.
#[test]
fn start_sign_in_call_skew_and_teardown_in_process() {
    let mut rows: Vec<(String, u128)> = Vec::new();
    let mut at = Instant::now();
    let mut lap = |what: &str, rows: &mut Vec<(String, u128)>, at: &mut Instant| {
        rows.push((what.to_owned(), at.elapsed().as_millis()));
        *at = Instant::now();
    };

    let dir = backend_dir("runner", EXPORTS, &[("ping.cbl", PING)]);
    let rig = rig(dir, "[limits]\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nuser_rate_per_sec = 5000\nuser_burst = 5000\n");
    let addr = rig.server.addr;
    lap("start", &mut rows, &mut at);

    let token = rig.issuer.access("user-1", AUDIENCE, 3600, json!({}));
    lap("sign in (mint a token)", &mut rows, &mut at);

    let mut current = client(&rig, "1.2.0", Some(token.clone()));
    let reply = current.call(&ping()).expect("the current application is served");
    assert!(reply.returning.as_deref().is_some_and(|r| r.contains("pong")), "{reply:?}");
    assert!(reply.warnings.is_empty(), "{:?}", reply.warnings);
    lap("call (1.2.0)", &mut rows, &mut at);

    let mut behind = client(&rig, "1.1.0", Some(token.clone()));
    let reply = behind.call(&ping()).expect("an older minor of the same major is served");
    assert!(reply.warnings.iter().any(|m| m.contains("compatible") && m.contains("1.1.0")), "{:?}", reply.warnings);
    lap("skew call, a minor behind (1.1.0)", &mut rows, &mut at);

    let mut ahead = client(&rig, "1.3.0", Some(token.clone()));
    let e = ahead.call(&ping()).expect_err("a minor ahead of the backend is refused");
    assert_eq!((e.class, e.code), (ErrorClass::Version, codes::CLIENT_TOO_NEW), "{e:?}");
    assert!(e.message.contains("update the backend"), "{}", e.message);
    lap("skew call, a minor ahead (1.3.0)", &mut rows, &mut at);

    drop((current, behind, ahead));
    rig.server.stop();
    lap("shutdown", &mut rows, &mut at);

    // The port is released: the same address binds again.
    let rebound = std::net::TcpListener::bind(addr);
    assert!(rebound.is_ok(), "the port {} was not released: {:?}", addr.port(), rebound.err());
    drop(rebound);
    lap("re-bind the port", &mut rows, &mut at);

    println!("backend runner (in process), AC3");
    for (what, ms) in &rows {
        println!("  {what:<36} {ms:>6} ms");
    }
    println!("  → 6 checks passed, 0 failed (5 steps, 1 refusal, the port re-bound)");
}

/// A key-set server of one: answers every GET with the kit's JWKS on a loopback port, so a built backend
/// trusts the kit's issuer through `jwks_uri` alone (a loopback `http` address is allowed; no `unsafe_*`).
fn serve_jwks(doc: String) -> u16 {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", doc.len(), doc);
        }
    });
    port
}

/// AC3, against the built binary: only with `PRC_WEB_E2E=1` (it builds the backend with `cargo`, which takes
/// minutes). Same sequence as the in-process run; the backend is a real process, stopped at the end.
#[test]
fn start_sign_in_call_skew_and_teardown_against_the_built_binary() {
    if std::env::var("PRC_WEB_E2E").as_deref() != Ok("1") {
        println!("skipped: the built-binary mode runs only with PRC_WEB_E2E=1 (it builds the backend with cargo)");
        return;
    }
    use cobolt_compiler::backend_build::{build_backend, BackendBuildOptions};
    let mut rows: Vec<(String, u128)> = Vec::new();
    let project = std::env::temp_dir().join(format!("prc-runner-binary-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("backend").join("src")).unwrap();
    std::fs::write(project.join("backend").join("exports.toml"), EXPORTS).unwrap();
    std::fs::write(project.join("backend").join("src").join("ping.cbl"), PING).unwrap();
    let staging = std::env::temp_dir().join(format!("prc-runner-binary-build-{}", std::process::id()));
    std::env::set_var("POWERRUSTCOBOL_BUILD_DIR", &staging);
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").canonicalize().unwrap();
    let t = Instant::now();
    let built = build_backend(&project, &BackendBuildOptions { debug: true, verbose: false, workspace_root: Some(workspace), accept_breaking: false });
    let built = match built {
        Ok(b) => b,
        Err(e) => {
            println!("skipped: the backend could not be built here ({e})");
            let _ = std::fs::remove_dir_all(&project);
            let _ = std::fs::remove_dir_all(&staging);
            return;
        }
    };
    rows.push(("build the backend (debug)".into(), t.elapsed().as_millis()));

    let t = Instant::now();
    let cert = test_cert();
    let issuer = Issuer::new("https://idp.example.test/pool", "k1");
    let jwks_port = serve_jwks(issuer.jwks());
    let dir = project.join("backend");
    std::fs::write(dir.join("server-cert.pem"), &cert.cert_pem).unwrap();
    std::fs::write(dir.join("server-key.pem"), &cert.key_pem).unwrap();
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let listen = probe.local_addr().unwrap().port();
    drop(probe);
    let config = dir.join("backend.toml");
    std::fs::write(
        &config,
        format!(
            "[server]\nlisten = \"127.0.0.1:{listen}\"\nallowed_origins = [\"{ORIGIN}\"]\n[tls]\ncert_chain = \"server-cert.pem\"\nprivate_key = \"server-key.pem\"\n[auth]\nissuer = [\"{}\"]\naudiences = [\"{AUDIENCE}\"]\njwks_uri = \"http://127.0.0.1:{jwks_port}/jwks.json\"\n",
            issuer.issuer
        ),
    )
    .unwrap();
    let mut child = std::process::Command::new(&built.binary).arg("--config").arg(&config).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().expect("the backend starts");
    // Ready when the port takes a connection.
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline && std::net::TcpStream::connect(("127.0.0.1", listen)).is_err() {
        std::thread::sleep(Duration::from_millis(100));
    }
    rows.push(("start (process listening)".into(), t.elapsed().as_millis()));

    let token = issuer.access("user-1", AUDIENCE, 3600, json!({}));
    let mk = |contract: &str| {
        let mut cfg = ClientConfig::new(format!("wss://localhost:{listen}/ws"), contract);
        cfg.extra_root_pem = Some(cert.cert_pem.clone().into_bytes());
        cfg.origin = Some(ORIGIN.to_string());
        cfg.call_timeout = Duration::from_secs(20);
        cfg.token = Some(Arc::new(StaticToken(token.clone())));
        WsClient::new(cfg)
    };
    let t = Instant::now();
    let reply = mk("1.2.0").call(&ping()).expect("the current application is served");
    assert!(reply.returning.as_deref().is_some_and(|r| r.contains("pong")), "{reply:?}");
    rows.push(("call (1.2.0)".into(), t.elapsed().as_millis()));
    let t = Instant::now();
    let reply = mk("1.1.0").call(&ping()).expect("a minor behind is served");
    assert!(reply.warnings.iter().any(|m| m.contains("1.1.0")), "{:?}", reply.warnings);
    rows.push(("skew call, a minor behind".into(), t.elapsed().as_millis()));
    let t = Instant::now();
    let e = mk("1.3.0").call(&ping()).expect_err("a minor ahead is refused");
    assert_eq!((e.class, e.code), (ErrorClass::Version, codes::CLIENT_TOO_NEW), "{e:?}");
    rows.push(("skew call, a minor ahead".into(), t.elapsed().as_millis()));

    let t = Instant::now();
    let _ = child.kill();
    let _ = child.wait();
    assert!(std::net::TcpListener::bind(("127.0.0.1", listen)).is_ok(), "the port was not released");
    rows.push(("stop and re-bind the port".into(), t.elapsed().as_millis()));
    println!("backend runner (built binary), AC3");
    for (what, ms) in &rows {
        println!("  {what:<36} {ms:>8} ms");
    }
    println!("  → 5 checks passed, 0 failed");
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(&staging);
}
