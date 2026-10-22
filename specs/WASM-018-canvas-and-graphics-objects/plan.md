<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Canvas 2D, Canvas 3D and graphics objects

- **Status:** revised 2026-10-22 against the spec as it stands (the operator's answers of 2026-10-22 to Q4,
  Q5, Q7 and Q8). Part 1 (the `cobolt-graphics` crate) is implemented and tested; part 2 (hosts) is
  **partly** started on the player side; part 3 (product surface) is not started. Awaiting review, then `/tasks`.
- **Spec:** ./spec.md   **First written:** 2026-10-20   **Revised:** 2026-10-22
- **Branch:** `features-2x` (2.x.x line). Part 1 was merged into `features-2x` as `dab1f504` (2.0.21).

## 0. Revision record (2026-10-22)

What changed in the spec since the first plan: only §7. Four questions now carry the operator's answer;
requirements R1–R20 and AC1–AC14 are unchanged (`git show 79400bd9 -- specs/WASM-018-canvas-and-graphics-objects/`).

| Item | Before (plan of 2026-10-20) | Now | Why |
|---|---|---|---|
| Q4 desktop renderer | "decide from the IDE's startup code" (tasks T11) | **Decided (operator):** Canvas3D shares eframe's wgpu device. Section 1.3 designs it | binding decision |
| Q5 software adapter | "none bundled; tests skip loudly; CI uses its own (lavapipe)" | **Decided (operator):** wgpu over a software Vulkan (Mesa lavapipe), **no fallback**. Kept the "no pure-Rust renderer of our own" choice; added a CI job that *requires* the adapter (section 1.6) | binding decision; the old wording left CI optional |
| Q7 fonts | not addressed (the player's Canvas part has no text at all, see below) | **Decided (operator):** ship every font the project references. Section 1.5 designs the collection and loading | binding decision |
| Q8 audio | out of scope | **Answered, to confirm.** Verified which mechanism exists (section 1.7); the question stays open for the operator to close | the answer asks for verification before closing |
| What exists on the player | "the player and its parts loader: not started" | **Wrong by now.** The 2.0.28 merge (`47ce767b`) brought a Canvas 2D part and its page glue (section 1.2). Its design does not meet R6/AC2 or R8 yet; section 1.4 says how it is brought in line | a plan must describe the tree as it is |
| Spec's "both glow and wgpu are compiled in" | repeated as a fact | **Not supported by the code:** `crates/cobolt-ide/src/main.rs:196-200` documents that eframe 0.36 renders through wgpu and that its default features include `wgpu`, not `glow`. `egui_glow` appears under `eframe` in `Cargo.lock`, but the lock also records optional dependencies, so it does not prove glow is built. `cargo tree -i egui_glow` is a task (it could not be run in this session) | do not repeat a claim the code contradicts |
| Spec numbering | — | The spec's §1 ("a `/plan` item (Q7)") and R16 ("(Q7)") point to the renderer question, which is **Q4** in §7; Q7 is fonts. Reported for a spec edit; this plan uses the §7 numbers | consistency |
| tasks.md commit ids | — | `tasks.md` names pre-merge ids (`4f1011f1`, …); the merged history has `7d1c2efc`, `ad753059`, `9bacadc6`, `7d3bbfe4`, `e29ac5a0`, `97961541`, `9b7ef22e`, `91eeb101`, `f6ea760a`. To be corrected by `/tasks` (this run writes plan.md only) | traceability |
| Versioning line in §2 | "no `version.rs`, `CHANGELOG.md`" | Every change bumps `z` with a CHANGELOG entry (`specs/steering/tech.md`, *Versioning*); the merge did (2.0.21). The pipeline does the bookkeeping | the old line contradicted the steering |

Everything else of the first plan (crate design, decisions on crates, evidence) is still right and is kept below.

## 1. Approach

### 1.1 Part 1 — the crate (implemented; unchanged)

One pure-Rust crate, `crates/cobolt-graphics`, that knows nothing of egui, `web-sys` or the file
system (R3). Hosts give it bytes (assets), a surface and a clock; it gives back pixels, text
extents, events and numbers.

```
COBOL program / interpreter / generated module
        |   Value (whole number, number, text, bytes, lists)
        v
   Registry  ---- handles with owners, memory accounting, "busy" guard ----
        |
   api::*  (one table of Method rows per object kind: name, parameters, answer, doc, call)
        |
  +-----------+----------+-----------+-----------+------------+
  | Canvas2D  | ImageBuf | Plot      | Physics   | Scene3D /  |
  | recording | filters  | plotters  | rapier    | Canvas3D   |
  | list      | codecs   | layout    | 2d / 3d   | wgpu PBR   |
  +-----+-----+----------+-----+-----+-----------+------------+
        |                      |
   display list v1  <----------+  (a Plot can be recorded straight into a canvas list)
        |
   Surface (tiny-skia, deterministic bands) + text engine (harfrust / skrifa / fontdb)
```

- **R6 display list.** One `commands!` macro table is the single definition of every command; it
  generates the enum, the encoder, the decoder, the typed builder, the name list (`COMMANDS`) and the
  dynamic builder (`push_dynamic`, proved byte-identical to the typed builder for every command).
  Versioned encoding (`PRGD`, version 1).
- **R7 Canvas2D.** Records into a list and flushes it in one call to `Surface` (tiny-skia 0.12).
  Reads (`ToImage`, `GetPixel`, `IsPointInPath`, `MeasureText`, `Capture`) draw first. Canvases of two
  megapixels or more are rasterised in 256-row bands in parallel; curves are flattened in `f64` with our
  own deterministic maths and blur kernels are Q16 integer, so one thread and many give the same bytes.
- **R8 text.** `harfrust` shapes, `skrifa` reads outlines, `fontdb` finds faces **among the fonts
  handed to it** (`FontSet::new` starts empty; `FontSet::add_font_data`, `crates/cobolt-graphics/src/text/mod.rs:129-153`),
  `unicode-bidi` orders, `icu_segmenter` breaks lines. `MeasureText` and `DrawText` share one layout.
- **R9 Scene3D / Canvas3D.** Scene graph, glTF/GLB loader (meshes, materials, skins, node and skeletal
  animation), culling, shadow fitting and picking are plain maths (`scene3d-core`); the renderer
  (`scene3d`) is `wgpu` 30 with one WGSL metallic-roughness pipeline, up to 8 lights, two shadow maps
  with 3x3 PCF, fog, GPU skinning (128 joints), instancing, 4x MSAA and read-back. Q3 is answered by the
  implementation: skeletal animation and shadows are **in**, nothing was dropped.
- **R10 physics.** `rapier2d`/`rapier3d` 0.36.1, `parallel` + `enhanced-determinism`, behind
  `PhysicsWorld` (handles, fixed-step accumulator, sorted events, guard around `parry` panics).
- **R11 ImageBuffer.** `image` 0.25.10 pure-Rust codecs, `imageproc`, `fast_image_resize`; pixel limit
  checked from the header before allocation; `Arc` copy-on-write.
- **R12 Plot.** `plotters` 0.3.7 without its font layer, through our own `DrawingBackend`s (display list
  and SVG); twelve chart kinds; `SetSeriesFromTable` reads a COBOL table described once
  (`"0:zoned(6,0); 6:packed(7,2,s)"`) in one pass.
- **R2 / R15.** `Registry` owns every object by whole-number handle with an owner (a form);
  `Registry::release_owner` (`registry.rs:376`) releases what a form made; `Limits`
  (`limits.rs:27`) are checked before allocation; hostile input is fuzzed.
- **Coarse API.** `Method` rows are both documentation and dispatch (`Registry::invoke`, `registry.rs:418`;
  `Registry::methods`, `registry.rs:527`); 313 methods on 7 object kinds.

### 1.2 What the hosts have today (read 2026-10-22)

| Host | State | Evidence |
|---|---|---|
| Form model | **No** `Canvas2D`/`Canvas3D`/object variants in `ControlType` (`crates/cobolt-forms/src/model.rs:2746`) | grep of `model.rs` |
| Closed method vocabulary | **No** graphics method in `is_known_method` (`crates/cobolt-ast/src/methods.rs:85`) | grep of `methods.rs` |
| Interpreter, form host, compiler | **No** use of `cobolt-graphics` (`crates/cobolt-runtime/src`, `crates/cobolt-form-host/src`, `crates/cobolt-compiler/src`) | grep |
| Generator (`cobolt-wasm`) | **No** graphics or display-list code | grep of `crates/cobolt-wasm/src` |
| Player | Canvas2D and Canvas3D are recognised **by name** as `ControlType::Custom` (`crates/cobolt-web-player/src/kinds.rs:20-23`, `PlayerKind::Canvas2D/Canvas3D`), drawn as a `<canvas>` (`view/controls.rs:725-733`, pointer and key listeners only) | read |
| Player, Canvas2D methods | Every method except the universal ones becomes a `canvas-call` command, **one per COBOL call**, text arguments (`methods.rs:121-123`, `canvas_method` at `methods.rs:549-584`). **Read-backs are refused** ("only drawing methods are available on a Canvas2D for now") | read |
| Player, Canvas part | `parts/canvas` (`cobolt-web-part-canvas`) wraps a `Registry` with one Canvas2D (`Surface2D`), converts text arguments by the method's parameter kinds and `putImageData`s the RGBA (`src/dom/canvas.rs`). Built with **`cobolt-graphics` `default-features = false`**: no `text`, no `imagebuf`, so `DrawText` and `DrawImage` cannot work there; resources are `NoResources` | `parts/canvas/Cargo.toml`, `parts/canvas/src/lib.rs` |
| Canvas3D, ImageBuffer, Plot, Scene3D, Physics on the web | **Nothing** beyond the `<canvas>` element | grep |

### 1.3 Desktop host (R16, R17, R13, R14; Q4 decided)

- **Renderer (Q4).** The IDE (`crates/cobolt-ide/src/main.rs:144-161`) and every form-host window
  (`cobolt_form_host::native_options`, `crates/cobolt-form-host/src/lib.rs:132-142`) build
  `eframe::NativeOptions` with `wgpu_options` and leave `renderer` at its default. Per the operator's decision
  both set **`renderer: eframe::Renderer::Wgpu` explicitly**, so that Canvas3D can rely on a wgpu device
  and a future eframe default cannot silently move it. The existing pin that keeps the IDE's
  `preferred_backends` equal to the form host's (`wgpu_backend_rule_matches_the_ide`, named at
  `cobolt-form-host/src/lib.rs:93-97`) is extended to the renderer choice.
  *What this changes for the rest of the IDE:* by the code's own account (`main.rs:196-200`) nothing — it
  already renders through wgpu; the edit makes it explicit. The Windows rule (DX12, then GL; Vulkan never
  attempted on a clean Windows 11) is unchanged and now also governs Canvas3D.
- **Sharing the device.** `Cargo.lock` resolves one `wgpu` 30.0.0, used by `egui-wgpu` 0.36.0 and by
  `cobolt-graphics`, so the types are the same. `Gpu` today only opens its **own** instance and adapter
  (`Gpu::new_async`, `crates/cobolt-graphics/src/scene3d/gpu.rs:128-153`, default backends,
  `required_limits` = the adapter's limits). Added: `Gpu::from_shared(device, queue, adapter_info)` that
  the host fills from `eframe::Frame::wgpu_render_state()`; `Gpu::new` stays for tests and headless
  tools. Using eframe's device has two consequences the tasks must cover: (1) a second instance on Windows
  would ignore `preferred_backends` and could reach Vulkan — sharing removes that hazard; (2) the device's
  limits are eframe's, not the adapter's maximum, so `Limits::max_texture_size` and the shadow-map size
  are clamped to `device.limits()` at `Canvas3D` creation, with a readable refusal past them (R15).
- **Canvas3D into egui.** The renderer draws into its own colour target; the host registers that texture
  with `egui_wgpu::Renderer::register_native_texture` and paints it as an image in the control's rect. No
  read-back on the frame path (read-back stays for `Screenshot`).
- **Canvas2D into egui.** On `Present` the host copies `Surface::write_rgba` into an `egui::ColorImage`
  and updates one `TextureHandle` per canvas (`set`, not re-created). The two drawing paths of the
  product (designer `paint.rs`, run form `render.rs`) both get the canvas: the designer draws the
  **placeholder** (R17: name, size, backend), the run form draws the texture.
- **Runtime binding.** `cobolt-runtime` gains an optional feature `graphics` (dependency
  `cobolt-graphics`), selected by `cobolt-compiler`'s `RuntimeFeatures` scan exactly as `audio` and
  `images` are today (`crates/cobolt-compiler/src/runtime_features.rs:96-136`, `scan_forms` at 239,
  `scan_program` at 283), so a program without graphics does not link wgpu or rapier. The interpreter
  keeps one `Registry` per running application; each object created on a form is owned by that form
  instance and `release_owner` runs when the form closes (AC9). Methods dispatch by name through
  `Registry::invoke`; a handle is a whole number returned by `RETURNING` or an inline call.
- **Display list in the interpreter (R6).** A drawing method appends to the canvas's recording
  (`DisplayListBuilder` / `push_dynamic`) in host memory; `Present`, `Flush`, the end of an `onFrame`
  handler with `AutoPresent`, and every read-back execute it in one call. The bytes are what the generated
  module produces (AC13's parity test compares them).
- **Input and frames (R13, R14).** The form host raises `onPointerDown/Move/Up/Enter/Leave`, `onWheel`,
  `onKeyDown/Up`, `onTouchStart/Move/End`, `onResize` and `onFrame` from egui input, only for bound
  handlers (the existing rule), and keeps the pollable state (`IsKeyDown`, `PointerX`, …) per canvas.
  `onFrame` is paced with `request_repaint_after(1/TargetFps)`, suppressed while the viewport is
  minimised or unfocused-and-occluded, with a fixed-step mode that accumulates real time and raises
  steps of exactly `1000/TargetFps` ms. Gamepad through `gilrs` (R13) — subject to the R4 guard (risk §5).
- **Parity (interpreter-binary-parity rule).** `rcrun run-form`, embedded child forms and the compiled
  binary all run through `cobolt-form-host`, so the adapter lives there once; the compiled binary gets
  the `graphics` runtime feature from the same scan.

### 1.4 Generated module and player (R6, R16, R18; AC2, AC13)

The player's current path (one `canvas-call` per COBOL call, read-backs refused) is a working bridge,
not the spec's design: AC2 counts **one** host call for ten thousand shapes, and R7 requires
`ToImage`/`GetPixel`/`MeasureText`/`IsPointInPath` to answer. The plan:

- **One definition of the wire.** The `commands!` table moves into a small `no_std`, no-`alloc` crate,
  **`cobolt-graphics-wire`** (opcodes, argument kinds, an encoder into `&mut [u8]`, the version header);
  `cobolt-graphics` re-exports it and keeps its decoder, builder and executor on top. `cobolt-wasm-rt`
  (`no_std`, per WASM-PLAN P1) links the encoder, so the generated module writes display-list bytes in the
  **arena** with the same code (spec §6: "generated from one definition"). The part-1 tests that prove
  builder ≡ dynamic builder ≡ re-encoded bytes guard the move.
- **ABI amendment (proposal A10, to be written into `specs/WASM-ABI-v1.md` with the code that needs it,
  as A1–A9 were):** a batch command `0x06 DRAW` = `path` (the canvas), `varint len`, `len` bytes of a
  display list. A drawing method in generated code appends to the canvas's list in the arena; `Present`
  (and the other R6 points) emits one `DRAW`. A read-back is a `CALLR` query; the batch before it carries
  the pending `DRAW`, so the read reflects the drawing (ABI §6's rule). If the part is still loading the
  player answers the query by **suspending** the task until it has loaded (the A5 suspension mechanism)
  instead of refusing. The same amendment lists the canvas events' argument texts in §12.2
  (`onFrame` = elapsed ms; pointer = `x,y,buttons`; wheel = `dx,dy`; key = the key name; touch =
  `id,x,y` lines; resize = `w,h`).
- **Objects other than Canvas2D** (ImageBuffer, Plot, Scene3D, Physics2D/3D, Canvas3D) cross the ABI as
  ordinary `CALL`/`CALLR` on their control id with text arguments, the way the Canvas part already
  converts them (`convert` in `parts/canvas/src/lib.rs`), plus a bytes argument where a method takes one;
  `SetSeriesFromTable` passes the table as one argument (count, stride, kind described once, WASM-017 R11).
- **Parts (R18).** The player loads, on first use, parts that are `cobolt-graphics` built with the
  features the part needs: **canvas** (core + `text` + `imagebuf`, so `DrawText`/`DrawImage` work — today it
  has neither), **plot**, **scene3d** (wgpu `webgpu` + `webgl`, per-target features already in
  `crates/cobolt-graphics/Cargo.toml:95-108`), **physics**. Whether ImageBuffer is its own part or rides
  with the canvas part is decided by measurement at `/tasks` (upper bounds in §6: `imagebuf` 356,833 bytes
  brotli). Each object of one form lives in **one `Registry` per form instance inside the part** so
  handles can cross objects (an ImageBuffer drawn on a Canvas2D, a body bound to a node); parts that hold
  objects of one registry are loaded into the same instance of the module rather than as separate
  registries — the exact split is a `/tasks` measurement. The Build report states each part's size
  (`BuildReport`/`PartSize`, named in `specs/WASM-016-prebuilt-player/plan.md` §1.7).
- **Model.** `ControlType` gains the seven kinds; the player's name-based recognition in `kinds.rs` then
  matches the real variants (its own table test, `kinds.rs:16-18`, forces the classification).

### 1.5 Fonts (R8; Q7 decided: every font the project references)

- **What "referenced" means**, collected by the Build (and by Run on the desktop): (a) every `Font`
  family named by a control or form in the project's `.cfrm` files; (b) every family named in a literal
  passed to `SetFont` (Canvas2D, Plot) in the project's COBOL; (c) every font file in the project's
  `Assets/` (and any named by `LoadFontAsset`). A family named only at run time from a data item cannot
  be found statically; the Build warns about a `SetFont` whose argument is not a literal and the program
  may ship the file under `Assets/`.
- **Resolution.** (c) is bytes already. (a) and (b) are resolved on the developer's machine with the
  same `fontdb` system query the desktop painter uses (`crates/cobolt-forms/src/fonts.rs`, `system_fonts`
  at 30 and the face lookup around 155-176). A family that cannot be resolved is a Build **error** for a Web
  or Mobile project (the picture would differ between platforms, AC1) and a warning on Desktop.
- **Shipping and loading.** The files go into the web bundle next to the parts and are handed to the
  Canvas/Plot part's `FontSet` with `add_font_data` when the part loads (they are only needed where text
  is drawn). The desktop registers the same files, so the CPU picture is identical (AC1, AC3).
- **Licence.** A system font is not ours to redistribute by default; the Build reads each file's
  embedding permission (OpenType `OS/2.fsType`) and its name-table licence string, lists them in the
  Build report, and refuses a font whose `fsType` forbids embedding. This is a consequence of the
  decision, recorded as risk §5, not a reversal of it.

### 1.6 3D in CI (R9, AC4, AC5; Q5 decided: software Vulkan, no fallback)

- No pure-Rust software renderer is written or bundled (decision kept, now the operator's).
- A Linux CI job installs Mesa's lavapipe and runs `cargo test -p cobolt-graphics --test scene3d_gpu`
  with `WGPU_BACKEND=vulkan` and **`GRAPHICS_REQUIRE_GPU=1`**, which already turns the loud skip into a
  failure (`crates/cobolt-graphics/tests/scene3d_gpu.rs:12-13, 38-39`). "No fallback" therefore means:
  in CI a missing adapter is red, never skipped. Local runs without an adapter keep the loud `SKIPPED`.
- The golden image's tolerance is measured on lavapipe and on Metal (both recorded); lavapipe reports
  `DeviceType::Cpu`, which `Gpu` already names `Software` (`gpu.rs:135-136`).
- `.github/workflows/build-all-platforms.yml` builds release packages and has no test job today; the
  job is added there or in a new workflow file at `/tasks` (YAML is configuration the repository already
  carries, not program source).

### 1.7 Audio (Q8: answered, to confirm — verified, left open)

What exists: spec 092 R15–R18 — `COBOL::"AUDIO-PLAY"` and four companions (`AUDIO-PAUSE`,
`AUDIO-RESUME`, `AUDIO-STOP`, `AUDIO-STATE`) behind `cobolt-runtime`'s feature `audio`
(`crates/cobolt-runtime/Cargo.toml:111-120`: `rodio` 0.22 through `cpal`, decoding with `symphonia`;
MP3, Vorbis, WAV, FLAC; on Linux ALSA is needed to build and run), in `audio_runtime.rs`, and used by
`examples/PowerCloud/forms/main-form.cfrm` (lines 5428-5751) to play synthesised speech from a file.
So the operator's answer holds **for the desktop**. What it does not cover, for the operator to close:
(1) it plays a **file path**, which the browser does not have; (2) the player has no audio verb (no match
for `AUDIO` in `crates/cobolt-web-player/src/methods.rs`); (3) it is a whole-sound player, not mixed
effects a game triggers. **Working assumption:** WASM-018 adds no audio (spec non-goal stands); web and
mobile audio, if wanted, is a separate spec. Not decided here.

### 1.8 Product surface (R1, R17, R19; steering)

- **Controls and objects:** `ControlType` variants with default sizes, icons, Properties-panel sections,
  `.cfrm` round trip (`crates/cobolt-forms/src/xml.rs`), and a `tests/controls/` test per control.
  Spec R1's `Backend` list has no `OpenGL`, but `Gpu` reports `OpenGl` on a native GL adapter
  (`gpu.rs:140-145`), which the Windows rule can select; the plan keeps `OpenGL` as a value and records
  the difference for the spec (§7, item 4).
- **Closed vocabulary:** every name in `Registry::methods(kind)` for the seven kinds is added to
  `is_known_method`; a test walks the registry rows and fails on a missing name (spec §6).
- **System KB:** the `cobolt-compiler` documentation tables gain two controls and five objects with
  every property, event and method, generated from the same `Method` rows (one source with the
  rustdoc tables), plus the HTML-canvas difference table; `cargo run -p cobolt-ide --example
  build_chunked_kb` regenerates `assets/knowledge/chunked.data` in the same change.
- **i18n:** toolbox labels, Properties sections and the readable messages (no adapter, lost device,
  limit refused, unresolved font) as `Tr` fields in six languages (`crates/cobolt-ide/src/i18n.rs`).
- **Sample project `examples/PowerGraphics`** (R19): game, chart gallery, image lab, 3D viewer; COBOL
  written with the RustCOBOL extensions; credentials scan before commit; the Guide chapter waits for
  release — nothing under `docs/` or `README.md` on 2.x.x.

## 2. Affected crates / files

Done (part 1): `crates/cobolt-graphics/` (library, 12 integration-test targets, 2 examples, 14
golden images); workspace `Cargo.toml` member line; `Cargo.lock`.

Planned:

| Crate / file | Change | R |
|---|---|---|
| new `crates/cobolt-graphics-wire` | `no_std` command table and encoder, moved out of `cobolt-graphics/src/display_list/` | R6 |
| `crates/cobolt-graphics` | re-export the wire; `Gpu::from_shared`; limit clamp to device limits | R6, R9, R15 |
| `crates/cobolt-forms/src/model.rs`, `xml.rs`, `paint.rs`, `render.rs` | seven `ControlType` variants; round trip; designer placeholder | R1, R2, R17 |
| `crates/cobolt-ast/src/methods.rs` | the method names | R1 |
| `crates/cobolt-runtime` (`Cargo.toml`, `interpreter.rs`) | feature `graphics`; registry per application, owner per form; dispatch; display-list recording | R2, R6, R16 |
| `crates/cobolt-compiler/src/runtime_features.rs`, doc tables | `graphics` feature scan; System KB tables | R16, steering |
| `crates/cobolt-form-host/src/lib.rs`, `host.rs` | explicit wgpu renderer; texture upload; shared device; input; frame pacing | R13, R14, R16 |
| `crates/cobolt-ide/src/main.rs`, `i18n.rs`, toolbox/properties | explicit wgpu renderer; Tr ×6; toolbox and sections | R1, Q4 |
| `crates/cobolt-wasm`, `crates/cobolt-wasm-rt` | drawing methods to display list in the arena; `DRAW`; read-back queries | R6, AC2, AC13 |
| `crates/cobolt-web-player` (`methods.rs`, `dom/canvas.rs`, `kinds.rs`, `parts/*`) | `DRAW` handling; read-backs answered (suspend while loading); canvas part with `text` + `imagebuf`; plot, scene3d, physics parts; events | R7, R8, R16, R18 |
| `crates/cobolt-project-tools` (web build) | font collection and resolution, licence check, part sizes in the Build report | R18, Q7 |
| `specs/WASM-ABI-v1.md` | amendment A10 (with the code) | R6 |
| `examples/PowerGraphics/` | sample | R19 |
| `assets/knowledge/chunked.data` | regenerated | steering |
| CI workflow | lavapipe job | Q5 |

Not touched: `README.md`, `docs/`.

## 3. Data / model changes

- **Display list v1** (`PRGD`): unchanged by the move to `cobolt-graphics-wire`; byte-identical (tested).
- **`.cfrm`:** new control types `Canvas2D`, `Canvas3D`, `ImageBuffer`, `Plot`, `Scene3D`, `Physics2D`,
  `Physics3D` with R1's properties. Additive: an older IDE reads them as `Custom` (the model's existing
  way to hold an unknown type, `kinds.rs:20-23`), so nothing is lost on a round trip there.
- **ABI v1:** additive command `0x06 DRAW` and new §12.2 rows (A10). A module without canvases never
  emits it; the player keeps accepting the old `canvas-call` form until the generator emits `DRAW`.
- **Web bundle:** a `fonts/` directory and a manifest list of fonts with their licence fields.
- **Project settings:** graphics limits (R15) with `Limits::default()` (`limits.rs:67`) as defaults.

## 4. Key decisions & alternatives

Crate versions are those resolved in `Cargo.lock` (§6 list).

| Spec row | Decision | Why | Rejected / differs from the spec |
|---|---|---|---|
| R5 text shaping | **`harfrust` 0.12** | `rustybuzz` is archived upstream; `harfrust` is its maintained successor, pure Rust | `rustybuzz`. **Differs from the spec's table** |
| R5 line breaking | **`icu_segmenter` ~2.2** (+ `icu_properties`) | `unicode-linebreak`'s last release (2023-07) fails the twelve-month rule; ICU4X 2.2 is already resolved in the workspace | `unicode-linebreak`. **Differs** |
| R5 rasteriser | **`tiny-skia` 0.12** | two-point conical `RadialGradient` = HTML `createRadialGradient`; 0.11.4 (the `resvg` pin) lacks it | 0.11.4. **Cost:** two `tiny-skia` versions in the tree |
| R5 charts | **`plotters` 0.3.7, default features off**, own backends and own text | `ttf` needs `font-kit` (C); on `wasm32` its canvas backend measures text through the DOM | `ab_glyph` text layer. **Differs** |
| R5 3D maths | **`glam` 0.33** | the release `rapier`/`parry` use; one vector type | older `glam` |
| R5 glTF | **`gltf` 1.4.1 without `import`** | `import` reads files and URLs (R15) | default features |
| R5 shader compiler | **`naga` not a library dependency** | `wgpu` brings its own; a dev-dependency validates WGSL | naga as a dependency |
| R9 features | `scene3d-core` + `scene3d` | maths and loader without a GPU | one all-or-nothing feature |
| R9 backends | per-target `wgpu` features | no unused backends | defaults |
| **Q4 (operator)** | **explicit `Renderer::Wgpu`; Canvas3D shares eframe's device** (`Gpu::from_shared`) | binding; also removes a second instance that would ignore the Windows backend rule | offscreen device with read-back (the spec's alternative) |
| **Q5 (operator)** | **lavapipe in CI, `GRAPHICS_REQUIRE_GPU=1`, no fallback** | binding | a pure-Rust software adapter |
| **Q7 (operator)** | **every referenced font ships**, resolved at Build, loaded with the text part; unresolvable family = error on Web/Mobile | binding; identical pictures need identical font files | a fixed base set; on-demand subsets |
| R6 on the web | **`cobolt-graphics-wire` (`no_std`) + ABI `DRAW`** | one definition for the interpreter, the rt and the player; one host call per present | opcodes duplicated in the generator; keeping one `canvas-call` per COBOL call (fails AC2) |
| Read-backs on the web | **query + suspend until the part loads** | R7 needs the answers; A5 already gives suspension | refusing them (current bridge) |
| R10 | `parallel` + `enhanced-determinism`; thread count capped at 256 | determinism; the API fuzz found an uncapped `SetThreads` slow | uncapped |
| R20 | parallel only over independent pixels/rows; fixed-order reductions | 1 thread = N, tested | order-dependent reductions |
| Safety | `catch_unwind` only around `parry` shape builders and the glTF loader, after own pre-validation | R15 | trusting upstream. **Caveat:** useless under `panic = "abort"` |
| Canvas3D `Screenshot` | blocking natively; `read_pixels_async` on `wasm32` | the browser cannot block on a GPU map | one blocking call |
| `Backend` values | keep `OpenGL` | the Windows rule can select wgpu's GL backend | hiding it as another name |

**Not machine-checked:** R4's "released in the last twelve months" (no release dates offline);
`unicode-bidi` 0.3.18, `plotters` 0.3.7 and `gltf` 1.4.1 may be older than twelve months.

## 5. Risks & mitigations

- **GPU goldens differ between adapters.** Structural GPU tests do not depend on exact pixels; only the
  golden test is tolerance-bound, and its tolerance is now measured on lavapipe (Q5) as well as Metal.
- **eframe's device limits are lower than the adapter's.** Clamp at creation and refuse readably (§1.3).
- **eframe owns the device's lifetime.** A lost device or a surface rebuild invalidates Canvas3D's
  resources; `Gpu` already records a lost device as an error and event (part-1 test), and the host
  re-creates the renderer from the new render state.
- **Font licences (Q7).** Shipping a system font may be forbidden; the Build reads `fsType` and refuses
  (§1.5). A CJK system font is large (tens of MB); the Build report shows it.
- **`gilrs` and R4.** Its Linux backend may link a system library (udev); this is **not verified** here.
  The dependency guard decides; if it fails, Linux gamepad input is reported as a gap rather than the
  guard being relaxed.
- **Moving the command table** risks a byte change: the part-1 equivalence and re-encoding tests run
  before and after the move.
- **Panics in `gltf`/`parry`** under `panic = "abort"`: pre-validation first, the guard is a net.
- **Wasm size:** parts on first use (R18), sizes in §6 are upper bounds.
- **Threaded `wasm32`** was type-checked only (part 1).
- **ALSA on Linux** for audio is outside this spec (Q8).

## 6. Test strategy and evidence

### 6.1 Part 1 evidence (run 2026-10-20; unchanged)

macOS, Apple M3 Pro (11 hardware threads), `cargo test -p cobolt-graphics --no-fail-fast`, default
features, debug: **330 passed, 0 failed, 0 ignored.** With `parallel` off: 328 passed (two
thread-comparison tests compile out).

| Test target | Passed | What it proves |
|---|---|---|
| lib (unit) | 196 | display list encode/decode/builder/dynamic equivalence, canvas internals, text, imagebuf, plot, physics, scene3d maths, `Value`, registry |
| `api` | 15 | every documented method dispatches; rustdoc tables equal the rows; ten thousand shapes are **one** host call; `ToImage` after drawing; image identical on 1 and N threads; 100,000-value table in one call; every chart kind into image, canvas, SVG; limits named; 20,000 random-call iterations never panic |
| `canvas` | 24 | reference scene matches golden; bands = one surface; threads do not change it; clipping, composite, blend, shadow, sprites, pixel ratio 2 |
| `dependency_guard` | 4 | no denied package, no C-compiling build script, permissive licences, MSRV ≤ 1.92; a stub `font-kit` makes it fail |
| `display_list_robustness` | 5 | 200,000 arbitrary inputs, 30,000 mutated lists, 1,500 executed: no panic, no hang; re-encode equality |
| `imagebuf` | 6 | filters and encoders 1 = N threads; decompression bomb refused from header; damaged-file fuzz |
| `lifecycle` | 2 | AC9 at library level: 100 forms opened and closed leave memory at baseline |
| `physics` | 13 | 1 = N threads, repeatability, settling, events, sensors, joints, trimesh, binding, release |
| `plot` | 13 | 12 kinds, SVG, goldens, table reading, recording into a canvas, thread independence |
| `scene3d` | 13 | GLB with skin and animation; skinned pick; data URIs; failed load atomic; limits; 7,000 hostile models never panic |
| `scene3d_gpu` | 16 | on Metal: WGSL valid, `Backend`, lights, shadows, fog, transparency, textures, 10,000 instances, skinning, MSAA, pick, lost device, golden |
| `text` | 17 | synthetic fonts: Latin, CJK, Arabic, Hebrew, bidi, wrapping, limits |
| `text_system_fonts` | 5 | real fonts: `MeasureText` = ink of `DrawText` within 1.5 px for Latin, Arabic, Hebrew, Japanese, Chinese |
| doctest | 1 | crate example |

Also: `cargo check` for 20 feature/target combinations (native and `wasm32-unknown-unknown`);
`cargo doc` 0 warnings; clippy 0 errors (26 style warnings).

Measurements (1 thread vs 11, `examples/measure.rs`, same bytes in every row): 24 MP Gaussian blur σ4
655.1 → 208.1 ms; σ20 500.5 → 211.9 ms; Lanczos resize 107.3 → 73.9 ms; sharpen 329.0 → 163.7 ms;
Canvas2D 10,000 shapes at 2048x1536 555.4 → 145.9 ms; Plot 100,000 points at 3000x2000 171.0 → 75.2 ms;
Physics3D 1,000 bodies x 240 steps 201.3 → 180.1 ms; culling 1,000,000 instances 41.8 → 27.1 ms. GPU
(Metal, 1280x720, 4x MSAA): 98,304 to 1,998,848 triangles in 2.3–4.0 ms per frame.

`wasm32` sizes, brotli, upper bounds from `examples/size_probe.rs`: core 148,668; text 400,871;
imagebuf 356,833; plot 560,452; physics 814,490; scene3d-core 460,682; scene3d 1,182,935; all five
2,127,515; all five + parallel 2,174,407. The player's actual Canvas 2D part (no text) measured
469,419 bytes uncompressed (`specs/WASM-016-prebuilt-player/plan.md`, baseline 2026-10-20).

Crates (resolved): `bytemuck` 1.25.0, `fast_image_resize` 6.1.0, `fontdb` 0.23.0, `glam` 0.33.12,
`gltf` 1.4.1, `harfrust` 0.12.0, `icu_properties`/`icu_segmenter` 2.2.0, `image` 0.25.10, `imageproc`
0.27.0, `kurbo` 0.13.1, `parry3d` 0.31.1, `plotters`/`plotters-backend` 0.3.7, `pollster` 0.4.0,
`rapier2d`/`rapier3d` 0.36.1, `rayon` 1.12.0, `skrifa` 0.44.0, `thiserror` 2.0.21, `tiny-skia` 0.12.0,
`unicode-bidi` 0.3.18, `wgpu` 30.0.0. Dev: `epaint_default_fonts` 0.36.1, `naga` 30.0.0, `serde_json`.

### 6.2 Tests to add (parts 2 and 3), each reporting numbers

| Test | Crate | Asserts and reports |
|---|---|---|
| wire move | `cobolt-graphics(-wire)` | every command encodes to the same bytes before and after; count of commands checked |
| renderer pin | `cobolt-form-host` | IDE and host both select `Renderer::Wgpu` and the same backends |
| shared device | `cobolt-form-host` (GPU, lavapipe in CI) | Canvas3D created from a `RenderState` renders, reports `Backend`, clamps a texture past `device.limits()` with a message |
| runtime binding | `cobolt-runtime` | a COBOL program creates each of the seven kinds, gets a handle with `RETURNING`, calls one method each; closing the form releases them (count before/after) |
| vocabulary | `cobolt-ast` | every `Registry::methods` name is known; reports the count (313 today) |
| display-list parity (AC13) | `cobolt-wasm` | the same program's list bytes from the interpreter and from the module (on `wasmi`) are equal; reports byte counts |
| one host call (AC2) | `cobolt-wasm` | 10,000 shapes then `Present` = 1 `flush` with 1 `DRAW`; reports calls and bytes |
| read-back on the web | `cobolt-web-player` | `GetPixel` after drawing answers the drawn colour; while the part loads, the task suspends and resumes |
| fonts (Q7) | `cobolt-project-tools` | a project referencing a family in a `.cfrm`, a `SetFont` literal and an `Assets/` file ships exactly those three; an unresolvable family fails a Web build; a `fsType`-restricted font is refused |
| `.cfrm` round trip | `cobolt-forms` | each new type and every R1 property survive save/load |
| controls | `tests/controls/` | one test per control |
| CPU parity (AC1) | player + desktop | the same list on the desktop host and the canvas part (native and `wasm32` via a headless engine) gives identical RGBA; reports differing-pixel count (must be 0) |
| GPU in CI (AC4, AC5) | `cobolt-graphics` | `scene3d_gpu` on lavapipe with `GRAPHICS_REQUIRE_GPU=1`; tolerance recorded |
| frames and input (AC12) | `cobolt-form-host`, player | `onFrame` rate within a stated tolerance; fixed step exact; hidden = paused; each input event reaches a bound handler |
| lifecycle (AC9) | form host, player | 100 open/close cycles with a canvas, an image and a scene: memory at baseline (`pr_dbg_live`/region use on the web) |
| KB freshness | `cobolt-ide` | `prebuilt_chunked_kb_matches_the_published_documentation` green after regeneration |

Manual/visual: the operator judges the look (Designer placeholder, Run Form, browser) with the MCP
`render_form`/`run_form` tools; no UI automation of the operator's window.

## 7. Open questions (state of the spec's §7, and what this plan adds)

1. **Q1 (vello), Q2 (own scene layer), Q6 (names):** recorded choices, no operator decision needed now;
   kept as the spec states them (`ImageBuffer`, `Plot`; own scene layer; `tiny-skia`).
2. **Q3:** answered by part 1 (skeletal animation and shadows implemented and tested on Metal).
3. **Q8 (audio): answered, to confirm — still open.** Verified in §1.7: desktop-only, file-based.
   Working assumption: no audio in WASM-018. The operator closes it.
4. **New, for the spec at `/clarify`:** (a) §1 and R16 cite "Q7" for the renderer, which is Q4;
   (b) "both glow and wgpu are compiled in" is contradicted by `cobolt-ide/src/main.rs:196-200` (to be
   confirmed with `cargo tree -i egui_glow`); (c) R1's `Backend` list lacks `OpenGL`, which the
   Windows rule can produce; (d) R5's table names `rustybuzz`, `unicode-linebreak` and `plotters`'
   `ab_glyph` text, which this plan replaced with reasons (§4).
5. **ABI A10** is a proposal in this plan; it becomes normative only when written into
   `specs/WASM-ABI-v1.md` with the code that needs it.

## 8. Steering compliance

- [x] 100 % Rust: dependency guard, deny list, build-script check (AC11); every new crate passes it.
- [x] No file read and no connection by the crate; fonts and assets come from the host.
- [ ] Interpreter/binary parity: designed (one adapter in `cobolt-form-host` for run-form, child forms
      and the compiled binary; the generated module's list compared byte for byte with the interpreter's).
- [ ] i18n: Tr fields ×6 planned (§1.8).
- [ ] Generated-code contract: `cobolt-codegen` emits nothing new for these controls beyond the existing
      control declarations; the banner and regenerate-on-action contract are unchanged.
- [ ] System KB: tables from the `Method` rows and regenerated `chunked.data`, in the same change as the
      controls.
- [x] `docs/` and `README.md` untouched on 2.x.x; the Guide chapter is written at release.
- [x] Feature, on `features-2x`; `z` bump and CHANGELOG by the pipeline.
- [x] No "cobolt" in user-facing text (crate names are build-only).
- [x] User code is sacred: no change rewrites a project's COBOL; the font collection only reads it.

## 9. What is not done (stated plainly)

- **Part 1 only partly meets:** AC4 (Metal only; lavapipe not run), AC5 (WebGPU/WebGL2, Vulkan, DX12
  not run), AC6 (plain vs threaded player not run), AC7 (threaded player speed not measured), R20 across
  native/`wasm32` not compared.
- **Not met at all yet:** AC1, AC2 (on the web), AC3 (on the web: the canvas part has no text), AC8 and
  AC10–AC11 beyond the library, AC12, AC13, AC14.
- **The player's Canvas2D bridge** works for drawing only and does not meet R6/AC2 or R8 (§1.2, §1.4).
