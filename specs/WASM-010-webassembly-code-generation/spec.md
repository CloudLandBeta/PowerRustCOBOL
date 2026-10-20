<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — WebAssembly code generation: the compiler emits the module, and the module carries what it needs

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-010-webassembly-code-generation/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the operator's answers of 2026-10-19 to the first batch: *no redb in the browser; the backend keeps the current runtime, with redb and `std::fs`, plus what the new functions need; threads are
  not needed in the browser*; asked *"can we generate WebAssembly directly?"* and said *"follow the plan to generate WebAssembly"*; and, on this spec: ***"there will be no runtime — the code is generated as
  WebAssembly by the compiler to run directly in the browser; the final code must incorporate what it needs to run."***
- **Depends on:** WASM-002. **Resolves:** WASM-002 Q1 (the execution model) and the "browser runtime" gap WASM-INDEX had listed as 010. **Needed by:** WASM-004, WASM-005 (suspension), WASM-008, WASM-015.
- **Prior art:** spec 006 (removed 2026-10-14) wanted COBOL transpiled to Rust and no interpreter shipped. This spec keeps *no interpreter* and drops *via Rust*.

## 1. Overview

A Web or Mobile application's COBOL is compiled by the compiler **directly into WebAssembly**. The output is a **self-contained module**: there is no interpreter, no AST and no separate runtime library in
it, and it does not depend on a runtime package being present in the browser. The routines the program needs — exact decimal arithmetic, numeric-edited pictures, string handling, intrinsic functions — are
**emitted into the module by the compiler, and only the ones this program uses**. The module's only contact with the outside is a short, versioned **host API** (reading and setting a control's property,
calling its method, asking for the time, making a remote call), which the page that hosts it implements; that is the browser boundary, not a runtime.

The **backend does not change its engine.** It keeps the current native runtime — tree-walking interpreter, `std::fs`, threads, the indexed engines including `redb`, SQL, HTTP — and gains what WASM-003 and
WASM-005 add to it. So the system has two engines for one language: the generator for the browser, the interpreter for the server and the desktop. Keeping them in step is a first-class requirement here (R16),
not a hope.

### Why generate directly (the operator's question, answered)

Yes: WebAssembly is an ordinary binary format with a small instruction set, and the AST the compiler already holds (`cobolt-ast`) is a sufficient input. The reasons to do it, rather than interpret in the browser
or go through Rust:

- **No Rust toolchain is needed to build the COBOL.** A `cargo build` of a generated crate takes minutes and needs `rustc`; emitting a module takes seconds.
- **A small, sandboxed artefact.** The module has no imports except the host API, so it cannot touch anything the host does not offer; its size is the program plus the routines it uses.
- **Suspension is natural.** A remote `CALL` must pause the program until the answer arrives (WASM-005 R9) and a browser cannot block. Code the compiler writes itself can keep its position and its `PERFORM`
  stack in linear memory and be resumed, with no dependence on browser features that are not everywhere.
- **`GO TO` and `ALTER` are ours to lower,** which a Rust transpiler would have to fight (Rust has no `goto`).

What it costs: a compiler back end and the routines above, and **two engines to keep equal**. That is why R16 makes conformance a measured gate, using the NIST CCVS85 suite the project already runs.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **The input is there.** `cobolt-ast` holds one `Stmt` variant per statement (`CALL` is `Stmt::Call`, `cobolt-ast/src/stmt.rs:802`), data declarations (`DataDecl`) and a `ProcedureDivision` with
  `USING` and `RETURNING` (`program.rs:275`). The compiler already serialises the AST (bincode + gzip, `cobolt-compiler/src/lib.rs:2259-2261`); **nothing emits code from it**: the program runs in the interpreter.
- **The interpreter's data model is value-based, not byte-based.** Storage is `IndexMap<String, CobolValue>` (`cobolt-runtime/src/environment.rs:129`); `CobolValue` is `Numeric` (an `i128` mantissa with a decimal
  count, about 38 digits, `value.rs:33-41`), `Float(f64)`, `String { bytes, capacity }` or `Unset` (`value.rs:333-345`). **A group has no bytes of its own**, `REDEFINES` is a write-driven refresh between separate
  slots (`environment.rs:972`), and `COMP`/`COMP-3` are held as values. A byte image exists only for files and record moves (`RecordLayout`, `files.rs:63`; `group_bytes`, `display_bytes`, `set_group_bytes`).
- **The routines to embed exist as Rust** — exact decimal arithmetic on `CobolNumeric` (`value.rs`), numeric-edited pictures in `crates/cobolt-forms/src/numedit.rs` (677 lines; **in `cobolt-forms`, not in the
  runtime**), intrinsic functions in `cobolt-stdlib`. They are the behaviour the module must match.
- **No emitter and no engine in the workspace.** `wasm-encoder` and `wasmparser` appear in `Cargo.lock` (transitively); `wasmtime`, `wasmi`, `walrus` and `binaryen` do not. There is no `wasm32` build of anything
  (WASM-002).
- **A conformance harness exists and is finished for the modules that matter here:** `NIST/progress.json` (measured at 1.70.12, 2026-09-14) lists **NC** (Nucleus), **IF** (Intrinsic Function), **IC**
  (Inter-Program Communication), **SM**, **ST**, **SQ**, **IX** and **RL** as *finished* — every in-scope program compiles and runs with 0 failures in its own report — **DB** compile-only, **SG** out of scope.
  GOLDEN RULE #9 (finish one module before the next) is how it was worked.
- **ALTER and `GO TO`** are in the interpreter (`alter_map`, `interpreter.rs:1908`, `:5736`), so the semantics to reproduce are defined by running code.

## 2. Goals / Non-goals

- **Goals:**
  - A compiler back end that turns the browser-side COBOL of a project into one **self-contained WebAssembly module**.
  - A byte-accurate data layout and exact decimal arithmetic, the same results as the interpreter.
  - Every COBOL-85 control-flow form, including `GO TO` and `ALTER`.
  - A module that can **pause and resume** at a call that waits.
  - A short, versioned host API as the module's only imports.
  - A **conformance gate**: the NIST programs in the subset, and a golden corpus, run on the generated module with 0 failures.
- **Non-goals:**
  - File I/O of any organisation (sequential, line sequential, indexed, relative), the `redb` engine, `std::fs`, threads, child processes, native sockets, SQL and `EXEC SQL`, `EXEC RUST`. In the browser the
    data lives on the backend (WASM-005).
  - A runtime, interpreter or AST in the module.
  - Compiling the backend: it keeps the native runtime.
  - WebAssembly GC, the component model, threads or SIMD proposals in v1.
  - Optimising beyond a straightforward, correct lowering.

## 3. User stories

- As a developer, I want **Build** to give me a WebAssembly module in seconds without installing a Rust build chain for my COBOL, so that the web loop is fast.
- As a developer, I want my program to compute **exactly** what it computes on the desktop, so that a total never differs by a cent between the browser and the server.
- As a maintainer, I want the NIST programs that fit the browser to pass on the generated module, so that "the same language" is measured and not assumed.
- As a security reviewer, I want a module whose imports I can list on one page, so that I know everything it can do.

## 4. Requirements (EARS)

**The module**
- **R1 (event):** When the Build of a Web or Mobile project compiles its browser-side COBOL — the generated form programs, event handlers, user procedures and the Common Code they `CALL` — the compiler shall emit **one
  WebAssembly module** directly from the AST, with no Rust compilation of that COBOL, and with no interpreter, AST or runtime package in the output.
- **R2 (ubiquitous):** The module shall be **self-contained**: it shall import only the host API (R12) and shall carry, emitted into it, every routine it needs to run and no routine it does not use. Its exports shall be
  the entry points of R10, R9 and its memory.
- **R3 (ubiquitous):** Every `WORKING-STORAGE`, `LOCAL-STORAGE` and `LINKAGE` item shall live at a fixed place in linear memory with a **byte-accurate layout**: `PICTURE` and `USAGE` (`DISPLAY`, `COMP`/`COMP-5`,
  `COMP-3`, `COMP-1`/`COMP-2`, national), sign position (trailing overpunch, `SEPARATE`, `LEADING`), `OCCURS` including `DEPENDING ON`, groups as contiguous bytes, `REDEFINES` sharing the same bytes, `FILLER`, `VALUE`
  initialisation and `INITIALIZE`.
- **R4 (ubiquitous):** Arithmetic shall be **exact decimal fixed-point** to at least the standard's 18 digits and to the width the interpreter supports today (an `i128` mantissa, about 38 digits); `ADD`,
  `SUBTRACT`, `MULTIPLY`, `DIVIDE` and `COMPUTE` with `GIVING`, `REMAINDER`, every `ROUNDED` mode and `SIZE ERROR` shall give the interpreter's results (R16).
- **R5 (ubiquitous):** Control flow shall cover paragraphs and sections, `PERFORM` in every form (simple, `THRU`, `TIMES`, `UNTIL`, `VARYING`, inline, `TEST BEFORE`/`AFTER`, `EXIT PERFORM`/`CYCLE`), `GO TO` (with
  `DEPENDING`), **`ALTER`**, `EVALUATE`, `IF` and `NEXT SENTENCE`, nested programs, and `CALL`, `CANCEL`, `GOBACK`, `EXIT PROGRAM` and `STOP RUN`. No program that compiles shall be refused for using `GO TO` or `ALTER`.
- **R6 (ubiquitous):** The routines emitted into a module — decimal arithmetic including the 128-bit operations WebAssembly lacks, numeric-edited pictures, character and string handling for `STRING`, `UNSTRING`,
  `INSPECT` and reference modification, intrinsic functions, date and time arithmetic — shall have **one source of truth with the interpreter's** where the same behaviour is required, so that a fix to one reaches
  both (Q3 on how). They shall contain no file, thread, process, `redb` or `std::fs` code.
- **R7 (ubiquitous):** The subset the generator accepts shall be all COBOL-85 statements that need no file, plus the PowerRustCOBOL extensions that do not need the host machine: a control's property read and written like a
  data item (`CTRL::Property`), method calls and method values, the value methods of data items (`Trim`, `UpperCase`, `Replace`, `Len`, …), expressions in `MOVE`/`SET`/`STRING`, and the inline `COBOL::"…"` built-ins
  that are browser-capable. Each built-in (`cobolt-runtime/src/builtins.rs` `BUILTINS`) shall be classified *browser*, *backend only* or *unsupported* in one table.
- **R8 (event):** When the compiler meets a construct outside the subset, it shall report an **error at its line** naming the construct and the alternative (the rule of WASM-002 R3); a test shall fail when a `Stmt`
  variant or a built-in has no entry in the table.

**Behaviour**
- **R9 (state):** While a call waits for an answer — a remote `CALL` (WASM-005), a dialog, a timed wait — the module shall be able to **stop, return control to the host and later resume at the same point**, with its
  position, its `PERFORM` stack and its data kept in linear memory; it shall not depend on a browser feature that is not available in every supported browser (Q4).
- **R10 (event):** When the host reports an event (an event id, a control id, parameters), the module shall run the bound handler **to completion or to the next suspension**, applying what the handler changes to the
  host through the host API as it goes, and shall report when it has finished.
- **R11 (event):** When a run-time error occurs — a subscript outside its table, a division by zero without `ON SIZE ERROR`, invalid numeric data, an unresolved `CALL` — `ON SIZE ERROR` / `ON EXCEPTION` shall behave as
  in the interpreter; an unhandled error shall end the handler, be reported through the host API with the COBOL line (R14), and leave the application running.
- **R12 (ubiquitous):** The host API shall be a **short, versioned list**, each entry with one documented meaning: get and set a control's property, call a control's method, write `DISPLAY` output to the host's
  console, read the clock and a random number, and make a remote call. The page implements it (WASM-002); a module built for API version *n* shall refuse to start against a host that does not offer it.
- **R13 (ubiquitous):** Apart from those host calls, the module shall be **deterministic**: the same events and the same answers give the same results. The conformance harness relies on it.
- **R14 (ubiquitous):** The module shall carry a **name section and a line table** mapping its instructions to COBOL source lines (generated and handler code mapped to their form and event, as Check reports them),
  enough for R11's messages and for the debugging spec proposed in WASM-INDEX.
- **R15 (ubiquitous):** The Build shall report the module's size and a start-up time. No limit is set here (Q6).

**Conformance**
- **R16 (ubiquitous):** The generated module and the interpreter shall be held equal by measurement:
  (a) the NIST CCVS85 programs of modules **NC**, **IF** and **IC** that fall inside the subset shall compile to modules and run on a headless WebAssembly engine reporting **0 failures** in their own CCVS reports,
  and each program outside the subset shall be listed with the construct that excludes it, as the ledger does; (b) the programs of `tests/cobol/` inside the subset shall print the same output on both engines;
  (c) where they differ, one has a bug — fixed on 1.x.x if the interpreter is wrong (and forward-ported), here if the generator is wrong; a difference is never documented away.
- **R17 (ubiquitous):** The interpreter/binary-parity rule (a runtime fix reaches every host that builds the interpreter) shall list the generator and its emitted routines as a further consumer, and name the routines
  that are shared source.
- **R18 (ubiquitous):** The work shall land in phases, each usable: **A** — layout, `MOVE`, arithmetic, `IF`, `PERFORM`, `GO TO`, `EVALUATE`, the host API for properties and methods, events; **B** — tables, `STRING`/
  `UNSTRING`/`INSPECT`, numeric-edited, intrinsic functions, nested programs and local `CALL`; **C** — suspension and the remote `CALL` with the JSON mapping (WASM-005); **D** — the rest of the subset. Each phase
  shall record in the NIST ledger style what compiles and what passes. Following GOLDEN RULE #9, **NC** is finished before **IF** and **IC** are started.

## 5. Acceptance criteria

- [ ] **AC1** — A form whose button handler does `MOVE`, `ADD` and `DISPLAY` compiles to a module that, run on a headless engine with a stub host, produces the expected property writes and output; its import section lists only the host API.
- [ ] **AC2** — A golden table of data layouts (a group with `REDEFINES`, `OCCURS … DEPENDING ON`, `COMP-3`, signed display numerics, `SEPARATE` signs) shows the memory image after initialisation and after `MOVE`s equal, byte for byte, to the expected image.
- [ ] **AC3** — A table of arithmetic cases (rounding modes, `SIZE ERROR`, 18- and 38-digit operands, division with remainder) gives the interpreter's results on the module.
- [ ] **AC4** — A program using `GO TO … DEPENDING`, `ALTER` and `PERFORM … THRU` with `EXIT PERFORM` runs identically on both engines.
- [ ] **AC5** — A handler that makes a call that waits stops at it, the host resumes it later with an answer, and the final data equals the expected; two events raised meanwhile are queued and leave the data consistent.
- [ ] **AC6** — A program with a file verb, an `EXEC RUST` block or an `EXEC SQL` statement fails to compile with the line and the alternative; a test fails if any `Stmt` variant or built-in lacks a table entry.
- [ ] **AC7** — The NIST numbers for NC, IF and IC are recorded (compiled, passed, excluded with reasons), with 0 failures among those run.
- [ ] **AC8** — The `tests/cobol/` corpus inside the subset prints the same output on both engines; any difference is an open bug entry.
- [ ] **AC9** — The Build reports module size and start-up time; a module run against a host without a required API entry refuses to start.
- [ ] **AC10** — The desktop build, the interpreter and the backend are unchanged: their existing tests stay green, and the backend still opens `redb` and files.

## 6. Constraints & steering check

- **i18n:** yes — the diagnostics and the Build report are IDE strings, six languages.
- **Generated-code contract:** the module is a build artefact downstream of the generated `.cbl`, never hand-edited.
- **System KB:** the table of R7/R8 — which built-ins and statements exist in the browser — is knowledge the agents need; it goes into the compiler's documentation tables and the regenerated `chunked.data`.
- **Interpreter/binary parity (2026-09-07 ruling):** R17.
- **NIST (GOLDEN RULE #9):** conformance is reported per module and as two numbers, never as a compile score standing for a run; a fix that changes the interpreter reruns every finished module's execution.
- **Tech steering:** pure-Rust, MSRV 1.92; any emitter or test-engine crate goes through the dependency guards (spec 063 AC8 as amended by spec 103) and is probed at `/plan`.
- **Docs:** `docs/` untouched on 2.x.x.
- **Fix vs feature:** **feature** (`features-2x`). Bugs R16(c) finds in the interpreter are **fixes** on 1.x.x.

## 7. Open questions

- **Q1 — Confirm "directly".** The operator asked whether it is possible; this spec says yes and recommends it. If instead the generator should go through Rust, R1, R2 and R6 change and a Rust toolchain returns to the build.
- **Q2 — The emitter and the test engine.** `wasm-encoder` (a pure-Rust emitter already in the lock file transitively) or a small writer of our own; and a pure-Rust engine for the headless runs (`wasmi`) or `wasmtime`. Probe licence,
  MSRV 1.92 and weight at `/plan`.
- **Q3 — How the routines are authored, given "no runtime".** (a) Written once in Rust, compiled to WebAssembly when the *product* is built, kept in the compiler as a blob, and **linked statically into each module with the unused
  functions dropped** — one source of truth with the interpreter, 128-bit arithmetic comes with it; (b) written in the compiler as code that emits each routine on demand — no blob, but a second implementation to keep equal.
  Recommended: (a). Either meets R2: nothing is loaded at run time.
- **Q4 — Suspension technique.** Explicit state kept by the generated code (recommended; works in every browser) against JavaScript Promise Integration or stack switching (not in every browser yet) against a post-processing
  tool such as Binaryen's Asyncify (an extra build dependency).
- **Q5 — Data-model parity.** R3 asks for real bytes; the interpreter keeps values. Where they differ (`REDEFINES`, group moves over `COMP-3`), the generator follows the standard and the NIST suite, and a difference
  found is an interpreter bug (R16c). Confirm this is the intended direction.
- **Q6 — Budgets** for module size and start-up: measure phase A before setting them.
- **Q7 — Which built-ins are browser-capable** — decided per entry of `BUILTINS` at `/plan`.
- **Q8 — In-memory indexed files and in-memory `SORT` in the browser.** Out of v1 (non-goal), as the operator's "no `redb`, data on the backend" implies; confirm, since spec 006 had memory-only indexed files.
- **Q9 — `DISPLAY` and `ACCEPT`:** `DISPLAY` goes to the host's console (R12); `ACCEPT` has no meaning where forms are the input — refuse it or map it to a dialog?
- **Q10 — One module or two.** The COBOL module is self-contained, but the user interface is Leptos (Rust). Is the final artefact **one** `.wasm` (the COBOL object linked into the Leptos build — needs a Rust build and a
  linker step per project) or **two** (a prebuilt Leptos "player" that renders the forms, plus the generated program, joined by the host page — no Rust build per project)? Recommended: two. See WASM-002 Q7.
