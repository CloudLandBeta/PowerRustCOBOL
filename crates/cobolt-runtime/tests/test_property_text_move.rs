// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A control property that happens to be all digits, moved into `PIC X`.
//!
//! A property reads as a number when it looks like one — so `MOVE Ctrl::Value
//! TO WS-N` and arithmetic work — and moving that number into an alphanumeric
//! item used to land it right-justified, as if edited into the receiving
//! field, instead of left-justified like any numeric-to-alphanumeric MOVE. A
//! SideMenu row id made of digits (a timestamp) arrived in `WS-ITEM` behind 24
//! spaces. Found by PowerChat (spec 071).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::Interpreter;

#[test]
fn a_digits_only_property_moves_into_pic_x_left_justified() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROPMOVE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ITEM  PIC X(40).
       01 WS-SHORT PIC X(4).
       01 WS-NUM   PIC 9(16).
       01 WS-DEC   PIC X(10).
       PROCEDURE DIVISION.
           MOVE MENU-1::SelectedItemId TO WS-ITEM
           DISPLAY "[" WS-ITEM "]"
           MOVE MENU-1::SelectedItemId TO WS-SHORT
           DISPLAY "[" WS-SHORT "]"
           MOVE MENU-1::SelectedItemId TO WS-NUM
           DISPLAY "[" WS-NUM "]"
           MOVE MENU-1::Ratio TO WS-DEC
           DISPLAY "[" WS-DEC "]"
           STOP RUN.
"#;
    let program = parse(tokenize(src, SourceFormat::Free)).program.expect("parses");
    let (_e, erx) = mpsc::channel();
    let (s, _srx) = mpsc::channel();
    let (d, drx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, erx, s, d);
    interp.seed_objects(vec![(
        "MENU-1".into(),
        "SideMenu".into(),
        vec![
            ("SelectedItemId".into(), "2026092422060303".into()),
            ("Ratio".into(), "3.25".into()),
        ],
    )]);
    interp.run().expect("runs");
    let out: Vec<String> = drx.try_iter().collect();
    assert_eq!(
        out,
        [
            format!("[2026092422060303{}]", " ".repeat(24)),
            "[2026]".to_string(),
            "[2026092422060303]".to_string(),
            "[3.25      ]".to_string(),
        ],
        "{out:#?}"
    );
}
