// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the tree-walking interpreter costs per statement — the number that
//! decides what native code generation (JIT or AOT) could buy.
//!
//! Worth stating plainly: `rcrun build` does NOT compile COBOL to machine code.
//! It serialises the AST, embeds it with `include_bytes!`, and the produced
//! binary runs `Interpreter::new(program)` over it. So a "compiled"
//! PowerRustCOBOL application tree-walks every statement exactly like `rcrun
//! run` does, and these numbers apply to both.
//!
//! The native baseline below does the SAME fixed-point work the interpreter
//! does (i128 accumulate, PIC 9(n) truncation), through `black_box` so the
//! optimiser cannot delete it. It is the floor a code generator would approach,
//! not a claim that generated code would hit it exactly.
//!
//! Run with:
//! `cargo test --release -p cobolt-runtime --test bench_interp_throughput -- --nocapture`

use std::hint::black_box;
use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

const ITERS: i64 = 300_000;

fn run_ns(src: &str) -> f64 {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_tx, rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, _d) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, rx, state_tx, display_tx);
    let start = Instant::now();
    interp.run().expect("run failed");
    start.elapsed().as_nanos() as f64
}

#[test]
fn interpreter_cost_per_statement() {
    // An empty loop: measures the loop control alone (test + increment).
    let empty = format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. L1.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01 I   PIC 9(9) VALUE 0.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN-PARA.\n\
         \x20          PERFORM VARYING I FROM 1 BY 1 UNTIL I > {ITERS}\n\
         \x20              CONTINUE\n\
         \x20          END-PERFORM.\n\
         \x20          STOP RUN.\n"
    );
    // The same loop with three ordinary statements in the body.
    let body = format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. L2.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01 I   PIC 9(9) VALUE 0.\n\
         \x20      01 ACC PIC 9(9) VALUE 0.\n\
         \x20      01 TMP PIC 9(9) VALUE 0.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN-PARA.\n\
         \x20          PERFORM VARYING I FROM 1 BY 1 UNTIL I > {ITERS}\n\
         \x20              ADD 1 TO ACC\n\
         \x20              MOVE ACC TO TMP\n\
         \x20              COMPUTE ACC = TMP + 1\n\
         \x20          END-PERFORM.\n\
         \x20          DISPLAY ACC.\n\
         \x20          STOP RUN.\n"
    );

    let n = ITERS as f64;
    let loop_only = run_ns(&empty) / n;
    let with_body = run_ns(&body) / n;
    let per_stmt = (with_body - loop_only) / 3.0;

    // Native floor: the same accumulate + move + compute on fixed-point i128.
    let start = Instant::now();
    let mut acc: i128 = 0;
    for _ in 0..ITERS {
        acc = black_box(acc) + 1;
        let tmp: i128 = black_box(acc);
        acc = black_box(tmp) + 1;
        acc %= 1_000_000_000; // PIC 9(9) truncation
    }
    black_box(acc);
    let native = start.elapsed().as_nanos() as f64 / n;

    println!("\n  ── Tree-walking interpreter, per iteration ──");
    println!("  empty PERFORM VARYING loop        {loop_only:>9.1} ns");
    println!("  loop + 3 statements               {with_body:>9.1} ns");
    println!("  → cost of ONE simple statement    {per_stmt:>9.1} ns");
    println!("  native equivalent (3 stmts)       {native:>9.2} ns");
    println!(
        "  interpreter / native               {:>8.0}×",
        with_body / native.max(0.001)
    );
    println!(
        "  statements/sec, one core           {:>9.0}\n",
        1e9 / per_stmt
    );
}
