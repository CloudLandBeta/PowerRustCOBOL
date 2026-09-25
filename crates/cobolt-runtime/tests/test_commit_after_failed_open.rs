// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COMMIT` and `ROLLBACK` after an `OPEN` that failed.
//!
//! A program that opens a file in a folder that does not exist gets a FILE
//! STATUS back and carries on — and its next `COMMIT` used to panic the
//! interpreter ("file open"), because COMMIT reaches every file the program
//! knows, including the one that never opened. Found by PowerChat (spec 071)
//! when its data folder was missing.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::Interpreter;

#[test]
fn commit_and_rollback_after_a_failed_open_carry_on() {
    let missing = std::env::temp_dir()
        .join(format!("prc-no-such-folder-{}", std::process::id()))
        .join("s.idx");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FAILOPEN.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT S-FILE ASSIGN TO "{}"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS S-NAME FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
       DATA DIVISION.
       FILE SECTION.
       FD  S-FILE.
       01  S-REC.
           05 S-NAME PIC X(20).
           05 S-VAL  PIC X(20).
       WORKING-STORAGE SECTION.
       01 WS-FS PIC XX.
       PROCEDURE DIVISION.
           OPEN I-O S-FILE
           DISPLAY "open " WS-FS
           COMMIT
           DISPLAY "commit ran"
           ROLLBACK
           DISPLAY "rollback ran"
           STOP RUN.
"#,
        missing.display()
    );
    let program = parse(tokenize(&src, SourceFormat::Free)).program.expect("parses");
    let (_e, erx) = mpsc::channel();
    let (s, _srx) = mpsc::channel();
    let (d, drx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, erx, s, d);
    interp.run().expect("the program runs to STOP RUN");
    let out: Vec<String> = drx.try_iter().map(|l| l.trim().to_string()).collect();
    assert_eq!(out.len(), 3, "{out:?}");
    assert!(out[0].starts_with("open ") && out[0] != "open 00", "the OPEN failed: {out:?}");
    assert_eq!(&out[1..], ["commit ran", "rollback ran"]);
    println!("OPEN in a missing folder → {}; COMMIT and ROLLBACK then did nothing, and the program went on", &out[0][5..]);
}
