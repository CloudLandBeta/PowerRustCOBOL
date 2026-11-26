// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend entry (ABI A59, spec 114 T12): a module that holds every program a Backend exports and is
//! called as `pr_backend_call(export)` with one JSON text per parameter.

mod common;

use cobolt_wasm::host::{run_backend, run_backend_index, BackendReply};
use cobolt_wasm::{compile_backend, BackendOptions, BackendProgram, Options};
use cobolt_wasm_abi::backend::{BackendSection, Refusal};
use std::time::Instant;

const ADD: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ADD-TWO.
DATA DIVISION.
LINKAGE SECTION.
01 REQ.
   05 A PIC 9(4).
   05 B PIC 9(4).
01 RES.
   05 TOTAL PIC 9(5).
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    COMPUTE TOTAL = A + B.
    GOBACK.
"#;

const GREET: &str = r#"
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
"#;

fn backend_options() -> Options {
    Options { native: true, ..Options::default() }
}

/// Every program nested in each of the others, as `cobolt_server::Backend::build` nests them.
fn module(sources: &[&str]) -> cobolt_wasm::Compiled {
    module_with(sources, &backend_options())
}

fn module_with(sources: &[&str], options: &Options) -> cobolt_wasm::Compiled {
    let programs: Vec<cobolt_ast::program::Program> = sources.iter().map(|s| common::parse(s)).collect();
    let tops: Vec<cobolt_ast::program::Program> = programs
        .iter()
        .map(|p| {
            let mut top = p.clone();
            for other in &programs {
                if other.identification.program_id != p.identification.program_id {
                    top.nested_programs.push(other.clone());
                }
            }
            top
        })
        .collect();
    let list: Vec<BackendProgram<'_>> = tops.iter().map(|t| BackendProgram { name: t.identification.program_id.to_ascii_uppercase(), program: t }).collect();
    compile_backend(&list, options).unwrap_or_else(|e| panic!("compile failed: {e}"))
}

#[test]
fn a_backend_module_has_the_entry_and_the_section_and_no_other_module_does() {
    let started = Instant::now();
    let c = module(&[ADD, GREET]);
    let scan = cobolt_wasm_abi::scan::scan_module(&c.wasm).unwrap();
    assert!(scan.has_export("pr_backend_call"), "the entry is exported");
    assert!(!scan.has_export("pr_run"), "a Backend module is not a program module");
    let text = std::str::from_utf8(scan.custom("prmanifest").unwrap()).unwrap();
    let section = BackendSection::from_manifest(text).unwrap().expect("the section is there");
    assert_eq!(section.exports.iter().map(|e| (e.name.as_str(), e.params, e.returning)).collect::<Vec<_>>(), vec![("ADD-TWO", 1, true), ("GREET", 2, false)]);
    // Any other module: neither the export nor the section.
    let plain =
        common::compile("IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nPROCEDURE DIVISION.\nMAIN-PARA.\n    DISPLAY \"X\".\n    STOP RUN.\n", &Options::default());
    let pscan = cobolt_wasm_abi::scan::scan_module(&plain.wasm).unwrap();
    assert!(!pscan.has_export("pr_backend_call"));
    assert_eq!(BackendSection::from_manifest(std::str::from_utf8(pscan.custom("prmanifest").unwrap()).unwrap()).unwrap(), None);
    // The Web bundler refuses a Backend module in words.
    let problems = cobolt_wasm_abi::scan::bundle_problems(&c.wasm, false);
    assert!(problems.iter().any(|p| p.contains("Backend module")), "{problems:?}");
    println!(
        "backend module: {} bytes, {} exports, entry and section present; a plain module has neither ({} ms)",
        c.wasm.len(),
        section.exports.len(),
        started.elapsed().as_millis()
    );
}

#[test]
fn a_valid_request_runs_and_an_over_limit_one_is_refused_before_the_program_runs() {
    let started = Instant::now();
    let c = module(&[ADD, GREET]);
    let reply = run_backend(&c.wasm, "ADD-TWO", &[r#"{"A":7,"B":35}"#]).unwrap();
    println!("ADD-TWO answered {reply:?}");
    let BackendReply::Answered(a) = reply else { panic!("not answered") };
    assert_eq!(a.args.len(), 1);
    assert!(a.args[0].contains("0007") || a.args[0].contains('7'), "{:?}", a.args);
    let ret = a.returning.expect("RETURNING");
    assert!(ret.contains("42"), "{ret}");

    let greet = run_backend(&c.wasm, "GREET", &[r#""WORLD""#, r#""""#]).unwrap();
    println!("GREET answered {greet:?}");
    let BackendReply::Answered(g) = greet else { panic!("not answered") };
    assert!(g.args[1].contains("HELLO WORLD"), "{:?}", g.args);
    assert_eq!(g.returning, None);

    // A string longer than its PICTURE is refused with the mapping's code and the member's path.
    let refused = run_backend(&c.wasm, "GREET", &[r#""THIS-IS-MUCH-TOO-LONG""#, r#""""#]).unwrap();
    println!("GREET refused {refused:?}");
    assert!(matches!(refused, BackendReply::Refused(Refusal { code: 6, .. })), "{refused:?}");
    // An unknown member of a group.
    let unknown = run_backend(&c.wasm, "ADD-TWO", &[r#"{"A":1,"Z":2}"#]).unwrap();
    println!("ADD-TWO refused {unknown:?}");
    assert!(matches!(unknown, BackendReply::Refused(Refusal { code: 3, .. })), "{unknown:?}");
    // More texts than parameters.
    let extra = run_backend(&c.wasm, "GREET", &[r#""A""#, r#""""#, r#""""#]).unwrap();
    assert_eq!(extra, BackendReply::BadRequest);
    println!("valid request answered, over-limit refused (6), unknown member refused (3), extra text bad request ({} ms)", started.elapsed().as_millis());
}

const NO_PARAMS: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. NO-PARAMS.
DATA DIVISION.
LINKAGE SECTION.
01 RES.
   05 V PIC 9(3).
PROCEDURE DIVISION RETURNING RES.
MAIN-PARA.
    MOVE 123 TO V.
    GOBACK.
"#;

const SAME: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SAME-ONE.
DATA DIVISION.
LINKAGE SECTION.
01 REQ.
   05 A PIC 9(4).
PROCEDURE DIVISION USING REQ RETURNING REQ.
MAIN-PARA.
    ADD 1 TO A.
    GOBACK.
"#;

const BOOM: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. BOOM.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 ZED PIC 9 VALUE 0.
01 R PIC 9(2).
LINKAGE SECTION.
01 P PIC 9(2).
PROCEDURE DIVISION USING P.
MAIN-PARA.
    COMPUTE R = FUNCTION MOD(7, ZED).
    GOBACK.
"#;

const UNSET: &str = r#"
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
    GOBACK.
"#;

#[test]
fn the_paths_of_the_entry_that_a_server_does_not_reach() {
    let started = Instant::now();
    let c = module(&[ADD, NO_PARAMS, SAME, BOOM, UNSET]);

    // A program with no parameter: the request is an empty frame, and RETURNING is still answered.
    let BackendReply::Answered(n) = run_backend(&c.wasm, "NO-PARAMS", &[]).unwrap() else { panic!("NO-PARAMS not answered") };
    assert!(n.args.is_empty(), "{:?}", n.args);
    assert!(n.returning.as_deref().is_some_and(|r| r.contains("123")), "{:?}", n.returning);

    // RETURNING the very record a parameter is: both are answered, with the same text.
    let BackendReply::Answered(s) = run_backend(&c.wasm, "SAME-ONE", &[r#"{"A":41}"#]).unwrap() else { panic!("SAME-ONE not answered") };
    assert_eq!(s.args.len(), 1);
    assert_eq!(s.returning.as_deref(), Some(s.args[0].as_str()));
    assert!(s.args[0].contains("42"), "{:?}", s.args);

    // An export index the manifest does not hold, and a negative one: a bad request, nothing runs.
    let past = run_backend_index(&c.wasm, 5, &[r#"{"A":1,"B":2}"#]).unwrap();
    let negative = run_backend_index(&c.wasm, -1, &[]).unwrap();
    assert_eq!((&past, &negative), (&BackendReply::BadRequest, &BackendReply::BadRequest));

    // A run-time error of the program is a trap (ABI §9), not a result.
    let trapped = run_backend(&c.wasm, "BOOM", &["7"]).unwrap();
    assert!(matches!(trapped, BackendReply::Trapped { .. }), "{trapped:?}");

    // A record the program leaves out of its range cannot be written back: result 4, with the member's path.
    let unwritable = run_backend(&c.wasm, "UNSET", &["5"]).unwrap();
    println!("UNSET answered {unwritable:?}");
    let BackendReply::Unwritable(u) = unwritable else { panic!("not unwritable") };
    assert!(u.path.contains("CELL") || u.path.contains("RES"), "{u:?}");
    println!(
        "no parameter answered, RETURNING a parameter answered, index 5 and -1 bad requests, a run-time error trapped, an unset count unwritable (code {}, {}) in {} ms",
        u.code,
        u.path,
        started.elapsed().as_millis()
    );
}

#[test]
fn an_answer_that_does_not_fit_the_response_area_is_too_large_and_a_short_call_is_filled_in() {
    let started = Instant::now();
    // The smallest response area (1,032 bytes): a request this small fits, an answer carrying a 2,000-byte text does not.
    let tight = Options { native: true, backend: Some(BackendOptions { request_cap: 4096, response_cap: 1032 }), ..Options::default() };
    let wide = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. WIDE.
DATA DIVISION.
LINKAGE SECTION.
01 WHO PIC X(10).
01 OUT PIC X(2000).
PROCEDURE DIVISION USING WHO OUT.
MAIN-PARA.
    MOVE ALL "X" TO OUT.
    GOBACK.
"#;
    let c = module_with(&[wide], &tight);
    let reply = run_backend(&c.wasm, "WIDE", &[r#""A""#, r#""""#]).unwrap();
    assert_eq!(reply, BackendReply::TooLarge, "{reply:?}");

    // One text for two parameters: the second is initialised, as `INITIALIZE` leaves a record (a server rejects such a
    // call by its arity rule before an engine runs; only a direct caller of the module gets here).
    let c = module(&[GREET_SHORT]);
    let BackendReply::Answered(a) = run_backend(&c.wasm, "GREET-SHORT", &[r#""BOB""#]).unwrap() else { panic!("not answered") };
    assert_eq!(a.args.len(), 2);
    assert!(a.args[1].contains("HELLO BOB"), "{:?}", a.args);
    println!("an answer over a 1,032-byte area: too large; one text for two parameters: the second initialised ({} ms)", started.elapsed().as_millis());
}

const GREET_SHORT: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. GREET-SHORT.
DATA DIVISION.
LINKAGE SECTION.
01 WHO PIC X(10).
01 MSG PIC X(20).
PROCEDURE DIVISION USING WHO MSG.
MAIN-PARA.
    STRING "HELLO " DELIMITED BY SIZE WHO DELIMITED BY SPACE INTO MSG.
    GOBACK.
"#;
