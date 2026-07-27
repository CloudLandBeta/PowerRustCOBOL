// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PowerRustCOBOL performance and allocation baseline.
//!
//! ```sh
//! cargo run --release -p cobolt-bench             # everything
//! cargo run --release -p cobolt-bench -- dispatch # one workload, by substring
//! PRC_BENCH_SCALE=0.1 cargo run --release -p cobolt-bench   # a tenth of the work
//! ```
//!
//! # What this measures, and why in-process
//!
//! Each workload runs through the **same path a shipped binary takes**: the
//! program is tokenized, parsed, analysed and handed to `Interpreter::run`,
//! exactly as `rcrun build`'s generated `main.rs` does with its embedded AST.
//! Running in-process rather than spawning the built executable is what makes
//! the allocator counters possible — the numbers describe the interpreter that
//! *is* inside every binary you ship.
//!
//! Startup cost and binary size are the two things this deliberately does not
//! capture; measure those on the real artifact from `rcrun build`.
//!
//! # Reading the output
//!
//! - **ops/sec** — the workload's own unit (COBOL statements, objects, records).
//! - **allocs** — trips through the allocator. The most stable signal there is:
//!   it does not move with machine load, so a change here is a real change.
//! - **MB churned** — total bytes allocated, live or not. High churn with low
//!   peak means the workload is allocating and freeing in a loop.
//! - **peak live** — the high-water mark, which is what a machine has to have.
//!
//! Always compare runs on the same machine. Absolute numbers travel badly
//! between platforms; ratios and allocation counts travel fine.

mod counting_alloc;

use counting_alloc::{reset_peak, Counters};
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOC: counting_alloc::CountingAlloc = counting_alloc::CountingAlloc;

// ── Result rows ───────────────────────────────────────────────────────────────

struct Row {
    name: &'static str,
    unit: &'static str,
    ops: u64,
    wall: Duration,
    counters: Counters,
}

impl Row {
    fn per_sec(&self) -> f64 {
        let s = self.wall.as_secs_f64();
        if s <= 0.0 {
            0.0
        } else {
            self.ops as f64 / s
        }
    }

    /// Allocations per unit of work — the number that says whether a workload
    /// is allocating once per record or once per field.
    fn allocs_per_op(&self) -> f64 {
        if self.ops == 0 {
            0.0
        } else {
            self.counters.allocations as f64 / self.ops as f64
        }
    }
}

/// Run one workload with the allocator counters wrapped around it.
fn measure<F: FnOnce() -> u64>(name: &'static str, unit: &'static str, body: F) -> Row {
    reset_peak();
    let before = Counters::snapshot();
    let t0 = Instant::now();
    let ops = body();
    let wall = t0.elapsed();
    let counters = before.since(Counters::snapshot());
    Row {
        name,
        unit,
        ops,
        wall,
        counters,
    }
}

// ── COBOL driver ──────────────────────────────────────────────────────────────

/// Compile and run a COBOL source exactly as a built binary would.
///
/// Panics on a parse or semantic error: a benchmark whose program does not
/// compile is measuring nothing, and silently reporting a number for it would
/// be worse than stopping.
fn run_cobol(src: &str) {
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;
    use cobolt_runtime::Interpreter;
    use cobolt_semantic::{analyze, Severity};

    let result = parse(tokenize(src, SourceFormat::Free));
    for d in &result.diagnostics {
        if d.severity == cobolt_parser::Severity::Error {
            panic!("benchmark program failed to parse: {}", d.message);
        }
    }
    let program = result.program.expect("benchmark program produced no AST");
    let sem = analyze(&program);
    for d in &sem.diagnostics {
        if d.severity == Severity::Error {
            panic!("benchmark program failed analysis: {}", d.message);
        }
    }
    let mut interp = Interpreter::new(program);
    if let Err(e) = interp.run() {
        if !e.is_exit_signal() {
            panic!("benchmark program failed at runtime: {e}");
        }
    }
}

// ── Workloads ─────────────────────────────────────────────────────────────────

/// Interpreter dispatch rate: a tight `PERFORM VARYING` whose body is one
/// statement, so what is being timed is the tree-walk itself — loop condition,
/// paragraph dispatch, operand resolution, store — with as little real work
/// underneath as COBOL allows.
///
/// This is the number that decides whether a bytecode VM is worth building. If
/// dispatch dominates, compiling the AST to a flat instruction stream pays; if
/// the value system dominates (compare against `record-batch`), it does not,
/// and the work belongs in `CobolValue` instead.
fn bench_dispatch(scale: f64) -> Row {
    let n = ((2_000_000.0 * scale) as u64).max(1_000);
    let src = format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. DISPATCH-BENCH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I        PIC 9(9)  COMP.
01 ACC      PIC 9(18) COMP.
01 LIMIT-N  PIC 9(9)  COMP VALUE {n}.
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE 0 TO ACC
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > LIMIT-N
        ADD 1 TO ACC
    END-PERFORM
    STOP RUN.
"#
    );
    // Each iteration is the loop test, the increment and the ADD — three
    // dispatched operations, which is what "statements/sec" counts here.
    measure("dispatch (PERFORM VARYING)", "stmt", move || {
        run_cobol(&src);
        n * 3
    })
}

/// Paragraph-call dispatch: `PERFORM <para>` N times. Separated from the loop
/// benchmark because a paragraph call costs more than an inline statement, and
/// knowing the gap tells you whether call overhead or statement overhead is the
/// thing to attack.
fn bench_paragraph_calls(scale: f64) -> Row {
    let n = ((500_000.0 * scale) as u64).max(500);
    let src = format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PARA-BENCH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I        PIC 9(9)  COMP.
01 ACC      PIC 9(18) COMP.
01 LIMIT-N  PIC 9(9)  COMP VALUE {n}.
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE 0 TO ACC
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > LIMIT-N
        PERFORM WORK-PARA
    END-PERFORM
    STOP RUN.
WORK-PARA.
    ADD 1 TO ACC.
"#
    );
    measure("dispatch (PERFORM paragraph)", "call", move || {
        run_cobol(&src);
        n
    })
}

/// Decimal arithmetic through `CobolNumeric`'s i128-scaled mantissa — COBOL
/// money math, which no real batch program avoids.
fn bench_decimal_math(scale: f64) -> Row {
    let n = ((500_000.0 * scale) as u64).max(500);
    let src = format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. DECIMAL-BENCH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I        PIC 9(9) COMP.
01 LIMIT-N  PIC 9(9) COMP VALUE {n}.
01 AMOUNT   PIC S9(13)V99 VALUE 1234.56.
01 RATE     PIC S9(3)V9(4) VALUE 1.0125.
01 TOTAL    PIC S9(15)V99 VALUE 0.
PROCEDURE DIVISION.
MAIN-PARA.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > LIMIT-N
        COMPUTE TOTAL = TOTAL + (AMOUNT * RATE)
    END-PERFORM
    STOP RUN.
"#
    );
    measure("decimal COMPUTE", "compute", move || {
        run_cobol(&src);
        n
    })
}

/// The named load: a batch of 1000 records built and read back, repeated, with
/// alphanumeric fields throughout.
///
/// Alphanumeric is the point. `CobolValue::String` owns a `Vec<u8>` sized to the
/// PIC width, so every field is a heap allocation and every `MOVE` between two
/// of them is another. `allocs/op` on this row is therefore roughly "allocations
/// per record", and is the single most actionable number the harness produces.
fn bench_record_batch(scale: f64) -> Row {
    let passes = ((200.0 * scale) as u64).max(5);
    let src = format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. BATCH-BENCH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I         PIC 9(9) COMP.
01 P         PIC 9(9) COMP.
01 PASSES    PIC 9(9) COMP VALUE {passes}.
01 CHECKSUM  PIC 9(18) COMP VALUE 0.
01 CUSTOMER-TABLE.
   05 CUSTOMER-ROW OCCURS 1000 TIMES.
      10 CUST-ID      PIC X(10).
      10 CUST-NAME    PIC X(30).
      10 CUST-CITY    PIC X(20).
      10 CUST-BALANCE PIC S9(11)V99.
01 WS-ID      PIC X(10).
01 WS-NAME    PIC X(30).
01 WS-CITY    PIC X(20).
PROCEDURE DIVISION.
MAIN-PARA.
    PERFORM VARYING P FROM 1 BY 1 UNTIL P > PASSES
        PERFORM WRITE-BATCH
        PERFORM READ-BATCH
    END-PERFORM
    STOP RUN.
WRITE-BATCH.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 1000
        MOVE "CUST000000"      TO CUST-ID(I)
        MOVE "ACME INDUSTRIES LIMITED"   TO CUST-NAME(I)
        MOVE "SAO PAULO"       TO CUST-CITY(I)
        MOVE 1500.75           TO CUST-BALANCE(I)
    END-PERFORM.
READ-BATCH.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 1000
        MOVE CUST-ID(I)   TO WS-ID
        MOVE CUST-NAME(I) TO WS-NAME
        MOVE CUST-CITY(I) TO WS-CITY
        ADD 1 TO CHECKSUM
    END-PERFORM.
"#
    );
    measure("record batch (1000 rows, write+read)", "record", move || {
        run_cobol(&src);
        passes * 1000 * 2
    })
}

/// Object create / destroy churn with property traffic, which is what a form
/// with many controls does every time COBOL touches it.
///
/// Driven through `ObjectRegistry` directly rather than through COBOL so the
/// measurement isolates the object store from the interpreter. That isolation
/// is the point: `CoboltObject`'s accessors upper-case the property name on
/// **every** read (`get_property`, `get_str`, `get_bool`, `get_i64`), which is
/// one `String` allocation per property access. This row is where that shows up
/// and where its removal would be visible.
fn bench_object_churn(scale: f64) -> Row {
    use cobolt_runtime::objects::ObjectRegistry;

    let objects = ((20_000.0 * scale) as u64).max(500);
    let reads_per_object = 8u64;

    measure("object churn (create/read/destroy)", "object", move || {
        let mut sink = 0u64;
        for round in 0..objects {
            let mut reg = ObjectRegistry::new();
            let name = format!("CONTROL-{round}");
            reg.register(name.clone(), "TextBox");
            reg.set_property(&name, "Caption", "Customer name");
            reg.set_property(&name, "Visible", true);
            reg.set_property(&name, "Enabled", true);
            reg.set_property(&name, "TabIndex", round as i64);

            for _ in 0..reads_per_object {
                if let Some(o) = reg.get(&name) {
                    if o.get_str("Caption").is_some() {
                        sink += 1;
                    }
                    if o.get_bool("Visible").unwrap_or(false) {
                        sink += 1;
                    }
                    sink += o.get_i64("TabIndex").unwrap_or(0) as u64 & 1;
                }
            }
            // reg drops here — the "destroy" half of the churn.
        }
        std::hint::black_box(sink);
        objects
    })
}

/// INDEXED file engine under load: bulk insert, then random-key reads against
/// the same redb store the COBOL `INDEXED` organisation writes to.
///
/// Recovered and generalised from the `open_table_cost` micro-benchmark that
/// lived `#[ignore]`d inside `cobolt-runtime::indexed_redb` — it only ever ran
/// if someone remembered the exact `--ignored` invocation, so in practice the
/// engine had no standing baseline. The table handle is opened once for the
/// whole write transaction, which is what that micro-benchmark concluded was
/// worth doing (~16% faster than opening twice per insert).
fn bench_indexed_redb(scale: f64) -> (Row, Row) {
    use redb::{Database, TableDefinition};

    const PRIMARY: TableDefinition<&[u8], &[u8]> = TableDefinition::new("primary");

    let n = ((100_000.0 * scale) as u64).max(1_000);
    let reads = ((50_000.0 * scale) as u64).max(1_000);

    let path = std::env::temp_dir().join(format!(
        "prc-bench-indexed-{}-{n}.redb",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let db = Database::create(&path).expect("create bench store");

    // A 96-byte payload, in the range a COBOL record actually occupies.
    let payload = vec![b'X'; 96];

    let write_row = measure("indexed redb (bulk insert)", "record", || {
        let w = db.begin_write().expect("begin write");
        {
            let mut t = w.open_table(PRIMARY).expect("open table");
            for i in 0..n {
                let k = i.to_be_bytes();
                t.insert(k.as_slice(), payload.as_slice()).expect("insert");
            }
        }
        w.commit().expect("commit");
        n
    });

    // Deterministic pseudo-random probe order: a multiplicative step coprime
    // with `n` visits every key exactly once with no clustering, so the read
    // pattern is reproducible run to run — a benchmark that reshuffles itself
    // cannot be compared against yesterday's number.
    let read_row = measure("indexed redb (random read)", "read", || {
        let r = db.begin_read().expect("begin read");
        let t = r.open_table(PRIMARY).expect("open table");
        let step: u64 = 2_654_435_761;
        let mut hits = 0u64;
        for i in 0..reads {
            let key = (i.wrapping_mul(step)) % n;
            let k = key.to_be_bytes();
            if let Some(v) = t.get(k.as_slice()).expect("get") {
                hits += v.value().len() as u64 & 1;
            }
        }
        std::hint::black_box(hits);
        reads
    });

    drop(db);
    let _ = std::fs::remove_file(&path);
    (write_row, read_row)
}

// ── Reporting ─────────────────────────────────────────────────────────────────

fn mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn report(rows: &[Row]) {
    println!();
    println!("| Workload | Ops | Wall | Ops/sec | Allocs | Allocs/op | MB churned | Peak live MB |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|");
    for r in rows {
        println!(
            "| {} | {} {} | {:.3}s | {:.0} | {} | {:.2} | {:.1} | {:.1} |",
            r.name,
            r.ops,
            r.unit,
            r.wall.as_secs_f64(),
            r.per_sec(),
            r.counters.allocations,
            r.allocs_per_op(),
            mb(r.counters.bytes),
            mb(r.counters.peak_live as u64),
        );
    }
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let filter = args.first().cloned();
    let scale: f64 = std::env::var("PRC_BENCH_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|v: &f64| *v > 0.0)
        .unwrap_or(1.0);

    let wanted = |name: &str| match &filter {
        Some(f) => name.contains(f.as_str()),
        None => true,
    };

    println!("PowerRustCOBOL benchmark — scale {scale}");
    println!(
        "profile: {}",
        if cfg!(debug_assertions) {
            "DEBUG (numbers are not meaningful — rerun with --release)"
        } else {
            "release"
        }
    );

    let mut rows = Vec::new();
    if wanted("dispatch") {
        rows.push(bench_dispatch(scale));
        rows.push(bench_paragraph_calls(scale));
    }
    if wanted("decimal") {
        rows.push(bench_decimal_math(scale));
    }
    if wanted("record") || wanted("batch") {
        rows.push(bench_record_batch(scale));
    }
    if wanted("object") {
        rows.push(bench_object_churn(scale));
    }
    if wanted("indexed") {
        let (w, r) = bench_indexed_redb(scale);
        rows.push(w);
        rows.push(r);
    }

    if rows.is_empty() {
        eprintln!("no workload matched {filter:?}");
        std::process::exit(1);
    }
    report(&rows);
}
