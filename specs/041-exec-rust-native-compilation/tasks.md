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

- [x] **T6 — Delete the interpreted micro-language** (R11) — *done: full workspace
      sweep **1516 passed, 0 failed, 8 ignored**.*
  - Files: `crates/cobolt-runtime/src/exec_rust.rs` (rewritten),
    `interpreter.rs` (registry field + dispatch), `crates/cobolt-ast/src/stmt.rs`
    (`block_id`), `crates/cobolt-parser/src/{parser.rs,stmt.rs}` (id assignment),
    `crates/cobolt-runtime/tests/test_exec_rust_catch.rs` (new)
  - Done: `interpret_statement`/`eval_expr`/`try_binary`/`numeric_op`/
    `apply_compound`/`cobol_name` and the silent-ignore branch are **gone**.
    Dispatch is `ExecRustRegistry` (`block_id → ExecRustFn`), populated by the
    generated `main.rs`; an unregistered id is a hard `ExecRustError` naming the
    id and telling the developer to build first.
  - **Report-or-fix outcome: nothing needed changing.** Survey of every test
    touching `EXEC RUST`: `cobolt-parser/tests/test_exec_rust.rs` is parse-only;
    `cobolt-semantic/tests/test_semantic.rs` (`exec_rust_bindings_resolved`,
    `exec_rust_no_spurious_partial_matches`) tests *binding resolution*, not
    execution. **No test anywhere asserted interpreted-mode behaviour** — the
    silent-ignore was never pinned by one, which is why it survived so long.
    *(Both semantic tests bind `PIC` items inside blocks and so become T7's
    concern under R5.)*
  - Verify: **AC5** — `an_unregistered_block_fails_loudly`. Plus **AC11, AC17,
    AC18** (deferred here from T5, now that a panic is reachable):
    `cargo test -p cobolt-runtime --test test_exec_rust_catch` **4 passed** —
    caught by `RUST-EXCEPTION` and execution continues; **not** caught by a
    plain `CATCH` and the run ends; both clauses route to their own class; a
    COBOL `THROW` never reaches the Rust clause.

- [x] **T7 — Semantic rules** (R5, R6, R8-revised, R16, R22) — *done:
      `cargo test -p cobolt-semantic --test test_semantic` **20 passed**, 0
      failed.*
  - **R16 done**, and its meaning corrected by the operator: not "`std`-only"
    but **`std` plus the crates every generated binary already links** — egui is
    the standard GUI for the IDE *and* the application, and
    `generate_cargo_toml` emits `eframe`/`egui_extras`/`cobolt-forms` into every
    build. `unlinked_crates()` inspects `use` declarations only (the clear
    signal); a bare `some_crate::f()` slips through to `rustc`, with a worse
    message but a correct verdict. **AC14 covered**
    (`exec_rust_rejects_an_unlinked_crate_but_allows_egui`).
  - **R5 done** — a referenced item that is not `USAGE OBJECT REFERENCE` is an
    error naming it (`exec_rust_rejects_a_pic_item`); a Rust-typed object binds
    cleanly (`exec_rust_accepts_an_object_reference_item`). **AC6 covered.**
  - **R6 done, but not as written.** AC7 said `WS-USER-NAME` must be rejected;
    that is impossible, because `cobol_to_rust` lowercases and swaps hyphens, so
    it becomes `ws_user_name` — already valid `snake_case`. Every ordinary COBOL
    name converts cleanly. What actually survives the conversion broken is a
    **Rust keyword** (`01 TYPE` → `type`) or a name that **cannot start an
    identifier** (`01 1ST-FLAG` → `1st_flag`), so `rust_name_problem()` catches
    those. Spec AC7 corrected with the reason recorded.
    **AC7 covered** (`exec_rust_rejects_a_name_that_is_a_rust_keyword`).
  - **Report-or-fix: resolved, with the operator's approval.**
    `exec_rust_bindings_resolved` and `exec_rust_no_spurious_partial_matches`
    bound `PIC` items and so described programs R5 now rejects — yet stayed
    green, because they only assert the `Info` binding list. Both now bind
    `USAGE OBJECT REFERENCE` items, keeping their real subject (whole-word
    binding resolution, no partial matches), and
    `exec_rust_bindings_resolved` additionally asserts **no R5 rejection**, so
    it can no longer pass while describing code that would not build.
  - **R8-revised done, R22 done.** The shipped type table moved from
    `cobolt-forms/src/model.rs` to `cobolt-ast/src/rust_types.rs` (operator chose
    option (a)), so `cobolt-forms` and `cobolt-semantic` cannot disagree about
    it; `cobolt-forms` gained a `cobolt-ast` dependency and `default_repository()`
    now reads the shared table. `check_repository_classes` rejects a `CLASS`
    naming neither a shipped type nor one declared by an item-level block, and
    names the developer's `CLASS` in the message. **AC9 covered**, plus
    `a_developer_defined_type_may_be_named_by_a_class` (the floor-not-ceiling
    case) and `a_shipped_class_is_accepted_on_its_own`.
    `types_declared_in_item_blocks` is a permissive lexical scan, not a Rust
    parser: a wrong accept costs a precise `rustc` error later, a wrong reject
    would refuse a valid program with a wrong reason.
  - Remaining: **R16** (AC14) — and its meaning changed, see the spec: not
    "`std`-only" but "`std` + the crates the binary already links", since
    `eframe`/`egui`/`egui_extras`/`cobolt-forms` are emitted into every generated
    `Cargo.toml`.
  - **⚠ Blocking question for R8-revised — where does the canonical Rust-type
    list live?** The 48 `CLASS RUST-*` entries are in
    `cobolt-forms/src/model.rs:4419`, but `cobolt-semantic` depends only on
    `cobolt-lexer` and `cobolt-ast` (`cobolt-forms` is a **dev**-dependency), so
    semantic analysis cannot see them. Options: **(a)** move the list to
    `cobolt-ast` as the shared home and have `cobolt-forms` reference it — one
    source of truth, but touches `cobolt-forms` and its tests; **(b)** duplicate
    it in `cobolt-semantic` — rejected, it will drift; **(c)** check only that a
    class is declared by an item-level block, and let `rustc` reject an unknown
    `std` type at T10 — cheapest, but the diagnostic names the generated type,
    not the developer's `CLASS`, so **AC9 would not be met as written**.
    Recommendation: **(a)**.
  - Files: `crates/cobolt-semantic/src/lib.rs`, `resolver.rs`
  - Do: reject a referenced item that is not `USAGE OBJECT REFERENCE RUST-*` (R5);
    reject a bound name that is not `snake_case` (R6); resolve a `CLASS` against
    `std` types **or** item-level definitions (R8-revised, R22); reject `use` of a
    non-`std` crate (R16). Every diagnostic names the offending item.
  - Verify: `cargo test -p cobolt-semantic`; **covers AC6, AC7, AC9, AC14**. Each
    test asserts the message *names* the item, not merely that it failed.

- [x] **T8 — Codegen: emit `exec_rust_blocks.rs`** (R1, R4, R19, R20) — *done:
      `cargo test -p cobolt-compiler --lib exec_rust` **6 passed**, 0 failed.*
  - **New module, not `lib.rs`:** the codegen lives in
    `crates/cobolt-compiler/src/exec_rust.rs`. The task said "near
    `generate_main_rs`"; `lib.rs` is already ~3 000 lines and this is a
    self-contained emitter with its own provenance bookkeeping.
  - **The T6 design error is fixed.** `ExecRustContext` carried
    `ObjectRegistry` (forms and controls) but not `RustBridge`, where bound
    objects actually live — so a generated block could see an item's *handle id*
    in `env` and nothing it pointed at. The context now carries all three.
  - **Binding is take-and-put-back**, and the emitted order matters: resolve
    ids → `check_binding` **every** item → take → run → put back → re-raise.
    Checking before taking is what stops a block that binds two objects from
    taking the first, failing on the second, and stranding the first slot.
  - **Containment moved inside the generated function** (plan §4 now records
    this). With `catch_unwind` only at the call site a panic would unwind past
    the taken values and drop them, leaving live COBOL handles pointing at
    nothing; the generated function puts everything back on both paths and then
    `resume_unwind`s, so the runtime's own containment still produces
    `RustPanic` and `CATCH RUST-EXCEPTION` is unaffected.
  - **A block body is a `Result`-returning function body**, which is what makes
    `?` usable at block level (AC2). An error reaching the end becomes a panic,
    so it arrives as a `RUST-EXCEPTION` like every other block failure.
  - **New: `cobolt_ast::rust_types::block_binding`** — what each shipped class
    binds as, and what it starts from. Every integer width binds as `i64` and
    both floats as `f64` because that is how the bridge *stores* them; binding
    `RUST-U8` as a real `u8` would make the next `INVOKE` on that item panic.
    A test asserts the two lists cannot drift.
  - **New: `RustBridge::create_uninitialised` / `check_binding` / `take_or_init`.**
    A `CLASS` naming a developer-defined type is not constructible by the curated
    bridge; such items used to get handle **0**, which silently aliased every one
    of them onto the same slot. They now get a real unique handle, and the first
    block to bind one initialises it (R22).
  - Also: `DataItemInfo` gained `object_class` — the only thing that says which
    Rust type a handle refers to.

- [x] **T9 — Register the table in `main.rs`** (R2, R9) — *done:
      `a_compiled_block_runs_mutates_and_shares_state` and
      `a_developer_defined_type_is_usable_across_paragraphs` **2 passed** (329 s
      and 16 s — the first build populates the shared target dir).*
  - `Interpreter::register_exec_rust_blocks` takes the generated module's
    `register` fn, so the generated crate never names `ExecRustRegistry`. Both
    run paths register: headless and the form-app interpreter thread.
  - **AC1, AC2, AC10 verified by execution, not by substring.** The test drives
    the real generators, runs `cargo build`, and executes the produced binary:
    a block mutates a bound `Rust.String` (`NAME=ada-lovelace` read back through
    `INVOKE`), a second block uses a closure, a generic `fn`, an iterator chain,
    `match` and `?` (`TWICE=12`), and reads the `Rust.Vec` the first filled
    (`TOTAL=60`, `COUNT=0003`).
  - **AC19 and the paragraph half of AC20** also verified here: a `struct Point`
    with an `impl` written in an item-level block, named by `CLASS MY-POINT`,
    mutated in one paragraph and read in another (`DIST=7`). The form-handler
    half of AC20 is T12's.

## Phase C — diagnostics, reach, IDE

- [x] **T10 — Map `rustc` diagnostics to COBOL spans** (R10) — *done:
      `cargo test -p cobolt-compiler --lib` mapping tests **3 passed**
      (`cargo_diagnostics_are_restated_in_cobol_coordinates`,
      `a_type_error_in_a_block_reports_the_developers_line_and_column`,
      `a_statement_in_an_item_level_block_is_rejected_at_its_own_line`).*
  - **The provenance is recorded while emitting, not reconstructed after.** The
    emitter writes developer source with *no added indentation*, so a generated
    column is already the developer's column and only the first line of a block
    needs an offset (the lexer trims what it captures). `GeneratedBlocks.line_map`
    carries the rest.
  - Cargo runs with `--message-format=json`: diagnostics on stdout, the human
    "Compiling …" lines still on stderr, so the progress bar is untouched. stdout
    is drained on its own thread — reading both pipes in sequence deadlocks as
    soon as one fills.
  - **A diagnostic that does not map is not dressed up.** Errors in `main.rs`, in
    a dependency, or on this codegen's own scaffolding fall through to the raw
    cargo output: blaming those on a COBOL line would point the developer at
    innocent code.
  - **AC4 asserted as exact numbers**, line *and* column, against a real build,
    and the message is checked for *not* mentioning `exec_rust_blocks`.
  - **AC21's second half lands here as T4 predicted**: `let stray = 1;` at module
    scope is rejected by `rustc` and reported at the developer's own line.

- [x] **T11 — Toolchain + target failures** (R14, R16, R18) — *done:
      `a_missing_toolchain_is_named`, `a_rustc_without_a_host_line_is_a_toolchain_error`,
      `a_non_host_target_is_refused` — **3 passed**.*
  - The toolchain is probed **before anything is staged**, so a missing `rustc`
    fails with its own diagnostic and leaves no half-built artefacts (AC12), and
    a cross-target request is refused before any work (AC16).
  - **Testable without editing `PATH`:** `probe_host_triple` takes the runner as
    an argument, so a test can hand it exactly the failure a missing `rustc`
    produces. Editing `PATH` would have been process-global and raced every other
    test in the binary.
  - `BuildOptions::target` exists so a cross-target request can be *refused
    clearly* rather than silently producing a host binary. The message names the
    requested triple, the host, and what to do instead.
  - R16 was already done in T7 (`unlinked_crates`), so nothing was needed here.

- [x] **T12 — Type-coverage suite** (R7, R22) — *done: `every_shipped_class_binds_inside_a_block`
      (compiler, built and run) and `every_shipped_class_gets_a_live_unique_handle`
      (runtime) — **AC8 exercises all 48 shipped classes**, reported by the test
      itself.*
  - **Split by where the claim actually lives.** AC8 says each class must be
    *used inside a block*, and a block is compiled — so the program that declares
    all 48, binds every one in a block and runs is a `cobolt-compiler` test. The
    runtime half extends `test_rust_ffi.rs` as instructed, asserting the thing the
    runtime owns: every declared item gets a **live, unique** handle.
  - The COBOL source is generated from `SHIPPED_RUST_TYPES`, so adding a class
    without a binding fails the test instead of shipping a class nobody can use.
    Two blocks touch every item — the second proves each value survived the
    first's put-back.
  - **A pre-existing parser bug had to be fixed to get here.** `repository` and
    `rust_items` were accumulated in parser state and moved out when a `Program`
    was built — but nested programs are parsed *before* the outer program is
    built, so the first nested program took the outer program's entries and the
    outer was left with none. A form's `CLASS RUST-STRING IS "Rust.String"`
    vanished the moment the form gained an event handler. Claimed before the
    nested loop now.
  - **AC20's event-handler half** is `a_block_inside_a_nested_program_runs`: a form
    event handler *is* a nested program, so the block walk now descends into them
    — a top-level-only walk dropped every handler block in silence, which is the
    defect class this spec exists to remove. A nested block also binds the
    containing program's `GLOBAL` items (and only those, as COBOL scoping says).

- [x] **T13 — IDE: *Run* builds when a block is present** (plan §4 / spec Q3, Q8) —
      *done: `cargo build -p cobolt-ide` clean; `exec_rust_run` **4 tests**.*
  - The decision is a tested, pure function (`crates/cobolt-ide/src/exec_rust_run.rs`)
    that asks the **lexer**, not a substring search: a program merely mentioning
    `EXEC RUST` in a comment or a literal would otherwise pay a cargo build on
    every *Run*, forever, for nothing.
  - *Run* on a program with a block builds first and starts the built binary;
    without one, the `rcrun run-form` fast path is untouched. Six `Tr` keys ×6
    languages carry the Output-panel messages.
  - **Debug is refused, explicitly.** The debugger steps interpreted COBOL over
    `@DBG` lines and a compiled block is native code with no such protocol; saying
    so beats starting a session that cannot step into the block.
  - **Not verified by me:** the on-screen behaviour. The IDE builds and the
    decision is unit-tested, but I do not drive the application — pressing *Run*
    on a form with a block, and on one without, is the operator's check.

- [x] **T14 — Rebuild economy** (R15) — *done, **measured**: build 1 compiled
      **237 crates in 26.4 s**; build 2 compiled **0 crates in 0.6 s**; after
      editing the block, build 3 compiled **1 crate**.*
  - **AC13 needed a fix, not just a measurement.** Every build rewrote
    `Cargo.toml`, `main.rs` and `exec_rust_blocks.rs` unconditionally, giving each
    a fresh mtime — and cargo's fingerprint is mtime-based, so an unchanged
    program recompiled its own crate every single time. `write_if_changed`
    compares first.
  - The measurement is a **count**, not a stopwatch: `BuildResult::crates_compiled`
    is cargo's own tally, and "compiled 0 crates" means the same thing on a fast
    machine and a slow one. The third build guards the opposite failure — a cache
    that never invalidates would pass the first assertion while being useless.
  - Also fixed here: the tests now remove their build staging **and hold a shared
    mutex while they build**. Each staged crate carries its own `target/`, so
    tests that built and walked away left about a gigabyte apiece behind, and
    four running at once needed several gigabytes at the same moment — enough to
    fill a disk, after which the `ENOSPC` surfaces as an unrelated failure
    somewhere else entirely. (Both happened on this machine mid-session.)

## Phase D — docs, i18n, finalize

- [x] **T15 — Docs & i18n** — *done: guide section added, 6 `Tr` keys ×6
      languages (`cargo test -p cobolt-ide --bins -- i18n` **3 passed**, including
      `no_empty_ui_translations` and `non_english_is_actually_translated`), KB
      table extended and `assets/knowledge/chunked.data` rebuilt.*
  - The guide gains `### Rust inside COBOL — EXEC RUST` inside §13 (the language
    section) rather than a new numbered section: renumbering would break every
    translation's structure and every TOC anchor, for no gain.
  - The System KB's `rustcobol_extensions.md` gains the same contract in the form
    the agents read it — the two block kinds and where each may appear, what may
    cross into a block, the `i64`/`f64` binding rule, developer-defined types and
    their `Default` requirement, the `Result`-body/`?` contract, shared state,
    the linked-crate set, and that Run builds.
  - `chunked.data` regenerated with `cargo run -p cobolt-ide --example
    build_chunked_kb`, so `prebuilt_chunked_kb_matches_the_published_documentation`
    is green rather than red-and-explained.
  - **Translations untouched**, per the standing rule.

  - Files touched: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`,
    `crates/cobolt-compiler/src/lib.rs` (KB tables), `assets/knowledge/chunked.data`.

- [x] **T16 — Finalize** — *full sweep **80 suites, 1545 passed, 0 failed, 8
      ignored**; CHANGELOG entry added; version bumped **1.60.8 → 1.60.9** (fix
      number only).*
  - Command: `cargo test --workspace --lib --tests --no-fail-fast`. Every
    `test result` line was collected and summed, not grepped for failures.
    `cobolt-compiler` alone is **33 passed in 1028 s** — the end-to-end tests
    each build and run a real binary.
  - **Two exclusions, stated rather than hidden.** `--lib --tests` runs no
    **examples**: `cobolt-forms`'s `gen_reference_theme` does not compile
    (`unresolved import 'image'`), which I confirmed is **pre-existing** by
    stashing this branch's changes and rebuilding it. It also runs no doc-tests.
  - The handoff's baseline was 1523 passed / 2 failed, both `secrets::macos`
    Keychain tests. Those **passed** in this run; they are environment-dependent
    (each waits ~60 s on the Keychain), not fixed by anything here.
  - **Version.** `tech.md` says a feature bumps the minor, which contradicts the
    operator's standing rule that only they raise `x`/`y` — the rule spec Q4
    defers to. So `z` was bumped and the contradiction is recorded in Q5 rather
    than resolved unilaterally.
  - **Q5 acted on**: this change edits the KB tables, so `build_chunked_kb` was
    run and the regenerated `assets/knowledge/chunked.data` (948 records from 5
    documents) is committed with it; `tech.md`'s "suspended" parenthetical was
    corrected to match the operator's 2026-07-31 lift, since leaving it would
    tell the next change to skip a required step and to read a real red test as
    expected. Flagged in the spec for confirmation.
  - **AC3 is partially observed and says so.** The binary runs its compiled block
    with `PATH` empty and `RUSTUP_HOME`/`CARGO_HOME` cleared. A genuinely
    toolchain-free *machine* cannot be produced from the build machine; that step
    is the operator's.
  - **AC15 is left unchecked, deliberately.** macOS is observed — every
    end-to-end test here builds and runs a binary on `aarch64-apple-darwin`.
    Linux and Windows cannot be verified from here and are not claimed.
  - **T13's on-screen behaviour is likewise unverified**: the IDE compiles and the
    build-or-interpret decision is unit-tested, but pressing *Run* is the
    operator's check.
  - **Not done here, on purpose:** nothing is pushed, and `features` is not merged
    into `main` — both are the operator's call.

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
