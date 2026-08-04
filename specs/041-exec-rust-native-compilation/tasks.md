<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — EXEC RUST compiled into the program binary

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-08-04

Ordered, small, independently-verifiable tasks. T1–T5 are additive and leave the
tree green. **T6 is the breaking change** (the interpreter is deleted); from there
until T9 the feature is mid-migration by design — do not stop between them without
saying so.

---

## Phase A — language surface (additive, tree stays green)

- [x] **T1 — `RUST-EXCEPTION` keyword** (R23) — *done: `cargo test -p cobolt-lexer`
      20 passed in `test_keywords.rs` (was 18), 0 failed.*
  - Files: `crates/cobolt-lexer/src/keywords.rs`, `token.rs`
  - Do: add `Token::RustException` for `"RUST-EXCEPTION"`, beside the existing
    `Try`/`Catch`/`EndTry` entries (`keywords.rs:293`).
  - Verify: `cargo test -p cobolt-lexer` green; a lexer test asserts the keyword
    tokenises and that `RUST-EXCEPTION` in a comment or literal does not.

- [x] **T2 — AST: second catch clause + item-level block** (R19, R23) — *done:
      workspace builds; `cargo test -p cobolt-ast -p cobolt-parser` green.*
  - Files: `crates/cobolt-ast/src/stmt.rs`, `program.rs`,
    `crates/cobolt-parser/src/{parser.rs,stmt.rs}` (default-init),
    `crates/cobolt-ast/tests/test_ast_construction.rs`
  - **Shape changed from the task's wording, deliberately:** not a catch-*kind*
    enum (`UserException | RustException`) but a **second clause pair**
    (`rust_exception_var` + `rust_catch_stmts`) beside the existing one. R24 lets a
    `TRY` carry *both* clauses, which a single kind cannot express. Same outcome,
    correct shape.
  - Item-level blocks are `Program::rust_items: Vec<RustItemBlock>`, beside
    `repository` — both declare what Rust types a program has.
  - Verify: `cargo build --workspace` clean; `cargo test -p cobolt-ast
    -p cobolt-parser` green (23 + parser suites).

- [x] **T3 — Parse `CATCH RUST-EXCEPTION`** (R23, R24) — *done:
      `cargo test -p cobolt-parser --test test_statements` 42 passed (was 37),
      0 failed. Clause loop accepts either order; a repeat of either clause
      emits "duplicate CATCH … clause".*
  - Files: `crates/cobolt-parser/src/stmt.rs`
  - Do: parse the new clause; allow **both** clauses on one `TRY`, in either order.
  - Verify: `cargo test -p cobolt-parser` green; new tests parse (a) each clause
    alone, (b) both together, (c) a duplicate clause → diagnostic.

- [x] **T4 — Parse item-level `EXEC RUST`** (R19, partial R21) — *done:
      `cargo test -p cobolt-parser` green, `test_exec_rust.rs` 8 → 10 passed.*
  - Files: `crates/cobolt-parser/src/parser.rs` (config-section arm, `rust_items`
    state), `data.rs` (placement rejection),
    `crates/cobolt-parser/tests/test_exec_rust.rs`
  - Done: item-level blocks captured into `Program::rust_items`; `EXEC RUST` in
    the `DATA DIVISION` is a hard error naming both legal homes.
  - **R21 is only half implemented, deliberately.** *Placement* is enforced here.
    The other half — "reject an item-level block that **contains statements**" —
    is **not**, because deciding it means parsing Rust, and a keyword-sniffing
    approximation would produce confident wrong answers on valid code. `rustc`
    already rejects a statement at module scope precisely, so this is left to it
    and surfaces through the T10 diagnostic mapping. **AC21 is therefore only
    partly covered here; its statement-in-item half moves to T10.**
  - Note: a statement-level block outside `PROCEDURE DIVISION` is now
    structurally unreachable — position alone decides which kind a block is — so
    that clause of R21 needs no runtime check.

- [x] **T5 — Runtime: `RustPanic` + catch routing** (R12, R13, R23, R24, R25) —
      *done: `cargo test -p cobolt-runtime` 104 passed, 0 failed; full workspace
      sweep **1509 passed, 0 failed, 8 ignored**.*
  - Files: `crates/cobolt-runtime/src/error.rs` (`RustPanic` variant),
    `exec_rust.rs` (`contain()` + tests), `interpreter.rs` (`TryCatch` arm)
  - Done: `RustPanic` added and routed **only** to a `CATCH RUST-EXCEPTION`
    clause; `UserException` routing untouched; `FINALLY` runs on both paths; a
    panic with no `RUST-EXCEPTION` clause propagates after `FINALLY` (R25).
    `contain()` wraps a block call in `catch_unwind`, keeping payload text for
    `DISPLAY` and reporting a non-string payload rather than an empty message.
  - **AC11/AC17/AC18 are NOT verified end-to-end yet — deferred to T6.** They
    need a *reachable* panic, and no statement can raise one until blocks are
    compiled and dispatched. What is proven here: a panic becomes `RustPanic`,
    payload text survives, and a panic is **not** a `UserException` (the
    difference R24 depends on). The COBOL-level routing tests land in T6.

## Phase B — the breaking change

- [ ] **T6 — Delete the interpreted micro-language** (R11)
  - Files: `crates/cobolt-runtime/src/exec_rust.rs`, `interpreter.rs` (~2320)
  - Do: remove `execute`/`exec_stmt`/`eval_expr`/`apply_compound` and the
    silent-ignore branch. Replace with a dispatch table (`block_id → fn`) plus a
    `catch_unwind` wrapper producing `RuntimeError::RustPanic`. An unregistered
    block id is a hard error, never a no-op.
  - **Report-or-fix:** any existing test asserting interpreted-mode behaviour must
    be listed and either updated or retired *with justification stated in the
    report* — never silently deleted.
  - Verify: `cargo test -p cobolt-runtime`; **covers AC5** (a block whose body is
    not compiled-and-registered fails loudly rather than succeeding silently).

- [ ] **T7 — Semantic rules** (R5, R6, R8-revised, R16, R21, R22)
  - Files: `crates/cobolt-semantic/src/lib.rs`, `resolver.rs`
  - Do: reject a referenced item that is not `USAGE OBJECT REFERENCE RUST-*` (R5);
    reject a bound name that is not `snake_case` (R6); resolve a `CLASS` against
    `std` types **or** item-level definitions (R8-revised, R22); reject `use` of a
    non-`std` crate (R16). Every diagnostic names the offending item.
  - Verify: `cargo test -p cobolt-semantic`; **covers AC6, AC7, AC9, AC14**. Each
    test asserts the message *names* the item, not merely that it failed.

- [ ] **T8 — Codegen: emit `exec_rust_blocks.rs`** (R1, R4, R19, R20)
  - Files: `crates/cobolt-compiler/src/lib.rs` (near `generate_main_rs`, ~939)
  - Do: emit item-level blocks verbatim at **module scope**; emit each
    statement-level block as one function with a preamble binding
    `referenced_data` by `downcast_mut::<T>()`. Carry the generated-by banner.
  - Verify: `cargo test -p cobolt-compiler` green; a test asserts the emitted file
    places items at module scope and functions below them.

- [ ] **T9 — Register the table in `main.rs`** (R2, R9)
  - Files: `crates/cobolt-compiler/src/lib.rs` (`generate_main_rs`)
  - Do: build the dispatch table and hand it to the `Interpreter` before `run()`;
    one shared context for the process.
  - Verify: `cargo test -p cobolt-compiler`; end-to-end test builds a project whose
    block mutates a bound `Rust.String` and asserts COBOL sees it (**AC1**), a
    block using closures/generics/iterators/`match`/`?` compiles and runs
    (**AC2**), and two blocks share state (**AC10**).

## Phase C — diagnostics, reach, IDE

- [ ] **T10 — Map `rustc` diagnostics to COBOL spans** (R10)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the `cargo build` invocation, ~600)
  - Do: run cargo with `--message-format=json`; translate each diagnostic's
    generated-file line/column back through a span table to the developer's
    `EXEC RUST` source. **The plan calls this the hard part — budget for it.**
  - Verify: `cargo test -p cobolt-compiler`; **covers AC4** — a deliberate type
    error reports the developer's line/column, asserted as exact numbers.

- [ ] **T11 — Toolchain + target failures** (R14, R16, R18)
  - Files: `crates/cobolt-compiler/src/lib.rs`
  - Do: explicit diagnostic when cargo/rustc is unusable (no silent fallback);
    reject a non-host target telling the developer to build on that OS.
  - Verify: `cargo test -p cobolt-compiler`; **covers AC12, AC16**.

- [ ] **T12 — Type-coverage suite** (R7, R22)
  - Files: `crates/cobolt-runtime/tests/test_rust_ffi.rs` (extend; do not fork)
  - Do: one table-driven test, one row per shipped `CLASS RUST-*`, declaring an
    item of that class and touching one method; plus a developer-defined
    `struct Point` + `impl` used from another paragraph and from a form handler.
  - Verify: `cargo test -p cobolt-runtime`; **covers AC8, AC19, AC20**. Report the
    count of classes exercised as a number.

- [ ] **T13 — IDE: *Run* builds when a block is present** (plan §4 / spec Q3, Q8)
  - Files: `crates/cobolt-ide/src/app.rs` (Run action), Output panel
  - Do: if the program contains any `EXEC RUST`, build before running and execute
    the built binary; otherwise keep today's interpreter path. Stream build
    progress to the Output panel so the pause is explained.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-ide`; manual: press
    *Run* on a form with a block and watch progress, then on one without and
    confirm the fast path is unchanged.

- [ ] **T14 — Rebuild economy** (R15)
  - Files: — (measurement only)
  - Do: confirm cargo's incremental cache means an unchanged block does not
    recompile.
  - Verify: **AC13** — build twice, report the measured cargo invocation count or
    elapsed seconds for each. **Verify-first: report the number the run produced,
    never an estimate.**

## Phase D — docs, i18n, finalize

- [ ] **T15 — Docs & i18n**
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`,
    `crates/cobolt-compiler/src/lib.rs` (KB tables ~1638)
  - Do: guide section — the two block kinds and where each may appear, the
    `USAGE OBJECT REFERENCE` restriction, `snake_case` names, shared context and
    that `CANCEL` does not reset it, `CATCH RUST-EXCEPTION`, `std`-only, host-only
    builds, and that *Run* builds. Add `Tr` keys ×6 for every new Output-panel
    string. Update the System KB documentation tables (same change, per `tech.md`).
    **Translations of the guide are user-maintained — do not touch.**
  - Verify: `cargo test -p cobolt-ide i18n` (no empty translations); guide renders
    in the doc viewer.

- [ ] **T16 — Finalize**
  - Do: full `cargo test --workspace --no-fail-fast`, collecting **every**
    `test result` line; CHANGELOG entry; confirm all 21 acceptance criteria.
    **Version bump is the operator's call (spec Q4) — do not bump the minor
    unprompted.** Raise spec Q5 (`tech.md` still records the KB reindex as
    suspended) for a decision.
  - Verify: full sweep green with totals reported; **AC3** — build an app with a
    block and run it where no Rust toolchain exists, and **AC15** — confirm the
    host build works. Both are *observed*, never inferred.

## Done criteria

All 21 acceptance criteria in `spec.md` are checked, the full sweep is green with
reported totals, docs and the System KB are updated, and the change is a **feature**
commit on the `features` branch — never mixed with fixes, and not committed or
pushed unless the operator asks.

### Acceptance-criteria coverage map

| AC | Task | AC | Task |
|----|------|----|------|
| AC1 | T9 | AC12 | T11 |
| AC2 | T9 | AC13 | T14 |
| AC3 | T16 | AC14 | T7 |
| AC4 | T10 | AC15 | T16 |
| AC5 | T6 | AC16 | T11 |
| AC6 | T7 | AC17 | T5 |
| AC7 | T7 | AC18 | T5 |
| AC8 | T12 | AC19 | T12 |
| AC9 | T7 | AC20 | T12 |
| AC10 | T9 | AC21 | T4 (placement) + T10 (statement-in-item, via rustc) |
| AC11 | T6 (needs a reachable panic) | AC17 | T6 |
| AC18 | T6 | | |
