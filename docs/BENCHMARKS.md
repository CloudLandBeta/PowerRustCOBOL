<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Benchmarks

The 1.37.0 baseline: how fast the runtime is under load, and how hard it leans
on the allocator getting there.

```sh
cargo run --release -p cobolt-bench              # everything
cargo run --release -p cobolt-bench -- dispatch  # one workload, by substring
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # a twentieth, for a quick check
```

`--release` is not optional. A debug build measures the absence of
optimisation, and the harness says so in its header rather than letting the
numbers be quoted.

## What is measured

Every COBOL workload runs the **same path a shipped binary takes** — tokenize,
parse, analyse, `Interpreter::run` — because that is what `rcrun build`'s
generated `main.rs` does with its embedded AST. Running in-process is what makes
the allocator counters possible: the numbers describe the interpreter that is
inside every binary you hand over.

Memory is reported as allocation behaviour rather than as a resident-set curve.
Rust has no garbage collector, so there are no pauses to measure; what matters
under load is **churn** — how many times a workload enters the allocator, how
many bytes pass through, and how much is live at the peak. A counting global
allocator ([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs))
supplies all three exactly, on all three platforms, with no external profiler.

Two things this deliberately does **not** measure: process startup and binary
size. Measure those on the real artifact from `rcrun build`.

## The 1.37.0 baseline

Apple M3 Pro, 18 GB, macOS 15.5, rustc 1.95.0, release profile, 2026-07-27.
Absolute numbers travel badly between machines; **allocations per op** travels
fine and is the column to watch.

| Workload | Ops | Wall | Ops/sec | Allocs | Allocs/op | MB churned | Peak live MB |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 stmt | 1.049s | 5 721 961 | 24 000 334 | 4.00 | 72.5 | 0.0 |
| dispatch (PERFORM paragraph) | 500 000 call | 0.729s | 686 318 | 9 000 356 | 18.00 | 409.6 | 0.0 |
| decimal COMPUTE | 500 000 compute | 0.824s | 606 461 | 10 000 499 | 20.00 | 41.0 | 0.0 |
| record batch (1000 rows, write+read) | 400 000 record | 2.179s | 183 612 | 26 023 007 | 65.06 | 227.9 | 0.8 |
| object churn (create/read/destroy) | 20 000 object | 0.092s | 216 320 | 1 100 000 | 55.00 | 27.5 | 0.0 |
| indexed redb (bulk insert) | 100 000 record | 0.710s | 140 922 | 65 854 | 0.66 | 188.9 | 22.4 |
| indexed redb (random read) | 50 000 read | 0.034s | 1 489 965 | 9 | 0.00 | 0.0 | 22.4 |

## What the baseline says

**The allocator is the bottleneck, not the tree-walk.** 5.7 M statements/sec is
a respectable dispatch rate — but reaching it took **24 million allocations for
6 million statements**. `ADD 1 TO ACC` on two `COMP` fields, which should touch
no heap at all, costs four trips through the allocator. That reframes the
optimization work: the first wins are in the value system and the operand path,
not in replacing the tree-walking interpreter with a bytecode VM. A VM would
make dispatch cheaper while leaving four allocations per statement untouched.

**Paragraph calls are expensive out of proportion.** 18 allocations and ~820
bytes per `PERFORM <paragraph>`, against 4 per inline statement. Half a million
calls churn 410 MB. Whatever the call path is building per invocation is the
highest-density target in the table.

**Alphanumeric records allocate per field, as expected.** 65 allocations per
record for a 4-field row read and written is `CobolValue::String` owning a
`Vec<u8>` per field, plus a fresh one for every `MOVE`. An inline
small-string representation, or slicing into the record's own buffer, would
show up here immediately.

**Object property reads allocate for no reason.** 55 allocations per object
across 24 property reads. `CoboltObject::get_property`, `get_str`, `get_bool`
and `get_i64` each call `name.to_ascii_uppercase()` — one `String` allocated and
dropped **per read**, purely to make the lookup case-insensitive. A
case-insensitive key wrapper removes the whole column.

**The INDEXED engine is not the problem.** redb inserts at 141 k records/sec with
0.66 allocations per record and serves 1.5 M random reads/sec with essentially
zero allocation. Storage is comfortably ahead of the interpreter feeding it.

Ranked by expected return, the optimization order the baseline suggests is:
the per-statement allocations, then the paragraph-call path, then `CobolValue`
for alphanumerics, then the object-property upper-casing. Storage does not
appear until well below those.

## Workloads

| Workload | What it isolates |
|---|---|
| `dispatch (PERFORM VARYING)` | Tree-walk overhead: loop test, increment, one statement, minimal work underneath |
| `dispatch (PERFORM paragraph)` | Paragraph-call overhead, against the inline case above |
| `decimal COMPUTE` | `CobolNumeric`'s i128-scaled arithmetic — COBOL money math |
| `record batch` | 1000-row table written and read back with alphanumeric fields; the value system under batch load |
| `object churn` | `ObjectRegistry` create/read/destroy — what a form with many controls costs |
| `indexed redb` | The INDEXED file engine: bulk insert, then random-key reads |

The two `indexed redb` rows are a recovered and generalised version of the
`open_table_cost` micro-benchmark that lived `#[ignore]`d inside
`cobolt-runtime::indexed_redb`. It only ran when someone remembered an exact
`--ignored` invocation, so the engine had no standing baseline; it now has one.
Its original conclusion is kept — the table handle is opened once for the whole
write transaction, which measured ~16 % faster than opening it twice per insert.

## Adding a workload

Add a `bench_*` function to
[`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) that
returns `measure(name, unit, || { ...; ops_performed })`, and register it in
`main` behind a `wanted(...)` filter. The counters wrap the closure
automatically. Return the number of units of *work*, not iterations, so
`ops/sec` and `allocs/op` stay comparable across workloads.

Keep new workloads deterministic. The random-read probe uses a fixed
multiplicative step rather than a random number generator for exactly this
reason: a benchmark that reshuffles itself between runs cannot be compared with
yesterday's number.
