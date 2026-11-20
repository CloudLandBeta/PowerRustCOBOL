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
    // or indexed USING or GIVING file compiles too (T39c), and COLLATING SEQUENCE compiles.
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
    check(
        "SORT … USING an indexed file compiles: capabilities files.indexed and files.sequential".into(),
        caps_of(&indexed_using).is_some_and(|c| c.len() == 2 && c.iter().any(|x| x == "files.indexed") && c.iter().any(|x| x == "files.sequential")),
    );
    let relative_giving = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n    SELECT IN-F ASSIGN TO \"in\".\n    SELECT RL ASSIGN TO \"rl\"\n        ORGANIZATION IS RELATIVE\n        RELATIVE KEY IS WS-RK.\n\
         DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nFD IN-F.\n01 IN-REC PIC X(5).\nFD RL.\n01 RL-REC PIC X(5).\nWORKING-STORAGE SECTION.\n01 WS-RK PIC 9(3).\nPROCEDURE DIVISION.\n    SORT W ON ASCENDING KEY W-REC USING IN-F GIVING RL.\n    STOP RUN.\n"
    );
    check(
        "SORT … GIVING a relative file compiles: capabilities files.relative and files.sequential".into(),
        caps_of(&relative_giving).is_some_and(|c| c.len() == 2 && c.iter().any(|x| x == "files.relative") && c.iter().any(|x| x == "files.sequential")),
    );
    let collating = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\nSPECIAL-NAMES.\n    ALPHABET REV IS \"Z\" THRU \"A\".\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n    SELECT IN-F ASSIGN TO \"in\".\n\
         DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nFD IN-F.\n01 IN-REC PIC X(5).\nPROCEDURE DIVISION.\n    SORT W ON ASCENDING KEY W-REC COLLATING SEQUENCE IS REV USING IN-F GIVING IN-F.\n    STOP RUN.\n"
    );
    check(
        "SORT … COLLATING SEQUENCE IS alphabet compiles: capability files.sequential".into(),
        caps_of(&collating).is_some_and(|c| c == ["files.sequential"]),
    );
    // OPEN … WITH REGISTERED USER compiles as the plain OPEN (T39c): the user goes only into an
    // INDEXED file's log, which a built program does not keep.
    let registered = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT IX ASSIGN TO \"ix\"\n        ORGANIZATION IS INDEXED\n        RECORD KEY IS IX-KEY.\n\
         DATA DIVISION.\nFILE SECTION.\nFD IX.\n01 IX-REC.\n    05 IX-KEY PIC X(5).\nWORKING-STORAGE SECTION.\n01 WS-USER PIC X(8) VALUE \"ADA\".\n\
         PROCEDURE DIVISION.\n    OPEN I-O IX WITH REGISTERED USER WS-USER\n    CLOSE IX\n    OPEN INPUT IX WITH REGISTERED USER \"BRUNO\"\n    CLOSE IX\n    STOP RUN.\n"
    );
    check(
        "OPEN … WITH REGISTERED USER compiles: capability files.indexed".into(),
        caps_of(&registered).is_some_and(|c| c == ["files.indexed"]),
    );
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
    // COBOL::"FILE-STATUS" reads in the module what the file verbs left there (T39c): it needs no
    // capability of its own. The file is named by a literal, as the IndexedFile control's paragraphs
    // name it, or by an alphanumeric item read at run time (T45c); a name in a numeric item and a
    // missing receiver are refused at the call.
    let status_none = format!(
        "{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-ST PIC XX.\nPROCEDURE DIVISION.\n    COBOL::\"FILE-STATUS\" ( \"NONE\" WS-ST )\n    DISPLAY WS-ST\n    STOP RUN.\n"
    );
    check(
        "COBOL::\"FILE-STATUS\" on a name that is no file compiles, with no capability".into(),
        caps_of(&status_none).is_some_and(|c| c.is_empty()),
    );
    let status_item = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT IX ASSIGN TO \"ix\"\n        ORGANIZATION IS INDEXED\n        RECORD KEY IS IX-KEY.\n\
         DATA DIVISION.\nFILE SECTION.\nFD IX.\n01 IX-REC.\n    05 IX-KEY PIC X(5).\nWORKING-STORAGE SECTION.\n01 WS-NAME PIC X(8) VALUE \"IX\".\n01 WS-ST PIC XX.\n\
         PROCEDURE DIVISION.\n    OPEN INPUT IX\n    COBOL::\"FILE-STATUS\" ( WS-NAME WS-ST )\n    STOP RUN.\n"
    );
    check(
        "COBOL::\"FILE-STATUS\" with the file's name in an alphanumeric item compiles: capability files.indexed".into(),
        caps_of(&status_item).is_some_and(|c| c == ["files.indexed"]),
    );
    let status_number = status_item.replace("01 WS-NAME PIC X(8) VALUE \"IX\".", "01 WS-NAME PIC 9(8) VALUE 1.");
    let e = refusal(&status_number);
    check(
        "COBOL::\"FILE-STATUS\" with the file's name in a numeric item: refused at its line, naming it".into(),
        e.subject == "COBOL::\"FILE-STATUS\"" && e.line == line_of(&status_number, "FILE-STATUS") && e.message.contains("alphanumeric"),
    );
    let status_one = format!("{HEAD}PROCEDURE DIVISION.\n    COBOL::\"FILE-STATUS\" ( \"IX\" )\n    STOP RUN.\n");
    let e = refusal(&status_one);
    check(
        "COBOL::\"FILE-STATUS\" with no receiver: refused at its line".into(),
        e.subject == "COBOL::\"FILE-STATUS\"" && e.line == line_of(&status_one, "FILE-STATUS") && e.message.contains("data item that receives"),
    );
    let status_sort = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT W ASSIGN TO \"w\".\n    SELECT IN-F ASSIGN TO \"in\".\n\
         DATA DIVISION.\nFILE SECTION.\nSD W.\n01 W-REC PIC X(5).\nFD IN-F.\n01 IN-REC PIC X(5).\nWORKING-STORAGE SECTION.\n01 WS-ST PIC XX.\n\
         PROCEDURE DIVISION.\n    SORT W ON ASCENDING KEY W-REC USING IN-F GIVING IN-F\n    COBOL::\"FILE-STATUS\" ( \"W\" WS-ST )\n    STOP RUN.\n"
    );
    // A SORT work file answers 00, as under the interpreter (SORT, RELEASE and RETURN set no status
    // under its name); a file only a nested program declares is read where its verbs left it.
    check(
        "COBOL::\"FILE-STATUS\" on a SORT work file compiles: capability files.sequential".into(),
        caps_of(&status_sort).is_some_and(|c| c == ["files.sequential"]),
    );
    let nested_file = |name: &str, n: usize| {
        let inner: String = (1..=n)
            .map(|i| {
                format!(
                    "IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER{i}.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT {name} ASSIGN TO \"n{i}\".\n\
                     DATA DIVISION.\nFILE SECTION.\nFD {name}.\n01 N{i}-REC PIC X(4).\nPROCEDURE DIVISION.\n    OPEN INPUT {name}\n    GOBACK.\nEND PROGRAM INNER{i}.\n"
                )
            })
            .collect();
        format!(
            "{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-ST PIC XX.\nPROCEDURE DIVISION.\n    CALL \"INNER1\"\n    COBOL::\"FILE-STATUS\" ( \"{name}\" WS-ST )\n    STOP RUN.\n{inner}END PROGRAM T16G.\n"
        )
    };
    check(
        "COBOL::\"FILE-STATUS\" on a file only a nested program declares compiles".into(),
        caps_of(&nested_file("INNERF", 1)).is_some_and(|c| c == ["files.sequential"]),
    );
    // Two programs that declare one name share one file, as under the interpreter: the status read is
    // the last verb's on either (the runner twin `file status` runs it).
    check(
        "COBOL::\"FILE-STATUS\" on a name two other programs declare compiles".into(),
        caps_of(&nested_file("SHARED", 2)).is_some_and(|c| c == ["files.sequential"]),
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

    // A built-in the runner does not serve yet: refused at its line, with the reason of its Desktop class.
    let models = format!(
        "{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-LIST PIC X(200).\n01 WS-N PIC 9(3).\n\
         PROCEDURE DIVISION.\n    COBOL::\"MODEL-LIST\" ( WS-LIST WS-N )\n    STOP RUN.\n"
    );
    let e = refusal(&models);
    check(
        "COBOL::\"MODEL-LIST\": refused at its line, the model lists (M6)".into(),
        e.subject == "COBOL::\"MODEL-LIST\""
            && e.line == line_of(&models, "MODEL-LIST")
            && e.message.contains("the model lists are not on the WebAssembly path yet")
            && e.message.contains("M6"),
    );

    // The HTTP built-ins are the runner's (T42, A45): the module asks for `http` and imports `http_call`.
    // One given fewer arguments than it takes is refused at its line, naming what it takes.
    let http = format!(
        "{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-URL PIC X(40) VALUE \"https://example.invalid/\".\n01 WS-BODY PIC X(80).\n01 WS-ST PIC 9(3).\n\
         PROCEDURE DIVISION.\n    COBOL::\"HTTP-GET\" ( WS-URL WS-BODY WS-ST )\n    STOP RUN.\n"
    );
    let http_scan = compile_program(&common::parse(&http), &native()).ok().and_then(|m| cobolt_wasm_abi::scan::scan_module(&m.wasm).ok());
    check(
        "COBOL::\"HTTP-GET\" compiles: capability http, import http_call".into(),
        http_scan.is_some_and(|s| {
            s.imports.iter().any(|i| i.name == "http_call") && s.embedded_manifest().is_ok_and(|m| m.capabilities == ["http"])
        }),
    );
    let short = format!(
        "{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-BODY PIC X(80).\n\
         PROCEDURE DIVISION.\n    COBOL::\"HTTP-GET\" ( \"https://example.invalid/\" WS-BODY )\n    STOP RUN.\n"
    );
    let e = refusal(&short);
    check(
        "COBOL::\"HTTP-GET\" with two arguments: refused at its line, naming what it takes".into(),
        e.subject == "COBOL::\"HTTP-GET\""
            && e.line == line_of(&short, "HTTP-GET")
            && e.message.contains("takes the URL and the data items that receive the response and the status code"),
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
            // The IndexedFile control is served (T39c); only its PartiQL members are refused.
            controls: vec![("BTN-1".into(), "Button".into()), ("IXF-1".into(), "IndexedFile".into()), ("AGT-1".into(), "AgentObject".into())],
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

    // The IndexedFile control (T39c): a form holding one compiles, and so do the members every control
    // has; a PartiQL member (a method, a property a statement answers in, the binding query) is
    // refused at its line, naming it, as T48 waits for them.
    let plain_members = handler("    MOVE IXF-1::OpenMode TO WS-PATH\n    MOVE IXF-1::IsBusy() TO WS-PATH\n    MOVE \"I-O\" TO IXF-1::OpenMode\n");
    check("a form with an IndexedFile control compiles, reading and writing its OpenMode and asking IsBusy()".into(), in_form(&plain_members).is_ok());
    let mut partiql = 0;
    let partiql_cases = [
        ("    MOVE IXF-1::Execute( \"SELECT A FROM T\" ) TO WS-PATH\n", "IXF-1::EXECUTE"),
        ("    MOVE IXF-1::Fetch() TO WS-PATH\n", "IXF-1::FETCH"),
        ("    MOVE IXF-1::StatusCode TO WS-PATH\n", "IXF-1::STATUSCODE"),
        ("    MOVE \"SELECT A FROM T\" TO IXF-1::BindingQuery\n", "IXF-1::BINDINGQUERY"),
    ];
    for (body, subject) in partiql_cases {
        let src = handler(body);
        let e = in_form(&src).err().unwrap_or_else(|| GenError::new(0, "compiled"));
        if e.subject == subject && e.line == line_of(&src, "IXF-1::") && e.message.contains("PartiQL members") && e.message.contains("M6") {
            partiql += 1;
        }
    }
    check(
        format!("the IndexedFile control's PartiQL members (Execute, Fetch, StatusCode read, BindingQuery written): {partiql} of {} refused at their line, naming each", partiql_cases.len()),
        partiql == partiql_cases.len(),
    );

    // The AgentObject (T47c): its Ask and declared tools compile; a member that names one of the program's
    // data items is refused at its line, naming it.
    let asked = handler(
        "    MOVE AGT-1::AddTool( \"get_rate\" \"A rate\" \"currency:string\" ) TO WS-PATH\n    MOVE AGT-1::Ask( \"What is the rate?\" ) TO WS-PATH\n    MOVE AGT-1::SetToolResult( \"call_1\" \"1.08\" ) TO WS-PATH\n    MOVE AGT-1::LastReply TO WS-PATH\n",
    );
    check("a form with an AgentObject compiles, with AddTool, Ask, SetToolResult and LastReply".into(), in_form(&asked).is_ok());
    let mut agent = 0;
    let agent_cases = [
        ("    MOVE \"WS-PATH\" TO AGT-1::ResponseDataItem\n", "ResponseDataItem"),
        ("    MOVE AGT-1::SetProperty( \"LastReply\" \"x\" ) TO WS-PATH\n", "ResponseDataItem"),
        ("    MOVE AGT-1::AllowFile( WS-PATH \"orders.cidx\" ) TO WS-PATH\n", "names its file in a data item"),
    ];
    for (body, why) in agent_cases {
        let src = handler(body);
        let e = in_form(&src).err().unwrap_or_else(|| GenError::new(0, "compiled"));
        if e.subject.to_ascii_uppercase().starts_with("AGT-1::") && e.line == line_of(&src, "AGT-1::") && e.message.contains(why) && e.message.contains("M6") {
            agent += 1;
        } else {
            println!("    AgentObject case {body:?}: {} at line {}: {}", e.subject, e.line, e.message);
        }
    }
    check(
        format!("the AgentObject's members naming a data item (ResponseDataItem, SetProperty, AllowFile's file name): {agent} of {} refused at their line, naming each", agent_cases.len()),
        agent == agent_cases.len(),
    );

    // Its file tools (T47c, A51): an AllowFile naming an INDEXED file of the form by a literal, with or
    // without its definition, DenyFile, RegisterFile and UnregisterFile compile, and the module requires
    // `agent.files`, which a module without them does not; an AllowFile naming a file that is not
    // INDEXED is refused at its line, naming it.
    let caps = |r: &Result<cobolt_wasm::Compiled, GenError>| -> Vec<String> {
        r.as_ref()
            .ok()
            .and_then(|c| cobolt_wasm_abi::scan::scan_module(&c.wasm).ok())
            .and_then(|scan| scan.embedded_manifest().ok())
            .map(|m| m.capabilities)
            .unwrap_or_default()
    };
    let with_files = |body: &str| {
        handler(body).replacen(
            "PROGRAM-ID. MAIN-FORM.\nDATA DIVISION.\n",
            "PROGRAM-ID. MAIN-FORM.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
             \x20   SELECT ORDERS ASSIGN TO \"orders.dat\" ORGANIZATION IS INDEXED ACCESS IS DYNAMIC RECORD KEY IS ORD-ID.\n\
             \x20   SELECT NOTES ASSIGN TO \"notes.txt\" ORGANIZATION IS LINE SEQUENTIAL.\n\
             DATA DIVISION.\nFILE SECTION.\nFD ORDERS.\n01 ORD-REC.\n   05 ORD-ID PIC 9(6).\n   05 ORD-CUSTOMER PIC X(30).\n\
             FD NOTES.\n01 NOTE-LINE PIC X(80).\n",
            1,
        )
    };
    let tools = with_files(
        "    MOVE AGT-1::AllowFile( \"ORDERS\" \"orders.cidx\" ) TO WS-PATH\n    MOVE AGT-1::AllowFile( \"ORDERS\" ) TO WS-PATH\n\
         \x20   MOVE AGT-1::RegisterFile( \"data/x.dat\" \"x.cidx\" \"X\" ) TO WS-PATH\n    MOVE AGT-1::UnregisterFile( \"X\" ) TO WS-PATH\n\
         \x20   AGT-1::DenyFile( \"ORDERS\" )\n",
    );
    let compiled = in_form(&tools);
    if let Err(e) = &compiled {
        println!("    file tools: {} at line {}: {}", e.subject, e.line, e.message);
    }
    let needs = caps(&compiled);
    check(
        format!("a form whose AgentObject uses AllowFile (an INDEXED file, with and without its definition), RegisterFile, UnregisterFile and DenyFile compiles, requiring {needs:?}"),
        compiled.is_ok() && needs.iter().any(|c| c == "agent") && needs.iter().any(|c| c == "agent.files"),
    );
    let plain_agent = caps(&in_form(&asked));
    check(
        format!("a form whose AgentObject uses none of them requires {plain_agent:?}, not agent.files or agent.kb"),
        plain_agent.iter().any(|c| c == "agent") && !plain_agent.iter().any(|c| c == "agent.files" || c == "agent.kb"),
    );
    // Its Knowledge Base tools (T47c, A52): AllowKnowledgeBase, with and without a collection and with the
    // KnowledgeBase named in a data item, and DenyKnowledgeBase compile, and the module requires `agent.kb`.
    let kb = in_form(&handler(
        "    MOVE AGT-1::AllowKnowledgeBase( \"KB-1\" \"docs\" ) TO WS-PATH\n    MOVE AGT-1::AllowKnowledgeBase( \"KB-1\" ) TO WS-PATH\n\
         \x20   MOVE AGT-1::AllowKnowledgeBase( WS-PATH ) TO WS-PATH\n    AGT-1::DenyKnowledgeBase( \"KB-1\" )\n",
    ));
    if let Err(e) = &kb {
        println!("    Knowledge Base tools: {} at line {}: {}", e.subject, e.line, e.message);
    }
    let kb_needs = caps(&kb);
    check(
        format!("a form whose AgentObject uses AllowKnowledgeBase (a literal, no collection, a data item) and DenyKnowledgeBase compiles, requiring {kb_needs:?}"),
        kb.is_ok() && kb_needs.iter().any(|c| c == "agent") && kb_needs.iter().any(|c| c == "agent.kb") && !kb_needs.iter().any(|c| c == "agent.files"),
    );
    let notes = with_files("    MOVE AGT-1::AllowFile( \"NOTES\" ) TO WS-PATH\n");
    let e = in_form(&notes).err().unwrap_or_else(|| GenError::new(0, "compiled"));
    check(
        format!("AllowFile naming a LINE SEQUENTIAL file: refused at its line, naming it ({} at line {})", e.subject, e.line),
        e.subject.to_ascii_uppercase() == "AGT-1::ALLOWFILE" && e.line == line_of(&notes, "AGT-1::") && e.message.contains("not INDEXED") && e.message.contains("M6"),
    );

    // What the runner serves still compiles.
    let plain = format!("{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-N PIC 9(4) VALUE 1.\nPROCEDURE DIVISION.\n    ADD 41 TO WS-N\n    DISPLAY \"N=\" WS-N\n    STOP RUN.\n");
    check("a console program compiles into a desktop module".into(), compile_program(&common::parse(&plain), &native()).is_ok());

    let took = t.elapsed().as_secs_f64() * 1000.0;
    println!("== native_desktop_refusals (DESKTOP-WASM-001 T16, T32, R13) ==");
    println!("programs compiled: {} (23 as desktop modules refused or accepted, {} desktop form modules, 2 browser modules, 1 plain console program) in {took:.1} ms", 26 + 14 + page_verbs.len(), 14 + page_verbs.len());
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    assert!(failed.is_empty(), "failed: {failed:?}");
}

/// A national or UTF-8 item is bytes in a module (WASM-PARITY 17), so a desktop module would display,
/// move and compare it otherwise than the interpreter: it is refused at its declaration, naming it
/// and its program, and the Build takes the Cargo path until T44. A browser module is unchanged.
#[test]
fn national_and_utf8_data_compute_in_the_module_or_take_the_cargo_path() {
    let t = Instant::now();
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |what: String, ok: bool| checks.push((what, ok));
    // Moved, shown, compared, initialised and measured: computed in the module (T44).
    let program = |ws: &str, n: &str| {
        format!(
            "{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-X PIC X(8).\n01 WS-L PIC 9(4).\n{ws}PROCEDURE DIVISION.\n    MOVE \"Ação\" TO WS-X {n}\n    DISPLAY {n} WS-X\n    IF {n} = \"Ação\" OR {n} = SPACE DISPLAY \"same\" END-IF\n    INITIALIZE {n}\n    MOVE FUNCTION LENGTH({n}) TO WS-L\n    MOVE {n} TO WS-X\n    STOP RUN.\n"
        )
    };
    let kept = [
        ("01 WS-N PIC N(6).\n", "WS-N", "PIC N"),
        ("01 WS-U PIC U(3).\n", "WS-U", "PIC U"),
        ("01 WS-B PIC U BYTE-LENGTH 5.\n", "WS-B", "PIC U BYTE-LENGTH"),
        ("01 WS-G.\n   05 WS-GX PIC X(2).\n   05 WS-GN PIC N(2) USAGE NATIONAL.\n", "WS-GN", "a national field in a group"),
    ];
    for (ws, n, what) in kept {
        let src = program(ws, n);
        let r = compile_program(&common::parse(&src), &native());
        check(format!("{what}: MOVE, DISPLAY, IF, INITIALIZE and LENGTH of {n} compile into a desktop module"), r.is_ok());
    }
    // Any other use takes the Cargo path, naming the item and the statement's line.
    let ws = "01 WS-N PIC N(6).\n01 WS-9 PIC 9(4).\n01 WS-C PIC 9(4).\n";
    let uses = [
        ("    DISPLAY WS-N(1:2)\n", "WS-N(1:2)", "a reference modification"),
        ("    INSPECT WS-N TALLYING WS-C FOR ALL \"a\"\n", "INSPECT", "INSPECT"),
        ("    STRING WS-N DELIMITED BY SIZE INTO WS-X\n", "STRING", "STRING"),
        ("    MOVE 5 TO WS-N\n", "MOVE 5", "a number moved into it"),
        ("    MOVE WS-N TO WS-9\n", "TO WS-9", "it moved into a number"),
        ("    MOVE FUNCTION UPPER-CASE(WS-N) TO WS-X\n", "UPPER-CASE", "a function's argument"),
        ("    IF WS-N = ZERO DISPLAY \"z\" END-IF\n", "ZERO", "a comparison with ZERO"),
    ];
    for (stmt, needle, what) in uses {
        let src = format!("{HEAD}DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-X PIC X(8).\n{ws}PROCEDURE DIVISION.\n{stmt}    STOP RUN.\n");
        let line = line_of(&src, needle);
        let e = refusal(&src);
        check(
            format!("{what}: \"WS-N\" refused at line {line}, unit T16G, the Cargo path, M6"),
            e.subject == "WS-N" && e.line == line && e.program == "T16G" && e.message.contains("Cargo path") && e.message.contains("M6"),
        );
    }
    // A PROGRAM COLLATING SEQUENCE would weigh national data: refused at the item's declaration.
    let collating = format!(
        "{HEAD}ENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\nOBJECT-COMPUTER. X PROGRAM COLLATING SEQUENCE IS ALPHA.\nSPECIAL-NAMES. ALPHABET ALPHA IS STANDARD-1.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-N PIC N(6).\nPROCEDURE DIVISION.\n    DISPLAY WS-N\n    STOP RUN.\n"
    );
    let e = refusal(&collating);
    let decl = line_of(&collating, "01 WS-N");
    check(
        format!("a PROGRAM COLLATING SEQUENCE: \"WS-N\" refused at its declaration, line {decl}"),
        e.subject == "WS-N" && e.line == decl && e.message.contains("COLLATING SEQUENCE") && e.message.contains("M6"),
    );
    let page = program("01 WS-N PIC N(6).\n", "WS-X");
    check(
        "a browser module with a PIC N item still compiles (unchanged)".into(),
        compile_program(&common::parse(&page), &Options::default()).is_ok(),
    );
    let took = t.elapsed().as_secs_f64() * 1000.0;
    println!("== native_desktop_refusals: national and UTF-8 data (DESKTOP-WASM-001 T44) ==");
    println!(
        "programs compiled: {} ({} desktop modules accepted, {} refused, 1 browser module) in {took:.1} ms",
        kept.len() + uses.len() + 2,
        kept.len(),
        uses.len() + 1
    );
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    for (what, ok) in &checks {
        println!("  {} {what}", if *ok { "pass" } else { "FAIL" });
    }
    println!("result: {passed} of {} checks passed", checks.len());
    let failed: Vec<&String> = checks.iter().filter(|(_, ok)| !*ok).map(|(w, _)| w).collect();
    assert!(failed.is_empty(), "failed: {failed:?}");
}
