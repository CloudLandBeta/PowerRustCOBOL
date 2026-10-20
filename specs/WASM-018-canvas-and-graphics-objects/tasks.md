<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Canvas 2D, Canvas 3D and graphics objects (`cobolt-graphics`, part 1)

- **Status:** part 1 (the crate) done; parts 2 and 3 not started
- **Plan:** ./plan.md   **Date:** 2026-10-20

Ordered, small, independently verifiable. `[x]` was done and verified in this part; `[ ]` was not
done. The commit that introduced each task is named.

## Part 1 — the crate (done)

- [x] **T1 — Foundation: display list v1, Canvas rasteriser, limits, handles** (R3, R6, R7, R15)
  - Files: `crates/cobolt-graphics/src/{display_list,canvas,limits,handles,par,pixels,color,detmath}*`
  - Verify: `cargo test -p cobolt-graphics --test canvas --test display_list_robustness` green. Commit `4f1011f1`.
- [x] **T2 — Text engine** (R8)
  - Files: `src/text/`, `src/textapi.rs`; `tests/text.rs` (synthetic fonts), `tests/text_system_fonts.rs` (real fonts)
  - Verify: Latin, Japanese, Chinese, Arabic, Hebrew; `MeasureText` equals the ink of `DrawText`. Commits `6c0c97f8`, `04969dc9`.
- [x] **T3 — ImageBuffer** (R11, R15, R20)
  - Files: `src/imagebuf/`; `tests/imagebuf.rs`
  - Verify: 1 thread equals N for every filter; header-checked pixel limit; fuzz. Commit `7373be74`.
- [x] **T4 — Plot** (R12, R20)
  - Files: `src/plot/`; `tests/plot.rs`, `tests/golden/plot-*.png`
  - Verify: 12 chart kinds, SVG, table block entry point. Commit `954b921d`.
- [x] **T5 — Physics2D / Physics3D** (R10, R20)
  - Files: `src/physics/`; `tests/physics.rs`
  - Verify: 1 thread equals N; settling positions; events; joints. Commit `2384a5a1`.
- [x] **T6 — Scene3D and Canvas3D** (R9, R15, R20)
  - Files: `src/scene3d/`; `tests/scene3d.rs`, `tests/scene3d_gpu.rs`, `tests/golden/scene3d-reference.png`
  - Verify: glTF/GLB with skin and animation, PBR, shadows, fog, instancing, culling, pick; glTF fuzz; GPU tests run on Metal. Commit `29c80b88`.
- [x] **T7 — COBOL-facing API, registry, handles with owners, crate docs** (R1 methods, R2, R15, AC2, AC8, AC9, AC10)
  - Files: `src/{value,registry,canvas2d}.rs`, `src/api/`; `tests/api.rs`, `tests/lifecycle.rs`
  - Verify: every documented method dispatches; rustdoc tables equal dispatch rows; AC9 memory baseline. Commit `9321d2c1`.
- [x] **T8 — Dependency guard** (R4, AC11)
  - Files: `tests/dependency_guard.rs`
  - Verify: passes on the real tree; fails when a denied package is added. Commit `04969dc9`.
- [x] **T9 — Measurement and size probes** (R18 measurement, R20, AC7 measurement)
  - Files: `examples/measure.rs`, `examples/size_probe.rs`
  - Verify: numbers in plan.md section 6. Commit `04969dc9`.
- [x] **T10 — plan.md and tasks.md** (this folder).

## Part 2 — hosts (not started)

- [ ] **T11 — egui form host adapter** (R16): upload the rasterised pixels as a texture; Canvas3D into a texture (Q4: decide the wgpu/glow question from the IDE's startup code); input events, `TargetFps` pacing (R13, R14).
- [ ] **T12 — Interpreter bindings** (R16): the objects as controls/non-visual controls; the closed method vocabulary in `cobolt-ast/src/methods.rs`; `RETURNING` for handles.
- [ ] **T13 — Generator and generated module** (R6, WASM-010 R7): the display list built in the transport arena, one host call.
- [ ] **T14 — Player parts** (R16, R18): load the 3D engine, charts and image filters on first use; the Build reports sizes (the crate-level probe is in plan.md).

## Part 3 — product surface (not started)

- [ ] **T15 — Controls Canvas2D and Canvas3D** (R1, R13, R14): properties, events, `.cfrm` round trip, Properties panel sections, toolbox icons, `tests/controls/`.
- [ ] **T16 — Designer placeholder** (R17).
- [ ] **T17 — System KB** (R1, steering): `cobolt-compiler` doc tables for two controls and five objects; `cargo run -p cobolt-ide --example build_chunked_kb`; commit `chunked.data`.
- [ ] **T18 — Six-language labels** (R1, steering): `Tr` fields in all six languages.
- [ ] **T19 — Gamepad** (R13): `gilrs` on the desktop, the browser's Gamepad API through the player.
- [ ] **T20 — Sample project `examples/PowerGraphics`** (R19, AC14): game, chart gallery, image lab, 3D viewer.
- [ ] **T21 — Browser and cross-platform verification** (AC1, AC5, AC6, AC7, AC12, AC13): WebGPU, WebGL2 and no-GPU browsers; plain against threaded player; native against wasm goldens; a software adapter in CI (Q5).
- [ ] **T22 — Docs and finalise**: the Developer's Guide chapter is written at release (WASM-INDEX); version and changelog by the operator's rules.

## Done criteria

Part 1: the crate builds natively and for `wasm32-unknown-unknown` for every feature combination;
`cargo test -p cobolt-graphics` green; the dependency guard passes and is proved to fail; the
measurements are in plan.md. The spec's acceptance criteria are met only where plan.md section 8
says so; AC1, AC12, AC13 and AC14 are not met by part 1, and AC5, AC4, AC6 and AC7 only in part.
