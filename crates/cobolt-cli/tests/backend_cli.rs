// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun backend` (WASM-003 R20, WASM-014 R5): the developer's loop for a
//! backend, driven through the real binary - describe, check, release, a break
//! caught, and the development server answering.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"PING\"\npublic = true\n";

fn ping(member: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. PING.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-PONG.\n    05 {member} PIC X(4).\nPROCEDURE DIVISION RETURNING LK-PONG.\nMAIN.\n    MOVE \"pong\" TO {member}\n    GOBACK.\n"
    )
}

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-cli-backend-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("backend").join("src")).unwrap();
    std::fs::write(dir.join("backend").join("exports.toml"), EXPORTS).unwrap();
    std::fs::write(dir.join("backend").join("src").join("ping.cbl"), ping("REPLY")).unwrap();
    dir
}

fn rcrun(args: &[&str], dir: &Path) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).arg("backend").args(args).arg(dir).output().expect("rcrun runs");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn describe_check_and_release_guard_the_contract() {
    let dir = project("loop");

    let (code, out, _) = rcrun(&["describe"], &dir);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"PING\"") && out.contains("\"REPLY\""), "{out}");
    let written = dir.join("backend").join("contract").join("interface.json");
    assert!(written.is_file(), "describe writes interface.json, which the frontend's Check reads");

    let (code, out, _) = rcrun(&["check"], &dir);
    assert_eq!(code, 0);
    assert!(out.contains("contract 1.0.0") && out.contains("no released contract") && out.contains("ok"), "{out}");

    let (code, out, _) = rcrun(&["release"], &dir);
    assert_eq!(code, 0, "{out}");
    let released = dir.join("backend").join("contract").join("released").join("1.0.0.json");
    assert!(released.is_file());
    assert_eq!(rcrun(&["release"], &dir).0, 0, "releasing the same interface again changes nothing");

    // A change that breaks the released contract: the answer loses REPLY.
    std::fs::write(dir.join("backend").join("src").join("ping.cbl"), ping("ANSWER")).unwrap();
    let (code, _, err) = rcrun(&["check"], &dir);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("breaking") && err.contains("REPLY") && err.contains("RETURNING"), "{err}");
    let (code, _, err) = rcrun(&["release"], &dir);
    assert_eq!(code, 1);
    assert!(err.contains("released already with a different interface"), "{err}");

    // Raising the major version is the deliberate way through.
    std::fs::write(dir.join("backend").join("exports.toml"), EXPORTS.replace("1.0.0", "2.0.0")).unwrap();
    let (code, out, err) = rcrun(&["check"], &dir);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("allowed by the new major version"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_without_a_backend_and_an_unknown_verb_are_told_so() {
    let dir = std::env::temp_dir().join(format!("prc-cli-backend-none-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (code, _, err) = rcrun(&["check"], &dir);
    assert_eq!(code, 1);
    assert!(err.contains("exports.toml"), "{err}");
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(["backend", "frobnicate"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_development_server_prints_its_address_and_a_token_and_answers_its_health_probe() {
    let dir = project("run");
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .args(["backend", "run"])
        .arg(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("starts");
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut url = String::new();
    let mut token = String::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        match lines.next() {
            Some(Ok(l)) if l.starts_with("backend listening on ") => url = l["backend listening on ".len()..].to_string(),
            Some(Ok(l)) if l.starts_with("development token") => {
                token = lines.next().and_then(|t| t.ok()).unwrap_or_default();
                break;
            }
            Some(_) => {}
            None => break,
        }
    }
    assert!(url.starts_with("ws://127.0.0.1:"), "a development run listens on loopback, plain: {url}");
    assert_eq!(token.split('.').count(), 3, "a JSON Web Token: {token}");
    let port: u16 = url["ws://127.0.0.1:".len()..].split('/').next().unwrap().parse().unwrap();

    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(s, "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut raw = String::new();
    let _ = s.read_to_string(&mut raw);
    assert!(raw.starts_with("HTTP/1.1 200"), "{raw}");
    assert!(raw.contains("\"status\":\"ok\"") && raw.contains("\"contract\":\"1.0.0\""), "{raw}");

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
