// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 092 R17 — a build WITHOUT the `audio` feature still compiles the five
//! sound built-ins and answers them with `NOFEATURE`, instead of failing to
//! link or falling through to "unknown program".
//!
//! Only meaningful in such a build, so the test is compiled only when the
//! feature is OFF:
//!
//! ```text
//! cargo test -p cobolt-runtime --no-default-features --test test_audio_off
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

#[cfg(not(feature = "audio"))]
#[test]
fn without_the_audio_feature_every_sound_built_in_answers_nofeature() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NO-AUDIO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ST      PIC X(10) VALUE "UNSET".
       01 WS-STATE   PIC X(10) VALUE "UNSET".
       01 WS-POS     PIC 9(7)  VALUE 9.
       01 WS-LEN     PIC 9(7)  VALUE 9.
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           DISPLAY "BEFORE " WS-STATE " " WS-POS " " WS-LEN.
           COBOL::"AUDIO-PLAY" ( "song.mp3" WS-ST ).
           DISPLAY "PLAY " WS-ST.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           DISPLAY "STATE " WS-STATE.
           COBOL::"AUDIO-PAUSE" ( WS-ST ).
           DISPLAY "PAUSE " WS-ST.
           COBOL::"AUDIO-RESUME" ( WS-ST ).
           DISPLAY "RESUME " WS-ST.
           COBOL::"AUDIO-STOP" ( WS-ST ).
           DISPLAY "STOP " WS-ST.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           DISPLAY "AFTER " WS-STATE.
           STOP RUN.
"#,
    );
    println!("{out:#?}");
    assert!(out.contains(&"BEFORE IDLE       0000000 0000000".to_string()), "{out:?}");
    assert!(out.contains(&"PLAY NOFEATURE".to_string()), "{out:?}");
    assert!(out.contains(&"STATE ERROR".to_string()), "a refused PLAY reads like any failed one: {out:?}");
    assert!(out.contains(&"PAUSE NOFEATURE".to_string()), "{out:?}");
    assert!(out.contains(&"RESUME NOFEATURE".to_string()), "{out:?}");
    assert!(out.contains(&"STOP NOFEATURE".to_string()), "{out:?}");
    assert!(out.contains(&"AFTER IDLE".to_string()), "{out:?}");
}
