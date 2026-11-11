// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Arithmetic whose receiver is a control's property (WASM-015 AC4, found while testing a Timer's
//! `onTick`): `ADD 1 TO NUD-1::Value` reads the property's text as a number, adds, and writes the
//! sum back, as the desktop does. The module raised COBOL run-time error 104 (a bad operand) for it.

mod common;

use cobolt_wasm::host::FormHost;
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;
use cobolt_wasm_abi::scan::scan_module;

/// A one-form project in the shape `cobolt-codegen` writes, with one handler on `BTN-1`'s click.
fn one_form(body: &str) -> (Vec<u8>, cobolt_wasm_abi::manifest::Manifest) {
    let s = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. MAIN-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n\
         01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'MAIN-FORM'.\n\
         PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    PERFORM COBOL-EVENT-LOOP\n    STOP RUN.\n\
         COBOL-EVENT-LOOP.\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
         \x20       EVALUATE COBOL-CONTROL-ID\n            WHEN \"BTN-1\"\n                EVALUATE COBOL-EVENT-ID\n\
         \x20                   WHEN \"onClick\"\n                        CALL \"BTN-1--ONCLICK\"\n                END-EVALUATE\n\
         \x20       END-EVALUATE\n    END-PERFORM.\n\
         IDENTIFICATION DIVISION.\nPROGRAM-ID. BTN-1--ONCLICK IS COMMON PROGRAM.\nPROCEDURE DIVISION.\n{body}    GOBACK.\nEND PROGRAM BTN-1--ONCLICK.\n\
         END PROGRAM MAIN-FORM.\n"
    );
    let program = common::parse(&s);
    let spec = FormSpec {
        name: "MAIN-FORM".into(),
        main: true,
        instances: 1,
        program: &program,
        controls: vec![
            ("BTN-1".into(), "Button".into()),
            ("NUD-1".into(), "NumericUpDown".into()),
        ],
        bindings: vec![("BTN-1".into(), "onClick".into())],
        layers: vec![],
        lines: vec![],
    };
    let c = compile_project(
        &[spec],
        &Options {
            multi_memory: false,
            name: "arith-prop".into(),
            ..Options::default()
        },
    )
    .unwrap_or_else(|e| panic!("compile_project failed: {e}"));
    let m = scan_module(&c.wasm).unwrap().embedded_manifest().unwrap();
    (c.wasm, m)
}

/// Run `body` as the click handler and return what it displayed.
fn click(body: &str) -> Vec<String> {
    let (wasm, m) = one_form(body);
    let click = m.event_index("onClick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    assert_eq!(host.event(inst, 3, click, &[]), 0, "{:?}", host.outcome());
    common::wasm_lines(&host.outcome())
}

/// What the desktop interpreter DISPLAYs for the same statements: the control is registered by its
/// first use, as an undeclared item is, so no form is needed to ask it.
fn desktop(body: &str) -> Vec<String> {
    common::run_interpreter(&format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nPROCEDURE DIVISION.\n{body}    STOP RUN.\n"
    ))
}

/// `ADD 1 TO NUD-1::Value` and its kin: `(name, statements)`, the same on the page and the desktop.
fn rows() -> Vec<(&'static str, String)> {
    let set = |v: &str, stmt: &str| {
        format!("    SET NUD-1::Value TO \"{v}\"\n    {stmt}\n    DISPLAY NUD-1::Value\n")
    };
    vec![
        ("ADD", set("5", "ADD 1 TO NUD-1::Value")),
        (
            "ADD twice",
            set("5", "ADD 1 TO NUD-1::Value\n    ADD 1 TO NUD-1::Value"),
        ),
        ("ADD giving", set("5", "ADD 1 2 TO 4 GIVING NUD-1::Value")),
        ("SUBTRACT", set("5", "SUBTRACT 2 FROM NUD-1::Value")),
        ("MULTIPLY", set("5", "MULTIPLY 3 BY NUD-1::Value")),
        (
            "COMPUTE",
            set("5", "COMPUTE NUD-1::Value = NUD-1::Value + 10"),
        ),
        ("MOVE of a quotient", set("1", "MOVE 8 / 2 TO NUD-1::Value")),
    ]
}

/// A quotient written to a property: the desktop cuts it to the scale of the property's current text
/// (`resolve_lvalue` seeds a synthetic item from it), so `DIVIDE 2 INTO` 7 leaves 3; the page writes the
/// quotient with its nine decimals, as it does for `MOVE 8 / 2 TO …` (which is the same on both engines).
/// Recorded here so the difference stays a decision, not a surprise: `(name, statements, page, desktop)`.
fn known_differences() -> Vec<(&'static str, String, &'static str, &'static str)> {
    let set = |v: &str, stmt: &str| {
        format!("    SET NUD-1::Value TO \"{v}\"\n    {stmt}\n    DISPLAY NUD-1::Value\n")
    };
    vec![
        (
            "DIVIDE exact",
            set("8", "DIVIDE 2 INTO NUD-1::Value"),
            "4.000000000",
            "4",
        ),
        (
            "DIVIDE not exact",
            set("7", "DIVIDE 2 INTO NUD-1::Value"),
            "3.500000000",
            "3",
        ),
        (
            "COMPUTE quotient",
            set("1", "COMPUTE NUD-1::Value = 1 / 4"),
            "0.250000000",
            "0",
        ),
    ]
}

#[test]
fn a_statement_whose_receiver_is_a_property_reads_adds_and_writes_it_back() {
    let mut wrong = Vec::new();
    for (name, body) in rows() {
        let (page, desk) = (click(&body), desktop(&body));
        if page != desk {
            wrong.push(format!("{name}: page {page:?}, desktop {desk:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_quotient_written_to_a_property_keeps_its_decimals_on_the_page_the_one_known_difference() {
    let mut wrong = Vec::new();
    for (name, body, page_want, desk_want) in known_differences() {
        let (page, desk) = (click(&body), desktop(&body));
        if page != vec![page_want.to_string()] || desk != vec![desk_want.to_string()] {
            wrong.push(format!("{name}: page {page:?} (recorded {page_want}), desktop {desk:?} (recorded {desk_want})"));
        }
    }
    assert!(
        wrong.is_empty(),
        "the recorded difference changed: {wrong:#?}"
    );
}
