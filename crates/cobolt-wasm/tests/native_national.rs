// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T44 (R11, C4): a desktop module computes national (`PIC N`) and UTF-8
//! (`PIC U`) data as the interpreter does. Each case is the interpreter's own national test
//! (`cobolt-runtime/tests/test_national.rs`) or a rule it relies on, run on both engines: the
//! lines DISPLAY writes must be identical, and so must the bytes of every item (the group `G`
//! holding them, read out of the module's memory and out of the interpreter's environment).

mod common;

use cobolt_wasm::host::{run_native, run_native_memory, NativeInput};
use cobolt_wasm::{compile_program, Options};
use std::time::{Duration, Instant};

fn native(multi: bool) -> Options {
    Options { native: true, multi_memory: multi, ..Options::default() }
}

const START: &[u8] = b"<<<<GROUP-START>";
const END: &[u8] = b"<GROUP-END>>>>>>";

fn program(items: &str, procedure: &str) -> String {
    // Items after a line `01 OUT.` are kept out of `G`: the interpreter keeps a binary or packed
    // item as its value and a module as its bytes, so their images are not comparable. Their
    // values are compared where the case DISPLAYs them.
    let (inside, outside) = match items.find("01 OUT.") {
        Some(i) => items.split_at(i),
        None => (items, ""),
    };
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T44N.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 MARK-A PIC X(16) VALUE \"<<<<GROUP-START>\".\n01 G.\n{inside}\
         01 MARK-B PIC X(16) VALUE \"<GROUP-END>>>>>>\".\n{outside}PROCEDURE DIVISION.\nMAIN-PARA.\n{procedure}    STOP RUN.\n"
    )
}

/// The bytes of `G` in the module's memory: between the markers of the live region (the last
/// copy: the initial image's blob comes before it).
fn module_group(memory: &[u8]) -> Vec<u8> {
    let start = memory.windows(START.len()).rposition(|w| w == START).expect("MARK-A in memory") + START.len();
    let end = start + memory[start..].windows(END.len()).position(|w| w == END).expect("MARK-B in memory");
    memory[start..end].to_vec()
}

/// What the interpreter DISPLAYs, and the bytes of `G` (`None` when the source has no `G`).
fn interpreter(src: &str) -> (Vec<String>, Option<Vec<u8>>) {
    let prog = common::parse(src);
    let (_event_tx, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    interp.run().expect("the interpreter runs it");
    let lines = display_rx.try_iter().map(|s: String| s.trim_end_matches(['\n', '\r']).to_owned()).collect();
    (lines, interp.env.group_bytes("G"))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" ")
}

/// (name, the items of `G`, the procedure)
const CASES: &[(&str, &str, &str)] = &[
    (
        "a national item holds characters; LENGTH 30, BYTE-LENGTH 60",
        "   05 WS-MSG PIC N(30) VALUE N\"Configuração concluída – ok\".\n   05 WS-L PIC 9(3).\n",
        "    DISPLAY \"[\" WS-MSG \"]\"\n    MOVE FUNCTION LENGTH(WS-MSG) TO WS-L\n    DISPLAY WS-L\n\
         \x20   MOVE FUNCTION BYTE-LENGTH(WS-MSG) TO WS-L\n    DISPLAY WS-L\n",
    ),
    (
        "a group's image, its REDEFINES, a group move",
        "   05 G1.\n      10 G1-N PIC N(3) VALUE N\"Açã\".\n      10 G1-X PIC X(2) VALUE \"xy\".\n\
         \x20  05 G1-R REDEFINES G1 PIC X(8).\n   05 G2.\n      10 G2-N PIC N(3).\n      10 G2-X PIC X(2).\n   05 WS-L PIC 9(3).\n",
        "    MOVE FUNCTION BYTE-LENGTH(G1) TO WS-L\n    DISPLAY WS-L\n    MOVE G1 TO G2\n    DISPLAY \"[\" G2-N \"][\" G2-X \"]\"\n",
    ),
    (
        "bytes written through a REDEFINES read back as characters",
        "   05 H1.\n      10 H1-N PIC N(2).\n   05 H1-R REDEFINES H1 PIC X(4).\n",
        "    MOVE X\"00410042\" TO H1-R\n    DISPLAY \"[\" H1-N \"]\"\n",
    ),
    (
        "a UTF-8 item: LENGTH 5, BYTE-LENGTH 20, a long move keeps whole characters",
        "   05 WS-U PIC U(5) VALUE U\"Ação!\".\n   05 WS-L PIC 9(3).\n",
        "    DISPLAY \"[\" WS-U \"]\"\n    MOVE FUNCTION LENGTH(WS-U) TO WS-L\n    DISPLAY WS-L\n\
         \x20   MOVE FUNCTION BYTE-LENGTH(WS-U) TO WS-L\n    DISPLAY WS-L\n    MOVE U\"Configuração\" TO WS-U\n    DISPLAY \"[\" WS-U \"]\"\n",
    ),
    (
        "a BYTE-LENGTH item keeps whole characters",
        "   05 WS-B PIC U BYTE-LENGTH 5.\n   05 WS-L PIC 9(3).\n",
        "    MOVE \"Ação\" TO WS-B\n    DISPLAY \"[\" WS-B \"]\"\n    MOVE FUNCTION BYTE-LENGTH(WS-B) TO WS-L\n    DISPLAY WS-L\n\
         \x20   MOVE FUNCTION LENGTH(WS-B) TO WS-L\n    DISPLAY WS-L\n",
    ),
    (
        "national to alphanumeric cuts on a whole character, and back",
        "   05 WS-N PIC N(10) VALUE N\"Configuração\".\n   05 WS-X PIC X(10).\n   05 WS-N2 PIC N(10).\n",
        "    MOVE WS-N TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    MOVE WS-X TO WS-N2\n    DISPLAY \"[\" WS-N2 \"]\"\n",
    ),
    (
        "comparisons by code point, padded with spaces",
        "   05 WS-N PIC N(10) VALUE N\"Ação\".\n   05 WS-M PIC N(3) VALUE N\"Açb\".\n",
        "    IF WS-N = \"Ação\" DISPLAY \"EQ\" ELSE DISPLAY \"NE\" END-IF\n    IF WS-N = N\"Ação\" DISPLAY \"EQ\" ELSE DISPLAY \"NE\" END-IF\n\
         \x20   IF WS-N > WS-M DISPLAY \"GT\" ELSE DISPLAY \"LE\" END-IF\n    IF WS-N = SPACES DISPLAY \"SP\" ELSE DISPLAY \"NS\" END-IF\n\
         \x20   IF WS-N < \"Ab\" DISPLAY \"LT\" ELSE DISPLAY \"GE\" END-IF\n    IF WS-M NOT = WS-N DISPLAY \"NE\" ELSE DISPLAY \"EQ\" END-IF\n",
    ),
    (
        "figurative constants and JUSTIFIED RIGHT",
        "   05 WS-N PIC N(3).\n   05 WS-J PIC N(5) JUSTIFIED RIGHT.\n   05 WS-H PIC N(1).\n   05 WS-A PIC N(4).\n",
        "    MOVE ZERO TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n    MOVE \"Ação\" TO WS-J\n    DISPLAY \"[\" WS-J \"]\"\n    MOVE HIGH-VALUE TO WS-H\n\
         \x20   MOVE ALL N\"ç\" TO WS-A\n    DISPLAY \"[\" WS-A \"]\"\n    MOVE SPACES TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n",
    ),
    (
        "a table of national items, and a SEARCH of it",
        "   05 T.\n      10 T-N PIC N(2) OCCURS 3 TIMES INDEXED BY TX.\n   05 WS-L PIC 9(3).\n",
        "    MOVE \"çã\" TO T-N(2)\n    DISPLAY \"[\" T-N(2) \"]\"\n    MOVE FUNCTION BYTE-LENGTH(T) TO WS-L\n    DISPLAY WS-L\n\
         \x20   SET TX TO 1\n    SEARCH T-N AT END DISPLAY \"none\" WHEN T-N(TX) = \"çã\" DISPLAY \"found\" END-SEARCH\n",
    ),
    (
        "INITIALIZE, and REPLACING by class",
        "   05 GI.\n      10 G-N PIC N(3) VALUE N\"abc\".\n      10 G-U PIC U(3) VALUE U\"def\".\n      10 G-X PIC X(3) VALUE \"ghi\".\n",
        "    INITIALIZE GI\n    DISPLAY \"[\" G-N \"][\" G-U \"][\" G-X \"]\"\n\
         \x20   INITIALIZE GI REPLACING NATIONAL DATA BY N\"çã\" UTF-8 DATA BY U\"日本\"\n    DISPLAY \"[\" G-N \"][\" G-U \"][\" G-X \"]\"\n",
    ),
    (
        "VALUE clauses: JUSTIFIED applies, figurative constants fill the class",
        "   05 WS-J1 PIC N(5) JUSTIFIED RIGHT VALUE \"ab\".\n   05 WS-J2 PIC N(4) JUSTIFIED RIGHT VALUE ALL N\"x\".\n\
         \x20  05 WS-S PIC N(3) VALUE SPACE.\n   05 WS-Z PIC U(3) VALUE ZERO.\n   05 WS-Q PIC U(2) VALUE QUOTE.\n\
         \x20  05 WS-LV PIC N(2) VALUE LOW-VALUE.\n   05 WS-HV PIC U(1) VALUE HIGH-VALUE.\n   05 WS-UN PIC N(2).\n",
        "    DISPLAY \"[\" WS-J1 \"][\" WS-J2 \"][\" WS-S \"][\" WS-Z \"][\" WS-Q \"][\" WS-UN \"]\"\n",
    ),
    (
        "a character that does not fit whole is left out",
        "   05 WS-N2 PIC N(2).\n   05 WS-U2 PIC U(2).\n   05 WS-B3 PIC U BYTE-LENGTH 3.\n   05 WS-X3 PIC X(3).\n",
        "    MOVE \"a😀b\" TO WS-N2\n    MOVE \"a😀b\" TO WS-U2\n    MOVE \"a😀b\" TO WS-B3\n    MOVE N\"é😀\" TO WS-X3\n\
         \x20   DISPLAY \"[\" WS-N2 \"][\" WS-U2 \"][\" WS-B3 \"][\" WS-X3 \"]\"\n",
    ),
    (
        "EVALUATE, literals of a class, DISPLAY of literals",
        "   05 WS-N PIC N(6) VALUE N\"Ação\".\n   05 WS-U PIC U(4) VALUE U\"日本\".\n",
        "    EVALUATE WS-N\n       WHEN \"Ab\" THRU \"Az\" DISPLAY \"range\"\n       WHEN \"Ação\" DISPLAY \"hit\"\n       WHEN OTHER DISPLAY \"miss\"\n    END-EVALUATE\n\
         \x20   IF WS-N = U\"Ação\" DISPLAY \"U-EQ\" ELSE DISPLAY \"U-NE\" END-IF\n    IF WS-U = N\"日本\" DISPLAY \"N-EQ\" ELSE DISPLAY \"N-NE\" END-IF\n\
         \x20   DISPLAY N\"Olá\" \"|\" U\"日本\" \"|\" WS-U\n",
    ),
    (
        "TRUE and FALSE spelled out compare as booleans",
        "   05 WS-T PIC N(5) VALUE N\"true\".\n   05 WS-O PIC N(1) VALUE N\"1\".\n   05 WS-F PIC U(6) VALUE U\" False\".\n",
        "    IF WS-T = \"TRUE\" DISPLAY \"T1\" ELSE DISPLAY \"F1\" END-IF\n    IF WS-T = WS-O DISPLAY \"T2\" ELSE DISPLAY \"F2\" END-IF\n\
         \x20   IF WS-F = \"false\" DISPLAY \"T3\" ELSE DISPLAY \"F3\" END-IF\n    IF WS-F NOT = WS-T DISPLAY \"T4\" ELSE DISPLAY \"F4\" END-IF\n\
         \x20   IF WS-O = \"1\" DISPLAY \"T5\" ELSE DISPLAY \"F5\" END-IF\n",
    ),
    (
        "senders and receivers beside national data: groups, edited items, several receivers",
        "   05 WS-N PIC N(4).\n   05 GR.\n      10 GR-A PIC X(3) VALUE \"abc\".\n   05 WS-E PIC ZZ9.99.\n\
         \x20  05 GR2.\n      10 GR2-X PIC X(8).\n   05 WS-X PIC X(4).\n   05 WS-U PIC U(2).\n",
        "    MOVE GR TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n    MOVE 3.5 TO WS-E\n    MOVE WS-E TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n\
         \x20   MOVE N\"çé\" TO WS-N\n    MOVE WS-N TO GR2\n    MOVE WS-N TO GR-A\n    DISPLAY \"[\" GR-A \"]\"\n\
         \x20   MOVE N\"ç\" TO WS-N WS-X WS-U\n    DISPLAY \"[\" WS-N \"][\" WS-X \"][\" WS-U \"]\"\n",
    ),
];

/// The functions a desktop module serves over national and UTF-8 data (DESKTOP-WASM-001 T50a1):
/// the interpreter's own national tests and the COBOL test programs `nat_functions.cbl` and
/// `utf8_basics.cbl`, case by case.
const FUNCTION_CASES: &[(&str, &str, &str)] = &[
    (
        "NATIONAL-OF and DISPLAY-OF: UTF-8, Windows-1252 and ISO-8859-1 by name and by CCSID",
        "   05 WS-X PIC X(10).\n   05 WS-X1 PIC X(1).\n   05 WS-N PIC N(10).\n   05 WS-CP PIC X(12) VALUE \"WINDOWS-1252\".\n   05 WS-L PIC 9(4).\n",
        "    MOVE FUNCTION DISPLAY-OF(FUNCTION NATIONAL-OF(\"Ação\")) TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n\
         \x20   MOVE FUNCTION DISPLAY-OF(N\"Ação\", 1252) TO WS-X\n    MOVE FUNCTION BYTE-LENGTH(FUNCTION DISPLAY-OF(N\"Ação\", 1252)) TO WS-L\n\
         \x20   MOVE FUNCTION NATIONAL-OF(WS-X, WS-CP) TO WS-N\n    DISPLAY WS-L \"[\" WS-N \"]\"\n\
         \x20   MOVE FUNCTION DISPLAY-OF(N\"ç\", 819) TO WS-X1\n    MOVE FUNCTION NATIONAL-OF(WS-X1, \"ISO-8859-1\") TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n\
         \x20   MOVE FUNCTION DISPLAY-OF(N\"€\", 819) TO WS-X1\n    IF WS-X1 = X\"7F\" DISPLAY \"SUB\" ELSE DISPLAY \"NOSUB\" END-IF\n\
         \x20   MOVE FUNCTION DISPLAY-OF(N\"€\", \"CP1252\") TO WS-X1\n    IF WS-X1 = X\"80\" DISPLAY \"EURO\" ELSE DISPLAY \"NOEURO\" END-IF\n\
         \x20   MOVE FUNCTION NATIONAL-OF(WS-X1, 1252) TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n",
    ),
    (
        "a code page held in a number, and one Windows-1252 leaves undefined",
        "   05 WS-CPN PIC 9(4) VALUE 1252.\n   05 WS-X PIC X(4).\n   05 WS-N PIC N(4).\n   05 WS-U PIC U(4).\n",
        "    MOVE FUNCTION DISPLAY-OF(N\"é€\", WS-CPN) TO WS-X\n    MOVE FUNCTION NATIONAL-OF(WS-X, WS-CPN) TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n\
         \x20   MOVE X\"E981\" TO WS-X\n    MOVE FUNCTION NATIONAL-OF(WS-X, 1252) TO WS-U\n    DISPLAY \"[\" WS-U \"]\"\n\
         \x20   MOVE FUNCTION NATIONAL-OF(WS-X, 819) TO WS-U\n    DISPLAY \"[\" WS-U \"]\"\n",
    ),
    (
        "UPPER-CASE and LOWER-CASE are Unicode for national and UTF-8 data, ASCII for any other",
        "   05 WS-N PIC N(10).\n   05 WS-U PIC U(10).\n   05 WS-A PIC X(10) VALUE \"aBc ção\".\n   05 WS-X PIC X(10).\n",
        "    MOVE FUNCTION UPPER-CASE(N\"ação\") TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n    MOVE FUNCTION LOWER-CASE(N\"ÉLAN\") TO WS-N\n    DISPLAY \"[\" WS-N \"]\"\n\
         \x20   MOVE FUNCTION UPPER-CASE(WS-N) TO WS-U\n    DISPLAY \"[\" WS-U \"]\"\n    MOVE FUNCTION LOWER-CASE(U\"ΟΔΥΣΣΕΥΣ ΑΣ\") TO WS-U\n    DISPLAY \"[\" WS-U \"]\"\n\
         \x20   MOVE FUNCTION UPPER-CASE(\"straße\") TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    MOVE FUNCTION UPPER-CASE(WS-A) TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n\
         \x20   MOVE FUNCTION UPPER-CASE(U\"straße\") TO WS-U\n    DISPLAY \"[\" WS-U \"]\"\n",
    ),
    (
        "the U-functions over alphanumeric text: characters, bytes, widths, validity",
        "   05 WS-T PIC X(10) VALUE \"Aç€😀\".\n   05 WS-S PIC X(10).\n   05 WS-L PIC 9(4).\n   05 H1.\n      10 H1-N PIC N(1) VALUE N\"é\".\n   05 H1-R REDEFINES H1 PIC X(2).\n",
        "    MOVE FUNCTION ULENGTH(FUNCTION TRIM(WS-T)) TO WS-L\n    DISPLAY \"ULENGTH \" WS-L\n    MOVE FUNCTION UPOS(WS-T, 3) TO WS-L\n    DISPLAY \"UPOS \" WS-L\n\
         \x20   MOVE FUNCTION UPOS(WS-T, 9) TO WS-L\n    DISPLAY \"UPOS-OUT \" WS-L\n    MOVE FUNCTION USUBSTR(WS-T, 2, 2) TO WS-S\n    DISPLAY \"USUBSTR [\" WS-S \"]\"\n\
         \x20   MOVE FUNCTION UWIDTH(WS-T, 4) TO WS-L\n    DISPLAY \"UWIDTH \" WS-L\n    MOVE FUNCTION USUPPLEMENTARY(WS-T) TO WS-L\n    DISPLAY \"USUPP \" WS-L\n\
         \x20   MOVE FUNCTION UVALID(WS-T) TO WS-L\n    DISPLAY \"UVALID \" WS-L\n    MOVE FUNCTION UVALID(H1-R) TO WS-L\n    DISPLAY \"UVALID-BAD \" WS-L\n\
         \x20   MOVE FUNCTION USUBSTR(WS-T, 3, 9) TO WS-S\n    DISPLAY \"USUBSTR-TAIL [\" WS-S \"]\"\n    MOVE FUNCTION USUBSTR(WS-T, 0, 2) TO WS-S\n    DISPLAY \"USUBSTR-ZERO [\" WS-S \"]\"\n",
    ),
    (
        "the U-functions over UTF-8 and national items and literals: UTF-8 bytes, UTF-16 bytes",
        "   05 WS-U PIC U(5) VALUE U\"Ação!\".\n   05 WS-N PIC N(5) VALUE N\"a😀bc\".\n   05 WS-B PIC U BYTE-LENGTH 6 VALUE U\"Aç\".\n   05 WS-S PIC X(12).\n   05 WS-L PIC 9(4).\n\
         \x20  05 H2.\n      10 H2-N PIC N(2) VALUE N\"ab\".\n   05 H2-R REDEFINES H2 PIC X(4).\n",
        "    MOVE FUNCTION ULENGTH(WS-U) TO WS-L\n    DISPLAY \"U ULENGTH \" WS-L\n    MOVE FUNCTION UPOS(WS-U, 4) TO WS-L\n    DISPLAY \"U UPOS \" WS-L\n\
         \x20   MOVE FUNCTION ULENGTH(WS-N) TO WS-L\n    DISPLAY \"N ULENGTH \" WS-L\n    MOVE FUNCTION UPOS(WS-N, 3) TO WS-L\n    DISPLAY \"N UPOS \" WS-L\n\
         \x20   MOVE FUNCTION UWIDTH(WS-N, 2) TO WS-L\n    DISPLAY \"N UWIDTH \" WS-L\n    MOVE FUNCTION USUPPLEMENTARY(WS-N) TO WS-L\n    DISPLAY \"N USUPP \" WS-L\n\
         \x20   MOVE FUNCTION UVALID(WS-N) TO WS-L\n    DISPLAY \"N UVALID \" WS-L\n    MOVE FUNCTION USUBSTR(WS-N, 2, 2) TO WS-S\n    DISPLAY \"N USUBSTR [\" WS-S \"]\"\n\
         \x20   MOVE FUNCTION ULENGTH(WS-B) TO WS-L\n    DISPLAY \"B ULENGTH \" WS-L\n    MOVE FUNCTION UPOS(N\"abc\", 2) TO WS-L\n    DISPLAY \"N-LIT UPOS \" WS-L\n\
         \x20   MOVE FUNCTION UPOS(U\"abc\", 2) TO WS-L\n    DISPLAY \"U-LIT UPOS \" WS-L\n    MOVE FUNCTION ULENGTH(N\"Ação😀\") TO WS-L\n    DISPLAY \"N-LIT ULENGTH \" WS-L\n\
         \x20   MOVE FUNCTION USUPPLEMENTARY(N\"ab😀\") TO WS-L\n    DISPLAY \"N-LIT USUPP \" WS-L\n    MOVE FUNCTION USUPPLEMENTARY(U\"ab😀\") TO WS-L\n    DISPLAY \"U-LIT USUPP \" WS-L\n\
         \x20   MOVE X\"D800\" TO H2-R\n    MOVE FUNCTION UVALID(H2-N) TO WS-L\n    DISPLAY \"N UVALID-BAD \" WS-L\n",
    ),
    (
        "every other function reads a national argument's text",
        "   05 WS-N PIC N(8) VALUE N\"Ação  \".\n   05 WS-V PIC N(6) VALUE N\"  12.5\".\n   05 WS-U PIC U(4) VALUE U\"日本\".\n   05 WS-X PIC X(24).\n   05 WS-L PIC 9(4).\n   05 WS-D PIC 9(3)V9.\n",
        "    MOVE FUNCTION TRIM(WS-N) TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    MOVE FUNCTION REVERSE(WS-N) TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n\
         \x20   MOVE FUNCTION CONCATENATE(WS-N, \"|\", WS-U) TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    MOVE FUNCTION STORED-CHAR-LENGTH(WS-N) TO WS-L\n    DISPLAY \"STORED \" WS-L\n\
         \x20   COMPUTE WS-D = FUNCTION NUMVAL(WS-V)\n    DISPLAY \"NUMVAL \" WS-D\n    MOVE FUNCTION ORD(WS-N) TO WS-L\n    DISPLAY \"ORD \" WS-L\n\
         \x20   MOVE FUNCTION REVERSE(U\"Aç日\") TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    MOVE FUNCTION UPPER-CASE(WS-N) TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n\
         \x20   IF WS-N = FUNCTION UPPER-CASE(WS-N) DISPLAY \"SAME\" ELSE DISPLAY \"DIFF\" END-IF\n\
         \x20   IF FUNCTION LOWER-CASE(N\"AÇÃO\") = WS-N DISPLAY \"EQ\" ELSE DISPLAY \"NE\" END-IF\n",
    ),
];

/// A nested program's national item keeps its class (every form handler is a nested program).
const NESTED: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\nPROCEDURE DIVISION.\nMAIN-PARA.\n    CALL \"INNER\"\n    CALL \"INNER\"\n    STOP RUN.\n\
IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-N PIC N(4).\n01 WS-L PIC 9(3).\n\
PROCEDURE DIVISION.\nINNER-PARA.\n    MOVE \"Coração\" TO WS-N\n    MOVE FUNCTION BYTE-LENGTH(WS-N) TO WS-L\n    DISPLAY \"[\" WS-N \"] \" WS-L\n\
    EXIT PROGRAM.\nEND PROGRAM INNER.\nEND PROGRAM OUTER.\n";

/// T50a3b: a national and a UTF-8 item passed to called programs (lines only: the items are not under `G`).
const CALL_PARAMS: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
01 WS-N PIC N(6) VALUE N\"Ação\".\n01 WS-U PIC U(6) VALUE U\"日本語\".\n01 WS-T PIC N(4) OCCURS 2.\n01 WS-J PIC N(6) JUSTIFIED RIGHT VALUE N\"ab\".\n01 WS-9 PIC 9(3) VALUE 42.\n\
PROCEDURE DIVISION.\nMAIN-PARA.\n    CALL \"INNER\" USING WS-N WS-U\n    DISPLAY \"[\" WS-N \"][\" WS-U \"]\"\n\
\x20   CALL \"INNER\" USING BY CONTENT WS-N BY CONTENT WS-U\n    DISPLAY \"[\" WS-N \"][\" WS-U \"]\"\n\
\x20   CALL \"INNER\" USING BY CONTENT N\"lit\" BY CONTENT U\"e\"\n    CALL \"INNER\" USING BY VALUE WS-N BY VALUE WS-U\n\
\x20   DISPLAY \"[\" WS-N \"][\" WS-U \"]\"\n    MOVE N\"a\" TO WS-T(1)\n    CALL \"ONE\" USING WS-T(1)\n    CALL \"ONE\" USING WS-T(2)\n\
\x20   DISPLAY \"[\" WS-T(1) \"][\" WS-T(2) \"]\"\n    CALL \"ONE\" USING BY CONTENT \"x\"\n\
\x20   CALL \"ONE\" USING BY CONTENT 7\n    DISPLAY \"[\" WS-T(1) \"]\"\n    CALL \"ONE\" USING BY CONTENT N\"\"\n    STOP RUN.\n\
IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-N PIC N(6).\n01 LK-U PIC U(6).\n\
PROCEDURE DIVISION USING LK-N LK-U.\nINNER-PARA.\n    DISPLAY \"in [\" LK-N \"][\" LK-U \"]\"\n    MOVE N\"ZZ\" TO LK-N\n    MOVE U\"qq\" TO LK-U\n    EXIT PROGRAM.\n\
END PROGRAM INNER.\n\
IDENTIFICATION DIVISION.\nPROGRAM-ID. ONE.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-T PIC N(4).\n\
PROCEDURE DIVISION USING LK-T.\nONE-PARA.\n    DISPLAY \"one [\" LK-T \"]\"\n    MOVE N\"abcdef\" TO LK-T\n    EXIT PROGRAM.\nEND PROGRAM ONE.\nEND PROGRAM OUTER.\n";

/// T50a3b: a national key in a SORT, ascending and descending, with a second key (lines only).
const SORT_KEYS: &str = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. T50B.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT SF ASSIGN TO "sortwork".
DATA DIVISION.
FILE SECTION.
SD SF.
01 SF-REC.
   05 SF-KEY PIC N(5).
   05 SF-NUM PIC 9(2).
WORKING-STORAGE SECTION.
01 WS-I PIC 9(2).
01 WS-EOF PIC 9 VALUE 0.
01 WS-S PIC N(5).
PROCEDURE DIVISION.
MAIN-PARA.
    SORT SF ON ASCENDING KEY SF-KEY INPUT PROCEDURE IS LOAD-IT OUTPUT PROCEDURE IS SHOW-IT
    DISPLAY "---"
    SORT SF ON DESCENDING KEY SF-KEY ASCENDING KEY SF-NUM INPUT PROCEDURE IS LOAD-IT OUTPUT PROCEDURE IS SHOW-IT
    STOP RUN.
LOAD-IT.
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 7
        EVALUATE WS-I
            WHEN 1 MOVE N"pão" TO SF-KEY
            WHEN 2 MOVE N"Zebra" TO SF-KEY
            WHEN 3 MOVE N"abc" TO SF-KEY
            WHEN 4 MOVE N"ção" TO SF-KEY
            WHEN 5 MOVE N"Éclat" TO SF-KEY
            WHEN 6 MOVE N"abc" TO SF-KEY
            WHEN 7 MOVE N"a" TO SF-KEY
        END-EVALUATE
        MOVE WS-I TO SF-NUM
        RELEASE SF-REC
    END-PERFORM.
SHOW-IT.
    MOVE 0 TO WS-EOF
    PERFORM UNTIL WS-EOF = 1
        RETURN SF AT END MOVE 1 TO WS-EOF
            NOT AT END
                MOVE SF-KEY TO WS-S
                DISPLAY "[" WS-S "] " SF-NUM
        END-RETURN
    END-PERFORM.
"#;

/// T50a4: a program whose collating sequence is the native order (`STANDARD-1` or `NATIVE`, which the
/// interpreter leaves in force as the byte order): national and UTF-8 data compare, sort and move as
/// they do with no sequence at all (lines only). A reordering sequence is the module-only test below.
const COLLATING_NATIVE: &str = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. T50C.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
OBJECT-COMPUTER. THIS-MACHINE PROGRAM COLLATING SEQUENCE IS ALPHA.
SPECIAL-NAMES. ALPHABET ALPHA IS @ALPHABET@.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC N(6).
01 WS-M PIC N(6).
01 WS-U PIC U(6).
01 WS-X PIC X(6).
01 WS-C PIC 9(3).
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE N"Ação" TO WS-N
    MOVE N"Ação" TO WS-M
    IF WS-N = WS-M DISPLAY "same" ELSE DISPLAY "differ" END-IF
    MOVE N"pão" TO WS-M
    IF WS-N < WS-M DISPLAY "A before p" ELSE DISPLAY "p before A" END-IF
    IF WS-N > N"Aa" DISPLAY "above Aa" ELSE DISPLAY "not above Aa" END-IF
    IF WS-M NOT < N"pa" DISPLAY "pão from pa" ELSE DISPLAY "pão below pa" END-IF
    MOVE U"ñandú" TO WS-U
    IF WS-U < U"zzz" DISPLAY "ñandú before zzz" ELSE DISPLAY "zzz before ñandú" END-IF
    IF WS-U = U"ñandú" DISPLAY "u same" ELSE DISPLAY "u differ" END-IF
    MOVE "Ab" TO WS-X
    MOVE N"Ab" TO WS-N
    IF WS-N = WS-X DISPLAY "n eq x" ELSE DISPLAY "n ne x" END-IF
    IF WS-N < "B" DISPLAY "n before B" ELSE DISPLAY "B before n" END-IF
    IF "a" < "B" DISPLAY "alnum a first" ELSE DISPLAY "alnum B first" END-IF
    IF WS-X > "Aa" DISPLAY "x above Aa" ELSE DISPLAY "x not above Aa" END-IF
    MOVE ZERO TO WS-C
    INSPECT WS-M TALLYING WS-C FOR ALL N"p"
    DISPLAY "[" WS-N "][" WS-M "][" WS-U "][" WS-X "] " WS-C
    MOVE LOW-VALUE TO WS-N
    DISPLAY "low " FUNCTION LENGTH(WS-N)
    STOP RUN.
"#;

/// T50a2: INSPECT, STRING and UNSTRING by character (name, the items of `G`, the procedure).
const VERB_CASES: &[(&str, &str, &str)] = &[
    (
        "STRING into a national item by character: POINTER, SIZE, SPACE, a national delimiter",
        r#"   05 WS-A PIC N(4) VALUE N"Ação".
   05 WS-B PIC N(4) VALUE N"Pão".
   05 WS-R PIC N(10).
   05 WS-P PIC 9(3).
   05 WS-D PIC N(2) VALUE N"ão".
"#,
        r#"    MOVE 1 TO WS-P
    STRING WS-A DELIMITED BY SIZE N"-" DELIMITED BY SIZE WS-B DELIMITED BY SPACE INTO WS-R WITH POINTER WS-P
    DISPLAY "[" WS-R "] " WS-P
    STRING WS-A DELIMITED BY WS-D INTO WS-R
    DISPLAY "[" WS-R "]"
    MOVE ALL N"*" TO WS-R
    STRING WS-B DELIMITED BY SIZE INTO WS-R
    DISPLAY "[" WS-R "]"
"#,
    ),
    (
        "STRING overflow, a pointer past the end, a supplementary character, JUSTIFIED RIGHT",
        r#"   05 WS-A PIC N(4) VALUE N"Ação".
   05 WS-B PIC N(4) VALUE N"Pão".
   05 WS-O PIC N(6).
   05 WS-S PIC N(4).
   05 WS-J PIC N(5) JUSTIFIED RIGHT.
   05 WS-P PIC 9(3).
"#,
        r#"    STRING WS-A WS-B DELIMITED BY SIZE INTO WS-O
        ON OVERFLOW DISPLAY "overflow"
        NOT ON OVERFLOW DISPLAY "fits"
    END-STRING
    DISPLAY "[" WS-O "]"
    MOVE 7 TO WS-P
    STRING WS-A DELIMITED BY SIZE INTO WS-O WITH POINTER WS-P
        ON OVERFLOW DISPLAY "overflow"
        NOT ON OVERFLOW DISPLAY "fits"
    END-STRING
    DISPLAY "[" WS-O "] " WS-P
    MOVE 3 TO WS-P
    STRING N"xy" DELIMITED BY SIZE INTO WS-O WITH POINTER WS-P
    DISPLAY "[" WS-O "] " WS-P
    STRING N"ab😀cd" DELIMITED BY SIZE INTO WS-S
        ON OVERFLOW DISPLAY "overflow"
    END-STRING
    DISPLAY "[" WS-S "]"
    STRING N"Ação" DELIMITED BY SIZE INTO WS-J
    DISPLAY "[" WS-J "]"
"#,
    ),
    (
        "STRING national and UTF-8 senders into an alphanumeric item, and an alphanumeric one into a UTF-8 item",
        r#"   05 WS-A PIC N(4) VALUE N"Ação".
   05 WS-U PIC U(5) VALUE U"naïve".
   05 WS-X PIC X(16).
   05 WS-V PIC U(10).
   05 WS-P PIC 9(3).
"#,
        r#"    MOVE SPACES TO WS-X
    MOVE 1 TO WS-P
    STRING WS-A DELIMITED BY SIZE "!" DELIMITED BY SIZE WS-U DELIMITED BY SIZE INTO WS-X WITH POINTER WS-P
    DISPLAY "[" WS-X "] " WS-P
    STRING "ação" DELIMITED BY SIZE " " DELIMITED BY SIZE WS-A DELIMITED BY SPACE INTO WS-V
    DISPLAY "[" WS-V "]"
"#,
    ),
    (
        "UNSTRING a national item by character: COUNT IN, TALLYING, POINTER, DELIMITER IN",
        r#"   05 WS-S PIC N(13) VALUE N"maçã,pêra,uva".
   05 WS-1 PIC N(5).
   05 WS-2 PIC N(5).
   05 WS-3 PIC N(5).
   05 WS-DL PIC N(2).
   05 WS-C1 PIC 9(3).
   05 WS-C2 PIC 9(3).
   05 WS-T PIC 9(3) VALUE 0.
   05 WS-P PIC 9(3) VALUE 1.
"#,
        r#"    UNSTRING WS-S DELIMITED BY N"," INTO WS-1 COUNT IN WS-C1 WS-2 COUNT IN WS-C2 WS-3 TALLYING IN WS-T
    DISPLAY "[" WS-1 "][" WS-2 "][" WS-3 "] " WS-C1 " " WS-C2 " " WS-T
    MOVE 6 TO WS-P
    UNSTRING WS-S DELIMITED BY N"," INTO WS-1 DELIMITER IN WS-DL COUNT IN WS-C1 WS-2 WITH POINTER WS-P
    DISPLAY "[" WS-1 "][" WS-2 "][" WS-DL "] " WS-C1 " " WS-P
"#,
    ),
    (
        "UNSTRING: ALL, several delimiters, no DELIMITED BY (split by the receivers' widths), overflow",
        r#"   05 WS-Q PIC N(14) VALUE N"a;;bç;c;;;d   ".
   05 WS-S PIC N(13) VALUE N"maçã,pêra,uva".
   05 WS-1 PIC N(5).
   05 WS-2 PIC N(5).
   05 WS-3 PIC N(5).
   05 WS-DL PIC N(3).
   05 WS-C1 PIC 9(3).
"#,
        r#"    UNSTRING WS-Q DELIMITED BY ALL N";" INTO WS-1 DELIMITER IN WS-DL COUNT IN WS-C1 WS-2 WS-3
    DISPLAY "[" WS-1 "][" WS-2 "][" WS-3 "][" WS-DL "] " WS-C1
    UNSTRING WS-Q DELIMITED BY N"ç" OR N";" INTO WS-1 WS-2 WS-3
    DISPLAY "[" WS-1 "][" WS-2 "][" WS-3 "]"
    UNSTRING WS-S INTO WS-1 WS-2 WS-3
    DISPLAY "[" WS-1 "][" WS-2 "][" WS-3 "]"
    UNSTRING WS-S DELIMITED BY N"," INTO WS-1 WS-2
        ON OVERFLOW DISPLAY "overflow"
        NOT ON OVERFLOW DISPLAY "fits"
    END-UNSTRING
    DISPLAY "[" WS-1 "][" WS-2 "]"
"#,
    ),
    (
        "UNSTRING: a UTF-8 item and a literal as the source, numeric, alphanumeric and national receivers",
        r#"   05 WS-US PIC U(13) VALUE U"maçã,pêra,uva".
   05 WS-AS PIC X(20) VALUE "maçã,pêra,uva".
   05 WS-1 PIC N(5).
   05 WS-2 PIC N(5).
   05 WS-3 PIC N(5).
   05 WS-X4 PIC X(4).
   05 WS-X5 PIC X(5).
   05 WS-N1 PIC 9(5).
   05 WS-N2 PIC 9(5).
   05 WS-N3 PIC 9(5).
"#,
        r#"    UNSTRING WS-US DELIMITED BY U"," INTO WS-1 WS-2 WS-3
    DISPLAY "[" WS-1 "][" WS-2 "][" WS-3 "]"
    UNSTRING WS-AS DELIMITED BY "," INTO WS-1 WS-2 WS-3
    DISPLAY "[" WS-1 "][" WS-2 "][" WS-3 "]"
    UNSTRING N"12,007,x4" DELIMITED BY N"," INTO WS-N1 WS-N2 WS-N3
    DISPLAY WS-N1 " " WS-N2 " " WS-N3
    UNSTRING WS-US DELIMITED BY N"," INTO WS-X4 WS-X5
    DISPLAY "[" WS-X4 "][" WS-X5 "]"
"#,
    ),
    (
        "INSPECT a national item counts and replaces characters",
        r#"   05 WS-N PIC N(12) VALUE N"ação, coração".
   05 WS-C PIC 9(3) VALUE 0.
   05 WS-D PIC 9(3) VALUE 0.
   05 WS-E PIC 9(3) VALUE 0.
"#,
        r#"    INSPECT WS-N TALLYING WS-C FOR ALL N"ç"
    INSPECT WS-N TALLYING WS-D FOR CHARACTERS BEFORE INITIAL N","
    INSPECT WS-N TALLYING WS-E FOR CHARACTERS
    DISPLAY WS-C " " WS-D " " WS-E
    INSPECT WS-N REPLACING CHARACTERS BY N"*" AFTER INITIAL N", "
    DISPLAY "[" WS-N "]"
"#,
    ),
    (
        "INSPECT: LEADING, TRAILING, FIRST, ALL, BEFORE and AFTER, and TALLYING with REPLACING",
        r#"   05 WS-W PIC N(16) VALUE N"  aaçãaa  bbb  ".
   05 WS-P PIC N(1) VALUE N"ç".
   05 WS-C PIC 9(3) VALUE 0.
   05 WS-D PIC 9(3) VALUE 0.
   05 WS-E PIC 9(3) VALUE 0.
"#,
        r#"    INSPECT WS-W TALLYING WS-C FOR LEADING N" " WS-D FOR TRAILING N" " WS-E FOR ALL N"a" BEFORE INITIAL WS-P
    DISPLAY WS-C " " WS-D " " WS-E
    INSPECT WS-W REPLACING FIRST N"a" BY N"A" AFTER INITIAL WS-P
    DISPLAY "[" WS-W "]"
    INSPECT WS-W REPLACING LEADING N" " BY N"_" TRAILING N" " BY N"-"
    DISPLAY "[" WS-W "]"
    INSPECT WS-W TALLYING WS-C FOR ALL N"b" REPLACING ALL N"b" BY N"B"
    DISPLAY "[" WS-W "] " WS-C
"#,
    ),
    (
        "INSPECT: CONVERTING, a replacement longer than the item, a supplementary character, CHARACTERS",
        r#"   05 WS-W PIC N(16) VALUE N"abç abç abç".
   05 WS-V PIC N(7) VALUE N"a😀b😀c".
   05 WS-C PIC 9(3) VALUE 0.
"#,
        r#"    INSPECT WS-W CONVERTING N"abç" TO N"ABC"
    DISPLAY "[" WS-W "]"
    INSPECT WS-W CONVERTING N"ABC" TO N"xyz" AFTER INITIAL N" "
    DISPLAY "[" WS-W "]"
    INSPECT WS-W REPLACING ALL N"y" BY N"ÿÿÿÿ"
    DISPLAY "[" WS-W "]"
    INSPECT WS-V TALLYING WS-C FOR CHARACTERS
    DISPLAY WS-C
    INSPECT WS-V REPLACING ALL N"b" BY N"B"
    DISPLAY "[" WS-V "]"
"#,
    ),
    (
        "INSPECT: a reference-modified national target, a JUSTIFIED RIGHT one, a UTF-8 one, an alphanumeric one with national patterns",
        r#"   05 WS-N PIC N(12) VALUE N"ação, coração".
   05 WS-JR PIC N(6) JUSTIFIED RIGHT VALUE N"abc".
   05 WS-U PIC U(8) VALUE U"naïve ñu".
   05 WS-X PIC X(20) VALUE "ação, coração".
   05 WS-C PIC 9(3) VALUE 0.
   05 WS-D PIC 9(3) VALUE 0.
"#,
        r#"    INSPECT WS-N(2:4) TALLYING WS-C FOR ALL N"ç" CHARACTERS
    DISPLAY WS-C
    INSPECT WS-N(7:) REPLACING ALL N"ç" BY N"c"
    DISPLAY "[" WS-N "]"
    INSPECT WS-JR REPLACING ALL N"b" BY N"BB"
    DISPLAY "[" WS-JR "]"
    INSPECT WS-U REPLACING ALL U"ñ" BY U"n"
    DISPLAY "[" WS-U "]"
    INSPECT WS-X TALLYING WS-D FOR ALL N"ç"
    DISPLAY WS-D
    INSPECT WS-X REPLACING ALL N"ç" BY "c"
    DISPLAY "[" WS-X "]"
"#,
    ),
    (
        "STRING and UNSTRING over a table of national items, a numeric sender, a figurative sender, no DELIMITED BY",
        r#"   05 T.
      10 T-N PIC N(6) OCCURS 3 TIMES INDEXED BY TX.
   05 WS-X PIC X(6) VALUE "ab".
   05 WS-A PIC N(3) VALUE N"çã".
   05 WS-NUM PIC 9(3) VALUE 42.
   05 WS-R PIC N(12).
   05 WS-P PIC 9(3).
"#,
        r#"    MOVE N"a,b,c" TO T-N(2)
    UNSTRING T-N(2) DELIMITED BY "," INTO T-N(1) T-N(3)
    DISPLAY "[" T-N(1) "][" T-N(2) "][" T-N(3) "]"
    STRING T-N(1) DELIMITED BY SPACE WS-NUM DELIMITED BY SIZE INTO WS-R
    DISPLAY "[" WS-R "]"
    MOVE 1 TO WS-P
    STRING WS-X WS-A DELIMITED BY SIZE INTO WS-R WITH POINTER WS-P
    DISPLAY "[" WS-R "] " WS-P
    STRING ZERO DELIMITED BY SIZE SPACE DELIMITED BY SIZE HIGH-VALUE DELIMITED BY SIZE INTO WS-R
    DISPLAY "[" WS-R "]"
    STRING WS-X WS-A INTO WS-R
    DISPLAY "[" WS-R "]"
"#,
    ),
    (
        "INSPECT a table element, a pattern held in a UTF-8 item, TALLYING two counters, and an unchanged item",
        r#"   05 T.
      10 T-N PIC N(6) OCCURS 2 TIMES.
   05 WS-PU PIC U(2) VALUE U"ãç".
   05 WS-C PIC 9(3) VALUE 5.
   05 WS-D PIC 9(3) VALUE 0.
"#,
        r#"    MOVE N"ãçãçãç" TO T-N(1)
    MOVE N"abc" TO T-N(2)
    INSPECT T-N(1) TALLYING WS-C FOR ALL WS-PU WS-D FOR ALL N"ã"
    DISPLAY WS-C " " WS-D
    INSPECT T-N(2) TALLYING WS-C FOR ALL N"z"
    DISPLAY "[" T-N(2) "] " WS-C
    INSPECT T-N(1) REPLACING ALL WS-PU BY N"xy"
    DISPLAY "[" T-N(1) "]"
"#,
    ),
];



/// Numbers in and out of national and UTF-8 items (T50a3): the interpreter keeps such an item as
/// text, so a number moved in is its digits or its text, and a number read out is what its text
/// spells.
const NUMBER_CASES: &[(&str, &str, &str)] = &[
    (
        "a number moved into a national or UTF-8 item: an item's digits, a literal's, a computed value's text",
        r#"   05 WS-N PIC N(6).
   05 WS-U PIC U(6).
   05 WS-9 PIC 9(4) VALUE 12.
   05 WS-S PIC S9(3)V99 VALUE -12.5.
   05 WS-X PIC X(8) VALUE "abcdefgh".
01 OUT.
   05 WS-B PIC 9(4) COMP VALUE 345.
   05 WS-P PIC S9(5) COMP-3 VALUE 678.
"#,
        r#"    MOVE 5 TO WS-N
    DISPLAY "[" WS-N "]"
    MOVE 0012 TO WS-N
    DISPLAY "[" WS-N "]"
    MOVE WS-9 TO WS-N
    DISPLAY "[" WS-N "]"
    MOVE WS-S TO WS-U
    DISPLAY "[" WS-U "]"
    MOVE WS-B TO WS-N
    DISPLAY "[" WS-N "]"
    MOVE WS-P TO WS-U
    DISPLAY "[" WS-U "]"
    MOVE 1234567 TO WS-N
    DISPLAY "[" WS-N "]"
    MOVE 1.5 TO WS-N
    DISPLAY "[" WS-N "]"
    MOVE -7 TO WS-N
    DISPLAY "[" WS-N "]"
    MOVE FUNCTION LENGTH(WS-X) TO WS-N
    DISPLAY "[" WS-N "]"
    MOVE FUNCTION NUMVAL("-3.5") TO WS-U
    DISPLAY "[" WS-U "]"
"#,
    ),
    (
        "a national or UTF-8 item moved into a number: what its text spells, else the receiver as it was",
        r#"   05 WS-N PIC N(6) VALUE N"  42".
   05 WS-M PIC N(6) VALUE N"-3.75".
   05 WS-K PIC N(6) VALUE N"abc".
   05 WS-E PIC N(6) VALUE N"12abc".
   05 WS-U PIC U(6) VALUE U"1000".
   05 WS-9 PIC 9(4) VALUE 7.
   05 WS-S PIC S9(3)V99 VALUE 0.
01 OUT.
   05 WS-B PIC 9(4) COMP VALUE 7.
   05 WS-P PIC S9(5) COMP-3 VALUE 7.
"#,
        r#"    MOVE WS-N TO WS-9
    DISPLAY WS-9
    MOVE WS-M TO WS-S
    DISPLAY WS-S
    MOVE 9 TO WS-9
    MOVE WS-K TO WS-9
    DISPLAY WS-9
    MOVE WS-E TO WS-9
    DISPLAY WS-9
    MOVE WS-N TO WS-B
    DISPLAY WS-B
    MOVE WS-M TO WS-P
    DISPLAY WS-P
    MOVE WS-U TO WS-9
    DISPLAY WS-9
"#,
    ),
    (
        "a national item against a number, ZERO and a class",
        r#"   05 WS-N PIC N(6) VALUE N"10".
   05 WS-M PIC N(6) VALUE N"abc".
   05 WS-Z PIC N(6) VALUE N"0".
   05 WS-U PIC U(6) VALUE U"ab".
   05 WS-9 PIC 9(4) VALUE 10.
"#,
        r#"    IF WS-N = 10 DISPLAY "N=10" ELSE DISPLAY "N#10" END-IF
    IF WS-N > 9 DISPLAY "N>9" ELSE DISPLAY "N<=9" END-IF
    IF WS-N = WS-9 DISPLAY "N=9" ELSE DISPLAY "N#9" END-IF
    IF WS-M = 10 DISPLAY "M=10" ELSE DISPLAY "M#10" END-IF
    IF WS-M > 5 DISPLAY "M>5" ELSE DISPLAY "M<=5" END-IF
    IF WS-Z = ZERO DISPLAY "Z=0" ELSE DISPLAY "Z#0" END-IF
    IF WS-N = ZERO DISPLAY "N=0" ELSE DISPLAY "N#0" END-IF
    IF WS-N IS NUMERIC DISPLAY "N num" ELSE DISPLAY "N not num" END-IF
    IF WS-M IS NUMERIC DISPLAY "M num" ELSE DISPLAY "M not num" END-IF
    IF WS-M IS ALPHABETIC DISPLAY "M alpha" ELSE DISPLAY "M not alpha" END-IF
    IF WS-N IS NOT ALPHABETIC DISPLAY "N not alpha" ELSE DISPLAY "N alpha" END-IF
    IF WS-U IS ALPHABETIC-LOWER DISPLAY "U lower" ELSE DISPLAY "U not lower" END-IF
"#,
    ),
    (
        "condition-names over national and UTF-8 hosts, SET TO TRUE, EVALUATE TRUE",
        r#"   05 WS-N PIC N(3) VALUE N"Y".
      88 IS-Y VALUE N"Y" N"y".
      88 IS-RANGE VALUE N"A" THRU N"F".
      88 IS-ALNUM VALUE "Y".
   05 WS-U PIC U(2) VALUE U"é".
      88 IS-E VALUE U"é".
"#,
        r#"    IF IS-Y DISPLAY "Y" ELSE DISPLAY "not Y" END-IF
    IF IS-ALNUM DISPLAY "alnum Y" ELSE DISPLAY "not alnum Y" END-IF
    IF IS-RANGE DISPLAY "range" ELSE DISPLAY "not range" END-IF
    SET IS-RANGE TO TRUE
    DISPLAY "[" WS-N "]"
    IF IS-RANGE DISPLAY "range" ELSE DISPLAY "not range" END-IF
    IF IS-E DISPLAY "E" ELSE DISPLAY "not E" END-IF
    EVALUATE TRUE
        WHEN IS-Y DISPLAY "w Y"
        WHEN IS-E DISPLAY "w E"
        WHEN OTHER DISPLAY "w other"
    END-EVALUATE
    SET IS-Y TO TRUE
    DISPLAY "[" WS-N "]"
"#,
    ),
    (
        "the text of a national item read as a number: spaces, signs, a point, an exponent, too many digits, no number",
        r#"   05 A1 PIC N(8) VALUE N" 42 ".
   05 A2 PIC N(8) VALUE N"4 2".
   05 A3 PIC N(8) VALUE N"+5".
   05 A4 PIC N(8) VALUE N"1e".
   05 A5 PIC N(8) VALUE N".5".
   05 A6 PIC N(8) VALUE N"5.".
   05 A7 PIC N(8) VALUE N"--3".
   05 A8 PIC N(8) VALUE N"1,5".
   05 A9 PIC N(8) VALUE N"12345678".
   05 AA PIC N(8) VALUE N"007".
   05 AB PIC N(8) VALUE N"".
   05 U1 PIC U(8) VALUE U"-12.345".
   05 W9 PIC 9(4) VALUE 7.
   05 WS PIC S9(3)V99 VALUE 1.
   05 WU PIC 9(2) VALUE 7.
"#,
        r#"    MOVE A1 TO W9 DISPLAY "A1 " W9
    MOVE 7 TO W9 MOVE A2 TO W9 DISPLAY "A2 " W9
    MOVE 7 TO W9 MOVE A3 TO W9 DISPLAY "A3 " W9
    MOVE 7 TO W9 MOVE A4 TO W9 DISPLAY "A4 " W9
    MOVE 7 TO W9 MOVE A5 TO W9 DISPLAY "A5 " W9
    MOVE 7 TO W9 MOVE A6 TO W9 DISPLAY "A6 " W9
    MOVE 7 TO W9 MOVE A7 TO W9 DISPLAY "A7 " W9
    MOVE 7 TO W9 MOVE A8 TO W9 DISPLAY "A8 " W9
    MOVE 7 TO W9 MOVE A9 TO W9 DISPLAY "A9 " W9
    MOVE 7 TO W9 MOVE AA TO W9 DISPLAY "AA " W9
    MOVE 7 TO W9 MOVE AB TO W9 DISPLAY "AB " W9
    MOVE A5 TO WS DISPLAY "A5S " WS
    MOVE U1 TO WS DISPLAY "U1S " WS
    MOVE U1 TO WU DISPLAY "U1U " WU
    MOVE A9 TO WU DISPLAY "A9U " WU
"#,
    ),
    (
        "a national or UTF-8 item against ZERO, an integer literal, a numeric item with and without decimals",
        r#"   05 A1 PIC N(6) VALUE N"00".
   05 A2 PIC N(6) VALUE N"0.0".
   05 A3 PIC N(6) VALUE N"abc".
   05 A4 PIC N(6) VALUE N"5".
   05 A5 PIC N(6) VALUE N"1e-20".
   05 A6 PIC N(6) VALUE N" -0".
   05 A7 PIC N(6) VALUE N"42".
   05 U1 PIC U(6) VALUE U"7".
   05 W9 PIC 9(4) VALUE 42.
   05 WS PIC S9(4) VALUE -42.
   05 W1 PIC 9 VALUE 0.
"#,
        r#"    IF A1 = ZERO DISPLAY "A1=0" ELSE DISPLAY "A1#0" END-IF
    IF A2 = ZERO DISPLAY "A2=0" ELSE DISPLAY "A2#0" END-IF
    IF A3 = ZERO DISPLAY "A3=0" ELSE DISPLAY "A3#0" END-IF
    IF A3 > ZERO DISPLAY "A3>0" ELSE DISPLAY "A3<=0" END-IF
    IF A4 > ZERO DISPLAY "A4>0" ELSE DISPLAY "A4<=0" END-IF
    IF ZERO < A4 DISPLAY "0<A4" ELSE DISPLAY "0>=A4" END-IF
    IF A5 = ZERO DISPLAY "A5=0" ELSE DISPLAY "A5#0" END-IF
    IF A6 = ZERO DISPLAY "A6=0" ELSE DISPLAY "A6#0" END-IF
    IF A7 = 42 DISPLAY "A7=42" ELSE DISPLAY "A7#42" END-IF
    IF A7 = 0042 DISPLAY "A7=0042" ELSE DISPLAY "A7#0042" END-IF
    IF A7 = W9 DISPLAY "A7=W9" ELSE DISPLAY "A7#W9" END-IF
    IF A7 < W9 DISPLAY "A7<W9" ELSE DISPLAY "A7>=W9" END-IF
    IF A7 = WS DISPLAY "A7=WS" ELSE DISPLAY "A7#WS" END-IF
    IF U1 = 7 DISPLAY "U1=7" ELSE DISPLAY "U1#7" END-IF
    IF U1 = W1 DISPLAY "U1=W1" ELSE DISPLAY "U1#W1" END-IF
    IF W1 = A1 DISPLAY "W1=A1" ELSE DISPLAY "W1#A1" END-IF
    IF 5 = A4 DISPLAY "5=A4" ELSE DISPLAY "5#A4" END-IF
    IF A4 NOT = 4 DISPLAY "A4#4" ELSE DISPLAY "A4=4" END-IF
"#,
    ),
    (
        "a number into JUSTIFIED RIGHT, UTF-8 and table receivers; a value computed for several receivers; an integer literal with leading zeros",
        r#"   05 WS-J PIC N(6) JUSTIFIED RIGHT.
   05 WS-U PIC U(6).
   05 T-N PIC N(5) OCCURS 3.
   05 WS-9 PIC 9(3) VALUE 45.
   05 WS-V PIC S9(2)V9 VALUE -1.5.
   05 WS-A PIC X(6).
   05 WS-Z PIC N(3).
"#,
        r#"    MOVE WS-9 TO WS-J
    DISPLAY "[" WS-J "]"
    MOVE 7 TO WS-J
    DISPLAY "[" WS-J "]"
    MOVE -5 TO WS-J
    DISPLAY "[" WS-J "]"
    MOVE 7 TO WS-U WS-J
    DISPLAY "[" WS-U "][" WS-J "]"
    MOVE FUNCTION LENGTH(WS-A) TO WS-J
    DISPLAY "[" WS-J "]"
    MOVE WS-9 TO WS-U
    DISPLAY "[" WS-U "]"
    MOVE WS-V TO WS-U
    DISPLAY "[" WS-U "]"
    MOVE WS-9 TO T-N(2)
    DISPLAY "[" T-N(1) "][" T-N(2) "][" T-N(3) "]"
    MOVE 0012 TO T-N(3)
    DISPLAY "[" T-N(3) "]"
    MOVE 2.50 TO WS-U WS-J
    DISPLAY "[" WS-U "][" WS-J "]"
    MOVE ZERO TO WS-U
    DISPLAY "[" WS-U "]"
    MOVE 100 TO WS-Z
    DISPLAY "[" WS-Z "]"
    COMPUTE WS-9 = 7 * 11
    MOVE WS-9 TO WS-Z
    DISPLAY "[" WS-Z "]"
    MOVE WS-U TO WS-A
    DISPLAY "[" WS-A "]"
    MOVE WS-9 TO WS-A
    DISPLAY "[" WS-A "]"
"#,
    ),
    (
        "class tests over national and UTF-8 text: a full item, padding, an accent, a literal",
        r#"   05 A1 PIC N(2) VALUE N"10".
   05 A2 PIC N(4) VALUE N"ab".
   05 A3 PIC N(4) VALUE N"AB".
   05 A4 PIC N(4) VALUE N"aB".
   05 A5 PIC N(3) VALUE N"çb".
   05 U1 PIC U(3) VALUE U"abc".
   05 U2 PIC U(3) VALUE U"12 ".
"#,
        r#"    IF A1 IS NUMERIC DISPLAY "A1 num" ELSE DISPLAY "A1 not num" END-IF
    IF A2 IS NUMERIC DISPLAY "A2 num" ELSE DISPLAY "A2 not num" END-IF
    IF A2 IS ALPHABETIC DISPLAY "A2 alpha" ELSE DISPLAY "A2 not alpha" END-IF
    IF A2 IS ALPHABETIC-LOWER DISPLAY "A2 lower" ELSE DISPLAY "A2 not lower" END-IF
    IF A2 IS ALPHABETIC-UPPER DISPLAY "A2 upper" ELSE DISPLAY "A2 not upper" END-IF
    IF A3 IS ALPHABETIC-UPPER DISPLAY "A3 upper" ELSE DISPLAY "A3 not upper" END-IF
    IF A4 IS ALPHABETIC-LOWER DISPLAY "A4 lower" ELSE DISPLAY "A4 not lower" END-IF
    IF A5 IS ALPHABETIC DISPLAY "A5 alpha" ELSE DISPLAY "A5 not alpha" END-IF
    IF U1 IS ALPHABETIC-LOWER DISPLAY "U1 lower" ELSE DISPLAY "U1 not lower" END-IF
    IF U2 IS NUMERIC DISPLAY "U2 num" ELSE DISPLAY "U2 not num" END-IF
    IF U2 IS NOT NUMERIC DISPLAY "U2 not num" ELSE DISPLAY "U2 num" END-IF
    IF N"abc" IS ALPHABETIC DISPLAY "lit alpha" ELSE DISPLAY "lit not alpha" END-IF
    IF N"123" IS NUMERIC DISPLAY "lit num" ELSE DISPLAY "lit not num" END-IF
"#,
    ),
    (
        "condition-names over a national table element, a range, SET TO TRUE, a UTF-8 host and a value of the other class",
        r#"   05 T-N PIC N(3) OCCURS 2.
      88 T-ON VALUE N"ON" N"on".
      88 T-LOW VALUE N"a" THRU N"m".
   05 WS-U PIC U(4) VALUE U"ação".
      88 U-OK VALUE U"ação" "ok".
      88 U-OTHER VALUE "xyz".
"#,
        r#"    MOVE N"on" TO T-N(1)
    MOVE N"zz" TO T-N(2)
    IF T-ON(1) DISPLAY "1 on" ELSE DISPLAY "1 off" END-IF
    IF T-ON(2) DISPLAY "2 on" ELSE DISPLAY "2 off" END-IF
    IF T-LOW(1) DISPLAY "1 low" ELSE DISPLAY "1 high" END-IF
    IF T-LOW(2) DISPLAY "2 low" ELSE DISPLAY "2 high" END-IF
    MOVE N"c" TO T-N(2)
    DISPLAY "[" T-N(2) "]"
    IF T-LOW(2) DISPLAY "2 low" ELSE DISPLAY "2 high" END-IF
    IF U-OK DISPLAY "u ok" ELSE DISPLAY "u not ok" END-IF
    IF U-OTHER DISPLAY "u other" ELSE DISPLAY "u not other" END-IF
    SET U-OTHER TO TRUE
    DISPLAY "[" WS-U "]"
    IF U-OTHER DISPLAY "u other" ELSE DISPLAY "u not other" END-IF
    IF NOT U-OK DISPLAY "u not ok" ELSE DISPLAY "u ok" END-IF
"#,
    ),
];

#[test]
fn national_and_utf8_data_compute_as_the_interpreter_computes_them() {
    let (mut t_compile, mut t_module, mut t_interp) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let (mut lines, mut bytes, mut runs) = (0usize, 0usize, 0usize);
    let mut results: Vec<(String, Result<(), String>)> = Vec::new();
    for (name, items, procedure) in CASES.iter().chain(FUNCTION_CASES).chain(VERB_CASES).chain(NUMBER_CASES) {
        let src = program(items, procedure);
        let t = Instant::now();
        let (want_lines, want_image) = interpreter(&src);
        t_interp += t.elapsed();
        let want_image = want_image.expect("the interpreter holds G");
        let mut outcome = Ok(());
        for multi in [true, false] {
            let t = Instant::now();
            let compiled = compile_program(&common::parse(&src), &native(multi));
            t_compile += t.elapsed();
            let c = match compiled {
                Ok(c) => c,
                Err(e) => {
                    outcome = Err(format!("multi={multi}: not compiled: {e}"));
                    break;
                }
            };
            let t = Instant::now();
            let (o, memory) = run_native_memory(&c.wasm, NativeInput::default()).expect("the host runs it");
            t_module += t.elapsed();
            runs += 1;
            let got_lines = common::wasm_lines(&o);
            let got_image = module_group(&memory);
            if o.trapped.is_some() || !o.errors.is_empty() {
                outcome = Err(format!("multi={multi}: trapped {:?} errors {:?}", o.trapped, o.errors));
            } else if got_lines != want_lines {
                outcome = Err(format!("multi={multi}: DISPLAY differs\n  module      {got_lines:?}\n  interpreter {want_lines:?}"));
            } else if got_image != want_image {
                outcome = Err(format!(
                    "multi={multi}: the bytes of G differ\n  module      {}\n  interpreter {}",
                    hex(&got_image),
                    hex(&want_image)
                ));
            } else {
                lines += got_lines.len();
                bytes += got_image.len();
                continue;
            }
            break;
        }
        results.push((name.to_string(), outcome));
    }
    // The nested programs: lines only (their items are not under G).
    let standard = COLLATING_NATIVE.replace("@ALPHABET@", "STANDARD-1");
    let native_seq = COLLATING_NATIVE.replace("@ALPHABET@", "NATIVE");
    for (name, src) in [
        ("a nested program's national item keeps its class", NESTED),
        ("a national and a UTF-8 item passed by reference, by content and by value; a table element; a literal; a number", CALL_PARAMS),
        ("a SORT on a national key: ascending, descending, a second key, accents and capitals", SORT_KEYS),
        ("a PROGRAM COLLATING SEQUENCE of STANDARD-1 over national, UTF-8 and alphanumeric data", standard.as_str()),
        ("a PROGRAM COLLATING SEQUENCE of NATIVE over national, UTF-8 and alphanumeric data", native_seq.as_str()),
    ] {
        let t = Instant::now();
        let (want, _) = interpreter(src);
        t_interp += t.elapsed();
        let mut outcome = Ok(());
        for multi in [true, false] {
            let t = Instant::now();
            let c = compile_program(&common::parse(src), &native(multi));
            t_compile += t.elapsed();
            match c {
                Ok(c) => {
                    let t = Instant::now();
                    let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
                    t_module += t.elapsed();
                    runs += 1;
                    let got = common::wasm_lines(&o);
                    if got != want {
                        outcome = Err(format!("multi={multi}: DISPLAY differs\n  module      {got:?}\n  interpreter {want:?}"));
                        break;
                    }
                    lines += got.len();
                }
                Err(e) => {
                    outcome = Err(format!("multi={multi}: not compiled: {e}"));
                    break;
                }
            }
        }
        results.push((name.into(), outcome));
    }

    let passed = results.iter().filter(|(_, r)| r.is_ok()).count();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("== native_national: national and UTF-8 data, module against interpreter (DESKTOP-WASM-001 T44, T50a1, T50a2, T50a3a, T50a3b, T50a4) ==");
    println!("cases: {} (each in both memory layouts: {runs} module runs)", results.len());
    for (name, r) in &results {
        match r {
            Ok(()) => println!("  pass {name}"),
            Err(e) => println!("  FAIL {name}: {e}"),
        }
    }
    println!(
        "compared: {lines} DISPLAY lines and {bytes} bytes of item images, all identical where passed\n\
         timings: interpreter {:.1} ms; compile {:.1} ms ({:.0} modules/s); module runs {:.1} ms ({:.0} runs/s)",
        ms(t_interp),
        ms(t_compile),
        runs as f64 / t_compile.as_secs_f64().max(1e-9),
        ms(t_module),
        runs as f64 / t_module.as_secs_f64().max(1e-9),
    );
    println!("result: {passed} of {} cases identical", results.len());
    let failed: Vec<&String> = results.iter().filter(|(_, r)| r.is_err()).map(|(n, _)| n).collect();
    assert!(failed.is_empty(), "failed: {failed:?}");
}

/// `SET T-LOW (2) TO TRUE` over a condition-name under a table element: the module sets the
/// element, as COBOL-85 says (T50a3a). The interpreter leaves the element as it was, so this one is
/// not a twin (queued as a separate fix: the interpreter's SET of a subscripted condition-name).
#[test]
fn a_condition_name_under_a_table_element_is_set_by_the_module() {
    let src = program(
        "   05 T-N PIC N(3) OCCURS 2.\n      88 T-LOW VALUE N\"a\" THRU N\"m\".\n      88 T-ON VALUE N\"ON\".\n",
        "    MOVE N\"zz\" TO T-N(1) T-N(2)\n    SET T-LOW(2) TO TRUE\n    DISPLAY \"[\" T-N(1) \"][\" T-N(2) \"]\"\n\
         \x20   IF T-LOW(1) DISPLAY \"1 low\" ELSE DISPLAY \"1 high\" END-IF\n\
         \x20   IF T-LOW(2) DISPLAY \"2 low\" ELSE DISPLAY \"2 high\" END-IF\n\
         \x20   SET T-ON(1) TO TRUE\n    DISPLAY \"[\" T-N(1) \"][\" T-N(2) \"]\"\n",
    );
    for multi in [true, false] {
        let c = compile_program(&common::parse(&src), &native(multi)).expect("compiles");
        let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
        assert_eq!(common::wasm_lines(&o), ["[zz ][a  ]", "1 high", "2 low", "[ON ][a  ]"], "multi={multi}");
    }
    println!("a condition-name under a table element: SET TO TRUE sets the element (2 layouts), as COBOL-85 says");
}

/// A `PROGRAM COLLATING SEQUENCE` orders alphanumeric comparison only: national and UTF-8 text
/// compare by code point whatever the sequence says (COBOL: the program collating sequence is the
/// alphanumeric one). The interpreter weighs such text by the alphabet's table through its UTF-8
/// bytes, so under a sequence that reorders it differs; this is not a twin, and the interpreter's
/// rule is queued as a separate fix (T50a4).
#[test]
fn a_program_collating_sequence_orders_alphanumeric_data_only() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T50Z.\nENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\n\
               OBJECT-COMPUTER. THIS-MACHINE PROGRAM COLLATING SEQUENCE IS ZED.\nSPECIAL-NAMES.\n    ALPHABET ZED IS \"Z\".\n\
               DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-N PIC N(4).\n01 WS-M PIC N(4).\n01 WS-U PIC U(4).\n01 WS-X PIC X(4).\n\
               PROCEDURE DIVISION.\nMAIN-PARA.\n\
               \x20   IF \"Z\" < \"A\" DISPLAY \"alnum: Z first\" ELSE DISPLAY \"alnum: A first\" END-IF\n\
               \x20   MOVE N\"Z\" TO WS-N\n    MOVE N\"A\" TO WS-M\n\
               \x20   IF WS-N < WS-M DISPLAY \"national: Z first\" ELSE DISPLAY \"national: A first\" END-IF\n\
               \x20   IF WS-N < N\"A\" DISPLAY \"national lit: Z first\" ELSE DISPLAY \"national lit: A first\" END-IF\n\
               \x20   MOVE U\"Z\" TO WS-U\n\
               \x20   IF WS-U < U\"A\" DISPLAY \"utf8: Z first\" ELSE DISPLAY \"utf8: A first\" END-IF\n\
               \x20   MOVE \"Z\" TO WS-X\n\
               \x20   IF WS-N = WS-X DISPLAY \"n eq x\" ELSE DISPLAY \"n ne x\" END-IF\n\
               \x20   IF WS-X < \"A\" DISPLAY \"x: Z first\" ELSE DISPLAY \"x: A first\" END-IF\n    STOP RUN.\n";
    let want = ["alnum: Z first", "national: A first", "national lit: A first", "utf8: A first", "n eq x", "x: Z first"];
    for multi in [true, false] {
        let c = compile_program(&common::parse(src), &native(multi)).expect("a national item beside a PROGRAM COLLATING SEQUENCE compiles");
        let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
        assert_eq!(common::wasm_lines(&o), want, "multi={multi}");
    }
    println!("a PROGRAM COLLATING SEQUENCE that reorders: alphanumeric Z first, national and UTF-8 by code point (6 lines x 2 layouts)");
}

/// A code page a data item names and nothing converts ends the run, in the interpreter's words
/// (DESKTOP-WASM-001 T50a1): what ran before it has shown, what follows has not.
#[test]
fn an_unknown_code_page_in_a_data_item_ends_the_run_in_the_interpreters_words() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T50A.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-CP PIC X(8) VALUE \"ebcdic\".\n\
               01 WS-X PIC X(4) VALUE \"abcd\".\n01 WS-N PIC N(4).\nPROCEDURE DIVISION.\nMAIN-PARA.\n    DISPLAY \"before\"\n\
               \x20   MOVE FUNCTION NATIONAL-OF(WS-X, WS-CP) TO WS-N\n    DISPLAY \"after\"\n    STOP RUN.\n";
    let want = "'ebcdic' is not a code page RustCOBOL converts; use UTF-8 (1208), WINDOWS-1252 (1252), ISO-8859-1 (819)";
    let prog = common::parse(src);
    let (_event_tx, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    let failure = interp.run().expect_err("the interpreter ends the run").to_string();
    let shown: Vec<String> = display_rx.try_iter().map(|s: String| s.trim_end_matches(['\n', '\r']).to_owned()).collect();
    assert!(failure.contains(want), "the interpreter said: {failure}");
    assert_eq!(shown, ["before"]);
    let mut checks = 0;
    for multi in [true, false] {
        let c = compile_program(&common::parse(src), &native(multi)).expect("the module compiles");
        let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
        assert_eq!(common::wasm_lines(&o), ["before"], "multi={multi}: what ran before the error shows, what follows does not");
        assert!(o.error_texts.iter().any(|m| m.contains(want)), "multi={multi}: the module said {:?}", o.error_texts);
        checks += 2;
    }
    println!("== native_national: an unknown code page in a data item (T50a1) ==\nmessage: {want}\nresult: {checks} of 4 checks identical to the interpreter");
}
