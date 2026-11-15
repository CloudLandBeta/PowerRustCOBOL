// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The built program ends with the exit code `RETURN-CODE` holds, as
//! `rcrun run` does (`cobolt-cli/tests/return_code_exit.rs`): after `STOP RUN`,
//! after `GOBACK`, and after the last statement of a program that has neither.
//! It used to end with 0 whatever the program had set.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_compiler::{build_project, BuildOptions};

/// The same program as `rcrun run`'s test: the case is the first argument.
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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

#[test]
#[ignore = "builds a real binary; run with --ignored"]
fn the_built_program_ends_with_the_exit_code_return_code_holds() {
    let project = std::env::temp_dir().join(format!("prc-return-code-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    let project = project.canonicalize().unwrap();
    std::fs::write(project.join("src").join("rc-exit.cbl"), PROGRAM).unwrap();
    std::fs::write(
        project.join("rcexit.project.toml"),
        "[project]\nname = \"Rcexit\"\nversion = \"1.0.0\"\nmain = \"src/rc-exit.cbl\"\n",
    )
    .unwrap();
    // The unoptimised build: this test proves behaviour, not speed.
    let opts = BuildOptions { verbose: false, workspace_root: Some(workspace_root()), debug: true, ..Default::default() };
    let t = Instant::now();
    let built = build_project(&project.join("rcexit.project.toml"), &opts).unwrap_or_else(|e| panic!("build failed: {e}"));
    let build_s = t.elapsed().as_secs_f64();

    let started = Instant::now();
    let mut lines = Vec::new();
    let mut failures = Vec::new();
    for (mode, want, last) in cases() {
        let out = std::process::Command::new(&built.binary_path)
            .arg(mode)
            .current_dir(&project)
            .output()
            .unwrap_or_else(|e| panic!("cannot run the built program: {e}"));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let got = out.status.code();
        // `contains`, not equality: `WITH NO ADVANCING` also writes the
        // console's cursor codes around its text.
        let ends = stdout.trim_end().lines().last().unwrap_or("").trim_end();
        if got != Some(want) || !ends.contains(last) {
            failures.push(format!(
                "{mode}: exit {got:?} (want {want}), output ends {ends:?} (want {last:?}); stderr: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        lines.push(format!("{mode} {}", got.map_or("none".to_string(), |c| c.to_string())));
    }
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    let n = cases().len();
    let _ = std::fs::remove_dir_all(&project);

    println!("Built program: RETURN-CODE is the exit code");
    println!("  cases (exit code seen): {}", lines.join(", "));
    println!("  forms: STOP RUN, GOBACK, the end of the program, MOVE, ADD, a fraction, 300 and -1, no RETURN-CODE");
    println!("  build {build_s:.1} s (debug); runs {n}, {ms:.1} ms ({:.1} ms per run)", ms / n as f64);
    println!("  PASS {} FAIL {}", n - failures.len(), failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
