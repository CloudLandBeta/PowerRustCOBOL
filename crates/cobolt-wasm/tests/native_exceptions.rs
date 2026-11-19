// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T45 (R11, R12): `TRY … CATCH … FINALLY … END-TRY` and `THROW` computed in a
//! module as the interpreter computes them. Each case runs under the interpreter and as a desktop
//! module in both memory layouts, and once as a browser module: the lines `DISPLAY` writes must be
//! identical, and so must the end of the run (a `THROW` nobody catches is the run-time error that
//! carries the interpreter's own words).

mod common;

use cobolt_wasm::host::{run_native, NativeInput};
use cobolt_wasm::{compile_program, Options};
use std::time::{Duration, Instant};

fn native(multi: bool) -> Options {
    Options { native: true, multi_memory: multi, ..Options::default() }
}

/// A program of one or more paragraphs; `procedure` starts with its first paragraph's name.
fn program(items: &str, procedure: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T45E.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-ERR PIC X(30).\n01 WS-MSG PIC X(20) VALUE \"TOO LONG FOR TEN\".\n01 WS-I PIC 9(4) VALUE 0.\n{items}\
         PROCEDURE DIVISION.\n{procedure}"
    )
}

/// What the interpreter DISPLAYs, and the text of the error that ended the run, if one did.
fn interpreter(src: &str) -> (Vec<String>, Option<String>) {
    let prog = common::parse(src);
    let (_event_tx, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    let ended = match interp.run() {
        Ok(()) => None,
        Err(e) if e.is_exit_signal() => None,
        Err(e) => Some(e.to_string()),
    };
    let lines = display_rx.try_iter().map(|s: String| s.trim_end_matches(['\n', '\r']).to_owned()).collect();
    (lines, ended)
}

/// (name, the items after the common ones, the procedure)
const CASES: &[(&str, &str, &str)] = &[
    (
        "a THROW is caught with its text, and FINALLY runs after CATCH",
        "",
        "MAIN-PARA.\n    TRY\n        DISPLAY \"BODY\"\n        THROW \"BUSINESS RULE\"\n        DISPLAY \"NOT REACHED\"\n\
         CATCH EXCEPTION WS-ERR\n        DISPLAY \"CAUGHT [\" WS-ERR \"]\"\n    FINALLY\n        DISPLAY \"FINALLY\"\n    END-TRY\n\
         DISPLAY \"AFTER\"\n    STOP RUN.\n",
    ),
    (
        "with no THROW, CATCH is skipped and FINALLY runs",
        "",
        "MAIN-PARA.\n    TRY\n        DISPLAY \"BODY\"\n    CATCH WS-ERR\n        DISPLAY \"NOT REACHED\"\n    FINALLY\n        DISPLAY \"FINALLY\"\n    END-TRY\n\
         DISPLAY \"AFTER\"\n    STOP RUN.\n",
    ),
    (
        "a THROW in a paragraph PERFORMed two deep reaches the TRY, and later PERFORMs still return",
        "",
        "MAIN-PARA.\n    TRY\n        PERFORM P1\n        DISPLAY \"NOT REACHED\"\n    CATCH WS-ERR\n        DISPLAY \"CAUGHT \" WS-ERR\n    END-TRY\n\
         PERFORM P3\n    PERFORM P3\n    DISPLAY \"END\"\n    STOP RUN.\n\
         P1.\n    DISPLAY \"IN P1\"\n    PERFORM P2\n    DISPLAY \"P1 NOT REACHED\".\n\
         P2.\n    DISPLAY \"IN P2\"\n    THROW WS-MSG.\n\
         P3.\n    DISPLAY \"IN P3\".\n",
    ),
    (
        "a THROW in an inner CATCH reaches the outer TRY, and the inner FINALLY does not run",
        "",
        "MAIN-PARA.\n    TRY\n        TRY\n            THROW \"FIRST\"\n        CATCH WS-ERR\n            DISPLAY \"INNER \" WS-ERR\n            THROW \"SECOND\"\n\
         FINALLY\n            DISPLAY \"INNER FINALLY\"\n        END-TRY\n        DISPLAY \"NOT REACHED\"\n\
         CATCH WS-ERR\n        DISPLAY \"OUTER \" WS-ERR\n    FINALLY\n        DISPLAY \"OUTER FINALLY\"\n    END-TRY\n    STOP RUN.\n",
    ),
    (
        "the text is cut to the CATCH item and JUSTIFIED RIGHT keeps its right end",
        "01 WS-SHORT PIC X(10).\n01 WS-RIGHT PIC X(20) JUSTIFIED RIGHT.\n",
        "MAIN-PARA.\n    TRY\n        THROW WS-MSG\n    CATCH WS-SHORT\n        DISPLAY \"[\" WS-SHORT \"]\"\n    END-TRY\n\
         TRY\n        THROW \"ABC\"\n    CATCH WS-RIGHT\n        DISPLAY \"[\" WS-RIGHT \"]\"\n    END-TRY\n\
         TRY\n        THROW \"A MESSAGE LONGER THAN TWENTY\"\n    CATCH WS-RIGHT\n        DISPLAY \"[\" WS-RIGHT \"]\"\n    END-TRY\n    STOP RUN.\n",
    ),
    (
        "a group named by CATCH takes the text over its fields",
        "01 WS-GRP.\n   05 WS-G1 PIC X(4).\n   05 WS-G2 PIC X(6).\n",
        "MAIN-PARA.\n    MOVE ALL \"*\" TO WS-GRP\n    TRY\n        THROW \"ABCDEFGHIJKL\"\n    CATCH EXCEPTION WS-GRP\n        DISPLAY \"[\" WS-G1 \"][\" WS-G2 \"]\"\n    END-TRY\n\
         TRY\n        THROW \"AB\"\n    CATCH WS-GRP\n        DISPLAY \"[\" WS-G1 \"][\" WS-G2 \"]\"\n    END-TRY\n    STOP RUN.\n",
    ),
    (
        "a TRY with no CATCH clause still takes the exception, and FINALLY runs",
        "",
        "MAIN-PARA.\n    TRY\n        THROW \"SWALLOWED\"\n    FINALLY\n        DISPLAY \"FINALLY\"\n    END-TRY\n\
         TRY\n        THROW \"SWALLOWED TOO\"\n    END-TRY\n    DISPLAY \"AFTER\"\n    STOP RUN.\n",
    ),
    (
        "a CATCH with no item, and the message is not kept anywhere",
        "",
        "MAIN-PARA.\n    MOVE \"UNCHANGED\" TO WS-ERR\n    TRY\n        THROW \"LOST\"\n    CATCH EXCEPTION\n        DISPLAY \"CAUGHT \" WS-ERR\n    END-TRY\n    STOP RUN.\n",
    ),
    (
        "GO TO out of a TRY runs FINALLY before the paragraph it names",
        "",
        "MAIN-PARA.\n    TRY\n        DISPLAY \"BODY\"\n        GO TO P-OUT\n    FINALLY\n        DISPLAY \"FINALLY\"\n    END-TRY\n    DISPLAY \"NOT REACHED\".\n\
         P-OUT.\n    DISPLAY \"OUT\"\n    STOP RUN.\n",
    ),
    (
        "EXIT PERFORM and EXIT PERFORM CYCLE out of a TRY run FINALLY",
        "",
        "MAIN-PARA.\n    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 6\n        TRY\n            IF WS-I = 2\n                EXIT PERFORM CYCLE\n            END-IF\n\
         IF WS-I = 4\n                EXIT PERFORM\n            END-IF\n            DISPLAY \"BODY \" WS-I\n        FINALLY\n            DISPLAY \"FINALLY \" WS-I\n\
         END-TRY\n    END-PERFORM\n    DISPLAY \"LOOP DONE \" WS-I\n    STOP RUN.\n",
    ),
    (
        "EXIT PARAGRAPH out of a TRY runs FINALLY, and the PERFORM returns",
        "",
        "MAIN-PARA.\n    PERFORM P1\n    PERFORM P1\n    DISPLAY \"BACK\"\n    STOP RUN.\n\
         P1.\n    TRY\n        DISPLAY \"IN P1\"\n        EXIT PARAGRAPH\n    FINALLY\n        DISPLAY \"FINALLY\"\n    END-TRY\n    DISPLAY \"NOT REACHED\".\n",
    ),
    (
        "a TRY in a PERFORMed paragraph catches its own THROW each time",
        "",
        "MAIN-PARA.\n    PERFORM P1 3 TIMES\n    DISPLAY \"DONE\"\n    STOP RUN.\n\
         P1.\n    ADD 1 TO WS-I\n    TRY\n        PERFORM P2\n    CATCH WS-ERR\n        DISPLAY \"CAUGHT \" WS-ERR WS-I\n    END-TRY.\n\
         P2.\n    THROW \"IN P2 \".\n",
    ),
    (
        "a THROW in FINALLY reaches the outer CATCH",
        "",
        "MAIN-PARA.\n    TRY\n        TRY\n            DISPLAY \"INNER BODY\"\n        FINALLY\n            DISPLAY \"INNER FINALLY\"\n            THROW \"FROM FINALLY\"\n\
         END-TRY\n        DISPLAY \"NOT REACHED\"\n    CATCH WS-ERR\n        DISPLAY \"OUTER \" WS-ERR\n    END-TRY\n    STOP RUN.\n",
    ),
    (
        "a THROW in EVALUATE inside a loop is caught 1000 times and the stack stays level",
        "01 WS-K PIC 9 VALUE 0.\n01 WS-ODD PIC 9(4) VALUE 0.\n01 WS-EVEN PIC 9(4) VALUE 0.\n",
        "MAIN-PARA.\n    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 1000\n        TRY\n            ADD 1 TO WS-K\n            EVALUATE WS-K\n\
         WHEN 2\n                MOVE 0 TO WS-K\n                PERFORM P-THROW\n            WHEN OTHER\n                ADD 1 TO WS-ODD\n\
         END-EVALUATE\n        CATCH WS-ERR\n            ADD 1 TO WS-EVEN\n        FINALLY\n            PERFORM P-NOTHING\n        END-TRY\n\
         END-PERFORM\n    DISPLAY WS-ODD \" \" WS-EVEN\n    STOP RUN.\n\
         P-THROW.\n    THROW \"EVEN\".\n\
         P-NOTHING.\n    CONTINUE.\n",
    ),
    (
        "STOP RUN in a paragraph PERFORMed from a TRY runs FINALLY before the run ends",
        "",
        "MAIN-PARA.\n    TRY\n        PERFORM STOPPER\n        DISPLAY \"NOT REACHED\"\n    FINALLY\n        DISPLAY \"FINALLY BEFORE STOP\"\n    END-TRY\n\
         DISPLAY \"NOT REACHED EITHER\"\n    STOP RUN.\n\
         STOPPER.\n    DISPLAY \"STOPPING\"\n    STOP RUN.\n",
    ),
    (
        "GOBACK of the main program inside nested TRYs runs both FINALLYs, inner first",
        "",
        "MAIN-PARA.\n    TRY\n        TRY\n            DISPLAY \"BODY\"\n            GOBACK\n        FINALLY\n            DISPLAY \"INNER FINALLY\"\n        END-TRY\n\
         FINALLY\n        DISPLAY \"OUTER FINALLY\"\n    END-TRY\n    DISPLAY \"NOT REACHED\"\n    STOP RUN.\n",
    ),
];

/// Programs of more than one unit: (name, source).
const CALLS: &[(&str, &str)] = &[
    (
        "a THROW in a CALLed program is caught by the caller, twice",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-ERR PIC X(20).\n\
         PROCEDURE DIVISION.\nMAIN-PARA.\n    PERFORM 2 TIMES\n        TRY\n            CALL \"INNER\"\n            DISPLAY \"NOT REACHED\"\n\
         CATCH WS-ERR\n            DISPLAY \"CAUGHT \" WS-ERR\n        END-TRY\n    END-PERFORM\n    DISPLAY \"END\"\n    STOP RUN.\n\
         IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nPROCEDURE DIVISION.\nINNER-PARA.\n    DISPLAY \"IN INNER\"\n    PERFORM INNER-THROW\n    EXIT PROGRAM.\n\
         INNER-THROW.\n    THROW \"FROM INNER\".\nEND PROGRAM INNER.\nEND PROGRAM OUTER.\n",
    ),
    (
        "GOBACK in a CALLed program's TRY runs its FINALLY, then the caller goes on",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\nPROCEDURE DIVISION.\nMAIN-PARA.\n    CALL \"INNER\"\n    DISPLAY \"BACK 1\"\n    CALL \"INNER\"\n\
         DISPLAY \"BACK 2\"\n    STOP RUN.\n\
         IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nPROCEDURE DIVISION.\nINNER-PARA.\n    TRY\n        DISPLAY \"INNER BODY\"\n        GOBACK\n\
         FINALLY\n        DISPLAY \"INNER FINALLY\"\n    END-TRY\n    DISPLAY \"NOT REACHED\".\nEND PROGRAM INNER.\nEND PROGRAM OUTER.\n",
    ),
    (
        "GOBACK in a paragraph PERFORMed from a CALLed program's TRY runs its FINALLY",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\nPROCEDURE DIVISION.\nMAIN-PARA.\n    CALL \"INNER\"\n    DISPLAY \"BACK\"\n    STOP RUN.\n\
         IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nPROCEDURE DIVISION.\nINNER-PARA.\n    TRY\n        PERFORM LEAVE-P\n        DISPLAY \"NOT REACHED\"\n\
         FINALLY\n        DISPLAY \"INNER FINALLY\"\n    END-TRY\n    DISPLAY \"NOT REACHED 2\".\nLEAVE-P.\n    GOBACK.\nEND PROGRAM INNER.\nEND PROGRAM OUTER.\n",
    ),
    (
        "STOP RUN in a CALLed program runs the caller's FINALLY",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\nPROCEDURE DIVISION.\nMAIN-PARA.\n    TRY\n        CALL \"INNER\"\n        DISPLAY \"NOT REACHED\"\n\
         FINALLY\n        DISPLAY \"OUTER FINALLY\"\n    END-TRY\n    DISPLAY \"NOT REACHED 2\"\n    STOP RUN.\n\
         IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nPROCEDURE DIVISION.\nINNER-PARA.\n    DISPLAY \"INNER STOPS\"\n    STOP RUN.\n\
         END PROGRAM INNER.\nEND PROGRAM OUTER.\n",
    ),
    (
        "a THROW nobody catches ends the run with the interpreter's words",
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. UNCAUGHT.\nPROCEDURE DIVISION.\nMAIN-PARA.\n    DISPLAY \"BEFORE\"\n    PERFORM P1\n\
         DISPLAY \"NOT REACHED\"\n    STOP RUN.\nP1.\n    THROW \"NOBODY CATCHES THIS\".\n",
    ),
];

/// How a module's run ended, in the interpreter's terms: `None` when it ended normally, else the
/// text of the run-time error it stopped at.
fn module_end(o: &cobolt_wasm::host::Outcome) -> Result<Option<String>, String> {
    if let Some(t) = &o.trapped {
        if o.errors.is_empty() {
            return Err(format!("trapped with no error: {t}"));
        }
    }
    match (o.errors.as_slice(), o.error_texts.as_slice()) {
        ([], _) => Ok(None),
        ([_], [text]) => Ok(Some(text.clone())),
        _ => Err(format!("errors {:?} texts {:?}", o.errors, o.error_texts)),
    }
}

#[test]
fn try_catch_finally_and_throw_compute_as_the_interpreter_computes_them() {
    let (mut t_compile, mut t_module, mut t_interp) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let (mut lines, mut runs, mut ends) = (0usize, 0usize, 0usize);
    let mut results: Vec<(String, Result<(), String>)> = Vec::new();
    let sources: Vec<(String, String)> = CASES
        .iter()
        .map(|(n, items, procedure)| (n.to_string(), program(items, procedure)))
        .chain(CALLS.iter().map(|(n, s)| (n.to_string(), s.to_string())))
        .collect();
    for (name, src) in &sources {
        let t = Instant::now();
        let (want_lines, want_end) = interpreter(src);
        t_interp += t.elapsed();
        let mut outcome = Ok(());
        // Both desktop layouts, then a browser module (the Web column lists both statements as compiled).
        for (label, opts) in [("desktop multi", native(true)), ("desktop single", native(false)), ("browser", Options::default())] {
            let t = Instant::now();
            let compiled = compile_program(&common::parse(src), &opts);
            t_compile += t.elapsed();
            let c = match compiled {
                Ok(c) => c,
                Err(e) => {
                    outcome = Err(format!("{label}: not compiled: {e}"));
                    break;
                }
            };
            let t = Instant::now();
            let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
            t_module += t.elapsed();
            runs += 1;
            let got_lines = common::wasm_lines(&o);
            let got_end = module_end(&o);
            if got_lines != want_lines {
                outcome = Err(format!("{label}: DISPLAY differs\n  module      {got_lines:?}\n  interpreter {want_lines:?}"));
            } else if got_end.as_ref() != Ok(&want_end) {
                outcome = Err(format!("{label}: the run ends differently\n  module      {got_end:?}\n  interpreter {want_end:?}"));
            } else {
                lines += got_lines.len();
                ends += usize::from(want_end.is_some());
                continue;
            }
            break;
        }
        results.push((name.clone(), outcome));
    }

    let passed = results.iter().filter(|(_, r)| r.is_ok()).count();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("== native_exceptions: TRY, CATCH, FINALLY and THROW, module against interpreter (DESKTOP-WASM-001 T45) ==");
    println!("cases: {} (each as a desktop module in both memory layouts and as a browser module: {runs} module runs)", results.len());
    for (name, r) in &results {
        match r {
            Ok(()) => println!("  pass {name}"),
            Err(e) => println!("  FAIL {name}: {e}"),
        }
    }
    println!(
        "compared: {lines} DISPLAY lines and {ends} run-time errors, all identical where passed\n\
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

/// The uses a module does not compute are refused at their line, so a desktop Build takes the
/// Cargo path and names why.
#[test]
fn the_exception_uses_a_module_does_not_compute_are_refused_at_their_line() {
    // (what, the items, the procedure, the text of the line the refusal names)
    let refused: &[(&str, &str, &str, &str)] = &[
        ("a THROW of a number", "", "MAIN-PARA.\n    TRY\n        THROW 42\n    END-TRY\n    STOP RUN.\n", "THROW 42"),
        (
            "a numeric CATCH item",
            "01 WS-NUM PIC 9(4).\n",
            "MAIN-PARA.\n    TRY\n        THROW \"X\"\n    CATCH WS-NUM\n        CONTINUE\n    END-TRY\n    STOP RUN.\n",
            "    TRY",
        ),
        (
            "NEXT SENTENCE inside a TRY",
            "",
            "MAIN-PARA.\n    TRY\n        IF WS-I = 0\n            NEXT SENTENCE\n        END-IF\n    END-TRY\n    STOP RUN.\n",
            "NEXT SENTENCE",
        ),
        (
            "GO TO … DEPENDING ON inside a TRY",
            "",
            "MAIN-PARA.\n    TRY\n        GO TO P1 DEPENDING ON WS-I\n    END-TRY\n    STOP RUN.\nP1.\n    STOP RUN.\n",
            "DEPENDING ON",
        ),
    ];
    let line_of = |src: &str, needle: &str| src.lines().position(|l| l.contains(needle)).map(|i| i as u32 + 1).expect("the line is in the source");
    let mut checks = 0;
    let mut failures: Vec<String> = Vec::new();
    for (what, items, procedure, needle) in refused {
        let src = program(items, procedure);
        let line = line_of(&src, needle);
        for (label, opts) in [("desktop", native(true)), ("browser", Options::default())] {
            match compile_program(&common::parse(&src), &opts) {
                Ok(_) => failures.push(format!("{label}: {what}: compiled")),
                Err(e) if e.line == line => checks += 1,
                Err(e) => failures.push(format!("{label}: {what}: refused at line {} (want {line}): {e}", e.line)),
            }
        }
    }
    println!("== native_exceptions: refusals (DESKTOP-WASM-001 T45) ==");
    println!("result: {checks} of {} refusals at their line", refused.len() * 2);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// WASM-PARITY #77: a run-time error that is not a `THROW` ends a module's run where it is found,
/// so a `FINALLY` around it does not run; the interpreter runs `FINALLY` first and then ends with
/// the error. Everything before the error, and the run ending in an error, are the same.
#[test]
fn a_routine_error_inside_a_try_ends_a_module_before_finally() {
    let src = program(
        "01 WS-ZERO PIC 9 VALUE 0.\n",
        "MAIN-PARA.\n    DISPLAY \"BEFORE\"\n    TRY\n        DISPLAY FUNCTION MOD(7 WS-ZERO)\n    FINALLY\n        DISPLAY \"CLEANUP\"\n\
         END-TRY\n    DISPLAY \"NOT REACHED\"\n    STOP RUN.\n",
    );
    let t = Instant::now();
    let (want_lines, want_end) = interpreter(&src);
    let t_interp = t.elapsed();
    let t = Instant::now();
    let c = compile_program(&common::parse(&src), &native(true)).expect("the program compiles");
    let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
    let t_module = t.elapsed();
    let got = common::wasm_lines(&o);
    let codes: Vec<u32> = o.errors.iter().map(|e| e.code).collect();
    println!("== native_exceptions: a routine error inside a TRY (WASM-PARITY #77) ==");
    println!("interpreter: {want_lines:?}, then {want_end:?} ({:.1} ms)", t_interp.as_secs_f64() * 1000.0);
    println!("module:      {got:?}, then errors {codes:?} ({:.1} ms)", t_module.as_secs_f64() * 1000.0);
    assert_eq!(want_lines, ["BEFORE", "CLEANUP"], "the interpreter runs FINALLY before the error ends the run");
    assert!(want_end.is_some(), "the interpreter's run ends in the error");
    assert_eq!(got, ["BEFORE"], "the module ends the run where the error is found");
    assert_eq!(codes.len(), 1, "the module's run ends in one error");
    println!("result: 4 of 4 checks; the one difference is FINALLY's line");
}
