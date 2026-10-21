<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — WebAssembly build target and host page (WASM-002)

- **Status:** in progress — the assembler, the loader and the page are done; Check, Build and Mobile are the IDE-side specs'
- **Plan:** ./plan.md   **Date:** 2026-10-20

- [x] **T1 — The bundle assembler** (R1, R8, R16, R17)
  - Files: `crates/cobolt-web-bundle/src/lib.rs`, `hash.rs`
  - Verify: `cargo test -p cobolt-web-bundle` green (unit 18, `csp_origins` 2, `no_string_execution` 3, `e2e_node` 14 scenarios).
- [x] **T2 — Secret scan** (R7, AC5): `src/secrets.rs`; a deliberate test secret stops the bundle naming the file and the pattern, never the value.
- [x] **T3 — The size report** (R8): every part, module, loader, stylesheet and asset, and which parts load on first use.
- [x] **T4 — The loader and the page** (R10-R13, R15, R19): `assets/{loader,app,page}`; mount into an element or fill the viewport; shadow root when embedded; progress and a readable failure.
- [x] **T5 — The Content-Security-Policy** (R14): `src/csp.rs`, `tests/csp_origins.rs`; no `unsafe-eval`, no `unsafe-inline`, `connect-src` the configured backends only; the player runs under it in Chrome.
- [x] **T6 — Feature baseline, variants, isolation** (R18, R20): probe on Chrome 152 (baseline true, nothing missing, multi-memory true); variant choice end to end (see WASM-016).
- [x] **T7 — Development server** (R1): `examples/serve.rs` serves a bundle on 127.0.0.1 with or without isolation headers (`--isolated`, `--credentialless`), refuses to leave its folder.
- [x] **T8 — Measured matrix cells** (R20, WASM-015): yield p50 4.8 / p95 5.2 / p99 6.1 / max 6.9 ms over 384 timer samples; fixed memory unchanged after 100 cycles plus a burst (Chrome 152, multi-memory, main mode).
- [x] **T9 — Reference bundles for the tests** (AC1, AC2): the control gallery, the shell, the standalone pair, the backend bundle (`tests/support/`, `examples/fixture_bundle.rs`).
- [ ] **T10 — Check against the portable subset** (R3, R4, AC3, AC4): the IDE and the generator; not this agent's part.
- [ ] **T11 — Mobile build** (R2, AC7): WASM-012.
- [ ] **T12 — Missing-tool messages** (R6, AC6): the IDE side (WASM-001/015).
- [ ] **T13 — Desktop unchanged** (R9, AC8): nothing in this spec's code touches `bin/`, eframe or the host-triple runner; the existing compiler tests were not re-run as part of this task.
- [ ] **T14 — Firefox, Safari, iOS, Android WebView cells of the matrix** (R18): **cannot be measured here** — none of those browsers is installed on this machine. Run them where they are, with the same node and page scenarios.

## Done criteria

AC1 and AC2 are covered by the Chrome and node runs with the reference bundles (a click answered, an embedded page isolated in both directions); AC5 by `secrets.rs`; AC4b by the generator. AC3, AC4, AC6, AC7 and AC8 are not this crate's. The browser matrix is Chrome 152 only.
