// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The built program gives its program the stack the deepest `PERFORM` nesting
//! needs, and a recursion with no end stops with a runtime error and exit code
//! 1, as under `rcrun run` (`cobolt-cli/tests/perform_stack.rs`). It used to
//! overflow the main thread's stack, ending with no runtime error at all.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::{build_project, BuildOptions};

/// A paragraph that performs itself until `WS-N` reaches the first argument;
/// with "call", a nested program that calls itself instead.
const PROGRAM: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. DEEP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-ARG PIC X(10).
01 WS-MAX PIC 9(6).
01 WS-N PIC 9(6) VALUE 0 GLOBAL.
PROCEDURE DIVISION.
MAIN-PARA.
    ACCEPT WS-ARG FROM ARGUMENT-VALUE
    IF WS-ARG = \"call\"
        CALL \"INNER\"
    ELSE
        MOVE FUNCTION NUMVAL(WS-ARG) TO WS-MAX
        PERFORM DIVE
    END-IF
    DISPLAY \"REACHED \" WS-N
    STOP RUN.
DIVE.
    ADD 1 TO WS-N
    IF WS-N < WS-MAX
        PERFORM DIVE
    END-IF.
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
END PROGRAM DEEP.
";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn the_built_program_ends_deep_recursion_with_a_runtime_error() {
    let project = std::env::temp_dir().join(format!("prc-perform-stack-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    let project = project.canonicalize().unwrap();
    std::fs::write(project.join("src").join("deep.cbl"), PROGRAM).unwrap();
    std::fs::write(
        project.join("deep.project.toml"),
        "[project]\nname = \"Deep\"\nversion = \"1.0.0\"\nmain = \"src/deep.cbl\"\n",
    )
    .unwrap();
    // The unoptimised build: its levels are the largest, so it is the one the stack must hold.
    let opts = BuildOptions { verbose: false, workspace_root: Some(workspace_root()), debug: true, ..Default::default() };
    let t = Instant::now();
    let built = build_project(&project.join("deep.project.toml"), &opts).unwrap_or_else(|e| panic!("build failed: {e}"));
    let build_s = t.elapsed().as_secs_f64();

    // (argument, exit code, what the output or the error says)
    let cases = [
        ("300", 0, "REACHED 000300"),
        ("500", 0, "REACHED 000500"),
        ("600", 1, "PERFORM nesting too deep (max 512)"),
        ("call", 1, "CALL nested too deeply"),
    ];
    let started = Instant::now();
    let mut lines = Vec::new();
    let mut failures = Vec::new();
    for (arg, want, says) in cases {
        let out = std::process::Command::new(&built.binary_path)
            .arg(arg)
            .current_dir(&project)
            .output()
            .unwrap_or_else(|e| panic!("cannot run the built program: {e}"));
        let all = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let code = out.status.code();
        if code != Some(want) || !all.contains(says) || all.contains("overflowed its stack") {
            failures.push(format!("{arg}: exit {code:?} (want {want}), want {says:?} in {all:?}"));
        }
        lines.push(format!("{arg} {}", code.map_or("none".to_string(), |c| c.to_string())));
    }
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    let n = cases.len();
    let _ = std::fs::remove_dir_all(&project);

    println!("Built program: deep PERFORM and CALL");
    println!("  cases (exit code seen): {}", lines.join(", "));
    println!("  forms: PERFORM of the paragraph being performed (300, 500, 600 levels), a nested program that CALLs itself");
    println!("  build {build_s:.1} s (debug); runs {n}, {ms:.1} ms");
    println!("  PASS {} FAIL {}", n - failures.len(), failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
