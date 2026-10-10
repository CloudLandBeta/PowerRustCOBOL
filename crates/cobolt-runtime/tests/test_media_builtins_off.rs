// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 092 R13 — a build WITHOUT the `images` (or `http`) feature still
//! compiles the three built-ins and answers them with `NOFEATURE`, instead of
//! failing to link or falling through to "unknown program".
//!
//! Only meaningful in such a build, so every test here is compiled only when
//! its feature is OFF:
//!
//! ```text
//! cargo test -p cobolt-runtime --no-default-features --test test_media_builtins_off
//! ```

#![allow(unused)]

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp =
        Interpreter::new_with_channels(result.program.expect("no program"), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

#[cfg(not(feature = "images"))]
#[test]
fn without_the_images_feature_both_picture_built_ins_answer_nofeature() {
    let dir = std::env::temp_dir().join(format!("prc-image-off-{}", std::process::id()));
    let target = dir.join("out.png");
    let out = run(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NO-IMAGES.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A       PIC X(12) VALUE "UNSET".
       01 WS-B       PIC X(12) VALUE "UNSET".
       01 WS-WIDTH   PIC 9(6).
       01 WS-HEIGHT  PIC 9(6).
       01 WS-FORMAT  PIC X(8).
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"IMAGE-FIT" ( "in.jpg" "{target}" 40 40 WS-A )
           COBOL::"IMAGE-INFO" ( "in.jpg" WS-WIDTH WS-HEIGHT WS-FORMAT WS-B )
           DISPLAY "A=[" WS-A "]"
           DISPLAY "B=[" WS-B "]"
           STOP RUN.
"#,
        target = target.display()
    ));
    assert!(out.contains(&"A=[NOFEATURE   ]".to_owned()), "{out:?}");
    assert!(out.contains(&"B=[NOFEATURE   ]".to_owned()), "{out:?}");
    assert!(!target.exists());
}

#[cfg(not(feature = "http"))]
#[test]
fn without_the_http_feature_http_download_answers_nofeature() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NO-HTTP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CODE    PIC 9(3) VALUE 999.
       01 WS-STATUS  PIC X(12) VALUE "UNSET".
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"HTTP-DOWNLOAD" ( "http://127.0.0.1:9/a.bin" "a.bin" WS-CODE WS-STATUS )
           DISPLAY "CODE=[" WS-CODE "]"
           DISPLAY "STATUS=[" WS-STATUS "]"
           STOP RUN.
"#,
    );
    assert!(out.contains(&"CODE=[000]".to_owned()), "{out:?}");
    assert!(out.contains(&"STATUS=[NOFEATURE   ]".to_owned()), "{out:?}");
    assert!(!std::path::Path::new("a.bin").exists());
}
