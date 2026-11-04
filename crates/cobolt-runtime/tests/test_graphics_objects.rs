// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-018 T17: a COBOL program, parsed from source, draws on a Canvas2D, makes a Physics2D world
//! and takes its handle back into a data item; the form's end releases everything.
#![cfg(feature = "graphics")]

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

#[test]
fn a_program_draws_reads_a_pixel_and_takes_a_handle_back() {
    let src = r##"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GFX.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PIXEL PIC 9(12).
       01 WS-WORLD PIC 9(9).
       01 WS-BODIES PIC 9(9).
       PROCEDURE DIVISION.
           CANVAS-1::SetFillColor("#2060C0").
           CANVAS-1::FillRect(10 10 40 20).
           MOVE CANVAS-1::GetPixel(20 20) TO WS-PIXEL.
           MOVE PHYS-1::CreateWorld(0 -9.81) TO WS-WORLD.
           MOVE PHYS-1::GetBodyCount(WS-WORLD) TO WS-BODIES.
           DISPLAY "PIXEL=" WS-PIXEL.
           DISPLAY "BODIES=" WS-BODIES.
           STOP RUN.
"##;
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", result.diagnostics);
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.expect("program"), event_rx, state_tx, display_tx);
    interp.seed_objects(vec![
        ("CANVAS-1".to_owned(), "Canvas2D".to_owned(), vec![("Width".to_owned(), "100".to_owned()), ("Height".to_owned(), "50".to_owned())]),
        ("PHYS-1".to_owned(), "Physics2D".to_owned(), vec![]),
    ]);
    interp.run().expect("run");
    let shown: Vec<String> = display_rx.try_iter().map(|d| format!("{d:?}")).collect();
    let all = shown.join("\n");
    println!("T17: displayed {shown:?}; objects live after the run {}", interp.graphics_object_count());
    assert!(all.contains("543211775"), "the pixel 0x2060C0FF was read back: {all}");
    assert!(all.contains("BODIES=000000000"), "the handle went into WS-WORLD and came back: {all}");
    assert_eq!(interp.graphics_object_count(), 0, "the form's end released them");
}
