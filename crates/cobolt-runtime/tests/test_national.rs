// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 M2 — national (`PIC N`) and UTF-8 (`PIC U`) items at run time:
//! storage, group images and REDEFINES, moves, figuratives, comparisons,
//! LENGTH / BYTE-LENGTH.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `ws` + `proc_`; the DISPLAY lines (untrimmed) and the interpreter.
fn run(ws: &str, proc_: &str) -> (Vec<String>, Interpreter) {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{ws}\nPROCEDURE DIVISION.\nMAIN-PARA.\n{proc_}\n    STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    let lines = display_rx.try_iter().map(|s| s.trim_end_matches(['\n', '\r']).to_owned()).collect();
    (lines, interp)
}

fn lines(ws: &str, proc_: &str) -> Vec<String> {
    run(ws, proc_).0
}

/// AC1: 27 characters and 3 spaces; LENGTH 30, BYTE-LENGTH 60.
#[test]
fn ac1_a_national_item_holds_characters() {
    let out = lines(
        "01 WS-MSG PIC N(30) VALUE N\"Configuração concluída – ok\".\n01 WS-L PIC 9(3).",
        "    DISPLAY \"[\" WS-MSG \"]\"\n    MOVE FUNCTION LENGTH(WS-MSG) TO WS-L\n    DISPLAY WS-L\n    MOVE FUNCTION BYTE-LENGTH(WS-MSG) TO WS-L\n    DISPLAY WS-L",
    );
    assert_eq!(out, vec!["[Configuração concluída – ok   ]", "030", "060"]);
}

/// AC2: `PIC N(3)` + `PIC X(2)` is 8 bytes; a `PIC X(8)` REDEFINES shows the
/// UTF-16 bytes; a group move to an identical group restores the characters.
#[test]
fn ac2_group_image_redefines_and_group_move() {
    let (out, interp) = run(
        "01 G1.\n   05 G1-N PIC N(3) VALUE N\"Açã\".\n   05 G1-X PIC X(2) VALUE \"xy\".\n01 G1-R REDEFINES G1 PIC X(8).\n01 G2.\n   05 G2-N PIC N(3).\n   05 G2-X PIC X(2).\n01 WS-L PIC 9(3).",
        "    MOVE FUNCTION BYTE-LENGTH(G1) TO WS-L\n    DISPLAY WS-L\n    MOVE G1 TO G2\n    DISPLAY \"[\" G2-N \"][\" G2-X \"]\"",
    );
    assert_eq!(out, vec!["008", "[Açã][xy]"]);
    assert_eq!(
        interp.env.display_bytes("G1-R").unwrap(),
        vec![0x00, 0x41, 0x00, 0xE7, 0x00, 0xE3, b'x', b'y']
    );
}

/// AC2, the other way: bytes written through the REDEFINES read back as the
/// national characters they encode.
#[test]
fn ac2_writing_the_redefinition_changes_the_characters() {
    let out = lines(
        "01 G1.\n   05 G1-N PIC N(2).\n01 G1-R REDEFINES G1 PIC X(4).",
        "    MOVE X\"00410042\" TO G1-R\n    DISPLAY \"[\" G1-N \"]\"",
    );
    assert_eq!(out, vec!["[AB]"]);
}

/// AC12: `PIC U(5)`: LENGTH 5, BYTE-LENGTH 20, a long move keeps whole
/// characters.
#[test]
fn ac12_a_utf8_item() {
    let out = lines(
        "01 WS-U PIC U(5) VALUE U\"Ação!\".\n01 WS-L PIC 9(3).",
        "    DISPLAY \"[\" WS-U \"]\"\n    MOVE FUNCTION LENGTH(WS-U) TO WS-L\n    DISPLAY WS-L\n    MOVE FUNCTION BYTE-LENGTH(WS-U) TO WS-L\n    DISPLAY WS-L\n    MOVE U\"Configuração\" TO WS-U\n    DISPLAY \"[\" WS-U \"]\"",
    );
    assert_eq!(out, vec!["[Ação!]", "005", "020", "[Confi]"]);
}

#[test]
fn a_byte_length_item_keeps_whole_characters() {
    let out = lines(
        "01 WS-B PIC U BYTE-LENGTH 5.\n01 WS-L PIC 9(3).",
        "    MOVE \"Ação\" TO WS-B\n    DISPLAY \"[\" WS-B \"]\"\n    MOVE FUNCTION BYTE-LENGTH(WS-B) TO WS-L\n    DISPLAY WS-L",
    );
    assert_eq!(out, vec!["[Açã]", "005"]);
}

/// AC5: national to alphanumeric stores whole characters; and back.
#[test]
fn ac5_national_to_alphanumeric_and_back() {
    let (out, interp) = run(
        "01 WS-N PIC N(10) VALUE N\"Configuração\".\n01 WS-X PIC X(10).\n01 WS-N2 PIC N(10).",
        "    MOVE WS-N TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    MOVE WS-X TO WS-N2\n    DISPLAY \"[\" WS-N2 \"]\"",
    );
    // "Configuraç" is ten characters and eleven bytes: the 'ç' does not fit
    // in ten bytes whole, so it is left out and its byte is a space.
    assert_eq!(out, vec!["[Configura ]", "[Configura ]"]);
    assert!(std::str::from_utf8(&interp.env.display_bytes("WS-X").unwrap()).is_ok());
}

/// AC6: comparison with an alphanumeric literal, padded in characters.
#[test]
fn ac6_comparisons() {
    let out = lines(
        "01 WS-N PIC N(10) VALUE N\"Ação\".\n01 WS-M PIC N(3) VALUE N\"Açb\".",
        "    IF WS-N = \"Ação\" DISPLAY \"EQ\" ELSE DISPLAY \"NE\" END-IF\n    IF WS-N = N\"Ação\" DISPLAY \"EQ\" ELSE DISPLAY \"NE\" END-IF\n    IF WS-N > WS-M DISPLAY \"GT\" ELSE DISPLAY \"LE\" END-IF\n    IF WS-N = SPACES DISPLAY \"SP\" ELSE DISPLAY \"NS\" END-IF",
    );
    // 'ç' (U+00E7) then 'ã' (U+00E3) > 'b' (U+0062): code point order.
    assert_eq!(out, vec!["EQ", "EQ", "GT", "NS"]);
}

#[test]
fn figuratives_and_justified() {
    let (out, interp) = run(
        "01 WS-N PIC N(3).\n01 WS-J PIC N(5) JUSTIFIED RIGHT.\n01 WS-H PIC N(1).\n01 WS-A PIC N(4).",
        "    MOVE ZERO TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n    MOVE \"Ação\" TO WS-J\n    DISPLAY \"[\" WS-J \"]\"\n    MOVE HIGH-VALUE TO WS-H\n    MOVE ALL N\"ç\" TO WS-A\n    DISPLAY \"[\" WS-A \"]\"\n    MOVE SPACES TO WS-N\n    DISPLAY \"[\" WS-N \"]\"",
    );
    assert_eq!(out, vec!["[000]", "[ Ação]", "[çççç]", "[   ]"]);
    assert_eq!(interp.env.display_string("WS-H").unwrap(), "\u{FFFF}");
}

/// A table of national items: each occurrence is its own class member.
#[test]
fn an_occurs_table_of_national_items() {
    let (out, interp) = run(
        "01 T.\n   05 T-N PIC N(2) OCCURS 3 TIMES.\n01 WS-L PIC 9(3).",
        "    MOVE \"çã\" TO T-N(2)\n    DISPLAY \"[\" T-N(2) \"]\"\n    MOVE FUNCTION BYTE-LENGTH(T) TO WS-L\n    DISPLAY WS-L",
    );
    assert_eq!(out, vec!["[çã]", "012"]);
    let image = interp.env.group_bytes("T").unwrap();
    assert_eq!(&image[4..8], &[0x00, 0xE7, 0x00, 0xE3]);
}

/// INITIALIZE gives a national item spaces; REPLACING NATIONAL / UTF-8
/// reaches only items of that category.
#[test]
fn initialize_and_replacing_by_class() {
    let out = lines(
        "01 G.\n   05 G-N PIC N(3) VALUE N\"abc\".\n   05 G-U PIC U(3) VALUE U\"def\".\n   05 G-X PIC X(3) VALUE \"ghi\".",
        "    INITIALIZE G\n    DISPLAY \"[\" G-N \"][\" G-U \"][\" G-X \"]\"\n    INITIALIZE G REPLACING NATIONAL DATA BY N\"çã\" UTF-8 DATA BY U\"日本\"\n    DISPLAY \"[\" G-N \"][\" G-U \"][\" G-X \"]\"",
    );
    assert_eq!(out, vec!["[   ][   ][   ]", "[çã ][日本 ][   ]"]);
}

/// A nested program's national item keeps its class in the shared
/// environment — every RAD form event handler is a nested program.
#[test]
fn a_nested_programs_national_item_keeps_its_class() {
    let src = "IDENTIFICATION DIVISION.
PROGRAM-ID. OUTER.
PROCEDURE DIVISION.
MAIN-PARA.
    CALL \"INNER\"
    CALL \"INNER\"
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. INNER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC N(4).
01 WS-L PIC 9(3).
PROCEDURE DIVISION.
INNER-PARA.
    MOVE \"Coração\" TO WS-N
    MOVE FUNCTION BYTE-LENGTH(WS-N) TO WS-L
    DISPLAY \"[\" WS-N \"] \" WS-L
    EXIT PROGRAM.
END PROGRAM INNER.
END PROGRAM OUTER.
";
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", result.diagnostics);
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    let out: Vec<String> = display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect();
    assert_eq!(out, vec!["[Cora] 008", "[Cora] 008"]);
}

/// AC4: NATIONAL-OF / DISPLAY-OF round trips, through UTF-8 and through
/// Windows-1252 bytes.
#[test]
fn ac4_code_page_conversion() {
    let (out, interp) = run(
        "01 WS-X PIC X(10).\n01 WS-N PIC N(10).\n01 WS-CP PIC X(12) VALUE \"WINDOWS-1252\".",
        "    MOVE FUNCTION DISPLAY-OF(FUNCTION NATIONAL-OF(\"Ação\")) TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    MOVE FUNCTION DISPLAY-OF(N\"Ação\", 1252) TO WS-X\n    MOVE FUNCTION NATIONAL-OF(WS-X, WS-CP) TO WS-N\n    DISPLAY \"[\" WS-N \"]\"",
    );
    // PIC X(10) is ten bytes: "Ação" takes six. PIC N(10) is ten characters.
    assert_eq!(out, vec!["[Ação    ]", "[Ação      ]"]);
    // The Windows-1252 bytes themselves: one per character.
    assert_eq!(&interp.env.display_bytes("WS-X").unwrap()[..4], b"A\xE7\xE3o");
}

/// AC12: ULENGTH on a PIC U(5) item.
#[test]
fn ac12_ulength_of_a_utf8_item() {
    let out = lines(
        "01 WS-U PIC U(5) VALUE U\"Ação!\".\n01 WS-L PIC 9(3).",
        "    MOVE FUNCTION ULENGTH(WS-U) TO WS-L\n    DISPLAY WS-L",
    );
    assert_eq!(out, vec!["005"]);
}

/// AC14: each U-function over the alphanumeric text "Aç€😀".
#[test]
fn ac14_u_functions_over_alphanumeric_text() {
    let out = lines(
        "01 WS-T PIC X(10) VALUE \"Aç€😀\".\n01 WS-S PIC X(10).\n01 WS-L PIC 9(3).\n01 G.\n   05 G-N PIC N(1) VALUE N\"é\".\n01 G-R REDEFINES G PIC X(2).",
        "    MOVE FUNCTION ULENGTH(FUNCTION TRIM(WS-T)) TO WS-L\n    DISPLAY WS-L\n    MOVE FUNCTION UPOS(WS-T, 3) TO WS-L\n    DISPLAY WS-L\n    MOVE FUNCTION USUBSTR(WS-T, 2, 2) TO WS-S\n    DISPLAY \"[\" WS-S \"]\"\n    MOVE FUNCTION UWIDTH(WS-T, 4) TO WS-L\n    DISPLAY WS-L\n    MOVE FUNCTION USUPPLEMENTARY(WS-T) TO WS-L\n    DISPLAY WS-L\n    MOVE FUNCTION UVALID(WS-T) TO WS-L\n    DISPLAY WS-L\n    MOVE FUNCTION UVALID(G-R) TO WS-L\n    DISPLAY WS-L",
    );
    // G-R holds X'00E9' — the national image of "é" — and X'E9' alone is
    // not UTF-8: the bad byte is the second.
    assert_eq!(out, vec!["004", "004", "[ç€     ]", "004", "007", "000", "002"]);
}

#[test]
fn upper_case_of_national_data_is_unicode() {
    let out = lines(
        "01 WS-N PIC N(5) VALUE N\"ação\".\n01 WS-M PIC N(5).",
        "    MOVE FUNCTION UPPER-CASE(WS-N) TO WS-M\n    DISPLAY \"[\" WS-M \"]\"",
    );
    assert_eq!(out, vec!["[AÇÃO ]"]);
}

/// AC7: INSPECT on a national item counts and replaces characters.
#[test]
fn ac7_inspect_counts_characters() {
    let out = lines(
        "01 WS-N PIC N(12) VALUE N\"ação, coração\".\n01 WS-C PIC 9(3) VALUE 0.\n01 WS-D PIC 9(3) VALUE 0.\n01 WS-E PIC 9(3) VALUE 0.",
        "    INSPECT WS-N TALLYING WS-C FOR ALL N\"ç\"\n    INSPECT WS-N TALLYING WS-D FOR CHARACTERS BEFORE INITIAL N\",\"\n    INSPECT WS-N TALLYING WS-E FOR CHARACTERS\n    DISPLAY WS-C \" \" WS-D \" \" WS-E\n    INSPECT WS-N REPLACING CHARACTERS BY N\"*\" AFTER INITIAL N\", \"\n    DISPLAY \"[\" WS-N \"]\"",
    );
    // "ação" is four characters before the comma; the item is twelve.
    assert_eq!(out, vec!["002 004 012", "[ação, ******]"]);
}

/// AC7: STRING two national items into a national receiver, POINTER in
/// characters; UNSTRING a national item by N",", COUNT IN in characters.
#[test]
fn ac7_string_and_unstring_by_character() {
    let out = lines(
        "01 WS-A PIC N(4) VALUE N\"Ação\".\n01 WS-B PIC N(4) VALUE N\"Pão\".\n01 WS-R PIC N(10).\n01 WS-P PIC 9(3) VALUE 1.\n01 WS-S PIC N(13) VALUE N\"maçã,pêra,uva\".\n01 WS-1 PIC N(5).\n01 WS-2 PIC N(5).\n01 WS-3 PIC N(5).\n01 WS-C1 PIC 9(3).\n01 WS-C2 PIC 9(3).",
        "    STRING WS-A DELIMITED BY SIZE N\"-\" DELIMITED BY SIZE WS-B DELIMITED BY SPACE INTO WS-R WITH POINTER WS-P\n    DISPLAY \"[\" WS-R \"] \" WS-P\n    UNSTRING WS-S DELIMITED BY N\",\" INTO WS-1 COUNT IN WS-C1 WS-2 COUNT IN WS-C2 WS-3\n    DISPLAY \"[\" WS-1 \"][\" WS-2 \"][\" WS-3 \"] \" WS-C1 \" \" WS-C2",
    );
    assert_eq!(out, vec!["[Ação-Pão  ] 009", "[maçã ][pêra ][uva  ] 004 004"]);
}
