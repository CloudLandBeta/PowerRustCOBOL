// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `DISPLAY … WITH NO ADVANCING` keeps the line and writes nothing but its
//! text. It used to be read as an empty screen phrase: the text went out with
//! the console's cursor codes around it (`ESC[1;1H` … `ESC[0m`), at the
//! terminal's top-left corner. `NO ADVANCING` without `WITH`, and
//! `UPON … WITH NO ADVANCING`, were not affected.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

const PROGRAM: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. NO-ADV.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9 VALUE 7.
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY \"A\" WITH NO ADVANCING
    DISPLAY \"B\" NO ADVANCING
    DISPLAY \"C\" UPON CONSOLE WITH NO ADVANCING
    DISPLAY \"D\" WS-N WITH NO ADVANCING END-DISPLAY
    DISPLAY \"E\"
    DISPLAY \"F\" WITH NO ADVANCING.
    STOP RUN.
";

/// Everything the program writes, byte for byte: six texts, one line end.
const EXPECTED: &str = "ABCD7E\nF";

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn display_with_no_advancing_keeps_the_line_and_writes_only_its_text() {
    let dir = Dir(std::env::temp_dir().join(format!("prc-no-advancing-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let src = dir.0.join("no-adv.cbl");
    std::fs::write(&src, PROGRAM).unwrap();

    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(["run", src.to_str().unwrap()]).output().unwrap();
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    let stdout = String::from_utf8_lossy(&out.stdout);

    let checks = [
        ("the run ends with 0", out.status.code() == Some(0)),
        ("the output is the six texts and one line end", stdout == EXPECTED),
        ("no cursor code (ESC) is written", !stdout.contains('\u{1b}')),
    ];
    let failed: Vec<_> = checks.iter().filter(|(_, ok)| !ok).map(|(what, _)| *what).collect();

    println!("rcrun run: DISPLAY … WITH NO ADVANCING");
    println!("  forms: WITH NO ADVANCING, NO ADVANCING, UPON CONSOLE WITH NO ADVANCING, two operands with END-DISPLAY, a plain DISPLAY, the last DISPLAY of the run");
    println!("  output {} bytes, expected {} bytes: {stdout:?}", stdout.len(), EXPECTED.len());
    println!("  run {ms:.1} ms");
    println!("  PASS {} FAIL {}", checks.len() - failed.len(), failed.len());
    assert!(
        failed.is_empty(),
        "failed: {failed:?}; output {stdout:?}; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
