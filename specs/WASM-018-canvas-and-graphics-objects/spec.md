<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Canvas 2D, Canvas 3D and graphics objects, on every platform

- **Status:** draft (needs `/clarify` for the questions in §7)
- **Folder:** specs/WASM-018-canvas-and-graphics-objects/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-20
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the operator, 2026-10-20: *add, for every platform of PowerRustCOBOL AI 2.0 (Desktop, Web, Mobile), new controls **Canvas 2D** and **Canvas 3D**, and objects with methods to generate graphics — games, charts, image manipulation and the like. Choose popular crates for them, always on the premise that they are **100 % Rust**.*
- **Depends on:** WASM-004 (the control model on the web), WASM-010 (the module and the host API), WASM-016 (the player), WASM-017 (memory, boundary, threads). **Adjusts:** WASM-004 (two new controls, a new class of object).

## 1. Overview

The product draws forms with a painter; a developer who wants to draw something of their own — a game, a custom chart, an image editor, a 3D model viewer — has the seven chart controls, a PictureBox and an Animator, and nothing to draw with. This spec adds **two controls** — **Canvas2D** and **Canvas3D** — and **five client-side objects** — **ImageBuffer**, **Plot**, **Scene3D**, **Physics2D** and **Physics3D** — to
the toolbox of **every project kind**, with one COBOL API on every platform.

All of it is implemented **once**, in a new pure-Rust crate (`cobolt-graphics`) that knows nothing of egui or of the browser; the **desktop** host (the egui form host, which the IDE's Preview, Run Form and compiled applications use) and the **player** (Web and Mobile) are thin adapters that give it a surface and an event loop. Because both run the same Rust, a drawing is the same on every platform, and Rayon (WASM-017 R19) speeds up the
heavy parts wherever threads exist.

### What exists today (checked 2026-10-20, `main` at 1.91.219)

- **Drawing controls:** seven chart controls (`BarChart`, `LineChart`, `PieChart`, `AreaChart`, `ScatterChart`, `DonutChart`, `RadarChart`), `Knob`, `Gauge`, `PictureBox`, `Animator`, `Maps` and `Viewer`, all painted by the egui painter of `cobolt-forms` (`paint.rs`); `ControlType::ALL` has 62 types. There is **no canvas, no scene, no image-processing object** and no way for a program to draw a line.
- **Non-visual objects** exist as controls with methods the interpreter implements (`AgentObject`, `RestClient`, `SqlDatabase`, `IndexedFile`, `KnowledgeBase`, `WebSearch`, `Timer` and others, `cobolt-runtime/src/interpreter.rs`); a method must be in the **closed vocabulary** `cobolt-ast/src/methods.rs:164` or it silently parses as a subscript; and, since 2026-10-19, `CALL … RETURNING` hands a value back, which is how a handle returns to COBOL.
- **Rust graphics crates already in the dependency tree:** `tiny-skia` 0.11.4 (through `resvg` 0.46), `kurbo`, `ab_glyph`, `skrifa`, `image` 0.25.10 (whose `ravif` already pulls `rayon` 1.12), `wgpu` 30.0.0 and `naga` (through `egui-wgpu`, a dependency of `eframe` 0.36), and `glow`/`egui_glow`. **Not present:** `imageproc`, `glam`, `lyon`, `plotters`, `rapier`, `gltf`, `rustybuzz`, `bevy`, `three-d`, `cosmic-text`. Both the glow and the wgpu renderers of `eframe` are compiled in; which one the IDE and the form host select at run time was not found in a first search and is a `/plan` item (Q7).
- **Fonts and text:** `cobolt-forms` has `fontdb` and `skrifa` and a base font set (`fonts.rs`); `resvg`'s own text path shapes with `rustybuzz`.
- **The browser side is empty:** no player yet (WASM-016), no `web-sys` of our own; `web-sys` is only in `Cargo.lock` transitively.

## 2. Goals / Non-goals

- **Goals:**
  - **Canvas2D** and **Canvas3D** controls, and **ImageBuffer**, **Plot**, **Scene3D**, **Physics2D**, **Physics3D** objects, on Desktop, Web and Mobile, with one COBOL API and the same results.
  - **100 % Rust** implementations from popular, maintained crates (R4, R5).
  - A **display-list** model that keeps the COBOL ⇄ host boundary coarse-grained and makes every host execute the same commands (R6).
  - Games, charts and image work that use all cores when the platform offers them and give identical results when it does not (R20).
  - A sample project that exercises all of it on every platform, and tests that compare platforms.
- **Non-goals:**
  - **Audio.** It is not a graphics object. (The desktop already plays sound through the `audio` feature for the Animator; nothing here changes that.)
  - A general game **engine** (an entity-component framework, a scheduler, an asset pipeline): the objects are the building blocks, the program is the game.
  - A GPU 2D renderer: Canvas2D is CPU-rasterised (R7). A GPU path, when a stable one exists, would be an internal change with the same API.
  - Replacing the seven chart controls, the PictureBox or the Animator; they stay as they are.
  - Ray tracing, compute shaders for the developer, and VR/AR.

## 3. User stories

- As a developer, I want to draw lines, shapes, text and images from COBOL and see them on a form, so that I can build a custom chart or a small game.
- As a developer, I want to load a photo, resize it, blur it and save it, so that I can write an image tool without another language.
- As a developer, I want to show a 3D model I can rotate, light and click, so that I can build a product viewer.
- As a developer, I want the same program to draw the same picture on the desktop, in the browser and on a phone.
- As a user on a multi-core machine, I want a filter on a large image not to freeze the page.

## 4. Requirements (EARS)

**The catalogue**
- **R1 (ubiquitous):** The toolbox of every project kind shall offer two visual controls — **Canvas2D** and **Canvas3D** — each with its properties, events and methods (below), a default size, an icon, Properties-panel sections, `.cfrm` round trip, documentation in the System KB tables, a `tests/controls/` test, six-language labels, and every method name in the closed vocabulary (`methods.rs`).
  Canvas2D: properties `Width`, `Height` (logical pixels), `PixelRatio`, `Smoothing`, `BackgroundColor`, `TargetFps` (0 = redraw on demand), `AutoPresent`, `AccessibleLabel`; events `onFrame` (with the elapsed milliseconds), `onPointerDown`/`Move`/`Up`/`Enter`/`Leave`, `onWheel`, `onKeyDown`/`Up`, `onTouchStart`/`Move`/`End`, `onResize`; methods in R7.
  Canvas3D: the same properties and events plus `ClearColor`, `Msaa` and a read-only `Backend` (`WebGPU`, `WebGL2`, `Vulkan`, `Metal`, `Dx12`, `Software`); methods in R9.
- **R2 (ubiquitous):** The catalogue shall also gain five **non-visual objects**, dropped on a form like `Timer` or `RestClient`: **ImageBuffer** (R11), **Plot** (R12), **Scene3D** (R9), **Physics2D** and **Physics3D** (R10). Things they create — a series, a node, a light, a body — are **handles**: whole numbers that `RETURNING` brings back into a data item and that the object's methods take; a handle is released
  by `Release`, by the removal of the thing it names, and when its object or form is destroyed (WASM-017 R14).
- **R3 (ubiquitous):** All of it shall be implemented in **one pure-Rust crate, `cobolt-graphics`**, with no dependency on egui, on `web-sys` or on the file system; it shall build for the host targets and for `wasm32` in both player variants (plain and threaded); the desktop host and the player shall only provide a surface, input events, the frame clock and asset bytes.

**100 % Rust**
- **R4 (constraint):** A crate enters only if: it is written in Rust; **no build script of its default feature set compiles C or C++** or links a system library (`cc`, `cmake`, `bindgen`, `pkg-config`), the browser and the operating system's graphics APIs excepted, which `wgpu` reaches through Rust bindings; its licence is permissive and compatible with Apache-2.0; it has had a release in the last twelve months; it supports `wasm32`; its minimum Rust version
  is at most 1.92 (the threaded player's nightly excepted, WASM-017 R16); and it passes the dependency guards of WASM-013 R9 (no `ring`, no `aws-lc-rs`). A **dependency-guard test** over `cobolt-graphics`'s tree shall fail on any package of a deny list (`font-kit`, `freetype-sys`, `harfbuzz-sys`, `skia-safe`, `cairo-sys-rs`, `libwebp-sys`, `mozjpeg-sys` and the like).
- **R5 (ubiquitous):** The crates shall be, unless the plan shows one fails R4 (the plan records the exact versions):

  | Purpose | Crate | Note |
  |---|---|---|
  | 2D rasteriser | **`tiny-skia`** | Already in the tree through `resvg`; CPU, paths, strokes, dashes, gradients, patterns, clip masks, blend modes; deterministic |
  | Curves and geometry | **`kurbo`** | Already in the tree |
  | Text | **`skrifa`** + **`rustybuzz`** + **`fontdb`** | Outlines, shaping (Latin, CJK, Arabic, Indic) and font lookup, all Rust; with `unicode-bidi` and `unicode-linebreak` for direction and wrapping |
  | Images | **`image`** + **`imageproc`** + **`fast_image_resize`** | `image` is already in the tree; pure-Rust codecs; `imageproc` for filters, morphology and drawing; `fast_image_resize` for resampling |
  | Charts | **`plotters`** | With its pure-Rust `ab_glyph` text feature and **our own `DrawingBackend` onto `tiny-skia`**; its default `font-kit` text feature (which needs C libraries) is **off** |
  | 3D | **`wgpu`** + **`naga`** + **`glam`** + **`bytemuck`** + **`gltf`** | `wgpu` is already in the tree (WebGPU and WebGL2 in the browser, Vulkan, Metal and Dx12 elsewhere); `glam` for mathematics; `gltf` to load glTF 2.0; the scene layer is ours (below) |
  | Collision and physics | **`rapier2d`**, **`rapier3d`** (with `parry`) | `parallel` through Rayon and `enhanced-determinism` on |
  | Parallelism | **`rayon`** | Already in the tree; in the browser through `wasm-bindgen-rayon` (WASM-017 R19) |
  | Gamepad | **`gilrs`** on desktop; the browser's Gamepad API through the player | |

  **Rejected, with reasons:** `bevy` as the engine (it brings an application framework, a scheduler and tens of megabytes of WebAssembly, and wants to own the loop); `three-d` (OpenGL through `glow`, a second graphics stack beside `wgpu`); `macroquad`/`miniquad` (own the window and the loop, so they cannot sit inside a form); `charming` (renders with JavaScript, not Rust); `skia-safe` and `cairo-rs` (C or C++); `vello` (a GPU 2D renderer, not yet stable enough — an internal replacement for the rasteriser behind the same API if it matures).

**Display list and parity**
- **R6 (ubiquitous):** A program's drawing methods shall **record commands into a display list** (a versioned binary encoding defined once in `cobolt-graphics`) that the control **executes in one call** — on `Present` or `Flush`, at the end of an `onFrame` handler with `AutoPresent`, and before any method that reads the picture back (`ToImage`, `GetPixel`, `MeasureText`, `IsPointInPath`). In the generated module the list is built in the transport arena
  (memory 1 of the multi-memory variant, WASM-017 R8) and one host call hands it over (WASM-017 R11); in the interpreter the same list is built in host memory. Every host decodes it with the same Rust, so the commands mean the same everywhere.

**Canvas2D**
- **R7 (ubiquitous):** Canvas2D shall provide, with the semantics of the HTML Canvas 2D API where a counterpart exists (a table of differences is part of the KB entry): state (`Save`, `Restore`, `Clear`); paths (`BeginPath`, `MoveTo`, `LineTo`, `QuadTo`, `CubicTo`, `ArcTo`, `Arc`, `Ellipse`, `Rect`, `RoundRect`, `ClosePath`, `Fill`, `Stroke`, `Clip`); shapes (`FillRect`, `StrokeRect`, `ClearRect`, `DrawLine`,
  `DrawCircle`, `DrawPolygon`); style (`SetFillColor`, `SetStrokeColor`, `SetLineWidth`, `SetLineCap`, `SetLineJoin`, `SetDash`, `SetAlpha`, `SetBlendMode`, `SetLinearGradient`, `SetRadialGradient`, `SetPattern`, `SetShadow`); transforms (`Translate`, `Rotate`, `Scale`, `SetTransform`, `ResetTransform`); text (`SetFont`, `SetTextAlign`, `DrawText`, `MeasureText`); images (`DrawImage` of an **ImageBuffer**, whole or a source rectangle,
  scaled or not; `DrawSprite` for a frame of a sprite sheet); read-back (`ToImage`, `GetPixel`, `IsPointInPath`); and `Present`. Rasterisation is **CPU, anti-aliased, deterministic** (`tiny-skia`), and a large canvas may be rasterised in tiles in parallel (R20).
- **R8 (ubiquitous):** Text shall be **shaped, bidirectional and wrapped** correctly for the six languages the product supports, including Japanese and Chinese, with fonts from the bundled base set or from the project's `Assets/`; `MeasureText` shall return exactly what `DrawText` will occupy.

**Canvas3D, Scene3D**
- **R9 (ubiquitous):** Canvas3D renders a **Scene3D** through `wgpu`: `SetScene`, `SetCamera`, `Render`, `Resize`, `Screenshot` (to an ImageBuffer) and `Pick` (the node under a pixel, by ray casting through `parry3d`). **Scene3D** shall provide: primitives (`AddBox`, `AddSphere`, `AddPlane`, `AddCylinder`, `AddCone`, `AddTorus`), `LoadModel` of a glTF 2.0 or GLB asset with its meshes, materials, node transforms and **skeletal and node animations**,
  transforms (`SetPosition`, `SetRotation`, `SetScale`, `LookAt`), materials (colour, metallic, roughness, emissive, an ImageBuffer as texture; a physically based model of the metallic-roughness kind), lights (`AddDirectionalLight`, `AddPointLight`, `AddSpotLight`, `SetAmbient`) with **shadow maps** for the directional and spot lights, `SetBackground`, `SetFog`, cameras (`AddCamera`, perspective and orthographic), `SetAnimation`/`PlayAnimation`, `Remove`, instancing for many copies of one mesh, and frustum culling.
  `Backend` shall name what runs: WebGPU, then WebGL2 on the web; Vulkan, Metal or Dx12 natively; a software adapter in tests; when none exists the control shall show a readable message and raise an event.
- **R10 (ubiquitous):** **Physics2D** and **Physics3D** shall provide a world (`CreateWorld` with gravity), bodies (static, dynamic, kinematic) with shapes (ball, box, capsule, convex hull, trimesh in 3D), `SetPosition`, `SetVelocity`, `ApplyForce`, `ApplyImpulse`, joints (fixed, revolute, prismatic), `RayCast`, `Step` with a **fixed time step** option, `GetPosition`, `GetRotation`, collision events, and `Bind` of a body to a sprite or a Scene3D node so that their transforms follow it.

**Image and chart objects**
- **R11 (ubiquitous):** **ImageBuffer** shall provide: `Create`, `LoadAsset`, `LoadFromBytes`, `Encode` (PNG, JPEG, GIF, WebP, BMP, QOI) and `Save` (the desktop writes a file; the web hands the bytes to the browser as a download or to the backend), `Resize` (nearest, bilinear, Catmull-Rom, Lanczos), `Crop`, `Rotate`, `Flip`, `Fill`, `GetPixel`, `SetPixel`, `DrawOnto` a Canvas2D, `Composite` with blend modes, and filters — grayscale, invert, brightness, contrast,
  hue, saturation, threshold, blur (Gaussian, box), sharpen, edge detection, `Convolve` with a kernel, dilate, erode, histogram. It shall decode only through pure-Rust codecs, with a limit on the pixel count of a decoded image.
- **R12** (ubiquitous): **Plot** shall build charts with `plotters` — line, bar, scatter, area, pie, histogram, candlestick, heatmap, polar, error bars and 3D surface and scatter — from data given by `AddPoint` or by **a COBOL table passed as one block** (`SetSeriesFromTable`: the count, the stride and the numeric kind are described once and the values are read in one pass, WASM-017 R11), with titles, axes, legends, grids and styles, and shall `Render` into a Canvas2D or an ImageBuffer or produce SVG text.

**Input, frames, accessibility**
- **R13 (ubiquitous):** Canvas controls shall be focusable, deliver keyboard, pointer, wheel, touch and gamepad input as events and as pollable state (`IsKeyDown`, `PointerX`, `PointerY`, `GamepadAxis`, `GamepadButton`), and expose `AccessibleLabel` and a description so that the element has a role and a name for assistive technology; a program may set a text alternative that a screen reader reads.
- **R14 (ubiquitous):** With `TargetFps` above zero a canvas shall raise `onFrame` at that rate with the **elapsed time**, paced by the display (`requestAnimationFrame` in the browser, the egui repaint on the desktop), paused while the page or window is hidden, and capped on battery-powered devices; a **fixed-step** mode shall give a game a steady `onFrame` for its physics.

**Safety and limits**
- **R15 (ubiquitous):** The graphics objects shall have **limits** — canvas size, decoded-image pixels, model size, texture size, number of nodes and bodies, display-list length — set by the project with safe defaults, and shall refuse past them with a readable error; shall read no file and open no connection of their own (assets are bundled, anything remote goes through the backend); shall decode untrusted images and models with pure-Rust codecs that are **fuzzed** in the test suite (WASM-013 R15); and shall not let a shader or model take the page down (a lost graphics device is handled and reported).

**Platforms**
- **R16 (ubiquitous):** Every host that runs a form shall implement the controls and objects: **the egui form host** (IDE Preview, Run Form, compiled applications) — Canvas2D uploads the rasterised pixels as a texture, Canvas3D renders into a texture (sharing the device with eframe when its renderer is wgpu, otherwise offscreen with a readback; Q7) — and **the player** (Web and Mobile, canvas elements on the web and in the mobile web view); the **interpreter** and the **generated module** implement the methods and build the display list (WASM-010 R7). The interpreter-binary parity rule applies to all of them.
- **R17 (ubiquitous):** The Form Designer shall show a canvas as a labelled placeholder with its size and backend; the Preview, Run Form and the built application show the live drawing.
- **R18 (ubiquitous):** The player shall load the 3D engine, the chart code and the image filters as **parts on first use** (WASM-016 R13, WASM-017 R3), so that a form without them does not pay for them, and the Build shall report their sizes.
- **R19 (ubiquitous):** A **sample project** (`examples/PowerGraphics`) shall ship with a game (sprites, physics, input), a chart gallery, an image lab and a 3D viewer, built and run on Desktop and Web in the test suite; the Developer's Guide chapter for it is written at release (WASM-INDEX).

**Determinism and parallelism**
- **R20 (ubiquitous):** Work done with Rayon — image filters and resampling, plot rendering, canvas tiling, physics broad phase — shall **give the same result for any number of threads, including one**: per-pixel work is independent, reductions use a fixed order, physics uses `enhanced-determinism`; a test compares one thread against many and the plain player against the threaded one, and golden images compare platforms (exact for the CPU rasteriser; within a stated tolerance for the GPU).

## 5. Acceptance criteria

- [ ] **AC1** — A Canvas2D on a form shows what a handler draws (a line, a filled and a stroked path, a gradient, shaped text in each of the six languages, an image) on the desktop, in a browser and in the mobile web view; the three pictures are pixel-identical (CPU path).
- [ ] **AC2** — A handler that draws ten thousand shapes makes **one** host call to present (a count of host calls in the generated module), and `ToImage` after drawing returns what was drawn.
- [ ] **AC3** — `MeasureText` equals the extent of `DrawText` for Latin, Japanese, Chinese and right-to-left text; wrapped text breaks at the same places on every platform.
- [ ] **AC4** — A glTF model with animation loads into a Scene3D and renders with lights and shadows on a software adapter; the picture matches the golden image within the tolerance; `Pick` returns the node under a pixel.
- [ ] **AC5** — On a machine with WebGPU, WebGL2 only, and neither, `Backend` reports each and a Canvas3D renders or shows the readable message.
- [ ] **AC6** — A Physics2D world with stacked boxes settles to the same final positions on desktop, plain player and threaded player; a body bound to a sprite moves it.
- [ ] **AC7** — An ImageBuffer resized, blurred and encoded gives the same bytes with one thread and with many; a 24-megapixel blur on the threaded player is faster than on the plain one (measured, reported) and does not freeze the page.
- [ ] **AC8** — A Plot of each chart kind renders into a Canvas2D, an ImageBuffer and SVG text; a COBOL table of 100,000 values reaches a series in one pass.
- [ ] **AC9** — Handles are released with their object and with the form; opening and closing a form with a canvas, an image and a scene 100 times leaves memory at its baseline (WASM-017 AC2).
- [ ] **AC10** — Limits refuse an oversized canvas, image, model or display list with a readable error; a corrupt image and a corrupt model are rejected without a crash (the fuzz targets run within their budget).
- [ ] **AC11** — The dependency-guard test over `cobolt-graphics` passes and fails when a denied package is added; no build script in its default tree compiles C or C++.
- [ ] **AC12** — Keyboard, pointer, wheel, touch and gamepad input reach the program on the desktop and in the browser; `onFrame` fires at `TargetFps`, pauses when hidden and steps at a fixed rate in fixed-step mode.
- [ ] **AC13** — The Form Designer shows the placeholder; Preview, Run Form, the compiled binary and the player show the live drawing; the interpreter and the generated module give the same display list for the same program.
- [ ] **AC14** — The sample project builds and runs on Desktop and Web; the Web build loads the 3D engine only when the 3D form is shown.

## 6. Constraints & steering check

- **i18n:** every IDE string (toolbox labels, Properties sections, the readable messages) in six languages.
- **Generated-code contract:** the methods are generated as inline calls and as display-list commands by the generator and the interpreter; the display-list encoding is generated from one definition.
- **System KB:** **required** — two controls, five objects, every property, event and method, the differences from the HTML canvas; regenerate `chunked.data`. The closed method vocabulary must list every method (a test checks each against the interpreter and the generator).
- **Interpreter/binary parity:** the interpreter, the egui host in its three uses, the compiled binary, the generated module and the player.
- **Tech steering:** MSRV 1.92 except the threaded player; the dependency guards, licence and supply-chain rules of WASM-013 R9 apply to every new crate; footprint is reported (R18).
- **Docs:** `docs/` untouched on 2.x.x; the chapter is written at release.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — The rasteriser.** `tiny-skia` on the CPU is chosen for determinism and because it is already here; `vello` (GPU) would be faster for heavy scenes. Revisit when `vello` is stable; the API does not change.
- **Q2 — A scene layer of our own** on `wgpu` against a library engine: ours is chosen for footprint and control; the cost is the work (R9's list). Revisit if a small engine with a library API appears.
- **Q3 — Skeletal animation and shadows** are in R9; if the plan finds either blocks the schedule it must say so rather than drop it silently.
- **Q4 — The desktop renderer.** `eframe` has both glow and wgpu compiled in; sharing the device for Canvas3D needs wgpu. Decide from the IDE's startup code whether to select wgpu explicitly, and what that changes for the rest of the IDE.
- **Q5 — A software adapter for CI** (`wgpu` over a software Vulkan, or a pure-Rust fallback): which, so the 3D goldens run without a GPU.
- **Q6 — Names:** `ImageBuffer` against `Bitmap`, `Plot` against `ChartObject`; chosen to avoid the `Image` and `Chart` control names already in use.
- **Q7 — Fonts:** which fonts ship in the base set for CJK and right-to-left text in the player (size against coverage), and whether the player loads them on demand.
- **Q8 — Audio** is out of this spec; a game wants it. A separate spec could add a sound object on a pure-Rust engine (`kira`); say if it is wanted.
