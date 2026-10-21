<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The prebuilt player (WASM-016)

- **Status:** in progress — the browser half is done and measured; the IDE-side Build step (T14) and the Preview (T15) are other specs'
- **Plan:** ./plan.md   **Date:** 2026-10-20

`[x]` was done and verified; `[ ]` was not done, with the reason. The commit that introduced each task is on
`features-2x` (or, for the last ones, on the player worktree branch until the coordinator merges them).

- [x] **T1 — ABI v1 host** (R7, R8, R9, R11)
  - Files: `src/host.rs`, `src/host/{auth,params,super}_tests.rs`, `tests/abi_host.rs`
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib --test abi_host` — 219 + 21 tests green; the host
    runs the fixture module in `wasmi` in both memory variants.
- [x] **T2 — Form tree from a description** (R3, R4, R5): `src/tree.rs`; the description is the sanitised `.cfrm` (Q1).
- [x] **T3 — The view layer as data** (R5, R15): `src/view/*` returns `VNode`; every class A and B control (WASM-004) has a view and an
  interaction function with tests.
- [x] **T4 — DOM patching, events, page state** (R5, R6, R7, R15): `src/dom/*`, `src/api.rs`; `tests/sinks.rs` proves no HTML-string API,
  no string execution and no leaked closure (it caught one in this session: the `pagehide` listener of `superlink.rs`, now owned and freed).
- [x] **T5 — Connection to the backend and Authentication** (R9): `src/net.rs`, `src/auth.rs`, `src/dom/captcha.rs`; proved against a mock backend over a
  real WebSocket in Chrome (focus after a refused password, hosted return, reCAPTCHA origins).
- [x] **T6 — Routing, the two application styles, standalone windows** (R10): see WASM-011.
- [x] **T7 — The loader, the parts, the variants** (R2, R13, R13c): `crates/cobolt-web-bundle/assets/loader.js`; parts Mermaid, PDF, Canvas 2D.
- [x] **T8 — Versions and refusal of a mismatched module** (R11, AC6): `tests/abi_host.rs`, the bundle's import check.
- [x] **T9 — Two variants, one source** (R13c): plain on stable; threaded on `nightly-2026-10-08` with `-Zbuild-std`, atomics, `wasm-bindgen-rayon`.
  - Verified in Chrome 152: isolated page (`require-corp` and `credentialless`) -> threaded variant on a Worker, a pool of 8 workers;
    non-isolated page -> plain variant on the main thread, threaded files not fetched; 300,000-row DataGrid `Sort` equals a stable JS sort,
    128-138 ms threaded warm against 150-172 ms plain.
- [x] **T10 — Disposal** (R13b): `unmount()` frees everything the page holds (listeners, channels, timers, the connection); node scenario `disposal`: 100 open/close
  cycles leave one live instance. **Caveat, not fixable here:** the Rayon worker pool cannot be disposed (`wasm-bindgen-rayon` has no API for it).
- [x] **T11 — Source fingerprint and the freshness test** (R12, AC7)
  - Files: `src/fingerprint.rs`, `examples/build_player.rs`, `tests/freshness.rs`, `assets/web-player/`
  - Verify: edit any input source -> `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test freshness` fails naming the changed file's effect; rebuild with
    `cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- --out assets/web-player` -> passes. (Shown in this session; the inputs are
    listed in `fingerprint::INPUTS`. **After merging other agents' commits that touch `cobolt-forms`, `cobolt-ast`, `cobolt-wasm-abi` or `cobolt-graphics`, rebuild once.**)
- [x] **T12 — Build reports sizes** (R13, AC8): `player-build.json`; plain 2,222,253 bytes (gzip 772,899), threaded 2,187,394 (gzip 773,318), parts Mermaid 1,906,933 / Canvas 2D 469,419 / PDF 219,273.
- [x] **T13 — Dead code removed, no `allow`** (steering): the 19 warnings are gone by deleting or using the code; five placeholder functions that hid unused imports are gone too.
- [ ] **T14 — IDE Build step writes the bundle** (R2, R12, AC1): not this agent's part — WASM-001/002 IDE side. The assembler (`assemble_bundle`) and the example that proves it exist.
- [ ] **T15 — Preview accepts a new module, description or stylesheet** (R16, AC11): the player's paths exist (`set_stylesheet`, `add_form`, `attach`); the Preview is WASM-008.
- [ ] **T16 — Leptos** (R1, Q2): **not done on purpose**; deviation recorded in plan.md section 4, awaiting the operator's ruling.
- [ ] **T17 — Equality with the desktop form host by measurement** (R14, AC9): the controls were compared by their tests and by eye; the full reference-form comparison needs the desktop
  harness of WASM-004 R11 and was not run.

## Done criteria

Everything above marked `[x]` is run and evidenced. AC1, AC9 and AC11 are **not met** (they need the IDE Build, the desktop comparison harness and the Preview). AC3-AC8 and AC10 have the
tests named above (host and node scenarios; the mock backend over a real WebSocket in Chrome). AC2 is exercised control by control by the view tests, but the WASM-004 R11 property round-trip
harness (every property set to a distinct value and read back from the DOM) was not run as one. Nothing was verified on a browser other than Chrome 152.
