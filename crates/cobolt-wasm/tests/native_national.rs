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
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T44N.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 MARK-A PIC X(16) VALUE \"<<<<GROUP-START>\".\n01 G.\n{items}\
         01 MARK-B PIC X(16) VALUE \"<GROUP-END>>>>>>\".\nPROCEDURE DIVISION.\nMAIN-PARA.\n{procedure}    STOP RUN.\n"
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

#[test]
fn national_and_utf8_data_compute_as_the_interpreter_computes_them() {
    let (mut t_compile, mut t_module, mut t_interp) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let (mut lines, mut bytes, mut runs) = (0usize, 0usize, 0usize);
    let mut results: Vec<(String, Result<(), String>)> = Vec::new();
    for (name, items, procedure) in CASES.iter().chain(FUNCTION_CASES) {
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
    // The nested program: lines only (its items are not under G).
    let t = Instant::now();
    let (want, _) = interpreter(NESTED);
    t_interp += t.elapsed();
    let mut nested = Ok(());
    for multi in [true, false] {
        let t = Instant::now();
        let c = compile_program(&common::parse(NESTED), &native(multi));
        t_compile += t.elapsed();
        match c {
            Ok(c) => {
                let t = Instant::now();
                let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
                t_module += t.elapsed();
                runs += 1;
                let got = common::wasm_lines(&o);
                if got != want {
                    nested = Err(format!("multi={multi}: DISPLAY differs\n  module      {got:?}\n  interpreter {want:?}"));
                    break;
                }
                lines += got.len();
            }
            Err(e) => {
                nested = Err(format!("multi={multi}: not compiled: {e}"));
                break;
            }
        }
    }
    results.push(("a nested program's national item keeps its class".into(), nested));

    let passed = results.iter().filter(|(_, r)| r.is_ok()).count();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("== native_national: national and UTF-8 data, module against interpreter (DESKTOP-WASM-001 T44, T50a1) ==");
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
