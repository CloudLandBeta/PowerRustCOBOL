// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun run` ends with the exit code `RETURN-CODE` holds when the program
//! is over: after `STOP RUN`, after `GOBACK`, and after the last statement of
//! a program that has neither. It used to end with 0 whatever the program
//! had set. The built program's half is `cobolt-compiler/tests/return_code_exit_build.rs`.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

/// One program, its case chosen by the first command-line argument.
const PROGRAM: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. RC-EXIT.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-MODE PIC X(10).
PROCEDURE DIVISION.
MAIN-PARA.
    ACCEPT WS-MODE FROM ARGUMENT-VALUE
    DISPLAY \"MODE \" WS-MODE
    EVALUATE WS-MODE
        WHEN \"unset\"
            STOP RUN
        WHEN \"stop\"
            MOVE 4 TO RETURN-CODE
            STOP RUN
        WHEN \"goback\"
            MOVE 16 TO RETURN-CODE
            GOBACK
        WHEN \"reset\"
            MOVE 12 TO RETURN-CODE
            MOVE 0 TO RETURN-CODE
            STOP RUN
        WHEN \"arith\"
            MOVE 5 TO RETURN-CODE
            ADD 3 TO RETURN-CODE
            STOP RUN
        WHEN \"fraction\"
            MOVE 7.9 TO RETURN-CODE
            STOP RUN
        WHEN \"wide\"
            MOVE 300 TO RETURN-CODE
            DISPLAY \"NO-NEWLINE\" WITH NO ADVANCING
            STOP RUN
        WHEN \"negative\"
            MOVE -1 TO RETURN-CODE
            STOP RUN
        WHEN \"fall\"
            MOVE 8 TO RETURN-CODE
    END-EVALUATE
    DISPLAY \"END \" WS-MODE.
";

/// Each case, the exit code the shell sees, and how the output ends. Linux and
/// macOS keep the low eight bits of an exit status; Windows keeps the value.
fn cases() -> Vec<(&'static str, i32, &'static str)> {
    vec![
        ("unset", 0, "MODE unset"),
        ("stop", 4, "MODE stop"),
        ("goback", 16, "MODE goback"),
        ("reset", 0, "MODE reset"),
        ("arith", 8, "MODE arith"),
        ("fraction", 7, "MODE fraction"),
        ("wide", if cfg!(windows) { 300 } else { 44 }, "NO-NEWLINE"),
        ("negative", if cfg!(windows) { -1 } else { 255 }, "MODE negative"),
        ("fall", 8, "END fall"),
    ]
}

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rcrun_run_ends_with_the_exit_code_return_code_holds() {
    let dir = Dir(std::env::temp_dir().join(format!("prc-return-code-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let src = dir.0.join("rc-exit.cbl");
    std::fs::write(&src, PROGRAM).unwrap();

    let started = Instant::now();
    let mut lines = Vec::new();
    let mut failures = Vec::new();
    for (mode, want, last) in cases() {
        let out = Command::new(env!("CARGO_BIN_EXE_rcrun"))
            .args(["run", src.to_str().unwrap(), mode])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let got = out.status.code();
        // `contains`, not equality: `WITH NO ADVANCING` also writes the
        // console's cursor codes around its text.
        let ends = stdout.trim_end().lines().last().unwrap_or("").trim_end();
        let ok = got == Some(want) && ends.contains(last);
        if !ok {
            failures.push(format!(
                "{mode}: exit {got:?} (want {want}), output ends {ends:?} (want {last:?}); stderr: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        lines.push(format!("{mode} {}", got.map_or("none".to_string(), |c| c.to_string())));
    }
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    let n = cases().len();

    println!("rcrun run: RETURN-CODE is the exit code");
    println!("  cases (exit code seen): {}", lines.join(", "));
    println!("  forms: STOP RUN, GOBACK, the end of the program, MOVE, ADD, a fraction, 300 and -1, no RETURN-CODE");
    println!("  runs {n}, {ms:.1} ms ({:.1} ms per run)", ms / n as f64);
    println!("  PASS {} FAIL {}", n - failures.len(), failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
