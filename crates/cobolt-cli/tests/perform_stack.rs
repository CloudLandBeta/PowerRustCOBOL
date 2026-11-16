// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun run` gives a program the stack the deepest `PERFORM` nesting needs,
//! and a recursion with no end stops with a runtime error and exit code 1. It
//! used to overflow the main thread's stack after about 200 levels (in an
//! unoptimised build), ending the process with no runtime error at all. The
//! interpreter's half is `cobolt-runtime/tests/test_stack_guard.rs`.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

/// A paragraph that performs itself until `WS-N` reaches the first argument.
const PERFORM_DEEP: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. DEEP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ARG PIC X(10).
01 WS-MAX PIC 9(6).
01 WS-N PIC 9(6) VALUE 0.
PROCEDURE DIVISION.
MAIN-PARA.
    ACCEPT WS-ARG FROM ARGUMENT-VALUE
    MOVE FUNCTION NUMVAL(WS-ARG) TO WS-MAX
    PERFORM DIVE
    DISPLAY \"REACHED \" WS-N
    STOP RUN.
DIVE.
    ADD 1 TO WS-N
    IF WS-N < WS-MAX
        PERFORM DIVE
    END-IF.
";

/// A nested program that calls itself, with no end in sight.
const CALL_DEEP: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. OUTER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(6) VALUE 0 GLOBAL.
PROCEDURE DIVISION.
MAIN-PARA.
    CALL \"INNER\"
    DISPLAY \"REACHED \" WS-N
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. INNER COMMON.
PROCEDURE DIVISION.
INNER-PARA.
    ADD 1 TO WS-N
    IF WS-N < 999999
        CALL \"INNER\"
    END-IF
    GOBACK.
END PROGRAM INNER.
END PROGRAM OUTER.
";

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn deep_recursion_ends_with_a_runtime_error_not_a_crash() {
    let dir = Dir(std::env::temp_dir().join(format!("prc-perform-stack-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let perform = dir.0.join("deep.cbl");
    let call = dir.0.join("deepcall.cbl");
    std::fs::write(&perform, PERFORM_DEEP).unwrap();
    std::fs::write(&call, CALL_DEEP).unwrap();

    // (case, program, argument, exit code, what the output or the error says)
    let cases: [(&str, &PathBuf, &str, i32, &str); 4] = [
        ("PERFORM 300 deep", &perform, "300", 0, "REACHED 000300"),
        ("PERFORM 500 deep", &perform, "500", 0, "REACHED 000500"),
        ("PERFORM 600 deep", &perform, "600", 1, "PERFORM nesting too deep (max 512)"),
        ("CALL without end", &call, "", 1, "CALL nested too deeply"),
    ];
    let started = Instant::now();
    let mut lines = Vec::new();
    let mut failures = Vec::new();
    for (what, src, arg, want, says) in cases {
        let t = Instant::now();
        let out = Command::new(env!("CARGO_BIN_EXE_rcrun")).args(["run", src.to_str().unwrap(), arg]).output().unwrap();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let all = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let code = out.status.code();
        let ok = code == Some(want) && all.contains(says) && !all.contains("overflowed its stack");
        lines.push(format!("  {what}: exit {code:?}, {ms:.0} ms"));
        if !ok {
            failures.push(format!("{what}: exit {code:?} (want {want}), want {says:?} in {all:?}"));
        }
    }

    let n = cases.len();
    println!("rcrun run: deep PERFORM and CALL");
    println!("{}", lines.join("\n"));
    println!("  forms: PERFORM of the paragraph being performed (300, 500, 600 levels), a nested program that CALLs itself");
    println!("  runs {n}, {:.0} ms", started.elapsed().as_secs_f64() * 1000.0);
    println!("  PASS {} FAIL {}", n - failures.len(), failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
