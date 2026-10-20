<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Module architecture, memory model, threads and the JavaScript boundary

- **Status:** draft, decisions of 2026-10-20 incorporated (needs `/clarify` only for the questions in §7)
- **Folder:** specs/WASM-017-module-architecture-and-js-boundary/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-20
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the operator's guidance of 2026-10-20, six topics — *(1)* modular architecture with on-demand instantiation and prompt disposal (Component Model, ES modules); *(2)* the JS ⇄ Wasm boundary (JS String Builtins, a separate memory for raw data); *(3)* explicit lifecycles for Rust
  objects (`.free()`); *(4)* avoiding the `memory.grow` trap with multiple memories; *(5)* coarse-grained calls and native concurrency (threads, Rayon); *(6)* a size-first release profile — and the operator's **decisions of the same day** on the open points: **one module for the whole project** (Q1's alternative);
  **threads and Rayon are in**; **multi-memory is the default and is switched off for iOS**; **Component Model and ES-module integration are kept only if Chrome supports them**.
- **Why a spec of its own.** The topics cut across the generated module (WASM-010), the player (WASM-016), the bundle and loader (WASM-002) and the navigation (WASM-011). This spec owns the architecture of the WebAssembly artefacts; the others cite it.
- **Depends on:** WASM-002, WASM-010, WASM-016. **Adjusts:** WASM-002, 010, 011, 013, 015, 016 (§6).

## 1. Overview

The guidance asks for a small footprint, memory that goes away when its owner does, a thin boundary between JavaScript and WebAssembly, real concurrency and a lean build. Each topic was checked against what the product decided before and what browsers run, and the operator then ruled on the open points. The disposition:

| # | Topic | Disposition | Where |
|---|---|---|---|
| 1a | Load components on demand | **One program module for the whole project** (operator, Q1), compiled while it streams; on-demand loading applies to the **player's heavy parts** (charts, maps, the Viewer, the 3D engine) | R1, R3 |
| 1b | Component Model | **Not kept**: the operator's condition — Chrome support — is not met (§1, "What was checked"); revisit when it is | R6 |
| 1c | ES-module integration | **Not kept as the baseline**, same condition; **optional and feature-detected**, with `fetch` + streaming instantiation as the path every browser takes | R3 |
| 1d | "Destroy on return" down the call chain | **Adapted to COBOL:** a form's data lives in a **region** that is released when the form is destroyed; state is kept until `CANCEL` or the form's end, except `INITIAL` programs, which start fresh and are discarded on return | R2, R4, R5 |
| 2a | JS String Builtins | **Optional**, where it removes a copy; not baseline (not in Safari) | R12 |
| 2b | A separate memory for raw data | **Multi-memory is the default** for the program module — the transport arena is memory 1 — with a **single-memory variant** for iOS and for any target matrix that includes Safari | R8, R9 |
| 3 | `.free()` on exposed Rust objects | **Adopted for the player** (and the graphics objects it hosts); the generated COBOL module has no such objects | R14 |
| 4 | `memory.grow` and detached views | **Adopted:** the program module's memory never grows; views are never cached; shared memory of the threaded player has a declared maximum | R7, R10 |
| 5a | Coarse-grained calls | **Adopted** as a rule of the host API | R11 |
| 5b | Threads and Rayon | **Adopted (operator, 2026-10-20), reversing the ruling of 2026-10-19.** Rayon in the player and the graphics objects; the program module in a Worker when the page is cross-origin isolated; **everything also works without threads** | R19 |
| 6 | Size-first profile, `wasm-opt` | **Adopted for the player**; the threaded variant needs a pinned **nightly** toolchain | R16, R17 |

### What was checked (2026-10-20)

- **WebAssembly 3.0** is a standard since 2025-09-17 — 64-bit memories, multiple memories, GC, exception handling, tail calls, typed references, relaxed SIMD, a deterministic profile ([webassembly.org](https://webassembly.org/news/2025-09-17-wasm-3.0)).
  **Threads are not part of it**: the threads proposal is separate and has been available in Chrome, Firefox and Safari for years ([web.dev](https://web.dev/articles/webassembly-threads)). What stood in the way was never browser support; it is **cross-origin isolation** (`COOP`/`COEP`) and, for Rust,
  a **nightly toolchain with a rebuilt standard library**: the `wasm-bindgen-rayon` documentation says the standard library for the WebAssembly target is built without threads and was tested with `nightly-2024-08-02` ([docs.rs](https://docs.rs/wasm-bindgen-rayon)). The operator's decision is carried out with those two costs stated (R19).
- **Multiple memories:** Can I Use's snapshot shows Chrome 120+, Firefox 125+ and **no Safari, macOS or iOS** ([caniuse.com](https://caniuse.com/wf-wasm-multi-memory)); the iOS target runs in **WKWebView**, Safari's engine. A snapshot from a secondary source: confirm at `/plan`.
- **JS String Builtins:** Chrome and Firefox, not Safari, in a 2024–2025 overview ([platform.uno](https://platform.uno/blog/state-of-webassembly-2024-2025/)).
- **ES-module integration and the Component Model in Chrome:** **not confirmed.** Source-phase imports are TC39 Stage 3, Firefox's tests for them are marked tentative ([proposal](https://github.com/tc39/proposal-source-phase-imports)); `jco transpile`, the usual route for components in a browser, calls itself experimental and unsuitable for production
  ([jco](https://www.socket.dev/npm/package/@bytecodealliance/jco-transpile)); the Component Model 1.0 is still being finished ([Bytecode Alliance](https://bytecodealliance.org/articles/the-road-to-component-model-1-0)). Chrome's own status page was not reachable in this session. By the operator's condition they are **not kept**, and R6 and R3 say what stays.
- **`wasm-pack`:** its upstream organisation was wound down in 2025 ([Rust blog](https://blog.rust-lang.org/inside-rust/2025/07/21/sunsetting-the-rustwasm-github-org)); `trunk` is the bundler of WASM-002 Q2.
- **In this repository:** the generated COBOL module is emitted by our compiler, not `rustc` (WASM-010), so Cargo profiles, `wasm-bindgen`, `.free()` and Rayon concern **the player and the graphics objects** (WASM-016, WASM-018). `rayon` is already in the workspace's dependency tree (through `image`); Rust is 1.95 stable (MSRV 1.92); the machine has no `wasm32` target yet.
  Forms are separate run units, each with its own copy of Common Code (spec 049; `cobolt-compiler/src/lib.rs:1519-1525`), and their lifecycle — displayed, resident, destroyed, `PreservePreviousForm` keeping one resident — is spec 049's.

## 2. Goals / Non-goals

- **Goals:**
  - A program module that streams, compiles while it downloads and starts fast; heavy player parts fetched only when used.
  - Form memory that returns when the form is destroyed, proven by a test.
  - A memory model with **no growth and no detached views** in the program module; multi-memory by default, a single-memory variant where the platform lacks it.
  - A host API cheap to cross: coarse-grained, batched, at most one copy of text each way.
  - **Real concurrency** where the platform allows it, never required for correctness.
  - A lean, reproducible build of the player, and a **stated feature baseline and browser matrix** the Build enforces.
- **Non-goals:**
  - Threads in the **COBOL program's own semantics**: COBOL has no threads; the program module runs in one thread (in a Worker when isolated, R19).
  - The Component Model, WebAssembly GC and 64-bit memories in v1.
  - Performance **budgets** (numbers): this spec makes them measurable.

## 3. User stories

- As a user, I want a large application to start fast and a long calculation never to freeze the page.
- As a user, I want the application not to grow in memory as I open and close screens.
- As a developer on a phone or a Safari browser, I want the same application to run, without multi-memory and without threads, if that is what the platform offers.
- As a maintainer, I want the rules of the JS boundary written once, so that the player and the loader cannot drift.

## 4. Requirements (EARS)

**Loading and form lifetime**
- **R1 (ubiquitous):** The Build shall emit **one program module for the whole project** — every form's programs, with the Common Code each form calls copied in as today — self-contained (WASM-010 R2). The loader shall fetch it with **streaming compilation** and start the main form as soon as it is ready.
- **R2 (ubiquitous):** Each form **instance** shall have its `WORKING-STORAGE` in a **region** of the module's data memory, allocated when the form is created and released when it is destroyed (spec 049: not resident and not preserved). The memory's size is fixed at compile time (R7) as the static data plus, for each form, **`Instances` × its size**, where `Instances` is a property of the
  form (default 4 for a form that `OpenFormSync`/`OpenFormAsync` can open, 1 otherwise); creating an instance beyond `Instances` shall raise the run-time error of WASM-010 R11 with the form's name.
- **R3 (ubiquitous):** The loader shall instantiate by `fetch` and streaming instantiation, which every supported browser offers. **ES-module integration** (importing `.wasm` natively) is **optional**: used when feature-detected, never required (R6, R18). Heavy **player parts** (WASM-016 R13) shall be fetched on first use by the same loader.
- **R4 (ubiquitous):** Lifetime shall follow COBOL, not the call stack: a program's `WORKING-STORAGE` is kept from its first `CALL` until `CANCEL` or until its form's instance is destroyed, **except** a program declared `INITIAL`, which starts fresh at every call and whose state is discarded when it returns.
- **R5 (event):** When a form instance is destroyed, the module shall release its region and reinitialise it, and the loader shall drop **every** reference the instance held — listeners, timers, pending calls (cancelled), host-table entries; when the application is unmounted, the loader shall drop the module, its memories and terminate its Workers; a debug build shall prove both (R20).
- **R6 (ubiquitous):** The Component Model shall **not** be used in v1: the operator kept it only if Chrome supported it, and that is not established. The host API stays plain imports and exports (WASM-010 R12). The decision shall be revisited when the three engines support the Component Model natively (Q7).

**Memory model**
- **R7 (ubiquitous):** The program module's **data memory** has a size **fixed at compile time** (R2); generated code contains no `memory.grow`; data that cannot be sized is a compile error naming the item.
- **R8 (ubiquitous):** Text and JSON frames that cross the boundary live in a **transport arena** of a fixed size taken from the project's payload limit (WASM-005 R11). In the **multi-memory variant** the arena is **memory 1**, separate from the COBOL data in memory 0; in the **single-memory variant** it is a reserved region of memory 0. The host API is the same: offset, length and a memory selector.
- **R9 (ubiquitous):** **Multi-memory is the default** for the program module (operator, 2026-10-20). The Build shall also produce a **single-memory variant** — no multi-memory instruction anywhere — and shall use it **always for iOS** (the Mobile iOS build has "multi-memory" off by default) and **whenever the project's target-browser matrix includes Safari or the iOS web view**, which the default matrix does: such a Web bundle carries **both**
  variants and the loader probes (`WebAssembly.validate` of a minimal multi-memory module) and loads the matching one. A project whose matrix has no Safari may choose "multi-memory only". Only the **emitted** module can use a second memory; Rust artefacts (the player) cannot address one and never do.
- **R10 (ubiquitous):** The host shall never keep a `TypedArray` or `DataView` over a module's memory across a call: it takes the view from the memory object at each use. The player, whose memory may grow, shall not hand JavaScript views over its memory that outlive a call into it; this holds for shared memory too.

**The boundary**
- **R11 (ubiquitous):** The host API is **coarse-grained**: property writes made during a handler are accumulated and delivered **in one call** when the handler ends or suspends, with reads reflecting the handler's own earlier writes; an event is one call; a remote call is one request and one answer; no host call is made per iteration of a loop. A drawing program uses the same rule: commands go into a buffer in the arena and are executed by one call (WASM-018 R6).
- **R12 (ubiquitous):** Text crossing the boundary is converted **once in each direction**, UTF-8 in the arena against JavaScript strings through `TextEncoder`/`TextDecoder`; where the browser offers **JS String Builtins** and using them removes a copy, the loader may use them (feature-detected, baseline path as fallback).
- **R13 (ubiquitous):** Data that does not need the COBOL program — a dropped file's bytes, an image, pixel buffers of the graphics objects — shall not pass through the program module: the player moves it (WASM-014 R15, WASM-018).
- **R14 (ubiquitous):** Every Rust object the player exposes to JavaScript through `wasm-bindgen` has an **explicit disposal** (`free()` or the dispose protocol), called by `unmount()` and by the loader on teardown; the objects exposed are few and listed; none is left to the JavaScript garbage collector alone. The same holds for the handles the graphics objects hand out (WASM-018).
- **R15 (ubiquitous):** The player shall be built so that a panic ends that application instance cleanly: the host page shows a readable error with a reload action and raises an error event; a COBOL run-time error is not a panic (WASM-010 R11). In the threaded variant a panic in a worker also ends the instance.

**Build and baseline**
- **R16 (ubiquitous):** The **player** is built with a size-first release profile — `lto`, `codegen-units = 1`, `panic = "abort"`, stripped symbols, a size-oriented `opt-level` chosen by measurement (`"z"` against `"s"`) — and a `wasm-opt` pass; sizes are reported (WASM-016 R13). It has **two variants**: **plain** (stable Rust, baseline) and **threaded** (a **pinned nightly** toolchain, a rebuilt standard library, atomics, bulk memory and shared memory, `--target web`, `wasm-bindgen-rayon`), built from the same source.
  The rest of the workspace stays on stable Rust, MSRV 1.92; the nightly is confined to the player's build step.
- **R17 (ubiquitous):** The product's build runs a pinned `wasm-bindgen` CLI, a pinned Binaryen `wasm-opt` and the bundler (WASM-002 Q2), never `wasm-pack`; these tools and the nightly exist only in the product's build; the supply-chain rules of WASM-013 R9 apply, and the nightly's date is recorded in every build.
- **R18 (ubiquitous):** The Build declares a **baseline of WebAssembly features** and a **matrix of supported browsers** (Chrome and Edge, Firefox, Safari, the iOS web view, the Android WebView), from MDN and Can I Use at `/plan`. The loader probes the browser and chooses, independently: the **program-module variant** (multi-memory or single, R9), the **player variant** (threaded or plain, R19) and the optional accelerations (JS String Builtins, ES-module
  integration). Below the baseline it shows the readable message of WASM-002 R15. The baseline is what the single-memory variant and the plain player use.

**Concurrency**
- **R19 (ubiquitous):** Threads and Rayon are **in v1** (operator, 2026-10-20), as follows.
  (a) **Rayon** runs in the **player and the graphics objects** — image filters and resizing, chart and canvas rasterisation, physics, large table work such as sorting and filtering a DataGrid — on a worker pool sized from `navigator.hardwareConcurrency` (capped, configurable), using the threaded player.
  (b) The **program module runs in a dedicated Worker** when the page is cross-origin isolated: host calls are proxied to the main thread (writes batched, R11; reads by a blocking wait on shared memory), so a long loop never freezes the page; the same module runs **on the main thread** with cooperative yielding (WASM-010 R9) when it is not. Results do not depend on the mode.
  (c) **Isolation** — `Cross-Origin-Opener-Policy: same-origin` with `Cross-Origin-Embedder-Policy: require-corp` or `credentialless` — is **required for threads and is a deployment property**: the Build prints the headers for the common hosts and an optional service-worker shim that sets them for hosts that cannot (off by default, with its trade-offs stated); resources from other origins must then allow it (CORS or `Cross-Origin-Resource-Policy`).
  (d) A page that embeds the application (WASM-002 R11) is isolated only if the host page is; otherwise the application **falls back to the plain player and the main thread**, with no loss of function.
  (e) **Nothing requires threads for correctness:** every feature works on the plain player and the main thread, and a test runs the same cases on each (R20).
  (f) Deterministic results: a Rayon reduction shall not make a result depend on the thread count (WASM-018 R20).
- **R20 (ubiquitous):** The test suite contains, for **each combination** of program-module variant (multi-memory, single) and player variant (threaded, plain), and for both program execution modes (Worker, main thread): a **loading** test (one project module fetched at start, a heavy player part only on first use); a **disposal** test (open and close a form many times; regions and instances return to baseline; an unmount ends the Workers); a **fixed-memory** test; a **batching** test (a handler that writes 1000 properties makes at most a stated number of host calls); a **strings** test (UTF-8 round trip including multi-byte text); a **view** test
  (a call that grows the player's memory leaves the host's reads correct); a **baseline and matrix** test (a browser below the baseline gets the message; each variant is chosen by its probe); an **isolation** test (with and without `COOP`/`COEP`, the threaded or plain player is chosen and results are the same); and a **yield** test (a very long loop keeps frames within a deadline in the main-thread mode).

## 5. Acceptance criteria

- [ ] **AC1** — A three-form project builds one program module; it is fetched once, at start, with streaming compilation; a heavy player part (a chart) is fetched only when a form using it is first shown.
- [ ] **AC2** — Opening and closing a form 100 times leaves the live instance count and the module's used regions at their starting values; creating more instances than `Instances` raises the run-time error naming the form.
- [ ] **AC3** — A form kept resident by `PreservePreviousForm` keeps its region; destroying it releases the region; an unmount terminates the Workers and drops the module.
- [ ] **AC4** — A called program declared `INITIAL` starts fresh on every call; a plain called program keeps its state across calls until `CANCEL`, and after `CANCEL` starts fresh.
- [ ] **AC5** — Generated modules contain no `memory.grow`; a program whose data cannot be sized fails to compile with the item named.
- [ ] **AC6** — The multi-memory variant has two memories with the arena in memory 1; the single-memory variant has one and no multi-memory instruction (a scan of its sections); the host API behaves identically on both.
- [ ] **AC7** — The iOS Mobile build uses only the single-memory variant by default; a Web bundle whose matrix includes Safari carries both variants and loads the right one on Chrome (multi-memory) and on a browser that lacks it (single); "multi-memory only" is refused by the Build when the matrix includes Safari.
- [ ] **AC8** — A handler that writes 1000 properties makes at most the stated number of host calls, and a read after a write sees the write.
- [ ] **AC9** — Text with multi-byte characters crosses the boundary intact in both directions; no JavaScript string is built in a loop (a trace counts conversions).
- [ ] **AC10** — A player call that grows the player's memory does not invalidate what the host reads next; no view is cached across calls (a source search of the loader and the test of R10).
- [ ] **AC11** — Every object the player exposes to JavaScript is on the list and is disposed by `unmount()`; a forced panic shows the readable error with the reload action and raises the error event.
- [ ] **AC12** — The Build prints both player variants' sizes before and after `wasm-opt`, the profile in force, the pinned tool versions and the nightly's date.
- [ ] **AC13** — On an isolated page the threaded player and the Worker execution mode are chosen and a Rayon-backed operation uses more than one worker; on a non-isolated page the plain player and the main thread are chosen, results are identical, and a ten-million-iteration loop keeps every frame under the test's deadline.
- [ ] **AC14** — Embedded in a non-isolated host page the application starts on the plain player and works; embedded in an isolated one it uses threads.
- [ ] **AC15** — The headers for isolation are generated for each documented host; an image, font or tile from another origin loads under `credentialless` or with CORS/CORP as documented, and fails with a readable diagnostic otherwise.

## 6. Constraints & steering check

- **Adjusts the earlier specs:** WASM-010 (one module for the project; regions per form instance; multi-memory default; Worker mode), WASM-002 (the bundle's variants; baseline and matrix; isolation headers), WASM-011 (form instance data regions instead of per-form modules), WASM-013 (isolation replaces the old "no cross-origin isolation" rule), WASM-015 (the test matrix), WASM-016 (player variants, nightly, parts on demand).
- **Reverses a ruling:** the 2026-10-19 ruling *threads are not needed in the browser* is **superseded by the operator's decision of 2026-10-20**; CLAUDE.md and the index say so.
- **Toolchain impact (stated, not avoided):** the threaded player needs a **pinned nightly** and `build-std`, in the **product's** build only; this is outside the workspace's MSRV and its dependency guards and is therefore recorded in every build (R17) and reviewed with WASM-013 R9.
- **i18n:** the readable messages of R15 and R18 belong to the host page; IDE strings (the Build report, the iOS multi-memory option) are six languages.
- **Docs:** `docs/` untouched on 2.x.x. **System KB:** the form `Instances` property and the `INITIAL`/`CANCEL` behaviour in the browser go into the compiler's documentation tables at implementation.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — decided** (operator, 2026-10-20): one program module for the whole project (R1). The cost accepted: a larger first download than per-form modules, mitigated by streaming compilation, compression and the player's on-demand parts; to be measured on a multi-form application.
- **Q2 — The yield budget** (a default, in milliseconds) for the main-thread mode; and the number of Workers by default (`hardwareConcurrency` minus one, capped at 8?).
- **Q3 — decided** by R9: both variants, chosen by probe; iOS off.
- **Q4 — The browser matrix** and its defaults: which minimum versions, in particular the oldest iOS the Mobile kind supports; from the data of `/plan`.
- **Q5 — Isolation on the mobile shell:** Tauri's configuration for the headers of its asset protocol, and whether the iOS web view isolates; if not, the Mobile kind runs the plain player and the main thread there.
- **Q6 — `wasm-opt`:** product's build only for the player (recommended), or also shipped for generated modules.
- **Q7 — Revisiting the Component Model and ES-module integration:** the trigger is native support in Chrome, Firefox and Safari; an owner to check Chrome's status page, which could not be read in this session.
- **Q8 — `opt-level`:** `"z"` against `"s"` by measurement.
- **Q9 — The nightly:** which pinned date, and whether `build-std` is still required by the toolchain of the time of the `/plan`.
- **Q10 — The service-worker shim** that sets the isolation headers on hosts that cannot: offer it, document it only, or leave it out.
