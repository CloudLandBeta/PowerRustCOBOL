// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Building a project's backend into a headless server binary (WASM-003 R2, R4,
//! R20; WASM-014 R5), end to end: the project is read and checked, its interface
//! description is written, a Cargo project with no window system is generated and
//! built, and the binary that comes out is run.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use cobolt_compiler::backend_build::{build_backend, BackendBuildOptions};

const EXPORTS: &str = r#"
[contract]
version = "1.0.0"

[[export]]
program = "PING"
public = true
"#;

const PING: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PING.
DATA DIVISION.
LINKAGE SECTION.
01 LK-PONG.
    05 REPLY PIC X(4).
PROCEDURE DIVISION RETURNING LK-PONG.
MAIN.
    MOVE "pong" TO REPLY
    GOBACK.
"#;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").canonicalize().expect("workspace root")
}

fn project(name: &str, exports: &str, program: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-backend-build-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("backend").join("src")).unwrap();
    std::fs::write(dir.join("backend").join("exports.toml"), exports).unwrap();
    std::fs::write(dir.join("backend").join("src").join("ping.cbl"), program).unwrap();
    dir
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn get(port: u16, path: &str) -> Option<(u16, String)> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").ok()?;
    let mut raw = String::new();
    s.read_to_string(&mut raw).ok()?;
    let status = raw.split(' ').nth(1)?.parse().ok()?;
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    Some((status, body))
}

struct Killed(std::process::Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_released_contract_is_protected_before_any_compiler_is_started() {
    // The first build records nothing; a release records the interface; a program
    // changed so that it breaks the released one is then refused at once — cargo
    // is never run, so this is quick.
    let dir = project("breaking", EXPORTS, PING);
    let opts = BackendBuildOptions { debug: false, verbose: false, workspace_root: Some(workspace()), accept_breaking: false };

    // Released by hand, as `rcrun backend release` does: the interface is copied
    // under contract/released/<version>.json.
    let backend = cobolt_server::backend::Backend::load(&dir.join("backend")).expect("loads");
    let released = dir.join("backend").join("contract").join("released");
    std::fs::create_dir_all(&released).unwrap();
    std::fs::write(released.join("1.0.0.json"), backend.interface.to_json()).unwrap();

    // The reply loses a field the released contract promised: a breaking change.
    let renamed = PING.replace("REPLY", "ANSWER");
    std::fs::write(dir.join("backend").join("src").join("ping.cbl"), renamed).unwrap();
    let t = Instant::now();
    let err = build_backend(&dir, &opts).expect_err("a breaking change to a released contract must not build");
    let text = err.to_string();
    assert!(text.contains("breaks the last released contract") && text.contains("1.0.0"), "{text}");
    assert!(text.contains("REPLY"), "the message names what broke: {text}");
    assert!(t.elapsed() < Duration::from_secs(20), "refused before cargo: {:?}", t.elapsed());
    // The interface description is still written, so the developer can read it.
    assert!(dir.join("backend").join("contract").join("interface.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_without_a_backend_is_told_what_is_missing() {
    let dir = std::env::temp_dir().join(format!("prc-backend-build-{}-none", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let opts = BackendBuildOptions { debug: true, verbose: false, workspace_root: Some(workspace()), accept_breaking: false };
    let err = build_backend(&dir, &opts).expect_err("no backend");
    assert!(err.to_string().contains("exports.toml"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_built_backend_is_a_binary_that_checks_describes_and_answers_its_health_probe() {
    let dir = project("built", EXPORTS, PING);
    // The generated Cargo project and its target folder go to a folder of this
    // test's own, removed at the end (a backend build is some hundreds of MB).
    let staging = std::env::temp_dir().join(format!("prc-backend-e2e-build-{}", std::process::id()));
    std::env::set_var("POWERRUSTCOBOL_BUILD_DIR", &staging);
    let opts = BackendBuildOptions { debug: true, verbose: false, workspace_root: Some(workspace()), accept_breaking: false };
    let t = Instant::now();
    let built = build_backend(&dir, &opts).expect("the backend builds");
    let build_secs = t.elapsed().as_secs_f64();
    assert!(built.binary.is_file(), "{}", built.binary.display());
    assert_eq!(built.contract, "1.0.0");
    assert_eq!(built.programs, 1);
    assert!(built.interface.is_file());

    // --version
    let v = Command::new(&built.binary).arg("--version").output().unwrap();
    assert!(v.status.success() && String::from_utf8_lossy(&v.stdout).contains("backend (cobolt-server"), "{v:?}");

    // --describe prints what interface.json holds.
    let d = Command::new(&built.binary).arg("--describe").output().unwrap();
    assert!(d.status.success());
    let printed: serde_json::Value = serde_json::from_slice(&d.stdout).expect("the description is JSON");
    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&built.interface).unwrap()).unwrap();
    assert_eq!(printed, written, "the binary and interface.json describe the same contract");
    assert!(printed.to_string().contains("PING"));

    // No configuration: a clear message, exit 2.
    let none = Command::new(&built.binary).args(["--check", "--config"]).arg(dir.join("missing.toml")).output().unwrap();
    assert_eq!(none.status.code(), Some(2), "{none:?}");
    assert!(String::from_utf8_lossy(&none.stderr).contains("needs its configuration"), "{none:?}");

    // A release binary refuses an unsafe setting.
    let health = free_port();
    let base = format!("[server]\nlisten = \"127.0.0.1:0\"\nhealth_listen = \"127.0.0.1:{health}\"\nallow_missing_origin = true\n[auth]\nissuer = [\"https://idp.example.test/pool\"]\naudiences = [\"prc-api\"]\n");
    let unsafe_cfg = dir.join("unsafe.toml");
    std::fs::write(&unsafe_cfg, format!("unsafe_allow_any_origin = true\n{base}")).unwrap();
    let refused = Command::new(&built.binary).args(["--check", "--config"]).arg(&unsafe_cfg).output().unwrap();
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    assert!(String::from_utf8_lossy(&refused.stderr).contains("unsafe_allow_any_origin"), "{refused:?}");

    // A good configuration checks…
    let cfg = dir.join("backend.toml");
    std::fs::write(&cfg, &base).unwrap();
    let ok = Command::new(&built.binary).args(["--check", "--config"]).arg(&cfg).output().unwrap();
    assert!(ok.status.success(), "{ok:?}");
    assert!(String::from_utf8_lossy(&ok.stdout).contains("configuration ok: contract 1.0.0"), "{ok:?}");

    // …and the backend runs and answers its health probe.
    let child = Command::new(&built.binary).args(["--config"]).arg(&cfg).stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("starts");
    let _guard = Killed(child);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut answer = None;
    while Instant::now() < deadline {
        if let Some(a) = get(health, "/health") {
            answer = Some(a);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let (status, body) = answer.expect("the backend answered its health probe");
    assert_eq!(status, 200);
    let h: serde_json::Value = serde_json::from_str(&body).expect("health is JSON");
    assert_eq!(h["status"], "ok");
    assert_eq!(h["contract"], "1.0.0");
    assert_eq!(get(health, "/other").map(|a| a.0), Some(404));

    let size = std::fs::metadata(&built.binary).unwrap().len();
    println!("\n  backend build: {build_secs:.0} s (debug), binary {:.1} MB, health 200 with contract {}", size as f64 / 1e6, h["contract"]);
    drop(_guard);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&staging);
}
