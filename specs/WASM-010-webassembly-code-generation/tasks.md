<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — WebAssembly code generation (WASM-010)

- **Status:** draft (awaiting the operator's review, then `/implement`)
- **Plan:** ./plan.md   **Date:** 2026-10-22
- **Branch / line:** `features-2x`, 2.x.x. **No documentation task:** `README.md` and `docs/` are never touched on 2.x.x. What 2.x.x says about itself goes in `specs/` (`WASM-PROGRESS.md`, `WASM-PARITY.md`, `WASM-ABI-v1.md` §12) and in the System KB.
- **Binding (operator, 2026-10-22):** Q2 `wasm-encoder` + `wasmtime`; Q4 explicit state; Q6 measure first; Q7 iOS → Safari, else Chrome/Safari parity; Q8 in-memory INDEXED + `SORT` are in; Q9 `DISPLAY` → console, `ACCEPT` ignored.
- **Open, with the working assumption each task uses (not decided here):** Q3 routines authored as option (a), as the code already does (no task depends on changing it); Q5 "avoid `REDEFINES`" → a Check **warning**, not a refusal (T13); Q-new-1 the record-limit clause spelling (T14 is **gated** on it); Q-new-2 only `cobolt-wasm`'s host moves to `wasmtime`, the player's tests stay on `wasmi` (T2, T21); Q-new-3 start-up measured in the Build only if T1 shows `wasmtime` is acceptable in the IDE graph, else `None` (T19); Q9 detail: `ACCEPT` ignored **with a warning** (T6).

Each task leaves the workspace green. Every task that edits `crates/cobolt-wasm-rt` ends with `cargo run -p cobolt-wasm --example build_rt` and a green `cargo test -p cobolt-wasm --test rt_blob` (the blob and `assets/rt.sources.hash` are regenerated in the same task). Every test prints a quantified summary line. `df -h` before any multi-crate run.

## Grounding (read for this task list, 2026-10-22)

The plan's two "not yet read" items are located:

- **The value-method list** is `cobolt_ast::methods::DATA_ITEM_METHODS` (with `CANONICAL_DATA_ITEM_METHODS`, `DATA_ITEM_METHOD_ALIASES`, `is_data_item_method`, `DATA_ITEM_METHODS_TEXT`) in `crates/cobolt-ast/src/methods.rs`: `Trim, UpperCase, ToUpperCase, Upper, LowerCase, ToLowerCase, Lower, Replace, Len, Length, Split`. The interpreter evaluates them in `crates/cobolt-runtime/src/interpreter.rs` (the `"REPLACE"`/`"TRIM"`/`"UPPERCASE"` arms near 15 817). T11 takes the list from `cobolt-ast`; nothing is re-listed in `cobolt-wasm`.
- **How a generated line maps back to its form and event** is `cobolt_codegen::SourceMap` (spec 053) in `crates/cobolt-codegen/src/lib.rs`: `MappedSpan { site: CodeSite, gen_start, gen_end, site_line_at_start }`, `OwnedSpan`/`GenOwner`, `SourceMap::resolve(gen_line) -> Option<(&CodeSite, u32)>` and `owner_of(gen_line)`, produced by `generate_with_map(&Form)`. The IDE uses it (`crates/cobolt-ide/src/app.rs`, `handler_breakpoint_gen_lines`). T10 builds `prlines` from it.

Other facts the tasks rely on, each read: `cobolt-wasm/src/module.rs` (`Options` :51, `Compiled` :83, `FormSpec` :141, `compile_program` :154, `compile_project` :161, `glue_needed` :369, `"forms": []` :889); `cobolt-wasm/Cargo.toml` (`wasm-encoder 0.244`, `[dependencies.wasmi]`, feature `host = ["dep:wasmi"]`); `cobolt-wasm/src/support.rs` (`enum Support`, exhaustive `support(&Stmt)`, `browser_builtin` answering 8 names); `cobolt-wasm/src/lower.rs::accept_` :2840 (refuses "ACCEPT from the operator has no meaning in a browser module"); `NCTEMP` checks in `lower_form.rs` :154/:181/:289 and `lower_more.rs` :147; `cobolt-project-tools/src/web/mod.rs` (`BuildReport.startup_ms` :169, `WebBuildError::{NotYetLinked, Refused}` :197/:200, `trait WebBuildBackend` :225, `link` :279, `backend` :289 — `web::link` is called today only from `tests/kinds.rs:275`); `cobolt_compiler::link_common_code` (`cobolt-compiler/src/lib.rs:1571`); `system_documentation()` (`cobolt-compiler/src/lib.rs:4983`); the `pk_build_*` keys of `cobolt-ide/src/i18n.rs` :410–414; `cobolt-ast/src/program.rs` (`storage_mode: StorageMode` :114, `enum StorageMode` :197; no `is_initial`/`is_common`; no record-limit clause in the lexer or parser); `cobolt-ast/src/stmt.rs` `WebSocket` :72; `crates/cobolt-semantic/examples/nist_conformance.rs`; `crates/cobolt-graphics/tests/dependency_guard.rs`; `crates/cobolt-web-player/tests/abi_host.rs`; `tests/cobol/` (21 folders); `crates/cobolt-ide/src/version.rs` `VERSION = "2.0.29"`.

## Tasks

### Engine (WP1)

- [x] **T1 — Probe `wasmtime` (stop gate)** — done 2026-10-22, result **stops**: `wasmtime 44.0.3` (MSRV 1.92) compiles `helpers.c` with `cc` on unix; plan.md §1 WP1 "Probe result". The operator chose `wasmtime 44.0.3`, the IDE included (D5 as planned), on 2026-10-22: T2+ proceed (plan.md §1 WP1). (Q2, spec §6 "probed at `/plan`", D2, D5)
  - Files: none in the tree; the findings go into `plan.md` §1 WP1 as a dated "Probe result" paragraph.
  - Do: for the newest `wasmtime` whose `rust-version` ≤ 1.92, record licence, `rust-version`, whether any build script in its minimal feature set (`cranelift`, `runtime`, no `cache`/`wat` unless needed) compiles C (`cc`, `cmake`), transitive package count, clean-build time of `cobolt-wasm --features host`, and the added weight if the IDE gained it (input for Q-new-3).
  - **If MSRV > 1.92 or a C build script cannot be turned off: stop and ask the operator.** `wasmi` is not kept silently.
  - Verify: the paragraph names the version, each of the five numbers, and the decision "passes / stops".

- [x] **T2 — `cobolt-wasm`'s test host on `wasmtime`, guarded** (Q2, R13, D2)
  - Files: root `Cargo.toml` + `Cargo.lock` (additions only), `crates/cobolt-wasm/Cargo.toml` (`host = ["dep:wasmtime"]`, `wasmi` removed; `cobolt-wasm-abi` added), `crates/cobolt-wasm/src/host.rs`, new `crates/cobolt-wasm/tests/dependency_guard.rs` (shape of `cobolt-graphics/tests/dependency_guard.rs`).
  - Do: rewrite `host.rs` over `wasmtime` with `Config::wasm_multi_memory(true)` and `cranelift_nan_canonicalization(true)`; replace the hand-rolled `varint`/`apply` with `cobolt_wasm_abi::batch`; keep `run_program`/`run_conformance` signatures. Root `[profile.dev.package.wasmi*]` stays (the player's tests, Q-new-2).
  - Verify: `cargo test -p cobolt-wasm --no-fail-fast` green (every existing test, both variants); `dependency_guard` asserts the licence set, MSRV ≤ 1.92 and no C-compiling build script, and is shown to fail on a planted package; `git diff Cargo.lock` shows additions plus the removal of `wasmi` from `cobolt-wasm` only.

- [x] **T3 — NIST NC, IF, IC rerun on `wasmtime`** (R16a, AC7)
  - Files: `crates/cobolt-wasm/examples/nist_wasm*.rs` (only if the engine API leaks into them), `specs/WASM-PROGRESS.md`.
  - Do: run `nist_conformance run NC|IF|IC` with `RCRUN=…/nist_wasm_run`; compare with the ledger (NC 95/95 4 612, IF 45/45 841, IC 25/25 309, 0 failures). Any change is a finding written in the ledger, not noise. Note the per-program time against the 20 s harness limit.
  - Verify: the ledger section for this version has, per module, compiled / passed / excluded-with-construct in two numbers, 0 failures.

### Subset and diagnostics (WP3)

- [x] **T4 — The built-in table** (R7, R8, AC6, Q7)
  - Files: `crates/cobolt-wasm/src/support.rs` (`browser_builtin` → `builtin_support(name) -> Support`, every caller updated), new `crates/cobolt-wasm/tests/subset_table.rs`.
  - Do: one entry per name of `cobolt_runtime::builtins::BUILTINS` (68 today): *browser* / *backend only* / *unsupported*; entries depending on a browser API carry the engines that have it (Safari decides for iOS, else both Chrome and Safari).
  - Verify: `cargo test -p cobolt-wasm --test subset_table` — iterates `BUILTINS`, fails on a missing name, prints "N of 68 classified"; a planted unknown name fails it.

- [x] **T5 — Typed refusals and warnings** (R8, D6, spec §6 i18n)
  - Files: `crates/cobolt-wasm/src/error.rs` (`GenError.kind`, `GenWarning`), `module.rs` (`Compiled.warnings`), every `GenError` construction site in `lower*.rs`/`support.rs`.
  - Do: kinds = backend-only construct, unsupported construct, unknown layer, unknown layer property, write to `Layer`, too many host reads in one statement, in-memory file without a record limit; warnings = `ACCEPT` ignored, `REDEFINES` may differ (Q5 assumption). English text unchanged.
  - Verify: `cargo test -p cobolt-wasm --no-fail-fast` green; a unit test asserts each refusal path sets its kind and keeps the line.

- [x] **T6 — `ACCEPT` ignored, with a warning** (Q9, R7; user code is sacred)
  - Files: `crates/cobolt-wasm/src/lower.rs::accept_`.
  - Do: in a non-conformance build, `ACCEPT` without `FROM` (or from a console mnemonic) emits no code and pushes the `ACCEPT` warning at its line; `FROM DATE/TIME/DAY/DAY-OF-WEEK` keep reading the host clock; conformance builds unchanged.
  - Verify: test: the receiving item keeps its value, one warning with the line; NIST IC/NC unaffected (T3 numbers rerun for NC).

- [x] **T7 — IDE diagnostics in six languages** (spec §6 i18n, D6)
  - Files: `crates/cobolt-ide/src/i18n.rs` (one `Tr` key per kind/warning of T5, EN/ES/PT/JA/ZH/FR), `crates/cobolt-ide/src/panels/web_preview.rs` (translate `Refused`/warnings by kind).
  - Do: no "cobolt" in the text; say "browser module" / "WebAssembly module"; pt-BR spelling.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` green (no empty translation).

### The project module (WP2)

- [x] **T8 — Manifest through `cobolt-wasm-abi`** (R1, R2, R12, D3; ABI A7, A9)
  - Files: `crates/cobolt-wasm/src/module.rs` (replace the `serde_json::json!` manifest at :889).
  - Do: build `cobolt_wasm_abi::manifest::Manifest` with `forms[]` (id, name, main, instances, region_size, controls, handlers, layers), `names`, `events`, `builtins`, `limits` (task rows incl. spare), `sizes`, `imports`.
  - Verify: test: for a two-form project, `Manifest::validate` is clean and `scan::bundle_problems(bytes, false)` / `(…, true)` are empty for both variants.

- [x] **T9 — Form exports, task table, `ERROR` fields** (R1, R10, R11, R3; ABI §4, A1, A2, A4, A5)
  - Files: `crates/cobolt-wasm/src/module.rs`, `crates/cobolt-wasm-rt/src/state.rs` (task rows), `crates/cobolt-wasm/src/host.rs` (form-driving API: `open_form`, `event`, `resume`, `cancel`, `close_form`, event queue), new `crates/cobolt-wasm/tests/forms.rs`.
  - Do: export `pr_open_form`, `pr_event`, `pr_resume`, `pr_cancel` (`-1` discards a trapped run's batch), `pr_tick`, `pr_status`, `pr_close_form` (region released and reinitialised on `DONE`), optional `pr_dbg_live`/`pr_dbg_region_used`; task table `Σ instances + spare`, exhaustion = run-time error naming the form; `glue.abort` writes magnitude, unit, form, event, `$line`; COBOL errors from 1000.
  - Verify: `forms.rs::button_handler_move_add_display` (**AC1**: expected `SET` commands and `DISPLAY` text on both variants; `scan_module` imports ⊆ ABI §3; prints commands / host calls); `forms.rs::runtime_error_has_line` (R11: divide by zero without `ON SIZE ERROR` → `ERROR` ≥ 1000 with line, form, event; the next event still runs); `forms.rs::instances_exhausted_is_an_error`.

- [x] **T10 — Imports, batching, line table** (R12, R14, AC9)
  - Files: `crates/cobolt-wasm/src/module.rs` (glue `query`, `random`, `remote`; flush at `DONE`/`SUSPEND`/`YIELD`/full batch; custom section `prlines`; `$line` exported; `Compiled.lines`), `crates/cobolt-wasm/Cargo.toml` (`cobolt-codegen` if `SourceMap` is taken directly).
  - Do: `prlines` = line id → unit, source line, column; for generated form code resolve through `cobolt_codegen::SourceMap::resolve`/`owner_of` (Grounding) — no second mapping.
  - Verify: `forms.rs::host_calls_per_handler` (one flush per handler end; reads see the handler's own earlier writes); `forms.rs::manifest_and_imports` (**AC9** part 2: a host missing `query` fails instantiation naming it); `forms.rs::error_line_maps_to_handler` (the reported line is the developer's handler line, as Check reports it).

### Phase D (WP4)

- [x] **T11 — Value methods on data items** (R7, R5)
  - Files: `crates/cobolt-wasm/src/lower_form.rs` (`member_read`/`member_call`: accept `cobolt_ast::methods::is_data_item_method`), `crates/cobolt-wasm-rt/src/strings.rs`; blob regenerated.
  - Do: lower every name of `DATA_ITEM_METHODS` as a value (into `T_CTEMP`) and as a `MOVE`/`SET`/`STRING` operand; other names keep the refusal with `DATA_ITEM_METHODS_TEXT`. `NCTEMP` stays 4; the refusal names the limit.
  - Verify: differential test through `tests/common::same_output`, one row per spelling (11), prints rows / equal; `rt_blob` green.

- [x] **T12 — Layers** (R19, R20, AC11; ABI A9)
  - Files: `crates/cobolt-wasm/src/module.rs` (`FormSpec.layers`), `lower_form.rs`.
  - Do: layer id `3 + n_controls + k`; `LAYER::Property` read/write through the control path; unknown layer / property → refusal naming the layer and listing `Name` (read-only), `Visible`, `Transparency`, `BackgroundColor`, the gradient properties, `BackgroundImage`, `BackgroundImageMode`; write to a control's `Layer` refused; no handler emitted or dispatched for a layer.
  - Verify: `forms.rs::unknown_layer_refused` (**AC11**: `LAYER-9::Visible` on a one-layer form fails naming the layer); `forms.rs::layer_visible_round_trip`; `forms.rs::no_layer_handler`.

- [x] **T13 — Parity gaps #7, #9, #10 and the `REDEFINES` warning** (R5, R16c, Q5 assumption)
  - Files: `crates/cobolt-ast/src/program.rs` (`is_initial`, `is_common`, `#[serde(default)]`), `crates/cobolt-parser` (keep `INITIAL`/`COMMON`), `crates/cobolt-wasm/src/lower_call.rs`, `lower_more.rs`, `crates/cobolt-wasm-rt/src/func.rs`/`call.rs`; `specs/WASM-PARITY.md` rows #7/#9/#10 closed.
  - Do: #7 table-slice argument with the run-time count; #9 `BY VALUE` with the callee's `USAGE` and `RETURNING` on a dynamic `CALL`; #10 `INITIAL`/`COMMON`; recursion stays refused. `REDEFINES` warning only where a difference is shown not to be a bug (Q5 working assumption — the operator may say "refuse").
  - Verify: `cargo test -p cobolt-parser -p cobolt-ast -p cobolt-wasm --no-fail-fast` green; one differential test per gap; interpreter NIST regression of every finished module (NC, IF, IC, SM, ST, SQ, IX, RL) unchanged (GOLDEN RULE #9 — the AST changed).

- [x] **T14 — In-memory INDEXED and `SORT`/`MERGE`** (R7, R3, Q8) — Q-new-1 **decided 2026-10-22**: no record limit and no new clause; when memory runs out the verb returns FILE STATUS `89` and keeps every record already held
  - Done 2026-10-23: `cobolt-wasm-rt/src/idx.rs` (file heap, in-memory INDEXED, SORT work file), `cobolt-wasm/src/lower_idx.rs`, `Support::MemoryFiles`, `Options::file_heap` (16 MiB default), ABI amendment A11, `WASM-PARITY.md` rows 15–16. `tests/memory_files.rs`: 8 INDEXED cases + 1 SORT case equal on both engines and both variants (113 lines), status 89 after 512 records in a 64 KiB heap with all 512 read back, 3 refusals, `pr_dbg_region_used` back to 0 after the form closes. The "unit tests for the clause" are moot: Q-new-1 removed the clause (and the `MemoryFileWithoutLimit` kind with its six `Tr` texts). `MERGE` stays *backend only* (it reads files); `SORT … USING/GIVING` likewise. **Working assumption, not decided here:** a browser memory file without `WITH PERSISTENCE` can be written (the desktop answers 37) — `WASM-PARITY.md` #15.
  - Files: no lexer/parser/semantic change (no clause); `crates/cobolt-wasm/src/support.rs` (file verbs context-aware on `StorageMode`), new `crates/cobolt-wasm-rt/src/idx.rs` (`no_std` B-tree over the instance region); blob regenerated.
  - Do: memory files compile; disk files stay *backend only*; `WITH PERSISTENCE` refused; no limit is asked for; memory exhausted → status `89`, records kept (a test fills a module with a small memory cap until `89`, then reads back every record written before it). Status codes, key order and duplicates held to the interpreter's in-memory engine.
  - Verify: lexer/parser/semantic unit tests for the clause; differential tests (status codes, `START`, duplicates, `SORT` with `INPUT`/`OUTPUT PROCEDURE`), prints cases / equal; the region is released with the instance (`pr_dbg_region_used` back to its start value).

### Phase C (WP5)

- [x] **T15 — Group ⇄ JSON in the rt** (WASM-005 R5b, R9)
  - Files: new `crates/cobolt-wasm-rt/src/json.rs`, new `crates/cobolt-wasm/tests/json_vectors.rs`; blob regenerated.
  - Verify: every vector of `specs/WASM-005-websocket-call-and-remote-data/vectors/group-json.v1.json` through the module equals `cobolt-wire::mapping` (prints vectors / equal).
  - Done 2026-10-23: `cobolt-wasm-rt/src/json.rs` (strict reader, encoder, atomic decoder over a shadow copy; `rt_json_encode`/`rt_json_decode`), `cobolt-wasm/src/json_layout.rs` (the descriptor, built from `cobolt_wire::from_decl` so a layout is refused by the same definition). `tests/json_vectors.rs`: 26 cases, 23 encodings, 22 round trips, 61 reads accepted, 92 refused, 3 layouts refused — the reference runner's tally exactly — and 176 of 176 answers equal to `cobolt-wire`'s. **How it ran:** the routines' native build on the generator's descriptor and region image (the code linked into every module, against a test memory), not inside a wasm instance; the first run inside a module is T16's. National items: `WASM-PARITY.md` #17.

- [x] **T16 — Suspension: the WebSocket `CALL` and the suspending verbs** (R9, R10, R11, AC5; ABI §12.1, A5, A6)
  - Files: `crates/cobolt-wasm/src/lower_call.rs`, `lower_form.rs`, `module.rs`, `host.rs` (queue).
  - Do: `CALL … WITH WEBSOCKET LINKAGE`, `MessageBox`/`Alert`/`Confirm`/`InputBox`, `Sleep`/`Wait`, `OpenFormSync`, `me::OpenFormSync` end their block with `SUSPEND`, `$resume` = continuation; `pr_resume` decodes `{"id","result"}` into `RETURNING`/`BY REFERENCE` items and `{"id","error"}` into `ON EXCEPTION` (or the R11 error).
  - Verify: `forms.rs::suspend_resume_with_queued_events` (**AC5**: `SUSPENDED`, two queued events, result frame then error frame, final data as expected).
  - Done 2026-10-23: `cobolt-wasm/src/lower_remote.rs` (the call, the waits, the ABI §12.1 verbs of `COBOL::`), `cobolt-wasm-rt/src/remote.rs`, `pr_resume` stages frame-in in memory 0, ABI amendment A12 (the request frame and the `result` body — WASM-014 Δ5's working assumption, since WASM-014 T6 is not built). `host.rs` needed no change: its queue already existed. `forms.rs`: `suspend_resume_with_queued_events` (AC5, both variants: SUSPENDED, 2 queued, result frame then error frame, 3 lines as expected, 2 request frames byte-checked), `a_bad_answer_or_a_call_without_on_exception` (ON EXCEPTION with the group untouched; run-time error 1110 without it; the next event runs), `a_message_box_suspends_the_handler`: 13 of 13 pass.

- [x] **T17 — Yield on budget** (R9)
  - Files: `crates/cobolt-wasm/src/module.rs` (fuel `-2` → status `YIELDED`, task row saved).
  - Verify: `forms.rs::yield_is_invisible`: one long `PERFORM` with budgets 1, 7, 20 000 gives identical data and batches.
  - Done 2026-10-23: `module.rs` already did it (`run_task`'s `YIELD` branch, part 1); only the test was missing. `yield_is_invisible`: 2 variants × budgets 1, 7, 20 000 → 409, 101 and 0 yields (the start-up run yields too under budget 1), identical output and the same 4 commands each; the batches are cut at each yield (5 flushes against 1), as ABI §3 says `flush` is called when a task yields.

### Conformance (WP7)

- [x] **T18 — Golden layouts, arithmetic table, control flow, determinism** (R3, R4, R5, R13; AC2, AC3, AC4)
  - Done 2026-10-23: **AC2** 2 images × 2 variants, 148 bytes equal to hand-written golden bytes (`host::run_program_memory` added to read memory 0); **AC3** 15 rows equal (truncation and `ROUNDED` for each of the 5 verbs — the language has no `ROUNDED MODE` clause —, SIZE ERROR, division by zero, 18 and 38 digits computed rather than written, a 39-digit SIZE ERROR, `DIVIDE … REMAINDER`); `COMPUTE … / 0 ON SIZE ERROR` differs and the interpreter is wrong (`WASM-PARITY.md` #18, a 1.x.x fix; pinned on the module by its own test); **AC4** 19 lines equal; **R13** 5 batches, 583 bytes per variant, identical over two runs (`HostOptions::record_batches` added).
  - Files: new `crates/cobolt-wasm/tests/layout_golden.rs`, `arith_table.rs`, `control_flow.rs`, `determinism.rs`.
  - Verify: `cargo test -p cobolt-wasm --test layout_golden --test arith_table --test control_flow --test determinism`: **AC2** byte-for-byte images after `VALUE` and after `MOVE`s (`REDEFINES`, `OCCURS DEPENDING ON`, `COMP-3`, trailing / `LEADING` / `SEPARATE` signs; prints bytes compared); **AC3** ≥ 1 row per `ROUNDED` mode × verb, `SIZE ERROR`, 18- and 38-digit operands, `DIVIDE … REMAINDER` equal to the interpreter (prints rows / equal); **AC4** `GO TO … DEPENDING`, `ALTER`, `PERFORM … THRU` + `EXIT PERFORM` identical on both engines and both variants; **R13** two runs of one event script per variant, batches byte-identical with fixed `now`/`random`.

### Build (WP6)

- [x] **T19 — `compile_web_project`, `GeneratorBackend`, the link, the report** (R1, R15, AC9; D5, Q-new-3)
  - Done 2026-10-23: `cobolt_wasm::compile_web_project` (+ `GenError::form`, so a refusal names its form), `cobolt-project-tools/src/web/generator.rs`, linked in the IDE's `main` and in `rcrun` before any command. Per the operator's T1 decision (wasmtime in the IDE, D5 as planned) `cobolt-project-tools` takes `cobolt-wasm` **with** `host`, and `startup_ms` is measured: `pr_init` + `pr_open_form(main)` on wasmtime, compile included (309 ms in a debug build on the fixture; a native proxy, not a browser figure). Forms are regenerated **in memory** by `cobolt-codegen` (the `.cbl` on disk is not rewritten by the web build; the IDE regenerates it before Build as before). The player comes from `PRC_WEB_PLAYER_DIR` or the repository's `assets/web-player`; shipping it with an installed IDE is WASM-002's. `cobolt-web-bundle::variants_agree` now ignores `limits.data_pages`, which differs by construction between the variants (the single-memory one holds the arena). `cargo test -p cobolt-project-tools --no-fail-fast`: 142 passed, 1 ignored (pre-existing), incl. `web_build.rs` 2 of 2 (AC9 part 1: `program.*.wasm` 87 108 B and `program.single.*.wasm` 87 125 B in a 25-file bundle, `startup_ms` present; `EXEC SQL` refused at `webforms/main-form.cfrm (main-form ▸ BTN-OK ▸ onClick):2`); `cobolt-web-bundle` 30 passed; `cargo build -p cobolt-ide -p cobolt-cli` green.
  - **Owner of the Web Build backend (operator, 2026-10-22).** WASM-002 T13–T16 consume this backend instead of
    writing their own `WasmBuild`; build it so the Web Build can call it as its one way from project to `dist/web`.
  - Files: `crates/cobolt-wasm/src/module.rs` (`compile_web_project(…) -> WebModules { multi, single, warnings }` per `ModuleVariants`), new `crates/cobolt-project-tools/src/web/generator.rs` + `mod generator` in `web/mod.rs`, `crates/cobolt-project-tools/Cargo.toml` (`cobolt-wasm` `default-features = false`, `cobolt-web-bundle`), the IDE start-up and `rcrun`'s `mcp`/`build` (one `web::link(Arc::new(GeneratorBackend))` each).
  - Do: forms regenerated by `cobolt-codegen` (generated-code contract), parsed, `link_common_code` applied; `assemble_bundle`; atomic bundle replace; `startup_ms` = `pr_init` + `pr_open_form(main)` on `wasmtime` natively **only if T1 accepted its weight in the IDE**, else `None` (Q-new-3 stays open; the choice is written in the CHANGELOG).
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast`: a build test on a fixture Web project gets `BuildReport` with `program.wasm`/`program.single.wasm` sizes and `startup_ms` (or `None`) (**AC9** part 1); a project with an `EXEC SQL` handler gets `WebBuildError::Refused` with the line; `cargo build -p cobolt-ide -p cobolt-cli` green.

- [x] **T20 — The `tests/cobol/` corpus** (R16b, AC8)
  - Done 2026-10-23: `tests/corpus.rs` — 57 programs, 17 compiled, **17 identical** (581 lines, both variants), 0 differ, 40 excluded each with its construct and line (files 18, `EXEC SQL` 11, national/UTF-8 functions and literals 3, value method as receiver 2, `MERGE`, `IMAGE-INFO`, `ACCEPT FROM Environment`, `SET` pointer, a `COPY … REPLACING` the free-form preprocessor calls malformed, a member expression the generator does not take). Two generator gaps found and fixed on the way: a callee's LINKAGE `RETURNING` record is now bound (`WASM-PARITY.md` #13 closed) and `CALL "paragraph"` performs a paragraph of the program as the interpreter does. Clock readings (lines with *elapsed* or *per second*) are compared by their label only: two runs of one engine differ there. A difference must be in `KNOWN` with its parity row (empty today); a known one that goes away fails too.
  - Files: new `crates/cobolt-wasm/tests/corpus.rs`, `specs/WASM-PARITY.md`.
  - Verify: `cargo test -p cobolt-wasm --test corpus` prints compiled / identical / excluded (with construct); every difference is an open `WASM-PARITY.md` row, never `#[ignore]`.

- [x] **T21 — The player's ABI test on a generated module** (R12, D3; Q-new-2 **decided 2026-10-22: the player's native tests move to `wasmtime` too**)
  - Files: `crates/cobolt-web-player/tests/abi_host.rs` (stays on `wasmi`).
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test abi_host` green against the fixtures and a module from `compile_project`.
  - Done 2026-10-23, per Q-new-2 (the player's native tests move, so the "stays on `wasmi`" of the Files line no longer holds): the player's rig runs on `wasmtime =44.0.3` (a dev-dependency only; `wasmi` has left both workspaces). The player is a workspace of its own and does not build the generator, so the generated module is committed in `crates/cobolt-web-player/tests/fixtures/generated/` (both variants and the form's `.cfrm`), written by `cobolt-wasm/tests/player_fixture.rs`, which fails when it is stale. `abi_host`: 22 of 22, incl. `a_module_the_generator_built_runs_in_the_player` (both variants: opened, 2 clicks, the caption in the form tree, 2 console lines). It found a real disagreement: the generator sent one `DISPLAY` command per operand and a separate line end, the player logs one command as one line — the routines now send one command per statement (ABI amendment A13). The player's `Cargo.toml`/`Cargo.lock` are inputs of its fingerprint, so `build_player` was run: the shipped wasm is byte-identical, only `player-build.json` changed. Whole player suite: 250 passed.

- [x] **T22 — System KB: "What runs in a browser module"** (R7, spec §6 System KB, D7)
  - Done 2026-10-23: the section is the end of `project_model_and_settings.md` (the System KB stays eight documents, which `system_documentation_is_what_publishing_writes` pins), rendered by `cobolt_compiler::browser_subset_doc()` from `cobolt_wasm::support::{STATEMENT_TABLE, BUILTIN_TABLE}` (`cobolt-compiler` takes `cobolt-wasm` without its engine). `STATEMENT_TABLE` is new: one row per `stmt_name`, held equal to the exhaustive `support()` by `subset_table.rs` (53 rows; 42 statement kinds cross-checked on parsed programs, the other 11 by name). Test `the_browser_subset_section_names_every_builtin_and_statement`: 68 built-ins and 53 statements present. `build_chunked_kb` run: 3 551 records from 8 documents, 12 730 368 bytes; `prebuilt_chunked_kb_matches_the_published_documentation` and `system_documentation_is_what_publishing_writes` green.
  - Files: `crates/cobolt-compiler/src/lib.rs` (`system_documentation()` section generated from `cobolt_wasm::support`: statements, built-ins, the in-memory file rule), `assets/knowledge/chunked.data`.
  - Verify: a `cobolt-compiler` test asserts every `BUILTINS` name appears; `cargo run -p cobolt-ide --example build_chunked_kb`; `prebuilt_chunked_kb_matches_the_published_documentation` and `system_documentation_is_what_publishing_writes` green.

- [x] **T23 — Interpreter/binary parity names the generator** (R17)
  - Done 2026-10-23: one paragraph, byte-identical in the three (md5 equal): a new section of `.claude/skills/interpreter-binary-parity/SKILL.md`, a new bullet of `specs/steering/tech.md`'s hard constraints, and after the "THREE hosts" paragraph of `CLAUDE.md` — edited at `PowerRustCOBOL-local-settings/CLAUDE.md`, which is untracked, so that edit is **not** in this change's commit. Shared source named from the manifests: `cobolt-decimal` (`cobolt-runtime` and `cobolt-wasm-rt` both depend on it) and `cobolt-numedit` (`cobolt-wasm-rt`, and the interpreter through `cobolt-forms`).
  - Files: `.claude/skills/interpreter-binary-parity/SKILL.md`, `specs/steering/tech.md`, and the CLAUDE.md rule (an untracked symlink, absent in this worktree: edit it through the local-settings copy, per "one rule lives in three files").
  - Do: the generator and its emitted routines are a further consumer; shared source = `cobolt-decimal`, `cobolt-numedit`; `cobolt-wasm-rt` = generator-only, held equal by measurement (NIST + corpus).
  - Verify: the three texts say the same thing (diff read side by side).

- [x] **T24 — Ledger and AC10 regression** (R16a, R18, AC7, AC10)
  - Done 2026-10-23 with one stated exception. **AC7:** NC 95/95 (4 612/0/5), IF 45/45 (841/0/0), IC 25/25 (309/0/4) on the final module, unchanged. **Interpreter NIST:** NC 4 614, IF 841, IC 309, SM 311, ST 735, SQ 624, IX 574, RL 354 PASS, 0 FAIL, equal to `NIST/progress.json`. **AC10 suites** (`cobolt-runtime`, `-parser`, `-semantic`, `-server`, `-compiler`): 245 test binaries, 2 201 passed, 34 ignored, **4 failed, all pre-existing and outside this change**: `sdk_tests::sdk_crate_set_is_closed_under_path_dependencies` ("cobolt-forms depends on cobolt-numedit, which the SDK does not ship") and three SDK builds failing on "failed to get `cobolt-decimal`" — at HEAD `cobolt-runtime` already needs `cobolt-decimal` and `cobolt-forms` `cobolt-numedit`, and `SDK_CRATES` (untouched here) lists neither. A two-line fix for `fixes-2x`, not mixed into this feature. The backend's `redb` and file tests are in the green `cobolt-runtime`/`cobolt-server` binaries. Ledger: `WASM-PROGRESS.md` 2.0.36 section.
  - Files: `specs/WASM-PROGRESS.md`, `specs/WASM-PARITY.md`.
  - Do: rerun NC/IF/IC on the final module (two numbers each, exclusions with construct); rerun the interpreter's finished modules (NC, IF, IC, SM, ST, SQ, IX, RL); record phase A–D status per R18 and the module size / start-up numbers (Q6: measured, no budget set).
  - Verify: **AC7** numbers recorded with 0 failures; **AC10** `cargo test -p cobolt-runtime -p cobolt-parser -p cobolt-semantic -p cobolt-server -p cobolt-compiler --no-fail-fast` green and interpreter NIST counts unchanged; a backend test still opens `redb` and files.

- [x] **T25 — Finalize**
  - Done (2.0.36): the crate sweep above plus `-p cobolt-web-bundle`: 1,280 passed, 4 failed. All 4 are the pre-existing `cobolt-compiler` SDK failures recorded in T24: `SDK_CRATES` lacks `cobolt-decimal` and `cobolt-numedit`; this is a fix on its own branch, not in these commits. `cobolt-ide --bin`: 1,870 passed, 5 failed. None of the 5 is touched by this diff, as checked against the diff's text:
    - the backend completion texts from f305f4a0 that are missing from the six-language table;
    - Portuguese documents that fall back to English;
    - two live crates.io verdicts now that egui 0.37 is published;
    - the Claude plugin validator asking for a README and a LICENSE.
  - The player passes 250. `cargo fmt --check` is not clean on the files this diff edits: those files already carried rustfmt differences, and every edited file except `tests/move_and_arith.rs` gained a few more (for example lib.rs 173 → 177). They were deliberately not reformatted, so the diff stays surgical; the files this feature adds are rustfmt-clean, and after formatting them `rt.wasm` was rebuilt and the player fixture regenerated. The credential scan of the diff and the new files is clean. `git diff --stat -- README.md docs/` is empty. The manual check of plan §6 is left to the operator.
  - Files: `crates/cobolt-ide/src/version.rs` (z bumped **once**: 2.0.29 → 2.0.30, or the next free number if a sibling session took it), `CHANGELOG.md` (one entry inserted above the previous `## [PowerRustCOBOL 2.0.z]` heading, dated, with *"(Written on `features-2x`, after 2.0.z.)"*).
  - Do: run the tests of every crate touched, with `--no-fail-fast`, and read every `test result` line: `cargo test -p cobolt-wasm -p cobolt-wasm-rt -p cobolt-wasm-abi -p cobolt-ast -p cobolt-lexer -p cobolt-parser -p cobolt-semantic -p cobolt-codegen -p cobolt-compiler -p cobolt-project-tools -p cobolt-cli --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`, `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`; `cargo fmt --check`; credential scan of the staged diff.
  - Verify: all green; manual check of plan §6 (Web project, button sets a Label, Build shows size and start-up in the IDE's language, Preview click changes the label) is **left to the operator**; no browser run is claimed that was not done. **No documentation task**: `README.md` and `docs/` untouched (`git diff --stat -- README.md docs/` empty).

## Not in this feature

Interpreter bugs found by parity (`WASM-PARITY.md` #1, #2, #5, #8 and any new one) are **fixes on 1.x.x**, each on its own fix branch with the full NIST regression, cherry-picked forward; never mixed into these commits (D8).

## Done criteria

AC1 (T9), AC2–AC4 (T18), AC5 (T16, T17), AC6 (T4, T5, T6), AC7 (T3, T24), AC8 (T20), AC9 (T10, T19), AC10 (T2, T24), AC11 (T12) are checked; tests pass; the System KB is regenerated; `README.md` and `docs/` untouched; the operator has answered or confirmed Q3, Q5, Q-new-1, Q-new-2, Q-new-3 and the `ACCEPT` warning, or each assumption above stands visibly in the CHANGELOG entry.
