// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 T4 — what Check refuses for national and UTF-8 data (R16, Q3,
//! AC4, AC11), each on its line, and a clean program with none.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

/// Every error, parser's and analyser's, as (line, message).
fn errors(src: &str) -> Vec<(u32, String)> {
    let r = parse(tokenize(src, SourceFormat::Free));
    let mut out: Vec<(u32, String)> = r
        .diagnostics
        .iter()
        .filter(|d| d.severity == cobolt_parser::Severity::Error)
        .map(|d| (d.span.line, d.message.clone()))
        .collect();
    if let Some(p) = r.program {
        let sem = cobolt_semantic::analyze(&p);
        out.extend(sem.errors().map(|d| (d.span.line, d.message.clone())));
    }
    out.sort();
    out
}

fn prog(ws: &str, proc_: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{ws}\nPROCEDURE DIVISION.\nMAIN-PARA.\n{proc_}\n    STOP RUN.\n"
    )
}

#[test]
fn a_clean_program_has_no_diagnostic() {
    let src = prog(
        "01 WS-N PIC N(10) VALUE N\"Ação\".\n01 WS-U PIC U(5) VALUE U\"Ação!\".\n01 WS-B PIC U BYTE-LENGTH 8.\n01 WS-X PIC X(10).\n01 WS-I PIC 9(3).",
        "    MOVE WS-N TO WS-X\n    MOVE WS-X TO WS-U\n    IF WS-N = \"Ação\" DISPLAY WS-N END-IF\n    COMPUTE WS-I = FUNCTION LENGTH(WS-N) + 1",
    );
    assert_eq!(errors(&src), Vec::<(u32, String)>::new());
}

/// AC11 and R16: arithmetic with a national or UTF-8 item, as a receiver or
/// an operand.
#[test]
fn arithmetic_on_national_or_utf8_is_refused() {
    let src = prog(
        "01 WS-N PIC N(5).\n01 WS-U PIC U(5).\n01 WS-I PIC 9(3).",
        "    ADD 1 TO WS-N\n    ADD WS-U TO WS-I\n    COMPUTE WS-I = WS-N * 2\n    MULTIPLY WS-I BY WS-U",
    );
    let e = errors(&src);
    let lines: Vec<u32> = e.iter().map(|(l, _)| *l).collect();
    for line in [10, 11, 12, 13] {
        assert!(lines.contains(&line), "an error on line {line}: {e:?}");
    }
    assert!(e.iter().any(|(_, m)| m.contains("'WS-U' is a UTF-8 item; ADD takes numeric operands only")), "{e:?}");
    assert!(e.iter().any(|(_, m)| m.contains("'WS-N' is a national item; COMPUTE")), "{e:?}");
}

/// Q3, and USAGE / PICTURE agreement.
#[test]
fn declarations_that_cannot_be_held() {
    let src = prog(
        "01 WS-A PIC 9(5) USAGE NATIONAL.\n01 WS-B PIC NNBNN.\n01 WS-C PIC X(5) USAGE NATIONAL.\n01 WS-D PIC X(5) USAGE UTF-8.\n01 WS-E PIC U(3) BYTE-LENGTH 9.\n01 WS-F PIC X(4) BYTE-LENGTH 4.",
        "",
    );
    let e = errors(&src);
    let has = |line: u32, text: &str| e.iter().any(|(l, m)| *l == line && m.contains(text));
    assert!(has(5, "national numeric items (PIC 9 USAGE NATIONAL) are not supported yet"), "{e:?}");
    assert!(has(6, "national-edited items are not supported yet"), "{e:?}");
    assert!(has(7, "USAGE NATIONAL needs PIC N"), "{e:?}");
    assert!(has(8, "USAGE UTF-8 needs PIC U"), "{e:?}");
    assert!(has(9, "BYTE-LENGTH goes with PIC U (a single U) only"), "{e:?}");
    assert!(has(10, "BYTE-LENGTH goes with PIC U (a single U) only"), "{e:?}");
}

#[test]
fn a_value_longer_than_its_item() {
    let src = prog(
        "01 WS-A PIC N(3) VALUE N\"Ação\".\n01 WS-B PIC U BYTE-LENGTH 4 VALUE U\"Ação\".\n01 WS-C PIC U(4) VALUE U\"Ação\".",
        "",
    );
    let e = errors(&src);
    assert_eq!(e.len(), 2, "{e:?}");
    assert!(e[0].1.contains("WS-A: the VALUE has 4 characters; the item holds 3"), "{e:?}");
    assert!(e[1].1.contains("WS-B: the VALUE has 6 bytes; the item holds 4"), "{e:?}");
}

/// AC4: a code page written as a literal is checked.
#[test]
fn an_unknown_code_page_is_refused() {
    let src = prog(
        "01 WS-X PIC X(10).\n01 WS-N PIC N(10).",
        "    MOVE FUNCTION NATIONAL-OF(WS-X, \"EBCDIC-037\") TO WS-N\n    MOVE FUNCTION DISPLAY-OF(WS-N, 1252) TO WS-X",
    );
    let e = errors(&src);
    assert!(e.iter().any(|(l, m)| *l == 9 && m.contains("'EBCDIC-037' is not a code page RustCOBOL converts")), "{e:?}");
    assert!(!e.iter().any(|(l, m)| *l == 10 && m.contains("code page")), "1252 is accepted: {e:?}");
}
