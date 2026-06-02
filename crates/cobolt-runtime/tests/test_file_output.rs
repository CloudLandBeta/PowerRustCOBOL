// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration test for the COBOL-WRITE-FILE / COBOL-APPEND-FILE built-ins.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) {
    let tokens = tokenize(src, SourceFormat::Free);
    let result = parse(tokens);
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let mut i = Interpreter::new(program);
    i.run().expect("run failed");
}

#[test]
fn write_then_append_produces_expected_file() {
    let dir = std::env::temp_dir().join(format!("rcrun-file-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.md");
    let path_str = path.to_string_lossy().to_string();

    // WRITE-FILE truncates/creates with a header; APPEND-FILE adds two rows.
    let src = format!(
        r##"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILE-TEST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PATH    PIC X(256) VALUE "{path}".
       01 WS-HEADER  PIC X(64)  VALUE "# Results".
       01 WS-ROW1    PIC X(64)  VALUE "Width | PASS".
       01 WS-ROW2    PIC X(64)  VALUE "Bold | FAIL".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-WRITE-FILE"  USING WS-PATH WS-HEADER
           CALL "COBOL-APPEND-FILE" USING WS-PATH WS-ROW1
           CALL "COBOL-APPEND-FILE" USING WS-PATH WS-ROW2
           STOP RUN.
    "##,
        path = path_str
    );

    run(&src);

    let contents = std::fs::read_to_string(&path).expect("file should exist");
    assert_eq!(contents, "# Results\nWidth | PASS\nBold | FAIL\n");

    // WRITE-FILE again must truncate (header only).
    let src2 = format!(
        r##"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILE-TEST2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PATH    PIC X(256) VALUE "{path}".
       01 WS-HEADER  PIC X(64)  VALUE "# Fresh".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-WRITE-FILE" USING WS-PATH WS-HEADER
           STOP RUN.
    "##,
        path = path_str
    );
    run(&src2);
    let contents2 = std::fs::read_to_string(&path).unwrap();
    assert_eq!(contents2, "# Fresh\n", "WRITE-FILE must truncate");

    let _ = std::fs::remove_dir_all(&dir);
}
