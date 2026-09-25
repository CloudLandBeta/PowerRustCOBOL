// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A relative `ASSIGN` path starts at the application's folder when one is
//! anchored — the project folder under Run Form, the executable's in a built
//! application — not at wherever the process was launched. Its own test binary:
//! the anchor is process-wide.
//!
//! Found 2026-09-25: PowerChat keeps its settings in `data/settings.idx`, and
//! under the IDE (launched from another folder) the file could never be
//! created, so the language flags wrote a setting that was never kept.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", result.diagnostics);
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().collect()
}

#[test]
fn a_relative_assign_starts_at_the_application_folder() {
    let app = std::env::temp_dir().join(format!("prc-anchor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&app);
    std::fs::create_dir_all(app.join("data")).unwrap();
    let outside = std::env::temp_dir().join(format!("prc-anchor-abs-{}.txt", std::process::id()));
    cobolt_forms::assets::set_base(&app);
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ANCHOR.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT LOG-FILE ASSIGN TO "data/log.txt"
               ORGANIZATION IS LINE SEQUENTIAL FILE STATUS IS FS1.
           SELECT SET-FILE ASSIGN TO WS-SET-PATH
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS SET-NAME FILE STATUS IS FS2
               STORAGE MODE IS DISK.
           SELECT ABS-FILE ASSIGN TO "{outside}"
               ORGANIZATION IS LINE SEQUENTIAL FILE STATUS IS FS3.
       DATA DIVISION.
       FILE SECTION.
       FD LOG-FILE.
       01 LOG-REC PIC X(20).
       FD SET-FILE.
       01 SET-REC.
          05 SET-NAME  PIC X(10).
          05 SET-VALUE PIC X(10).
       FD ABS-FILE.
       01 ABS-REC PIC X(20).
       WORKING-STORAGE SECTION.
       01 WS-SET-PATH PIC X(60) VALUE "data/settings.idx".
       01 FS1 PIC XX.
       01 FS2 PIC XX.
       01 FS3 PIC XX.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT LOG-FILE
           MOVE "kept" TO LOG-REC
           WRITE LOG-REC
           CLOSE LOG-FILE
           OPEN OUTPUT SET-FILE
           MOVE "LANG" TO SET-NAME
           MOVE "pt" TO SET-VALUE
           WRITE SET-REC
           CLOSE SET-FILE
           OPEN INPUT SET-FILE
           MOVE "LANG" TO SET-NAME
           READ SET-FILE
           DISPLAY "LANG=" SET-VALUE " " FS2
           CLOSE SET-FILE
           OPEN OUTPUT ABS-FILE
           WRITE ABS-REC FROM "absolute"
           CLOSE ABS-FILE
           DISPLAY FS1 FS3
           STOP RUN.
"#,
        outside = outside.display()
    );
    let out = run(&src);
    assert_eq!(out, vec!["LANG=pt         00", "0000"]);
    assert_eq!(std::fs::read_to_string(app.join("data/log.txt")).unwrap().trim_end(), "kept");
    assert!(app.join("data/settings.idx").exists(), "the keyed file is in the application's data folder");
    assert!(std::fs::read_to_string(&outside).unwrap().starts_with("absolute"), "an absolute path is left as written");
    let _ = std::fs::remove_dir_all(&app);
    let _ = std::fs::remove_file(&outside);
}
