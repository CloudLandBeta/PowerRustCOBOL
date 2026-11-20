// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 AC16 (2.0.628): **a module carries no function type it does not use.** Every type
//! in its type section is named by an import, a function or one of its blocks.
//!
//! A type the routine blob holds for a routine the module does not link, or one of a glue function the
//! module does not make (a desktop import, in a browser module), would put bytes in the module that
//! change when the blob or the desktop's imports change. Until 2.0.628 every module carried all of the
//! blob's types in the blob's order, so a routine added for the desktop moved the browser's modules and
//! the PowerAnalyticsWeb pin had to be recorded again.

use std::collections::BTreeSet;
use std::time::Instant;

use cobolt_forms::model::{Control, ControlType, EventBinding, Form};
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;
use wasmparser::{BlockType, Operator, Parser, Payload, TypeRef};

/// The number of types in `module`'s type section, and the ones nothing in it names.
fn unused_types(module: &[u8]) -> (usize, Vec<u32>) {
    let mut declared = 0usize;
    let mut used: BTreeSet<u32> = BTreeSet::new();
    for payload in Parser::new(0).parse_all(module) {
        match payload.expect("a module the generator wrote reads") {
            Payload::TypeSection(r) => declared = r.count() as usize,
            Payload::ImportSection(r) => {
                for imports in r {
                    for import in imports.expect("an import reads") {
                        if let TypeRef::Func(t) = import.expect("an import reads").1.ty {
                            used.insert(t);
                        }
                    }
                }
            }
            Payload::FunctionSection(r) => used.extend(r.into_iter().map(|t| t.expect("a function's type reads"))),
            Payload::CodeSectionEntry(body) => {
                let mut ops = body.get_operators_reader().expect("a body reads");
                while !ops.eof() {
                    match ops.read().expect("an instruction reads") {
                        Operator::CallIndirect { type_index, .. } | Operator::ReturnCallIndirect { type_index, .. } => {
                            used.insert(type_index);
                        }
                        Operator::Block { blockty: BlockType::FuncType(t) }
                        | Operator::Loop { blockty: BlockType::FuncType(t) }
                        | Operator::If { blockty: BlockType::FuncType(t) } => {
                            used.insert(t);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    (declared, (0..declared as u32).filter(|t| !used.contains(t)).collect())
}

/// A form with a button whose handler writes a label: the player fixture's form.
fn browser_module(multi: bool) -> Vec<u8> {
    let mut form = Form::new("main-form", "Generated", 400, 300);
    let mut btn = Control::new("BTN-GO", ControlType::Button, 10, 10);
    let mut ev = EventBinding::for_control("BTN-GO", "onClick");
    ev.code = "       PROCEDURE DIVISION.\n           MOVE \"from the module\" TO LBL-OUT::Caption\n           DISPLAY \"clicked\".".to_string();
    btn.events.push(ev);
    form.controls.push(btn);
    form.controls.push(Control::new("LBL-OUT", ControlType::Label, 10, 60));
    let (src, _) = cobolt_codegen::generate_with_map(&form);
    let program = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free)).program.expect("the generated program parses");
    let spec = FormSpec {
        name: form.name.clone(),
        main: true,
        instances: 1,
        program: &program,
        controls: form.controls.iter().map(|c| (c.id.clone(), format!("{:?}", c.control_type))).collect(),
        bindings: vec![("BTN-GO".into(), "onClick".into())],
        layers: vec![],
        lines: vec![],
    };
    let opts = Options { multi_memory: multi, name: "types".into(), ..Options::default() };
    compile_project(std::slice::from_ref(&spec), &opts).unwrap_or_else(|e| panic!("{e}")).wasm
}

/// A desktop console program that reaches most of the desktop's imports: disk files, a built-in's text
/// file, the HTTP and PartiQL built-ins, the environment and the clock.
const DESKTOP: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. TYPES.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT ORDERS ASSIGN TO \"orders.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS ORD-ID.
DATA DIVISION.
FILE SECTION.
FD ORDERS.
01 ORD-REC.
   05 ORD-ID   PIC 9(6).
   05 ORD-CITY PIC X(12).
WORKING-STORAGE SECTION.
01 WS-N    PIC 9(4).
01 WS-ST   PIC XX.
01 WS-BODY PIC X(40).
01 WS-HOME PIC X(40).
01 WS-NOW  PIC X(21).
PROCEDURE DIVISION.
    OPEN I-O ORDERS
    MOVE 1 TO ORD-ID
    READ ORDERS KEY IS ORD-ID INVALID KEY DISPLAY \"none\" END-READ
    COBOL::\"PARTIQL\" ( \"ORDERS\" \"SELECT * FROM ORDERS\" WS-N WS-ST )
    COBOL::\"HTTP-GET\" ( \"http://127.0.0.1:9/x\" WS-BODY WS-N )
    COBOL::\"WRITE-FILE\" ( \"out.txt\" WS-BODY )
    ACCEPT WS-HOME FROM ENVIRONMENT \"HOME\"
    MOVE FUNCTION CURRENT-DATE TO WS-NOW
    CLOSE ORDERS
    STOP RUN.
";

#[test]
fn a_module_carries_no_type_it_does_not_use() {
    let t = Instant::now();
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(DESKTOP, cobolt_lexer::SourceFormat::Free));
    let program = parsed.program.expect("the desktop program parses");
    let native = Options { native: true, ..Options::default() };
    let desktop = cobolt_wasm::compile_program(&program, &native).unwrap_or_else(|e| panic!("the generator refused the desktop program: {e}")).wasm;
    let modules = [("browser, multi-memory", browser_module(true)), ("browser, single memory", browser_module(false)), ("desktop console", desktop)];
    let compile_ms = t.elapsed().as_secs_f64() * 1000.0;

    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let mut types_total = 0;
    for (name, module) in &modules {
        let (declared, unused) = unused_types(module);
        types_total += declared;
        if !unused.is_empty() {
            failures.push(format!("{name}: types {unused:?} of {declared} are named by nothing"));
        }
        rows.push(format!("  {name:<23} {declared:>3} types, {:>2} unused; {:>7} bytes", unused.len(), module.len()));
    }
    let desktop_imports: Vec<String> = cobolt_wasm_abi::scan::scan_module(&modules[2].1).unwrap().imports.iter().filter(|i| i.module == "prn").map(|i| i.name.clone()).collect();
    for wanted in ["f_open", "http_call", "pq_file", "t_write", "env_get"] {
        if !desktop_imports.iter().any(|i| i == wanted) {
            failures.push(format!("the desktop program imports prn.{wanted}: it imports {desktop_imports:?}"));
        }
    }

    println!("── a_module_carries_no_type_it_does_not_use (DESKTOP-WASM-001 AC16) ──");
    println!("  modules: a browser form (both memory variants) and a desktop console program importing {} prn functions", desktop_imports.len());
    for r in &rows {
        println!("{r}");
    }
    println!("  {types_total} types checked in {} modules, compiled in {compile_ms:.1} ms", modules.len());
    println!("  checks: {} passed, {} failed", modules.len() + 5 - failures.len(), failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
