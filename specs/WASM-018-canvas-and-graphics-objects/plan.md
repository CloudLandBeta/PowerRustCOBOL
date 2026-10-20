<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Canvas 2D, Canvas 3D and graphics objects: the `cobolt-graphics` crate (part 1)

- **Status:** part 1 (the crate) implemented and tested; parts 2 and 3 (hosts and controls) not started
- **Spec:** ./spec.md   **Date:** 2026-10-20
- **Branch:** `features-2x` line (worktree branch `worktree-agent-ae803a03303e7c7c3`); nothing is pushed.

This plan covers the **crate** (R3-R12, R15, R18's measurement, R20, AC2, AC3, AC4, AC7-AC11 at the
level of a library). The controls, the egui host, the player, the generator, the System KB, the
Designer, six-language labels, the sample project and the browser tests (R1, R13, R14, R16, R17,
R19, AC1, AC5's browser rows, AC12-AC14) are separate work; section 8 says plainly what that leaves
undone.

## 1. Approach

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
  generates the enum, the encoder, the decoder, the typed builder, the name list (`COMMANDS`, with
  each command's one-line documentation) and the dynamic builder (`push_dynamic`, which records any
  command by name from `Value`s and is proved byte-identical to the typed builder for every
  command). The encoding is versioned (`PRGD`, version 1). Encoder, decoder and executor share the
  table, so they cannot drift.
- **R7 Canvas2D.** `Canvas2D` records into a list and flushes it in one call to the rasteriser
  (`Surface`, tiny-skia 0.12). Reads (`ToImage`, `GetPixel`, `IsPointInPath`, `MeasureText`,
  `Capture`) draw first. Large canvases are rasterised in horizontal bands (256 rows when the canvas
  is two megapixels or more) that run in parallel; curves are flattened in `f64` with our own
  deterministic maths and the blur kernels are Q16 integer, so one thread and many give the same
  bytes.
- **R8 text.** `harfrust` shapes, `skrifa` reads outlines and metrics, `fontdb` finds fonts,
  `unicode-bidi` orders, `icu_segmenter` breaks lines, `icu_properties` gives the script of each
  run. `MeasureText` and `DrawText` use the same layout, so the extents agree (tested against the
  ink of the drawn pixels).
- **R9 Scene3D / Canvas3D.** The scene graph, the glTF/GLB loader (meshes, materials, node
  transforms, skins, node and skeletal animation), culling, shadow fitting and picking are plain
  data and maths (`scene3d-core`, no GPU). The renderer (`scene3d`) is `wgpu` 30 with one WGSL
  physically based pipeline: metallic-roughness (GGX, Smith, Schlick), up to 8 lights, two shadow
  maps with 3x3 PCF, fog, gradient background, GPU skinning (up to 128 joints), instancing, 4x MSAA,
  premultiplied blending and read-back. `Backend` names what ran.
- **R10 physics.** `rapier2d`/`rapier3d` 0.36.1 with `parallel` and `enhanced-determinism`, behind a
  `PhysicsWorld` that owns the handle table, a fixed-step accumulator, sorted collision events and a
  last-resort `catch_unwind` around the parry calls that can panic on degenerate input.
- **R11 ImageBuffer.** `image` 0.25.10 pure-Rust codecs, `imageproc`, `fast_image_resize`; a pixel
  limit checked **from the header before allocation**; `Arc` copy-on-write so handing an image to a
  canvas does not copy it.
- **R12 Plot.** `plotters` 0.3.7 without its font layer, drawing through **our own
  `DrawingBackend`** (a display-list backend and an SVG backend), with twelve chart kinds and a text
  description of a COBOL table row (`"0:zoned(6,0); 6:packed(7,2,s)"`) so a table of 100,000 rows
  reaches a series in one pass (count, stride, numeric kind).
- **R2 / R15 handles and limits.** `Registry` owns every object by whole-number handle with an
  owner (a form); `release_owner` releases what a form made; memory is accounted against
  `max_total_bytes`; an object that uses its own handle during its own method gets "busy" instead of
  a deadlock. `Limits` are checked before any allocation; hostile input is fuzzed (below).
- **Coarse API.** `Value` and `Method<T>` rows are both the documentation and the dispatch: the
  method tables in the crate's top-level rustdoc are **generated from the rows** and a test fails
  when they drift (`DOC_UPDATE=1` rewrites them). 313 methods on 7 object kinds.

## 2. Affected crates / files

New: `crates/cobolt-graphics/` (library, 12 integration-test targets, 2 examples, 14 golden images).
Edited outside it: workspace `Cargo.toml` (one line: the new workspace member) and `Cargo.lock`.
Nothing else: no `version.rs`, `CHANGELOG.md`, `WASM-PROGRESS.md`, `README.md` or `docs/` (2.x.x rules).

| Module | Contents |
|---|---|
| `display_list/` | the command table, `DisplayList` encoder/decoder, `DisplayListBuilder`, `push_dynamic`, constants |
| `canvas/` | `Surface` on tiny-skia: path, paint, state, bands, blur/shadow, flatten |
| `canvas2d.rs` | `Canvas2D`, the recording wrapper |
| `text/`, `textapi.rs` | fonts, script runs, layout, the `TextEngine` trait |
| `imagebuf/` | codecs, geometry, filters, blur |
| `plot/` | kinds, table reader, `ListBackend`/`SvgPlotBackend`, render |
| `physics/` | `PhysicsWorld`, `dim2`, `dim3` |
| `scene3d/` | scene, mesh, primitives, model (glTF), anim, frame (culling, shadows, lights), pick, gpu, canvas3d, shaders |
| `value.rs`, `registry.rs`, `api/` | the COBOL-facing layer |
| `limits.rs`, `handles.rs`, `error.rs`, `par.rs`, `detmath.rs`, `pixels.rs`, `color.rs`, `resources.rs` | shared plumbing |

## 3. Data / model changes

- **Display list v1** (`PRGD`): header (magic, version, command count, payload length) then
  commands; every length and count is checked against `Limits` before it allocates. The decoder
  refuses what it does not know instead of guessing. Re-encoding a decoded list gives the same bytes
  (tested on mutated lists).
- **Table description** for `SetSeriesFromTable`: a text spec of column offset and usage
  (`zoned`, `packed`, binary, display, signed), read once per call.
- No `.cfrm`, `.cidx` or IDE model changes yet (that is the controls' work).

## 4. Key decisions & alternatives

Crate versions are those resolved in `Cargo.lock` on 2026-10-20 (the full list is in section 6).

| Spec row | Decision | Why | Rejected / differs from the spec |
|---|---|---|---|
| R5 text shaping | **`harfrust` 0.12** | `rustybuzz` is archived upstream; `harfrust` is its maintained successor from the same shaping line, pure Rust | `rustybuzz` (fails R4's "released in the last twelve months / maintained" intent) |
| R5 line breaking | **`icu_segmenter` ~2.2** (+ `icu_properties` ~2.2 for the script of a run) | `unicode-linebreak`'s last release is 2023-07, which fails the twelve-month rule; the ICU4X 2.2 line is the one `idna`/`url` already resolve in this workspace, so no shared package moves; correct CJK line breaking rules (no break before closing punctuation) | `unicode-linebreak` |
| R5 rasteriser | **`tiny-skia` 0.12** | its two-point conical `RadialGradient` is exactly HTML `createRadialGradient(x0,y0,r0,x1,y1,r1)`, which R7 needs; 0.11.4 (the `resvg` pin) lacks it | 0.11.4. **Cost:** a second `tiny-skia` version is now in the workspace tree (0.11.4 through `resvg`, 0.12 here) |
| R5 charts | **`plotters` 0.3.7, default features off**, own backends | its `ttf` feature uses `font-kit` (C libraries) and, on `wasm32`, its canvas backend measures text through the DOM; neither is acceptable (R4, R20). Layout is plotters', drawing and text are ours, so a chart is identical on every platform | plotters' `ttf` layer (C libraries) and the `ab_glyph` layer the spec names (a second text path that would measure and shape differently from Canvas2D). **Differs from the spec:** the spec says `ab_glyph`; this crate uses its own text engine instead |
| R5 3D maths | **`glam` 0.33** | the release `rapier`/`parry` (through `glamx`) already use: one vector type shared by a Scene3D node and a physics body, no second `glam` | an older `glam` |
| R5 glTF | **`gltf` 1.4.1 without its `import` feature** | `import` reads files and fetches URLs; R15 forbids both. Buffers and images are resolved by our own loader from the GLB chunk, `data:` URIs or the host's `AssetHost` | the default feature set |
| R5 shader compiler | **`naga` is not a library dependency** | `wgpu` compiles the WGSL with its own `naga`; a dev-dependency validates the shaders in a test | naga as a normal dependency (it was one until this commit) |
| R9 features | extra feature **`scene3d-core`** | scene graph, glTF loader, culling, picking and shadow fitting run on any machine with no GPU; `scene3d` = `scene3d-core` + `wgpu` | one all-or-nothing `scene3d` |
| R9 backends | `wgpu` backend features **per target** (Metal on Apple, Vulkan + GL on other unix, DX12 + Vulkan on Windows, WebGPU + WebGL2 on `wasm32`) | the default features would pull every backend into every target | `gles` on Apple, `webgl` natively |
| R9 / Q5 software adapter | **none bundled** | no pure-Rust software `wgpu` adapter exists; a CI machine uses its own (Mesa `lavapipe`/`llvmpipe`). Tests skip **loudly** (a line starting `SKIPPED`) when no adapter exists; `GRAPHICS_REQUIRE_GPU=1` makes the skip a failure | shipping a software rasteriser of our own |
| R10 | `rapier` `parallel` + `enhanced-determinism`; thread counts capped at 256 | determinism (R20); a billions-of-threads `SetThreads` took 66 s in the API fuzz | uncapped |
| R20 | Parallel only where work is per-pixel or per-row independent; reductions in fixed order; `parallel` OFF gives the **same bytes** | tests compare 1 thread with N for images, canvas, plot, physics and culling | any order-dependent reduction |
| Safety | `catch_unwind` as a last resort in two places only: the physics shape builders (`parry` panics on degenerate hulls) and the glTF loader (the `gltf` crate indexes unchecked in places); own pre-validation first. The image decoders have **no** guard: the fuzz found no panic in them | R15: a model or image must not take the page down. **Caveat:** useless with `panic = "abort"`, and a caught panic still prints its message | trusting the upstream crates |
| Canvas3D `Screenshot` | blocking and native-only; `Renderer::read_pixels_async` is the `wasm32` form | the browser cannot block on a GPU map | one blocking call everywhere |

**Not machine-checked:** R4's "released in the last twelve months" for every crate. Release dates
are not available offline; `unicode-bidi` 0.3.18, `plotters` 0.3.7 and `gltf` 1.4.1 are the newest
resolved releases but may be older than twelve months. The dependency guard checks licence, minimum
Rust version, the deny list and build-script tooling, not release age.

## 5. Risks & mitigations

- **GPU goldens differ between adapters.** Tolerance-based comparison; only Apple Metal was run
  here. *Mitigation:* the structural GPU tests (lit/unlit sides, shadow present only when asked,
  fog blend, instancing, skin pose changes the picture, MSAA smooths edges, pick agrees with the
  picture) do not depend on exact pixels; the golden test is the only tolerance-bound one.
- **Panics inside `gltf` and `parry`.** Own validation before the call, a guard around it, and the
  fuzz asserts the guard's "INTERNAL ERROR" marker never appears (the pre-validation catches
  everything the fuzz produced; the guard is the net under it).
- **Wasm size.** The player loads each heavy area as a part on first use (R18); sizes below.
- **Threads in the browser.** `rayon` is used only through `par`, which runs sequentially on
  `wasm32` without `atomics`; the threaded build was type-checked only.
- **`catch_unwind` with `panic = "abort"`** (a plausible wasm profile) turns a guarded panic into an
  abort. The pre-validation exists for that reason, but the guard is not a guarantee there.

## 6. Test strategy and evidence

Run on macOS (Apple M3 Pro, 11 hardware threads), `cargo test -p cobolt-graphics --no-fail-fast`,
default features, debug profile: **330 tests passed, 0 failed, 0 ignored.**

| Test target | Passed | What it proves |
|---|---|---|
| lib (unit) | 196 | display list encode/decode/builder/dynamic equivalence, canvas internals, text, imagebuf, plot, physics, scene3d maths, `Value`, registry |
| `api` | 15 | every documented method dispatches and nothing else does; the rustdoc tables equal the dispatch rows; ten thousand shapes are **one** host call; `ToImage` after drawing; images cross between objects; image resized, blurred and encoded is identical on 1 and N threads; a COBOL table of 100,000 values reaches a series in one call; every chart kind renders into an image, a canvas and SVG; body drives scene node; limits named in errors; **20,000 iterations of random calls with hostile values never panic** (5,367 calls accepted and 13,809 refused with a readable message; the rest of the iterations create objects or skip a deliberately slow call) |
| `canvas` | 24 | the reference scene (every command) matches its golden image; bands equal one surface; threads do not change a banded picture; clipping, composite, blend, shadow, sprite frames, pixel ratio 2 |
| `dependency_guard` | 4 | the whole tree (every target and feature) has no denied package, no C-compiling build script, only permissive licences, MSRV <= 1.92; **a stub `font-kit` added to a real cargo project makes the same walk fail** |
| `display_list_robustness` | 5 | 200,000 arbitrary byte strings, 30,000 mutated lists, 1,500 executed lists, extreme numbers: no panic, no hang; a decodable list re-encodes to the same bytes |
| `imagebuf` | 6 | every filter gives the same bytes on 1 and N threads; encoders likewise; a decompression bomb is refused from its header; 6 formats x 6,000 damaged files and 20,000 random inputs with a valid signature never panic; odd-size round trips |
| `lifecycle` | 2 | AC9: a hundred forms with a canvas, an image, a chart, a scene and a physics world, opened and closed, leave memory at its baseline (counting allocator: 0 bytes above after 25 forms, 80 after 50, 75 after 100); handles do not survive their form |
| `physics` | 13 | 1 thread equals N (2D and 3D), same program twice equals same bytes, stacked boxes settle to known positions, events, sensors, joints (pendulum, slider, fixed, hinge limit), trimesh floor and hull body, bound transforms, release |
| `plot` | 13 | 12 chart kinds render pixels and SVG, golden images, legends and category names, pie labels, table reading (zoned, packed, mixed), recorded straight into a canvas list, thread independence, degenerate plots |
| `scene3d` | 13 | a GLB with skin, animation and material loads completely; the skeleton bends the mesh where the animation says; skinned pick; `.gltf` with data URI equals with files; failed load leaves the scene as it was; limits, texture bomb, no network, sparse accessor, 50,000-deep hierarchy; **4,000 mutated models (561 loaded, 3,439 refused) and 3,000 random inputs never panic**; error messages are words |
| `scene3d_gpu` | 16 | (on Metal) valid WGSL, `Backend` reported, no connection = status not crash, lighting, shadows only when asked, spot and point lights, fog, transparency, textures, 10,000 instances in one draw with culling, skinned pose, MSAA, determinism and resize, pick agrees with the picture, **lost device = error and event, never a crash**, golden image |
| `text` | 17 | synthetic fonts with exact metrics: Latin widths, CJK full width, Arabic and Hebrew right to left, mixed text reordered per line, wrapping (Latin, CJK, closing punctuation), long words, explicit line height, repeatable from many threads, limits |
| `text_system_fonts` | 5 | **real fonts**, read from the machine, never copied into the repo: Latin (Helvetica, kerning), Arabic (Geeza Pro, four joined letters 61.21 wide against 102.35 as isolated forms), Hebrew and Japanese (Arial Unicode MS), Chinese (Hiragino Sans GB); in each, `MeasureText` equals the ink of `DrawText` within 1.5 px (e.g. "Hello, World" 217.1 wide, Arabic "مرحبا بالعالم" 178.0, "日本語のテキスト" 320.0). A script with no font **skips loudly** (`SKIPPED`); `GRAPHICS_REQUIRE_FONTS=1` fails instead |
| doctest | 1 | the crate's top-level example |

The same suite with `parallel` **off** (`--no-default-features --features
text,imagebuf,plot,physics,scene3d`, the sequential fallback) passes too: 328 passed, 0 failed (the
two physics tests that compare 1 thread with many need rayon and are compiled out). The golden-image
tests pass in both builds, which is the evidence for "the fallback gives the same pictures".

Also run: `cargo check` for every feature alone and the default set, natively and for
`wasm32-unknown-unknown` (20 of 20 combinations OK: no features, each of the seven features alone,
the default set, and `text,imagebuf,physics,scene3d`, on each target); `cargo doc` with 0 warnings; `cargo clippy
--all-targets` with 0 errors (26 style warnings remain in the library: `is_multiple_of`,
assign-ops, argument counts; none changes behaviour).

### Measurements (1 thread against N)

`cargo run --release --example measure -p cobolt-graphics` on the machine above; best of 2-3 runs;
"same bytes" is a byte comparison of the output.

| What | 1 thread | 11 threads | speed-up | same bytes |
|---|---|---|---|---|
| ImageBuffer 24 MP (6000 x 4000): Gaussian blur sigma 4 (exact kernel) | 655.1 ms | 208.1 ms | 3.15x | yes |
| ImageBuffer 24 MP: Gaussian blur sigma 20 (three box passes) | 500.5 ms | 211.9 ms | 2.36x | yes |
| ImageBuffer 24 MP: resize to 3000 x 2000, Lanczos | 107.3 ms | 73.9 ms | 1.45x | yes |
| ImageBuffer 24 MP: saturation + contrast | 145.3 ms | 120.2 ms | 1.21x | yes |
| ImageBuffer 24 MP: sharpen | 329.0 ms | 163.7 ms | 2.01x | yes |
| Canvas2D 2048 x 1536: execute a display list of 10,000 shapes | 555.4 ms | 145.9 ms | 3.81x | yes |
| Plot 100,000 points: render 1200 x 800 | 42.3 ms | 39.9 ms | 1.06x | yes |
| Plot 100,000 points: render 3000 x 2000 | 171.0 ms | 75.2 ms | 2.28x | yes |
| Physics3D: 1,000 bodies, 240 steps | 201.3 ms | 180.1 ms | 1.12x | yes |
| Scene3D: frustum culling of 100,000 instances | 4.5 ms | 3.6 ms | 1.24x | yes |
| Scene3D: frustum culling of 1,000,000 instances | 41.8 ms | 27.1 ms | 1.54x | yes |

Other numbers from the same run: recording 10,000 shapes into a display list takes 0.5 ms (400 KB,
70,000 commands); 100,000 points added one by one 1.6 ms; a COBOL table of 100,000 rows (zoned x,
packed y) into a series in one pass 3.7 ms.

GPU (Apple M3 Pro, Metal; 1280 x 720, 4x MSAA, one sun with a shadow map, a floor and N instanced
spheres of 4,096 triangles; best of 5 frames after a warm-up; the frame figure is the CPU build, the
GPU work and a wait for the GPU to go idle; the read-back is best of 3). The triangle count is the
scene's total: the renderer culls instances outside the frustum, so fewer may be drawn:

| Triangles | frame | read-back |
|---|---|---|
| 98,304 | 2.3 ms | 1.4 ms |
| 499,712 | 1.6 ms | 1.2 ms |
| 999,424 | 2.5 ms | 1.2 ms |
| 1,998,848 | 4.0 ms | 1.7 ms |

Physics gains little from threads at this size (1.12x); the figure is reported as measured.

### `wasm32` size of each heavy area (R18)

`cargo build --release --example size_probe --target wasm32-unknown-unknown`, one feature set at a
time (the probe keeps every method of every object of that feature set alive). The wasm-opt column is
after `wasm-opt`. **These are upper bounds:** the probe retains every method, the areas share code
(the all-features figure is smaller than the sum), and the player splits parts differently.

| Feature set | raw | wasm-opt | gzip | brotli |
|---|---|---|---|---|
| core (display list, canvas, registry; no features) | 622,819 | 581,817 | 197,324 | 148,668 |
| text | 1,602,513 | 1,421,610 | 522,597 | 400,871 |
| imagebuf | 1,343,068 | 1,231,276 | 459,346 | 356,833 |
| plot (includes text) | 2,476,756 | 2,220,926 | 760,955 | 560,452 |
| physics | 3,166,993 | 2,905,467 | 1,156,331 | 814,490 |
| scene3d-core | 1,819,628 | 1,657,988 | 605,797 | 460,682 |
| scene3d (core + wgpu) | 5,217,521 | 4,711,828 | 1,602,075 | 1,182,935 |
| all five areas | 8,911,155 | 8,037,752 | 2,949,014 | 2,127,515 |
| all five + `parallel` | 9,343,809 | 8,384,984 | 3,072,090 | 2,174,407 |

Bytes. "Physics" is both rapier worlds (2D and 3D).

### Final crate list (resolved versions)

Normal: `bytemuck` 1.25.0, `fast_image_resize` 6.1.0, `fontdb` 0.23.0, `glam` 0.33.12, `gltf` 1.4.1,
`harfrust` 0.12.0, `icu_properties` 2.2.0, `icu_segmenter` 2.2.0, `image` 0.25.10, `imageproc`
0.27.0, `kurbo` 0.13.1, `parry3d` 0.31.1, `plotters` 0.3.7, `plotters-backend` 0.3.7, `pollster`
0.4.0 (native only), `rapier2d` 0.36.1, `rapier3d` 0.36.1, `rayon` 1.12.0, `skrifa` 0.44.0,
`thiserror` 2.0.21, `tiny-skia` 0.12.0, `unicode-bidi` 0.3.18, `wgpu` 30.0.0.
Dev only: `epaint_default_fonts` 0.36.1 (fonts egui already ships), `image` (png), `naga` 30.0.0,
`serde_json` 1.0.150.

## 7. Steering compliance

- [x] 100 % Rust: dependency guard over the tree; build-script tooling check; deny list; fails when a
      denied package is added (AC11).
- [x] No file read, no connection opened by the crate (assets come from the host; the glTF loader
      follows no URL).
- [x] Interpreter/binary parity: the crate is the one implementation all hosts will call; the host
      adapters are not written, so the parity test that needs them is not either.
- [ ] i18n: no IDE strings in this part; the controls' six-language labels are the controls' work.
- [ ] System KB: the method tables exist (generated from the dispatch rows) but are not yet in the
      `cobolt-compiler` doc tables, and `chunked.data` is unchanged.
- [x] `docs/`, `README.md`, `version.rs`, `CHANGELOG.md`, `WASM-PROGRESS.md` untouched.
- [x] Feature, on the 2.x.x line (`features-2x`). Commit trailer as the harness specifies.
- [x] No "cobolt" in user-facing text: the crate's user-facing strings (errors, method docs) say
      "the canvas", "the image" and so on; `cobolt-graphics` is a build-only crate name.

## 8. What is not done

Stated plainly, so no criterion is read as met that was not run:

- **Not part of the crate work (separate specs / later parts):** the Canvas2D and Canvas3D
  controls and their properties, events and `.cfrm` round trip (R1, R13, R14, R17), the egui host
  adapter (R16), the player and its parts loader (R16, R18's loading), the generator and the
  interpreter bindings, closed method vocabulary entries (`methods.rs`), System KB tables and
  `chunked.data`, six-language labels, `tests/controls/`, the sample project `examples/PowerGraphics`
  (R19), gamepad (R13, `gilrs`), Designer placeholder.
- **AC1, AC12, AC13, AC14: not run** (need the hosts).
- **AC5: partly.** `Backend` is reported and the no-adapter and lost-device paths are tested;
  WebGPU and WebGL2 were **not run** (no browser); Vulkan and DX12 were not run.
- **AC4: partly.** A skinned, animated glTF loads, renders with lights and shadows and picks, and
  matches its golden image **on Metal**. No software adapter was run; the golden tolerance on other
  adapters is **unverified**.
- **AC6: partly.** Stacked boxes settle to the same positions on every run and with 1 or N threads
  on this machine; the plain player against the threaded player was **not run**. Binding a body to a
  *sprite* is the control's job; binding to a Scene3D node is tested.
- **AC7: partly.** Same bytes for 1 thread and N is tested and measured; "a 24-megapixel blur on the
  threaded player is faster than on the plain one" was **not run** (no browser, no threaded wasm
  build).
- **R20 across platforms:** native-versus-wasm exactness and the SIMD-versus-scalar paths of
  `fast_image_resize` were **not compared**.
- **Threaded `wasm32`** (nightly, `build-std`, `+atomics`) was type-checked, **not built or run**.
- **R4 release-age rule** is not machine-checked (above).
- **Clippy:** 26 style warnings remain in the library (list: `cargo clippy -p cobolt-graphics`).
- The wasm sizes are upper bounds from a probe, not a player build.
