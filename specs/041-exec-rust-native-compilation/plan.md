<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — EXEC RUST compiled into the program binary

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-08-04

## 1. Approach

The spec was written assuming this feature needs a new compilation pipeline —
invoking `rustc`, producing a `cdylib`, loading it, crossing an `extern "C"`
boundary. **Exploring the code shows almost none of that is necessary.**

`build_core` already generates a Cargo project and shells out to
`cargo build --release` ([`cobolt-compiler/src/lib.rs:572-600`](../../crates/cobolt-compiler/src/lib.rs)):
a generated `Cargo.toml` depending on `cobolt-runtime`/`cobolt-forms`/`eframe`, a
generated `src/main.rs` that embeds forms and assets via `include_bytes!`, and a
`cargo build` step whose failure is already `CompilerError::Cargo` (`lib.rs:99`).
The produced binary embeds the COBOL program and runs
`cobolt_runtime::Interpreter` over it (`lib.rs:1204`).

So a compiled PowerRustCOBOL application **is already a generated Rust crate**, and
a Rust toolchain is **already required to build one**. This feature is therefore
not "add a Rust compiler" — it is "**emit one more generated module into the crate
that is already being compiled**".

The shape:

1. **Codegen (R1, R19-R22).** Emit one generated `src/exec_rust_blocks.rs` holding
   both block kinds. **Item-level** blocks (R19) are emitted verbatim at *module
   scope*, so their `struct`/`impl`/`trait`/`use` are visible program-wide (R20).
   **Statement-level** blocks become one function each, with a preamble binding the
   block's `referenced_data` items as typed references and the developer's Rust as
   the body. Module scope is what makes developer-defined types bindable (R22) and
   turns the 48 classes into a floor (revised R8).
2. **Registration (R2, R9).** Generated `main.rs` builds a dispatch table
   (`block_id → fn`) and hands it to the `Interpreter` before `run()`. Blocks share
   one context object, giving R9 for free.
3. **Dispatch (R11).** `Stmt::ExecRust` in the interpreter stops interpreting and
   calls the registered function. The interpreted micro-language and its
   silent-ignore branch are **deleted**, not bypassed.
4. **Containment (R12, R13, R23-R25).** Each call is wrapped in `catch_unwind`; an
   `Err` becomes a new `RuntimeError::RustPanic { message }`, caught **only** by the
   new `CATCH RUST-EXCEPTION <name>` clause (R24). This is a language addition, not
   a convention: a `RUST-EXCEPTION` keyword in the lexer, a catch-kind on
   `Stmt::TryCatch`, and a runtime arm beside the existing `UserException` one at
   `interpreter.rs:2334`. `DISPLAY <name>` prints the payload and location in plain
   text — no substring inspection (R23).
5. **Diagnostics (R10).** Generated code carries `#[line]`-style provenance so
   `cargo`'s errors can be mapped back to the developer's `EXEC RUST` span.

Requirements satisfied by **existing** behaviour, needing verification rather than
construction: **R3** (built binaries already need no toolchain), **R14** (missing
cargo already fails the build), **R15** (cargo's incremental build already caches;
no bespoke hash cache), **R17/R18** (the build is already host-only — no
cross-compilation exists to remove).

## 2. Affected crates / files

- `crates/cobolt-runtime/src/exec_rust.rs` — **replaced**. The interpreter
  (`execute`, `exec_stmt`, `eval_expr`, `apply_compound`) is deleted; the module
  becomes the dispatch table, the block context, and the `catch_unwind` wrapper.
- `crates/cobolt-runtime/src/interpreter.rs` — `Stmt::ExecRust` arm (line ~2320)
  dispatches to the registered block instead of interpreting; new
  `Interpreter::register_exec_rust_blocks(...)`.
- `crates/cobolt-runtime/src/rust_bridge.rs` — expose typed `downcast_mut::<T>()`
  access by handle for the generated preamble. The 9 curated shims stay for
  `INVOKE`/`::`; compiled blocks bypass them and use the real `std` types (R7).
- `crates/cobolt-lexer/src/keywords.rs`, `token.rs` — new `RUST-EXCEPTION` keyword
  (R23).
- `crates/cobolt-ast/src/stmt.rs` — a catch-kind on `Stmt::TryCatch` (R23/R24); a
  new item-level `EXEC RUST` node, since `Stmt::ExecRust` is procedure-division-only
  today (R19).
- `crates/cobolt-parser/src/stmt.rs` (+ the division parser) — parse
  `CATCH RUST-EXCEPTION`, and accept an item-level block in the
  `CONFIGURATION SECTION` after `REPOSITORY` (R19, R21).
- `crates/cobolt-semantic/src/lib.rs` (+ `resolver.rs`) — enforce R5 (only
  `USAGE OBJECT REFERENCE RUST-*` items may be referenced), R6 (`snake_case` bound
  names), **revised R8** (a `CLASS` must name a `std` type *or* a type defined by an
  item-level block — so class resolution now depends on item-block contents), R16
  (no external crates), R21 (item/statement placement), R22 (bind
  developer-defined types).
- `crates/cobolt-compiler/src/lib.rs` — emit `src/exec_rust_blocks.rs`; extend
  `generate_main_rs` to register the table; map cargo diagnostics back to COBOL
  spans (R10); the KB documentation tables (`rustcobol_extensions.md` generator at
  line ~1638) must describe the new `EXEC RUST` contract.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` keys ×6 for any Output-panel message
  (missing toolchain, block compile failure, Run-requires-build notice).
- `docs/developers-guide-en.md` — new `EXEC RUST` section (English only).
- `CHANGELOG.md` — feature entry.

## 3. Data / model changes

- **New:** `ExecRustBlockId` (stable per program, assigned at codegen) and a
  `ExecRustContext` handed to each block — holds the object registry reference and
  the shared cross-block state (R9).
- **`Stmt::ExecRust`** gains a resolved block id alongside the existing `source`,
  `referenced_data` and `span`. `referenced_data` is already populated by
  `cobolt-semantic`; no analysis work is added.
- **No on-disk format changes.** `.cfrm` untouched; `cobolt.toml` untouched.
- **Compatibility:** any existing program relying on the interpreted
  micro-language changes behaviour — by design (R11). Such a program was either
  doing arithmetic that still compiles as Rust, or silently doing nothing. Both are
  covered by AC5.

## 4. Key decisions & alternatives

- **Decision: emit into the crate cargo already builds.** — *Why:* the toolchain,
  the incremental cache, the link step and the "no toolchain for end users"
  property all exist already; this is one more generated file. — *Rejected:*
  invoking `rustc` directly to build a `cdylib` and loading it (the spec's original
  framing, and the module's own "future" note). It would add per-platform dynamic
  loading, artefact caching, version-skew risk and a second compilation path, for
  no benefit — and no `dlopen`/`libloading` exists in the workspace today.
- **Decision: no `extern "C"`, no C ABI.** — *Why:* generated blocks are compiled
  into the same binary as the runtime, so calls are ordinary Rust. Repr/ABI
  stability is irrelevant. — *Rejected:* the spec's `extern "C"` boundary (R12),
  which is only needed across a dynamic-library edge that no longer exists.
  **`catch_unwind` alone satisfies R12/R13.** *Spec R12/R13 should be reworded.*
- **Decision (T8, revises §1.4): containment lives *inside* the generated
  function, not only at the call site.** — *Why:* binding is take-and-put-back,
  so a block holds its objects as locals while it runs. With `catch_unwind` only
  around the call, a panic unwinds past those locals and drops them, leaving live
  COBOL handles pointing at nothing. The generated function wraps the body,
  **puts every value back on both paths**, then `resume_unwind`s — so the
  runtime's `contain()` still turns it into `RustPanic` and `CATCH
  RUST-EXCEPTION` is unchanged. — *Rejected:* cloning each object (silently
  discards the block's mutations on the panic path) and a drop guard (needs the
  bridge borrowed while the body holds the values, which is the aliasing this
  design exists to avoid).
- **Decision (T8): a block body is a `Result`-returning function body.** —
  *Why:* AC2 requires `?` inside a block, which needs an enclosing `Result`
  return. An error that reaches the end becomes a panic, so it arrives as a
  `RUST-EXCEPTION` like every other failure in a block — one door, not two. —
  *Consequence to document:* an early exit is `return Ok(())`, not `return;`.
- **Decision: bind objects by typed `downcast_mut`, not by value marshalling.** —
  *Why:* the registry already stores genuine `std` values as `Box<dyn Any>`; the
  preamble can hand the block a real `&mut String`/`&mut Vec<_>`. This is what makes
  all 48 types free (R7). — *Rejected:* a `BridgeValue`-shaped accessor vtable,
  which would re-impose the dynamic typing this feature exists to escape.
- **Decision (resolves spec Q3): a program containing `EXEC RUST` is built before
  it runs; the IDE's *Run* performs that build automatically.** — *Why:* the spec
  forbids dual semantics, and the in-process tree-walking path cannot compile Rust.
  Auto-building keeps one meaning for `EXEC RUST` and confines the cost to programs
  that use it; programs without a block keep the fast interpreter path unchanged.
  — *Rejected:* (a) making `EXEC RUST` unavailable under *Run* — a worse edit-run
  loop and a second behaviour; (b) interpreting under *Run* and compiling under
  *Build* — precisely the dual semantics the spec rules out.
- **Correction to an earlier reading (spec Q7): event handlers do NOT breach the
  regenerate contract.** The plan first assumed developer Rust in a generated `.cbl`
  would conflict with `tech.md`. It does not: a handler's body lives in
  `EventBinding.code` in the `.cfrm`, and `cobolt-codegen` emits it into the `.cbl`,
  which is "a build artifact — never edited by hand" (`cobolt-codegen/src/lib.rs:12`).
  The developer authors in the designer; the `.cbl` stays fully derived. So
  `EXEC RUST` is allowed in handlers, lifecycle handlers and user procedures, and
  **no rejection rule is needed for generated `.cbl`** — nothing is authored there.
- **Decision: item-level blocks live in the `CONFIGURATION SECTION` after
  `REPOSITORY`.** — *Why:* that is already where Rust types are declared
  (`CLASS RUST-*`), so all Rust-type declarations sit together, and the
  `DATA DIVISION` stays about data. — *Rejected:* a slot in `WORKING-STORAGE`
  (mixes types with data) and a free-floating position (no obvious ordering rule,
  and item order matters for readability far more than for `rustc`).
- **Decision (resolves spec Q2): the shared context lives for the process (one
  run-unit).** — *Why:* the binary is one process running one `Interpreter`. —
  *Consequence to document:* `CANCEL` does **not** reset it in v1; a second `CALL`
  of the same program sees prior state. Flagged in the guide.

## 5. Risks & mitigations

- **Risk: diagnostic mapping is the hard part (R10).** Cargo reports errors against
  generated line numbers; a developer must never see them. → Emit
  `#[cfg_attr]`/`// line` provenance and a span table, and translate cargo's JSON
  diagnostics (`--message-format=json`) before surfacing. Budget real effort here —
  AC4 is the acceptance gate.
- **Risk (raised by resolving Q7, now spec Q8 — the biggest open cost): allowing
  blocks in event handlers puts the cargo build on the core RAD loop.** One Rust
  block anywhere in a form means every *Run* is a build, and a first build of the
  generated crate pulls in `eframe` and the runtime. The earlier claim that
  "programs without `EXEC RUST` keep the fast path" is much weaker once handlers
  qualify, because handlers are where form logic lives. → Mitigations: cargo's
  incremental cache; build only when a block is actually present; report build
  progress in the Output panel so the pause is explained rather than mysterious.
  **ACCEPTED by the operator 2026-08-04 (spec Q8):** the compile-time cost is
  acceptable; handlers keep their blocks. Note the cost is compile-time only —
  execution does not regress, since the built binary interprets the COBOL as it
  does today and the Rust blocks become native code rather than the removed
  micro-interpreter.
- **Risk: arbitrary developer Rust in a generated crate can do anything** — file
  I/O, network, `unsafe`. → This is inherent to the feature and matches what a
  compiled application can already do. Document it; no sandbox in v1.
- **Risk: deleting the interpreter is a breaking change.** → Intended (R11);
  covered by AC5 and called out in the CHANGELOG.
- **Risk: `Box<dyn Any>` downcast to the wrong type panics.** → The `REPOSITORY`
  class fixes the static type; semantic analysis (R5/R8) guarantees the pairing,
  and `catch_unwind` contains any residual mismatch.
- **Risk: 48 types × real usage is a large test surface (AC8).** → One compact
  table-driven test declaring an item of each class and touching one method.

## 6. Test strategy

- **`cobolt-semantic`** — rejection tests for R5 (`PIC` item referenced), R6
  (`WS-USER-NAME` not `snake_case`), R8 (`Rust.Nope`), R16 (`use serde::…`). Each
  asserts the diagnostic *names the offending item*, and reports the message.
- **`cobolt-runtime`** — dispatch, shared context across two blocks (AC10), and
  panic containment reaching `TRY … CATCH` (AC11). Extend the existing
  `tests/test_rust_ffi.rs` rather than adding a parallel harness.
- **`cobolt-compiler`** — the generated `exec_rust_blocks.rs` compiles; a
  deliberate type error fails the build with the developer's span (AC4); an
  unrecognised statement fails rather than being skipped (**AC5 — the regression
  guard for the reported defect**); cargo is invoked once for an unchanged block on
  rebuild (AC13, reported as a counted measurement).
- **48-type coverage (AC8)** — one table-driven test, one class per row.
- **Manual / measured:** build an app containing a block, run it on a machine with
  no Rust toolchain (AC3) — this must be *observed*, never asserted from
  reasoning. Launch the IDE, press *Run* on a program with a block, confirm the
  auto-build path and that the Output panel shows localised progress.

## 7. Steering compliance

- [ ] i18n: all new UI strings in 6 languages (`i18n.rs`) — Output-panel messages
      for build failure, missing toolchain, Run-requires-build.
- [ ] Generated-code banner + regenerate-on-action contract preserved — the new
      `exec_rust_blocks.rs` is a build artefact, carries a generated-by banner, is
      never hand-edited, and is regenerated every build.
- [ ] English dev guide updated (`docs/developers-guide-en.md`); translations
      untouched.
- [x] Fix vs feature: **feature** → `features` branch. **Version bump deferred to
      the operator** (spec Q4: `tech.md` says minor, the operator's standing rule
      is that only they raise `x`/`y`). CHANGELOG entry required.
- [ ] No "cobolt" in user-facing text; COBOL identifiers and generated Rust stay
      English.
- [ ] System KB: `cobolt-compiler` documentation tables updated in the same change
      (spec Q5 — confirm whether `build_chunked_kb` reindex is expected now that
      the operator lifted the suspension on 2026-07-31).
