// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for visual-object **method execution**:
//!   `Label-1::SetCaption("Hi")` · `INVOKE obj "SetText" USING …`
//!   `MOVE obj::GetText() TO X` · `IF CheckBox-1::IsChecked() = "1"`
//! Methods are sugar over property get/set; setters also notify the UI thread.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{Interpreter, StateUpdate};

fn run(src: &str) -> (Vec<String>, Vec<StateUpdate>) {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}", result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx)  = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    (display_rx.try_iter().collect(), state_rx.try_iter().collect())
}

const SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-X PIC X(20).
       PROCEDURE DIVISION.
      *> setter (inline) then getter (expression)
           Label-1::SetCaption("Hello").
           MOVE Label-1::GetCaption() TO WS-X.
           DISPLAY "CAP=[" WS-X "]".
      *> INVOKE form + getter
           INVOKE TextBox-1 "SetText" USING "World".
           MOVE TextBox-1::GetText() TO WS-X.
           DISPLAY "TXT=[" WS-X "]".
      *> boolean method + method as a condition
           CheckBox-1::SetChecked("1").
           IF CheckBox-1::IsChecked() = "1"
               DISPLAY "CHECKED"
           END-IF.
      *> list method + count getter
           ListBox-1::AddItem("a").
           ListBox-1::AddItem("b").
           MOVE ListBox-1::GetCount() TO WS-X.
           DISPLAY "N=[" WS-X "]".
      *> universal geometry method
           Button-1::MoveTo(40, 60).
           STOP RUN.
"#;

const SQL_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N PIC X(20).
       PROCEDURE DIVISION.
           INVOKE Db-1 "open" USING ":memory:".
           INVOKE Db-1 "execute" USING "CREATE TABLE c (id INTEGER, name TEXT)".
           INVOKE Db-1 "execute"
               USING "INSERT INTO c (id, name) VALUES (1, 'ANA'), (2, 'BEA')".
           MOVE Db-1::query("SELECT id, name FROM c") TO WS-N.
           DISPLAY "ROWS=[" WS-N "]".
           INVOKE Db-1 "close".
           STOP RUN.
"#;

#[test]
fn methods_set_get_and_notify_ui() {
    let (out, updates) = run(SRC);
    let joined = out.join("\n");
    assert!(joined.contains("CAP=[Hello"), "SetCaption/GetCaption failed: {out:?}");
    assert!(joined.contains("TXT=[World"), "INVOKE SetText/GetText failed: {out:?}");
    assert!(joined.contains("CHECKED"),    "IsChecked condition failed: {out:?}");
    assert!(joined.contains("N=[2"),       "AddItem/GetCount failed: {out:?}");

    // Setters must also notify the UI thread (so a running form updates live).
    // COBOL upper-cases unquoted identifiers, so the control id is normalised.
    let got = |ctrl: &str, prop: &str, v: &str| updates.iter().any(|u|
        u.ctrl_id.eq_ignore_ascii_case(ctrl) && u.prop == prop && u.value == v);
    assert!(got("Label-1", "Caption", "Hello"), "no Caption StateUpdate: {updates:?}");
    assert!(got("TextBox-1", "Text", "World"),  "no Text StateUpdate");
    assert!(got("CheckBox-1", "Checked", "1"),  "no Checked StateUpdate");
    assert!(got("Button-1", "X", "40") && got("Button-1", "Y", "60"), "MoveTo X/Y not set");
}

#[test]
fn sql_widget_methods_run_against_db_engine() {
    let (out, _updates) = run(SQL_SRC);
    let joined = out.join("\n");
    // query() returns the SELECT row count via the live SQLite engine.
    assert!(joined.contains("ROWS=[2"), "SqlDatabase methods failed: {out:?}");
}
