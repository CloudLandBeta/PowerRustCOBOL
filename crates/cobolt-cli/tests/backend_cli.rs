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

const WHOAMI: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. WHOAMI.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-SUB PIC X(40).\nLINKAGE SECTION.\n01 LK-ME.\n    05 WHO PIC X(40).\nPROCEDURE DIVISION RETURNING LK-ME.\nMAIN.\n    COBOL::\"WS-CLAIM\" ( \"sub\" WS-SUB )\n    MOVE WS-SUB TO WHO\n    GOBACK.\n";

const CALLER: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALLER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 PONG.
          05 REPLY PIC X(4).
       01 ME.
          05 WHO PIC X(40).
       01 WS-CLASS PIC X(16).
       01 WS-CODE  PIC 9(5).
       01 WS-TEXT  PIC X(80).
       PROCEDURE DIVISION.
       MAIN.
           CALL \"PING\" WITH WEBSOCKET LINKAGE RETURNING PONG
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( WS-CLASS WS-CODE WS-TEXT )
                   DISPLAY \"ping failed \" WS-CLASS WS-CODE
               NOT ON EXCEPTION
                   DISPLAY \"ping=\" REPLY
           END-CALL
           CALL \"WHOAMI\" WITH WEBSOCKET LINKAGE RETURNING ME
               ON EXCEPTION
                   COBOL::\"WS-LAST-ERROR\" ( WS-CLASS WS-CODE WS-TEXT )
                   DISPLAY \"whoami failed \" WS-CLASS WS-CODE
               NOT ON EXCEPTION
                   DISPLAY \"who=\" WHO
           END-CALL
           STOP RUN.
";

/// Start `rcrun backend run` on `dir`; returns the child, its address and its development token.
fn start_dev_backend(dir: &Path) -> (std::process::Child, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(["backend", "run"]).arg(dir).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("starts");
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let (mut url, mut token) = (String::new(), String::new());
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
    // The pipe is kept open for the child by leaking the reader: dropping it would close stdout under the server.
    std::mem::forget(lines);
    (child, url, token)
}

#[test]
fn a_program_run_by_rcrun_calls_its_backend_with_the_token_the_development_server_printed() {
    let dir = project("caller");
    std::fs::write(dir.join("backend").join("exports.toml"), format!("{EXPORTS}\n[[export]]\nprogram = \"WHOAMI\"\n")).unwrap();
    std::fs::write(dir.join("backend").join("src").join("whoami.cbl"), WHOAMI).unwrap();
    std::fs::write(dir.join("caller.cbl"), CALLER).unwrap();
    let (mut backend, url, token) = start_dev_backend(&dir);
    assert!(url.starts_with("ws://127.0.0.1:"), "{url}");

    // With the backend's address and token in the environment, both calls are answered, the
    // second by a program that read the caller's subject from the validated token.
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(["run", "caller.cbl"]).current_dir(&dir).env("COBOLT_BACKEND_URL", &url).env("COBOLT_BACKEND_TOKEN", &token).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("ping=pong"), "{text}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(text.contains("who=developer"), "{text}");

    // With no token the public program still answers and the protected one is refused (2000, authentication).
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(["run", "caller.cbl"]).current_dir(&dir).env("COBOLT_BACKEND_URL", &url).env_remove("COBOLT_BACKEND_TOKEN").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("ping=pong"), "{text}");
    assert!(text.contains("whoami failed AUTHENTICATION") && text.contains("02000"), "{text}");

    // With no backend configured at all, the call fails visibly, never silently (1003, transport).
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(["run", "caller.cbl"]).current_dir(&dir).env_remove("COBOLT_BACKEND_URL").env_remove("COBOLT_BACKEND_TOKEN").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("ping failed TRANSPORT") && text.contains("01003"), "{text}");

    let _ = backend.kill();
    let _ = backend.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

/// WASM-003 T2 (R22; C5): the Backend a new Web project is created with checks
/// clean and starts as it is — its `exports.toml` and its development
/// `backend.toml` are the scaffold's, untouched.
#[test]
fn a_new_web_projects_backend_checks_clean_and_answers_its_health_probe() {
    let dir = std::env::temp_dir().join(format!("prc-cli-backend-scaffold-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let request = cobolt_project_tools::create::CreateRequest::new("Shop").template("web");
    cobolt_project_tools::create::create_project_with(&dir, &request).unwrap();

    let (code, out, err) = rcrun(&["check"], &dir);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("contract 1.0.0: 1 exported program(s)") && out.trim_end().ends_with("ok"), "{out}");

    // backend.toml names a fixed loopback port; --listen moves it so the test never collides.
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .args(["backend", "run", "--listen", "127.0.0.1:0"])
        .arg(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("starts");
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let started = Instant::now();
    let mut url = String::new();
    while started.elapsed() < Duration::from_secs(60) {
        match lines.next() {
            Some(Ok(l)) if l.starts_with("backend listening on ") => {
                url = l["backend listening on ".len()..].to_string();
                break;
            }
            Some(_) => {}
            None => break,
        }
    }
    if url.is_empty() {
        let _ = child.kill();
        let mut e = String::new();
        let _ = child.stderr.take().unwrap().read_to_string(&mut e);
        panic!("the scaffolded backend did not start: {e}");
    }
    let port: u16 = url["ws://127.0.0.1:".len()..].split('/').next().unwrap().parse().unwrap();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(s, "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut raw = String::new();
    let _ = s.read_to_string(&mut raw);
    assert!(raw.starts_with("HTTP/1.1 200"), "{raw}");
    println!("new Web project: `rcrun backend check` → {:?}; `backend run` up on {url} in {:.0} ms; /health 200", out.lines().next().unwrap_or(""), started.elapsed().as_secs_f64() * 1000.0);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
