// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T16 and T32: what a desktop module refuses, and why. The Desktop Build turns each
//! refusal into the reason it takes the Cargo path, so each one must name its construct, carry
//! the reason of its Desktop class (T10), stand at the line of its statement and say which
//! program unit it is in. What the runner can serve is compiled; a browser module is unchanged.

mod common;

use cobolt_wasm::lower_remote::{APP_VERBS, MOBILE_VERBS};
use cobolt_wasm::{compile_program, compile_project, FormSpec, GenError, Options};
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

    // A file on disk: the runner keeps it (T38). The module asks for its organisation's capability
    // and carries its declaration in `prfiles`.
    let disk = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT LOG-F ASSIGN TO \"t16.log\"\n        ORGANIZATION IS LINE SEQUENTIAL.\n\
         DATA DIVISION.\nFILE SECTION.\nFD LOG-F.\n01 LOG-REC PIC X(20).\nPROCEDURE DIVISION.\n    OPEN OUTPUT LOG-F\n    CLOSE LOG-F\n    STOP RUN.\n"
    );
    let module = compile_program(&common::parse(&disk), &native()).map(|m| m.wasm);
    let scan = module.as_ref().ok().and_then(|m| cobolt_wasm_abi::scan::scan_module(m).ok());
    let caps = scan.as_ref().and_then(|s| s.embedded_manifest().ok()).map(|m| m.capabilities).unwrap_or_default();
    let files = scan
        .as_ref()
        .and_then(|s| s.custom(cobolt_wasm_abi::files::FILES_SECTION))
        .and_then(|b| cobolt_wasm_abi::files::FilesSection::decode(b).ok())
        .map(|f| f.files)
        .unwrap_or_default();
    check(
        "a LINE SEQUENTIAL file on disk compiles: capability files.line-sequential, LOG-F in prfiles (20-byte record)".into(),
        caps == ["files.line-sequential"] && files.len() == 1 && files[0].name == "LOG-F" && files[0].record_len == 20,
    );
    let browser = compile_program(&common::parse(&disk), &Options::default()).err().map(|e| e.message).unwrap_or_default();
    check("the same file in a browser module keeps the browser's refusal".into(), !browser.is_empty() && !browser.contains("WebAssembly path"));

    // A LINAGE file and SAME RECORD AREA compile (T39b); ADVANCING a mnemonic-name on a LINAGE
    // file, which the interpreter reads as a data item, is refused at its WRITE.
    let linage = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT RPT ASSIGN TO \"t38.rpt\".\n    SELECT OTHER-RPT ASSIGN TO \"t38.oth\".\n\
         I-O-CONTROL.\n    SAME RECORD AREA FOR RPT OTHER-RPT.\n\
         DATA DIVISION.\nFILE SECTION.\nFD RPT\n    LINAGE IS 10 LINES.\n01 RPT-REC PIC X(20).\nFD OTHER-RPT.\n01 OTHER-REC PIC X(30).\n\
         PROCEDURE DIVISION.\n    OPEN OUTPUT RPT\n    WRITE RPT-REC\n    DISPLAY LINAGE-COUNTER\n    CLOSE RPT\n    OPEN INPUT OTHER-RPT\n    READ OTHER-RPT\n    CLOSE OTHER-RPT\n    STOP RUN.\n"
    );
    let linage_caps = compile_program(&common::parse(&linage), &native())
        .ok()
        .and_then(|m| cobolt_wasm_abi::scan::scan_module(&m.wasm).ok()?.embedded_manifest().ok())
        .map(|m| m.capabilities);
    check(
        "a LINAGE file with LINAGE-COUNTER and a SAME RECORD AREA peer compiles: capability files.sequential".into(),
        linage_caps.is_some_and(|c| c == ["files.sequential"]),
    );
    let mnemonic = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\nSPECIAL-NAMES.\n    C01 IS NEXT-PAGE.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT RPT ASSIGN TO \"t38.rpt\".\n\
         DATA DIVISION.\nFILE SECTION.\nFD RPT\n    LINAGE IS 10 LINES.\n01 RPT-REC PIC X(20).\nPROCEDURE DIVISION.\n    OPEN OUTPUT RPT\n    WRITE RPT-REC AFTER ADVANCING NEXT-PAGE\n    CLOSE RPT\n    STOP RUN.\n"
    );
    let e = refusal(&mnemonic);
    check(
        format!("ADVANCING a mnemonic-name on a LINAGE file: \"WRITE ADVANCING\" at line {}, the Cargo path, unit T16G", line_of(&mnemonic, "WRITE RPT-REC")),
        e.subject == "WRITE ADVANCING" && e.line == line_of(&mnemonic, "WRITE RPT-REC") && e.message.contains("Cargo path") && e.program == "T16G",
    );
    let written = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT RPT ASSIGN TO \"t38.rpt\".\n\
         DATA DIVISION.\nFILE SECTION.\nFD RPT\n    LINAGE IS 10 LINES.\n01 RPT-REC PIC X(20).\nPROCEDURE DIVISION.\n    OPEN OUTPUT RPT\n    MOVE 3 TO LINAGE-COUNTER\n    CLOSE RPT\n    STOP RUN.\n"
    );
    let e = refusal(&written);
    check(
        format!("LINAGE-COUNTER as a receiving item: \"LINAGE-COUNTER\" at line {}, the Cargo path, unit T16G", line_of(&written, "MOVE 3")),
        e.subject == "LINAGE-COUNTER" && e.line == line_of(&written, "MOVE 3") && e.message.contains("Cargo path") && e.program == "T16G",
    );

    // SORT and MERGE with USING and GIVING: compiled (T39); the files reach the runner. A relative
    // or indexed USING file, which the interpreter does not read, and COLLATING SEQUENCE are refused.
    let using = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n    SELECT IN-F ASSIGN TO \"in\".\n    SELECT OUT-F ASSIGN TO \"out\".\n\
         DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nFD IN-F.\n01 IN-REC PIC X(5).\nFD OUT-F.\n01 OUT-REC PIC X(5).\nPROCEDURE DIVISION.\n    SORT W ON ASCENDING KEY W-REC USING IN-F GIVING OUT-F\n    MERGE W ON ASCENDING KEY W-REC USING IN-F OUT-F GIVING OUT-F\n    STOP RUN.\n"
    );
    let caps_of = |src: &str| -> Option<Vec<String>> {
        let m = compile_program(&common::parse(src), &native()).ok()?;
        Some(cobolt_wasm_abi::scan::scan_module(&m.wasm).ok()?.embedded_manifest().ok()?.capabilities)
    };
    check(
        "SORT … USING/GIVING and MERGE compile: capability files.sequential".into(),
        caps_of(&using).is_some_and(|c| c == ["files.sequential"]),
    );
    let indexed_using = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n    SELECT IX ASSIGN TO \"ix\"\n        ORGANIZATION IS INDEXED\n        RECORD KEY IS IX-KEY.\n    SELECT OUT-F ASSIGN TO \"out\".\n\
         DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nFD IX.\n01 IX-REC.\n    05 IX-KEY PIC X(5).\nFD OUT-F.\n01 OUT-REC PIC X(5).\nPROCEDURE DIVISION.\n    SORT W ON ASCENDING KEY W-REC USING IX GIVING OUT-F.\n    STOP RUN.\n"
    );
    let e = refusal(&indexed_using);
    check(
        "SORT … USING an indexed file: refused at its line, naming the file (M3)".into(),
        e.subject == "SORT … USING/GIVING" && e.line == line_of(&indexed_using, "SORT W") && e.message.contains("USING IX") && e.message.contains("M3"),
    );
    let collating = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\nSPECIAL-NAMES.\n    ALPHABET REV IS \"Z\" THRU \"A\".\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n    SELECT IN-F ASSIGN TO \"in\".\n\
         DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nFD IN-F.\n01 IN-REC PIC X(5).\nPROCEDURE DIVISION.\n    SORT W ON ASCENDING KEY W-REC COLLATING SEQUENCE IS REV USING IN-F GIVING IN-F.\n    STOP RUN.\n"
    );
    let e = refusal(&collating);
    check("SORT … COLLATING SEQUENCE: refused at its line".into(), e.line == line_of(&collating, "SORT W") && e.message.contains("COLLATING SEQUENCE"));
    // COMMIT, ROLLBACK and UNLOCK reach an INDEXED file's engine through the runner (A35).
    let tx = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT IX ASSIGN TO \"ix\"\n        ORGANIZATION IS INDEXED\n        RECORD KEY IS IX-KEY.\n\
         DATA DIVISION.\nFILE SECTION.\nFD IX.\n01 IX-REC.\n    05 IX-KEY PIC X(5).\nPROCEDURE DIVISION.\n    OPEN I-O IX\n    COMMIT\n    ROLLBACK\n    CLOSE IX.\n    UNLOCK IX\n    STOP RUN.\n"
    );
    let tx_ok = compile_program(&common::parse(&tx), &native())
        .ok()
        .and_then(|m| cobolt_wasm_abi::scan::scan_module(&m.wasm).ok())
        .is_some_and(|s| s.imports.iter().any(|i| i.name == "f_tx") && s.embedded_manifest().is_ok_and(|m| m.capabilities == ["files.indexed"]));
    check("COMMIT, ROLLBACK and UNLOCK compile: import f_tx, capability files.indexed".into(), tx_ok);

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

    // The form and the application object in a program with no form: refused at the member, with
    // the reason that a form's program is served them (T33).
    let me = format!("{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-T PIC X(20).\nPROCEDURE DIVISION.\n    MOVE ME::Title TO WS-T\n    STOP RUN.\n");
    let e = refusal(&me);
    check(
        "ME::Title read with no form: refused as ME::TITLE at its line, no form (M2)".into(),
        e.subject == "ME::TITLE" && e.line == line_of(&me, "ME::Title") && e.message.contains("a program with no form has no controls or application object") && e.message.contains("M2"),
    );
    let set = format!("{HEAD}PROCEDURE DIVISION.\n    MOVE \"x\" TO ME::Title\n    STOP RUN.\n");
    let e = refusal(&set);
    check("ME::Title written with no form: refused as ME::TITLE at its line, no form (M2)".into(), e.subject == "ME::TITLE" && e.line == line_of(&set, "ME::Title") && e.message.contains("M2"));
    let invoke = format!("{HEAD}PROCEDURE DIVISION.\n    INVOKE ME \"Close\"\n    STOP RUN.\n");
    let e = refusal(&invoke);
    check("INVOKE with no form: refused at its line, no form (M2)".into(), e.subject == "INVOKE" && e.line == line_of(&invoke, "INVOKE") && e.message.contains("M2"));

    // A nested program's refusal names that program.
    let nested = format!(
        "{HEAD}PROCEDURE DIVISION.\n    CALL \"INNER\"\n    STOP RUN.\n\
         IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-T PIC X(20).\nPROCEDURE DIVISION.\n    MOVE ME::Title TO WS-T\n    GOBACK.\nEND PROGRAM INNER.\nEND PROGRAM T16G.\n"
    );
    let e = refusal(&nested);
    check("a refusal in the nested program INNER names INNER".into(), e.program == "INNER" && e.line == line_of(&nested, "ME::Title"));

    // A file dialog waits while the operator chooses, which a desktop module does in a form's
    // handler only (T32): outside one it is refused at its line, naming the built-in.
    let dialog = format!(
        "{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-PATH PIC X(80).\nPROCEDURE DIVISION.\n    COBOL::\"OPEN-FILE-DIALOG\" ( \"Open\" WS-PATH )\n    STOP RUN.\n"
    );
    let e = refusal(&dialog);
    check(
        "COBOL::\"OPEN-FILE-DIALOG\" outside a form's handler: refused at its line, naming it".into(),
        e.subject == "COBOL::\"OPEN-FILE-DIALOG\"" && e.line == line_of(&dialog, "OPEN-FILE-DIALOG") && e.message.contains("form's handler"),
    );
    let browser = compile_program(&common::parse(&dialog), &Options::default()).err().map(|e| e.message).unwrap_or_default();
    check("the same dialog in a browser module keeps the browser's refusal".into(), browser.contains("not available in a browser module"));

    // In a form's handler the dialog compiles; the page's own verbs do not, since a desktop form has no
    // message box (T6) and its windows are `ME`'s: each is refused at its line, naming it (T32).
    let handler = |body: &str| {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. MAIN-FORM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
             01 COBOL-QUIT PIC 9 VALUE 0.\n01 COBOL-EVENT-ID PIC X(64) VALUE SPACES.\n\
             01 COBOL-CONTROL-ID PIC X(64) VALUE SPACES.\n01 FORM-NAME PIC X(64) VALUE 'MAIN-FORM'.\n\
             PROCEDURE DIVISION.\nCOBOL-MAIN.\n    COBOL::\"INIT-FORM\" ( FORM-NAME )\n    PERFORM COBOL-EVENT-LOOP\n    STOP RUN.\n\
             COBOL-EVENT-LOOP.\n    PERFORM UNTIL COBOL-QUIT = 1\n        COBOL::\"WAIT-EVENT\" ( COBOL-EVENT-ID COBOL-CONTROL-ID )\n\
             \x20       EVALUATE COBOL-CONTROL-ID\n            WHEN \"BTN-1\"\n                CALL \"BTN-1--ONCLICK\"\n        END-EVALUATE\n    END-PERFORM.\n\
             IDENTIFICATION DIVISION.\nPROGRAM-ID. BTN-1--ONCLICK IS COMMON PROGRAM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-PATH PIC X(80).\n\
             PROCEDURE DIVISION.\n{body}    GOBACK.\nEND PROGRAM BTN-1--ONCLICK.\nEND PROGRAM MAIN-FORM.\n"
        )
    };
    let in_form = |src: &str| {
        let program = common::parse(src);
        let spec = FormSpec {
            name: "MAIN-FORM".into(),
            main: true,
            instances: 1,
            program: &program,
            controls: vec![("BTN-1".into(), "Button".into())],
            bindings: vec![("BTN-1".into(), "onClick".into())],
            layers: vec![],
            lines: vec![],
        };
        compile_project(&[spec], &native())
    };
    let opened = handler("    COBOL::\"OPEN-FILE-DIALOG\" ( \"Open\" WS-PATH )\n");
    check("COBOL::\"OPEN-FILE-DIALOG\" in a form's handler compiles into a desktop module".into(), in_form(&opened).is_ok());
    let page_verbs: Vec<&str> = APP_VERBS.iter().copied().filter(|v| !MOBILE_VERBS.contains(v)).collect();
    let mut refused = 0;
    for verb in &page_verbs {
        let src = handler(&format!("    COBOL::\"{verb}\" ( \"x\" )\n"));
        let e = in_form(&src).err().unwrap_or_else(|| GenError::new(0, "compiled"));
        // The verb in quotes: `WAIT` alone is also in the loop's `"WAIT-EVENT"`.
        if e.subject == format!("COBOL::\"{verb}\"") && e.line == line_of(&src, &format!("\"{verb}\"")) && e.message.contains("web page's application object") {
            refused += 1;
        }
    }
    check(format!("the page's {} own verbs (COBOL::\"MESSAGEBOX\" …) in a form's handler: {refused} refused at their line, naming each", page_verbs.len()), refused == page_verbs.len());

    // What the runner serves still compiles.
    let plain = format!("{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-N PIC 9(4) VALUE 1.\nPROCEDURE DIVISION.\n    ADD 41 TO WS-N\n    DISPLAY \"N=\" WS-N\n    STOP RUN.\n");
    check("a console program compiles into a desktop module".into(), compile_program(&common::parse(&plain), &native()).is_ok());

    let took = t.elapsed().as_secs_f64() * 1000.0;
    println!("== native_desktop_refusals (DESKTOP-WASM-001 T16, T32, R13) ==");
    println!("programs compiled: {} (11 as desktop modules refused or accepted, {} desktop form modules, 2 browser modules, 1 plain console program) in {took:.1} ms", 14 + 1 + page_verbs.len(), 1 + page_verbs.len());
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    assert!(failed.is_empty(), "failed: {failed:?}");
}
