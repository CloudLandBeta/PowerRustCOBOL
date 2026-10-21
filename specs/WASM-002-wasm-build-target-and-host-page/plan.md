<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — WebAssembly build target and host page (WASM-002)

- **Status:** the bundle assembler, the loader and the host page are implemented and measured; the IDE-side Check/Build/tool messages (R3, R6) and the Mobile build (R2) are other specs'
- **Spec:** ./spec.md   **Date:** 2026-10-20

Tools: rustc 1.95.0 stable; `nightly-2026-10-08` (rustc 1.101.0-nightly `1d81eb4ad`, 2026-10-07) for the threaded player only; `wasm-bindgen` 0.2.129; `wasm-opt` 133; node v23.3.0; Chrome 152.0.7977.149.

## 1. Approach

`crates/cobolt-web-bundle` is a **pure function** `assemble_bundle(&BundleInput) -> Bundle`: it takes the program module (multi-memory and single-memory), the forms (sanitised `.cfrm`), menus, the stylesheet, assets and the player (variants, parts, build record), and
returns the files of a static site plus a size report. Nothing runs a toolchain there; the player build is `build_player` (WASM-016).

- **R1, R16, R17 — what is in the bundle.** `index.html` (one entry page; the router is the player's, WASM-011), the loader, the player variants, the parts, the program modules, the manifest, the form descriptions, the stylesheet, the assets, `config.json`
  and `_headers`. File names carry a content hash (`hash.rs`) so a static host can cache them forever. The program modules are exactly the generator's output; the assembler adds no interpreter, AST or runtime (AC4b is the generator's check, WASM-010 R1-R2).
- **R7 — no secret.** `secrets.rs` scans every text file for known shapes (keys, tokens, PEM blocks, connection strings, client secrets) and for long random-looking values: a known shape is an **error that names the file and the pattern, never the value**; a random-looking string is a warning.
  A picture written inline as a data URI is not a finding (it was; fixed with a test). Property names that carry a placeholder or an empty value are not findings.
- **R8 — sizes.** The report lists the player variants, each part, the modules, the loader, the stylesheet and the assets, and says which parts are loaded on first use.
- **R10-R13, R15 — the host page.** `assets/page.html` + `assets/app.js` + `assets/loader.js`. The loader mounts into the element it is told to (`options.mount`) or fills the viewport; embedded, the application lives in a shadow root so the host page's CSS and the application's do not leak
  (computed styles asserted on both sides in the node scenarios); the JavaScript surface is `mount`, `unmount`, `start parameters`, `settled()`, `part`, `partModule`, `playerConfig`; progress is shown while loading, and a load failure shows a readable error with a reload action, never a blank page.
- **R14 — CSP.** `csp.rs` builds a default-deny policy: `script-src 'self' 'wasm-unsafe-eval'` (never `unsafe-eval`, never `unsafe-inline`), `style-src 'self'` with constructed stylesheets only, `connect-src` the configured backend origins only, `img-src` the configured image origins only (a form with a Maps control adds the tile origin; a site key adds exactly the reCAPTCHA prefixes
  and only then), `require-trusted-types-for 'script'` with a default Trusted Types policy. `tests/csp_origins.rs` and the unit tests in `csp.rs` pin each.
- **R18, R19 — features and streaming.** The loader probes the browser (`WebAssembly.validate` of small modules for the baseline set: bulk memory, mutable globals, sign extension, non-trapping float-to-int, reference types, multivalue; and for multi-memory), then picks the program variant (multi-memory, else single-memory) and the player variant
  (threaded when the page is cross-origin isolated, else plain), and instantiates with `WebAssembly.instantiateStreaming` (a response with the right MIME type, an integrity digest checked before it runs). On Chrome 152: baseline true, nothing missing, multi-memory true.
- **R20 — threads when isolated.** The page's isolation comes from the headers the bundle writes (`_headers`, `Isolation::RequireCorp` or `Credentialless`) or, where headers cannot be set, from `coi-serviceworker`. Without isolation the plain player runs on the main thread, the application works, and the choice is reported in the loader's `choose` result
  (`reasons`: program module, player variant, execution: in a Worker or on the main thread).

## 2. Affected crates / files

- `crates/cobolt-web-bundle/src/{lib,csp,hash,secrets,devserver}.rs`, `assets/{page.html,app.js,app.css,loader.js,worker.js,coi-register.js,coi-serviceworker.js}`.
- `crates/cobolt-web-bundle/examples/{fixture_bundle,serve,mock_backend}.rs` — assemble a bundle around the fixture module, serve it on 127.0.0.1 (with or without isolation headers), play a backend.
- `crates/cobolt-web-bundle/tests/{e2e_node,csp_origins,no_string_execution}.rs` and `tests/support/{gallery,harness,mod}.rs` (the node harness, the control gallery, the reference bundles).
- Not here: IDE Check/Build/messages (WASM-001, WASM-015), the Tauri shell (WASM-012).

## 3. Data / model changes

`config.json` (what the loader reads: module and player entries with integrity digests, parts, style, options, backend origins, isolation) is the contract between the assembler and the loader; its keys are documented in `lib.rs`. No `.cfrm` change.

## 4. Key decisions & alternatives

- **Hand-written loader, shipped as a fixed file (Q6).** Small, auditable, no framework; `tests/no_string_execution.rs` scans it (and the threaded glue and `workerHelpers.js`) for `eval`, `new Function`, string `setTimeout`, `innerHTML` and the like.
- **No `trunk`, no Leptos shell (Q2).** The page is static files from the assembler; see WASM-016 plan section 4 for the Leptos deviation, which the operator has still to rule on.
- **`coi-serviceworker` as the fallback for hosts that cannot send headers.** Rejected as the default: a service worker is a second moving part and reloads the page once; headers are preferred; the option `service_worker_shim` adds the worker for a host that cannot set them.
- **A part is loaded with the same integrity checks as the player** and a part that failed to load may be asked for again (the failed promise is dropped from the cache).

## 5. Risks & mitigations

- Risk: a browser lacks a baseline feature. → The probe names what is missing and the loader refuses with a readable message (R15); measured only on Chrome 152.
- Risk: a static host strips `Cross-Origin-*` headers. → The plain player runs without them (the loader reports the choice and its reason).
- Risk: the first request for a hashed name races a deploy. → Names are content-addressed; an old page keeps working against its own files.

## 6. Test strategy

- `cargo test -p cobolt-web-bundle`: 18 unit tests (CSP, hashing, secrets, dev server), `csp_origins` 2, `no_string_execution` 3, and `e2e_node`: **14 scenarios** against the real player and loader under node (variant choice by probe, memory-fixed core in 4 variant/mode combinations, hosted sign-in return,
  loading fetches nothing twice, parts on first use and the Canvas part drawing a display list).
- Chrome 152, served from 127.0.0.1: the gallery bundle (a form with every control), the shell bundle, the standalone bundle (two tabs), the backend bundle against a mock backend over a real WebSocket, the threaded bundle with and without isolation headers (see WASM-016 plan section 6).
- **Measured (R20 matrix cells that can be measured here).** Yield on the main thread: 12 runs of about 150 ms, 384 timer samples, p50 4.8 ms, p95 5.2, p99 6.1, max 6.9 ms (timer-based because `requestAnimationFrame` is throttled in a background pane). Fixed memory: the data memory 64 KiB and the arena 1 MiB were unchanged after 100
  open/close cycles plus a burst of 1,000 writes (multi-memory, main mode); the node scenarios show the same in the other three combinations.
- **Not measurable here, and why:** Firefox, Safari (desktop and iOS) and Android WebView are not installed on this machine, so no cell of the matrix for them was run; the multi-memory single-memory fallback for Safari is therefore proved only by the probe's logic and by running the single-memory variant under node and Chrome.

## 7. Steering compliance

- [x] No secret in a bundle (R7), proved by tests.
- [x] COBOL identifiers English; no "cobolt" in user-facing text (the loader's texts say "the program", "the player").
- [x] `README.md` and `docs/` untouched; the guide chapter is written at the release (WASM-INDEX).
- [x] Fix vs feature: feature on `features-2x`.
- [ ] R2, R3, R6, R9 (Mobile, Check, missing-tool messages, Desktop unchanged) belong to the IDE-side specs.
