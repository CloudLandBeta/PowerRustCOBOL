<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Canvas 2D, Canvas 3D and graphics objects

- **Status:** part 1 (the crate `cobolt-graphics`) done; parts 2 (hosts) and 3 (product surface) not started.
  Revised 2026-10-22 against plan.md as revised the same day (the operator's decisions on Q4, Q5, Q7; Q8
  answered, to confirm).
- **Plan:** ./plan.md   **First written:** 2026-10-20   **Revised:** 2026-10-22
- **Branch:** `features-2x` (2.x.x). Part 1 was merged as `dab1f504` (2.0.21).

Ordered, small, independently verifiable. `[x]` was done and verified; `[ ]` is not done. Each task
names its files, the requirements (R) and acceptance criteria (AC) it serves, and the exact check.

Rules every task obeys (CLAUDE.md, CONVENTIONS.md, `specs/steering/*.md`):

- **Nothing under `docs/` and no `README.md`** on 2.x.x. What 2.x.x says about itself goes in `specs/`.
  There is no documentation task; the Developer's Guide chapter is written at release (WASM-INDEX).
- Rust only (PRIME DIRECTIVE); a CI workflow file is configuration the repository already carries
  (`.github/workflows/*.yml`), not program source.
- Interpreter/binary parity: a runtime change lands in `rcrun run-form`, embedded child forms and the
  compiled binary (all through `cobolt-form-host`, plan §1.3) and in the generated module and the player.
- User code is sacred: no task rewrites a project's COBOL; the font collection (T27) only reads it.
- `cobolt-forms` tests run with `--features render`; `cobolt-ide` tests with `--bin cobolt-ide`; the player
  is its own Cargo workspace (`exclude = ["crates/cobolt-web-player"]` in the root `Cargo.toml`), tested
  with `--manifest-path crates/cobolt-web-player/Cargo.toml`. `df -h .` before any multi-crate test run.
- Commits are the pipeline's; tasks name the merged commit ids only.

## Part 1 — the crate (done)

The commit ids below are the merged history (`git log` of `features-2x`); the ids this file named on
2026-10-20 (`4f1011f1`, `6c0c97f8`, `04969dc9`, `7373be74`, `954b921d`, `2384a5a1`, `29c80b88`,
`9321d2c1`) were the pre-merge ones (plan §0).

- [x] **T1 — Foundation: display list v1, Canvas rasteriser, limits, handles** (R3, R6, R7, R15)
  - Files: `crates/cobolt-graphics/src/{display_list,canvas,limits,handles,par,pixels,color,detmath}*`
  - Verify: `cargo test -p cobolt-graphics --test canvas --test display_list_robustness` green. Commit `7d1c2efc`.
- [x] **T2 — Text engine** (R8, AC3 at library level)
  - Files: `src/text/`, `src/textapi.rs`; `tests/text.rs` (synthetic fonts), `tests/text_system_fonts.rs` (real fonts)
  - Verify: Latin, Japanese, Chinese, Arabic, Hebrew; `MeasureText` equals the ink of `DrawText` within 1.5 px.
    Commits `ad753059` (engine), `91eeb101` (real-font tests).
- [x] **T3 — ImageBuffer** (R11, R15, R20)
  - Files: `src/imagebuf/`; `tests/imagebuf.rs`
  - Verify: 1 thread equals N for every filter; header-checked pixel limit; fuzz. Commit `9bacadc6`.
- [x] **T4 — Plot** (R12, R20)
  - Files: `src/plot/`; `tests/plot.rs`, `tests/golden/plot-*.png`
  - Verify: 12 chart kinds, SVG, table block entry point. Commit `7d3bbfe4`.
- [x] **T5 — Physics2D / Physics3D** (R10, R20)
  - Files: `src/physics/`; `tests/physics.rs`
  - Verify: 1 thread equals N; settling positions; events; joints. Commit `e29ac5a0`.
- [x] **T6 — Scene3D and Canvas3D** (R9, R15, R20)
  - Files: `src/scene3d/`; `tests/scene3d.rs`, `tests/scene3d_gpu.rs`, `tests/golden/scene3d-reference.png`
  - Verify: glTF/GLB with skin and animation, PBR, shadows, fog, instancing, culling, pick; glTF fuzz;
    GPU tests run on Metal. Commit `97961541`.
- [x] **T7 — COBOL-facing API, registry, handles with owners, crate docs** (R1 methods, R2, R15; AC2, AC8, AC9, AC10 at library level)
  - Files: `src/{value,registry,canvas2d}.rs`, `src/api/`; `tests/api.rs`, `tests/lifecycle.rs`
  - Verify: every documented method dispatches; rustdoc tables equal dispatch rows; AC9 memory baseline. Commit `9b7ef22e`.
- [x] **T8 — Dependency guard** (R4, AC11)
  - Files: `tests/dependency_guard.rs`
  - Verify: passes on the real tree; fails when a denied package is added. Commit `91eeb101`.
- [x] **T9 — Measurement and size probes** (R18 measurement, R20, AC7 measurement)
  - Files: `examples/measure.rs`, `examples/size_probe.rs`
  - Verify: numbers in plan.md §6.1. Commit `91eeb101`.
- [x] **T10 — plan.md and tasks.md, first version** (this folder). Commit `f6ea760a`; merged with `dab1f504` (2.0.21).

Evidence for T1–T9 (plan §6.1, run 2026-10-20 on macOS, Apple M3 Pro): `cargo test -p cobolt-graphics
--no-fail-fast` — 330 passed, 0 failed, 0 ignored; 20 feature/target `cargo check` combinations, native and
`wasm32-unknown-unknown`.

## Part 2 — the crate's host-facing changes, the desktop host, the runtime (not started)

- [ ] **T11 — Move the command table into `cobolt-graphics-wire`** (R6, spec §6 "generated from one definition")
  - Files: new `crates/cobolt-graphics-wire/` (`no_std`, no `alloc`: opcodes, argument kinds, encoder into
    `&mut [u8]`, `PRGD` v1 header); `crates/cobolt-graphics/src/display_list/` re-exports it and keeps decoder,
    builder and executor; root `Cargo.toml` member line; `Cargo.lock`.
  - Do: before the move, record the bytes every command encodes to (a test fixture); move; compare.
  - Verify: `cargo test -p cobolt-graphics-wire -p cobolt-graphics --no-fail-fast` green; a new test asserts every
    command in `COMMANDS` encodes to the same bytes as before and **reports the count of commands checked**;
    `cargo check -p cobolt-graphics-wire --target wasm32-unknown-unknown` green; the dependency guard
    (`--test dependency_guard`) still green.

- [ ] **T12 — `Gpu::from_shared` and the device-limit clamp** (R9, R15; Q4 decided)
  - Files: `crates/cobolt-graphics/src/scene3d/gpu.rs` (`Gpu::new`/`new_async` at 123/128 stay for tests and tools),
    `src/limits.rs`; `tests/scene3d_gpu.rs`
  - Do: `Gpu::from_shared(device, queue, adapter_info)`; at Canvas3D creation clamp `Limits::max_texture_size` and
    the shadow-map size to `device.limits()`; past them, a readable refusal.
  - Verify: `cargo test -p cobolt-graphics --test scene3d_gpu --no-fail-fast` green on Metal locally (loud `SKIPPED`
    where no adapter); new tests: a `Gpu` built from a shared device renders the reference scene; a texture
    larger than the device limit is refused with a message naming the limit.

- [ ] **T13 — Explicit `Renderer::Wgpu` in the IDE and every form-host window** (Q4 decided, R16)
  - Files: `crates/cobolt-ide/src/main.rs` (`NativeOptions` at 144-161), `crates/cobolt-form-host/src/lib.rs`
    (`native_options` at 132)
  - Do: set `renderer: eframe::Renderer::Wgpu` in both; extend the pin `wgpu_backend_rule_matches_the_ide`
    (it lives at `crates/cobolt-ide/src/main.rs:347`, not in the form host as plan §1.3 implies) to the renderer.
    Run `cargo tree -i egui_glow` and record its answer in plan.md §7 item 4(b) (spec claim "both glow and wgpu
    are compiled in"). The Windows backend rule is unchanged.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide wgpu_backend_rule_matches_the_ide` and
    `cargo test -p cobolt-form-host --no-fail-fast` green; the IDE launches (operator's check of the running build,
    version string confirmed).

- [ ] **T14 — Seven `ControlType` variants and their `.cfrm` round trip** (R1, R2)
  - Files: `crates/cobolt-forms/src/model.rs` (`ControlType`, `ALL`, default sizes), `src/xml.rs`;
    `crates/cobolt-web-player/src/kinds.rs` (name-based `PlayerKind::Canvas2D/Canvas3D` now matches the real
    variants; the table test at `kinds.rs:241` forces the classification)
  - Do: `Canvas2D`, `Canvas3D` (visual) and `ImageBuffer`, `Plot`, `Scene3D`, `Physics2D`, `Physics3D`
    (non-visual) with R1's properties (`Width`, `Height`, `PixelRatio`, `Smoothing`, `BackgroundColor`,
    `TargetFps`, `AutoPresent`, `AccessibleLabel`; Canvas3D adds `ClearColor`, `Msaa`, read-only `Backend` with the
    values of R1 plus `OpenGL`, plan §1.8) and events. Additive format: an older reader keeps them as `Custom`.
  - Verify: `cargo test -p cobolt-forms --features render --no-fail-fast` green with a new round-trip test per type
    that saves and reloads every R1 property (**loads a fixture**, not only `Control::new`);
    `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml kinds` green;
    `cargo test -p cobolt-codegen --no-fail-fast` green and its snapshots unchanged (plan §8: codegen emits nothing
    new beyond the control declarations; banner and regenerate contract untouched).

- [ ] **T15 — Closed method vocabulary** (R1, spec §6)
  - Files: `crates/cobolt-ast/src/methods.rs` (`is_known_method` at 85); a test in `cobolt-ast` or a crate that may
    depend on `cobolt-graphics` (dev-dependency only; the AST crate gains no runtime dependency)
  - Do: add every name in `Registry::methods(kind)` (`crates/cobolt-graphics/src/registry.rs:527`) for the seven kinds.
  - Verify: the new test walks the registry rows and fails on a missing name; it **reports the count** (313 today,
    plan §1.1); `cargo test -p cobolt-ast -p cobolt-parser --no-fail-fast` green.

- [ ] **T16 — Runtime feature `graphics` and the compiler's feature scan** (R16, parity)
  - Files: `crates/cobolt-runtime/Cargo.toml` (`[features]`, beside `images`/`audio`),
    `crates/cobolt-compiler/src/runtime_features.rs` (`RuntimeFeatures`, `scan_forms`, `scan_program`)
  - Verify: `cargo test -p cobolt-compiler runtime_features --no-fail-fast` green with tests: a form with any of the
    seven types selects `graphics`, a program without them does not (no wgpu/rapier linked);
    `cargo check -p cobolt-runtime --no-default-features` and `--features graphics` both green.

- [ ] **T17 — Interpreter bindings and display-list recording** (R2, R6, R7, R16; AC2, AC9 in the interpreter)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (and a `graphics` module beside it)
  - Do: one `Registry` per running application; each object owned by its form instance; `release_owner`
    (`registry.rs:376`) when the form closes; dispatch by name through `Registry::invoke` (`registry.rs:418`);
    handles back through `RETURNING` and inline calls. Drawing methods append to the canvas's recording;
    `Present`, `Flush`, the end of an `onFrame` handler with `AutoPresent`, and `ToImage`/`GetPixel`/`MeasureText`/
    `IsPointInPath` execute it in one call. Project-level limits (R15) from the project settings, defaulting to
    `Limits::default()` (`limits.rs:67`).
  - Verify: `cargo test -p cobolt-runtime --no-fail-fast` green with new tests: a COBOL program creates each of the
    seven kinds, gets a handle with `RETURNING`, calls one method each; closing the form releases them (**count
    before/after reported**); 10,000 shapes then `Present` execute one list (reported count of executions = 1);
    `ToImage` after drawing returns the drawn pixels; an oversized canvas is refused with a readable error.

- [ ] **T18 — Form-host adapter: Canvas2D and Canvas3D on the desktop** (R16; AC13 desktop, AC4/AC5 natively)
  - Files: `crates/cobolt-form-host/src/{lib,host}.rs`; `crates/cobolt-forms/src/render.rs` (run-form path)
  - Do: Canvas2D — on `Present`, `Surface::write_rgba` into one `TextureHandle` per canvas (`set`, never re-created).
    Canvas3D — `Gpu::from_shared` from `eframe::Frame::wgpu_render_state()`; render into its own target registered
    with `register_native_texture`; no read-back on the frame path; a lost device or surface rebuild re-creates the
    renderer and raises the event; no adapter shows the readable message (Tr from T22).
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast` and `cargo test -p cobolt-forms --features render
    --no-fail-fast` green; a GPU test (lavapipe in CI, T29) creates Canvas3D from a `RenderState`, renders, reports
    `Backend`; the operator checks Run Form, `rcrun run-form`, a child form and a compiled binary (MCP `run_form`,
    never UI automation).

- [ ] **T19 — Desktop input and frame pacing** (R13, R14; AC12 on the desktop)
  - Files: `crates/cobolt-form-host/src/host.rs`
  - Do: `onPointerDown/Move/Up/Enter/Leave`, `onWheel`, `onKeyDown/Up` (match modifiers exactly),
    `onTouchStart/Move/End`, `onResize`, `onFrame` with elapsed ms — only for bound handlers; pollable state
    (`IsKeyDown`, `PointerX`, `PointerY`) per canvas; focusable canvas with `AccessibleLabel` as its accessible name;
    `request_repaint_after(1/TargetFps)`, paused while minimised or occluded; fixed-step mode raising steps of
    exactly `1000/TargetFps` ms.
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast` with new tests: `onFrame` rate within a stated tolerance
    (reported), fixed-step deltas exact, hidden = no frames, each event kind reaches a bound handler and none
    reaches an unbound one.

- [ ] **T20 — Designer placeholder** (R17; AC13 Designer)
  - Files: `crates/cobolt-forms/src/paint.rs` (designer path)
  - Do: a labelled placeholder with the control's name, size and (Canvas3D) backend; the run form path (T18)
    draws the live picture. If the placeholder changes how a control border, radius or clip is drawn, run the GUI
    Border Validation Agent (`.agents/agents/gui-border-validation-agent.md`) before closing the task.
  - Verify: `cargo test -p cobolt-forms --features render --no-fail-fast` green with a test that loads a fixture
    `.cfrm` with both canvases and asserts the painted label text and position; the operator judges the look
    (MCP `render_form`).

- [ ] **T21 — Toolbox, icons, Properties-panel sections, `tests/controls/`** (R1)
  - Files: `crates/cobolt-ide/src/` (toolbox and Properties panel), the icon catalogue, `tests/controls/` (one test
    file per new type plus its `[[test]]` entry in `tests/controls/Cargo.toml`)
  - Verify: `cargo test --manifest-path tests/controls/Cargo.toml --no-fail-fast` green (seven new tests reported);
    `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` green; every project kind's toolbox lists the seven.

- [ ] **T22 — Six-language strings** (R1, steering i18n)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: `Tr` fields in en, es, pt (pt-BR), fr, ja, zh for the toolbox labels, Properties sections and the readable
    messages: no adapter, lost device, limit refused, unresolved font (T27).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` green (no empty translation); Properties panel frames
    checked in all six languages for width growth.

- [ ] **T23 — System KB** (R1, spec §6, steering System KB rule)
  - Files: the `cobolt-compiler` documentation tables (Rust constants: two controls, five objects, every property,
    event and method, generated from the same `Method` rows as the rustdoc tables, plus the HTML-canvas difference
    table); `assets/knowledge/chunked.data` regenerated with `cargo run -p cobolt-ide --example build_chunked_kb`
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    (`crates/cobolt-ide/src/grace_host.rs:4780`) green after regeneration; `cargo test -p cobolt-compiler
    --no-fail-fast` green; `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test freshness` green.

## Part 3 — generated module, player, fonts, CI, sample (not started)

- [ ] **T24 — ABI amendment A10 and the generator's display list** (R6, R16; AC2, AC13 parity)
  - Files: `specs/WASM-ABI-v1.md` (A10: batch command `0x06 DRAW` = `path`, `varint len`, list bytes; read-backs as
    `CALLR` after the pending `DRAW`; §12.2 rows for the canvas events' argument texts), `crates/cobolt-wasm-abi`,
    `crates/cobolt-wasm`, `crates/cobolt-wasm-rt` (links `cobolt-graphics-wire`'s encoder; the list lives in the arena)
  - Verify: `cargo test -p cobolt-wasm -p cobolt-wasm-rt -p cobolt-wasm-abi --no-fail-fast` green with: (AC2) 10,000
    shapes then `Present` = 1 flush carrying 1 `DRAW` (**calls and bytes reported**); (AC13) the same program's list
    bytes from the interpreter (T17) and from the module on `wasmi` are equal (**byte counts reported**); a module
    without canvases never emits `DRAW`. NIST NC still 95/95 on the generator (the gate in memory
    `wasm-generator-nist-nc-finished`).

- [ ] **T25 — Player: `DRAW`, read-backs answered, the canvas part with text and images** (R7, R8, R16; AC1, AC2, AC3 on the web)
  - Files: `crates/cobolt-web-player/src/methods.rs` (`canvas_method` at 554 stops refusing read-backs, message at 562),
    `src/dom/canvas.rs`, `src/view/controls.rs`, `parts/canvas/{Cargo.toml,src/lib.rs}` (`cobolt-graphics` with
    `text` + `imagebuf`)
  - Do: handle `DRAW`; keep accepting the old `canvas-call` form until the generator emits `DRAW`; answer read-backs,
    suspending the task while the part loads (A5); events with the A10 argument texts.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast` green with: `GetPixel`
    after drawing answers the drawn colour; a read-back during part load suspends and resumes; `DrawText` and
    `DrawImage` draw in the part. Part size before/after reported.

- [ ] **T26 — Player parts for plot, scene3d, physics (and ImageBuffer by measurement)** (R16, R18; AC5, AC14 part)
  - Files: `crates/cobolt-web-player/parts/*`, `examples/build_player.rs`, the Build report (`BuildReport`/`PartSize`,
    WASM-016 plan §1.7)
  - Do: measure whether ImageBuffer rides with the canvas part or is its own (plan §1.4); one `Registry` per form
    instance so handles cross objects; scene3d with `webgpu` + `webgl` (`crates/cobolt-graphics/Cargo.toml:95-108`);
    `Backend` reports WebGPU then WebGL2; neither → readable message and event.
  - Verify: player tests green; the Build report lists each part's size (**numbers recorded in plan §6**); a form
    without a 3D control never fetches the scene3d part (request log).

- [ ] **T27 — Fonts: every font the project references** (R8; Q7 decided; AC1, AC3)
  - Files: `crates/cobolt-project-tools` (web build), the desktop font registration (`crates/cobolt-forms/src/fonts.rs`,
    `system_fonts` at 30), the web bundle manifest
  - Do: collect `.cfrm` `Font` families, `SetFont` literals, `Assets/` font files; resolve with `fontdb`; unresolved
    family = error on Web/Mobile, warning on Desktop; warn on a non-literal `SetFont`; read `OS/2.fsType` and the
    licence string, refuse a font that forbids embedding, list them in the Build report; ship under `fonts/` and hand
    to `FontSet::add_font_data` when the text part loads; the desktop registers the same files.
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast` green with: a project referencing one family in a
    `.cfrm`, one in a `SetFont` literal and one `Assets/` file ships exactly those three; an unresolvable family fails a
    Web build; an `fsType`-restricted font is refused. The project's COBOL is read, never written.

- [ ] **T28 — Gamepad** (R13; AC12 gamepad)
  - Files: `crates/cobolt-form-host` (`gilrs`), `crates/cobolt-web-player` (Gamepad API); `GamepadAxis`, `GamepadButton`
  - Do: add `gilrs` only if the dependency guard passes for it on Linux (plan §5: udev not verified); if it fails,
    Linux gamepad input is reported as a gap in plan.md, the guard is **not** relaxed.
  - Verify: `cargo test -p cobolt-graphics --test dependency_guard` green; a host test feeds a synthetic gamepad event
    to a bound handler on each side.

- [ ] **T29 — CI job on a software Vulkan, no fallback** (Q5 decided; AC4, AC5 software adapter)
  - Files: `.github/workflows/` (a job in `build-all-platforms.yml` or a new file)
  - Do: Linux, Mesa lavapipe, `WGPU_BACKEND=vulkan GRAPHICS_REQUIRE_GPU=1 cargo test -p cobolt-graphics --test scene3d_gpu`;
    a missing adapter is red, never skipped. Measure the golden tolerance on lavapipe and on Metal.
  - Verify: the job runs green on a push; both tolerances recorded in plan §6; `Backend` reports `Software` on lavapipe.

- [ ] **T30 — Cross-platform parity and determinism** (R20; AC1, AC3, AC5, AC6, AC7, AC12 browser)
  - Files: tests in `cobolt-form-host`, the player, `cobolt-graphics`
  - Verify, each reporting its numbers: (AC1) the same list on the desktop host and the canvas part (native and
    `wasm32` in a headless engine) gives identical RGBA — **differing pixels = 0**, including text in six languages;
    (AC3) `MeasureText` = `DrawText` extent and identical wrap points on both; (AC6) stacked boxes settle to the same
    positions on desktop, plain and threaded player, and a bound body moves its sprite; (AC7) resize + blur + encode
    gives the same bytes on 1 and N threads and plain vs threaded player; the 24 MP blur time on both players is
    measured and the page stays responsive; (AC5) WebGPU, WebGL2-only and no-GPU browsers report `Backend` or show the
    message; (AC12) input and `onFrame` in the browser, paused when the tab is hidden.

- [ ] **T31 — Lifecycle and limits on every host** (R2, R15; AC9, AC10)
  - Files: tests in `cobolt-form-host` and the player
  - Verify: 100 open/close cycles of a form with a canvas, an image and a scene leave memory at baseline on the
    desktop and the player (`pr_dbg_live`/region use; **numbers reported**); an oversized canvas, image, model and
    display list are each refused with a readable error through COBOL; corrupt image and model rejected without a
    crash; the crate's fuzz targets (`display_list_robustness`, `imagebuf`, `scene3d`) run within budget.

- [ ] **T32 — Sample project `examples/PowerGraphics`** (R19; AC14, and AC8 from COBOL)
  - Files: `examples/PowerGraphics/` (game: sprites, physics, input; chart gallery: every Plot kind into a Canvas2D,
    an ImageBuffer and SVG, a 100,000-value table; image lab; 3D viewer)
  - Do: COBOL in the RustCOBOL extensions (skill `rustcobol-code-style`); no scratch items for properties.
  - Verify: the project checks clean (`rcrun check`); it builds and runs on Desktop (compiled binary) and Web in the
    test suite; the Web build fetches the 3D part only when the 3D form is shown; the staged diff passes the
    credential scan (`APIKey|Password|Token|AKIA|ASIA` and the operator's account id) **as a gate** before commit.

- [ ] **T33 — Finalize**
  - Bump `z` **once** in `crates/cobolt-ide/src/version.rs` (`VERSION`, 2.0.30 when this was written; take the value
    current at the time) and add **one** `CHANGELOG.md` entry above the previous `## [PowerRustCOBOL …]` heading,
    dated, with the line `*(Written on `features-2x`, after <previous>.)*`. No `README.md`, nothing under `docs/`.
  - Run the tests of every crate touched, with `--no-fail-fast`, and read every `test result` line:
    `cargo test -p cobolt-graphics-wire -p cobolt-graphics -p cobolt-ast -p cobolt-parser -p cobolt-runtime
    -p cobolt-compiler -p cobolt-codegen -p cobolt-form-host -p cobolt-project-tools -p cobolt-wasm -p cobolt-wasm-rt
    -p cobolt-wasm-abi --no-fail-fast`; `cargo test -p cobolt-forms --features render --no-fail-fast`;
    `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`; `cargo test --manifest-path
    crates/cobolt-web-player/Cargo.toml --no-fail-fast`; `cargo test --manifest-path tests/controls/Cargo.toml
    --no-fail-fast`.
  - Verify: all green (or each failure named with its output); the running IDE shows the new version string; plan.md
    §9 updated with what is met.

## Coverage of the acceptance criteria

| AC | Tasks whose verification covers it |
|---|---|
| AC1 — pixel-identical Canvas2D on desktop, browser, mobile web view | T18, T25, T27, T30 |
| AC2 — one host call for 10,000 shapes; `ToImage` | T7 (library), T17 (interpreter), T24 (module), T25 (player) |
| AC3 — `MeasureText` = `DrawText`; same wrapping everywhere | T2 (library), T25, T27, T30 |
| AC4 — glTF with animation, lights, shadows on a software adapter; golden; `Pick` | T6 (Metal), T12, T18, T29 |
| AC5 — `Backend` on WebGPU, WebGL2, neither | T6, T18, T26, T29, T30 |
| AC6 — Physics2D settles identically on desktop, plain and threaded player; binding | T5 (library), T30 |
| AC7 — same bytes 1 vs N threads; threaded player faster, page not frozen | T3, T9 (library), T30 |
| AC8 — every Plot kind into Canvas2D, ImageBuffer, SVG; 100,000-value table in one pass | T4, T7 (library), T32 (from COBOL) |
| AC9 — handles released with object and form; 100 cycles at baseline | T7 (library), T17, T31 |
| AC10 — limits refuse; corrupt image/model rejected; fuzz in budget | T3, T6, T7 (library), T12, T17, T31 |
| AC11 — dependency guard passes and fails on a denied package | T8, T11, T28 |
| AC12 — input and `onFrame` on desktop and browser; pause; fixed step | T19, T28, T30 |
| AC13 — Designer placeholder; live drawing in Preview, Run Form, binary, player; interpreter = module list | T18, T20, T24 |
| AC14 — sample builds and runs on Desktop and Web; 3D part loaded on demand | T26, T32 |

## Open, not decided by this task list

- **Q8 (audio): answered, to confirm — still open.** Plan §1.7 verified the desktop-only, file-path mechanism
  (spec 092, `COBOL::"AUDIO-PLAY"` behind `cobolt-runtime`'s `audio` feature). **Working assumption:** WASM-018 adds
  no audio task; web and mobile audio would be a separate spec. The operator closes it.
- **Q1, Q2, Q6:** recorded choices (`tiny-skia`, own scene layer, `ImageBuffer`/`Plot`); no task changes them.
- **Spec edits proposed by the plan (§7 item 4)** — the "Q7" references that mean Q4, the glow claim (T13 records
  `cargo tree -i egui_glow`), `OpenGL` missing from R1's `Backend` list, and R5's replaced crates — are for
  `/clarify`; this list only writes `tasks.md`.

## Done criteria

Every acceptance criterion above is checked by a green task; T33's test runs are green; nothing under `docs/` or
`README.md` changed; the System KB and `chunked.data` match; parity holds across the interpreter, the egui host in
its three uses, the compiled binary, the generated module and the player.
