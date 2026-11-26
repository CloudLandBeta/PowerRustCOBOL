// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A Backend parameter the runtime's JSON routines cannot hold is refused at generation time (spec 114, review of
//! M2): the routines keep a copy of the record (512 KiB) and a fixed table of the `OCCURS DEPENDING ON` counts
//! they meet, so a program whose record can exceed either would answer every call with an internal error where the
//! interpreter answers it. Refused here, the Build takes the Cargo path for that Backend instead.
//!
//! The limits are read from the runtime's own constants, so a change to them moves this test with them.
mod common;

use cobolt_wasm::host::{run_backend, BackendReply};
use cobolt_wasm::{compile_backend, BackendProgram, Options};
use cobolt_wasm_rt::{json::MAX_ODO, state::JSON_SHADOW_SIZE};
use std::time::Instant;

fn compile(source: &str) -> Result<cobolt_wasm::Compiled, String> {
    let p = common::parse(source);
    let list = [BackendProgram { name: "P".into(), program: &p }];
    compile_backend(&list, &Options { native: true, ..Options::default() }).map_err(|e| e.to_string())
}

fn compiles(source: &str) -> Result<(), String> {
    compile(source).map(|_| ())
}

/// A program whose parameter holds `tables` `OCCURS DEPENDING ON` tables side by side, each with its own count.
fn odo_tables(tables: usize) -> String {
    let mut s = String::from("IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nDATA DIVISION.\nLINKAGE SECTION.\n01 REQ.\n");
    for i in 0..tables {
        s.push_str(&format!("    05 N{i} PIC 9(1).\n    05 T{i} PIC 9(1) OCCURS 1 TO 2 TIMES DEPENDING ON N{i}.\n"));
    }
    s.push_str("PROCEDURE DIVISION USING REQ.\nMAIN-PARA.\n    GOBACK.\n");
    s
}

/// A program whose parameter is a table of `rows` rows, each holding an `OCCURS DEPENDING ON` table that depends on
/// one count outside the rows: the table has an occurrence of its own in every row.
fn odo_inside_a_table(rows: usize) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nDATA DIVISION.\nLINKAGE SECTION.\n01 REQ.\n    05 N PIC 9(1).\n    05 ROW OCCURS {rows} TIMES.\n        10 T PIC 9(1) OCCURS 1 TO 2 TIMES DEPENDING ON N.\nPROCEDURE DIVISION USING REQ.\nMAIN-PARA.\n    GOBACK.\n"
    )
}

fn sized(bytes: usize) -> String {
    format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nDATA DIVISION.\nLINKAGE SECTION.\n01 REQ.\n    05 BIG PIC X({bytes}).\nPROCEDURE DIVISION USING REQ.\nMAIN-PARA.\n    GOBACK.\n")
}

#[test]
fn a_parameter_the_json_routines_cannot_hold_is_refused_and_one_they_can_is_not() {
    let started = Instant::now();
    let mut checked = 0u32;

    // Size: at the limit it is taken, a byte over it is refused with the number and the limit.
    let limit = JSON_SHADOW_SIZE as usize;
    compiles(&sized(limit)).unwrap_or_else(|e| panic!("a record of {limit} bytes is refused: {e}"));
    let over = compiles(&sized(limit + 1)).expect_err("a record over the limit is taken");
    assert!(over.contains(&format!("{}", limit + 1)) && over.contains(&format!("{limit}")), "{over}");
    checked += 2;

    // The tables the record can hold: MAX_ODO of them is taken, one more is refused.
    compiles(&odo_tables(MAX_ODO)).unwrap_or_else(|e| panic!("{MAX_ODO} OCCURS DEPENDING ON tables are refused: {e}"));
    let many = compiles(&odo_tables(MAX_ODO + 1)).expect_err("one table over the limit is taken");
    assert!(many.contains("OCCURS DEPENDING ON") && many.contains(&MAX_ODO.to_string()), "{many}");
    checked += 2;

    // A table inside a table holds one occurrence of its own per row.
    compiles(&odo_inside_a_table(MAX_ODO)).unwrap_or_else(|e| panic!("{MAX_ODO} rows of a table are refused: {e}"));
    let rows = compiles(&odo_inside_a_table(MAX_ODO + 1)).expect_err("one row over the limit is taken");
    assert!(rows.contains("OCCURS DEPENDING ON"), "{rows}");
    checked += 2;

    // What is taken is held: a request that fills every table the record can hold is answered, not refused by the routines.
    let full = (0..MAX_ODO).map(|i| format!("\"N{i}\":2,\"T{i}\":[1,2]")).collect::<Vec<_>>().join(",");
    let m = compile(&odo_tables(MAX_ODO)).unwrap();
    let reply = run_backend(&m.wasm, "P", &[&format!("{{{full}}}")]).unwrap();
    assert!(matches!(reply, BackendReply::Answered(_)), "{MAX_ODO} tables, each filled: {reply:?}");
    let rows = (0..MAX_ODO).map(|_| "{\"T\":[1,2]}").collect::<Vec<_>>().join(",");
    let m = compile(&odo_inside_a_table(MAX_ODO)).unwrap();
    let reply = run_backend(&m.wasm, "P", &[&format!("{{\"N\":2,\"ROW\":[{rows}]}}")]).unwrap();
    assert!(matches!(reply, BackendReply::Answered(_)), "{MAX_ODO} rows, each with a table: {reply:?}");
    checked += 2;

    println!(
        "limits read from the runtime: record {} bytes, {} OCCURS DEPENDING ON tables: {checked} cases, at the limit taken and one over refused ({} ms)",
        JSON_SHADOW_SIZE,
        MAX_ODO,
        started.elapsed().as_millis()
    );
    println!("PASS {checked:03} FAIL 000");
}
