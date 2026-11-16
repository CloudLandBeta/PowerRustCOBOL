// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T16: what a desktop module refuses, and why. The Desktop Build turns each
//! refusal into the reason it takes the Cargo path, so each one must name its construct, carry
//! the reason of its Desktop class (T10), stand at the line of its statement and say which
//! program unit it is in. What the runner can serve is compiled; a browser module is unchanged.

mod common;

use cobolt_wasm::{compile_program, GenError, Options};
use std::time::Instant;

fn native() -> Options {
    Options { native: true, ..Options::default() }
}

const HEAD: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T16G.\n";

fn line_of(src: &str, needle: &str) -> u32 {
    src.lines().position(|l| l.contains(needle)).map(|i| i as u32 + 1).expect("the line is in the source")
}

/// The first refusal of `src` as a desktop module.
fn refusal(src: &str) -> GenError {
    match compile_program(&common::parse(src), &native()) {
        Ok(_) => panic!("the desktop module should be refused:\n{src}"),
        Err(e) => e,
    }
}

#[test]
fn a_desktop_module_refuses_what_the_runner_cannot_serve_with_its_reason() {
    let t = Instant::now();
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |what: String, ok: bool| checks.push((what, ok));

    // A file on disk: refused at its verb, with M3's reason.
    let disk = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT LOG-F ASSIGN TO \"t16.log\"\n        ORGANIZATION IS LINE SEQUENTIAL.\n\
         DATA DIVISION.\nFILE SECTION.\nFD LOG-F.\n01 LOG-REC PIC X(20).\nPROCEDURE DIVISION.\n    OPEN OUTPUT LOG-F\n    CLOSE LOG-F\n    STOP RUN.\n"
    );
    let e = refusal(&disk);
    check(
        format!("a file on disk: \"file LOG-F\" at line {}, files are not on the path yet (M3), unit T16G", line_of(&disk, "OPEN OUTPUT")),
        e.subject == "file LOG-F" && e.line == line_of(&disk, "OPEN OUTPUT") && e.message.contains("files are not on the WebAssembly path yet") && e.message.contains("M3") && e.program == "T16G",
    );
    let browser = compile_program(&common::parse(&disk), &Options::default()).err().map(|e| e.message).unwrap_or_default();
    check("the same file in a browser module keeps the browser's refusal".into(), !browser.is_empty() && !browser.contains("WebAssembly path"));

    // SORT with USING and GIVING: refused, with M3's reason.
    let using = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n    SELECT IN-F ASSIGN TO \"in\".\n\
         DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nFD IN-F.\n01 IN-REC PIC X(5).\nPROCEDURE DIVISION.\n    SORT W ON ASCENDING KEY W-REC USING IN-F GIVING IN-F.\n    STOP RUN.\n"
    );
    let e = refusal(&using);
    check(
        "SORT … USING/GIVING: refused at its line, SORT files (M3)".into(),
        e.subject == "SORT … USING/GIVING" && e.line == line_of(&using, "SORT W") && e.message.contains("SORT with USING or GIVING") && e.message.contains("M3"),
    );

    // SORT with an INPUT and an OUTPUT PROCEDURE: compiled into the module.
    let procs = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n\
         DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nWORKING-STORAGE SECTION.\n01 EOF PIC 9 VALUE 0.\n\
         PROCEDURE DIVISION.\nMAIN-PARA.\n    SORT W ON ASCENDING KEY W-REC INPUT PROCEDURE IS GIVE OUTPUT PROCEDURE IS TAKE\n    STOP RUN.\n\
         GIVE.\n    MOVE \"B\" TO W-REC\n    RELEASE W-REC\n    MOVE \"A\" TO W-REC\n    RELEASE W-REC.\n\
         TAKE.\n    PERFORM UNTIL EOF = 1\n        RETURN W AT END MOVE 1 TO EOF NOT AT END DISPLAY W-REC END-RETURN\n    END-PERFORM.\n"
    );
    let sorted = compile_program(&common::parse(&procs), &native());
    check("SORT with INPUT and OUTPUT PROCEDUREs compiles into a desktop module".into(), sorted.is_ok());

    // A built-in: refused at its line, with the reason of its Desktop class.
    let http = format!(
        "{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-URL PIC X(40) VALUE \"https://example.invalid/\".\n01 WS-BODY PIC X(80).\n01 WS-ST PIC 9(3).\n\
         PROCEDURE DIVISION.\n    COBOL::\"HTTP-GET\" ( WS-URL WS-BODY WS-ST )\n    STOP RUN.\n"
    );
    let e = refusal(&http);
    check(
        "COBOL::\"HTTP-GET\": refused at its line, HTTP (M4)".into(),
        e.subject == "COBOL::\"HTTP-GET\"" && e.line == line_of(&http, "HTTP-GET") && e.message.contains("HTTP is not on the WebAssembly path yet") && e.message.contains("M4"),
    );

    // The form and the application object: refused at the member, with M2's reason.
    let me = format!("{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-T PIC X(20).\nPROCEDURE DIVISION.\n    MOVE ME::Title TO WS-T\n    STOP RUN.\n");
    let e = refusal(&me);
    check(
        "ME::Title read: refused as ME::TITLE at its line, forms (M2)".into(),
        e.subject == "ME::TITLE" && e.line == line_of(&me, "ME::Title") && e.message.contains("forms, their controls and the application object") && e.message.contains("M2"),
    );
    let set = format!("{HEAD}PROCEDURE DIVISION.\n    MOVE \"x\" TO ME::Title\n    STOP RUN.\n");
    let e = refusal(&set);
    check("ME::Title written: refused as ME::TITLE at its line, forms (M2)".into(), e.subject == "ME::TITLE" && e.line == line_of(&set, "ME::Title") && e.message.contains("M2"));
    let invoke = format!("{HEAD}PROCEDURE DIVISION.\n    INVOKE ME \"Close\"\n    STOP RUN.\n");
    let e = refusal(&invoke);
    check("INVOKE: refused at its line, forms (M2)".into(), e.subject == "INVOKE" && e.line == line_of(&invoke, "INVOKE") && e.message.contains("M2"));

    // A nested program's refusal names that program.
    let nested = format!(
        "{HEAD}PROCEDURE DIVISION.\n    CALL \"INNER\"\n    STOP RUN.\n\
         IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-T PIC X(20).\nPROCEDURE DIVISION.\n    MOVE ME::Title TO WS-T\n    GOBACK.\nEND PROGRAM INNER.\nEND PROGRAM T16G.\n"
    );
    let e = refusal(&nested);
    check("a refusal in the nested program INNER names INNER".into(), e.program == "INNER" && e.line == line_of(&nested, "ME::Title"));

    // What the runner serves still compiles.
    let plain = format!("{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-N PIC 9(4) VALUE 1.\nPROCEDURE DIVISION.\n    ADD 41 TO WS-N\n    DISPLAY \"N=\" WS-N\n    STOP RUN.\n");
    check("a console program compiles into a desktop module".into(), compile_program(&common::parse(&plain), &native()).is_ok());

    let took = t.elapsed().as_secs_f64() * 1000.0;
    println!("== native_desktop_refusals (DESKTOP-WASM-001 T16, R13) ==");
    println!("programs compiled: 10 (8 as desktop modules refused or accepted, 1 browser module, 1 plain console program) in {took:.1} ms");
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    assert!(failed.is_empty(), "failed: {failed:?}");
}
