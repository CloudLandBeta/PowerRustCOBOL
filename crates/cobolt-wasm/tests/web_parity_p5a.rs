// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! P5a (the PowerAnalyticsWeb parity job, 2026-10-28): what a page read of a property and a comparison of one did
//! differently from the desktop.
//!
//! * **A long property answer.** A property read (`SET WS-OUT TO LST-1::CheckedItems`) kept 4,096 bytes and dropped
//!   the rest without a word: the filter popups lost every name ticked after about 37. The desktop hands a statement
//!   the whole text, and the receiving item decides what a MOVE keeps (no item is longer than 65,535 bytes).
//! * **A property against a figurative constant.** After a click a toggle reads `1`/`0`: the desktop finds it equal to
//!   `NULL` (the number 0) and to `ALL "TRUE"`/`ALL "false"` (the literal is never shortened against a number); a
//!   property that reads `""` is equal to none of `ZERO`, `QUOTE`, `HIGH-VALUE`, `LOW-VALUE`, `ALL literal`.
//!
//! Every expected line is the desktop's (`rcrun run-form --headless`, probe of `CLAIMS-P5A.md`, 2026-10-28).

mod common;

use cobolt_wasm::host::FormHost;
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;
use cobolt_wasm_abi::scan::scan_module;

/// A one-form project in the shape `cobolt-codegen` writes; `handlers` are `(control, event, body)`. Control ids
/// follow the manifest: 3, 4, …
fn one_form(data: &str, controls: &[(&str, &str)], handlers: &[(&str, &str, &str)]) -> (Vec<u8>, cobolt_wasm_abi::manifest::Manifest) {
    let mut s = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. MAIN-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n\
         01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'MAIN-FORM'.\n{data}\
         PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    PERFORM COBOL-EVENT-LOOP\n    STOP RUN.\n\
         COBOL-EVENT-LOOP.\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
         \x20       EVALUATE COBOL-CONTROL-ID\n"
    );
    for (c, e, _) in handlers {
        s.push_str(&format!(
            "            WHEN \"{c}\"\n                EVALUATE COBOL-EVENT-ID\n                    WHEN \"{e}\"\n                        CALL \"{c}--{}\"\n                END-EVALUATE\n",
            e.to_ascii_uppercase()
        ));
    }
    s.push_str("        END-EVALUATE\n    END-PERFORM.\n");
    for (c, e, body) in handlers {
        let p = format!("{c}--{}", e.to_ascii_uppercase());
        s.push_str(&format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. {p} IS COMMON PROGRAM.\nPROCEDURE DIVISION.\n{body}    GOBACK.\nEND PROGRAM {p}.\n"));
    }
    s.push_str("END PROGRAM MAIN-FORM.\n");
    let program = common::parse(&s);
    let spec = FormSpec {
        name: "MAIN-FORM".into(),
        main: true,
        instances: 1,
        program: &program,
        controls: controls.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
        bindings: handlers.iter().map(|(c, e, _)| (c.to_string(), e.to_string())).collect(),
        layers: vec![],
        lines: vec![],
    };
    let c = compile_project(&[spec], &Options { multi_memory: false, name: "p5a".into(), ..Options::default() })
        .unwrap_or_else(|e| panic!("compile_project failed: {e}"));
    let m = scan_module(&c.wasm).unwrap().embedded_manifest().unwrap();
    (c.wasm, m)
}

const CONTROLS: &[(&str, &str)] = &[("TXT-1", "TextBox"), ("LST-1", "ListBox"), ("BTN-1", "Button"), ("BTN-2", "Button"), ("BTN-3", "Button")];

/// Click BTN-1, BTN-2, … (controls 5, 6, …) in turn, the handler of each being one of `bodies`; the DISPLAY lines and
/// the run-time errors with their text.
fn click_all(data: &str, bodies: &[&str]) -> (Vec<String>, Vec<(u32, String)>) {
    let names = ["BTN-1", "BTN-2", "BTN-3"];
    let handlers: Vec<(&str, &str, &str)> = bodies.iter().enumerate().map(|(i, b)| (names[i], "onClick", *b)).collect();
    let (wasm, m) = one_form(data, CONTROLS, &handlers);
    let click = m.event_index("onClick").unwrap() as i32;
    let mut host = FormHost::new(&wasm).unwrap();
    let (inst, _) = host.open_form(0, -1);
    for ctrl in 5..5 + bodies.len() as i32 {
        // A long loop yields at its back-edges (status 2): the host resumes it, as the player does on its next turn.
        let mut status = host.event(inst, ctrl, click, &[]);
        while status == 2 {
            let task = host.tick();
            status = host.resume(task, &[]);
        }
    }
    let o = host.outcome();
    assert!(o.trapped.is_none() || !o.errors.is_empty(), "the module trapped with no run-time error: {:?}", o.trapped);
    let errors = o.errors.iter().zip(&o.error_texts).map(|(e, t)| (e.code, t.clone())).collect();
    (common::wasm_lines(&o), errors)
}

const DATA: &str = "\
       01 WS-BIG PIC X(90000) VALUE SPACES GLOBAL.
       01 WS-OUT PIC X(90000) VALUE SPACES GLOBAL.
       01 WS-LF PIC X VALUE SPACE GLOBAL.
       01 WS-I PIC 9(4) VALUE 0 GLOBAL.
       01 WS-K PIC 9(5) VALUE 0 GLOBAL.
       01 WS-N PIC 9(5) VALUE 0 GLOBAL.
       01 WS-P PIC 9(6) VALUE 0 GLOBAL.
       01 WS-L PIC 9(6) VALUE 0 GLOBAL.
       01 WS-C PIC 9(6) VALUE 0 GLOBAL.
       01 WS-V PIC X(8) VALUE SPACES GLOBAL.
       01 WS-R PIC X(8) VALUE SPACES GLOBAL.
";

/// `n` names `prefix` + a five-digit number, joined by LF into WS-BIG.
fn names_into_big(n: u32, prefix: &str) -> String {
    format!(
        "    MOVE FUNCTION CHAR(11) TO WS-LF\n    MOVE SPACES TO WS-BIG\n    MOVE 1 TO WS-P\n\
         \x20   PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > {n}\n\
         \x20     IF WS-K > 1\n        STRING WS-LF DELIMITED BY SIZE INTO WS-BIG WITH POINTER WS-P END-STRING\n      END-IF\n\
         \x20     STRING \"{prefix}\" WS-K DELIMITED BY SIZE INTO WS-BIG WITH POINTER WS-P END-STRING\n    END-PERFORM\n"
    )
}

/// WS-OUT's length without trailing spaces in WS-L and its LF-separated names in WS-C.
const MEASURE: &str = "    PERFORM VARYING WS-L FROM 90000 BY -1 UNTIL WS-L = 0 OR WS-OUT(WS-L:1) NOT = SPACE\n    END-PERFORM\n\
    \x20   MOVE 0 TO WS-C\n    INSPECT WS-OUT TALLYING WS-C FOR ALL WS-LF\n    ADD 1 TO WS-C\n";

/// P-F1: ticks written to a ListBox's `CheckedItems` and read back whole — 500 and 2,000 names (13,999 and 55,999
/// bytes). The page kept 4,096 bytes (147 names) and refused the 2,000-name SET with an error that had no text.
#[test]
fn a_long_property_answer_reaches_the_statement_whole() {
    let mut body = String::new();
    for n in [500, 2000] {
        body.push_str(&names_into_big(n, "FUNDO DE INVESTIMENTO "));
        body.push_str("    SET LST-1::CheckedItems TO WS-BIG\n    MOVE SPACES TO WS-OUT\n    SET WS-OUT TO LST-1::CheckedItems\n");
        body.push_str(MEASURE);
        body.push_str("    DISPLAY \"N \" WS-K \" LEN \" WS-L \" NAMES \" WS-C \" LAST [\" WS-OUT(WS-L - 26 : 27) \"]\"\n");
    }
    let (lines, errors) = click_all(DATA, &[&body]);
    assert!(errors.is_empty(), "{errors:?}");
    // The desktop's lines (WS-K is one past the last name).
    assert_eq!(
        lines,
        ["N 00501 LEN 013999 NAMES 000500 LAST [FUNDO DE INVESTIMENTO 00500]", "N 02001 LEN 055999 NAMES 002000 LAST [FUNDO DE INVESTIMENTO 02000]"]
    );
}

/// A value method keeps every byte of a long text (`SET LST-SM::CheckedItems TO WS-BIG::Trim()` in PowerAnalytics):
/// the page read the receiver up to 12 KiB and the result up to 4 KiB without a word. And `X::Len` of an item of a
/// few words trapped the module (core's word-at-a-time count reached a stubbed panic).
#[test]
fn a_value_method_keeps_every_byte_of_a_long_text() {
    let mut body = names_into_big(600, "  fundo de investimento ");
    body.push_str("    SET LST-1::CheckedItems TO WS-BIG::Trim()\n    MOVE SPACES TO WS-OUT\n    SET WS-OUT TO LST-1::CheckedItems::Upper()\n");
    body.push_str(MEASURE);
    body.push_str("    DISPLAY \"VM LEN \" WS-L \" NAMES \" WS-C \" FIRST [\" WS-OUT(1:26) \"] LAST [\" WS-OUT(WS-L - 28 : 29) \"]\"\n");
    body.push_str("    MOVE \"ab\" TO WS-V\n    MOVE WS-V::Len() TO WS-L\n    DISPLAY \"LEN8 \" WS-L\n");
    let (lines, errors) = click_all(DATA, &[&body]);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(lines, ["VM LEN 017997 NAMES 000600 FIRST [FUNDO DE INVESTIMENTO 0000] LAST [  FUNDO DE INVESTIMENTO 00600]", "LEN8 000008"]);
}

/// Past the limits — an answer over 64 KiB, a SET over the batch — the statement stops with run-time error 1111 and a
/// text that says why, never with part of the text.
#[test]
fn a_text_beyond_the_limits_is_a_run_time_error_that_says_so() {
    // 2,500 names: 69,999 bytes, more than one SET carries.
    let mut set = names_into_big(2500, "FUNDO DE INVESTIMENTO ");
    set.push_str("    SET LST-1::CheckedItems TO WS-BIG\n    DISPLAY \"NOT REACHED\"\n");
    // 2,400 names: 67,199 bytes, which a SET carries and no answer holds.
    let mut read = names_into_big(2400, "FUNDO DE INVESTIMENTO ");
    read.push_str("    SET LST-1::CheckedItems TO WS-BIG\n    SET WS-OUT TO LST-1::CheckedItems\n    DISPLAY \"NOT REACHED\"\n");
    let (lines, errors) = click_all(DATA, &[&set, &read]);
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(
        errors,
        [
            (1111, "a property is set to 69999 bytes; one SET carries at most 69623 bytes".to_string()),
            (1111, "a property or method answered more than the 65536 bytes a statement can read of one answer".to_string()),
        ]
    );
}

/// P-2 residuals: a property against every figurative constant and `ALL literal`, with `=`, `NOT =`, `<`, `>`, `<=`,
/// `>=` and the constant on the left (`F = P`, `F < P`), for the texts toggles and text boxes read (`""` untouched,
/// `1`/`0` after a click, the words, numbers, characters). 2,016 conditions and 12 EVALUATEs: the desktop's answers,
/// one 8-letter group per constant in the order of `RHS`. The page differed in 195 of them.
#[test]
fn a_property_compares_with_every_figurative_constant_as_on_the_desktop() {
    const RHS: [&str; 21] = [
        "ZERO", "ZEROS", "ZEROES", "SPACE", "SPACES", "HIGH-VALUE", "HIGH-VALUES", "LOW-VALUE", "LOW-VALUES", "QUOTE", "QUOTES", "NULL",
        "ALL \"0\"", "ALL \"1\"", "ALL \"00\"", "ALL \"TRUE\"", "ALL \"true\"", "ALL \"false\"", "ALL \"FALSE\"", "ALL \"x\"", "ALL \"ab\"",
    ];
    let desktop: [(&str, &str, &str); 12] = [
        ("", "nYYnYnnn nYYnYnnn nYYnYnnn YnnnYYYn YnnnYYYn nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn", "SPACE"),
        ("0", "YnnnYYYn YnnnYYYn YnnnYYYn nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY YnnnYYYn YnnnYYYn nYYnYnnn YnnnYYYn nYYnYnnn nYYnYnnn YnYnYnYn YnYnYnYn nYYnYnnn nYYnYnnn", "NULL"),
        ("1", "nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY YnnnYYYn nYnYnYnY YnYnYnYn YnYnYnYn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn", "ALLTRUE"),
        ("00", "YnnnYYYn YnnnYYYn YnnnYYYn nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY YnnnYYYn YnnnYYYn nYYnYnnn YnnnYYYn nYYnYnnn nYYnYnnn YnYnYnYn YnYnYnYn nYYnYnnn nYYnYnnn", "NULL"),
        ("0.0", "YnnnYYYn YnnnYYYn YnnnYYYn nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY YnnnYYYn YnnnYYYn nYYnYnnn YnnnYYYn nYYnYnnn nYYnYnnn YnYnYnYn YnYnYnYn nYYnYnnn nYYnYnnn", "NULL"),
        ("true", "nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY YnnYnYYY YnnnYYYn nYnYnYnY nYnYnYnY nYYnYnnn nYnYnYnY", "ALLTRUE"),
        ("false", "YnnYnYYY YnnYnYYY YnnYnYYY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY YnnYnYYY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn YnnnYYYn YnnYnYYY nYYnYnnn nYnYnYnY", "NULL"),
        ("TRUE", "nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY YnnnYYYn YnYnYnYn nYYnYnnn nYnYnYnY nYYnYnnn nYYnYnnn", "ALLTRUE"),
        ("abc", "nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYYnYnnn nYnYnYnY", "OTHER"),
        ("12", "nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn", "OTHER"),
        ("-1", "nYYnYnnn nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn nYYnYnnn", "OTHER"),
        ("x", "nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYYnYnnn nYYnYnnn nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY nYnYnYnY YnnnYYYn nYnYnYnY", "OTHER"),
    ];
    let p = "TXT-1::Text";
    let mut body = String::new();
    let mut want = Vec::new();
    for (vi, (text, answers, ev)) in desktop.iter().enumerate() {
        let lit = if text.is_empty() { "SPACES".to_string() } else { format!("\"{text}\"") };
        body.push_str(&format!("    MOVE {lit} TO WS-V\n    SET {p} TO WS-V\n"));
        for (ri, (r, answer)) in RHS.iter().zip(answers.split(' ')).enumerate() {
            body.push_str("    MOVE SPACES TO WS-R\n");
            let conds = [
                format!("{p} = {r}"),
                format!("{p} NOT = {r}"),
                format!("{p} < {r}"),
                format!("{p} > {r}"),
                format!("{p} <= {r}"),
                format!("{p} >= {r}"),
                format!("{r} = {p}"),
                format!("{r} < {p}"),
            ];
            for (ci, c) in conds.iter().enumerate() {
                body.push_str(&format!("    IF {c} MOVE \"Y\" TO WS-R({}:1) ELSE MOVE \"n\" TO WS-R({}:1) END-IF\n", ci + 1, ci + 1));
            }
            body.push_str(&format!("    DISPLAY \"R{vi}-{ri} \" WS-R\n"));
            want.push(format!("R{vi}-{ri} {answer}"));
        }
        body.push_str(&format!(
            "    EVALUATE {p} WHEN NULL DISPLAY \"E{vi} NULL\" WHEN SPACE DISPLAY \"E{vi} SPACE\" WHEN ALL \"TRUE\" DISPLAY \"E{vi} ALLTRUE\" WHEN OTHER DISPLAY \"E{vi} OTHER\" END-EVALUATE\n"
        ));
        want.push(format!("E{vi} {ev}"));
    }
    let (lines, errors) = click_all(DATA, &[&body]);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(lines.len(), want.len());
    // Each differing row named by the property's text and the constant.
    let name = |l: &str| -> String {
        let Some(id) = l.split(' ').next().and_then(|t| t.strip_prefix('R')) else { return l.to_string() };
        let (v, r) = id.split_once('-').unwrap();
        format!("[{}] {}", desktop[v.parse::<usize>().unwrap()].0, RHS[r.parse::<usize>().unwrap()])
    };
    let wrong: Vec<String> =
        lines.iter().zip(&want).filter(|(a, b)| a != b).map(|(a, b)| format!("{}: page {a}, desktop {b}", name(b))).collect();
    assert!(wrong.is_empty(), "{} of {} rows differ from the desktop: {wrong:#?}", wrong.len(), want.len());
}
