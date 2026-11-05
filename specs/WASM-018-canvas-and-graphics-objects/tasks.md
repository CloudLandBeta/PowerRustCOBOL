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

- [x] **T11 — Move the command table into `cobolt-graphics-wire`** (R6, spec §6 "generated from one definition")
  - Files: new `crates/cobolt-graphics-wire/` (`no_std`, no `alloc`: opcodes, argument kinds, encoder into
    `&mut [u8]`, `PRGD` v1 header); `crates/cobolt-graphics/src/display_list/` re-exports it and keeps decoder,
    builder and executor; root `Cargo.toml` member line; `Cargo.lock`.
  - Do: before the move, record the bytes every command encodes to (a test fixture); move; compare.
  - Verify: `cargo test -p cobolt-graphics-wire -p cobolt-graphics --no-fail-fast` green; a new test asserts every
    command in `COMMANDS` encodes to the same bytes as before and **reports the count of commands checked**;
    `cargo check -p cobolt-graphics-wire --target wasm32-unknown-unknown` green; the dependency guard
    (`--test dependency_guard`) still green.
  - Evidence (2026-11-03, macOS, 2.0.188 on `features-2x-wasm018`): the bytes of all 61 commands were recorded
    first, on the untouched code (`tests/wire_bytes.rs` → `tests/golden/display-list-v1-commands.txt`); after the move
    `cargo test -p cobolt-graphics-wire -p cobolt-graphics --no-fail-fast` — every `test result` line `ok`
    (wire 6 passed; graphics lib 194, api 15, canvas 24, dependency_guard 4, display_list_robustness 5, imagebuf 6,
    lifecycle 2, physics 13, plot 13, scene3d 13, scene3d_gpu 16, text 17, text_system_fonts 5, wire_bytes 1; 0 failed).
    Both encoders reproduce the file: the wire crate's own no-alloc one (`cobolt-graphics-wire: 61 commands encode to
    the recorded bytes`) and `Command::encode` in `cobolt-graphics` (`wire_bytes: 61 commands checked`).
    `cargo check -p cobolt-graphics-wire --target wasm32-unknown-unknown` green. How it is built: the table is written
    once, in `with_command_table!` (wire crate); both crates expand it; `consts` (the one-byte enums) moved to the wire
    crate and is re-exported; header constants and `write_header` live there; the operands' byte forms are the wire
    crate's `put_*`. `no_std`, no dependencies. Not measured: the generated module does not link the crate yet (T24).

- [x] **T12 — `Gpu::from_shared` and the device-limit clamp** (R9, R15; Q4 decided)
  - Files: `crates/cobolt-graphics/src/scene3d/gpu.rs` (`Gpu::new`/`new_async` at 123/128 stay for tests and tools),
    `src/limits.rs`; `tests/scene3d_gpu.rs`
  - Do: `Gpu::from_shared(device, queue, adapter_info)`; at Canvas3D creation clamp `Limits::max_texture_size` and
    the shadow-map size to `device.limits()`; past them, a readable refusal.
  - Verify: `cargo test -p cobolt-graphics --test scene3d_gpu --no-fail-fast` green on Metal locally (loud `SKIPPED`
    where no adapter); new tests: a `Gpu` built from a shared device renders the reference scene; a texture
    larger than the device limit is refused with a message naming the limit.
  - Evidence (2026-11-03, macOS Apple M3 Pro, Metal, 2.0.189): `cargo test -p cobolt-graphics --no-fail-fast` — every
    `test result` ok; `scene3d_gpu` 18 passed (was 16): `a_gpu_built_from_a_shared_device_renders_the_reference_scene`
    (device opened by the test, `Gpu::from_shared`, matches the golden; prints `shared device: Apple M3 Pro on Metal`) and
    `a_texture_larger_than_the_device_limit_is_refused_naming_the_limit` (device max 16384; a 16385-texel texture is refused:
    "texture edge: 16385 requested, but this graphics device creates textures up to 16384 texels on a side (its
    max_texture_dimension_2d)"). `Canvas3D::connect_with` holds `Limits::max_texture_size` to the device's
    (`Limits::clamped_to_device`); `Renderer::new` refuses a shadow map, `render` a picture or texture, past it.
    `cargo check -p cobolt-graphics --target wasm32-unknown-unknown` green. Not measured: the shadow-map refusal branch
    (needs a device whose limit is under 8192; this Metal device allows 16384, so that assertion is skipped here and
    runs where the limit is lower, e.g. the software adapter of T29). A shared device installs no loss callback
    (`is_lost` stays false: the host watches, T18).

- [x] **T13 — Explicit `Renderer::Wgpu` in the IDE and every form-host window** (code and tests done; the operator's launch check is open) (Q4 decided, R16)
  - Files: `crates/cobolt-ide/src/main.rs` (`NativeOptions` at 144-161), `crates/cobolt-form-host/src/lib.rs`
    (`native_options` at 132)
  - Do: set `renderer: eframe::Renderer::Wgpu` in both; extend the pin `wgpu_backend_rule_matches_the_ide`
    (it lives at `crates/cobolt-ide/src/main.rs:347`, not in the form host as plan §1.3 implies) to the renderer.
    Run `cargo tree -i egui_glow` and record its answer in plan.md §7 item 4(b) (spec claim "both glow and wgpu
    are compiled in"). The Windows backend rule is unchanged.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide wgpu_backend_rule_matches_the_ide` and
    `cargo test -p cobolt-form-host --no-fail-fast` green; the IDE launches (operator's check of the running build,
    version string confirmed).
  - Evidence (2026-11-03, 2.0.190): `RENDERER` constants in `cobolt-ide/src/main.rs` and `cobolt-form-host/src/lib.rs`
    (`pub const RENDERER`, applied by `native_options`); new test `the_renderer_is_explicit_and_the_same_in_the_ide_and_the_form_host`
    ok together with `wgpu_backend_rule_matches_the_ide` and `windows_never_reaches_for_vulkan`.
    `cargo test -p cobolt-form-host --no-fail-fast`: 187 + 1 + 1 + 1 + 21 + 0 passed, 0 failed. `cargo tree -i egui_glow` for
    `cobolt-ide` and `cobolt-form-host`: "nothing to print" (recorded in plan §7 item 4(b)). Not verified: the filter `backend`
    also ran 5 unrelated IDE tests (`backend_run`, `web_preview`) that FAIL here because `target/debug/rcrun` was not built in
    this worktree ("run `cargo build -p cobolt-cli --bin rcrun` first") — environmental, not caused by this change; the full
    `cargo test -p cobolt-ide --bin cobolt-ide` run is left for T21/T33. The running IDE launch is the operator's check.

- [x] **T14 — Seven `ControlType` variants and their `.cfrm` round trip** (R1, R2)
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

- [x] **T15 — Closed method vocabulary** (R1, spec §6)
  - Files: `crates/cobolt-ast/src/methods.rs` (`is_known_method` at 85); a test in `cobolt-ast` or a crate that may
    depend on `cobolt-graphics` (dev-dependency only; the AST crate gains no runtime dependency)
  - Do: add every name in `Registry::methods(kind)` (`crates/cobolt-graphics/src/registry.rs:527`) for the seven kinds.
  - Verify: the new test walks the registry rows and fails on a missing name; it **reports the count** (313 today,
    plan §1.1); `cargo test -p cobolt-ast -p cobolt-parser --no-fail-fast` green.
  - Evidence (2026-11-03, 2.0.191): `crates/cobolt-ast/tests/test_graphics_methods.rs` walks `Registry::methods` for every
    available kind. Before the change it failed with 299 of 313 methods missing; after adding 244 distinct names to
    `is_known_method` it prints `graphics methods checked: 313 on 7 object kinds, 0 missing`.
    `cargo test -p cobolt-ast -p cobolt-parser --no-fail-fast`: every `test result` line ok, 0 failed. Not run: the
    semantic, runtime and generator suites that also read `is_known_method` (T17 and T24 run theirs).

- [x] **T16 — Runtime feature `graphics` and the compiler's feature scan** (R16, parity)
  - Files: `crates/cobolt-runtime/Cargo.toml` (`[features]`, beside `images`/`audio`),
    `crates/cobolt-compiler/src/runtime_features.rs` (`RuntimeFeatures`, `scan_forms`, `scan_program`)
  - Verify: `cargo test -p cobolt-compiler runtime_features --no-fail-fast` green with tests: a form with any of the
    seven types selects `graphics`, a program without them does not (no wgpu/rapier linked);
    `cargo check -p cobolt-runtime --no-default-features` and `--features graphics` both green.
  - Evidence (2026-11-03, 2.0.192): feature `graphics = ["dep:cobolt-graphics"]` in `cobolt-runtime` (in `default`, so `rcrun` and
    the IDE keep the full surface) and forwarded by `cobolt-form-host`; `RuntimeFeatures.graphics` set by `scan_forms` for any of
    the seven control names (also inside a container; also the `unknown:<name>` spelling an older build reads), by
    `scan_control_class`, by an `EXEC RUST` block naming `cobolt_graphics`, and by a computed `CALL` (everything on); never part
    of `shared_platform`. New test `a_form_with_a_graphics_control_links_the_graphics_engine_and_one_without_does_not` prints
    `graphics selected by 7 of 7 control names; 0 by a Button form`. `cargo test -p cobolt-compiler --no-fail-fast`: lib 225
    passed, 0 failed, 1 ignored, and every other `test result` ok. `cargo check -p cobolt-runtime --no-default-features` and
    `--no-default-features --features graphics` finish; `cargo tree -p cobolt-runtime --no-default-features` lists 0 wgpu/rapier
    lines. Found on the way: the redistributable SDK (`SDK_CRATES`, 19 -> 21) must now ship `cobolt-graphics` and
    `cobolt-graphics-wire`, or the closure guard and the generated-binary resolve test fail (they did, before the fix).

- [x] **T17 — Interpreter bindings and display-list recording** (R2, R6, R7, R16; AC2, AC9 in the interpreter)
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
  - Evidence, first part (2026-11-04, 2.0.195): `crates/cobolt-runtime/src/interpreter/graphics.rs` (feature `graphics`), routed by class
    in `exec_method`, released at the end of `run()`. Tests: seven kinds made and released (7 live, 7 released, 0 after); 10,000
    `FillRect` then `Present` = 1 execution (665 ms recording, 63 ms present); `GetPixel` returns the drawn colour; oversized canvas
    refused naming the limit; `CreateWorld` handle taken by the next call. `cargo test -p cobolt-runtime --lib`: 552 passed.
    **Still open in T17:** project limits from settings (defaults used), `AutoPresent` at the end of `onFrame`, a COBOL-source test
    with `RETURNING`, `ToImage` returning pixels, and the full `cargo test -p cobolt-runtime --no-fail-fast` sweep (disk full).
  - Evidence, closed (2026-11-04, 2.0.196): `AutoPresent` True = 1 execution at the end of `onFrame`, False = 0; project limit
    `max_canvas_width=200` refuses a 300-pixel canvas naming the setting (`Limits::from_settings`, `publish_graphics_limits`);
    `ToImage` returns the drawn pixels; `tests/test_graphics_objects.rs` parses COBOL source (`MOVE CANVAS-1::GetPixel(20 20) TO
    WS-PIXEL` = 543211775, `MOVE PHYS-1::CreateWorld(0 -9.81) TO WS-WORLD`, 0 objects live after the run). Sweep:
    `cargo test -p cobolt-runtime --no-fail-fast` lib 555 passed, 199 integration files all `0 failed` (1,088 passed, 26 ignored);
    `cobolt-graphics --lib` 195 passed. **Left for T21/T23:** the manifest key and the Settings control that call `publish_graphics_limits`.

- [x] **T18 — Form-host adapter: Canvas2D and Canvas3D on the desktop** (code and tests done; the operator's check of the four hosts is open) (R16; AC13 desktop, AC4/AC5 natively)
  - Files: `crates/cobolt-form-host/src/{lib,host}.rs`; `crates/cobolt-forms/src/render.rs` (run-form path)
  - Do: Canvas2D — on `Present`, `Surface::write_rgba` into one `TextureHandle` per canvas (`set`, never re-created).
    Canvas3D — `Gpu::from_shared` from `eframe::Frame::wgpu_render_state()`; render into its own target registered
    with `register_native_texture`; no read-back on the frame path; a lost device or surface rebuild re-creates the
    renderer and raises the event; no adapter shows the readable message (Tr from T22).
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast` and `cargo test -p cobolt-forms --features render
    --no-fail-fast` green; a GPU test (lavapipe in CI, T29) creates Canvas3D from a `RenderState`, renders, reports
    `Backend`; the operator checks Run Form, `rcrun run-form`, a child form and a compiled binary (MCP `run_form`,
    never UI automation).
  - Evidence, Canvas2D half (2026-11-04, 2.0.197): `cobolt-runtime/src/canvas_frames.rs` (slot per canvas, `_CanvasFrame` update),
    `cobolt-form-host/src/canvas.rs` (one `TextureHandle` per canvas, `set` in place, same texture id after a second Present),
    `paint::draw_canvas_surface` (picture or placeholder) fed from both `pic_tex` sites in `render.rs`. Corner harnesses needed
    `self_clipping_type` to exclude both canvases. `cobolt-forms` 97 result lines 0 failed, `cobolt-form-host` 212 passed,
    `cobolt-runtime --lib` 557 passed, `cargo check --workspace --all-targets` ok.
  - Evidence, Canvas3D half (2026-11-04, 2.0.198): `cobolt-runtime/src/gpu_share.rs` (the window's device, render-target slots,
    `_Canvas3DFrame`), `cobolt-form-host/src/canvas.rs` (`share_window_device` from `Frame::wgpu_render_state()` in both
    `ui` entry points; `register_target` = `register_native_texture` once, same id afterwards),
    `interpreter/graphics.rs` (`graphics_sync_gpu` follows a replaced device and reconnects, `graphics_publish_3d`),
    `paint::canvas_texture` answers a `TextureId` for either canvas. GPU tests on Apple M3 Pro / Metal: form-host
    `a_canvas3d_draws_on_the_window_device_and_egui_shows_its_target` (same egui texture after a second Render, sphere pixel
    red, background black) and runtime `a_canvas3d_draws_on_the_published_device_and_follows_a_replaced_one` (one Render =
    one frame update; replaced device reported in `LastError`). `cobolt-runtime --lib` 558 passed, `cobolt-form-host --lib`
    189 passed, `cargo check --workspace --all-targets` ok. **Open (operator):** Run Form, `rcrun run-form`, a child form and
    a compiled binary on screen; the software-Vulkan run is T29; the translated no-adapter placeholder is T22.

- [x] **T19 — Desktop input and frame pacing** (code and tests done; gamepad is T27, the operator's check by hand is open) (R13, R14; AC12 on the desktop)
  - Files: `crates/cobolt-form-host/src/host.rs`
  - Do: `onPointerDown/Move/Up/Enter/Leave`, `onWheel`, `onKeyDown/Up` (match modifiers exactly),
    `onTouchStart/Move/End`, `onResize`, `onFrame` with elapsed ms — only for bound handlers; pollable state
    (`IsKeyDown`, `PointerX`, `PointerY`) per canvas; focusable canvas with `AccessibleLabel` as its accessible name;
    `request_repaint_after(1/TargetFps)`, paused while minimised or occluded; fixed-step mode raising steps of
    exactly `1000/TargetFps` ms.
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast` with new tests: `onFrame` rate within a stated tolerance
    (reported), fixed-step deltas exact, hidden = no frames, each event kind reaches a bound handler and none
    reaches an unbound one.
  - Evidence (2026-11-04, 2.0.199): `cobolt-forms/src/canvas_input.rs` (`FrameClock`, `run`), called from `render_interactive` for both canvases, so every host that renders a form gets it; `IsKeyDown` in `interpreter/graphics.rs`; seeds `FixedStep`, `PointerX`, `PointerY`. 30 fps on 144 Hz, 10 s: 299 frames (29.90/s); fixed step 60 fps: every delta 16.6667 ms; all 11 input event kinds reach a bound canvas, none an unbound one. Not done: battery cap, gamepad (T27), hands-on check.

- [x] **T20 — Designer placeholder** (code and tests done; the operator judges the look) (R17; AC13 Designer)
  - Files: `crates/cobolt-forms/src/paint.rs` (designer path)
  - Do: a labelled placeholder with the control's name, size and (Canvas3D) backend; the run form path (T18)
    draws the live picture. If the placeholder changes how a control border, radius or clip is drawn, run the GUI
    Border Validation Agent (`.agents/agents/gui-border-validation-agent.md`) before closing the task.
  - Verify: `cargo test -p cobolt-forms --features render --no-fail-fast` green with a test that loads a fixture
    `.cfrm` with both canvases and asserts the painted label text and position; the operator judges the look
    (MCP `render_form`).
  - Evidence (2026-11-04, 2.0.200): `paint::canvas_placeholder_lines` + dashed outline in `draw_canvas_surface`; `paint::set_designer_backend` fed once from the IDE's wgpu device (`app.rs`); test `a_canvas_shows_a_labelled_placeholder_in_the_designer` (saves and reloads a `.cfrm`, asserts the lines, their centring and order, and that a picture removes them). `cobolt-forms --features render` 98 result lines, 1832 passed, 0 failed. The outline is dashed but still a square rect: no corner path touched, so the Border Validation Agent was not run.

- [x] **T21 — Toolbox, icons, Properties-panel sections, `tests/controls/`** (code and tests done; the operator looks at the Toolbox and Properties) (R1)
  - Files: `crates/cobolt-ide/src/` (toolbox and Properties panel), the icon catalogue, `tests/controls/` (one test
    file per new type plus its `[[test]]` entry in `tests/controls/Cargo.toml`)
  - Verify: `cargo test --manifest-path tests/controls/Cargo.toml --no-fail-fast` green (seven new tests reported);
    `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` green; every project kind's toolbox lists the seven.
  - Evidence (2026-11-04, 2.0.201): `toolbox.rs` (two canvases under Graphics, five objects Non-Visual, seven vector icons), `properties.rs` (Canvas section: PixelRatio, Smoothing, TargetFps, FixedStep, AutoPresent; Canvas3D adds Msaa, ClearColor), `i18n.rs` `sec_canvas` in six languages, seven `tests/controls/*.rs` with `[[test]]` entries (all pass); toolbox test: 7 of 7 on Desktop, Web, Mobile, seven distinct icons. In `tests/controls`, `agentobject` and `restclient` fail with and without this change (pre-existing).

- [x] **T22 — Six-language strings** (done; messages other than no-adapter await the surfaces that raise them) (R1, steering i18n)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: `Tr` fields in en, es, pt (pt-BR), fr, ja, zh for the toolbox labels, Properties sections and the readable
    messages: no adapter, lost device, limit refused, unresolved font (T27).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` green (no empty translation); Properties panel frames
    checked in all six languages for width growth.
  - Evidence (2026-11-04, 2.0.202): 12 `Tr` fields in six languages, test `the_graphics_texts_are_in_all_six_languages_with_their_slots` (72 texts, slots intact); Toolbox hints wired; `no adapter` shown under the Canvas3D placeholder (`paint::set_designer_no_adapter_text`). 117 Properties, help and Toolbox tests pass. Not done: a pixel check of the Properties frames per language (the new rows are short, option lists and labels are unchanged English identifiers); the other three messages are not shown by any surface yet.

- [x] **T23 — System KB** (R1, spec §6, steering System KB rule)
  - Files: the `cobolt-compiler` documentation tables (Rust constants: two controls, five objects, every property,
    event and method, generated from the same `Method` rows as the rustdoc tables, plus the HTML-canvas difference
    table); `assets/knowledge/chunked.data` regenerated with `cargo run -p cobolt-ide --example build_chunked_kb`
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    (`crates/cobolt-ide/src/grace_host.rs:4780`) green after regeneration; `cargo test -p cobolt-compiler
    --no-fail-fast` green; `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test freshness` green.
  - **Done (2.0.203):** `form_designer_controls.md` has a purpose line for each of the seven types and 313 methods from
    `Registry::methods` (plus `IsKeyDown` on both canvases) and the Canvas2D-vs-HTML-canvas table; test
    `the_system_kb_documents_the_seven_graphics_types` (7 types, 313 methods, 0 missing). `chunked.data` regenerated
    (4196 records); `prebuilt_chunked_kb_matches_the_published_documentation` ok; `cobolt-compiler` 226 passed, 0 failed;
    `freshness` 6 passed after rebuilding `assets/web-player` (it had been red since T18–T22 changed `cobolt-forms` and
    `cobolt-graphics`).

## Part 3 — generated module, player, fonts, CI, sample (not started)

- [x] **T24 — ABI amendment A10 and the generator's display list** (R6, R16; AC2, AC13 parity)
  - **Done (2.0.204, 2.0.205).** AC13: 15 calls, interpreter 156 bytes and module 156 bytes, identical. The amendment is **A23** with opcode **`0x08`** (A10 and `0x06` were taken),
    and `DRAW` names `offset`/`len` in a new arena region (`limits.gfx_size`) rather than carrying the bytes: 10,000
    `FillRect` are ~170 KB, the batch is 68 KiB (decided with the operator). AC2: 10,002 commands, 1 flush, 1 `DRAW`,
    0 host calls, 170,022 bytes, equal to the shared encoder. No canvas: no `DRAW`. NIST NC 95/95, 4612 PASS, 0 FAIL.
    `cobolt-wasm`/`-rt`/`-abi` suites green. **Known difference:** a call the module cannot encode exactly (a colour
    name, a list operand, an odd number) goes to the host as a `CALL`; the module does not copy the interpreter's
    `LastError` text.
  - Files: `specs/WASM-ABI-v1.md` (A10: batch command `0x06 DRAW` = `path`, `varint len`, list bytes; read-backs as
    `CALLR` after the pending `DRAW`; §12.2 rows for the canvas events' argument texts), `crates/cobolt-wasm-abi`,
    `crates/cobolt-wasm`, `crates/cobolt-wasm-rt` (links `cobolt-graphics-wire`'s encoder; the list lives in the arena)
  - Verify: `cargo test -p cobolt-wasm -p cobolt-wasm-rt -p cobolt-wasm-abi --no-fail-fast` green with: (AC2) 10,000
    shapes then `Present` = 1 flush carrying 1 `DRAW` (**calls and bytes reported**); (AC13) the same program's list
    bytes from the interpreter (T17) and from the module on `wasmi` are equal (**byte counts reported**); a module
    without canvases never emits `DRAW`. NIST NC still 95/95 on the generator (the gate in memory
    `wasm-generator-nist-nc-finished`).

- [x] **T25 — Player: `DRAW`, read-backs answered, the canvas part with text and images** (R7, R8, R16; AC1, AC2, AC3 on the web)
  - Files: `crates/cobolt-web-player/src/methods.rs` (`canvas_method` at 554 stops refusing read-backs, message at 562),
    `src/dom/canvas.rs`, `src/view/controls.rs`, `parts/canvas/{Cargo.toml,src/lib.rs}` (`cobolt-graphics` with
    `text` + `imagebuf`)
  - Do: handle `DRAW`; keep accepting the old `canvas-call` form until the generator emits `DRAW`; answer read-backs,
    suspending the task while the part loads (A5); events with the A10 argument texts.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast` green with: `GetPixel`
    after drawing answers the drawn colour; a read-back during part load suspends and resumes; `DrawText` and
    `DrawImage` draw in the part. Part size before/after reported.
  - **Status 2026-11-05 (2.0.206): partly done, not ticked.** Done: `DRAW` drawn by the Canvas part; read-backs answered by it once loaded; text + imagebuf in the part (655,286 → 3,173,219 bytes after wasm-opt); pointer and key events with the A23 texts. Open: a read-back before the part has loaded is refused with a sentence, not suspended (needs a generator change); `onFrame`, `onWheel`, touch events; a real `DrawText`/`DrawImage` check in a browser (needs fonts, T27).
  - **Update 2026-11-05 (2.0.207): the read-back before the part loads is closed without suspending.** The player loads the Canvas part before the first call into a module whose manifest has a graphics region (`limits.gfx_size > 0`, A23), and builds a canvas's `Surface2D` synchronously from it, so a read-back answers at once even in the first handler; no ABI change, no generator change. The planned "a read-back during part load suspends and resumes" no longer arises for such a module (the one case left is a page whose loader cannot load parts: the read-back is refused with a sentence). Still open for T25: `onFrame`, `onWheel` and touch events; `DrawText`/`DrawImage` in a real browser (fonts, T27).
  - **Closed 2026-11-05 (2.0.215).** Events: pointer (`"x y"` in the canvas's own pixels), wheel (`"dx dy"`), touch and key events reach the handlers with the desktop's argument texts, and `IsKeyDown`/`PointerX`/`PointerY`/`GamepadAxis`/`GamepadButton` answer from the page (`canvas_input_chrome`, real Chrome). `onFrame` came with T30 (2.0.211). `DrawText` is lettered with the shipped font and `DrawImage` draws an ImageBuffer onto a canvas in the part, read back in Chrome (`graphics_parts_chrome`: 76 inked pixels; the image's pixel `16711935` at the place it was put, `0` outside). Two bugs found by driving a real handler (`graphics_players_chrome`): the page looked the part up through a host that was mid-call, so every `GetPixel` from a handler was refused; and the drawing queued before the read had not reached the part, so it answered a transparent pixel. Both fixed; the host now names the part and hands over the queued drawing in order (`a_read_back_is_handed_the_drawing_queued_before_it_in_order`).

- [x] **T26 — Player parts for plot, scene3d, physics (and ImageBuffer by measurement)** (R16, R18; AC5, AC14 part)
  - Files: `crates/cobolt-web-player/parts/*`, `examples/build_player.rs`, the Build report (`BuildReport`/`PartSize`,
    WASM-016 plan §1.7)
  - Do: measure whether ImageBuffer rides with the canvas part or is its own (plan §1.4); one `Registry` per form
    instance so handles cross objects; scene3d with `webgpu` + `webgl` (`crates/cobolt-graphics/Cargo.toml:95-108`);
    `Backend` reports WebGPU then WebGL2; neither → readable message and event.
  - Verify: player tests green; the Build report lists each part's size (**numbers recorded in plan §6**); a form
    without a 3D control never fetches the scene3d part (request log).
  - **Done 2026-11-05 (2.0.208).** Three parts from one shared source (`parts/graphics.rs`): `canvas` (+ImageBuffer, by measurement: it rides with the canvas so a picture and the canvas that shows it share a registry), `graphics` (+Plot, Physics2D/3D), `graphics3d` (+Scene3D, Canvas3D); one `Graphics` registry per form instance; every method of the other six kinds answers at once from the part, with a refusal written to the control's `LastError` as on the desktop; `Render` of a Canvas3D opens the page's device (WebGPU, else WebGL2) and the picture is copied out asynchronously (`Renderer::begin_read`/`PendingRead`); no adapter is a readable message. The parts are loaded before the form that uses them opens, and the Build ships only the parts some form needs. Sizes in plan §6.3; a real-Chrome test (`graphics_parts_chrome`) loads the three built parts, and the 3D one rendered through WebGPU. Added `GetHandle` to every kind (library, closed vocabulary): without it COBOL had no way to hand a control's object to another (`SetScene`, `RenderToCanvas`).

- [x] **T27 — Fonts: every font the project references** (code and tests done; the compiled binary registers `Assets/` fonts only, see the note) (R8; Q7 decided; AC1, AC3)
  - Files: `crates/cobolt-project-tools` (web build), the desktop font registration (`crates/cobolt-forms/src/fonts.rs`,
    `system_fonts` at 30), the web bundle manifest
  - Do: collect `.cfrm` `Font` families, `SetFont` literals, `Assets/` font files; resolve with `fontdb`; unresolved
    family = error on Web/Mobile, warning on Desktop; warn on a non-literal `SetFont`; read `OS/2.fsType` and the
    licence string, refuse a font that forbids embedding, list them in the Build report; ship under `fonts/` and hand
    to `FontSet::add_font_data` when the text part loads; the desktop registers the same files.
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast` green with: a project referencing one family in a
    `.cfrm`, one in a `SetFont` literal and one `Assets/` file ships exactly those three; an unresolvable family fails a
    Web build; an `fsType`-restricted font is refused. The project's COBOL is read, never written.
  - **Done 2026-11-05 (2.0.209).** `cobolt-project-tools/src/fonts.rs` (collect: `FontName` of every control, `SetFont` literals, `Assets/`; resolve: `fontdb`, the desktop painter's query; licence: `OS/2.fsType` and name ids 13/14 through `ttf-parser`); `generator.rs` `project_fonts` (error on Web/Mobile, `SetFont` of a data item = warning at its line, fonts listed in `BuildReport.fonts`); the bundle lists the stand-in face under asset key `0-stand-in/ubuntu-light.ttf`; the player fetches the font files once when the first graphics part loads and calls the part's new `Graphics.add_font` per registry; `cobolt-runtime` registers the stand-in face then the project's fonts in path order on every desktop registry (before this a desktop canvas drew no text without `LoadFont`), and `rcrun run-form` publishes the families the project names that the machine has. Tests: `fonts` unit tests (5), `tests/fonts_build.rs`, the parts in Chrome (`graphics_parts_chrome`), the runtime registration test. **Not done:** a compiled desktop binary finds the project's own `Assets/` fonts but not the machine's fonts a Web build would ship (the compiler cannot depend on `cobolt-project-tools`; copying them into a desktop build's `Assets/` needs the Binary Compiler to call it); `LoadFontAsset` still takes a name from the asset host.

- [x] **T28 — Gamepad** (R13; AC12 gamepad)
  - Files: `crates/cobolt-form-host` (`gilrs`), `crates/cobolt-web-player` (Gamepad API); `GamepadAxis`, `GamepadButton`
  - Do: add `gilrs` only if the dependency guard passes for it on Linux (plan §5: udev not verified); if it fails,
    Linux gamepad input is reported as a gap in plan.md, the guard is **not** relaxed.
  - Verify: `cargo test -p cobolt-graphics --test dependency_guard` green; a host test feeds a synthetic gamepad event
    to a bound handler on each side.
  - **Done 2026-11-05 (2.0.210).** `gilrs` is refused by the dependency guard (test `gilrs_is_refused_by_the_dependency_guard`; guard not relaxed), so the desktop has no OS gamepad source: Linux/macOS/Windows gamepad input on the desktop is a **gap**, recorded in plan §5. What exists: `cobolt_forms::gamepad` (`Pad`, `diff`, `snapshot`, `feed`, `current`), events `onGamepadButton` (`pad button 1|0`) and `onGamepadAxis` (`pad axis value`) on Canvas and Canvas3D, the pollable `GamepadAxis(axis [pad])` and `GamepadButton(button [pad])` (added to the closed inline vocabulary), a host `feed` on the desktop side (a test feeds a synthetic pad to a bound handler: `a_pad_the_host_feeds_reaches_a_bound_canvas_as_gamepad_events`) and, in the browser, the player polls `navigator.getGamepads()` every 16 ms while a canvas has a bound gamepad handler (`a_gamepad_event_reaches_a_canvas_with_a_bound_handler_and_no_other`); runtime test `the_gamepad_methods_read_the_pads_the_host_keeps`. **Not done:** a real pad in a real browser (no device here), and the browser polling is untested in Chrome.

- [x] **T29 — CI job on a software Vulkan, no fallback** (Q5 decided; AC4, AC5 software adapter)
  - Files: `.github/workflows/` (a job in `build-all-platforms.yml` or a new file)
  - Do: Linux, Mesa lavapipe, `WGPU_BACKEND=vulkan GRAPHICS_REQUIRE_GPU=1 cargo test -p cobolt-graphics --test scene3d_gpu`;
    a missing adapter is red, never skipped. Measure the golden tolerance on lavapipe and on Metal.
  - Verify: the job runs green on a push; both tolerances recorded in plan §6; `Backend` reports `Software` on lavapipe.
  - **Written 2026-11-05 (2.0.210); left open on purpose.** `.github/workflows/graphics-gpu.yml` installs Mesa lavapipe, sets `GRAPHICS_REQUIRE_GPU=1`, `WGPU_BACKEND=vulkan` and `GRAPHICS_EXPECT_BACKEND=Software`, and fails on a skipped test or a backend other than `Software`; `the_backend_is_reported` asserts the expected backend; `assert_golden` prints a `GOLDEN-TOLERANCE` line under `GRAPHICS_REPORT_TOLERANCE=1`. Metal measured locally: worst channel 0, 0 of 76,800 pixels over the threshold (plan §6.3). **Not done, needs GitHub:** the job has not run, so there is no lavapipe tolerance and no green run; tick this task after the first run and record its `GOLDEN-TOLERANCE` line in plan §6.3.
  - **Closed 2026-11-05 (2.0.222), run 42111370290 and the run of c0ce3e32.** The job runs green on GitHub on Mesa lavapipe (llvmpipe, LLVM 20.1.2, Mesa 25.2.8): `Canvas3D backend: Software`, 18 passed, 0 failed, nothing skipped. Two things the first runs taught: (1) the step piped `cargo test` into `tee` with no `pipefail`, so 2 failing tests had left the job green (2.0.219–2.0.221 were not honest); fixed with `shell: bash`. (2) The golden tolerance measured on lavapipe: worst channel 32 (the guess was 24), 1 of 76,800 pixels over 6; Metal: 0 and 0. The limit is now 40. The numbers are public as run annotations (the log and the job summary need a sign-in).

- [x] **T30 — Cross-platform parity and determinism** (R20; AC1, AC3, AC5, AC6, AC7, AC12 browser)
  - Files: tests in `cobolt-form-host`, the player, `cobolt-graphics`
  - Verify, each reporting its numbers: (AC1) the same list on the desktop host and the canvas part (native and
    `wasm32` in a headless engine) gives identical RGBA — **differing pixels = 0**, including text in six languages;
    (AC3) `MeasureText` = `DrawText` extent and identical wrap points on both; (AC6) stacked boxes settle to the same
    positions on desktop, plain and threaded player, and a bound body moves its sprite; (AC7) resize + blur + encode
    gives the same bytes on 1 and N threads and plain vs threaded player; the 24 MP blur time on both players is
    measured and the page stays responsive; (AC5) WebGPU, WebGL2-only and no-GPU browsers report `Backend` or show the
    message; (AC12) input and `onFrame` in the browser, paused when the tab is hidden.
  - **Partly done 2026-11-05 (2.0.211); left open on purpose.** Met: **AC1** (`cobolt-graphics/tests/parity.rs` pins one scene, `golden/parity-scene.txt`, byte for byte; the interpreter and the canvas part as `wasm32` in a real Chrome give the same picture: 0 of 19,200 pixels differ; text in English, Portuguese, Spanish and French is lettered, Japanese and Chinese come out as the font's missing-glyph box on every host because Ubuntu Light has no CJK); **AC3 in part** (`MeasureText` answers are identical on desktop and in Chrome, and `DrawText` ink lies inside the measured box within 1 px, the box within 2.44 px of tight; identical wrap points are not tested); **AC6 in part** (three stacked boxes settle to the same positions and `GetStateFingerprint` on desktop and in Chrome; the plain-versus-threaded player comparison is not made: the graphics parts are one module in both players); **AC7 in part** (resize + blur + saturation + histograms + PNG size agree between desktop and Chrome, and 1 versus 8 threads were already pinned in `api.rs`/`imagebuf.rs`; the 24 MP Gaussian blur takes 1.6 s in Chrome **on the page's own thread, so the page does not stay responsive: running graphics work in a worker is open**); **AC5** (the no-adapter message path, in `graphics_parts_chrome`); **AC12 in the browser in part**: `onFrame` now exists in the player (`Host::frame_tick`, the desktop's own `FrameClock`, moved to `cobolt_forms::frame_clock`); real Chrome: 29.9 frames/s at `TargetFps` 30, 0 frames while `document.hidden`, 29 on resuming with no burst (`canvas_frames_chrome.rs`). The elapsed time is the event's argument; the browser does not write the `ElapsedMs` property. **Not done:** pointer, wheel, key and touch events and `IsKeyDown`/`PointerX` polling in the browser; the wrap points; the plain/threaded player comparison; a worker for long graphics calls.
  - **Update 2026-11-05 (2.0.215); still open, now only for the worker.** Closed since: pointer, wheel, key and touch events and `IsKeyDown`/`PointerX` polling in the browser (`canvas_input_chrome`); the wrap points (three `DrawTextBox` cases in the parity scene; 0 of 19,200 pixels differ between desktop and Chrome, the interpreter equal); the plain-versus-threaded comparison (`graphics_players_chrome`: the same page served without isolation headers runs the plain player on the page's thread, with them the threaded player and the program in a Worker; both read back the red Canvas2D pixel `4278190335` and the blurred ImageBuffer pixel `4287103231`, in 31 ms and 46 ms). **Still open: AC7's "the page stays responsive" during the 24 MP blur** (1.6 s on the page's own thread). It needs the graphics registry to live in a Worker, the player's synchronous read-backs to become replies the program waits for (a SAB wait in worker mode; no equivalent in plain mode without an ABI change), so it is an architectural change of its own, not a test: it should be specified (a WASM-018 amendment or its own spec) before it is built. Not ticked for that reason.
  - **Closed 2026-11-05 (2.0.216): the page stays responsive.** On a page that runs the program in a Worker (the threaded player, isolated), a long method of a large ImageBuffer (`BlurGaussian`, `BlurBox`, `Sharpen`, `EdgeDetect`, `Convolve`, `Dilate`, `Erode`, `Resize`, `Rotate`, `Histogram`, `Encode`; images of 1,000,000 pixels or more; `cobolt_graphics::imagebuf::offload_mode`) runs in a graphics worker (`gfx-worker.js`, one file of the bundle) on a copy of the pixels, using the part's own compiled module. The program is blocked on its shared buffer anyway, so the main thread answers later: the host parks the rest of the program's batch and holds the batches that arrive meanwhile (`Host::finish_deferred`, `a_call_that_goes_to_a_worker_parks_its_batch_…`), so the order of every effect and answer is the order the program sent them; the loader writes the parked query's answer when the worker returns. With the program on the page's own thread nothing can wait, so the call runs in place (a page that is not isolated). Measured in Chrome (`graphics_responsive_chrome`), a 6000 x 4000 Gaussian blur (sigma 3): in place, the page is frozen for **7,032 ms** (2 ticks of a 10 ms timer); in the worker, the longest gap is **216 ms** (684 ticks) — that is the copy of the 96 MB of pixels out of the part, the one piece still done on the page's thread — and both read back the same pixel. The library pins that every offloadable method gives the same bytes on a copy (`an_offloadable_method_gives_the_same_bytes_on_a_copy_of_the_pixels`). **Not met, stated plainly:** AC7's "threaded player faster": the graphics parts are single-threaded WebAssembly in both players, so the blur takes the same 7 s in the worker as in place; a multi-threaded part (atomics + a thread pool inside the worker) would be a build change of the parts and is not part of this task. T30 is ticked for what its Verify names: same bytes, both players measured, the page responsive.

- [x] **T31 — Lifecycle and limits on every host** (R2, R15; AC9, AC10)
  - Files: tests in `cobolt-form-host` and the player
  - Verify: 100 open/close cycles of a form with a canvas, an image and a scene leave memory at baseline on the
    desktop and the player (`pr_dbg_live`/region use; **numbers reported**); an oversized canvas, image, model and
    display list are each refused with a readable error through COBOL; corrupt image and model rejected without a
    crash; the crate's fuzz targets (`display_list_robustness`, `imagebuf`, `scene3d`) run within budget.
  - **Done 2026-11-05 (2.0.212), with two narrow gaps stated.** *Desktop host* (`cobolt-runtime`, the interpreter, one interpreter = one form): `a_hundred_forms_through_the_interpreter_leave_no_object_and_no_byte` (a canvas drawn and presented, an image made and blurred, a chart of 50 points, a physics world; 100 cycles in 909 ms; at most 4 objects and 424 KB held at once; **0 objects and 0 registry bytes after every close**); `oversized_and_corrupt_inputs_are_refused_through_cobol_with_a_sentence` (an image of 100,000 x 100,000, bytes that are no image, bytes that are no model, a model the host has no assets for: each a sentence in `LastError` naming the setting, and the canvas draws afterwards). *The player* (`cobolt-web-bundle/tests/graphics_lifecycle_chrome.rs`, the shipped `canvas` and `graphics` parts in a real Chrome): 100 lifetimes of each in 262 ms, 0 objects left after every cycle, and the module's linear memory is **3,008 KB (canvas) and 2,176 KB (graphics) at cycle 10 and at cycle 100, byte for byte**; a canvas of 100,000 x 100,000 (limit 8,192), an image of 10,000,000,000 pixels (limit 50,000,000), a display list claiming 4,294,967,295 commands (limit 2,000,000) and bytes that are no image are each refused with a sentence, and the part still draws. *The crate's fuzz suites* run within budget: `display_list_robustness` 6.3 s, `imagebuf` 10.5 s, `scene3d` 0.3 s, `api` (20,000 random calls, no panic) 0.8 s, `lifecycle` (AC9, 100 forms with a scene: 32 bytes above baseline at form 100) 8.2 s. **Gaps:** the scene (Scene3D/Canvas3D) is in the library's 100-form cycle but not in the interpreter's or the browser's (the 3D part needs a graphics device); an oversized *model* is refused at the library (`limits_refuse_a_model_that_is_too_big_before_building_it`) but not driven through COBOL on the hosts, only a corrupt one is. Found and fixed on the way: T26's `GetHandle` had left `tests/api.rs` red (the crate's method tables in `lib.rs` were out of date and `GetHandle` with arguments was not refused with the argument-count error); the tables are regenerated and the registry now refuses it.

- [x] **T32 — Sample project `examples/PowerGraphics`** (R19; AC14, and AC8 from COBOL)
  - Files: `examples/PowerGraphics/` (game: sprites, physics, input; chart gallery: every Plot kind into a Canvas2D,
    an ImageBuffer and SVG, a 100,000-value table; image lab; 3D viewer)
  - Do: COBOL in the RustCOBOL extensions (skill `rustcobol-code-style`); no scratch items for properties.
  - Verify: the project checks clean (`rcrun check`); it builds and runs on Desktop (compiled binary) and Web in the
    test suite; the Web build fetches the 3D part only when the 3D form is shown; the staged diff passes the
    credential scan (`APIKey|Password|Token|AKIA|ASIA` and the operator's account id) **as a gate** before commit.
  - **Done 2026-11-05 (2.0.213), with the gaps stated.** Four forms (`game-form`, `charts-form`, `imagelab-form`, `viewer3d-form`) and the font under `Assets/fonts/`. `check`: 0 errors, 0 warnings. Run off screen (`run_form`) the handlers work: the five balls fall and `GetPosition` parses back through `UNSTRING` + `NUMVAL`; all twelve Plot kinds (each into a canvas, a rendered picture and SVG) and the 100,000-row table (`SetSeriesFromTable`) leave `LastError` empty; the five filters run clean. `rcrun build` produces the binary. `cobolt-project-tools/tests/power_graphics_web.rs` derives a Web project from the four forms and builds it: per-form part need = game `graphics`, charts `graphics`, image lab `canvas`, viewer `graphics3d`; a project without the viewer ships no `graphics3d`. **Found and fixed:** the interpreter had no way to pass a list of numbers, so heatmap and surface series, `Convolve`, `AddMesh` and `SetInstances` could not be used from COBOL; a list argument may now be text (`"1 2 3, 4 5 6"`), test `a_list_argument_may_be_written_as_text_because_cobol_has_no_list_type`. A handle needs `PIC 9(10)` (32 bits, grows on slot reuse). **Gaps:** the compiled binary was built and not run (no window is driven; the form host ran the same handlers off screen); a Canvas3D has no adapter off screen (`Backend` None), so the 3D picture itself is judged by the operator; the browser's lazy fetch of the 3D part is T26's tested behaviour, here only the build's per-form need and the shipped set are asserted; the credential scan ran on the staged diff before the commit.

- [x] **T33 — Finalize**
  - **Closed 2026-11-05 (2.0.223).** Versions 2.0.205–2.0.223 each bumped `z` with their own CHANGELOG entry (the one-bump rule was written for a single sitting). The sweep and its numbers are in plan §9: all green except 3 pre-existing, unrelated `cobolt-ide` tests. Found and fixed on the way: the controls harness expected credentials to reload; `write_fixtures` needed its feature declared. Left to the operator: the running IDE's version string.
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
