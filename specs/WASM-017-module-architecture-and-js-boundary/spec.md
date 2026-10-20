<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Module architecture, memory model and the JavaScript boundary

- **Status:** draft (needs `/clarify`; D15 in the index lists what needs a ruling)
- **Folder:** specs/WASM-017-module-architecture-and-js-boundary/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-20
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the operator's guidance of 2026-10-20, six topics: *(1)* a modular architecture with on-demand instantiation and prompt disposal (Component Model, ES modules); *(2)* the JS ⇄ Wasm boundary — JS String Builtins and a separate memory for raw data;
  *(3)* explicit lifecycles for Rust objects (`.free()`); *(4)* avoiding the `memory.grow` trap with multiple memories; *(5)* coarse-grained calls and native concurrency (threads, Rayon); *(6)* a size-first release profile in `Cargo.toml` and `wasm-opt`. The operator asked where
  it belongs, or whether it needs a spec of its own, and asked for the earlier specs to be adjusted.
- **Why a spec of its own.** The topics cut across four specs — the generated module (WASM-010), the player (WASM-016), the bundle and loader (WASM-002) and the navigation that decides when a form appears and goes (WASM-011) — and none of them is the natural home. This spec owns
  the architecture of the WebAssembly artefacts; the others cite it.
- **Depends on:** WASM-002, WASM-010, WASM-016. **Adjusts:** WASM-002, 010, 011, 013, 015, 016 (listed in §6).

## 1. Overview

The guidance asks for a small initial footprint, memory that goes away when its owner does, a thin and cheap boundary between JavaScript and WebAssembly, and a lean build. Most of it fits this product — but each topic was checked against what the product
decided before and against what browsers offer today, and several points had to be **adapted** or **left out**. The disposition of each:

| # | Topic | Disposition | Where |
|---|---|---|---|
| 1a | Load components on demand | **Adopted, adapted:** one program module **per form**, loaded when the form is first shown | R1–R3 |
| 1b | Component Model | **Deferred:** not natively available in browsers; each module is self-contained, so nothing needs linking | R6 |
| 1c | ES-module integration | **Adopted as an optimisation, feature-detected**, with `fetch` + instantiate as the baseline | R3 |
| 1d | "Destroy on return" down the call chain | **Adapted to COBOL:** state lives until `CANCEL` or the form's end, except `IS INITIAL` programs, which are discarded on return | R4 |
| 2a | JS String Builtins | **Optional**, where it removes a copy; **not** baseline (not in Safari) | R12 |
| 2b | A separate memory for big data (multi-memory) | **Not baseline** — the engine of iOS web views lacks it; the same separation is made with a **fixed arena inside one memory** | R8, R9 |
| 3 | `.free()` on exposed Rust objects | **Adopted for the player**; the generated COBOL module has no such objects | R14 |
| 4 | `memory.grow` and detached views | **Adopted:** the data memory never grows; views are never cached | R7, R10 |
| 5a | Coarse-grained calls | **Adopted** as a rule of the host API | R11 |
| 5b | Threads, Rayon | **Not adopted:** contradicts the ruling of 2026-10-19 (no threads in the browser) and needs cross-origin isolation | R19 |
| 6 | Size-first profile, `wasm-opt` | **Adopted for the player** (the generated module has no `Cargo.toml`) | R16, R17 |

### What was checked (2026-10-20)

- **WebAssembly 3.0** was announced as a standard on 2025-09-17; it includes 64-bit memories, multiple memories, garbage collection, exception handling, tail calls, typed references, relaxed SIMD and a deterministic profile
  ([webassembly.org](https://webassembly.org/news/2025-09-17-wasm-3.0)). A standard is not a browser baseline: what matters is what the engines this product targets run.
- **Multiple memories:** Can I Use's snapshot shows support from Chrome 120 and Firefox 125 and **none in Safari, on macOS or iOS** ([caniuse.com/wf-wasm-multi-memory](https://caniuse.com/wf-wasm-multi-memory)). The iOS target of this series runs in **WKWebView**, which is Safari's engine, so a module
  that uses a second memory would not even validate there. (A snapshot, from a secondary aggregator: confirm against MDN and WebKit's release notes at `/plan`.)
- **JS String Builtins:** live in Chrome and Firefox, not in Safari, as of a 2024–2025 overview ([platform.uno](https://platform.uno/blog/state-of-webassembly-2024-2025/); see also [The New Stack](https://thenewstack.io/wasm-3-0-offers-new-way-to-handle-javascript-strings/)). Same caution.
- **Component Model and ES-module integration:** an early-2026 article says the Component Model's browser features are still under development and not universal ([sesamedisk.com](https://sesamedisk.com/webassembly-in-2026-future-web-integration/)); Mozilla is implementing ES-module integration. No current per-browser table was found.
- **`wasm-pack`:** the `rustwasm` GitHub organisation was wound down in 2025 and `wasm-bindgen` moved to an organisation of its own ([Rust blog](https://blog.rust-lang.org/inside-rust/2025/07/21/sunsetting-the-rustwasm-github-org)); `wasm-pack` was not named a new home in what was read. WASM-002 Q2 already prefers `trunk` as the bundler.
- **In this repository:** the generated COBOL module is emitted by our compiler, not by `rustc` (WASM-010), so Cargo profiles, `wasm-bindgen` and `.free()` concern **only the player** (WASM-016). Forms are separate run units, each with its own copy of Common Code (spec 049; `cobolt-compiler/src/lib.rs:1519-1525`),
  so a module per form follows an existing boundary. The form lifecycle — displayed, resident, destroyed, with `PreservePreviousForm` keeping a form resident — is spec 049's.

## 2. Goals / Non-goals

- **Goals:**
  - A small first load: only the forms the user reaches are fetched.
  - Memory that returns when a form is destroyed, proven by a test.
  - A memory model with **no growth and no detached views** in the program module.
  - A host API cheap to cross: coarse-grained, batched, with at most one copy of text each way.
  - A lean, reproducible build of the player, and a **stated feature baseline and browser matrix** the Build enforces.
- **Non-goals:**
  - Threads, shared memory, Rayon and Web-Worker execution in v1 (R19).
  - The Component Model, WebAssembly GC and 64-bit memories in v1.
  - A second memory (multi-memory) in v1; the design leaves the door open.
  - Performance **budgets** (numbers); this spec makes them measurable, the budgets are set after phase A of WASM-010.

## 3. User stories

- As a user, I want a large application to start fast and to fetch a screen only when I open it.
- As a user, I want the application not to grow in memory as I open and close screens.
- As a developer, I want a long loop in my program not to freeze the page.
- As a maintainer, I want the rules of the JS boundary written once, so that the player and the loader cannot drift.

## 4. Requirements (EARS)

**Modular loading**
- **R1 (ubiquitous):** The Build shall emit **one program module per form** — the form's run unit, with the Common Code its programs call copied into it as today — each self-contained (WASM-010 R2). A project with one form has one module. The module of the **main form** is loaded at start. (Q1 on the granularity.)
- **R2 (event):** When a form is first shown — by a route, a menu load, `OpenFormSync`/`OpenFormAsync` or the start of a standalone page (WASM-011) — the loader shall fetch and instantiate that form's module, and shall not fetch a module whose form is never shown.
- **R3 (ubiquitous):** The loader shall instantiate through ES-module integration when the browser offers it and otherwise through `fetch` and streaming instantiation; both paths shall give the same behaviour. The host page may hint likely next forms for prefetch (Q4).
- **R4 (ubiquitous):** Instance lifetime shall follow COBOL, not the call stack: a program's `WORKING-STORAGE` is kept from its first `CALL` until `CANCEL` or until its form is destroyed, **except** a program declared `INITIAL`, which starts fresh at every call and whose state is discarded when it returns. Subprograms
  shall live inside their form's module unless the plan finds a reason to separate them; if one is separated, its instance obeys the same rule.
- **R5 (event):** When a form is destroyed (spec 049: not resident and not preserved), the loader shall drop **every** reference to its instance and memory — listeners, timers, pending calls (cancelled), entries in host tables — so that the engine can collect it; and a debug build shall be able to prove it (R20).
- **R6 (ubiquitous):** The Component Model shall not be used in v1; the host API stays plain imports and exports (WASM-010 R12). The decision shall be reviewed when browsers support the Component Model natively.

**Memory model**
- **R7 (ubiquitous):** A program module's **data memory** shall have a size **fixed at compile time** from its declared items (`OCCURS` at their maximum, `OCCURS … DEPENDING ON` at their upper bound); generated code shall contain no `memory.grow`; a program whose data cannot be sized is a compile error.
- **R8 (ubiquitous):** Each module shall reserve, inside its one memory, a **transport arena** — a region apart from the COBOL data, of a fixed size taken from the project's payload limit (WASM-005 R11) — for the text and the JSON frames that cross the boundary. The host reads and writes the arena by offset and length, and touches COBOL data only through the host API.
- **R9 (ubiquitous):** Multi-memory shall **not** be part of the baseline (R18). The arena of R8 is laid out so that it could become a second memory in an enhanced variant without changing the host API.
- **R10 (ubiquitous):** The host shall never keep a `TypedArray` or `DataView` over a module's memory across a call: it takes the view from the memory object at each use. The player, whose own memory may grow, shall not hand JavaScript views over its memory that outlive a call into it.

**The boundary**
- **R11 (ubiquitous):** The host API shall be **coarse-grained**: property writes made during a handler are accumulated and delivered **in one call** when the handler ends or suspends, with reads reflecting the handler's own earlier writes; an event is one call; a remote call is one request and one answer; no host call
  shall be made per iteration of a loop.
- **R12 (ubiquitous):** Text crossing the boundary shall be converted **once in each direction**, UTF-8 in the arena against JavaScript strings through `TextEncoder`/`TextDecoder`; where the browser offers **JS String Builtins** and using them removes a copy, the loader may use them (feature-detected, with the baseline path as the fallback).
- **R13 (ubiquitous):** Data that does not need the COBOL program — a dropped file's bytes, an image — shall not pass through the program module: the player moves it between the browser and the backend (WASM-014 R15).
- **R14 (ubiquitous):** Every Rust object the player exposes to JavaScript through `wasm-bindgen` shall have an **explicit disposal** (`free()` or the dispose protocol), called by `unmount()` and by the loader on teardown; the objects exposed shall be few and listed; and no object shall be left to the JavaScript garbage collector alone.
- **R15 (ubiquitous):** The player shall be built so that a panic ends that application instance cleanly: the host page shall show a readable error with a reload action and raise an error event; a run-time error of the COBOL program is not a panic (WASM-010 R11).

**Build and baseline**
- **R16 (ubiquitous):** The **player** shall be built with a size-first release profile — `lto`, `codegen-units = 1`, `panic = "abort"`, stripped symbols and a size-oriented `opt-level` chosen by measurement (`"z"` against `"s"`) — followed by a `wasm-opt` pass; the sizes shall be reported (WASM-016 R13).
- **R17 (ubiquitous):** The product's build shall run a pinned `wasm-bindgen` CLI and a pinned Binaryen `wasm-opt` through the bundler (WASM-002 Q2) and shall not depend on `wasm-pack`; these tools exist only in the product's build, and the supply-chain rules of WASM-013 R9 apply to them.
- **R18 (ubiquitous):** The Build shall declare a **baseline set of WebAssembly features** and a **matrix of supported browsers** — Chrome and Edge, Firefox, Safari, the iOS web view and the Android WebView — taken from MDN and Can I Use at `/plan`; the program module and the player shall use only the baseline; the loader shall probe the browser (`WebAssembly.validate`) and,
  below the baseline, show the readable message of WASM-002 R15. A feature above the baseline (multi-memory, JS String Builtins, ES-module integration, tail calls…) shall be optional, with a fallback or a second variant of the module.
- **R19 (constraint):** The application shall use **no threads and no shared memory** in v1 (operator, 2026-10-19: *threads are not needed in the browser*), shall not require cross-origin isolation (`COOP`/`COEP`), and shall not use Rayon. Responsiveness shall come from **cooperative yielding**: generated code shall yield to the host when a handler has run longer than a budget (a
  default, configurable), at loop back-edges, through the suspension mechanism of WASM-010 R9, and resume on the host's next turn; results shall not depend on where it yielded.

**Assurance**
- **R20 (ubiquitous):** The test suite shall contain: a **laziness** test (a network trace shows a form's module fetched only when the form is shown); a **disposal** test (open and close a form many times; live instances and memory return to their baseline, using weak references in a debug build); a **fixed-memory** test; a **batching** test (a handler that writes 1000 properties makes at most
  a stated number of host calls); a **strings** test (UTF-8 round trip including multi-byte text); a **view** test (a call that grows the player's memory leaves the host's reads correct); a **baseline** test (modules validate on the baseline, and a browser below it gets the message); and a **yield** test (a very long loop keeps frames within a deadline and gives the same result).

## 5. Acceptance criteria

- [ ] **AC1** — A three-form project builds three program modules; at start only the main form's is fetched; showing the second form fetches its module; a form never shown is never fetched.
- [ ] **AC2** — Opening and closing a non-resident form 100 times leaves the live instance count and the memory at their starting values (debug leak check).
- [ ] **AC3** — A form kept resident by `PreservePreviousForm` keeps its instance; destroying it releases the instance.
- [ ] **AC4** — A called program declared `INITIAL` starts fresh on every call; a plain called program keeps its state across calls until `CANCEL`, and after `CANCEL` starts fresh.
- [ ] **AC5** — Generated modules contain no `memory.grow`; a program with data of unbounded size fails to compile with the item named.
- [ ] **AC6** — A handler that writes 1000 properties makes at most the stated number of host calls, and a read after a write sees the write.
- [ ] **AC7** — Text with multi-byte characters crosses the boundary intact in both directions; no JavaScript string is built in a loop (a trace counts conversions).
- [ ] **AC8** — A player call that grows the player's memory does not invalidate what the host reads next; no view is cached across calls (a source search of the loader and the test of R10).
- [ ] **AC9** — Every object the player exposes to JavaScript is on the list and is disposed by `unmount()`.
- [ ] **AC10** — A forced panic in the player shows the readable error with the reload action and raises the error event; a COBOL run-time error does not.
- [ ] **AC11** — The Build prints the player's size before and after `wasm-opt`, and the profile in force; the pinned tool versions are recorded.
- [ ] **AC12** — On a browser below the baseline the loader shows the message; on the baseline, modules validate; no baseline module uses multi-memory (a scan of the module's sections).
- [ ] **AC13** — A loop of ten million iterations keeps every animation frame under the deadline of the test and yields the same results as without yielding; the page never needs cross-origin isolation (a header-free static host works).

## 6. Constraints & steering check

- **Adjusts the earlier specs:** WASM-010 (one module **per form**; memory fixed at compile time with an arena; batching; yielding), WASM-002 (the bundle holds several program modules loaded on demand; the baseline and the browser matrix; no cross-origin isolation), WASM-011 (the module of a form is loaded when it is shown and discarded when it is destroyed),
  WASM-016 (the bundle is the player plus the program modules; the build profile; disposal), WASM-013 (no cross-origin isolation, no threads), WASM-015 (the tests of R20).
- **A tension to rule on (D15):** the operator's decision of 2026-10-19 was **"two `.wasm` files"** — the player and the program module. Per-form loading makes it **one player plus one module per form**. The design intent (a prebuilt player, generated COBOL, no Rust toolchain) is unchanged; the count of files is not. Q1 offers the alternative of one module for the whole project.
- **i18n:** the readable messages of R15 and R18 belong to the host page (translatable by the developer); IDE strings (the Build report) are six languages.
- **Credentials/security:** no secret enters a module or the arena; R19 keeps the page embeddable and free of cross-origin-isolation requirements.
- **Tech steering:** Binaryen, `wasm-bindgen` and the bundler enter only the product's build; MSRV 1.92 and the dependency guards apply.
- **Docs:** `docs/` untouched on 2.x.x. **System KB:** the form-lifecycle and `INITIAL`/`CANCEL` behaviour in the browser belongs in the compiler's documentation tables at implementation.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — Granularity.** One module per form (recommended: it follows the run-unit boundary and gives the smallest first load) against **one per subsystem** (the menu-carrying form with its children, spec 049) or **one for the whole project** (simplest, no duplication, a larger first load). A module per form copies the routines it uses and the Common Code it calls into each module;
  with transport compression this is small, but it should be measured on a real multi-form application before the choice is final.
- **Q2 — The yield budget** (a default in milliseconds) and whether running the program in a Web Worker is worth its cost later (the DOM is not reachable from a worker, so every host call becomes asynchronous).
- **Q3 — An enhanced variant** (multi-memory, JS String Builtins, tail calls) when Safari and the iOS web view support them: build two variants and let the loader choose, or wait?
- **Q4 — Prefetch policy:** none, idle-time prefetch of the forms a menu lists, or a hint property on the form.
- **Q5 — The browser matrix:** which minimum versions, in particular the oldest iOS the Mobile kind supports; decided from the data of `/plan`.
- **Q6 — `wasm-opt`:** run only in the product's build (for the player), or also shipped with the IDE to optimise generated modules (a native tool on the developer's machine)? Recommended: only the product's build in v1; the emitter does its own dead-code removal (WASM-010 R2).
- **Q7 — Revisiting the Component Model and ES-module integration:** a trigger (support in the three engines) and an owner.
- **Q8 — `opt-level`:** `"z"` against `"s"`: size against speed for the player, by measurement on a form-heavy page.
