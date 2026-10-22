<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — WebAssembly code generation: the compiler emits the module, and the module carries what it needs

- **Status:** draft (awaiting the operator's review, then `/tasks`)
- **Spec:** ./spec.md   **Date:** 2026-10-22
- **Branch / line:** `features-2x`, version line 2.x.x. Companions: `specs/WASM-PLAN.md` (P1–P8), `specs/WASM-ABI-v1.md` (the contract, amendments A1–A9), `specs/WASM-PROGRESS.md` (the ledger), `specs/WASM-PARITY.md` (divergences).
- **Binding decisions (operator, 2026-10-22, from §7 of the spec):** Q1 direct generation (confirmed 2026-10-20); Q2 **`wasm-encoder` emitter, `wasmtime` engine**; Q4 **explicit state kept by the generated code**; Q6 measure phase A before setting budgets; Q7 iOS → Safari's built-ins, otherwise Chrome/Safari parity; Q8 **in-memory INDEXED files and in-memory `SORT` are in**; Q9 **`DISPLAY` → host console, `ACCEPT` is ignored**; Q10 two `.wasm` files.
- **Still open (working assumptions stated, not decided here):** Q3 (how the routines are authored), Q5 (what "avoid `REDEFINES`" means). See §8.

## 0. Where the code stands (read 2026-10-22, `features-2x` at 79400bd9, 2.0.29)

This plan starts from code that already exists, so it plans the **remaining** work. I didn't infer anything below; each item was read in the file named.

| Area | Exists | Evidence |
|---|---|---|
| Shared decimal kernel | `crates/cobolt-decimal` (`no_std`, no deps), re-exported by the interpreter | `cobolt-decimal/Cargo.toml`; ledger "V, 1527 passed" |
| Shared edited-picture engine | `crates/cobolt-numedit` | `cobolt-wasm-rt/Cargo.toml` depends on it; ledger "V" |
| Routines (Q3 option (a), as WASM-PLAN P1) | `crates/cobolt-wasm-rt` (`rlib` + `cdylib`), compiled to `crates/cobolt-wasm/assets/rt.wasm` by `cargo run -p cobolt-wasm --example build_rt`, freshness pinned by `assets/rt.sources.hash` + `tests/rt_blob.rs` | `cobolt-wasm-rt/Cargo.toml`, `cobolt-wasm/src/rtsrc.rs`, `module.rs` `static RT_WASM` |
| Emitter + mini-linker | `crates/cobolt-wasm`: `wasm-encoder 0.244`, `link.rs` (`Blob::load`, `Blob::closure` tree-shakes the rt from the roots the blocks call) | `cobolt-wasm/Cargo.toml`; `module.rs::compile_forms` "which routines" |
| Code shape (Q4) | trampoline of block functions, dispatcher `run` over `call_indirect`, fuel global, reserved codes `DONE -1 / STOP -4 / PARK -5 / SUSPEND -6`, yield on fuel `< 0` (`-2`) | `module.rs` "dispatcher and exports"; `ir.rs` consts |
| Memory | fixed pages (`minimum == maximum`), no `memory.grow`; arena = memory 1 or a region of memory 0; per-form templates and `instances × region_size` regions | `module.rs` "memory map", `MemorySection` |
| Statement table (R7/R8) | `support.rs::support(&Stmt) -> Support { Module, Conformance, BackendOnly, Unsupported }`, an **exhaustive `match`** (a new `Stmt` variant does not compile until classified) | `cobolt-wasm/src/support.rs` |
| Built-in table (R7) | **incomplete:** `support.rs::browser_builtin(name)` answers 8 names; `cobolt-runtime/src/builtins.rs` `BUILTINS` has **68** entries | `grep -c '^    b("' builtins.rs` = 68 |
| Program mode | `compile_program` → module with `pr_run`; NIST **NC 95/95, IF 45/45, IC 25/25** with 0 failures on **`wasmi` 2.0** | ledger sections 2.0.19, 2.0.20, 2.0.22 |
| Project mode | **skeleton only:** `compile_project(&[FormSpec], &Options)` lowers every form, builds control/event name tables and instance regions, but **exports none of** `pr_open_form`, `pr_event`, `pr_resume`, `pr_cancel`, `pr_tick`, `pr_close_form`, `pr_status`; manifest `"forms": []`; no `query`, `random`, `remote` glue; `ERROR` writes unit/form/event as 0; no `prlines` section | `module.rs` lines 816–890; ledger "Where the work stopped (2.0.23)" item (1) |
| Host members | `lower_form.rs`: `Ctrl::Property` read/write, `Ctrl::Method(args)`, `INVOKE`, reserved ids 0/1/2; **value methods on data items are refused** ("not a control member", `member_read`); at most `NCTEMP` = 4 host reads per statement | `lower_form.rs::member_read`, `member_call` |
| `ACCEPT` | refused in a non-conformance build ("has no meaning in a browser module") | `lower.rs::accept_` |
| Test host | `cobolt-wasm/src/host.rs` over **`wasmi`** (feature `host`, default on); hand-rolled `varint`/`apply`; `run_program`, `run_conformance` | `host.rs`, `Cargo.toml [dependencies.wasmi]`, root `Cargo.toml [profile.dev.package.wasmi*]` |
| Differential harness | `tests/common/mod.rs::same_output` runs both variants against `cobolt_runtime::interpreter::Interpreter` | file read |
| ABI crate | `cobolt-wasm-abi`: `manifest::Manifest` (+ `validate`), `batch` codecs (`write_varint`, `read_text`, …), `scan::scan_module`, `scan::bundle_problems(bytes, single_memory)` | `cobolt-wasm-abi/src/*.rs` |
| Build seam | `cobolt-project-tools/src/web/mod.rs`: trait `WebBuildBackend`, `NotYetLinked`, `link()`, `BuildReport { startup_ms, … }`, `WebBuildError::Refused(Vec<String>)`; IDE shows it in `panels/web_preview.rs` with `pk_build_*` Tr keys (six languages) | files read |
| Bundle | `cobolt-web-bundle::assemble_bundle(&BundleInput)` takes `program_multi`/`program_single` bytes and checks them with `bundle_problems` | `cobolt-web-bundle/src/lib.rs:415` |
| Common Code | `cobolt_compiler::link_common_code(path, &mut Program)` already copies the project's Common Code into a program | `cobolt-compiler/src/lib.rs:1571` |
| JSON mapping reference | `cobolt-wire::mapping` + golden vectors `specs/WASM-005-websocket-call-and-remote-data/vectors/group-json.v1.json`; WASM-005's plan assigns "R9 in the browser module" and "the generator's own implementation of the same rules (R5b)" to this spec | `cobolt-wire/src/lib.rs` doc; WASM-005 `plan.md` lines 41–61 |
| Known divergences | `WASM-PARITY.md` rows 1–10 (interpreter bugs #1, #2, #5, #8 open; generator gaps #7, #9, #10) | file read |

So phases **A** and **B** of R18 are done **for program mode** and measured by NIST. What's left is **the project module** (forms, events, tasks), **phase C** (suspension and the remote `CALL`), **phase D** (the rest of the subset: value methods, in-memory INDEXED/SORT, layers, gaps #7/#9/#10), **the engine switch to `wasmtime`**, **the Build link**, and **the conformance items** that aren't measured yet (R16(b) corpus, R13 determinism, R14 line table, R15 report).

## 1. Approach

The architecture of WASM-PLAN P1–P8 stays as built. Below are the work packages in order, each mapped to the requirements it satisfies.

### WP1 — Engine switch to `wasmtime` (Q2, decided) — first, because every later gate runs on it
- **Probe before code** (spec §6 "probed at `/plan`"): I couldn't probe here. The registry cache has no `wasmtime` and network lookups were refused in this session. The probe is the **first task**. It checks the licence (expected Apache-2.0 WITH LLVM-exception), the `rust-version` of the newest `wasmtime` compatible with **MSRV 1.92**, whether any build script of its default or minimal feature set compiles C (`cc`, which the tech steering's pure-Rust rule and the WASM-018 dependency-guard pattern forbid), the transitive package count and the clean-build time. The guard is a test in the shape of `crates/cobolt-graphics/tests/dependency_guard.rs`, written for `cobolt-wasm`'s **dev/test** graph.
- **If the probe fails** (MSRV above 1.92 or a C build script that can't be turned off), **work stops and asks the operator**, as spec 103 did for its MSRV. `wasmi` is not kept silently.
- **If it passes:** `host.rs` is rewritten over `wasmtime` with `Config::wasm_multi_memory(true)` and `Config::cranelift_nan_canonicalization(true)`, which R13 determinism needs for `COMP-1/2`. Fuel or epoch interruption replaces nothing in the module, because the module yields itself. The hand-rolled `varint`/`apply` are replaced by `cobolt_wasm_abi::batch` (one codec, shared with the player). `wasmi` leaves `cobolt-wasm`. The root `[profile.dev.package.wasmi*]` entries **stay** because `cobolt-web-player/tests/abi_host.rs` still uses `wasmi`; whether the player's tests also move is open (§8, Q-new-2).
- **Acceptance of the switch:** NC, IF and IC rerun on `wasmtime` with the same two numbers as the ledger (95/95 4 612, 45/45 841, 25/25 309, 0 failures). Any change is a finding, not noise.

### WP2 — The project module (R1, R2, R10, R12, R14; ABI §4, A1, A2, A4, A5, A7, A9)
In `cobolt-wasm/src/module.rs`, finish what `compile_project` set up:
- **Exports:** `pr_open_form(form, opener)` allocates an instance from the form's region slots, initialises it with `init_form_<i>` and runs the form's start-up. `pr_event(inst, ctrl, event, argc)` reads `argc` texts from the answer region and dispatches through the `(ctrl, event) → entry` table. `pr_resume(task, len)`, `pr_cancel(task)` (with `-1` = discard a trapped run's half-built batch, A5), `pr_tick`, `pr_status` (A1), and `pr_close_form(inst) -> status` (A2: the region is released and reinitialised when the closing task is `DONE`, WASM-017 R5). There are also the optional `pr_dbg_live` / `pr_dbg_region_used` (A3) for WASM-017 R20.
- **Task table (A5):** one row per task holding `pc`, `$resume`, the `PERFORM` stack pointer (already in the region, `stack_off`), the owning instance and the status. Tasks of different instances interleave, and one instance's tasks are serialised by the host. The number of rows is `Σ instances + spare`, fixed at compile time (R3: no growth).
- **Imports:** add `query`, `random` and `remote` (with `now` and `abort`, which exist). A module imports exactly what its code reaches, as today's `glue_needed` does. The manifest `imports` lists them so that the loader can refuse a host missing one (R12/AC9; the refusal itself is the loader's, ABI §11 step 2).
- **Manifest** built as a `cobolt_wasm_abi::manifest::Manifest`, not hand-written JSON: `forms[]` (id, name, main, instances, region_size, controls, handlers, **layers** per A9), `names`, `events`, `builtins` (A7), `limits`, `sizes`. It must pass `Manifest::validate` and `scan::bundle_problems` for both variants. That one test proves the producer and the consumers agree.
- **`ERROR` (A4, R11):** `glue.abort` writes the code's **magnitude**, the real **unit**, **form** and **event** (globals set by `pr_event`), and the line from `$line`. COBOL run-time errors use numbers from 1000.
- **Line table (R14):** a custom section `prlines` (`line id → unit name, source line, column`) and `$line` exported as a global for diagnostics. Lines of generated form code map to their form and event the way Check reports them. *Not yet read:* how Check maps a generated line back to its form and event; `/tasks` has to locate that mapping and reuse it, not invent one. The `name` section already names every function and is kept.
- **Batching (R12, WASM-017 R11):** `rt_cmd_set`/`rt_cmd_call` already append to the batch, and `rt_cmd_query` flushes before a query. WP2 adds the flush at `DONE`, `SUSPEND`, `YIELD` and on a full batch. A test counts host calls per handler.

### WP3 — The subset table, completed (R7, R8, AC6; Q7, Q8, Q9)
- **Built-ins:** replace `browser_builtin(name) -> bool` with `builtin_support(name) -> Support`, one entry per `BUILTINS` name: *browser*, *backend only* (files, SQL, AWS, processes, indexed on disk) or *unsupported*. A test iterates `cobolt_runtime::builtins::BUILTINS` (already a dev-dependency) and **fails when a name has no entry**. That is R8's rule for built-ins, matching the exhaustive `match` that already holds it for `Stmt`. Per Q7, an entry that depends on a browser API is marked with the engines that have it. For an iOS target the Safari column decides, and otherwise the entry must exist in both Chrome and Safari.
- **`ACCEPT` (Q9 "ignored"):** `accept_` with no `FROM` (or a console mnemonic) in a non-conformance build emits **no code**. The receiving item keeps its value. `ACCEPT FROM DATE/TIME/DAY/DAY-OF-WEEK` keep working from the host clock. *Working assumption:* "ignored" still gets a **build warning** at the line, so the developer learns that the statement does nothing. The warning needs `Compiled.warnings` (§3). If the operator meant silent, the warning is dropped and nothing else changes.
- **In-memory INDEXED and `SORT`/`MERGE` (Q8):** `support()` becomes context-aware for the file verbs. `OPEN/READ/WRITE/REWRITE/DELETE/START/CLOSE` on a file whose `SELECT` has `StorageMode` *memory* (`cobolt-ast/src/program.rs`, `storage_mode`) compile to the module. So do `SORT`/`MERGE`/`RELEASE`/`RETURN` whose `USING`/`GIVING` are memory files or procedures. Disk files stay *backend only* with today's alternative text, and `WITH PERSISTENCE` is refused because it writes to disk. The engine is a new `cobolt-wasm-rt` module: a B-tree over a byte arena inside the **form instance's region**, sized at compile time from a new per-file maximum record count (§3), released with the instance (R7). Its semantics, including status codes, key order and duplicates, are held to the interpreter by differential tests. The interpreter's in-memory engine is the oracle; it isn't shared source, because it is `std`-based.
- **Diagnostics (R8):** every refusal carries the construct, the line and the alternative. `GenError` gains a typed **kind** so that the IDE can translate it (i18n, §2). The tools keep English text, as `NOT_YET_LINKED_MESSAGE` does.

### WP4 — Value methods and remaining phase-D gaps (R5, R7)
- **Value methods on data items** (`Trim`, `UpperCase`, `LowerCase`, `Replace`, `Len`, `Split`, …: exactly the list the interpreter accepts. *Not yet read:* where that list is defined; `/tasks` locates it so the generator takes it from the same place) lowered to rt string routines in `cobolt-wasm-rt/src/strings.rs`, as values (into `T_CTEMP`) and as `MOVE`/`SET`/`STRING` operands. They are differential-tested against the interpreter.
- **`NCTEMP` = 4** character temporaries per statement: kept, with the refusal naming the limit. *Risk noted in §5.* If a real form exceeds it, the limit is raised in `cobolt-wasm-rt/src/state.rs`. It is fixed memory either way.
- **Parity gaps** `WASM-PARITY.md` #7 (`FUNCTION MAX/MIN/SUM (t (ALL))` over `OCCURS DEPENDING ON`, solved with a table-slice argument carrying the count at run time), #9 (`BY VALUE` with the callee's `USAGE`, and `RETURNING` on a dynamic `CALL` through the callee table), #10 (`PROGRAM-ID … INITIAL`/`COMMON`, which needs the AST to **keep** them (§3), WASM-017 R4). Recursion stays refused (COBOL-85 doesn't allow it).
- **Layers (R19, R20, AC11):** `FormSpec` gains the form's layers in stack order. A layer's id is `3 + n_controls + k` (A9). `LAYER::Property` reads and writes go through the same path code as controls. A name that is not a layer of the form, or a property a layer doesn't have, is a build error naming the layer and listing its properties (`Name` read-only; `Visible`, `Transparency`, `BackgroundColor`, the gradient properties, `BackgroundImage`, `BackgroundImageMode` writable, per A9). A write to a control's `Layer` is refused. No handler is generated or dispatched for a layer.

### WP5 — Phase C: suspension and the remote `CALL` (R9, R10, AC5; ABI A5, A6, A7)
- **Waiting statements:** `CALL … WITH WEBSOCKET LINKAGE` (AST from WASM-005 M1, `cobolt-ast/src/stmt.rs` `WebSocket`), the suspending `COBOL::` verbs of ABI §12.1 (`MessageBox`/`Alert`/`Confirm`/`InputBox`, `Sleep`/`Wait`, `OpenFormSync`), and `me::OpenFormSync`. Each ends its block with `SUSPEND`, with `$resume` = the continuation block. The task row keeps everything, since registers never live across a suspension (`cobolt-wasm-rt/src/state.rs` doc).
- **Frames:** the `USING` group is encoded to the request frame in **frame-out** by an rt implementation of the WASM-005 mapping. On `pr_resume`, frame-in is decoded: `{"id","result"}` goes into the `RETURNING`/`BY REFERENCE` items, and `{"id","error":{class,…}}` (A6) raises `ON EXCEPTION`, or the run-time error of R11 when no `ON EXCEPTION` is present. **The rt mapping is held to `group-json.v1.json`** by a test that runs every vector through the module and through `cobolt-wire::mapping` (WASM-005 R5b).
- **Yield (R9):** the dispatcher's fuel check already returns `-2`. WP2 turns it into status `YIELDED` with the task row saved. A test runs one long `PERFORM` with budgets 1, 7 and 20 000 and requires identical results (R9 "not depend on where it yielded").
- **Two events while suspended (AC5):** the host queues them (ABI §4). The test host implements the queue, and the test checks data consistency after both.

### WP6 — Build link and report (R1, R15, AC9)
- `cobolt-wasm` gains `compile_web_project(…) -> WebModules { multi: Option<Compiled>, single: Option<Compiled>, warnings }`, which emits the variants `ModuleVariants` asks for (WASM-017 R9). Its input is each form's program as the desktop path builds it: forms regenerated by `cobolt-codegen` (the generated-code contract), parsed, and with `cobolt_compiler::link_common_code` applied.
- A `WebBuildBackend` implementation, `GeneratorBackend`, in `crates/cobolt-project-tools/src/web/generator.rs`, calls `compile_web_project`, then `cobolt_web_bundle::assemble_bundle`, and replaces the bundle atomically as the trait's documentation requires. It is linked once with `web::link(…)` where the IDE and `rcrun` start (the doc comment in `web/mod.rs` names the IDE's `main` and `rcrun`'s `mcp` command). `cobolt-project-tools` depends on `cobolt-wasm` with **`default-features = false`**, so no engine reaches the IDE through the generator. The start-up measurement needs one: see §4 D5.
- **R15:** `BuildReport.size.parts` already lists `program.wasm` / `program.single.wasm`. `startup_ms` is filled by instantiating the multi-memory module and running `pr_init` + `pr_open_form(main)` against a stub host, measured on `wasmtime` natively and **reported as that**: a native proxy, not a browser figure. Browser timings belong to WASM-017 R20 / WASM-015. No budget is set (Q6: measure phase A first). The numbers are recorded in the ledger.
- The bundle composition, the player artefact and the loader are **WASM-002/016's**. This plan only hands them the modules.

### WP7 — Conformance and parity (R13, R16, R17, AC7, AC8, AC10)
- **R16(a):** NC/IF/IC on `wasmtime` (WP1). Members outside the subset are listed with their construct by `nist_wasm census` (exists), recorded in the ledger in two numbers.
- **R16(b), AC8:** a new test `crates/cobolt-wasm/tests/corpus.rs` walks `tests/cobol/**` (21 folders today). For each program inside the subset it runs `same_output`. For each outside, it records the refusing construct. It prints compiled / matched / excluded-with-reason counts. A difference is an open entry in `WASM-PARITY.md`, never an `#[ignore]`.
- **R16(c), Q5:** each difference is classified in `WASM-PARITY.md`. An **interpreter** bug is fixed on **1.x.x** (`main`, a *fix* branch, GOLDEN RULE #5) and cherry-picked forward. That work is **separate from this feature's commits**: never mix fixes and features. The four open interpreter rows (#1, #2, #5, #8) are listed for that path. Under the operator's Q5 answer ("check whether the difference is a bug; if it is, fix it"), they are candidates for fixing, and each still needs its own fix-branch run and the NIST regression of every finished module.
- **R13:** a test runs the same event script twice per variant, with fixed `now`/`random` answers, and compares the command batches byte for byte.
- **R17:** the interpreter/binary-parity rule (`.claude/skills/interpreter-binary-parity/SKILL.md`, the steering and CLAUDE.md: *one rule lives in three files*) gains the generator as a consumer. It names the shared sources `cobolt-decimal` and `cobolt-numedit`, and `cobolt-wasm-rt` as the generator-only routines held equal by measurement.
- **AC10:** the interpreter, desktop and backend test suites stay green. `cobolt-runtime` is not touched by this plan, except the AST additions of §3, which are additive and `serde(default)`.

## 2. Affected crates / files

- `crates/cobolt-wasm/Cargo.toml`: `wasmi` → `wasmtime` (feature `host`, default on, dev and harness only); `cobolt-wasm-abi` (`manifest`, `batch`) as a dependency.
- `crates/cobolt-wasm/src/host.rs`: rewritten over `wasmtime`; form-driving API (`open_form`, `event`, `resume`, a scripted queue) for the tests and the start-up measurement.
- `crates/cobolt-wasm/src/module.rs`: form exports, task table, `query`/`random`/`remote` glue, manifest through `cobolt-wasm-abi`, `prlines`, exported `$line`, `ERROR` fields, `compile_web_project`.
- `crates/cobolt-wasm/src/support.rs`: `builtin_support`, context-aware file verbs, the typed refusal kind.
- `crates/cobolt-wasm/src/error.rs`: `GenError` gains `kind` and the warnings type.
- `crates/cobolt-wasm/src/lower.rs`, `lower_form.rs`, `lower_call.rs`, `lower_more.rs`: `ACCEPT` ignored, value methods, layers, suspending verbs, the WebSocket `CALL`, the in-memory file verbs, gaps #7/#9/#10.
- `crates/cobolt-wasm-rt/src/`: new `idx.rs` (in-memory INDEXED + SORT/MERGE), new `json.rs` (group ⇄ JSON per WASM-005), `strings.rs` (value methods), `state.rs` (task rows); `assets/rt.wasm` + `rt.sources.hash` regenerated (`build_rt`).
- `crates/cobolt-wasm/tests/`: `corpus.rs`, `forms.rs` (AC1, AC5, AC9, AC11), `layout_golden.rs` (AC2), `arith_table.rs` (AC3), `control_flow.rs` (AC4), `subset_table.rs` (AC6), `determinism.rs` (R13), `json_vectors.rs` (WASM-005 R5b), `dependency_guard.rs` (WP1).
- `crates/cobolt-ast/src/program.rs`: `is_initial`, `is_common` on the program; a maximum record count on a memory file's `SELECT` (§3). `crates/cobolt-parser`: keep `INITIAL`/`COMMON` instead of dropping them; parse the maximum record count. `crates/cobolt-semantic`: validate it.
- `crates/cobolt-project-tools/src/web/generator.rs` (new) and `web/mod.rs` (`mod generator`); `Cargo.toml` adds `cobolt-wasm` (`default-features = false`) and `cobolt-web-bundle`.
- `crates/cobolt-ide/src/main.rs` (or where the IDE starts) and `crates/cobolt-cli` (`mcp`, `build`): one `web::link(Arc::new(GeneratorBackend))` each.
- `crates/cobolt-ide/src/i18n.rs`: new `Tr` keys ×6 (EN/ES/PT/JA/ZH/FR) for the refusal kinds (backend-only construct, unsupported construct, unknown layer, unknown layer property, write to `Layer`, too many host reads in one statement, `ACCEPT` ignored warning). `pk_build_done` already carries size and start-up. `crates/cobolt-ide/src/panels/web_preview.rs` translates `Refused` entries by kind.
- `crates/cobolt-compiler/src/lib.rs` (System KB, in `system_documentation()`): a section **"What runs in a browser module"**: the statement table, the built-in table and the in-memory file rule. It is generated from `cobolt_wasm::support`, so the doc can't drift. A test asserts that every `BUILTINS` name appears. Then `cargo run -p cobolt-ide --example build_chunked_kb` and commit `assets/knowledge/chunked.data`.
- `.claude/skills/interpreter-binary-parity/SKILL.md`, `specs/steering/tech.md` (+ CLAUDE.md via the local-settings copy): R17.
- `specs/WASM-PROGRESS.md`, `specs/WASM-PARITY.md`: measured numbers and divergences.
- Root `Cargo.toml` / `Cargo.lock`: the `wasmtime` addition, **additions only** (spec 103 AC8 style).
- **Not touched:** `README.md`, anything under `docs/` (2.x.x rule), `cobolt-runtime`'s behaviour.

## 3. Data / model changes

- **Module format (ABI v1, no version bump):** project modules now carry what ABI §4/§10 and A1–A9 already specify (exports, `prmanifest` with `forms`/`names`/`events`/`layers`/`builtins`, `prlines`, `$line`). Nothing incompatible. If a rule of `WASM-ABI-v1.md` proves incomplete, the amendment goes into its §12 table **in the same commit** as the code that needs it, as A1–A9 did.
- **AST (additive, `#[serde(default)]`):** `Program.is_initial: bool`, `Program.is_common: bool` (PARITY #10, WASM-017 R4); a `max_records: Option<u32>` on a `STORAGE IS MEMORY` file description, needed to size the fixed memory (R3). **Bincode:** the AST is serialised by `cobolt-compiler` and decoded by the runtime of the same build, so an added field changes the format only between builds. The existing `AST_MISMATCH_HELP` path covers a stale binary. The interpreter's NIST baselines are rerun to prove no behavioural change (GOLDEN RULE #9 regression).
- **Syntax (working assumption, to confirm at `/tasks`):** the maximum record count of an in-memory file is written `RECORD LIMIT IS n` (or a `.cidx` property for files defined there). When it is absent, a browser build refuses the file with "an in-memory file in a browser module needs a record limit", and the desktop is unaffected. This is a new PowerRustCOBOL extension clause, so lexer, parser, semantic analyser, KB and IDE highlighting all change together.
- **`cobolt_wasm::Compiled`:** gains `warnings: Vec<GenWarning>` and `lines` (the `prlines` table, for the IDE). `FormSpec` gains `layers`.
- **`.cfrm`, `cobolt.toml`, generated `.cbl`:** unchanged. `ModuleVariants` and `[web]`/`[mobile]` already exist (`cobolt-compiler/src/project_kind.rs`).

## 4. Key decisions & alternatives

- **D1: keep the existing architecture (WASM-PLAN P1–P5)** and finish it. *Why:* it is built and NIST-measured on three modules. *Rejected:* restarting any piece.
- **D2: `wasmtime` replaces `wasmi` in `cobolt-wasm`'s host** (operator, Q2). *Why:* binding. *Rejected:* keeping both behind a trait. That means two engines to keep green for no requirement; the trait would be speculative. *Guarded by:* the WP1 probe; failure stops and asks.
- **D3: the manifest is produced through `cobolt-wasm-abi::manifest`** and checked with `bundle_problems`. *Why:* one definition for the producer and the consumers. *Rejected:* `serde_json::json!` as today, which already drifted (`forms: []`).
- **D4: in-memory INDEXED is a new `no_std` routine in `cobolt-wasm-rt`**, held equal by differential tests. *Why:* the interpreter's engines (`cobolt-indexed`, `redb`) are `std` and file-backed; the module must stay allocator-free with fixed memory. *Rejected:* compiling `cobolt-indexed` to `wasm32`, which pulls `std`, `redb` and the allocator, against R6 "no `redb`".
- **D5: start-up is measured natively on `wasmtime`, in the Build, behind the build backend.** *Why:* R15 asks the Build for a number, and the Build runs natively. *Cost:* the IDE gains `wasmtime` through `cobolt-project-tools` → `cobolt-wasm/host`. *Alternative kept open:* report `startup_ms: None` from the Build and let the Preview measure in the web view. *Working choice:* measure in the Build only if WP1's probe shows that `wasmtime`'s weight in the IDE is acceptable; otherwise `None` plus the Preview's figure. This is flagged in §8 rather than decided silently.
- **D6: refusals are typed** (`kind` + English text). *Why:* six-language IDE diagnostics (spec §6) without translating free text. *Rejected:* English-only diagnostics in the IDE.
- **D7: the KB table is generated from `support.rs`.** *Why:* "a test fails when a variant or built-in lacks an entry" then covers the documentation too. *Rejected:* a hand-written table.
- **D8: interpreter bugs found by parity are 1.x.x fixes on their own branch**, never part of this feature's commits (CONVENTIONS "never mix fixes and features").

## 5. Risks & mitigations

- **`wasmtime` fails the guards** (MSRV > 1.92, a `cc` build script, weight). → Probe first, stop and ask. No silent fallback.
- **Engine change moves NIST numbers** (JIT vs interpreter: traps, stack depth, float NaN bits). → Rerun all three modules before anything else lands; NaN canonicalisation on; every difference goes into the ledger.
- **The 20 s per-program harness limit** (ledger) under `wasmtime` compile time per module. → Measure; cache compiled modules per run if needed (`Module::serialize` is harness-only).
- **Fixed memory vs in-memory files:** a record limit is new syntax and a new failure mode. → Refuse clearly at build time; the desktop is unaffected; confirm the clause at `/tasks`.
- **Task table size** (A5) fixed at compile time: a project with deep `OpenFormSync` chains could run out. → `Σ instances + spare`, a run-time error naming the form (WASM-017 R2's message) instead of a trap; the spare count goes in the manifest.
- **`NCTEMP` = 4** host reads per statement can refuse a real handler. → Measure on the examples' web ports during WP6; raise the constant if needed (memory is fixed either way).
- **The rt blob gets stale** after routine edits ("the routine blob has no export rt_…"). → `tests/rt_blob.rs` already fails on a stale hash; `build_rt` is a step in every task that touches `cobolt-wasm-rt`.
- **Two engines drift** (the spec's own warning). → The corpus test (R16b) and NIST rerun on every routine change; `WASM-PARITY.md` forbids "accepted" differences.
- **Disk:** a workspace test sweep links ~190 binaries. → Targeted `cargo test -p cobolt-wasm`; `df` before sweeps; clone `target/` with `cp -cRp`.
- **Concurrent sessions on `features-2x`:** version/CHANGELOG collisions. → Hunk-level staging; the CHANGELOG gap check from the ledger's operating rules.

## 6. Test strategy

Every test prints a quantified summary line (tech steering "new tests report quantified results").

| AC | Test (crate `cobolt-wasm` unless named) | Asserts / reports |
|---|---|---|
| AC1 | `tests/forms.rs::button_handler_move_add_display` | a two-control form compiled with `compile_project`; `pr_open_form` + `pr_event(click)` on the test host give the expected `SET` commands and `DISPLAY` text; `scan_module` import list ⊆ ABI §3; both variants; reports commands / host calls |
| AC2 | `tests/layout_golden.rs` | memory image after `VALUE` init and after a sequence of `MOVE`s, byte for byte, for `REDEFINES`, `OCCURS DEPENDING ON`, `COMP-3`, signed DISPLAY (trailing, `LEADING`, `SEPARATE`); golden hex tables in the test; reports bytes compared |
| AC3 | `tests/arith_table.rs` | ≥ 1 table row per `ROUNDED` mode × verb, `SIZE ERROR`, 18- and 38-digit operands, `DIVIDE … REMAINDER`; module vs interpreter through `same_output`; reports rows / equal |
| AC4 | `tests/control_flow.rs` | `GO TO … DEPENDING`, `ALTER`, `PERFORM … THRU` with `EXIT PERFORM`; both engines; both variants |
| AC5 | `tests/forms.rs::suspend_resume_with_queued_events` | a handler doing a WebSocket `CALL` returns `SUSPENDED`; two events are queued; `pr_resume` with a result frame, then an A6 error frame (→ `ON EXCEPTION`); final data as expected; plus the budget-independence run (R9) |
| AC6 | `tests/subset_table.rs` | a program with `OPEN` on a disk file, `EXEC RUST`, `EXEC SQL` → three diagnostics with line and alternative; every `BUILTINS` name has a `builtin_support` entry (count printed: N of 68) |
| AC7 | `examples/nist_wasm` + `nist_conformance run NC/IF/IC` with `RCRUN=…/nist_wasm_run` | compiled / passed / excluded-with-reason per module, 0 failures; recorded in `WASM-PROGRESS.md` |
| AC8 | `tests/corpus.rs` | `tests/cobol/**`: compiled / identical / excluded (reason), differences → `WASM-PARITY.md` |
| AC9 | `tests/forms.rs::manifest_and_imports` + `cobolt-project-tools` build test | `Sizes.module_bytes` and `startup_ms` (or `None`, per D5) present in `BuildReport`; a host missing `query` fails instantiation with the missing import named |
| AC10 | `cargo test -p cobolt-runtime -p cobolt-parser -p cobolt-semantic -p cobolt-server -p cobolt-compiler` + interpreter NIST NC/IF/IC/SQ/IX/RL/ST/SM | unchanged counts |
| AC11 | `tests/forms.rs::unknown_layer_refused` | `LAYER-9::Visible` on a one-layer form → build error naming the layer and listing its properties |
| R13 | `tests/determinism.rs` | two runs of one event script per variant, batches byte-identical |
| R14 | `tests/forms.rs::runtime_error_has_line` | divide by zero without `ON SIZE ERROR` → `ERROR` with code ≥ 1000, the COBOL line, form and event; the next event still runs |
| WASM-005 R5b | `tests/json_vectors.rs` | every vector of `group-json.v1.json` through the module equals `cobolt-wire::mapping` |
| WP1 | `tests/dependency_guard.rs` | `wasmtime` graph: licence set, MSRV ≤ 1.92, no C-compiling build script; shown to fail on a planted package |
| KB | `cobolt-compiler` test + `prebuilt_chunked_kb_matches_the_published_documentation` | the browser-subset section lists every statement and built-in; the store is fresh |

The player's existing native ABI test (`crates/cobolt-web-player/tests/abi_host.rs`) is pointed at a **generated** module as well as the fixtures. That makes it the cross-check that the producer and the consumer agree.

**Manual check:** with WP6 linked, create a Web project in the IDE, add a button whose handler sets a Label's text, and press Build. The Output panel shows the size and the start-up in the IDE's language. Preview shows the page, and clicking the button changes the label. The operator judges the look (memory: "never drive the application's window"). I make no claim about a browser run that wasn't done.

## 7. Steering compliance

- [ ] **i18n:** the new `Tr` keys (refusal kinds, `ACCEPT` warning, layer errors, read limit) in all six languages; tools stay English as the `NOT_YET_LINKED_MESSAGE` pattern does.
- [ ] **Generated-code contract:** the module is built from the regenerated `.cbl` (forms regenerated on Build), never hand-edited; no banner change.
- [ ] **Docs:** **not touched** on 2.x.x (`README.md`, `docs/`). What 2.x.x says about itself goes in `specs/` (ledger, parity, ABI amendments) and in the **System KB** (compiler doc section + regenerated `chunked.data`).
- [ ] **System KB:** the browser-subset section and the in-memory file clause, regenerated store committed in the same change.
- [ ] **Interpreter/binary parity (R17):** the skill, steering and CLAUDE.md name the generator and the shared sources; `cobolt-runtime` behaviour unchanged.
- [ ] **NIST (GOLDEN RULE #9):** two numbers per module; any interpreter change reruns every finished module.
- [ ] **PRIME DIRECTIVE:** Rust only; no generated JS/HTML enters the repo from this spec (the loader is WASM-002's); throwaway scripts stay outside the tree.
- [ ] **User code is sacred:** `ACCEPT` is ignored, not removed from the source; the build never rewrites developer code.
- [ ] **Dependencies:** `wasmtime` probed and guarded (WP1); `Cargo.lock` diff is additions only.
- [ ] **Fix vs feature:** this is a **feature** on `features-2x`; each commit bumps `z` in `crates/cobolt-ide/src/version.rs` (2.0.z) with a top `CHANGELOG.md` entry, inserted above the previous heading. Interpreter bugs are **fixes** on 1.x.x, in their own commits.
- [ ] **No "cobolt" in user-facing text:** diagnostics say "browser module" / "WebAssembly module"; COBOL identifiers in examples and tests are English.

## 8. Open questions (none decided here)

- **Q3 — how the routines are authored. Not answered.** *Working assumption:* option (a). It is what the code already does (`cobolt-wasm-rt` → `rt.wasm`, linked and tree-shaken; `cobolt-decimal` and `cobolt-numedit` shared with the interpreter), it is NIST-measured, and it is what WASM-PLAN P1 chose. Nothing in this plan depends on changing it. If the operator chooses (b), WP3–WP5's new routines would be emitted by the compiler instead, and the existing ones rewritten.
- **Q5 — data-model parity. Answered, to confirm.** The plan applies the first half: a difference is checked, and an interpreter bug is fixed on 1.x.x (§1 WP7). *What "avoid `REDEFINES`" means for the generator is not decided.* *Working assumption:* when a `REDEFINES` difference turns out **not** to be a bug in either engine (implementation-defined behaviour, such as reading invalid packed data through a redefinition), the generator compiles it as the standard says and **Check warns at the `REDEFINES`** that the result may differ from the desktop. It does not refuse, because R5/R7 say a program that compiles isn't refused. The operator should confirm or say "refuse".
- **Q-new-1 — the in-memory file's record limit** (§3): the clause's spelling, and whether a `.cidx` property is enough. Needed before WP3's in-memory files.
- **Q-new-2 — does "`wasmtime` as the engine" cover the player's native ABI tests** (`cobolt-web-player/tests/abi_host.rs`, `wasmi` today)? *Working assumption:* only `cobolt-wasm`'s host and the NIST gate move; the player's tests stay on `wasmi` unless the operator says otherwise.
- **Q-new-3 — start-up measurement in the Build** (D5): accept `wasmtime` in the IDE's dependency graph, or report `None` from the Build and measure in the Preview.
- **Q9 detail — `ACCEPT` ignored: warn or silent.** *Working assumption:* warn.
- **D11 in `WASM-INDEX.md`** still says "Open" although Q5 now has an answer to confirm. The index is not edited by this run (only `plan.md` is written); it is listed for whoever updates the index.
