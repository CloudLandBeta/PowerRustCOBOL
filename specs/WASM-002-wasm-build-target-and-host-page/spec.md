<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — The WebAssembly build target (Leptos + Tauri 2) and the host page

- **Status:** draft (needs `/clarify`; Q1 decides the shape of the whole series)
- **Folder:** specs/WASM-002-wasm-build-target-and-host-page/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator item **2** (version 2.0 generates WebAssembly; HTML, CSS3 and JavaScript load the application inside a
  pre-existing layout or fill the window's available area) and the stack named in item **1** (Tauri 2 + Leptos).
- **Depends on:** WASM-001 (the project kinds). **Amends:** spec 041 R17/R18 (*build for the host triple only*) — for the web and
  mobile kinds only; spec 017 (*one renderer everywhere*) — the web kinds are rendered by the DOM, see WASM-004.
- **Prior art:** spec 006 (removed 2026-10-14, `git show ccfa3449^:specs/006-web-wasm-projects/spec.md`): an eframe-web
  `<canvas>` bundle with `trunk`, COBOL transpiled to Rust, no interpreter shipped. Carried over: the portable-subset check
  (R3), toolchain detection (R6), no in-IDE web debugger. Changed: DOM + CSS instead of a canvas; the execution model is open (Q1).

## 1. Overview

Version 2.x.x builds a Web or Mobile project to **WebAssembly** instead of a native executable. The user interface is rendered
by **Leptos** (Rust compiled to WebAssembly that builds ordinary DOM elements), so HTML and CSS3 are the real surface and the
browser does the text, accessibility and layout work. The bundle is a set of static files (an HTML page, a `.wasm` module, a
small JavaScript loader, a stylesheet and assets) that any web server can host. For **Android and iOS** the same frontend runs
inside a **Tauri 2** shell, which supplies the native wrapper, the system web view and the store package.

A **host page** is the thin HTML/CSS3/JavaScript layer that loads the module. It supports two ways to place the application:
**in the available window area** (the application is the page), or **inside a pre-existing layout** (an element on someone
else's page).

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **The compiler is native and host-only.** `build_project` (`crates/cobolt-compiler/src/lib.rs:1737`) serialises the AST with
  bincode + gzip (`:2259-2261`), embeds it with `include_bytes!` (`:4510`), generates a Cargo project and runs
  `cargo build --release` with **no `--target`** (`:2852-2882`); the output goes to `bin/` (`:3003-3014`). The generated crate
  depends on `cobolt-form-host` and eframe 0.36 (`:3791-3799`). `BuildOptions.target` (`:1633-1639`) is only compared with the host
  triple; any other value is `UnsupportedTarget` (`:752`, `:1993-2000`), by design (spec 041 R17/R18).
- **There is no WebAssembly anything.** No `wasm-bindgen`, `web-sys`, `js-sys`, `leptos`, `trunk` or `tauri` in any `Cargo.toml` of the
  workspace (they appear in `Cargo.lock` only transitively); the only `wasm32` mention is a `cfg` gate on `syntect`
  (`cobolt-ide/Cargo.toml:127`). `toolchain.rs` only checks the rustc version and offers the rustup installer.
- **There is no COBOL → Rust generator.** `cobolt-codegen` turns *forms* into COBOL; the only Rust the compiler emits from COBOL
  source is `EXEC RUST` blocks (`cobolt-compiler/src/exec_rust.rs`). The program runs in the **tree-walking interpreter** bundled
  into the binary.
- **The runtime is not browser-ready.** The event loop blocks on `std::sync::mpsc` (`cobolt-runtime/src/interpreter.rs` ~`:3942`,
  `:3958`); `thread::spawn` appears in 12 runtime files (`async_op.rs` among them); `std::fs` in 23 (`interpreter.rs` 62 times,
  `indexed_disk.rs` 201); `Instant::now` 23 times in `interpreter.rs`; `redb`, `sysinfo`, `aes-gcm`, `getrandom` and `chrono`
  (clock) are linked whatever the features. The lexer, AST, parser and semantic crates use no file, thread, process or network
  API (the lexer reads files only in `copybook.rs`). `cobolt-forms` without `render` is free of egui but has unconditional
  `lopdf`, `mermaid-rs-renderer` and `tl` dependencies and ungated `std::fs` in `viewer.rs`, `dropzone.rs`, `assets.rs` and
  `items_file.rs`. `cobolt-form-host` is native (`rfd`, `pollster`, eframe).
- **Optional bridges are already feature-gated** in `cobolt-runtime`: `default = sql, http, maps, kb, smb, pdf, office, aws, images,
  audio`, and generated applications use `default-features = false` (`cobolt-compiler/src/lib.rs:3770`) — the same mechanism a
  browser profile would use.
- **Docs and specs that already speak of the web:** the CLAUDE.md 2.x.x section; `CONVENTIONS.md` and spec 047 mention WebAssembly.

## 2. Goals / Non-goals

- **Goals:**
  - A **Build** for the Web kind that produces a static bundle, and for the Mobile kind an Android package and an iOS package.
  - The **same program behaviour** in the browser as on the desktop for everything the portable subset allows.
  - A **portable-subset check** at Check/Build that says, per line, what cannot run in a browser and what to use instead.
  - The **host page**: window-filling mode and embedded-in-a-layout mode, with a small documented JavaScript surface.
  - Toolchain detection with clear, honest messages.
- **Non-goals:**
  - Server-side rendering, multi-threaded WebAssembly, offline/PWA behaviour, hosting or deploying the bundle (WASM-INDEX lists
    each as a proposed spec).
  - A web debugger inside the IDE (browser developer tools serve; source maps where the toolchain gives them).
  - Changing the desktop build (egui, `bin/`, host triple) in any way.
  - The backend (WASM-003); it is a different binary.

## 3. User stories

- As a developer, I want **Build** on a Web project to give me a folder I can drop on any static host, so that deployment needs
  nothing from me but a web server.
- As a web developer with an existing site, I want to **place the application inside one element** of my page, so that my header,
  navigation and footer stay mine.
- As a developer, I want the IDE to tell me **which line** uses something a browser cannot do, so that I fix it before building.
- As a mobile developer, I want the same project to give me an **Android** and an **iOS** package, so that I keep one codebase.

## 4. Requirements (EARS)

**The build**
- **R1 (event):** When the developer builds a Web project, the IDE shall produce, in the project's build folder (specs 100/101 govern
  the folder and cache), a **static bundle**: the host `index.html`, the WebAssembly module, the JavaScript loader, the form
  stylesheet (WASM-007) and the assets the forms use, with content-hashed file names.
- **R2 (event):** When the developer builds a Mobile project, the IDE shall produce an **Android package** and/or an **iOS
  package** that wrap the same frontend in a Tauri 2 shell, for the platform(s) the project selected.
- **R3 (event):** When the developer runs Check or Build on a Web or Mobile project, the IDE shall validate the COBOL against the
  **portable subset** and report each construct that cannot run in a browser as an **error** at its line, naming the construct and
  the alternative (a remote call, WASM-005, or a different control); no bundle shall be produced while such errors remain. Constructs
  that need the local disk, a child process, an OS thread, a native socket or an embedded secret are not portable.
- **R4 (ubiquitous):** For every construct in the portable subset the application shall behave as the same program does on the desktop
  runtime: same results, same file-status/`ON EXCEPTION` outcomes, same event order. Any difference shall be listed, per construct,
  in one place the developer can read.
- **R5 (ubiquitous):** The user interface of a Web or Mobile application shall be built as DOM elements styled with CSS3 (not drawn on
  a `<canvas>`), so that the browser's text selection, zoom, accessibility tree and find-in-page work.
- **R6 (event):** When a required tool is missing (the `wasm32-unknown-unknown` target, the bundler, the `wasm-bindgen` CLI at the
  matching version, the Tauri 2 CLI, the Android SDK/NDK, Xcode), the IDE shall name it, say whether it can install it itself, and
  install only what is safe to install after the developer agrees (`rustup target add` is; Xcode and the Android SDK are not).
- **R7 (ubiquitous):** The bundle shall contain **no secret**: no key, token, password, connection string or client secret; Build shall
  scan the produced files and the forms for them and fail, naming the file and the pattern (the credential gate the examples already need).
- **R8 (ubiquitous):** Build shall report the size of each part (module, loader, stylesheet, assets) so that growth is visible from
  one build to the next.
- **R9 (constraint):** A Desktop project shall build exactly as before; nothing in this spec changes `bin/`, eframe or the host-triple rule
  for the desktop kind.

**The host page**
- **R10 (optional):** Where the application is placed **in the available window area**, the page shall make it fill the viewport (including
  the dynamic viewport on phones) without producing scroll bars of its own, and shall re-lay it out when the window changes.
- **R11 (optional):** Where the application is placed **inside a pre-existing layout**, the page shall mount it into an element chosen by the
  host (an attribute or a selector), size it to that element, and re-lay it out when the element changes size (not only when the window
  does); the responsive breakpoints of the forms shall be evaluated against the **element's** width in this mode.
- **R12 (optional):** Where the application is embedded, the host page's CSS shall not leak into it and the application's CSS shall not leak
  out, while the host can still pass theme values (CSS custom properties) in.
- **R13 (ubiquitous):** The JavaScript surface the host can use shall be small and documented: mount, unmount, initial parameters
  (locale, theme, backend address), and events out (ready, error, navigated, signed in/out). It shall offer no way to evaluate arbitrary
  script.
- **R14 (ubiquitous):** The application shall run under a Content-Security-Policy that allows WebAssembly (`'wasm-unsafe-eval'`) but not
  `'unsafe-eval'`, and the build shall print the policy it needs (script, style, font, image and connect sources).
- **R15 (state):** While the module is loading, the page shall show progress; if the browser cannot run WebAssembly, or loading fails, it
  shall show a readable message and a way to retry, not a blank page.

## 5. Acceptance criteria

- [ ] **AC1** — A hello-world Web project builds to a folder of static files that, served by any static server, shows its form and answers a
  button click in a headless browser.
- [ ] **AC2** — The same hello-world, mounted into one `<div>` of a test page next to other content, fills only that `<div>`, resizes with it,
  and neither page's CSS affects the other (a test asserts a computed style on each side).
- [ ] **AC3** — A form using a construct outside the subset (for example an `OPEN` of a disk file) makes Check fail with the line, the
  construct and the alternative; no bundle is written.
- [ ] **AC4** — A program in the subset, run on the desktop runtime and in the browser (headless), prints identical output for a fixed set of
  programs (a conformance list is part of the plan; it is what R4's "behave as" is measured against).
- [ ] **AC5** — A key placed in a form's property (a deliberate test secret) stops Build with a message that names the file and pattern.
- [ ] **AC6** — Missing-tool messages for each tool of R6 appear when that tool is absent (tested by hiding it from `PATH`).
- [ ] **AC7** — The Mobile build produces an Android package and (on macOS) an iOS package from the same project; both start and show the form.
- [ ] **AC8** — A Desktop project builds byte-for-byte as before (the existing compiler tests stay green).

## 6. Constraints & steering check

- **i18n (6 languages):** yes — toolchain messages, the portability diagnostics and the Build report are IDE strings.
- **Generated-code contract:** the generated COBOL and its banner are unchanged; the web frontend is a *new build artefact* downstream of
  the generated `.cbl`, never hand-edited. Spec 017's "one renderer" is amended for the web kinds (the DOM is their renderer; WASM-004).
- **Docs:** `docs/` is not touched on 2.x.x (CLAUDE.md ruling); the guide chapter is planned in WASM-INDEX.
- **System KB:** a build target is not a control/property/event; the portable-subset list is, however, something Grace and the agents must
  know — it is part of the documentation/KB spec in WASM-INDEX.
- **Fix vs feature:** **feature** (`features-2x`).
- **Tech steering:** *no external COBOL toolchain at runtime* still holds; the new dependencies (WebAssembly target, bundler, Tauri CLI,
  platform SDKs) are **build-time** only. MSRV 1.92 applies to every crate added; the crate probe belongs to `/plan`.

## 7. Open questions

- **Q1 — The execution model. This decides the series.** (a) **Interpreter in the browser:** compile `cobolt-runtime` and the AST to
  `wasm32` and embed the AST in the bundle — the behaviour parity of R4 and the control reuse of WASM-004 come almost free, at the cost of
  making the runtime browser-capable (the blockers above: blocking channels, threads, `std::fs`, `redb`, clocks) — see proposed WASM-010.
  (b) **A COBOL → Rust transpiler** (spec 006's choice): faster and smaller output, but no such generator exists, and "full parity" was
  called the highest risk even then. **Recommendation: (a) for 2.0**, with (b) as a later optimisation. Note that (a) reverses spec 006's
  "no interpreter shipped" — say so explicitly if you agree.
- **Q2 — Bundler and framework layout:** Leptos in client-side-rendering mode with `trunk`, or `cargo-leptos`? Recommended: client-side
  rendering with `trunk` (a static bundle, no server component).
- **Q3 — Is Tauri 2 also used for the desktop kind** later (one web frontend for all), or does Desktop stay on egui? Recommended: Desktop stays
  on egui in 2.0 (R9).
- **Q4 — `EXEC RUST` in a web project:** compiled into the module (so it must itself target `wasm32`) or rejected by R3? Recommended: rejected
  in 2.0, allowed later (WASM-INDEX, external crates on `wasm32`).
- **Q5 — A size/startup budget** for the module and a measured target for first paint: set at `/clarify` once the runtime profile is measured.
- **Q6 — Where the host page's JavaScript comes from:** hand-written and shipped as a fixed file (recommended; small, auditable), or generated
  per build.
