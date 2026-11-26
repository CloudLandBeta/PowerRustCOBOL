// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T14 (R9, R20, AC3's engine half): the Backend module's entry against the interpreter.
//!
//! A small Backend is loaded as the server loads one. Each exported program is called through
//! `cobolt_server::exec::execute` (the interpreter, what a Cargo-built Backend runs on) and through the
//! module `cobolt_wasm::compile_backend` makes of the same programs, called as `pr_backend_call(export)`;
//! the two answers must be equal byte for byte, and a request one refuses the other must refuse, naming
//! the same member.

use cobolt_server::backend::Backend;
use cobolt_server::exec::{execute, ExecOptions};
use cobolt_wasm::host::{run_backend, BackendReply};
use cobolt_wasm::{compile_backend, BackendProgram, Options};
use cobolt_wire::error::{codes, ErrorClass};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One program of the test Backend: its source, and the calls to make on it. A call is one JSON text per
/// parameter.
struct Case {
    name: &'static str,
    source: &'static str,
    calls: &'static [(&'static str, &'static [&'static str])],
}

const KINDS: Case = Case {
    name: "KINDS",
    source: r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. KINDS.
DATA DIVISION.
LINKAGE SECTION.
01 REC.
    05 TXT  PIC X(8).
    05 UNS  PIC 9(5).
    05 SGN  PIC S9(4).
    05 DEC  PIC 9(3)V99.
    05 SDEC PIC S9(3)V99.
    05 BIN  PIC S9(4) COMP.
    05 PCK  PIC S9(5)V99 COMP-3.
PROCEDURE DIVISION USING REC.
MAIN-PARA.
    ADD 1 TO UNS.
    ADD 1 TO SGN.
    ADD 1 TO DEC.
    ADD 1 TO SDEC.
    ADD 1 TO BIN.
    ADD 1 TO PCK.
    MOVE "DONE" TO TXT.
    GOBACK.
"#,
    calls: &[
        ("ordinary", &[r#"{"TXT":"abc","UNS":7,"SGN":-3,"DEC":1.5,"SDEC":-2.25,"BIN":40,"PCK":1234.56}"#]),
        ("empty object", &["{}"]),
        ("at the limits", &[r#"{"TXT":"12345678","UNS":99998,"SGN":9998,"DEC":998.99,"SDEC":997.99,"BIN":9998,"PCK":99998.99}"#]),
        ("a text over its picture", &[r#"{"TXT":"123456789"}"#]),
        ("a number over its picture", &[r#"{"UNS":100000}"#]),
        ("a wrong type", &[r#"{"UNS":"seven"}"#]),
        ("an unknown member", &[r#"{"NOPE":1}"#]),
        ("a member twice", &[r#"{"UNS":1,"UNS":2}"#]),
        ("not JSON", &["{"]),
        ("no text at all", &[]),
    ],
};

const TABLE: Case = Case {
    name: "TABLE",
    source: r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TABLE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 9(2).
LINKAGE SECTION.
01 TBL.
    05 ROW OCCURS 5 TIMES.
        10 QTY PIC 9(3).
01 OUT.
    05 SUMQ PIC 9(5).
PROCEDURE DIVISION USING TBL RETURNING OUT.
MAIN-PARA.
    MOVE 0 TO SUMQ.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5
        ADD QTY(I) TO SUMQ
    END-PERFORM.
    GOBACK.
"#,
    calls: &[
        ("ordinary", &[r#"{"ROW":[{"QTY":1},{"QTY":2},{"QTY":3},{"QTY":4},{"QTY":5}]}"#]),
        ("fewer rows than the table", &[r#"{"ROW":[{"QTY":9}]}"#]),
        ("at the limit", &[r#"{"ROW":[{"QTY":999},{"QTY":999},{"QTY":999},{"QTY":999},{"QTY":999}]}"#]),
        ("a row over the table", &[r#"{"ROW":[{"QTY":1},{"QTY":1},{"QTY":1},{"QTY":1},{"QTY":1},{"QTY":1}]}"#]),
        ("empty", &["{}"]),
    ],
};

const ODO: Case = Case {
    name: "ODO",
    source: r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ODO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 9(2).
LINKAGE SECTION.
01 TBL.
    05 N PIC 9(2).
    05 ITEM OCCURS 1 TO 4 TIMES DEPENDING ON N.
        10 V PIC 9(3).
PROCEDURE DIVISION USING TBL.
MAIN-PARA.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > N
        ADD 1 TO V(I)
    END-PERFORM.
    GOBACK.
"#,
    calls: &[
        ("ordinary", &[r#"{"N":3,"ITEM":[{"V":1},{"V":2},{"V":3}]}"#]),
        ("the smallest", &[r#"{"N":1,"ITEM":[{"V":10}]}"#]),
        ("the largest", &[r#"{"N":4,"ITEM":[{"V":1},{"V":2},{"V":3},{"V":4}]}"#]),
        ("a count over the maximum", &[r#"{"N":5,"ITEM":[{"V":1},{"V":2},{"V":3},{"V":4},{"V":5}]}"#]),
        ("a count that does not agree", &[r#"{"N":2,"ITEM":[{"V":1}]}"#]),
    ],
};

const EARLY: Case = Case {
    name: "EARLY",
    source: r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. EARLY.
DATA DIVISION.
LINKAGE SECTION.
01 P.
    05 V PIC 9(3).
PROCEDURE DIVISION USING P.
MAIN-PARA.
    IF V > 100
        STOP RUN
    END-IF.
    IF V > 50
        GOBACK
    END-IF.
    ADD 1 TO V.
    GOBACK.
"#,
    calls: &[("ordinary", &[r#"{"V":5}"#]), ("ends early on GOBACK", &[r#"{"V":60}"#]), ("ends early on STOP RUN", &[r#"{"V":200}"#])],
};

const ADDER: Case = Case {
    name: "ADDER",
    source: r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ADDER.
DATA DIVISION.
LINKAGE SECTION.
01 A PIC 9(3).
01 B PIC 9(3).
PROCEDURE DIVISION USING A B.
MAIN-PARA.
    ADD A TO B.
    GOBACK.
"#,
    calls: &[("ordinary", &["5", "7"]), ("at the limit", &["999", "999"]), ("one text short", &["5"])],
};

const CALLER: Case = Case {
    name: "CALLER",
    source: r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. CALLER.
DATA DIVISION.
LINKAGE SECTION.
01 REQ.
    05 X PIC 9(3).
    05 Y PIC 9(3).
01 RES.
    05 NOTE PIC X(10).
    05 TOTAL PIC 9(4).
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    CALL "ADDER" USING X Y.
    MOVE Y TO TOTAL.
    MOVE "VIA ADDER" TO NOTE.
    GOBACK.
"#,
    calls: &[("ordinary", &[r#"{"X":10,"Y":32}"#]), ("empty", &["{}"]), ("over its picture", &[r#"{"X":1000}"#])],
};

const GREET: Case = Case {
    name: "GREET",
    source: r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. GREET.
DATA DIVISION.
LINKAGE SECTION.
01 WHO PIC X(10).
01 MSG PIC X(20).
PROCEDURE DIVISION USING WHO MSG.
MAIN-PARA.
    STRING "HELLO " DELIMITED BY SIZE WHO DELIMITED BY SPACE INTO MSG.
    GOBACK.
"#,
    calls: &[
        ("ordinary", &[r#""WORLD""#, r#""""#]),
        ("with accents", &[r#""café""#, r#""""#]),
        ("a text at its picture", &[r#""1234567890""#, r#""""#]),
        ("a text over its picture", &[r#""12345678901""#, r#""""#]),
        ("a number for a text", &["7", r#""""#]),
    ],
};

const UNSET: Case = Case {
    name: "UNSET",
    source: r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. UNSET.
DATA DIVISION.
LINKAGE SECTION.
01 REQ PIC 9(2).
01 RES.
    05 M PIC 9(2).
    05 CELL PIC X OCCURS 1 TO 5 TIMES DEPENDING ON M.
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    IF REQ > 80
        MOVE 9 TO M
    ELSE
        IF REQ > 50
            MOVE 3 TO M
        END-IF
    END-IF.
    GOBACK.
"#,
    calls: &[("a count the program sets", &["60"]), ("a count the program never sets", &["5"]), ("a count over the maximum", &["90"])],
};

const CASES: [&Case; 8] = [&KINDS, &TABLE, &ODO, &EARLY, &ADDER, &CALLER, &GREET, &UNSET];

fn folder() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-backend-entry-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let mut exports = String::from("[contract]\nversion = \"1.0.0\"\n");
    for c in CASES {
        exports.push_str(&format!("\n[[export]]\nprogram = \"{}\"\n", c.name));
        std::fs::write(dir.join("src").join(format!("{}.cbl", c.name.to_ascii_lowercase())), c.source).unwrap();
    }
    std::fs::write(dir.join("exports.toml"), exports).unwrap();
    dir
}

/// What one engine said, reduced to what both must agree on.
#[derive(Debug)]
enum Said {
    Answered {
        args: Vec<String>,
        returning: Option<String>,
    },
    /// Refused as an invalid payload, naming the member; `note` is what the engine said, for the report only.
    Refused {
        path: String,
        note: String,
    },
    /// Refused for a reason of the request's shape that is the same on both (an arity the server checks first).
    BadArity,
}

impl PartialEq for Said {
    fn eq(&self, other: &Said) -> bool {
        match (self, other) {
            (Said::Answered { args: a, returning: r }, Said::Answered { args: b, returning: s }) => a == b && r == s,
            (Said::Refused { path: a, .. }, Said::Refused { path: b, .. }) => a == b,
            (Said::BadArity, Said::BadArity) => true,
            _ => false,
        }
    }
}

fn interpreter(prog: &cobolt_server::backend::ExportedProgram, args: &[&str]) -> Said {
    // The arity is the server's check (`engine::check_arity`, the same for every engine): it is made here as it is
    // there, before an engine is asked.
    if cobolt_server::engine::check_arity(prog, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).is_err() {
        return Said::BadArity;
    }
    let texts: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let opts = ExecOptions::plain(Arc::new(AtomicBool::new(false)), Duration::from_secs(1));
    match execute(prog, &texts, opts) {
        Ok(r) => Said::Answered { args: r.args, returning: r.returning },
        Err(e) if e.class == ErrorClass::Validation && e.code == codes::INVALID_PAYLOAD => {
            Said::Refused { path: e.detail.clone().unwrap_or_default(), note: e.message.clone() }
        }
        Err(e) => panic!("the interpreter failed the call unexpectedly: {e}"),
    }
}

fn module(wasm: &[u8], name: &str, params: usize, args: &[&str]) -> Said {
    // The server checks the arity before an engine runs; the module is asked only what the server would ask it.
    if args.len() != params {
        return Said::BadArity;
    }
    match run_backend(wasm, name, args).unwrap_or_else(|e| panic!("the module could not be called: {e}")) {
        BackendReply::Answered(a) => Said::Answered { args: a.args, returning: a.returning },
        BackendReply::Refused(r) => Said::Refused { path: r.path, note: format!("rt code {}", r.code) },
        // The program ran and a record it holds cannot be written: the interpreter answers it with the same validation
        // error, naming the member.
        BackendReply::Unwritable(r) => Said::Refused { path: r.path, note: format!("rt code {} (written back)", r.code) },
        other => panic!("the module answered {other:?} where the interpreter ran the program"),
    }
}

#[test]
fn the_backend_module_answers_every_call_as_the_interpreter_does() {
    let dir = folder();
    let backend = Backend::load(&dir).unwrap_or_else(|e| panic!("the test Backend does not load: {e:?}"));
    let exported: Vec<_> = CASES.iter().map(|c| backend.program(c.name).expect("exported").clone()).collect();

    let started = Instant::now();
    let list: Vec<BackendProgram<'_>> =
        exported.iter().map(|e| BackendProgram { name: e.sig.name.clone(), program: e.program.as_ref().expect("the interpreter's source") }).collect();
    let compiled = compile_backend(&list, &Options { native: true, ..Options::default() }).unwrap_or_else(|e| panic!("the module does not compile: {e}"));
    let compile_ms = started.elapsed().as_millis();

    let (mut calls, mut equal, mut answered, mut refused, mut arity) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let (mut interp_ms, mut module_ms) = (0u128, 0u128);
    let mut different: Vec<String> = Vec::new();
    let mut rows: Vec<String> = Vec::new();
    for (case, prog) in CASES.iter().zip(&exported) {
        let (mut ok_here, mut refused_here) = (0u32, 0u32);
        for (what, args) in case.calls {
            let t = Instant::now();
            let a = interpreter(prog, args);
            interp_ms += t.elapsed().as_millis();
            let t = Instant::now();
            let b = module(&compiled.wasm, case.name, prog.params.len(), args);
            module_ms += t.elapsed().as_millis();
            calls += 1;
            match &a {
                Said::Answered { .. } => {
                    answered += 1;
                    ok_here += 1;
                }
                Said::Refused { .. } => {
                    refused += 1;
                    refused_here += 1;
                }
                Said::BadArity => arity += 1,
            }
            if a == b {
                equal += 1;
            } else {
                different.push(format!("{} / {what}\n  interpreter: {a:?}\n  module:      {b:?}", case.name));
            }
        }
        rows.push(format!("  {:<7} {:>2} calls: {ok_here} answered, {refused_here} refused", case.name, case.calls.len()));
        assert!(ok_here > 0, "{} has no call that runs: the comparison would prove nothing for it", case.name);
    }
    println!("=== BACKEND ENTRY PARITY (spec 114 T14) ===");
    println!("programs {}: module {} bytes, compiled in {compile_ms} ms", CASES.len(), compiled.wasm.len());
    for r in &rows {
        println!("{r}");
    }
    println!(
        "calls {calls}: equal {equal}, different {} ({answered} answered, {refused} refused, {arity} arity): interpreter {interp_ms} ms, module {module_ms} ms (a fresh instance each call)",
        different.len()
    );
    println!("PASS {equal:03} FAIL {:03}", different.len());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(different.is_empty(), "the module and the interpreter differ:\n{}", different.join("\n"));
}
