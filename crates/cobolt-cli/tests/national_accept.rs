// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 AC8 and Q7: `ACCEPT` into a `PIC N(20)` takes the typed text —
//! UTF-8, or Windows-1252 when the operator's console sends it — and
//! `DISPLAY` shows the same characters back. Driven through `rcrun run`
//! with a real stdin, which an in-process test cannot give it.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const PROGRAM: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. ACCEPT-NATIONAL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC N(20).
01 WS-X PIC X(20).
PROCEDURE DIVISION.
MAIN-PARA.
    ACCEPT WS-N
    DISPLAY \"[\" WS-N \"]\"
    ACCEPT WS-X
    DISPLAY \"[\" WS-X \"]\"
    STOP RUN.
";

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_with_input(tag: &str, input: &[u8]) -> Vec<String> {
    let dir = Dir(std::env::temp_dir().join(format!("prc-077-accept-{tag}-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let src = dir.0.join("accept.cbl");
    std::fs::write(&src, PROGRAM).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .args(["run", src.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "rcrun failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).lines().map(|l| l.trim_end_matches('\r').to_string()).collect()
}

#[test]
fn ac8_accept_utf8_into_a_national_item() {
    let out = run_with_input("utf8", "Olá, João\nabc\n".as_bytes());
    assert_eq!(out, vec!["[Olá, João           ]", "[abc                 ]"]);
}

#[test]
fn q7_windows_1252_typed_text_is_read_as_characters() {
    // "Olá, João" as a Windows console without UTF-8 sends it.
    let out = run_with_input("cp1252", b"Ol\xE1, Jo\xE3o\nabc\n");
    assert_eq!(out[0], "[Olá, João           ]");
}
